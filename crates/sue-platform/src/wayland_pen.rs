//! Pen position, pressure, tilt, barrel buttons, eraser end and cursor on
//! Wayland, read directly from `zwp_tablet_manager_v2` (version 2).
//!
//! `winit` does not bind Wayland's tablet protocol. Once Sue binds a
//! `zwp_tablet_seat_v2`, the compositor stops emulating `wl_pointer` events
//! for the stylus over Sue's surface and sends native `zwp_tablet_tool_v2`
//! frames instead. Each hardware frame can bundle `proximity_in`, `down`,
//! `motion`, `pressure`, `tilt`, `button`, `up` and `proximity_out` in any
//! order before `frame(time)`, so [`ToolRecord`] collects a frame's changes
//! and commits them together when `frame` arrives.

use std::os::fd::AsFd;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::fd::OwnedFd;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop, event_created_child,
};
use wayland_cursor::CursorTheme;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_dial_v2::{
    self, ZwpTabletPadDialV2,
};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_group_v2::{
    self, EVT_DIAL_OPCODE, EVT_RING_OPCODE, EVT_STRIP_OPCODE, ZwpTabletPadGroupV2,
};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_ring_v2::{
    self, ZwpTabletPadRingV2,
};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_strip_v2::{
    self, ZwpTabletPadStripV2,
};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::{
    self, EVT_GROUP_OPCODE, ZwpTabletPadV2,
};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::{
    self, EVT_PAD_ADDED_OPCODE, EVT_TABLET_ADDED_OPCODE, EVT_TOOL_ADDED_OPCODE, ZwpTabletSeatV2,
};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::{
    self, ButtonState as ToolButtonState, Type as ToolType, ZwpTabletToolV2,
};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_v2::{self, ZwpTabletV2};
use winit::window::Window;

use crate::stylus::{self, PenSource, Sample as StylusSample, SampleKind};

/// Linux `input-event-codes.h` button codes sent by `zwp_tablet_tool_v2.button`.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
/// First stylus barrel button: right-click / eyedropper, matching desktop
/// tablet defaults.
const BTN_STYLUS: u32 = 0x14b;
/// Second stylus barrel button: middle-click / pan.
const BTN_STYLUS2: u32 = 0x14c;
/// Third stylus button (eraser switch on pens that report it as a button).
const BTN_STYLUS3: u32 = 0x149;

/// Maximum `zwp_tablet_tool_v2.pressure` value in the protocol.
const MAX_RAW_PRESSURE: f32 = 65_535.0;

/// One Wayland window surface owned by Sue (the main window or a dialog /
/// floating palette viewport).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SurfaceTarget {
    pub surface_id: u32,
    pub viewport: egui::ViewportId,
    pub to_points: f32,
}

/// One committed action buffered between `poll` and `take_events`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BufferedEvent {
    Move {
        viewport: egui::ViewportId,
        pos: egui::Pos2,
        pressure: Option<f32>,
        tilt: [f32; 2],
        eraser: bool,
    },
    Button {
        viewport: egui::ViewportId,
        pos: egui::Pos2,
        button: egui::PointerButton,
        pressed: bool,
        pressure: Option<f32>,
        tilt: [f32; 2],
        eraser: bool,
    },
    Gone {
        viewport: egui::ViewportId,
    },
}

/// Changes accumulated for one tool before `zwp_tablet_tool_v2.frame`.
#[derive(Clone, Debug, Default)]
struct PendingFrame {
    /// `Some((serial, surface_protocol_id))` when `proximity_in` arrived.
    proximity_in: Option<(u32, u32)>,
    proximity_out: bool,
    motion: Option<[f32; 2]>,
    down: bool,
    up: bool,
    pressure: Option<f32>,
    tilt: Option<[f32; 2]>,
    buttons: Vec<(u32, bool)>,
}

/// Pure per-tool state machine, independent of Wayland proxy types so tests
/// can drive every edge case without a compositor.
#[derive(Clone, Debug, Default)]
struct ToolRecord {
    /// Physical tool is an eraser end (`ToolType::Eraser`).
    eraser_tool: bool,
    /// Eraser button held on a single-ended barrel-eraser pen.
    eraser_button: bool,
    /// Raw Wayland surface id the tool is currently in proximity over.
    entered_surface: Option<u32>,
    /// Sue viewport corresponding to `entered_surface`, when it belongs to Sue.
    in_viewport: Option<egui::ViewportId>,
    /// Scale factor from surface-local coordinates to `egui` points for `in_viewport`.
    viewport_to_points: f32,
    /// Latest `proximity_in` serial while in proximity.
    enter_serial: u32,
    /// Needs a `set_cursor` update after entering proximity.
    needs_cursor: bool,
    /// Last surface-local logical coordinates `(x, y)`.
    surface_pos: Option<[f32; 2]>,
    /// Nib is in logical contact (`down` without `up`).
    touching: bool,
    /// Latest contact pressure (`0.0..=1.0`), or `None` when hovering.
    pressure: Option<f32>,
    /// Latest `[tilt_x, tilt_y]` in degrees.
    tilt: [f32; 2],
    /// Currently pressed barrel buttons mapped to `egui::PointerButton`.
    held_secondary: bool,
    held_middle: bool,
    pending: PendingFrame,
}

impl ToolRecord {
    fn is_eraser(&self) -> bool {
        self.eraser_tool || self.eraser_button
    }

    /// Releases contact and held buttons and leaves the current Sue viewport cleanly.
    fn leave_window(&mut self, out: &mut Vec<BufferedEvent>) {
        let Some(viewport) = self.in_viewport.take() else {
            self.touching = false;
            self.pressure = None;
            self.tilt = [0.0, 0.0];
            self.held_secondary = false;
            self.held_middle = false;
            self.eraser_button = false;
            return;
        };
        let to_points = if self.viewport_to_points > 0.0 {
            self.viewport_to_points
        } else {
            1.0
        };
        let pos = self
            .surface_pos
            .map(|[x, y]| egui::pos2(x * to_points, y * to_points))
            .unwrap_or(egui::Pos2::ZERO);
        let eraser = self.is_eraser();
        if self.touching {
            self.touching = false;
            self.pressure = None;
            out.push(BufferedEvent::Button {
                viewport,
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                pressure: None,
                tilt: self.tilt,
                eraser,
            });
        }
        if self.held_secondary {
            self.held_secondary = false;
            out.push(BufferedEvent::Button {
                viewport,
                pos,
                button: egui::PointerButton::Secondary,
                pressed: false,
                pressure: self.pressure,
                tilt: self.tilt,
                eraser,
            });
        }
        if self.held_middle {
            self.held_middle = false;
            out.push(BufferedEvent::Button {
                viewport,
                pos,
                button: egui::PointerButton::Middle,
                pressed: false,
                pressure: self.pressure,
                tilt: self.tilt,
                eraser,
            });
        }
        self.needs_cursor = false;
        self.pressure = None;
        self.tilt = [0.0, 0.0];
        self.eraser_button = false;
        out.push(BufferedEvent::Gone { viewport });
    }

    /// Commits one `zwp_tablet_tool_v2.frame`, turning all events gathered
    /// since the previous `frame` into ordered [`BufferedEvent`]s.
    fn commit_frame(&mut self, surfaces: &[SurfaceTarget], out: &mut Vec<BufferedEvent>) {
        let frame = std::mem::take(&mut self.pending);

        if let Some((serial, surface_id)) = frame.proximity_in {
            self.entered_surface = Some(surface_id);
            self.enter_serial = serial;
            let matched = surfaces
                .iter()
                .find(|s| s.surface_id == surface_id)
                .copied();
            match matched {
                None => {
                    self.leave_window(out);
                }
                Some(target) => {
                    if self.in_viewport != Some(target.viewport) {
                        self.leave_window(out);
                        self.touching = false;
                        self.pressure = None;
                        self.held_secondary = false;
                        self.held_middle = false;
                    }
                    self.in_viewport = Some(target.viewport);
                    self.viewport_to_points = target.to_points;
                    self.needs_cursor = true;
                }
            }
        } else if let Some(surface_id) = self.entered_surface {
            // Keep `to_points` up to date on fractional scale changes, and
            // attach viewports whose `wl_surface` registered right after enter.
            if let Some(target) = surfaces
                .iter()
                .find(|s| s.surface_id == surface_id)
                .copied()
            {
                if self.in_viewport != Some(target.viewport) {
                    self.leave_window(out);
                    self.in_viewport = Some(target.viewport);
                    self.needs_cursor = true;
                }
                self.viewport_to_points = target.to_points;
            } else if self.in_viewport.is_some() {
                self.leave_window(out);
            }
        }

        let Some(viewport) = self.in_viewport else {
            if frame.proximity_out {
                self.entered_surface = None;
            }
            self.touching = false;
            self.pressure = None;
            return;
        };
        let to_points = self.viewport_to_points;

        if let Some(motion) = frame.motion {
            self.surface_pos = Some(motion);
        }
        if let Some(tilt) = frame.tilt {
            self.tilt = tilt;
        }
        if let Some(raw_pressure) = frame.pressure {
            self.pressure = Some(raw_pressure.clamp(0.0, 1.0));
        }

        for &(code, pressed) in &frame.buttons {
            if code == BTN_STYLUS3 {
                self.eraser_button = pressed;
            }
        }

        let pos = self
            .surface_pos
            .map(|[x, y]| egui::pos2(x * to_points, y * to_points));

        // Emit motion before `down` when the pen lands at a new position so
        // hover/hit-testing already knows where the nib touched.
        if (frame.motion.is_some() || frame.tilt.is_some() || frame.pressure.is_some())
            && !frame.down
            && !frame.up
            && let Some(pos) = pos
        {
            let active_pressure = if self.touching { self.pressure } else { None };
            out.push(BufferedEvent::Move {
                viewport,
                pos,
                pressure: active_pressure,
                tilt: self.tilt,
                eraser: self.is_eraser(),
            });
        } else if frame.motion.is_some()
            && frame.down
            && let Some(pos) = pos
        {
            out.push(BufferedEvent::Move {
                viewport,
                pos,
                pressure: None,
                tilt: self.tilt,
                eraser: self.is_eraser(),
            });
        }

        if frame.down {
            self.touching = true;
            // Some tablets send `down` with pressure > 0 in the next frame;
            // start with a light positive pressure if none arrived yet so the
            // stroke sees a pen rather than a mouse's full 1.0.
            let contact_pressure = Some(
                self.pressure
                    .filter(|&p| p > 0.0)
                    .unwrap_or(0.05)
                    .clamp(0.0, 1.0),
            );
            self.pressure = contact_pressure;
            if let Some(pos) = pos {
                out.push(BufferedEvent::Button {
                    viewport,
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    pressure: contact_pressure,
                    tilt: self.tilt,
                    eraser: self.is_eraser(),
                });
            }
        }

        for (code, pressed) in frame.buttons {
            let mapped = match code {
                BTN_LEFT => Some(egui::PointerButton::Primary),
                BTN_STYLUS | BTN_RIGHT => {
                    self.held_secondary = pressed;
                    Some(egui::PointerButton::Secondary)
                }
                BTN_STYLUS2 | BTN_MIDDLE => {
                    self.held_middle = pressed;
                    Some(egui::PointerButton::Middle)
                }
                _ => None,
            };
            if let (Some(button), Some(pos)) = (mapped, pos) {
                let active_pressure = if self.touching { self.pressure } else { None };
                out.push(BufferedEvent::Button {
                    viewport,
                    pos,
                    button,
                    pressed,
                    pressure: active_pressure,
                    tilt: self.tilt,
                    eraser: self.is_eraser(),
                });
            }
        }

        if frame.up {
            // If `motion` arrived in the same frame as `up`, deliver that last
            // point before lifting the button.
            if frame.motion.is_some()
                && !frame.down
                && let Some(pos) = pos
            {
                out.push(BufferedEvent::Move {
                    viewport,
                    pos,
                    pressure: self.pressure,
                    tilt: self.tilt,
                    eraser: self.is_eraser(),
                });
            }
            self.touching = false;
            self.pressure = None;
            if let Some(pos) = pos {
                out.push(BufferedEvent::Button {
                    viewport,
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    pressure: None,
                    tilt: self.tilt,
                    eraser: self.is_eraser(),
                });
            }
        }

        if frame.proximity_out {
            self.entered_surface = None;
            self.leave_window(out);
        }
    }
}

/// One live `ZwpTabletToolV2` paired with its own cursor `WlSurface`.
struct LiveTool {
    id: ObjectId,
    proxy: ZwpTabletToolV2,
    record: ToolRecord,
    /// `tablet-v2` forbids sharing a cursor `WlSurface` across tools: each
    /// tool owns its own surface.
    cursor_surface: Option<WlSurface>,
    applied_cursor: Option<(egui::CursorIcon, i32)>,
}

struct WaylandState {
    surfaces: Vec<SurfaceTarget>,
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    manager: Option<ZwpTabletManagerV2>,
    seats: Vec<(u32, WlSeat)>,
    tablet_seats: Vec<ZwpTabletSeatV2>,
    tools: Vec<LiveTool>,
    buffered: Vec<BufferedEvent>,
    cursor_theme: Option<(i32, CursorTheme)>,
}

impl WaylandState {
    fn new(root_surface_id: u32) -> Self {
        Self {
            surfaces: vec![SurfaceTarget {
                surface_id: root_surface_id,
                viewport: egui::ViewportId::ROOT,
                to_points: 1.0,
            }],
            compositor: None,
            shm: None,
            manager: None,
            seats: Vec::new(),
            tablet_seats: Vec::new(),
            tools: Vec::new(),
            buffered: Vec::new(),
            cursor_theme: None,
        }
    }

    fn tool_mut(&mut self, proxy: &ZwpTabletToolV2) -> &mut LiveTool {
        let id = proxy.id();
        if let Some(index) = self.tools.iter().position(|tool| tool.id == id) {
            return &mut self.tools[index];
        }
        self.tools.push(LiveTool {
            id,
            proxy: proxy.clone(),
            record: ToolRecord::default(),
            cursor_surface: None,
            applied_cursor: None,
        });
        let last = self.tools.len() - 1;
        &mut self.tools[last]
    }

    fn apply_cursors(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        mut cursor_for: impl FnMut(egui::ViewportId) -> (egui::CursorIcon, i32),
    ) {
        for tool in &mut self.tools {
            let Some(viewport) = tool.record.in_viewport else {
                continue;
            };
            let (icon, raw_scale) = cursor_for(viewport);
            let scale = raw_scale.max(1);
            if !tool.record.needs_cursor && tool.applied_cursor == Some((icon, scale)) {
                continue;
            }
            tool.record.needs_cursor = false;
            tool.applied_cursor = Some((icon, scale));

            if icon == egui::CursorIcon::None {
                tool.proxy.set_cursor(tool.record.enter_serial, None, 0, 0);
                continue;
            }

            let reload = self
                .cursor_theme
                .as_ref()
                .is_none_or(|(loaded_scale, _)| *loaded_scale != scale);
            if reload
                && let Some(shm) = self.shm.clone()
                && let Ok(theme) = CursorTheme::load(conn, shm, (24 * scale) as u32)
            {
                self.cursor_theme = Some((scale, theme));
            }

            let Some(compositor) = self.compositor.as_ref() else {
                continue;
            };
            let surface = tool
                .cursor_surface
                .get_or_insert_with(|| compositor.create_surface(qh, ()));

            let Some((_, theme)) = self.cursor_theme.as_mut() else {
                continue;
            };
            let mut matched = None;
            for &name in cursor_names(icon) {
                if theme.get_cursor(name).is_some() {
                    matched = Some(name);
                    break;
                }
            }
            let Some(cursor) = matched.and_then(|name| theme.get_cursor(name)) else {
                continue;
            };
            let image = &cursor[0];
            let (width, height) = image.dimensions();
            let (hot_x, hot_y) = image.hotspot();
            if surface.version() >= 3 {
                surface.set_buffer_scale(scale);
            }
            surface.attach(Some(image), 0, 0);
            if surface.version() >= 4 {
                surface.damage_buffer(0, 0, width as i32, height as i32);
            } else {
                surface.damage(0, 0, i32::MAX, i32::MAX);
            }
            surface.commit();
            tool.proxy.set_cursor(
                tool.record.enter_serial,
                Some(surface),
                hot_x as i32 / scale,
                hot_y as i32 / scale,
            );
        }
    }
}

fn cursor_names(icon: egui::CursorIcon) -> &'static [&'static str] {
    match icon {
        egui::CursorIcon::Crosshair => &["crosshair", "tcross", "default", "left_ptr"],
        egui::CursorIcon::Grab => &["openhand", "grab", "hand1", "default", "left_ptr"],
        egui::CursorIcon::Grabbing => &["closedhand", "grabbing", "dnd-move", "default"],
        egui::CursorIcon::Move => &["move", "fleur", "size_all", "default", "left_ptr"],
        egui::CursorIcon::ZoomIn => &["zoom-in", "zoom_in", "default", "left_ptr"],
        egui::CursorIcon::ZoomOut => &["zoom-out", "zoom_out", "default", "left_ptr"],
        egui::CursorIcon::PointingHand => &["pointer", "hand2", "pointing_hand", "default"],
        egui::CursorIcon::Text | egui::CursorIcon::VerticalText => {
            &["text", "xterm", "ibeam", "default"]
        }
        egui::CursorIcon::ResizeHorizontal
        | egui::CursorIcon::ResizeEast
        | egui::CursorIcon::ResizeWest
        | egui::CursorIcon::ResizeColumn => &["ew-resize", "sb_h_double_arrow", "default"],
        egui::CursorIcon::ResizeVertical
        | egui::CursorIcon::ResizeNorth
        | egui::CursorIcon::ResizeSouth
        | egui::CursorIcon::ResizeRow => &["ns-resize", "sb_v_double_arrow", "default"],
        egui::CursorIcon::ResizeNeSw
        | egui::CursorIcon::ResizeNorthEast
        | egui::CursorIcon::ResizeSouthWest => &["nesw-resize", "size_bdiag", "default"],
        egui::CursorIcon::ResizeNwSe
        | egui::CursorIcon::ResizeNorthWest
        | egui::CursorIcon::ResizeSouthEast => &["nwse-resize", "size_fdiag", "default"],
        egui::CursorIcon::NotAllowed | egui::CursorIcon::NoDrop => {
            &["not-allowed", "crossed_circle", "default"]
        }
        egui::CursorIcon::Wait | egui::CursorIcon::Progress => {
            &["wait", "watch", "progress", "default"]
        }
        _ => &["default", "left_ptr", "arrow"],
    }
}

impl Dispatch<WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "wl_compositor" if state.compositor.is_none() => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
                }
                "wl_shm" if state.shm.is_none() => {
                    state.shm = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "wl_seat" => {
                    let seat: WlSeat = registry.bind(name, version.min(8), qh, ());
                    if let Some(manager) = state.manager.as_ref() {
                        state
                            .tablet_seats
                            .push(manager.get_tablet_seat(&seat, qh, ()));
                    }
                    state.seats.push((name, seat));
                }
                "zwp_tablet_manager_v2" if state.manager.is_none() => {
                    // Bind up to version 2 (supporting `zwp_tablet_v2.bustype`
                    // and `zwp_tablet_pad_dial_v2` on KDE Plasma 6 and newer
                    // compositors without overflowing protocol definitions).
                    let manager: ZwpTabletManagerV2 = registry.bind(name, version.min(2), qh, ());
                    for (_, seat) in &state.seats {
                        state
                            .tablet_seats
                            .push(manager.get_tablet_seat(seat, qh, ()));
                    }
                    state.manager = Some(manager);
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { name } => {
                state.seats.retain(|(seat_name, _)| *seat_name != name);
            }
            _ => {}
        }
    }
}

delegate_noop!(WaylandState: ignore WlCompositor);
delegate_noop!(WaylandState: ignore WlShm);
delegate_noop!(WaylandState: ignore WlSeat);
delegate_noop!(WaylandState: ignore WlSurface);
delegate_noop!(WaylandState: ignore ZwpTabletManagerV2);

impl Dispatch<ZwpTabletSeatV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &ZwpTabletSeatV2,
        event: zwp_tablet_seat_v2::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwp_tablet_seat_v2::Event::ToolAdded { id } = event {
            let _ = state.tool_mut(&id);
        }
    }

    event_created_child!(WaylandState, ZwpTabletSeatV2, [
        EVT_TABLET_ADDED_OPCODE => (ZwpTabletV2, ()),
        EVT_TOOL_ADDED_OPCODE => (ZwpTabletToolV2, ()),
        EVT_PAD_ADDED_OPCODE => (ZwpTabletPadV2, ()),
    ]);
}

impl Dispatch<ZwpTabletV2, ()> for WaylandState {
    fn event(
        _: &mut Self,
        tablet: &ZwpTabletV2,
        event: zwp_tablet_v2::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, zwp_tablet_v2::Event::Removed) {
            tablet.destroy();
        }
    }
}

impl Dispatch<ZwpTabletToolV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &ZwpTabletToolV2,
        event: zwp_tablet_tool_v2::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_tablet_tool_v2::Event::Type {
                tool_type: WEnum::Value(ToolType::Eraser),
            } => {
                state.tool_mut(proxy).record.eraser_tool = true;
            }
            zwp_tablet_tool_v2::Event::ProximityIn {
                serial, surface, ..
            } => {
                let sid = surface.id().protocol_id();
                state.tool_mut(proxy).record.pending.proximity_in = Some((serial, sid));
            }
            zwp_tablet_tool_v2::Event::ProximityOut => {
                state.tool_mut(proxy).record.pending.proximity_out = true;
            }
            zwp_tablet_tool_v2::Event::Down { .. } => {
                state.tool_mut(proxy).record.pending.down = true;
            }
            zwp_tablet_tool_v2::Event::Up => {
                state.tool_mut(proxy).record.pending.up = true;
            }
            zwp_tablet_tool_v2::Event::Motion { x, y } => {
                state.tool_mut(proxy).record.pending.motion = Some([x as f32, y as f32]);
            }
            zwp_tablet_tool_v2::Event::Pressure { pressure } => {
                state.tool_mut(proxy).record.pending.pressure =
                    Some((pressure as f32 / MAX_RAW_PRESSURE).clamp(0.0, 1.0));
            }
            zwp_tablet_tool_v2::Event::Tilt { tilt_x, tilt_y } => {
                state.tool_mut(proxy).record.pending.tilt = Some([tilt_x as f32, tilt_y as f32]);
            }
            zwp_tablet_tool_v2::Event::Button {
                button,
                state: WEnum::Value(btn_state),
                ..
            } => {
                let pressed = matches!(btn_state, ToolButtonState::Pressed);
                state
                    .tool_mut(proxy)
                    .record
                    .pending
                    .buttons
                    .push((button, pressed));
            }
            zwp_tablet_tool_v2::Event::Frame { .. } => {
                let id = proxy.id();
                if let Some(index) = state.tools.iter().position(|tool| tool.id == id) {
                    let surfaces = state.surfaces.clone();
                    let mut buffered = std::mem::take(&mut state.buffered);
                    state.tools[index]
                        .record
                        .commit_frame(&surfaces, &mut buffered);
                    state.buffered = buffered;
                }
            }
            zwp_tablet_tool_v2::Event::Removed => {
                let id = proxy.id();
                if let Some(index) = state.tools.iter().position(|tool| tool.id == id) {
                    let mut removed = state.tools.swap_remove(index);
                    removed.record.leave_window(&mut state.buffered);
                    if let Some(surface) = removed.cursor_surface.take() {
                        surface.destroy();
                    }
                }
                proxy.destroy();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpTabletPadV2, ()> for WaylandState {
    fn event(
        _: &mut Self,
        pad: &ZwpTabletPadV2,
        event: zwp_tablet_pad_v2::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, zwp_tablet_pad_v2::Event::Removed) {
            pad.destroy();
        }
    }

    event_created_child!(WaylandState, ZwpTabletPadV2, [
        EVT_GROUP_OPCODE => (ZwpTabletPadGroupV2, ()),
    ]);
}

impl Dispatch<ZwpTabletPadGroupV2, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwpTabletPadGroupV2,
        _: zwp_tablet_pad_group_v2::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(WaylandState, ZwpTabletPadGroupV2, [
        EVT_RING_OPCODE => (ZwpTabletPadRingV2, ()),
        EVT_STRIP_OPCODE => (ZwpTabletPadStripV2, ()),
        EVT_DIAL_OPCODE => (ZwpTabletPadDialV2, ()),
    ]);
}

impl Dispatch<ZwpTabletPadRingV2, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwpTabletPadRingV2,
        _: zwp_tablet_pad_ring_v2::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTabletPadStripV2, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwpTabletPadStripV2,
        _: zwp_tablet_pad_strip_v2::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTabletPadDialV2, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &ZwpTabletPadDialV2,
        _: zwp_tablet_pad_dial_v2::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// Converts buffered tablet events into `egui::Event`s routed by `ViewportId`
/// and synchronized [`stylus::Sample`]s in lockstep.
fn flush_buffered(
    buffered: &mut Vec<BufferedEvent>,
    modifiers: egui::Modifiers,
    mut route: impl FnMut(egui::ViewportId, egui::Event),
) {
    for item in buffered.drain(..) {
        match item {
            BufferedEvent::Move {
                viewport,
                pos,
                pressure,
                tilt,
                eraser,
            } => {
                stylus::push_sample(
                    PenSource::Wayland,
                    StylusSample {
                        kind: SampleKind::Move,
                        pos: [pos.x, pos.y],
                        pressure,
                        tilt,
                        eraser,
                    },
                );
                route(viewport, egui::Event::PointerMoved(pos));
            }
            BufferedEvent::Button {
                viewport,
                pos,
                button,
                pressed,
                pressure,
                tilt,
                eraser,
            } => {
                stylus::push_sample(
                    PenSource::Wayland,
                    StylusSample {
                        kind: if pressed {
                            SampleKind::Press
                        } else {
                            SampleKind::Release
                        },
                        pos: [pos.x, pos.y],
                        pressure,
                        tilt,
                        eraser,
                    },
                );
                route(
                    viewport,
                    egui::Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers,
                    },
                );
            }
            BufferedEvent::Gone { viewport } => {
                stylus::set_state(PenSource::Wayland, None, [0.0, 0.0], false);
                route(viewport, egui::Event::PointerGone);
            }
        }
    }
}

#[derive(Default)]
struct WakeFlags {
    armed: bool,
    polling: bool,
    stop: bool,
}

/// Watches the shared `wl_display` socket fd while `winit` sleeps in
/// `ControlFlow::Wait` or `ControlFlow::WaitUntil` without calling any
/// `libwayland` functions, nudging `winit`'s `EventLoopProxy` when a
/// `zwp_tablet_tool_v2` packet arrives so `winit` does not ignore the wakeup
/// as spurious when its own `wl_pointer` queue is empty.
struct IdleWake {
    state: Arc<(Mutex<WakeFlags>, Condvar)>,
    stop_write: OwnedFd,
    thread: Option<JoinHandle<()>>,
}

impl IdleWake {
    fn spawn(conn: Connection, proxy: winit::event_loop::EventLoopProxy<()>) -> Option<Self> {
        let (stop_read, stop_write) = rustix::pipe::pipe().ok()?;
        let state = Arc::new((Mutex::new(WakeFlags::default()), Condvar::new()));
        let shared = Arc::clone(&state);
        let thread = std::thread::Builder::new()
            .name("sue-wayland-tablet".to_owned())
            .spawn(move || {
                let (lock, cvar) = &*shared;
                loop {
                    let mut flags = lock.lock().unwrap_or_else(PoisonError::into_inner);
                    while !flags.armed && !flags.stop {
                        flags = cvar.wait(flags).unwrap_or_else(PoisonError::into_inner);
                    }
                    if flags.stop {
                        break;
                    }
                    flags.armed = false;
                    flags.polling = true;
                    drop(flags);

                    let mut fds = [
                        PollFd::from_borrowed_fd(conn.as_fd(), PollFlags::IN),
                        PollFd::from_borrowed_fd(stop_read.as_fd(), PollFlags::IN),
                    ];
                    let _ = rustix::event::poll(&mut fds, None);

                    let mut flags = lock.lock().unwrap_or_else(PoisonError::into_inner);
                    flags.polling = false;
                    if flags.stop || !fds[1].revents().is_empty() {
                        break;
                    }
                    drop(flags);

                    if !fds[0].revents().is_empty() {
                        let _ = proxy.send_event(());
                    }
                }
            })
            .ok()?;

        Some(Self {
            state,
            stop_write,
            thread: Some(thread),
        })
    }

    fn arm(&self) {
        let (lock, cvar) = &*self.state;
        let mut flags = lock.lock().unwrap_or_else(PoisonError::into_inner);
        if !flags.polling && !flags.stop {
            flags.armed = true;
            cvar.notify_one();
        }
    }
}

impl Drop for IdleWake {
    fn drop(&mut self) {
        {
            let (lock, cvar) = &*self.state;
            let mut flags = lock.lock().unwrap_or_else(PoisonError::into_inner);
            flags.stop = true;
            cvar.notify_one();
        }
        let _ = rustix::io::write(&self.stop_write, &[1u8]);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A connection to Wayland's `tablet-v2` protocol sharing `winit`'s display.
///
/// Field drop order matters: `idle_wake` joins before `queue` and `window_conn`
/// drop, while `window`'s `wl_display` is still alive.
pub(crate) struct Pen {
    idle_wake: Option<IdleWake>,
    queue: EventQueue<WaylandState>,
    qh: QueueHandle<WaylandState>,
    state: WaylandState,
    window_conn: sue_wayland::WindowConnection,
    cursor_icon: egui::CursorIcon,
}

impl Pen {
    /// Opens a `tablet-v2` event queue on `window`'s Wayland connection, or
    /// returns `None` on X11 or when the compositor exposes no tablet manager.
    pub(crate) fn open(
        window: &Arc<Window>,
        proxy: winit::event_loop::EventLoopProxy<()>,
    ) -> Option<Self> {
        let window_conn = sue_wayland::WindowConnection::open(window)?;
        let conn = window_conn.connection();
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        let mut state = WaylandState::new(window_conn.surface_id());
        let _registry = conn.display().get_registry(&qh, ());

        // Discover globals and bind `zwp_tablet_manager_v2`.
        queue.roundtrip(&mut state).ok()?;
        state.manager.as_ref()?;
        // Receive the initial tablet, tool (including Eraser tool types) and
        // pad announcements from each bound `zwp_tablet_seat_v2`.
        let _ = queue.roundtrip(&mut state);

        let idle_wake = IdleWake::spawn(conn.clone(), proxy);

        Some(Self {
            idle_wake,
            queue,
            qh,
            state,
            window_conn,
            cursor_icon: egui::CursorIcon::Default,
        })
    }

    /// The Wayland protocol id (`wl_surface@...`) of the main window.
    pub(crate) fn root_surface_id(&self) -> u32 {
        self.window_conn.surface_id()
    }

    /// Reads any pending Wayland tablet frames without blocking. Returns
    /// `true` when new pointer events are waiting for a frame.
    ///
    /// Note: `libwayland-client` requires checking `poll(POLLIN)` before
    /// calling `ReadEventsGuard::read()`, otherwise `wl_display_read_events`
    /// blocks on `pthread_cond_wait` whenever another reader (such as the
    /// Vulkan Wayland WSI thread) holds a prepared read guard on an empty
    /// socket.
    pub(crate) fn poll(&mut self, surfaces: Vec<SurfaceTarget>, root_scale: i32) -> bool {
        self.state.surfaces = surfaces;
        let _ = self.queue.dispatch_pending(&mut self.state);
        while let Some(guard) = self.queue.prepare_read() {
            let zero = Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            let mut fds = [PollFd::from_borrowed_fd(
                guard.connection_fd(),
                PollFlags::IN,
            )];
            if rustix::event::poll(&mut fds, Some(&zero)).is_ok_and(|ready| ready > 0)
                && !fds[0].revents().is_empty()
            {
                let _ = guard.read();
                let _ = self.queue.dispatch_pending(&mut self.state);
            } else {
                // Dropping `guard` calls `wl_display_cancel_read` immediately.
                break;
            }
        }
        let icon = self.cursor_icon;
        self.state
            .apply_cursors(self.window_conn.connection(), &self.qh, |_| {
                (icon, root_scale)
            });
        let _ = self.queue.flush();
        !self.state.buffered.is_empty()
    }

    /// Arms the zero-CPU socket watcher before `winit` blocks in `Wait` or
    /// `WaitUntil`.
    pub(crate) fn arm_wake(&self) {
        if let Some(wake) = self.idle_wake.as_ref() {
            wake.arm();
        }
    }

    /// Updates the cursor icon shown for any tablet tool currently in
    /// proximity over any of Sue's windows.
    pub(crate) fn set_cursors(
        &mut self,
        root_icon: egui::CursorIcon,
        root_scale: i32,
        mut child_cursor: impl FnMut(egui::ViewportId) -> Option<(egui::CursorIcon, i32)>,
    ) {
        self.cursor_icon = root_icon;
        self.state
            .apply_cursors(self.window_conn.connection(), &self.qh, |viewport| {
                if viewport == egui::ViewportId::ROOT {
                    (root_icon, root_scale)
                } else {
                    child_cursor(viewport).unwrap_or((root_icon, root_scale))
                }
            });
        let _ = self.queue.flush();
    }

    /// Routes buffered pointer events to their target `ViewportId` and pushes
    /// their matching per-sample pressure, tilt and eraser readings to
    /// [`crate::stylus`].
    pub(crate) fn take_events(
        &mut self,
        modifiers: egui::Modifiers,
        route: impl FnMut(egui::ViewportId, egui::Event),
    ) {
        flush_buffered(&mut self.state.buffered, modifiers, route);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_target(to_points: f32) -> [SurfaceTarget; 1] {
        [SurfaceTarget {
            surface_id: 30,
            viewport: egui::ViewportId::ROOT,
            to_points,
        }]
    }

    #[test]
    fn frame_commits_down_with_same_frame_motion_pressure_and_tilt() {
        let _guard = stylus::lock_for_test();
        stylus::end_frame();
        let surfaces = root_target(0.5);
        let mut tool = ToolRecord::default();
        let mut buffered = Vec::new();

        // KWin sends `proximity_in` + `motion` + `tilt` in the first frame.
        tool.pending.proximity_in = Some((42, 30));
        tool.pending.motion = Some([100.0, 200.0]);
        tool.pending.tilt = Some([12.5, -8.0]);
        tool.commit_frame(&surfaces, &mut buffered);
        assert_eq!(tool.in_viewport, Some(egui::ViewportId::ROOT));
        assert!(tool.needs_cursor);
        assert_eq!(tool.enter_serial, 42);

        // Next frame: `down` arrives BEFORE `motion` and `pressure` inside the
        // same hardware frame.
        tool.pending.down = true;
        tool.pending.motion = Some([110.0, 220.0]);
        tool.pending.pressure = Some(0.4);
        tool.pending.tilt = Some([15.0, -10.0]);
        tool.commit_frame(&surfaces, &mut buffered);

        // Next two stroke frames at 200 Hz with distinct pressures and tilts.
        tool.pending.motion = Some([130.0, 240.0]);
        tool.pending.pressure = Some(0.75);
        tool.pending.tilt = Some([20.0, -12.0]);
        tool.commit_frame(&surfaces, &mut buffered);

        tool.pending.up = true;
        tool.pending.proximity_out = true;
        tool.commit_frame(&surfaces, &mut buffered);

        let mut events = Vec::new();
        flush_buffered(&mut buffered, egui::Modifiers::NONE, |vp, ev| {
            assert_eq!(vp, egui::ViewportId::ROOT);
            events.push(ev);
        });
        let samples = stylus::take_samples();

        assert_eq!(
            events,
            vec![
                egui::Event::PointerMoved(egui::pos2(50.0, 100.0)),
                egui::Event::PointerMoved(egui::pos2(55.0, 110.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(55.0, 110.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(egui::pos2(65.0, 120.0)),
                egui::Event::PointerButton {
                    pos: egui::pos2(65.0, 120.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerGone,
            ]
        );
        // Every non-Gone event has a matching queued stylus sample with its
        // exact pressure and tilt, even though the batch ended with ProximityOut.
        assert_eq!(samples.len(), 5);
        assert_eq!(samples[0].pressure, None);
        assert_eq!(samples[2].kind, SampleKind::Press);
        assert_eq!(samples[2].pos, [55.0, 110.0]);
        assert_eq!(samples[2].pressure, Some(0.4));
        assert_eq!(samples[2].tilt, [15.0, -10.0]);
        assert_eq!(samples[3].kind, SampleKind::Move);
        assert_eq!(samples[3].pressure, Some(0.75));
        assert_eq!(samples[3].tilt, [20.0, -12.0]);
        assert_eq!(samples[4].kind, SampleKind::Release);
        assert_eq!(samples[4].pressure, None);
        assert_eq!(stylus::pressure(), None);
        stylus::end_frame();
    }

    #[test]
    fn barrel_buttons_and_dialog_viewports_route_cleanly() {
        let _guard = stylus::lock_for_test();
        stylus::end_frame();
        let dialog_vp = egui::ViewportId::from_hash_of("pen-pressure-dialog");
        let surfaces = [
            SurfaceTarget {
                surface_id: 30,
                viewport: egui::ViewportId::ROOT,
                to_points: 1.0,
            },
            SurfaceTarget {
                surface_id: 77,
                viewport: dialog_vp,
                to_points: 1.0,
            },
        ];
        let mut tool = ToolRecord {
            eraser_tool: true,
            ..ToolRecord::default()
        };
        let mut buffered = Vec::new();

        // Events on a foreign app's surface (id 999) are ignored.
        tool.pending.proximity_in = Some((1, 999));
        tool.pending.motion = Some([10.0, 10.0]);
        tool.pending.down = true;
        tool.commit_frame(&surfaces, &mut buffered);
        assert!(buffered.is_empty());

        // Entering the main window and pressing both barrel buttons.
        tool.pending.proximity_in = Some((2, 30));
        tool.pending.motion = Some([30.0, 40.0]);
        tool.pending.buttons = vec![(BTN_STYLUS, true), (BTN_STYLUS2, true)];
        tool.commit_frame(&surfaces, &mut buffered);

        // Moving onto the Pen Pressure Settings dialog (id 77) releases held
        // buttons + Gone on ROOT, then delivers motion + press to `dialog_vp`.
        tool.pending.proximity_in = Some((3, 77));
        tool.pending.motion = Some([15.0, 25.0]);
        tool.pending.down = true;
        tool.pending.pressure = Some(0.6);
        tool.commit_frame(&surfaces, &mut buffered);

        let mut routed = Vec::new();
        flush_buffered(&mut buffered, egui::Modifiers::ALT, |vp, ev| {
            routed.push((vp, ev));
        });
        let samples = stylus::take_samples();

        assert!(samples.iter().take(3).all(|sample| sample.eraser));
        assert_eq!(routed[0].0, egui::ViewportId::ROOT);
        assert_eq!(
            routed[5],
            (egui::ViewportId::ROOT, egui::Event::PointerGone)
        );
        assert_eq!(
            routed[6],
            (dialog_vp, egui::Event::PointerMoved(egui::pos2(15.0, 25.0)))
        );
        assert_eq!(
            routed[7],
            (
                dialog_vp,
                egui::Event::PointerButton {
                    pos: egui::pos2(15.0, 25.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::ALT,
                }
            )
        );
        stylus::end_frame();
    }
}
