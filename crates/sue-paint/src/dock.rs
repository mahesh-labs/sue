//! Where Sue Paint's palettes sit, and how people move and resize them:
//! docked in columns at the left and right of the canvas, docked in rows at
//! the top and bottom, floating in resizable windows of their own, or
//! collapsed to icons in a strip, as in Clip Studio Paint. Drag any panel's
//! header or grip to move it anywhere, and drag any edge or splitter to
//! resize it.

use egui::{
    Align2, Color32, CursorIcon, Id, Order, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2,
};
use sue_wm::{AppletContext, Theme};

use crate::icons::{self, Icon};
use crate::panels::icon_button;
use crate::studio::{Panels, SuePaint};

/// A palette people can dock, float, resize or collapse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Palette {
    /// The top command bar.
    CommandBar,
    /// The tool strip.
    Tools,
    /// Sub Tool (Tool Group) and its companion tabs.
    SubTool,
    /// Tool Settings (Tool Property) as a standalone panel.
    ToolProperty,
    /// The tall brush size and opacity sliders.
    BrushSize,
    /// The brush size preset grid as a standalone panel.
    BrushGrid,
    /// The color wheel and its companion tabs.
    Color,
    /// Color Slider as a standalone panel.
    ColorSlider,
    /// Intermediate Color as a standalone panel.
    Intermediate,
    /// Approximate Color as a standalone panel.
    Approximate,
    /// The color set and its companion tabs.
    ColorSet,
    /// Color History as a standalone panel.
    ColorHistory,
    /// The navigator and sub view.
    Navigator,
    /// The layers and their companion tabs.
    Layer,
    /// The animation timeline.
    Timeline,
    /// The materials.
    Material,
    /// The current layer's properties and effects.
    LayerProperty,
    /// The undo history.
    History,
    /// The light table and cel buttons.
    Cels,
    /// A picture to look at or pick colors from.
    SubView,
    /// Recorded steps to play again.
    AutoAction,
    /// Things kept to use again.
    ItemBank,
    /// The pointer's place and the canvas's size.
    Info,
}

/// The palettes Clip Studio Paint keeps folded in the strip at the right
/// from the start.
pub(crate) const STRIP_PALETTES: [Palette; 8] = [
    Palette::Material,
    Palette::LayerProperty,
    Palette::Cels,
    Palette::History,
    Palette::SubView,
    Palette::AutoAction,
    Palette::ItemBank,
    Palette::Info,
];

/// Every palette, for reading layouts back.
pub(crate) const PALETTES: [Palette; 23] = [
    Palette::CommandBar,
    Palette::Tools,
    Palette::SubTool,
    Palette::ToolProperty,
    Palette::BrushSize,
    Palette::BrushGrid,
    Palette::Color,
    Palette::ColorSlider,
    Palette::Intermediate,
    Palette::Approximate,
    Palette::ColorSet,
    Palette::ColorHistory,
    Palette::Navigator,
    Palette::Layer,
    Palette::Timeline,
    Palette::Material,
    Palette::LayerProperty,
    Palette::History,
    Palette::Cels,
    Palette::SubView,
    Palette::AutoAction,
    Palette::ItemBank,
    Palette::Info,
];

impl Palette {
    /// Stable language-independent key for workspace files and widget ids.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::CommandBar => "command_bar",
            Self::Tools => "tools",
            Self::SubTool => "sub_tool",
            Self::ToolProperty => "tool_property",
            Self::BrushSize => "brush_sliders",
            Self::BrushGrid => "brush_grid",
            Self::Color => "color_wheel",
            Self::ColorSlider => "color_slider",
            Self::Intermediate => "intermediate",
            Self::Approximate => "approximate",
            Self::ColorSet => "color_set",
            Self::ColorHistory => "color_history",
            Self::Navigator => "navigator",
            Self::Layer => "layer",
            Self::Timeline => "timeline",
            Self::Material => "material",
            Self::LayerProperty => "layer_property",
            Self::History => "history",
            Self::Cels => "animation_cels",
            Self::SubView => "sub_view",
            Self::AutoAction => "auto_actions",
            Self::ItemBank => "item_bank",
            Self::Info => "info",
        }
    }

    /// What its window and icon are called.
    #[must_use]
    pub fn title(self) -> String {
        match self {
            Self::CommandBar => tr!("command-bar"),
            Self::Tools => tr!("tool"),
            Self::SubTool => tr!("sub-tool"),
            Self::ToolProperty => tr!("tool-settings"),
            Self::BrushSize => tr!("brush-size"),
            Self::BrushGrid => tr!("brush-size"),
            Self::Color => tr!("color-wheel"),
            Self::ColorSlider => tr!("color-slider"),
            Self::Intermediate => tr!("intermediate-color"),
            Self::Approximate => tr!("approximate-color"),
            Self::ColorSet => tr!("color-set"),
            Self::ColorHistory => tr!("color-history"),
            Self::Navigator => tr!("navigator"),
            Self::Layer => tr!("layer-2"),
            Self::Timeline => tr!("timeline"),
            Self::Material => tr!("material"),
            Self::LayerProperty => tr!("layer-property"),
            Self::History => tr!("history"),
            Self::Cels => tr!("animation-cels"),
            Self::SubView => tr!("sub-view"),
            Self::AutoAction => tr!("auto-actions"),
            Self::ItemBank => tr!("item-bank"),
            Self::Info => tr!("info"),
        }
    }

    /// Its icon in headers and the strip.
    #[must_use]
    pub const fn icon(self) -> Icon {
        match self {
            Self::CommandBar => Icon::Menu,
            Self::Tools => Icon::Pen,
            Self::SubTool => Icon::Brush,
            Self::ToolProperty => Icon::Sliders,
            Self::BrushSize | Self::BrushGrid => Icon::BrushSize,
            Self::Color => Icon::Wheel,
            Self::ColorSlider => Icon::ColorSlider,
            Self::Intermediate => Icon::Intermediate,
            Self::Approximate => Icon::Approximate,
            Self::ColorSet => Icon::Swatches,
            Self::ColorHistory => Icon::History,
            Self::Navigator => Icon::Navigator,
            Self::Layer => Icon::Layers,
            Self::Timeline => Icon::Timeline,
            Self::Material => Icon::Material,
            Self::LayerProperty => Icon::LayerProperty,
            Self::History => Icon::History,
            Self::Cels => Icon::Cel,
            Self::SubView => Icon::SubView,
            Self::AutoAction => Icon::AutoAction,
            Self::ItemBank => Icon::ItemBank,
            Self::Info => Icon::Info,
        }
    }

    /// The initial width of a narrow column palette when docked into its own
    /// column, though every column can still be resized freely afterward.
    #[must_use]
    pub const fn default_column_width(self) -> Option<f32> {
        match self {
            Self::Tools => Some(46.0),
            Self::BrushSize => Some(86.0),
            _ => None,
        }
    }

    /// The least width it stays usable at.
    #[must_use]
    pub const fn min_width(self) -> f32 {
        match self {
            Self::Tools => 38.0,
            Self::CommandBar => 46.0,
            Self::BrushSize => 56.0,
            _ => 136.0,
        }
    }

    /// The least height it stays usable at.
    #[must_use]
    pub const fn min_height(self) -> f32 {
        match self {
            Self::CommandBar => 34.0,
            Self::Timeline => 84.0,
            Self::Tools | Self::BrushSize => 96.0,
            Self::Color => 130.0,
            _ => 80.0,
        }
    }

    /// `true` when the Window menu shows it.
    #[must_use]
    pub fn shown(self, panels: &Panels) -> bool {
        match self {
            Self::CommandBar => panels.command_bar,
            Self::Tools => panels.tool,
            Self::SubTool => panels.sub_tool || panels.tool_property,
            Self::BrushSize => panels.brush_size,
            Self::Color => panels.color,
            Self::ColorSet => panels.color_set,
            Self::Navigator => panels.navigator,
            Self::Layer => panels.layer,
            Self::Timeline => panels.timeline,
            Self::ToolProperty
            | Self::BrushGrid
            | Self::ColorSlider
            | Self::Intermediate
            | Self::Approximate
            | Self::ColorHistory
            | Self::Material
            | Self::LayerProperty
            | Self::History
            | Self::Cels
            | Self::SubView
            | Self::AutoAction
            | Self::ItemBank
            | Self::Info => true,
        }
    }

    /// `true` for standalone tear-off palettes beyond the primary layout.
    #[cfg(test)]
    #[must_use]
    pub const fn is_standalone_extra(self) -> bool {
        matches!(
            self,
            Self::ToolProperty
                | Self::BrushGrid
                | Self::ColorSlider
                | Self::Intermediate
                | Self::Approximate
                | Self::ColorHistory
        )
    }

    /// The default size of its window when it floats.
    #[must_use]
    pub const fn float_size(self) -> Vec2 {
        match self {
            Self::CommandBar => vec2(620.0, 44.0),
            Self::Tools => vec2(54.0, 440.0),
            Self::BrushSize => vec2(88.0, 360.0),
            Self::Layer => vec2(300.0, 420.0),
            Self::Timeline => vec2(680.0, 200.0),
            _ => vec2(260.0, 320.0),
        }
    }
}

/// A side of the canvas for vertical columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Side {
    Left,
    Right,
}

/// A column of palettes sharing its height by weight.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Column {
    /// The palettes, top to bottom, with their share of the height.
    pub palettes: Vec<(Palette, f32)>,
    /// Its width in points.
    pub width: f32,
}

/// A horizontal row of palettes (top or bottom) sharing its width by weight.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    /// The palettes, left to right, with their share of the width.
    pub palettes: Vec<(Palette, f32)>,
    /// Its height in points.
    pub height: f32,
}

/// Where a dragged palette will land if released.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DropTarget {
    /// Inside an existing left/right column at index `at`.
    InColumn {
        side: Side,
        column: usize,
        at: usize,
        preview: Rect,
    },
    /// As a brand new column at index `at` on `side`.
    NewColumn {
        side: Side,
        at: usize,
        preview: Rect,
    },
    /// Inside an existing top/bottom row at index `at`.
    InRow {
        top: bool,
        row: usize,
        at: usize,
        preview: Rect,
    },
    /// As a brand new top/bottom row at index `at`.
    NewRow { top: bool, at: usize, preview: Rect },
    /// Collapsed into the icon strip.
    Iconize { preview: Rect },
    /// Floating in a window at `rect`.
    Float { rect: Rect },
}

impl DropTarget {
    const fn preview(self) -> Rect {
        match self {
            Self::InColumn { preview, .. }
            | Self::NewColumn { preview, .. }
            | Self::InRow { preview, .. }
            | Self::NewRow { preview, .. }
            | Self::Iconize { preview }
            | Self::Float { rect: preview } => preview,
        }
    }
}

/// What is being dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// A column's inner edge.
    Edge {
        side: Side,
        column: usize,
        from: f32,
        width: f32,
    },
    /// The line between two palettes of a column.
    Split {
        side: Side,
        column: usize,
        upper: Palette,
        lower: Palette,
        from: f32,
        weights: (f32, f32),
        per_point: f32,
    },
    /// A top or bottom row's inner horizontal edge.
    RowEdge {
        top: bool,
        row: usize,
        from: f32,
        height: f32,
    },
    /// The vertical line between two palettes of a top or bottom row.
    RowSplit {
        top: bool,
        row: usize,
        left: Palette,
        right: Palette,
        from: f32,
        weights: (f32, f32),
        per_point: f32,
    },
    /// Resizing a floating palette from an edge or corner (`dx`, `dy` in -1..=1).
    FloatResize {
        palette: Palette,
        start_rect: Rect,
        from: Pos2,
        dx: i8,
        dy: i8,
    },
    /// Moving a palette (docked, popped, torn-off tab, or floating) by dragging.
    Move {
        palette: Palette,
        from: Pos2,
        grab_offset: Vec2,
        was_floating: Option<Rect>,
        active: bool,
    },
}

/// Where every palette is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Dock {
    /// Rows from the window's top edge downward.
    pub top: Vec<Row>,
    /// Rows from the center's bottom edge upward.
    pub bottom: Vec<Row>,
    /// Columns from the window's left edge inward.
    pub left: Vec<Column>,
    /// Columns from the window's right edge inward.
    pub right: Vec<Column>,
    /// In-workspace floating palettes and their place in the studio.
    pub floating: Vec<(Palette, Rect)>,
    /// Palettes popped out into separate system windows.
    pub detached: Vec<(Palette, Rect)>,
    /// Palettes collapsed to icons on purpose.
    pub iconized: Vec<Palette>,
    /// The collapsed palette showing beside the strip.
    pub popped: Option<Palette>,
    /// The palette whose menu is open.
    pub menu: Option<Palette>,
    /// The timeline's height (kept in sync with the bottom timeline row).
    pub timeline: f32,
    /// Each side's palette columns folded into icons (Clip Studio Paint's
    /// single arrow), left then right. The tools stay.
    pub folded: [bool; 2],
    /// Each side closed whole, palettes and icons (Clip Studio Paint's
    /// double arrow), left then right.
    pub hidden: [bool; 2],
    drag: Option<Drag>,
}

impl Side {
    /// Its place in [`Dock::folded`] and [`Dock::hidden`].
    pub(crate) fn index(self) -> usize {
        match self {
            Side::Left => 0,
            Side::Right => 1,
        }
    }
}

impl Default for Dock {
    fn default() -> Self {
        let column = |palettes: &[(Palette, f32)], width: f32| Column {
            palettes: palettes.to_vec(),
            width,
        };
        let row = |palettes: &[(Palette, f32)], height: f32| Row {
            palettes: palettes.to_vec(),
            height,
        };
        Self {
            top: vec![row(&[(Palette::CommandBar, 1.0)], 38.0)],
            bottom: vec![row(&[(Palette::Timeline, 1.0)], 160.0)],
            left: vec![
                column(&[(Palette::Tools, 1.0)], 46.0),
                column(
                    &[
                        (Palette::SubTool, 0.44),
                        (Palette::Color, 0.3),
                        (Palette::ColorSet, 0.26),
                    ],
                    236.0,
                ),
                column(&[(Palette::BrushSize, 1.0)], 86.0),
            ],
            right: vec![column(
                &[(Palette::Navigator, 0.42), (Palette::Layer, 0.58)],
                300.0,
            )],
            floating: Vec::new(),
            detached: Vec::new(),
            iconized: STRIP_PALETTES.to_vec(),
            popped: None,
            menu: None,
            timeline: 160.0,
            folded: [false; 2],
            hidden: [false; 2],
            drag: None,
        }
    }
}

/// The width of the icon strip.
pub(crate) const STRIP: f32 = 34.0;

/// The bar at the top of every column and strip, with Clip Studio Paint's
/// arrows: one folds the side's columns into icons, two close the side.
pub(crate) const COLUMN_BAR: f32 = 12.0;

/// How wide a closed side's bar is, with its arrows to open it again.
pub(crate) const HANDLE: f32 = 14.0;

/// The canvas never gets narrower than this while columns can collapse.
const MIN_CANVAS: f32 = 260.0;

/// Where everything goes this frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Laid {
    /// Docked column palettes and their rects.
    pub docked: Vec<(Palette, Rect)>,
    /// Docked top and bottom row palettes and their rects.
    pub bands: Vec<(Palette, Rect)>,
    /// Columns: side, index, rect.
    pub columns: Vec<(Side, usize, Rect)>,
    /// Rows: `true` for top / `false` for bottom, index, rect.
    pub rows: Vec<(bool, usize, Rect)>,
    /// Lines between palettes in a column: side, column, upper, lower, rect,
    /// the column's height.
    pub splits: Vec<(Side, usize, Palette, Palette, Rect, f32)>,
    /// Columns' inner edges people can drag: side, column, rect.
    pub edges: Vec<(Side, usize, Rect)>,
    /// Rows' inner horizontal edges people can drag: top?, row, rect.
    pub row_edges: Vec<(bool, usize, Rect)>,
    /// Vertical lines between palettes in a row: top?, row, left, right, rect,
    /// the row's width.
    pub row_splits: Vec<(bool, usize, Palette, Palette, Rect, f32)>,
    /// The icon strip at the right, when it shows.
    pub strip: Option<Rect>,
    /// Palettes in the right strip: the collapsed ones, the right side's
    /// folded columns, then those the window had no room for.
    pub icons: Vec<Palette>,
    /// The icon strip at the left, while the left columns are folded.
    pub left_strip: Option<Rect>,
    /// Palettes in the left strip.
    pub left_icons: Vec<Palette>,
    /// Closed sides' bars.
    pub handles: Vec<(Side, Rect)>,
    /// Area below the top rows (where columns, bottom rows and canvas live).
    pub mid: Rect,
    /// Center area between left and right columns (before bottom rows).
    pub center_column: Rect,
    /// What is left for the canvas.
    pub center: Rect,
}

fn palette_named(name: &str) -> Option<Palette> {
    PALETTES
        .into_iter()
        .find(|palette| palette.key() == name || palette.title() == name)
}

impl Dock {
    /// `true` while a drag or resize is in progress.
    #[must_use]
    pub const fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Starts a potential drag-move on `palette` from `pointer`. Until the
    /// pointer moves at least 5 points, normal clicks on tabs or buttons
    /// proceed untouched.
    pub fn begin_move(
        &mut self,
        palette: Palette,
        pointer: Pos2,
        panel_rect: Rect,
        was_floating: Option<Rect>,
    ) {
        if self.drag.is_some() {
            return;
        }
        let grab_offset = (pointer - panel_rect.min).clamp(Vec2::ZERO, panel_rect.size());
        self.drag = Some(Drag::Move {
            palette,
            from: pointer,
            grab_offset,
            was_floating,
            active: false,
        });
    }

    /// Starts dragging `palette` from a palette header or tab, preserving
    /// floating-window motion if `palette` is already floating.
    pub fn begin_move_from_header(&mut self, palette: Palette, pointer: Pos2, panel_rect: Rect) {
        let was_floating = self
            .floating
            .iter()
            .find(|(found, _)| *found == palette)
            .map(|(_, rect)| *rect);
        self.begin_move(
            palette,
            pointer,
            was_floating.unwrap_or(panel_rect),
            was_floating,
        );
    }

    /// The layout as lines (a registered workspace): each column and row with
    /// its size and palettes, the floating and detached palettes, the
    /// collapsed ones and the timeline's height.
    #[must_use]
    pub fn to_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (band, rows) in [("top", &self.top), ("bottom", &self.bottom)] {
            for row in rows {
                let palettes: Vec<String> = row
                    .palettes
                    .iter()
                    .map(|(palette, weight)| format!("{}={weight}", palette.key()))
                    .collect();
                if !palettes.is_empty() {
                    let height = if band == "bottom"
                        && row.palettes.len() == 1
                        && row.palettes[0].0 == Palette::Timeline
                        && (row.height - 160.0).abs() < 0.01
                        && (self.timeline - 160.0).abs() > 0.01
                    {
                        self.timeline
                    } else {
                        row.height
                    };
                    out.push(format!("{band} {height} {}", palettes.join(",")));
                }
            }
        }
        for (side, columns) in [("left", &self.left), ("right", &self.right)] {
            for column in columns {
                let palettes: Vec<String> = column
                    .palettes
                    .iter()
                    .map(|(palette, weight)| format!("{}={weight}", palette.key()))
                    .collect();
                if !palettes.is_empty() {
                    out.push(format!("{side} {} {}", column.width, palettes.join(",")));
                }
            }
        }
        for (palette, rect) in &self.floating {
            out.push(format!(
                "float {} {} {} {} {}",
                rect.min.x,
                rect.min.y,
                rect.width(),
                rect.height(),
                palette.key()
            ));
        }
        for (palette, rect) in &self.detached {
            out.push(format!(
                "detach {} {} {} {} {}",
                rect.min.x,
                rect.min.y,
                rect.width(),
                rect.height(),
                palette.key()
            ));
        }
        for palette in &self.iconized {
            out.push(format!("icon {}", palette.key()));
        }
        out.push(format!("timeline {}", self.timeline));
        for side in [Side::Left, Side::Right] {
            let name = if side == Side::Left { "left" } else { "right" };
            if self.folded[side.index()] {
                out.push(format!("fold {name}"));
            }
            if self.hidden[side.index()] {
                out.push(format!("hide {name}"));
            }
        }
        out
    }

    /// A layout from [`Self::to_lines`] (lines it does not know are
    /// skipped); `None` when no palette is placed.
    #[must_use]
    pub fn from_lines(lines: &[String]) -> Option<Self> {
        let mut dock = Self {
            top: Vec::new(),
            bottom: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            floating: Vec::new(),
            detached: Vec::new(),
            iconized: Vec::new(),
            ..Self::default()
        };
        let mut saw_top = false;
        let mut saw_bottom = false;
        for line in lines {
            let mut words = line.splitn(3, ' ');
            match (words.next(), words.next(), words.next()) {
                (Some(band @ ("top" | "bottom")), Some(height), Some(palettes)) => {
                    let palettes: Vec<(Palette, f32)> = palettes
                        .split(',')
                        .filter_map(|entry| {
                            let (title, weight) = entry.split_once('=')?;
                            Some((palette_named(title)?, weight.parse().ok()?))
                        })
                        .collect();
                    if palettes.is_empty() {
                        continue;
                    }
                    let height = height
                        .parse()
                        .unwrap_or(if band == "top" { 38.0 } else { 160.0 });
                    if band == "top" {
                        saw_top = true;
                        dock.top.push(Row { palettes, height });
                    } else {
                        saw_bottom = true;
                        if palettes.iter().any(|(p, _)| *p == Palette::Timeline) {
                            dock.timeline = height;
                        }
                        dock.bottom.push(Row { palettes, height });
                    }
                }
                (Some(side @ ("left" | "right")), Some(width), Some(palettes)) => {
                    let palettes: Vec<(Palette, f32)> = palettes
                        .split(',')
                        .filter_map(|entry| {
                            let (title, weight) = entry.split_once('=')?;
                            Some((palette_named(title)?, weight.parse().ok()?))
                        })
                        .collect();
                    if palettes.is_empty() {
                        continue;
                    }
                    let column = Column {
                        palettes,
                        width: width.parse().unwrap_or(260.0),
                    };
                    if side == "left" {
                        dock.left.push(column);
                    } else {
                        dock.right.push(column);
                    }
                }
                (Some(kind @ ("float" | "detach")), Some(x), Some(rest)) => {
                    let parts: Vec<&str> = rest.splitn(4, ' ').collect();
                    if let ([y, w, h, title], Ok(x)) = (parts.as_slice(), x.parse::<f32>())
                        && let (Ok(y), Ok(w), Ok(h), Some(palette)) = (
                            y.parse::<f32>(),
                            w.parse::<f32>(),
                            h.parse::<f32>(),
                            palette_named(title),
                        )
                    {
                        let rect = Rect::from_min_size(pos2(x, y), vec2(w, h));
                        if kind == "detach" {
                            dock.detached.push((palette, rect));
                        } else {
                            dock.floating.push((palette, rect));
                        }
                    }
                }
                (Some("icon"), Some(first), rest) => {
                    let title =
                        rest.map_or_else(|| first.to_owned(), |rest| format!("{first} {rest}"));
                    if let Some(palette) = palette_named(&title) {
                        dock.iconized.push(palette);
                    }
                }
                (Some("timeline"), Some(height), None) => {
                    if let Ok(height) = height.parse() {
                        dock.timeline = height;
                    }
                }
                (Some(what @ ("fold" | "hide")), Some(side @ ("left" | "right")), None) => {
                    let at = usize::from(side == "right");
                    if what == "fold" {
                        dock.folded[at] = true;
                    } else {
                        dock.hidden[at] = true;
                    }
                }
                _ => {}
            }
        }
        let placed = !dock.top.is_empty()
            || !dock.bottom.is_empty()
            || !dock.left.is_empty()
            || !dock.right.is_empty()
            || !dock.floating.is_empty()
            || !dock.detached.is_empty()
            || !dock.iconized.is_empty();
        if !placed {
            return None;
        }
        // Workspaces saved before CommandBar and Timeline joined the dock
        // keep them in their classic places at the top and bottom.
        if !saw_top && !dock.has(Palette::CommandBar) {
            dock.top.push(Row {
                palettes: vec![(Palette::CommandBar, 1.0)],
                height: 38.0,
            });
        }
        if !saw_bottom && !dock.has(Palette::Timeline) {
            dock.bottom.push(Row {
                palettes: vec![(Palette::Timeline, 1.0)],
                height: dock.timeline,
            });
        }
        // A layout from before a strip palette existed keeps it in the strip.
        for palette in STRIP_PALETTES {
            if !dock.has(palette) {
                dock.iconized.push(palette);
            }
        }
        Some(dock)
    }

    /// `palette` is somewhere: docked, floating, detached or in the strip.
    #[must_use]
    pub fn has(&self, palette: Palette) -> bool {
        self.top
            .iter()
            .chain(self.bottom.iter())
            .any(|row| row.palettes.iter().any(|(found, _)| *found == palette))
            || self
                .left
                .iter()
                .chain(self.right.iter())
                .any(|column| column.palettes.iter().any(|(found, _)| *found == palette))
            || self.floating.iter().any(|(found, _)| *found == palette)
            || self.detached.iter().any(|(found, _)| *found == palette)
            || self.iconized.contains(&palette)
    }

    /// `true` when `palette` is docked or floating (not merely iconized).
    #[must_use]
    pub fn is_open_panel(&self, palette: Palette) -> bool {
        self.top
            .iter()
            .chain(self.bottom.iter())
            .any(|row| row.palettes.iter().any(|(found, _)| *found == palette))
            || self
                .left
                .iter()
                .chain(self.right.iter())
                .any(|column| column.palettes.iter().any(|(found, _)| *found == palette))
            || self.floating.iter().any(|(found, _)| *found == palette)
            || self.detached.iter().any(|(found, _)| *found == palette)
    }

    /// Takes `palette` out of wherever it is.
    pub fn remove(&mut self, palette: Palette) {
        for row in self.top.iter_mut().chain(self.bottom.iter_mut()) {
            row.palettes.retain(|(found, _)| *found != palette);
        }
        self.top.retain(|row| !row.palettes.is_empty());
        self.bottom.retain(|row| !row.palettes.is_empty());
        for column in self.left.iter_mut().chain(self.right.iter_mut()) {
            column.palettes.retain(|(found, _)| *found != palette);
        }
        self.left.retain(|column| !column.palettes.is_empty());
        self.right.retain(|column| !column.palettes.is_empty());
        self.floating.retain(|(found, _)| *found != palette);
        self.detached.retain(|(found, _)| *found != palette);
        self.iconized.retain(|found| *found != palette);
        if self.popped == Some(palette) {
            self.popped = None;
        }
    }

    /// Docks `palette` on `side`: in a column of its own for a narrow tool
    /// palette, else at the bottom of the innermost wide column.
    pub fn dock(&mut self, palette: Palette, side: Side) {
        self.remove(palette);
        let columns = match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        };
        match palette.default_column_width() {
            Some(width) => columns.push(Column {
                palettes: vec![(palette, 1.0)],
                width,
            }),
            None => {
                let wide = columns.iter_mut().rev().find(|column| {
                    column
                        .palettes
                        .iter()
                        .all(|(found, _)| found.default_column_width().is_none())
                });
                match wide {
                    Some(column) => {
                        let share = 1.0 / (column.palettes.len() as f32 + 1.0);
                        column.palettes.push((palette, share.max(0.2)));
                    }
                    None => columns.push(Column {
                        palettes: vec![(palette, 1.0)],
                        width: 260.0,
                    }),
                }
            }
        }
    }

    /// Docks `palette` as a brand new column at `at` on `side`.
    pub fn dock_new_column(&mut self, palette: Palette, side: Side, at: usize) {
        let existing_width = self
            .left
            .iter()
            .chain(self.right.iter())
            .find(|col| col.palettes.len() == 1 && col.palettes[0].0 == palette)
            .map(|col| col.width);
        self.remove(palette);
        let width = existing_width
            .or_else(|| palette.default_column_width())
            .unwrap_or(250.0)
            .max(palette.min_width());
        let columns = match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        };
        let index = at.min(columns.len());
        columns.insert(
            index,
            Column {
                palettes: vec![(palette, 1.0)],
                width,
            },
        );
    }

    /// Inserts `palette` inside `column` on `side` at position `at`.
    pub fn dock_in_column(&mut self, palette: Palette, side: Side, column: usize, at: usize) {
        // Adjust column index if removing `palette` deletes an earlier singleton column on `side`.
        let mut target_col = column;
        let mut target_at = at;
        {
            let cols = match side {
                Side::Left => &self.left,
                Side::Right => &self.right,
            };
            if let Some(col_idx) = cols
                .iter()
                .position(|c| c.palettes.iter().any(|(p, _)| *p == palette))
            {
                if cols[col_idx].palettes.len() == 1 && col_idx < target_col {
                    target_col = target_col.saturating_sub(1);
                } else if col_idx == target_col
                    && let Some(pos) = cols[col_idx]
                        .palettes
                        .iter()
                        .position(|(p, _)| *p == palette)
                    && pos < target_at
                {
                    target_at = target_at.saturating_sub(1);
                }
            }
        }
        self.remove(palette);
        let columns = match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        };
        if columns.is_empty() {
            columns.push(Column {
                palettes: vec![(palette, 1.0)],
                width: palette.default_column_width().unwrap_or(250.0),
            });
            return;
        }
        let last = columns.len() - 1;
        let col = &mut columns[target_col.min(last)];
        col.width = col.width.max(palette.min_width());
        let total: f32 = col.palettes.iter().map(|(_, w)| w.max(0.05)).sum();
        let share = (total / (col.palettes.len() as f32).max(1.0)).clamp(0.15, 1.0);
        let insert_at = target_at.min(col.palettes.len());
        col.palettes.insert(insert_at, (palette, share));
    }

    /// Docks `palette` in a brand new top (`top == true`) or bottom row at `at`.
    pub fn dock_new_row(&mut self, palette: Palette, top: bool, at: usize) {
        let default_h = match palette {
            Palette::CommandBar => 38.0,
            Palette::Timeline => self.timeline.clamp(90.0, 480.0),
            Palette::Tools => 48.0,
            _ => 180.0,
        };
        self.remove(palette);
        let rows = if top { &mut self.top } else { &mut self.bottom };
        let index = at.min(rows.len());
        rows.insert(
            index,
            Row {
                palettes: vec![(palette, 1.0)],
                height: default_h.max(palette.min_height()),
            },
        );
    }

    /// Inserts `palette` inside an existing top or bottom `row` at `at`.
    pub fn dock_in_row(&mut self, palette: Palette, top: bool, row: usize, at: usize) {
        let mut target_row = row;
        let mut target_at = at;
        {
            let rows = if top { &self.top } else { &self.bottom };
            if let Some(row_idx) = rows
                .iter()
                .position(|r| r.palettes.iter().any(|(p, _)| *p == palette))
            {
                if rows[row_idx].palettes.len() == 1 && row_idx < target_row {
                    target_row = target_row.saturating_sub(1);
                } else if row_idx == target_row
                    && let Some(pos) = rows[row_idx]
                        .palettes
                        .iter()
                        .position(|(p, _)| *p == palette)
                    && pos < target_at
                {
                    target_at = target_at.saturating_sub(1);
                }
            }
        }
        self.remove(palette);
        let rows = if top { &mut self.top } else { &mut self.bottom };
        if rows.is_empty() {
            self.dock_new_row(palette, top, 0);
            return;
        }
        let last = rows.len() - 1;
        let target = &mut rows[target_row.min(last)];
        target.height = target.height.max(palette.min_height());
        let total: f32 = target.palettes.iter().map(|(_, w)| w.max(0.05)).sum();
        let share = (total / (target.palettes.len() as f32).max(1.0)).clamp(0.15, 1.0);
        let insert_at = target_at.min(target.palettes.len());
        target.palettes.insert(insert_at, (palette, share));
    }

    /// Floats `palette` in an in-workspace window at `at`.
    pub fn float(&mut self, palette: Palette, at: Pos2) {
        self.float_rect(palette, Rect::from_min_size(at, palette.float_size()));
    }

    /// Floats `palette` at the exact `rect`.
    pub fn float_rect(&mut self, palette: Palette, rect: Rect) {
        self.remove(palette);
        let size = vec2(
            rect.width().max(palette.min_width()),
            rect.height().max(palette.min_height()),
        );
        self.floating
            .push((palette, Rect::from_min_size(rect.min, size)));
    }

    /// Pops `palette` out into its own system viewport window.
    pub fn detach(&mut self, palette: Palette, at: Pos2) {
        self.remove(palette);
        self.detached
            .push((palette, Rect::from_min_size(at, palette.float_size())));
    }

    /// Collapses `palette` to an icon in the strip.
    pub fn iconize(&mut self, palette: Palette) {
        self.remove(palette);
        self.iconized.push(palette);
    }

    /// The side `palette` is docked on, if it is docked in a column.
    #[must_use]
    pub fn side_of(&self, palette: Palette) -> Option<Side> {
        let has = |columns: &[Column]| {
            columns
                .iter()
                .any(|column| column.palettes.iter().any(|(found, _)| *found == palette))
        };
        if has(&self.left) {
            Some(Side::Left)
        } else if has(&self.right) {
            Some(Side::Right)
        } else {
            None
        }
    }

    /// `Some(true)` if `palette` is in a top row, `Some(false)` if in a bottom row.
    #[must_use]
    pub fn band_of(&self, palette: Palette) -> Option<bool> {
        if self
            .top
            .iter()
            .any(|row| row.palettes.iter().any(|(found, _)| *found == palette))
        {
            Some(true)
        } else if self
            .bottom
            .iter()
            .any(|row| row.palettes.iter().any(|(found, _)| *found == palette))
        {
            Some(false)
        } else {
            None
        }
    }

    /// Lays everything out in `body`; `shown` says which palettes the
    /// Window menu shows and `hide` (0 to 1) slides the bars and columns out
    /// to their edges.
    #[allow(
        clippy::too_many_lines,
        reason = "4-edge multi-column and multi-row layout"
    )]
    pub fn layout(&self, body: Rect, shown: &dyn Fn(Palette) -> bool, hide: f32) -> Laid {
        let visible_col = |column: &Column| -> Vec<(Palette, f32)> {
            column
                .palettes
                .iter()
                .copied()
                .filter(|(palette, _)| shown(*palette))
                .collect()
        };
        let visible_row = |row: &Row| -> Vec<(Palette, f32)> {
            row.palettes
                .iter()
                .copied()
                .filter(|(palette, _)| shown(*palette))
                .collect()
        };
        let width_of = |column: &Column| -> f32 {
            let palettes = visible_col(column);
            let min_w = palettes
                .iter()
                .map(|(palette, _)| palette.min_width())
                .fold(38.0_f32, f32::max);
            column.width.max(min_w)
        };
        let height_of_row = |row: &Row, is_bottom: bool| -> f32 {
            let palettes = visible_row(row);
            let min_h = palettes
                .iter()
                .map(|(palette, _)| palette.min_height())
                .fold(34.0_f32, f32::max);
            let raw = if is_bottom
                && row.palettes.len() == 1
                && row.palettes[0].0 == Palette::Timeline
                && (row.height - 160.0).abs() < 0.01
                && (self.timeline - 160.0).abs() > 0.01
            {
                self.timeline
            } else {
                row.height
            };
            raw.max(min_h)
        };

        let mut laid = Laid {
            docked: Vec::new(),
            bands: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            splits: Vec::new(),
            edges: Vec::new(),
            row_edges: Vec::new(),
            row_splits: Vec::new(),
            strip: None,
            icons: Vec::new(),
            left_strip: None,
            left_icons: Vec::new(),
            handles: Vec::new(),
            mid: body,
            center_column: body,
            center: body,
        };

        let both_closed = self.hidden == [true; 2];

        // 1. Top rows across the full width of `body`.
        let mut top_rows: Vec<(usize, f32)> = Vec::new();
        if !both_closed {
            let mut budget = (body.height() - 220.0).max(0.0);
            for (index, row) in self.top.iter().enumerate() {
                if visible_row(row).is_empty() {
                    continue;
                }
                let h = height_of_row(row, false).min(budget);
                if h >= 28.0 {
                    top_rows.push((index, h));
                    budget = (budget - h).max(0.0);
                }
            }
        }
        let top_total: f32 = top_rows.iter().map(|(_, h)| *h).sum();
        let top_lift = hide * top_total;
        let mut top_y = body.top() - top_lift;
        let place_row = |laid: &mut Laid, is_top: bool, index: usize, rect: Rect, row: &Row| {
            laid.rows.push((is_top, index, rect));
            let palettes = visible_row(row);
            let total: f32 = palettes.iter().map(|(_, w)| w.max(0.01)).sum();
            let mut widths: Vec<f32> = palettes
                .iter()
                .map(|(palette, w)| {
                    (rect.width() * w.max(0.01) / total).max(palette.min_width().min(rect.width()))
                })
                .collect();
            let excess: f32 = widths.iter().sum::<f32>() - rect.width();
            if excess > 0.0
                && let Some(widest) = widths.iter_mut().max_by(|a, b| a.total_cmp(b))
            {
                *widest = (*widest - excess).max(36.0);
            }
            let mut x = rect.left();
            for (at, ((palette, _), w)) in palettes.iter().zip(&widths).enumerate() {
                let piece = Rect::from_min_size(pos2(x, rect.top()), vec2(*w, rect.height()));
                laid.bands.push((*palette, piece));
                x = piece.right();
                if let Some((right_pal, _)) = palettes.get(at + 1) {
                    laid.row_splits.push((
                        is_top,
                        index,
                        *palette,
                        *right_pal,
                        Rect::from_center_size(pos2(x, rect.center().y), vec2(8.0, rect.height())),
                        rect.width(),
                    ));
                }
            }
            let edge_y = if is_top { rect.bottom() } else { rect.top() };
            laid.row_edges.push((
                is_top,
                index,
                Rect::from_center_size(pos2(rect.center().x, edge_y), vec2(rect.width(), 8.0)),
            ));
        };
        for (index, h) in &top_rows {
            let rect = Rect::from_min_max(pos2(body.left(), top_y), pos2(body.right(), top_y + *h));
            place_row(&mut laid, true, *index, rect, &self.top[*index]);
            top_y = rect.bottom();
        }

        let mid = Rect::from_min_max(pos2(body.left(), top_y.max(body.top())), body.max);
        laid.mid = mid;

        // 2. Left and Right columns inside `mid`.
        let tools = |column: &Column| {
            column
                .palettes
                .iter()
                .any(|(palette, _)| *palette == Palette::Tools)
        };
        let mut sides: Vec<(Side, usize, f32, bool)> = Vec::new();
        let mut left_icons: Vec<Palette> = Vec::new();
        let mut folded_right: Vec<Palette> = Vec::new();
        for (side, columns) in [(Side::Left, &self.left), (Side::Right, &self.right)] {
            if self.hidden[side.index()] {
                continue;
            }
            for (index, column) in columns.iter().enumerate() {
                if visible_col(column).is_empty() {
                    continue;
                }
                if self.folded[side.index()] && !tools(column) {
                    let into = if side == Side::Left {
                        &mut left_icons
                    } else {
                        &mut folded_right
                    };
                    into.extend(visible_col(column).iter().map(|(palette, _)| *palette));
                } else {
                    sides.push((side, index, width_of(column), true));
                }
            }
        }
        let mut icons: Vec<Palette> = if self.hidden[1] {
            Vec::new()
        } else {
            self.iconized
                .iter()
                .copied()
                .filter(|palette| shown(*palette))
                .collect()
        };
        icons.extend(folded_right);
        // Too narrow: the widest columns collapse to icons (never the tools).
        loop {
            let used: f32 = sides
                .iter()
                .filter(|(_, _, _, open)| *open)
                .map(|(_, _, width, _)| *width)
                .sum();
            let strip = if icons.is_empty() { 0.0 } else { STRIP }
                + if left_icons.is_empty() { 0.0 } else { STRIP };
            if used + strip + MIN_CANVAS <= mid.width() {
                break;
            }
            let widest = sides
                .iter_mut()
                .filter(|(side, index, _, open)| {
                    let column = match side {
                        Side::Left => &self.left[*index],
                        Side::Right => &self.right[*index],
                    };
                    *open
                        && !visible_col(column)
                            .iter()
                            .any(|(palette, _)| *palette == Palette::Tools)
                })
                .max_by(|a, b| a.2.total_cmp(&b.2));
            let Some(widest) = widest else {
                break;
            };
            widest.3 = false;
            let column = match widest.0 {
                Side::Left => &self.left[widest.1],
                Side::Right => &self.right[widest.1],
            };
            icons.extend(visible_col(column).iter().map(|(palette, _)| *palette));
        }
        let open = |side: Side| -> Vec<(usize, f32)> {
            sides
                .iter()
                .filter(|(found, _, _, open)| *found == side && *open)
                .map(|(_, index, width, _)| (*index, *width))
                .collect()
        };
        let (left, right) = (open(Side::Left), open(Side::Right));
        let strip_width = if icons.is_empty() && !(self.folded[1] && !self.hidden[1]) {
            0.0
        } else {
            STRIP
        };
        let left_strip_width = if self.folded[0] && !self.hidden[0] {
            STRIP
        } else {
            0.0
        };
        let left_handle = if self.hidden[0] { HANDLE } else { 0.0 };
        let right_handle = if self.hidden[1] { HANDLE } else { 0.0 };
        let left_total: f32 = left.iter().map(|(_, width)| *width).sum::<f32>() + left_strip_width;
        let right_total: f32 = right.iter().map(|(_, width)| *width).sum::<f32>() + strip_width;

        laid.icons = icons;
        laid.left_icons = left_icons;

        if self.hidden[0] {
            laid.handles.push((
                Side::Left,
                Rect::from_min_max(mid.min, pos2(mid.left() + HANDLE, mid.bottom())),
            ));
        }
        if self.hidden[1] {
            laid.handles.push((
                Side::Right,
                Rect::from_min_max(pos2(mid.right() - HANDLE, mid.top()), mid.max),
            ));
        }
        let place_column = |laid: &mut Laid, side: Side, index: usize, rect: Rect| {
            let column = match side {
                Side::Left => &self.left[index],
                Side::Right => &self.right[index],
            };
            laid.columns.push((side, index, rect));
            let rect = Rect::from_min_max(pos2(rect.left(), rect.top() + COLUMN_BAR), rect.max);
            let palettes = visible_col(column);
            let total: f32 = palettes.iter().map(|(_, weight)| weight.max(0.01)).sum();
            let mut heights: Vec<f32> = palettes
                .iter()
                .map(|(palette, weight)| {
                    (rect.height() * weight.max(0.01) / total)
                        .max(palette.min_height().min(rect.height()))
                })
                .collect();
            let excess: f32 = heights.iter().sum::<f32>() - rect.height();
            if excess > 0.0
                && let Some(tallest) = heights.iter_mut().max_by(|a, b| a.total_cmp(b))
            {
                *tallest = (*tallest - excess).max(24.0);
            }
            let mut top = rect.top();
            for (at, ((palette, _), height)) in palettes.iter().zip(&heights).enumerate() {
                let piece =
                    Rect::from_min_size(pos2(rect.left(), top), vec2(rect.width(), *height));
                laid.docked.push((*palette, piece));
                top = piece.bottom();
                if let Some((lower, _)) = palettes.get(at + 1) {
                    laid.splits.push((
                        side,
                        index,
                        *palette,
                        *lower,
                        Rect::from_center_size(pos2(rect.center().x, top), vec2(rect.width(), 8.0)),
                        rect.height(),
                    ));
                }
            }
            // Every column (including Tools and Tool Sliders) can be resized.
            let edge_x = match side {
                Side::Left => rect.right(),
                Side::Right => rect.left(),
            };
            laid.edges.push((
                side,
                index,
                Rect::from_center_size(pos2(edge_x, rect.center().y), vec2(8.0, rect.height())),
            ));
        };
        let mut x = mid.left() + left_handle - hide * left_total;
        if left_strip_width > 0.0 {
            laid.left_strip = Some(Rect::from_min_max(
                pos2(x, mid.top()),
                pos2(x + STRIP, mid.bottom()),
            ));
            x += STRIP;
        }
        for (index, width) in &left {
            let rect = Rect::from_min_max(pos2(x, mid.top()), pos2(x + width, mid.bottom()));
            place_column(&mut laid, Side::Left, *index, rect);
            x = rect.right();
        }
        let center_left = x;
        let mut x = mid.right() - right_handle + hide * right_total;
        if strip_width > 0.0 {
            laid.strip = Some(Rect::from_min_max(
                pos2(x - STRIP, mid.top()),
                pos2(x, mid.bottom()),
            ));
            x -= STRIP;
        }
        for (index, width) in &right {
            let rect = Rect::from_min_max(pos2(x - width, mid.top()), pos2(x, mid.bottom()));
            place_column(&mut laid, Side::Right, *index, rect);
            x = rect.left();
        }
        let center_col = Rect::from_min_max(
            pos2(center_left, mid.top()),
            pos2(x.max(center_left), mid.bottom()),
        );
        laid.center_column = center_col;

        // 3. Bottom rows across `center_col` (between left and right columns).
        let mut bottom_y = center_col.bottom();
        if !both_closed && center_col.height() > 260.0 && center_col.width() > 220.0 {
            let mut bottom_rows: Vec<(usize, f32)> = Vec::new();
            let mut budget = (center_col.height() - 180.0).max(0.0);
            for (index, row) in self.bottom.iter().enumerate() {
                if visible_row(row).is_empty() {
                    continue;
                }
                let h = height_of_row(row, true).min(budget);
                if h >= 32.0 {
                    bottom_rows.push((index, h));
                    budget = (budget - h).max(0.0);
                }
            }
            let bottom_total: f32 = bottom_rows.iter().map(|(_, h)| *h).sum();
            let slide = hide * bottom_total;
            let mut cur_bottom = center_col.bottom() + slide;
            for (index, h) in &bottom_rows {
                let top = cur_bottom - *h;
                let rect = Rect::from_min_max(
                    pos2(center_col.left(), top),
                    pos2(center_col.right(), cur_bottom),
                );
                place_row(&mut laid, false, *index, rect, &self.bottom[*index]);
                cur_bottom = top;
            }
            bottom_y = cur_bottom.min(center_col.bottom());
        }

        laid.center = Rect::from_min_max(
            center_col.min,
            pos2(center_col.right(), bottom_y.max(center_col.top())),
        );
        laid
    }

    fn column_mut(&mut self, side: Side, index: usize) -> Option<&mut Column> {
        match side {
            Side::Left => self.left.get_mut(index),
            Side::Right => self.right.get_mut(index),
        }
    }

    fn row_mut(&mut self, top: bool, index: usize) -> Option<&mut Row> {
        if top {
            self.top.get_mut(index)
        } else {
            self.bottom.get_mut(index)
        }
    }

    fn apply_drop(&mut self, palette: Palette, target: DropTarget) {
        match target {
            DropTarget::InColumn {
                side, column, at, ..
            } => {
                self.hidden[side.index()] = false;
                self.folded[side.index()] = false;
                self.dock_in_column(palette, side, column, at);
            }
            DropTarget::NewColumn { side, at, .. } => {
                self.hidden[side.index()] = false;
                self.folded[side.index()] = false;
                self.dock_new_column(palette, side, at);
            }
            DropTarget::InRow { top, row, at, .. } => {
                self.dock_in_row(palette, top, row, at);
            }
            DropTarget::NewRow { top, at, .. } => {
                self.dock_new_row(palette, top, at);
            }
            DropTarget::Iconize { .. } => {
                self.iconize(palette);
            }
            DropTarget::Float { rect } => {
                self.float_rect(palette, rect);
            }
        }
    }

    /// Computes where `palette` would drop at `pointer`.
    /// When `from_floating` is `true`, only explicit dock bands (outer edges,
    /// column/row edges, headers, and strips) snap to dock so moving a
    /// floating window around the canvas stays smooth.
    #[allow(clippy::too_many_lines, reason = "evaluates every dock zone cleanly")]
    fn drop_target(
        &self,
        laid: &Laid,
        body: Rect,
        palette: Palette,
        pointer: Pos2,
        grab_offset: Vec2,
        from_floating: bool,
    ) -> Option<DropTarget> {
        // 1. Icon strips.
        if let Some(strip) = laid.strip
            && strip.expand(4.0).contains(pointer)
        {
            return Some(DropTarget::Iconize { preview: strip });
        }
        if let Some(strip) = laid.left_strip
            && strip.expand(4.0).contains(pointer)
        {
            return Some(DropTarget::Iconize { preview: strip });
        }

        // 2. Outer top edge -> new top row.
        let top_band = Rect::from_min_max(body.min, pos2(body.right(), body.top() + 18.0));
        if top_band.contains(pointer) {
            return Some(DropTarget::NewRow {
                top: true,
                at: 0,
                preview: Rect::from_min_max(body.min, pos2(body.right(), body.top() + 44.0)),
            });
        }
        // Existing top rows: left/right split or new row below.
        for (is_top, row_idx, rect) in &laid.rows {
            if !*is_top || !rect.contains(pointer) {
                continue;
            }
            if pointer.y > rect.bottom() - 10.0 {
                return Some(DropTarget::NewRow {
                    top: true,
                    at: row_idx + 1,
                    preview: Rect::from_min_max(
                        pos2(body.left(), rect.bottom() - 4.0),
                        pos2(body.right(), rect.bottom() + 40.0),
                    ),
                });
            }
            if !from_floating || pointer.y < rect.top() + 28.0 {
                let row = &self.top[*row_idx];
                let at = if pointer.x < rect.center().x {
                    0
                } else {
                    row.palettes.len()
                };
                let preview = if pointer.x < rect.center().x {
                    Rect::from_min_max(rect.min, pos2(rect.center().x, rect.bottom()))
                } else {
                    Rect::from_min_max(pos2(rect.center().x, rect.top()), rect.max)
                };
                return Some(DropTarget::InRow {
                    top: true,
                    row: *row_idx,
                    at,
                    preview,
                });
            }
        }

        // 3. Outer left / right edges and column boundaries -> new column.
        let preview_w = palette.default_column_width().unwrap_or(180.0).min(220.0);
        if Rect::from_min_max(
            laid.mid.min,
            pos2(laid.mid.left() + 20.0, laid.mid.bottom()),
        )
        .contains(pointer)
        {
            return Some(DropTarget::NewColumn {
                side: Side::Left,
                at: 0,
                preview: Rect::from_min_max(
                    laid.mid.min,
                    pos2(laid.mid.left() + preview_w, laid.mid.bottom()),
                ),
            });
        }
        let right_outer = laid.strip.map_or(laid.mid.right(), |s| s.left());
        if Rect::from_min_max(
            pos2(right_outer - 20.0, laid.mid.top()),
            pos2(right_outer, laid.mid.bottom()),
        )
        .contains(pointer)
        {
            return Some(DropTarget::NewColumn {
                side: Side::Right,
                at: 0,
                preview: Rect::from_min_max(
                    pos2(right_outer - preview_w, laid.mid.top()),
                    pos2(right_outer, laid.mid.bottom()),
                ),
            });
        }
        // Inner edges of left and right columns -> new column beside them.
        for (side, col_idx, edge_rect) in &laid.edges {
            if edge_rect.expand2(vec2(10.0, 0.0)).contains(pointer) {
                let cx = edge_rect.center().x;
                let preview = match side {
                    Side::Left => Rect::from_min_max(
                        pos2(cx - 8.0, laid.mid.top()),
                        pos2(cx + preview_w * 0.65, laid.mid.bottom()),
                    ),
                    Side::Right => Rect::from_min_max(
                        pos2(cx - preview_w * 0.65, laid.mid.top()),
                        pos2(cx + 8.0, laid.mid.bottom()),
                    ),
                };
                return Some(DropTarget::NewColumn {
                    side: *side,
                    at: col_idx + 1,
                    preview,
                });
            }
        }

        // 4. Bottom of `center_column` or existing bottom rows.
        let bottom_band = Rect::from_min_max(
            pos2(laid.center_column.left(), laid.center.bottom() - 22.0),
            pos2(laid.center_column.right(), laid.center.bottom() + 6.0),
        );
        if bottom_band.contains(pointer) {
            let at = self.bottom.len();
            return Some(DropTarget::NewRow {
                top: false,
                at,
                preview: Rect::from_min_max(
                    pos2(laid.center_column.left(), laid.center.bottom() - 110.0),
                    pos2(laid.center_column.right(), laid.center.bottom()),
                ),
            });
        }
        for (is_top, row_idx, rect) in &laid.rows {
            if *is_top || !rect.contains(pointer) {
                continue;
            }
            if pointer.y > rect.bottom() - 14.0 {
                return Some(DropTarget::NewRow {
                    top: false,
                    at: *row_idx,
                    preview: Rect::from_min_max(pos2(rect.left(), rect.bottom() - 60.0), rect.max),
                });
            }
            if !from_floating || pointer.y < rect.top() + 30.0 {
                let row = &self.bottom[*row_idx];
                let at = if pointer.x < rect.center().x {
                    0
                } else {
                    row.palettes.len()
                };
                let preview = if pointer.x < rect.center().x {
                    Rect::from_min_max(rect.min, pos2(rect.center().x, rect.bottom()))
                } else {
                    Rect::from_min_max(pos2(rect.center().x, rect.top()), rect.max)
                };
                return Some(DropTarget::InRow {
                    top: false,
                    row: *row_idx,
                    at,
                    preview,
                });
            }
        }

        // 5. Stacking above/below an existing docked column palette.
        for (side, col_idx, col_rect) in &laid.columns {
            if !col_rect.contains(pointer) {
                continue;
            }
            let col = match side {
                Side::Left => &self.left[*col_idx],
                Side::Right => &self.right[*col_idx],
            };
            for (pal, piece) in &laid.docked {
                if !col_rect.intersects(*piece) || !piece.contains(pointer) {
                    continue;
                }
                // For an already-floating window, snap into a column palette
                // only when hovering near its header or bottom split.
                if from_floating
                    && pointer.y > piece.top() + 32.0
                    && pointer.y < piece.bottom() - 20.0
                {
                    break;
                }
                let pos_in_col = col
                    .palettes
                    .iter()
                    .position(|(found, _)| found == pal)
                    .unwrap_or(0);
                if pointer.y < piece.center().y {
                    return Some(DropTarget::InColumn {
                        side: *side,
                        column: *col_idx,
                        at: pos_in_col,
                        preview: Rect::from_min_max(
                            piece.min,
                            pos2(piece.right(), piece.center().y),
                        ),
                    });
                }
                return Some(DropTarget::InColumn {
                    side: *side,
                    column: *col_idx,
                    at: pos_in_col + 1,
                    preview: Rect::from_min_max(pos2(piece.left(), piece.center().y), piece.max),
                });
            }
        }

        if from_floating {
            return None;
        }

        // 6. Dragged out over the canvas -> float!
        let size = palette.float_size();
        let offset = vec2(
            grab_offset.x.clamp(16.0, size.x - 16.0),
            grab_offset.y.clamp(8.0, 22.0),
        );
        let min = pos2(
            (pointer.x - offset.x).clamp(
                body.left() + 8.0,
                (body.right() - size.x).max(body.left() + 8.0),
            ),
            (pointer.y - offset.y).clamp(
                body.top() + 8.0,
                (body.bottom() - 48.0).max(body.top() + 8.0),
            ),
        );
        Some(DropTarget::Float {
            rect: Rect::from_min_size(min, size),
        })
    }
}

/// One or two small arrows in `rect` pointing right (`point` 1) or left
/// (-1), as Clip Studio Paint's palette arrows. `true` when clicked.
fn arrows(
    ui: &mut Ui,
    rect: Rect,
    count: usize,
    point: f32,
    id: impl std::hash::Hash + std::fmt::Debug,
    tip: &str,
    theme: &Theme,
) -> bool {
    let response = ui
        .interact(rect, Id::unique(id), Sense::click())
        .on_hover_text(tip);
    if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, theme.hover);
    }
    let color = if response.hovered() {
        theme.text
    } else {
        theme.muted
    };
    let c = rect.center();
    let offsets: &[f32] = if count == 2 { &[-2.0, 2.0] } else { &[0.0] };
    for offset in offsets {
        let x = c.x + offset;
        ui.painter().add(egui::Shape::line(
            vec![
                pos2(x - 1.5 * point, c.y - 3.0),
                pos2(x + 1.5 * point, c.y),
                pos2(x - 1.5 * point, c.y + 3.0),
            ],
            Stroke::new(1.0, color),
        ));
    }
    response.clicked()
}

/// Draws subtle grip dots inside `rect` and starts dragging `palette` when dragged.
pub(crate) fn grip_handle(
    dock: &mut Dock,
    ui: &mut Ui,
    rect: Rect,
    palette: Palette,
    panel_rect: Rect,
    vertical_dots: bool,
    theme: &Theme,
) {
    let response = ui
        .interact(
            rect,
            Id::unique((
                "sue-paint-grip",
                palette.key(),
                panel_rect.min.x as i32,
                panel_rect.min.y as i32,
            )),
            Sense::drag(),
        )
        .on_hover_text(tr!("dock-float-or-collapse", title = palette.title()));
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
        ui.painter().rect_filled(rect, 2.0, theme.hover);
    }
    let color = if response.hovered() {
        theme.text
    } else {
        theme.muted.gamma_multiply(0.75)
    };
    let c = rect.center();
    for step in [-4.0_f32, 0.0, 4.0] {
        let dot = if vertical_dots {
            pos2(c.x, c.y + step)
        } else {
            pos2(c.x + step, c.y)
        };
        ui.painter().circle_filled(dot, 1.2, color);
    }
    if response.drag_started()
        && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
    {
        dock.begin_move(palette, pointer, panel_rect, None);
    }
}

impl SuePaint {
    /// Draws one palette into `rect`.
    pub(crate) fn draw_palette(
        &mut self,
        ui: &mut Ui,
        palette: Palette,
        rect: Rect,
        cx: &mut AppletContext<'_>,
        theme: &Theme,
    ) {
        ui.painter().rect_filled(rect, 0.0, theme.panel);
        match palette {
            Palette::CommandBar => self.command_bar(ui, rect, cx, theme),
            Palette::Tools => self.tool_strip(ui, rect, cx, theme),
            Palette::SubTool => self.tool_panel(ui, rect, cx, theme),
            Palette::BrushSize => self.size_column(ui, rect, theme),
            Palette::Color => self.color_panel(ui, rect, theme),
            Palette::ColorSet => self.color_set_panel(ui, rect, cx, theme),
            Palette::Navigator => self.navigator(ui, rect, cx, theme),
            Palette::Layer => self.layer_panel(ui, rect, cx, theme),
            Palette::Timeline => self.timeline_panel(ui, rect, cx, theme),
            other => self.single_palette(ui, other, rect, cx, theme),
        }
    }

    /// Ensures `palette` is visible in the Window menu flags.
    pub(crate) fn set_palette_shown(&mut self, palette: Palette, on: bool) {
        match palette {
            Palette::CommandBar => self.panels.command_bar = on,
            Palette::Tools => self.panels.tool = on,
            Palette::SubTool => {
                self.panels.sub_tool = on;
                if !on {
                    self.panels.tool_property = false;
                }
            }
            Palette::BrushSize => self.panels.brush_size = on,
            Palette::Color => self.panels.color = on,
            Palette::ColorSet => self.panels.color_set = on,
            Palette::Navigator => self.panels.navigator = on,
            Palette::Layer => self.panels.layer = on,
            Palette::Timeline => self.panels.timeline = on,
            other if !on => {
                if STRIP_PALETTES.contains(&other) {
                    self.dock.iconize(other);
                } else {
                    self.dock.remove(other);
                }
            }
            _ => {}
        }
    }

    /// A palette's menu button, at its header's right end, and its menu.
    fn palette_menu(
        &mut self,
        ui: &mut Ui,
        palette: Palette,
        rect: Rect,
        floating: bool,
        theme: &Theme,
    ) -> bool {
        let button = match palette {
            Palette::Tools if rect.width() < 76.0 && rect.height() > rect.width() => {
                Rect::from_min_size(
                    pos2(rect.center().x - 9.0, rect.bottom() - 20.0),
                    vec2(18.0, 18.0),
                )
            }
            Palette::CommandBar => Rect::from_min_size(
                pos2(
                    rect.right() - 22.0,
                    rect.top() + (rect.height().min(38.0) - 18.0) * 0.5,
                ),
                vec2(18.0, 18.0),
            ),
            _ if floating => Rect::from_min_size(
                pos2(rect.right() - 42.0, rect.top() + 5.0),
                vec2(18.0, 18.0),
            ),
            _ => Rect::from_min_size(
                pos2(rect.right() - 22.0, rect.top() + 5.0),
                vec2(18.0, 18.0),
            ),
        };
        let open = self.dock.menu == Some(palette);
        if icon_button(
            ui,
            button,
            Icon::Menu,
            true,
            open,
            &tr!("dock-float-or-collapse", title = palette.title()),
            theme,
        ) {
            self.dock.menu = if open { None } else { Some(palette) };
        }
        if self.dock.menu != Some(palette) {
            return false;
        }
        let side = self.dock.side_of(palette);
        let band = self.dock.band_of(palette);
        let can_detach = !ui.ctx().embed_viewports();
        let mut chosen: Option<&str> = None;
        let shown = egui::Area::new(Id::unique(("sue-paint-dock-menu", palette.key())))
            .order(Order::Tooltip)
            .fade_in(false)
            .fixed_pos(pos2(button.right() - 196.0, button.bottom() + 2.0).max(pos2(4.0, 4.0)))
            .show(ui.ctx(), |ui| {
                sue_wm::widgets::window_frame(theme)
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_min_width(184.0);
                        let mut item = |ui: &mut Ui, label: &str, key: &'static str| {
                            if ui.add(egui::Button::new(label).frame(false)).clicked() {
                                chosen = Some(key);
                            }
                        };
                        if side != Some(Side::Left) {
                            item(ui, &tr!("dock-on-the-left"), "left");
                        }
                        item(ui, &tr!("new-column-on-the-left"), "new_left");
                        if side != Some(Side::Right) {
                            item(ui, &tr!("dock-on-the-right"), "right");
                        }
                        item(ui, &tr!("new-column-on-the-right"), "new_right");
                        if band != Some(true) {
                            item(ui, &tr!("dock-at-the-top"), "top");
                        }
                        if band != Some(false) {
                            item(ui, &tr!("dock-at-the-bottom"), "bottom");
                        }
                        if !floating {
                            item(ui, &tr!("float-in-a-window"), "float");
                        }
                        if can_detach {
                            item(ui, &tr!("detach-window"), "detach");
                        }
                        if !self.dock.iconized.contains(&palette) {
                            item(ui, &tr!("collapse-to-an-icon"), "icon");
                        }
                        item(ui, &tr!("close"), "close");
                    });
            });
        let inside = shown.response.rect;
        let outside = ui.input(|input| {
            input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|point| !inside.contains(point) && !button.contains(point))
        });
        let mut changed = false;
        match chosen {
            Some("left") => {
                self.dock.hidden[0] = false;
                self.dock.folded[0] = false;
                self.dock.dock(palette, Side::Left);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("new_left") => {
                let at = self.dock.left.len();
                self.dock.hidden[0] = false;
                self.dock.folded[0] = false;
                self.dock.dock_new_column(palette, Side::Left, at);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("right") => {
                self.dock.hidden[1] = false;
                self.dock.folded[1] = false;
                self.dock.dock(palette, Side::Right);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("new_right") => {
                let at = self.dock.right.len();
                self.dock.hidden[1] = false;
                self.dock.folded[1] = false;
                self.dock.dock_new_column(palette, Side::Right, at);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("top") => {
                let at = self.dock.top.len();
                self.dock.dock_new_row(palette, true, at);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("bottom") => {
                self.dock.dock_new_row(palette, false, 0);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("float") => {
                let at = (rect.min + vec2(24.0, 24.0)).max(pos2(16.0, 48.0));
                self.dock.float(palette, at);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("detach") => {
                let at = (rect.min + vec2(32.0, 32.0)).max(pos2(24.0, 48.0));
                self.dock.detach(palette, at);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("icon") => {
                self.dock.iconize(palette);
                self.set_palette_shown(palette, true);
                changed = true;
            }
            Some("close") => {
                self.close_palette(palette);
                changed = true;
            }
            _ => {}
        }
        if chosen.is_some() || outside {
            self.dock.menu = None;
        }
        changed
    }

    /// Hides `palette` from the Window menu's point of view.
    pub(crate) fn close_palette(&mut self, palette: Palette) {
        self.set_palette_shown(palette, false);
        if self.dock.popped == Some(palette) {
            self.dock.popped = None;
        }
    }

    /// The palettes, docked, collapsed and floating, around the canvas.
    /// Returns what is left for the canvas.
    pub(crate) fn dock_ui(
        &mut self,
        ui: &mut Ui,
        body: Rect,
        cx: &mut AppletContext<'_>,
        theme: &Theme,
    ) -> Rect {
        let panels = self.panels;
        let menus_hidden = self.menus_hidden;
        let palettes_hidden = self.hidden_panels.is_some();
        let shown = move |palette: Palette| {
            if palette == Palette::CommandBar && menus_hidden {
                return false;
            }
            if palettes_hidden && palette != Palette::CommandBar {
                return false;
            }
            palette.shown(&panels)
        };
        let laid = self.dock.layout(body, &shown, self.hide);
        for (_, _, rect) in &laid.columns {
            ui.painter().rect_filled(*rect, 0.0, theme.panel);
            let bar = Rect::from_min_size(rect.min, vec2(rect.width(), COLUMN_BAR));
            ui.painter().rect_filled(bar, 0.0, theme.void);
        }
        for (_, _, rect) in &laid.rows {
            ui.painter().rect_filled(*rect, 0.0, theme.panel);
        }
        for (palette, rect) in laid.bands.iter().chain(laid.docked.iter()) {
            self.draw_palette(ui, *palette, *rect, cx, theme);
            ui.painter()
                .hline(rect.x_range(), rect.bottom(), theme.hairline());
            ui.painter()
                .vline(rect.right(), rect.y_range(), theme.hairline());
        }
        for (side, _, rect) in &laid.columns {
            let x = match side {
                Side::Left => rect.right(),
                Side::Right => rect.left(),
            };
            ui.painter().vline(x, rect.y_range(), theme.hairline());
        }
        let mut changed = false;
        if self.ready {
            for (palette, rect) in laid.bands.iter().chain(laid.docked.iter()) {
                if self.palette_menu(ui, *palette, *rect, false, theme) {
                    changed = true;
                }
            }
        }
        if let Some(strip) = laid.strip
            && self.strip_ui(ui, strip, Side::Right, &laid.icons, cx, theme)
        {
            changed = true;
        }
        if let Some(strip) = laid.left_strip
            && self.strip_ui(ui, strip, Side::Left, &laid.left_icons, cx, theme)
        {
            changed = true;
        }
        if self.side_arrows(ui, &laid, theme) {
            changed = true;
        }
        if self.ready {
            if self.floating_ui(ui, body, &shown, cx, theme) {
                changed = true;
            }
            if self.splitters_and_moves(ui, body, &laid, theme) {
                changed = true;
            }
        }
        if changed {
            self.layout_dirty = true;
        }
        if self.layout_dirty && !self.dock.is_dragging() {
            self.layout_dirty = false;
            self.save_current_workspace(cx);
        }
        laid.center
    }

    /// Handles column/row splitters, floating resizes, and live drag-to-move.
    /// Returns `true` when a drag finished changing the layout.
    #[allow(
        clippy::too_many_lines,
        reason = "all resize and move interactions in one place"
    )]
    fn splitters_and_moves(&mut self, ui: &mut Ui, body: Rect, laid: &Laid, theme: &Theme) -> bool {
        let pointer = ui.input(|input| {
            input
                .pointer
                .interact_pos()
                .or_else(|| input.pointer.latest_pos())
        });

        // Column width splitters.
        for (side, column, rect) in &laid.edges {
            let response = ui.interact(
                *rect,
                Id::unique(("sue-paint-edge", *side == Side::Left, *column)),
                Sense::drag(),
            );
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                ui.painter().vline(
                    rect.center().x,
                    rect.y_range(),
                    Stroke::new(2.0, theme.border_active),
                );
            }
            if response.drag_started()
                && let (Some(point), Some(found)) = (pointer, self.dock.column_mut(*side, *column))
            {
                self.dock.drag = Some(Drag::Edge {
                    side: *side,
                    column: *column,
                    from: point.x,
                    width: found.width,
                });
            }
        }

        // Intra-column height splitters.
        for (side, column, upper, lower, rect, height) in &laid.splits {
            let response = ui.interact(
                *rect,
                Id::unique(("sue-paint-split", *side == Side::Left, *column, upper.key())),
                Sense::drag(),
            );
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
                ui.painter().hline(
                    rect.x_range(),
                    rect.center().y,
                    Stroke::new(2.0, theme.border_active),
                );
            }
            if response.drag_started()
                && let (Some(point), Some(found)) = (pointer, self.dock.column_mut(*side, *column))
            {
                let weight = |palette: Palette| {
                    found
                        .palettes
                        .iter()
                        .find(|(p, _)| *p == palette)
                        .map_or(0.5, |(_, w)| *w)
                };
                let total: f32 = found.palettes.iter().map(|(_, w)| w.max(0.01)).sum();
                self.dock.drag = Some(Drag::Split {
                    side: *side,
                    column: *column,
                    upper: *upper,
                    lower: *lower,
                    from: point.y,
                    weights: (weight(*upper), weight(*lower)),
                    per_point: total / height.max(1.0),
                });
            }
        }

        // Top and bottom row height splitters.
        for (is_top, row, rect) in &laid.row_edges {
            let response = ui.interact(
                *rect,
                Id::unique(("sue-paint-row-edge", *is_top, *row)),
                Sense::drag(),
            );
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
                ui.painter().hline(
                    rect.x_range(),
                    rect.center().y,
                    Stroke::new(2.0, theme.border_active),
                );
            }
            if response.drag_started()
                && let (Some(point), Some(found)) = (pointer, self.dock.row_mut(*is_top, *row))
            {
                self.dock.drag = Some(Drag::RowEdge {
                    top: *is_top,
                    row: *row,
                    from: point.y,
                    height: found.height,
                });
            }
        }

        // Intra-row width splitters.
        for (is_top, row, left_pal, right_pal, rect, width) in &laid.row_splits {
            let response = ui.interact(
                *rect,
                Id::unique(("sue-paint-row-split", *is_top, *row, left_pal.key())),
                Sense::drag(),
            );
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                ui.painter().vline(
                    rect.center().x,
                    rect.y_range(),
                    Stroke::new(2.0, theme.border_active),
                );
            }
            if response.drag_started()
                && let (Some(point), Some(found)) = (pointer, self.dock.row_mut(*is_top, *row))
            {
                let weight = |palette: Palette| {
                    found
                        .palettes
                        .iter()
                        .find(|(p, _)| *p == palette)
                        .map_or(0.5, |(_, w)| *w)
                };
                let total: f32 = found.palettes.iter().map(|(_, w)| w.max(0.01)).sum();
                self.dock.drag = Some(Drag::RowSplit {
                    top: *is_top,
                    row: *row,
                    left: *left_pal,
                    right: *right_pal,
                    from: point.x,
                    weights: (weight(*left_pal), weight(*right_pal)),
                    per_point: total / width.max(1.0),
                });
            }
        }

        let (down, suppress_dock) = ui.input(|input| {
            (
                input.pointer.primary_down(),
                input.modifiers.ctrl || input.modifiers.shift,
            )
        });
        let mut finished = false;

        match (self.dock.drag, pointer) {
            (
                Some(Drag::Edge {
                    side,
                    column,
                    from,
                    width,
                }),
                Some(point),
            ) if down => {
                let delta = match side {
                    Side::Left => point.x - from,
                    Side::Right => from - point.x,
                };
                if let Some(found) = self.dock.column_mut(side, column) {
                    let min_w = found
                        .palettes
                        .iter()
                        .map(|(p, _)| p.min_width())
                        .fold(38.0_f32, f32::max);
                    found.width = (width + delta).clamp(min_w, 640.0);
                }
            }
            (
                Some(Drag::Split {
                    side,
                    column,
                    upper,
                    lower,
                    from,
                    weights,
                    per_point,
                }),
                Some(point),
            ) if down => {
                let moved = (point.y - from) * per_point;
                let least = 0.06;
                let up = (weights.0 + moved).max(least);
                let low = (weights.0 + weights.1 - up).max(least);
                if let Some(found) = self.dock.column_mut(side, column) {
                    for (palette, weight) in &mut found.palettes {
                        if *palette == upper {
                            *weight = weights.0 + weights.1 - low;
                        } else if *palette == lower {
                            *weight = low;
                        }
                    }
                }
            }
            (
                Some(Drag::RowEdge {
                    top,
                    row,
                    from,
                    height,
                }),
                Some(point),
            ) if down => {
                let delta = if top { point.y - from } else { from - point.y };
                let sync_timeline = self.dock.row_mut(top, row).and_then(|found| {
                    let min_h = found
                        .palettes
                        .iter()
                        .map(|(p, _)| p.min_height())
                        .fold(34.0_f32, f32::max);
                    found.height = (height + delta).clamp(min_h, 580.0);
                    (!top && found.palettes.iter().any(|(p, _)| *p == Palette::Timeline))
                        .then_some(found.height)
                });
                if let Some(new_h) = sync_timeline {
                    self.dock.timeline = new_h;
                }
            }
            (
                Some(Drag::RowSplit {
                    top,
                    row,
                    left: left_pal,
                    right: right_pal,
                    from,
                    weights,
                    per_point,
                }),
                Some(point),
            ) if down => {
                let moved = (point.x - from) * per_point;
                let least = 0.06;
                let l = (weights.0 + moved).max(least);
                let r = (weights.0 + weights.1 - l).max(least);
                if let Some(found) = self.dock.row_mut(top, row) {
                    for (palette, weight) in &mut found.palettes {
                        if *palette == left_pal {
                            *weight = weights.0 + weights.1 - r;
                        } else if *palette == right_pal {
                            *weight = r;
                        }
                    }
                }
            }
            (
                Some(Drag::FloatResize {
                    palette,
                    start_rect,
                    from,
                    dx,
                    dy,
                }),
                Some(point),
            ) if down => {
                let delta = point - from;
                let min_w = palette.min_width();
                let min_h = palette.min_height() + 28.0;
                let mut left = start_rect.left();
                let mut right = start_rect.right();
                let mut top = start_rect.top();
                let mut bottom = start_rect.bottom();
                if dx < 0 {
                    left = (start_rect.left() + delta.x).min(right - min_w);
                } else if dx > 0 {
                    right = (start_rect.right() + delta.x).max(left + min_w);
                }
                if dy < 0 {
                    top = (start_rect.top() + delta.y).min(bottom - min_h);
                } else if dy > 0 {
                    bottom = (start_rect.bottom() + delta.y).max(top + min_h);
                }
                if let Some(entry) = self
                    .dock
                    .floating
                    .iter_mut()
                    .find(|(found, _)| *found == palette)
                {
                    entry.1 = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
                }
            }
            (
                Some(Drag::Move {
                    palette,
                    from,
                    grab_offset,
                    was_floating,
                    mut active,
                }),
                Some(point),
            ) if down => {
                if !active && (point - from).length() >= 5.0 {
                    active = true;
                    self.dock.drag = Some(Drag::Move {
                        palette,
                        from,
                        grab_offset,
                        was_floating,
                        active: true,
                    });
                }
                if active {
                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                    if let Some(start_rect) = was_floating {
                        let delta = point - from;
                        let size = start_rect.size();
                        let moved = Rect::from_min_size(
                            pos2(
                                (start_rect.left() + delta.x)
                                    .clamp(body.left() - size.x + 64.0, body.right() - 64.0),
                                (start_rect.top() + delta.y)
                                    .clamp(body.top(), (body.bottom() - 28.0).max(body.top())),
                            ),
                            size,
                        );
                        if let Some(entry) = self
                            .dock
                            .floating
                            .iter_mut()
                            .find(|(found, _)| *found == palette)
                        {
                            entry.1 = moved;
                        }
                    }
                    let target = if suppress_dock && was_floating.is_some() {
                        None
                    } else {
                        self.dock.drop_target(
                            laid,
                            body,
                            palette,
                            point,
                            grab_offset,
                            was_floating.is_some(),
                        )
                    };
                    let overlay = ui.ctx().layer_painter(egui::LayerId::new(
                        Order::Tooltip,
                        Id::unique("sue-paint-dock-preview"),
                    ));
                    if let Some(target) = target {
                        let preview = target.preview();
                        overlay.rect_filled(preview, 4.0, theme.selection.gamma_multiply(0.45));
                        overlay.rect_stroke(
                            preview,
                            4.0,
                            Stroke::new(2.0, theme.border_active),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if was_floating.is_none() {
                        let pill = Rect::from_min_size(point + vec2(12.0, 12.0), vec2(148.0, 26.0));
                        overlay.rect_filled(pill, 4.0, theme.void.gamma_multiply(0.92));
                        overlay.rect_stroke(
                            pill,
                            4.0,
                            Stroke::new(1.0, theme.border_active),
                            egui::StrokeKind::Inside,
                        );
                        icons::paint(
                            &overlay,
                            Rect::from_center_size(
                                pill.left_center() + vec2(14.0, 0.0),
                                vec2(15.0, 15.0),
                            ),
                            palette.icon(),
                            theme.text,
                        );
                        overlay.text(
                            pill.left_center() + vec2(26.0, 0.0),
                            Align2::LEFT_CENTER,
                            palette.title(),
                            theme.font(theme.small_size + 0.5),
                            theme.text,
                        );
                    }
                }
            }
            (Some(drag), release_point) if !down => {
                self.dock.drag = None;
                if let Drag::Move {
                    palette,
                    grab_offset,
                    was_floating,
                    active: true,
                    ..
                } = drag
                {
                    if let Some(point) = release_point
                        && !(suppress_dock && was_floating.is_some())
                        && let Some(target) = self.dock.drop_target(
                            laid,
                            body,
                            palette,
                            point,
                            grab_offset,
                            was_floating.is_some(),
                        )
                    {
                        self.dock.apply_drop(palette, target);
                        self.set_palette_shown(palette, true);
                    }
                    finished = true;
                } else if !matches!(drag, Drag::Move { active: false, .. }) {
                    finished = true;
                }
            }
            _ => {}
        }
        finished
    }

    /// Clip Studio Paint's arrows at the top of each side: one folds the
    /// side's palette columns into icons and opens them again, two close
    /// the whole side; a closed side leaves a thin bar whose arrows open it.
    fn side_arrows(&mut self, ui: &mut Ui, laid: &Laid, theme: &Theme) -> bool {
        let mut changed = false;
        for (side, handle) in &laid.handles {
            ui.painter().rect_filled(*handle, 0.0, theme.panel);
            let edge = match side {
                Side::Left => handle.right(),
                Side::Right => handle.left(),
            };
            ui.painter().vline(edge, handle.y_range(), theme.hairline());
            let top = Rect::from_min_size(handle.min, vec2(handle.width(), COLUMN_BAR + 4.0));
            let point = if *side == Side::Left { 1.0 } else { -1.0 };
            let id = ("sue-paint-open-side", *side == Side::Left);
            if arrows(ui, top, 2, point, id, &tr!("show-palettes"), theme) {
                self.dock.hidden[side.index()] = false;
                changed = true;
            }
        }
        for side in [Side::Left, Side::Right] {
            if self.dock.hidden[side.index()] {
                continue;
            }
            let strip = match side {
                Side::Left => laid.left_strip,
                Side::Right => laid.strip,
            };
            let outermost = laid
                .columns
                .iter()
                .filter(|(found, _, _)| *found == side)
                .map(|(_, _, rect)| *rect)
                .reduce(|a, b| match side {
                    Side::Left if b.left() < a.left() => b,
                    Side::Right if b.right() > a.right() => b,
                    _ => a,
                });
            let Some(host) = strip.or(outermost) else {
                continue;
            };
            let bar = Rect::from_min_size(host.min, vec2(host.width(), COLUMN_BAR));
            let out = if side == Side::Left { -1.0 } else { 1.0 };
            let (close_at, fold_at) = match side {
                Side::Left => (bar.left(), bar.left() + 16.0),
                Side::Right => (bar.right() - 16.0, bar.right() - 32.0),
            };
            let close = Rect::from_min_size(pos2(close_at, bar.top()), vec2(16.0, COLUMN_BAR));
            let close_id = ("sue-paint-close-side", side == Side::Left);
            if arrows(ui, close, 2, out, close_id, &tr!("hide-palettes"), theme) {
                self.dock.hidden[side.index()] = true;
                self.dock.popped = None;
                changed = true;
            }
            let columns = match side {
                Side::Left => &self.dock.left,
                Side::Right => &self.dock.right,
            };
            let foldable = columns.iter().any(|column| {
                !column
                    .palettes
                    .iter()
                    .any(|(palette, _)| *palette == Palette::Tools)
            });
            if !foldable {
                continue;
            }
            let folded = self.dock.folded[side.index()];
            let fold = Rect::from_min_size(pos2(fold_at, bar.top()), vec2(16.0, COLUMN_BAR));
            let (point, tip) = if folded {
                (-out, tr!("open-palettes"))
            } else {
                (out, tr!("fold-column-into-icons"))
            };
            let fold_id = ("sue-paint-fold-side", side == Side::Left);
            if arrows(ui, fold, 1, point, fold_id, &tip, theme) {
                self.dock.folded[side.index()] = !folded;
                self.dock.popped = None;
                changed = true;
            }
        }
        changed
    }

    /// An icon strip (the right one, or the left one while the left side
    /// is folded), and the collapsed palette it shows.
    fn strip_ui(
        &mut self,
        ui: &mut Ui,
        strip: Rect,
        side: Side,
        icons_shown: &[Palette],
        cx: &mut AppletContext<'_>,
        theme: &Theme,
    ) -> bool {
        let mut changed = false;
        ui.painter().rect_filled(strip, 0.0, theme.panel);
        ui.painter().rect_filled(
            Rect::from_min_size(strip.min, vec2(strip.width(), COLUMN_BAR)),
            0.0,
            theme.void,
        );
        let edge = match side {
            Side::Left => strip.right(),
            Side::Right => strip.left(),
        };
        ui.painter().vline(edge, strip.y_range(), theme.hairline());
        for (index, palette) in icons_shown.iter().enumerate() {
            let rect = Rect::from_min_size(
                pos2(
                    strip.left() + 4.0,
                    strip.top() + COLUMN_BAR + 6.0 + index as f32 * 30.0,
                ),
                vec2(26.0, 26.0),
            );
            let open = self.dock.popped == Some(*palette);
            let response = ui
                .interact(
                    rect,
                    Id::unique(("sue-paint-strip-icon", side == Side::Left, palette.key())),
                    Sense::click_and_drag(),
                )
                .on_hover_text(palette.title());
            if open {
                ui.painter()
                    .rect_filled(rect, theme.control_radius, theme.active);
            } else if response.hovered() {
                ui.painter()
                    .rect_filled(rect, theme.control_radius, theme.hover);
            }
            icons::paint(
                ui.painter(),
                rect.shrink(rect.width() * 0.13),
                palette.icon(),
                if open {
                    theme.text
                } else {
                    theme.text.gamma_multiply(0.85)
                },
            );
            if response.clicked() {
                self.dock.popped = if open { None } else { Some(*palette) };
            }
            if response.drag_started()
                && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
            {
                self.dock.begin_move(*palette, pointer, rect, None);
            }
        }
        let Some(palette) = self
            .dock
            .popped
            .filter(|palette| icons_shown.contains(palette))
        else {
            self.dock.popped = None;
            return changed;
        };
        let size = match palette.default_column_width() {
            Some(width) => vec2(width + 12.0, (strip.height() * 0.7).max(240.0)),
            None => vec2(280.0, (strip.height() * 0.7).clamp(220.0, 560.0)),
        };
        let at = match side {
            Side::Left => pos2(strip.right() + 4.0, strip.top() + 4.0),
            Side::Right => pos2(strip.left() - size.x - 4.0, strip.top() + 4.0),
        };
        let area = egui::Area::new(Id::unique(("sue-paint-popped", palette.key())))
            .order(Order::Foreground)
            .fade_in(false)
            .fixed_pos(at)
            .show(ui.ctx(), |ui| {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                ui.painter()
                    .rect_filled(rect.expand(1.0), 0.0, theme.border);
                self.draw_palette(ui, palette, rect, cx, theme);
                if self.palette_menu(ui, palette, rect, false, theme) {
                    changed = true;
                }
                rect
            });
        let inside = area.inner;
        let clicked_outside = ui.input(|input| {
            input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|point| !inside.contains(point) && !strip.contains(point))
        });
        if clicked_outside && self.dock.menu.is_none() && !self.dock.is_dragging() {
            self.dock.popped = None;
        }
        changed
    }

    /// Floating palettes (both in-workspace windows driven directly by
    /// `dock.floating` so workspace switches move and resize them on the
    /// exact frame, and detached system windows in `dock.detached`).
    #[allow(
        clippy::too_many_lines,
        reason = "floating window frame, 8-way resize and redock"
    )]
    fn floating_ui(
        &mut self,
        ui: &mut Ui,
        body: Rect,
        shown: &dyn Fn(Palette) -> bool,
        cx: &mut AppletContext<'_>,
        theme: &Theme,
    ) -> bool {
        let mut changed = false;
        let floating = self.dock.floating.clone();
        let mut raise: Option<Palette> = None;
        let mut redock_to: Option<(Palette, Side)> = None;
        let mut to_close: Option<Palette> = None;

        for (palette, raw_rect) in floating {
            if !shown(palette) {
                continue;
            }
            let min_w = palette.min_width();
            let min_h = palette.min_height() + 28.0;
            let size = vec2(raw_rect.width().max(min_w), raw_rect.height().max(min_h));
            let pos = pos2(
                raw_rect.left().clamp(
                    body.left() - size.x + 64.0,
                    (body.right() - 64.0).max(body.left()),
                ),
                raw_rect
                    .top()
                    .clamp(body.top(), (body.bottom() - 28.0).max(body.top())),
            );
            let win_rect = Rect::from_min_size(pos, size);

            egui::Area::new(Id::unique(("sue-paint-float-win", palette.key())))
                .order(Order::Middle)
                .fade_in(false)
                .fixed_pos(win_rect.min)
                .show(ui.ctx(), |ui| {
                    let (whole, bg_resp) = ui.allocate_exact_size(win_rect.size(), Sense::click());
                    if bg_resp.clicked() {
                        raise = Some(palette);
                    }
                    // Window shadow & frame.
                    ui.painter().add(egui::epaint::RectShape::filled(
                        whole.expand(1.0),
                        4.0,
                        theme.border,
                    ));
                    ui.painter().rect_filled(whole, 3.0, theme.panel);

                    // Title bar (drag to move or dock onto any edge/column/row).
                    let title_h = 26.0;
                    let title_rect = Rect::from_min_size(whole.min, vec2(whole.width(), title_h));
                    ui.painter().rect_filled(title_rect, 3.0, theme.void);
                    ui.painter()
                        .hline(title_rect.x_range(), title_rect.bottom(), theme.hairline());

                    let close_btn = Rect::from_min_size(
                        pos2(title_rect.right() - 21.0, title_rect.top() + 4.0),
                        vec2(18.0, 18.0),
                    );
                    let dock_btn = Rect::from_min_size(
                        pos2(title_rect.right() - 61.0, title_rect.top() + 4.0),
                        vec2(18.0, 18.0),
                    );
                    let drag_bar = Rect::from_min_max(
                        title_rect.min,
                        pos2(
                            (dock_btn.left() - 2.0).max(title_rect.left() + 12.0),
                            title_rect.bottom(),
                        ),
                    );

                    icons::paint(
                        ui.painter(),
                        Rect::from_center_size(
                            pos2(title_rect.left() + 13.0, title_rect.center().y),
                            vec2(14.0, 14.0),
                        ),
                        palette.icon(),
                        theme.text,
                    );
                    if drag_bar.width() > 32.0 {
                        ui.painter().with_clip_rect(drag_bar).text(
                            pos2(title_rect.left() + 25.0, title_rect.center().y),
                            Align2::LEFT_CENTER,
                            palette.title(),
                            theme.font(theme.small_size + 0.5),
                            theme.text,
                        );
                    }

                    let drag_resp = ui.interact(
                        drag_bar,
                        Id::unique(("sue-paint-float-drag", palette.key())),
                        Sense::click_and_drag(),
                    );
                    if drag_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::Grab);
                    }
                    if drag_resp.clicked() || drag_resp.drag_started() {
                        raise = Some(palette);
                    }
                    if drag_resp.drag_started()
                        && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
                    {
                        self.dock
                            .begin_move(palette, pointer, win_rect, Some(win_rect));
                    }

                    if drag_bar.width() > 20.0
                        && icon_button(
                            ui,
                            dock_btn,
                            Icon::Up,
                            true,
                            false,
                            &tr!("dock-it-again"),
                            theme,
                        )
                    {
                        let side = if win_rect.center().x < body.center().x {
                            Side::Left
                        } else {
                            Side::Right
                        };
                        redock_to = Some((palette, side));
                    }

                    if self.palette_menu(ui, palette, whole, true, theme) {
                        changed = true;
                    }

                    // Close button (`x`).
                    let close_resp = ui
                        .interact(
                            close_btn,
                            Id::unique(("sue-paint-float-close", palette.key())),
                            Sense::click(),
                        )
                        .on_hover_text(tr!("close"));
                    if close_resp.hovered() {
                        ui.painter()
                            .rect_filled(close_btn, theme.control_radius, theme.hover);
                    }
                    let c = close_btn.center();
                    let x_col = if close_resp.hovered() {
                        theme.text
                    } else {
                        theme.muted
                    };
                    ui.painter().line_segment(
                        [c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)],
                        Stroke::new(1.2, x_col),
                    );
                    ui.painter().line_segment(
                        [c + vec2(4.0, -4.0), c + vec2(-4.0, 4.0)],
                        Stroke::new(1.2, x_col),
                    );
                    if close_resp.clicked() {
                        to_close = Some(palette);
                    }

                    // Content area.
                    let content = Rect::from_min_max(
                        pos2(whole.left() + 1.0, title_rect.bottom()),
                        pos2(whole.right() - 1.0, whole.bottom() - 1.0),
                    );
                    self.draw_palette(ui, palette, content, cx, theme);

                    // Subtle bottom-right resize grip mark.
                    let br = whole.max;
                    for step in [4.0_f32, 8.0] {
                        ui.painter().line_segment(
                            [pos2(br.x - step, br.y - 2.0), pos2(br.x - 2.0, br.y - step)],
                            Stroke::new(1.0, theme.muted.gamma_multiply(0.6)),
                        );
                    }

                    // 8-way resize edges and corners.
                    let e = 5.0;
                    let k = 12.0;
                    let (l, r, t, b) = (whole.left(), whole.right(), whole.top(), whole.bottom());
                    let zones: [(Rect, i8, i8, CursorIcon); 8] = [
                        (
                            Rect::from_min_max(pos2(l - 2.0, t - 2.0), pos2(l + k, t + k)),
                            -1,
                            -1,
                            CursorIcon::ResizeNorthWest,
                        ),
                        (
                            Rect::from_min_max(pos2(r - k, t - 2.0), pos2(r + 2.0, t + k)),
                            1,
                            -1,
                            CursorIcon::ResizeNorthEast,
                        ),
                        (
                            Rect::from_min_max(pos2(l - 2.0, b - k), pos2(l + k, b + 2.0)),
                            -1,
                            1,
                            CursorIcon::ResizeSouthWest,
                        ),
                        (
                            Rect::from_min_max(pos2(r - k, b - k), pos2(r + 2.0, b + 2.0)),
                            1,
                            1,
                            CursorIcon::ResizeSouthEast,
                        ),
                        (
                            Rect::from_min_max(pos2(l + k, t - 2.0), pos2(r - k, t + e)),
                            0,
                            -1,
                            CursorIcon::ResizeNorth,
                        ),
                        (
                            Rect::from_min_max(pos2(l + k, b - e), pos2(r - k, b + 2.0)),
                            0,
                            1,
                            CursorIcon::ResizeSouth,
                        ),
                        (
                            Rect::from_min_max(pos2(l - 2.0, t + k), pos2(l + e, b - k)),
                            -1,
                            0,
                            CursorIcon::ResizeWest,
                        ),
                        (
                            Rect::from_min_max(pos2(r - e, t + k), pos2(r + 2.0, b - k)),
                            1,
                            0,
                            CursorIcon::ResizeEast,
                        ),
                    ];
                    for (z_idx, (zone, dx, dy, cursor)) in zones.into_iter().enumerate() {
                        let resp = ui.interact(
                            zone,
                            Id::unique(("sue-paint-float-rsz", palette.key(), z_idx)),
                            Sense::drag(),
                        );
                        if resp.hovered() || resp.dragged() {
                            ui.ctx().set_cursor_icon(cursor);
                        }
                        if resp.drag_started()
                            && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
                        {
                            raise = Some(palette);
                            self.dock.drag = Some(Drag::FloatResize {
                                palette,
                                start_rect: win_rect,
                                from: pointer,
                                dx,
                                dy,
                            });
                        }
                    }
                });
        }

        if let Some(palette) = raise
            && let Some(pos) = self
                .dock
                .floating
                .iter()
                .position(|(found, _)| *found == palette)
            && pos + 1 < self.dock.floating.len()
        {
            let item = self.dock.floating.remove(pos);
            self.dock.floating.push(item);
        }
        if let Some((palette, side)) = redock_to {
            self.dock.dock(palette, side);
            changed = true;
        }
        if let Some(palette) = to_close {
            self.close_palette(palette);
            changed = true;
        }

        // Detached OS viewport windows (for multi-monitor setups).
        if !ui.ctx().embed_viewports() {
            let detached = self.dock.detached.clone();
            for (palette, rect) in detached {
                if !shown(palette) {
                    continue;
                }
                if self.palette_window(ui, palette, rect, body, cx, theme) {
                    changed = true;
                }
            }
        }
        let _ = Color32::TRANSPARENT;
        changed
    }

    /// A detached palette in a separate system window: can move outside
    /// Sue's main window onto other monitors and resize freely.
    fn palette_window(
        &mut self,
        ui: &mut Ui,
        palette: Palette,
        rect: Rect,
        body: Rect,
        cx: &mut AppletContext<'_>,
        theme: &Theme,
    ) -> bool {
        let ctx = ui.ctx().clone();
        let main = ctx
            .input(|input| input.viewport().inner_rect)
            .map_or(Pos2::ZERO, |inner| inner.min);
        let own_frame = sue_wm::widgets::own_frames(&ctx);
        let bar = if own_frame {
            sue_wm::widgets::TITLE_BAR
        } else {
            0.0
        };
        let builder = egui::ViewportBuilder::default()
            .with_title(palette.title())
            .with_position(main + rect.min.to_vec2())
            .with_inner_size(rect.size() + vec2(0.0, bar))
            .with_min_inner_size(vec2(palette.min_width(), palette.min_height() + bar))
            .with_resizable(true)
            .with_decorations(!own_frame);
        let (closed, redock, now_rect, content_size) = ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of(("sue-paint-palette", palette.key())),
            builder,
            |ui, _| {
                let whole = ui.max_rect();
                let mut area = whole;
                let mut shut = false;
                if own_frame {
                    let top = Rect::from_min_size(whole.min, vec2(whole.width(), bar));
                    shut = sue_wm::widgets::title_bar(ui, top, &palette.title(), false, theme);
                    area.min.y = top.bottom();
                }
                self.draw_palette(ui, palette, area, cx, theme);
                let back = Rect::from_min_size(
                    pos2(area.right() - 22.0, area.bottom() - 22.0),
                    vec2(18.0, 18.0),
                );
                let redock = icon_button(
                    ui,
                    back,
                    Icon::Up,
                    true,
                    false,
                    &tr!("dock-it-again"),
                    theme,
                );
                if own_frame {
                    sue_wm::widgets::resize_edges(ui, whole);
                }
                let (closed, now) = ui.input(|input| {
                    let viewport = input.viewport();
                    (viewport.close_requested(), viewport.inner_rect)
                });
                (closed || shut, redock, now, area.size())
            },
        );
        let mut changed = false;
        if let Some(entry) = self
            .dock
            .detached
            .iter_mut()
            .find(|(found, _)| *found == palette)
        {
            let new_min = now_rect.map_or(entry.1.min, |now| now.min - main.to_vec2());
            let new_size = vec2(
                content_size.x.max(palette.min_width()),
                content_size.y.max(palette.min_height()),
            );
            let updated = Rect::from_min_size(new_min, new_size);
            if (updated.min - entry.1.min).length() > 0.5
                || (updated.size() - entry.1.size()).length() > 0.5
            {
                entry.1 = updated;
                changed = true;
            }
        }
        if closed || redock {
            let side = if rect.center().x < body.center().x {
                Side::Left
            } else {
                Side::Right
            };
            self.dock.dock(palette, side);
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_round_trips_as_lines() {
        let mut dock = Dock::default();
        dock.float(Palette::Color, pos2(40.0, 60.0));
        dock.iconize(Palette::ColorSet);
        dock.dock_new_row(Palette::Info, true, 1);
        dock.timeline = 210.0;
        let back = Dock::from_lines(&dock.to_lines()).unwrap();
        assert_eq!(back.to_lines(), dock.to_lines());
        assert!(Dock::from_lines(&["nonsense".to_owned()]).is_none());
    }

    fn classic_columns(palette: Palette) -> bool {
        !matches!(palette, Palette::CommandBar | Palette::Timeline)
            && !palette.is_standalone_extra()
    }

    #[test]
    fn the_default_layout_is_like_a_painting_apps_and_every_panel_resizes() {
        let dock = Dock::default();
        let body = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0));
        let laid = dock.layout(body, &classic_columns, 0.0);
        assert_eq!(laid.docked.len(), 7);
        assert!(
            laid.strip.is_some(),
            "the folded palettes, as Clip Studio Paint"
        );
        assert_eq!(laid.icons.len(), STRIP_PALETTES.len());
        assert_eq!(laid.center.left(), 46.0 + 236.0 + 86.0);
        assert_eq!(laid.center.right(), 1600.0 - 300.0 - STRIP);
        let sub = laid
            .docked
            .iter()
            .find(|(p, _)| *p == Palette::SubTool)
            .unwrap()
            .1;
        let set = laid
            .docked
            .iter()
            .find(|(p, _)| *p == Palette::ColorSet)
            .unwrap()
            .1;
        assert_eq!(sub.top(), COLUMN_BAR, "below the column's bar");
        assert!((set.bottom() - 900.0).abs() < 0.5);
        assert_eq!(laid.splits.len(), 3);
        assert_eq!(
            laid.edges.len(),
            4,
            "every column resizes, tools and sliders included"
        );

        // Top CommandBar and Bottom Timeline also lay out and expose resize edges.
        let with_bars = dock.layout(body, &|p| !p.is_standalone_extra(), 0.0);
        assert_eq!(with_bars.bands.len(), 2);
        assert_eq!(with_bars.row_edges.len(), 2);
        assert_eq!(with_bars.mid.top(), 38.0);
        assert!((with_bars.center.bottom() - (900.0 - 160.0)).abs() < 0.5);
    }

    #[test]
    fn a_narrow_window_collapses_the_widest_columns_to_icons() {
        let dock = Dock::default();
        let body = Rect::from_min_size(Pos2::ZERO, vec2(760.0, 700.0));
        let laid = dock.layout(body, &classic_columns, 0.0);
        assert!(laid.strip.is_some());
        assert!(laid.icons.contains(&Palette::Navigator) && laid.icons.contains(&Palette::Layer));
        assert!(
            laid.center.width() >= MIN_CANVAS - 0.5,
            "{}",
            laid.center.width()
        );
        assert!(
            laid.docked.iter().any(|(p, _)| *p == Palette::Tools),
            "the tools stay"
        );
    }

    #[test]
    fn sides_fold_into_icons_and_close_and_come_back() {
        let body = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0));
        let mut dock = Dock::default();
        let open = dock.layout(body, &classic_columns, 0.0);
        dock.folded[1] = true;
        let laid = dock.layout(body, &classic_columns, 0.0);
        assert!(laid.icons.contains(&Palette::Navigator));
        assert!(!laid.docked.iter().any(|(p, _)| *p == Palette::Layer));
        assert_eq!(laid.center.right(), 1600.0 - STRIP, "only the strip");
        dock.folded[0] = true;
        let laid = dock.layout(body, &classic_columns, 0.0);
        assert!(laid.docked.iter().any(|(p, _)| *p == Palette::Tools));
        assert!(laid.left_icons.contains(&Palette::SubTool));
        assert!(laid.left_strip.is_some());
        dock.hidden = [true; 2];
        let laid = dock.layout(body, &classic_columns, 0.0);
        assert!(laid.docked.is_empty() && laid.strip.is_none());
        assert_eq!(laid.handles.len(), 2);
        assert_eq!(laid.center.width(), 1600.0 - 2.0 * HANDLE);
        let back = Dock::from_lines(&dock.to_lines()).unwrap();
        assert_eq!((back.folded, back.hidden), ([true; 2], [true; 2]));
        dock.hidden = [false; 2];
        dock.folded = [false; 2];
        assert_eq!(dock.layout(body, &classic_columns, 0.0), open);
    }

    #[test]
    fn palettes_move_to_any_column_row_or_floating_rect_and_resize() {
        let mut dock = Dock::default();
        dock.dock(Palette::Color, Side::Right);
        assert_eq!(dock.side_of(Palette::Color), Some(Side::Right));
        assert_eq!(dock.right[0].palettes.len(), 3);
        dock.float_rect(
            Palette::Layer,
            Rect::from_min_size(pos2(120.0, 140.0), vec2(340.0, 460.0)),
        );
        assert_eq!(dock.side_of(Palette::Layer), None);
        assert_eq!(dock.floating.len(), 1);
        assert_eq!(dock.floating[0].1.size(), vec2(340.0, 460.0));

        // Tools and BrushSize columns resize freely and can move to rows or float.
        dock.left[0].width = 92.0;
        let body = Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0));
        let laid = dock.layout(body, &classic_columns, 0.0);
        let tools_rect = laid
            .docked
            .iter()
            .find(|(p, _)| *p == Palette::Tools)
            .unwrap()
            .1;
        assert_eq!(tools_rect.width(), 92.0);

        // Timeline can move from bottom to a right column or top row.
        dock.dock_new_column(Palette::Timeline, Side::Right, 1);
        assert_eq!(dock.side_of(Palette::Timeline), Some(Side::Right));
        assert!(dock.bottom.is_empty());

        dock.iconize(Palette::Navigator);
        let laid = dock.layout(body, &classic_columns, 0.0);
        assert_eq!(laid.icons.last(), Some(&Palette::Navigator));
        dock.dock(Palette::Navigator, Side::Left);
        assert!(!dock.iconized.contains(&Palette::Navigator));
        dock.dock(Palette::BrushSize, Side::Right);
        assert!(
            dock.right
                .iter()
                .any(|column| column.palettes == [(Palette::BrushSize, 1.0)])
        );
        assert!(
            dock.left.iter().all(|column| !column.palettes.is_empty()),
            "no empty columns"
        );
    }
}
