//! Pen pressure, tilt and eraser detection on X11 via XInput 2.2 (`x11rb`).
//!
//! On X11 (and Xwayland when an app is forced onto X11), `winit` already
//! delivers pointer motion and button clicks for drawing tablets, so the pen
//! moves and clicks out of the box, but `winit` discards XInput2's extended
//! valuators (`Abs Pressure`, `Abs Tilt X`, `Abs Tilt Y`) and device names
//! (`... eraser`). Opening a lightweight pure-Rust `x11rb` connection on the
//! same X11 display and selecting `XIEventMask` on Sue's window XID lets Sue
//! read pressure, tilt and eraser state in 100% safe Rust without disturbing
//! `winit`'s own X11 event stream.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{
    ConnectionExt as _, Device, DeviceClassData, DeviceClassDataValuator, EventMask, Fp3232,
    XIEventMask,
};
use x11rb::protocol::xproto::{Atom, ConnectionExt as _};
use x11rb::rust_connection::RustConnection;

use crate::stylus::{self, PenSource};

/// Normalized mapping for one XInput2 valuator axis.
#[derive(Clone, Copy, Debug, PartialEq)]
struct AxisRange {
    number: u16,
    min: f64,
    max: f64,
    /// Units per radian when the server reports angular resolution, else 0.
    resolution: u32,
}

impl AxisRange {
    fn from_valuator(val: &DeviceClassDataValuator) -> Self {
        Self {
            number: val.number,
            min: fp3232_to_f64(val.min),
            max: fp3232_to_f64(val.max),
            resolution: val.resolution,
        }
    }

    /// Maps raw valuator reading to `0.0..=1.0` pressure.
    fn normalize_pressure(&self, raw: f64) -> f32 {
        let span = self.max - self.min;
        if span <= 1e-9 {
            return 0.0;
        }
        ((raw - self.min) / span).clamp(0.0, 1.0) as f32
    }

    /// Maps raw valuator reading to tilt in degrees (`-90.0..=90.0`).
    ///
    /// Linux `xf86-input-libinput` and `xf86-input-wacom` either report tilt
    /// directly in degrees (`min = -60..=-90`, `max = 60..=90`, `resolution = 0`
    /// or `57`/`58` units/rad) or in radians * `resolution`.
    fn normalize_tilt_degrees(&self, raw: f64) -> f32 {
        let deg = if self.resolution > 0 && self.max.abs() > 90.5 {
            (raw / f64::from(self.resolution)).to_degrees()
        } else if self.max > self.min && self.max.abs() > 90.5 {
            let mid = 0.5 * (self.min + self.max);
            let half = 0.5 * (self.max - self.min);
            ((raw - mid) / half) * 60.0
        } else {
            raw
        };
        deg.clamp(-90.0, 90.0) as f32
    }
}

#[derive(Clone, Debug, Default)]
struct DeviceAxes {
    pressure: Option<AxisRange>,
    tilt_x: Option<AxisRange>,
    tilt_y: Option<AxisRange>,
    is_eraser: bool,
    last_pressure: f32,
    last_tilt: [f32; 2],
}

impl DeviceAxes {
    fn is_stylus(&self) -> bool {
        self.pressure.is_some() || self.tilt_x.is_some() || self.tilt_y.is_some() || self.is_eraser
    }

    fn update_from_valuators(&mut self, mask: &[u32], values: &[Fp3232]) {
        let mut cursor = 0usize;
        for (word_idx, &word) in mask.iter().enumerate() {
            if word == 0 {
                continue;
            }
            for bit in 0..32u16 {
                if word & (1u32 << bit) == 0 {
                    continue;
                }
                let Some(&raw_fp) = values.get(cursor) else {
                    return;
                };
                cursor += 1;
                let axis = (word_idx as u16) * 32 + bit;
                let raw = fp3232_to_f64(raw_fp);
                if let Some(p) = self.pressure
                    && p.number == axis
                {
                    self.last_pressure = p.normalize_pressure(raw);
                }
                if let Some(tx) = self.tilt_x
                    && tx.number == axis
                {
                    self.last_tilt[0] = tx.normalize_tilt_degrees(raw);
                }
                if let Some(ty) = self.tilt_y
                    && ty.number == axis
                {
                    self.last_tilt[1] = ty.normalize_tilt_degrees(raw);
                }
            }
        }
    }
}

fn fp3232_to_f64(value: Fp3232) -> f64 {
    f64::from(value.integral) + f64::from(value.frac) / 4_294_967_296.0
}

fn primary_pressed(button_mask: &[u32], pressed_now: Option<(u32, bool)>) -> bool {
    // X11 buttons are 1-indexed: button 1 (primary) is bit 1 of word 0.
    let mask_bit = button_mask.first().copied().unwrap_or(0) & (1 << 1) != 0;
    match pressed_now {
        Some((1, true)) => true,
        Some((1, false)) => false,
        _ => mask_bit,
    }
}

/// Safe XInput2 tablet listener attached to X11 windows.
pub(crate) struct Tablet {
    conn: RustConnection,
    atom_pressure: Atom,
    atom_tilt_x: Atom,
    atom_tilt_y: Atom,
    devices: HashMap<u16, DeviceAxes>,
    watched_windows: HashSet<u32>,
    touching: bool,
}

fn x11_window_id(window: &Window) -> Option<u32> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Xlib(h) => u32::try_from(h.window).ok(),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    }
}

impl Tablet {
    pub(crate) fn open(window: &Arc<Window>) -> Option<Self> {
        let window_xid = x11_window_id(window)?;

        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let version = conn.xinput_xi_query_version(2, 2).ok()?.reply().ok()?;
        if version.major_version < 2 {
            return None;
        }

        let intern = |name: &[u8]| -> Option<Atom> {
            Some(conn.intern_atom(false, name).ok()?.reply().ok()?.atom)
        };
        let atom_pressure = intern(b"Abs Pressure")?;
        let atom_tilt_x = intern(b"Abs Tilt X")?;
        let atom_tilt_y = intern(b"Abs Tilt Y")?;

        conn.xinput_xi_select_events(
            window_xid,
            &[EventMask {
                deviceid: Device::ALL_MASTER.into(),
                mask: vec![
                    XIEventMask::MOTION
                        | XIEventMask::BUTTON_PRESS
                        | XIEventMask::BUTTON_RELEASE
                        | XIEventMask::DEVICE_CHANGED
                        | XIEventMask::LEAVE,
                ],
            }],
        )
        .ok()?;

        if let Some(screen) = conn.setup().roots.get(screen_num) {
            let _ = conn.xinput_xi_select_events(
                screen.root,
                &[EventMask {
                    deviceid: Device::ALL.into(),
                    mask: vec![XIEventMask::HIERARCHY],
                }],
            );
        }
        conn.flush().ok()?;

        let mut watched_windows = HashSet::new();
        watched_windows.insert(window_xid);
        let mut tablet = Self {
            conn,
            atom_pressure,
            atom_tilt_x,
            atom_tilt_y,
            devices: HashMap::new(),
            watched_windows,
            touching: false,
        };
        tablet.refresh_devices();
        Some(tablet)
    }

    /// Subscribes to XInput2 valuator events on an additional dialog or
    /// floating-palette X11 window once.
    pub(crate) fn watch_window(&mut self, window: &Window) {
        let Some(window_xid) = x11_window_id(window) else {
            return;
        };
        if !self.watched_windows.insert(window_xid) {
            return;
        }
        if self
            .conn
            .xinput_xi_select_events(
                window_xid,
                &[EventMask {
                    deviceid: Device::ALL_MASTER.into(),
                    mask: vec![
                        XIEventMask::MOTION
                            | XIEventMask::BUTTON_PRESS
                            | XIEventMask::BUTTON_RELEASE
                            | XIEventMask::DEVICE_CHANGED
                            | XIEventMask::LEAVE,
                    ],
                }],
            )
            .is_ok()
        {
            let _ = self.conn.flush();
        }
    }

    fn refresh_devices(&mut self) {
        let Ok(cookie) = self.conn.xinput_xi_query_device(Device::ALL) else {
            return;
        };
        let Ok(reply) = cookie.reply() else {
            return;
        };
        let mut updated = HashMap::new();
        for info in reply.infos {
            let name = String::from_utf8_lossy(&info.name).to_ascii_lowercase();
            let mut axes = DeviceAxes {
                is_eraser: name.contains("eraser"),
                ..DeviceAxes::default()
            };
            for class in &info.classes {
                if let DeviceClassData::Valuator(val) = &class.data {
                    let range = AxisRange::from_valuator(val);
                    let current = fp3232_to_f64(val.value);
                    if val.label == self.atom_pressure {
                        axes.last_pressure = range.normalize_pressure(current);
                        axes.pressure = Some(range);
                    } else if val.label == self.atom_tilt_x {
                        axes.last_tilt[0] = range.normalize_tilt_degrees(current);
                        axes.tilt_x = Some(range);
                    } else if val.label == self.atom_tilt_y {
                        axes.last_tilt[1] = range.normalize_tilt_degrees(current);
                        axes.tilt_y = Some(range);
                    }
                }
            }
            if let Some(prev) = self.devices.get(&info.deviceid) {
                if axes.pressure.is_some() && prev.pressure.is_some() {
                    axes.last_pressure = prev.last_pressure;
                }
                axes.last_tilt = prev.last_tilt;
            }
            updated.insert(info.deviceid, axes);
        }
        self.devices = updated;
    }

    fn handle_pointer_event(
        &mut self,
        deviceid: u16,
        sourceid: u16,
        button_mask: &[u32],
        valuator_mask: &[u32],
        axisvalues: &[Fp3232],
        pressed_now: Option<(u32, bool)>,
    ) {
        let lookup_id = if self.devices.contains_key(&sourceid) {
            sourceid
        } else {
            deviceid
        };
        let Some(dev) = self.devices.get_mut(&lookup_id) else {
            return;
        };
        if !dev.is_stylus() {
            if self.touching {
                self.touching = false;
                stylus::set_state(PenSource::X11, None, [0.0, 0.0], false);
            }
            return;
        }
        dev.update_from_valuators(valuator_mask, axisvalues);
        let down = primary_pressed(button_mask, pressed_now);
        self.touching = down;
        let pressure = down.then(|| {
            if dev.pressure.is_some() {
                dev.last_pressure
            } else {
                1.0
            }
        });
        stylus::set_state(PenSource::X11, pressure, dev.last_tilt, dev.is_eraser);
    }

    pub(crate) fn poll(&mut self) {
        let mut refresh = false;
        while let Ok(Some(event)) = self.conn.poll_for_event() {
            match event {
                Event::XinputMotion(ev) => {
                    self.handle_pointer_event(
                        ev.deviceid,
                        ev.sourceid,
                        &ev.button_mask,
                        &ev.valuator_mask,
                        &ev.axisvalues,
                        None,
                    );
                }
                Event::XinputButtonPress(ev) => {
                    self.handle_pointer_event(
                        ev.deviceid,
                        ev.sourceid,
                        &ev.button_mask,
                        &ev.valuator_mask,
                        &ev.axisvalues,
                        Some((ev.detail, true)),
                    );
                }
                Event::XinputButtonRelease(ev) => {
                    self.handle_pointer_event(
                        ev.deviceid,
                        ev.sourceid,
                        &ev.button_mask,
                        &ev.valuator_mask,
                        &ev.axisvalues,
                        Some((ev.detail, false)),
                    );
                }
                Event::XinputLeave(_) => {
                    if self.touching {
                        self.touching = false;
                    }
                    stylus::set_state(PenSource::X11, None, [0.0, 0.0], false);
                }
                Event::XinputDeviceChanged(_) | Event::XinputHierarchy(_) => {
                    refresh = true;
                }
                _ => {}
            }
        }
        if refresh {
            self.refresh_devices();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(integral: i32, frac: u32) -> Fp3232 {
        Fp3232 { integral, frac }
    }

    #[test]
    fn valuator_normalization_preserves_delta_compressed_axes() {
        let mut dev = DeviceAxes {
            pressure: Some(AxisRange {
                number: 2,
                min: 0.0,
                max: 8191.0,
                resolution: 0,
            }),
            tilt_x: Some(AxisRange {
                number: 3,
                min: -60.0,
                max: 60.0,
                resolution: 57,
            }),
            tilt_y: Some(AxisRange {
                number: 4,
                min: -60.0,
                max: 60.0,
                resolution: 57,
            }),
            is_eraser: false,
            last_pressure: 0.0,
            last_tilt: [0.0, 0.0],
        };

        // Packet 1 updates axes 0, 2, 3, 4.
        let mask1 = [(1u32 << 0) | (1u32 << 2) | (1u32 << 3) | (1u32 << 4)];
        let values1 = [fp(500, 0), fp(4095, 2_147_483_648), fp(25, 0), fp(-15, 0)];
        dev.update_from_valuators(&mask1, &values1);
        assert!((dev.last_pressure - 0.5).abs() < 1e-3);
        assert_eq!(dev.last_tilt, [25.0, -15.0]);

        // Packet 2 only updates x/y (axes 0 and 1); steady pressure and tilt
        // stay intact.
        let mask2 = [(1u32 << 0) | (1u32 << 1)];
        let values2 = [fp(510, 0), fp(300, 0)];
        dev.update_from_valuators(&mask2, &values2);
        assert!((dev.last_pressure - 0.5).abs() < 1e-3);
        assert_eq!(dev.last_tilt, [25.0, -15.0]);
    }
}
