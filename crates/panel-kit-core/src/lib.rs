//! Renderer-agnostic core of panel-kit: the panel-workspace state machine.
//!
//! Everything here is pure data and math — no DOM, no terminal, no async.
//! A renderer shell (the Dioxus `panel-kit` crate, the ratatui
//! `panel-kit-tui` crate, or anything else that can draw rectangles and
//! deliver pointer coordinates) owns shared panel state and calls into these
//! contracts:
//!
//! - [`PanelKey`] / [`PanelCatalog`] to identify panels and map them to the
//!   stable string IDs layouts persist,
//! - [`reducer`] for host-owned [`reducer::Snapshot`] state and the pure
//!   [`reducer::reduce`] event path, which delegates to the free functions
//!   below,
//! - [`SurfaceProfile`] / [`effective_mode`] to adapt layout behavior to the
//!   rendered surface,
//! - [`command_for`] / [`apply_command`] for keyboard window management,
//! - [`begin_drag`] / [`begin_tile_resize`] on pointer-down,
//! - [`apply_drag`] on pointer-move,
//! - [`reorder_tile`], [`front_z`], [`restore`] for tiling reorder and
//!   z-order management,
//! - [`effective_rect`] to project stored geometry through the viewport
//!   clamp at render time,
//! - [`StoredLayout`], [`migrate_v1`], [`reconcile_units`], and
//!   [`merge_defaults`] for persistence (renderers supply the actual storage:
//!   localStorage, a JSON file, a KV bucket),
//! - [`views`] for named workspace views: the registry shape and the
//!   storage-key scheme every shell shares, each view holding one
//!   [`SavedLayoutV2`].
//!
//! Units are deliberately abstract: the web shell feeds CSS pixels, the TUI
//! shell feeds character cells. Unit-dependent constants live in
//! [`Clamp`], [`TileMetrics`], and [`CommandStep`].

#![warn(missing_docs)]

pub mod badge;
pub mod frame;
pub mod loading;
pub mod panel;
pub mod persist;
pub mod reducer;
pub mod spec;
pub mod theme;
pub mod tokens;
pub mod views;
pub mod widgets;

use serde::{Deserialize, Serialize};

pub use panel::{CatalogError, PanelCatalog, PanelKey, PanelMeta, SpecPanelId};
pub use spec::{
    BackendKind, BindingManifest, Charset, ChromeSpec, CommandBinding, ContentKind, InputSpec,
    LayoutSpec, PanelProviderDeclaration, PanelSpec, PersistenceSpec, ResolvedWorkspace,
    SpecDiagnostic, SpecErrors, SurfaceSpec, TileLayoutSpec, WindowSpec, WorkspaceSpec,
    WORKSPACE_SPEC_VERSION,
};

/// The app's panel identifier — typically a fieldless enum.
///
/// One variant per panel plus a [`title`](PanelKind::title). The serde
/// bounds exist because layouts (including each panel's kind) persist; the
/// `Copy + Eq + Hash` bounds let workspaces use kinds as cheap, stable panel
/// identities across reorders and reloads. Every `PanelKind` also
/// implements [`PanelKey`], the narrower bound generic panel APIs accept.
pub trait PanelKind:
    Copy + PartialEq + Eq + std::hash::Hash + Serialize + serde::de::DeserializeOwned + 'static
{
    /// Human-readable panel title, shown in the panel header (and slugified
    /// into CSS classes by the web shell), so it should be stable.
    fn title(self) -> &'static str;
}

/// Per-panel window state, cycled by the traffic lights.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
pub enum WinState {
    /// Normal: shown at its stored geometry (floating mode) or in its grid
    /// slot (tiling mode).
    Floating,
    /// Collapsed into a dock chip; restoring brings it back.
    Minimized,
    /// Fills the whole workspace area, hiding every other panel.
    Maximized,
}

/// Workspace layout mode.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
pub enum Mode {
    /// Free placement: panels are absolutely positioned, draggable,
    /// resizable, and overlap by z-order.
    Floating,
    /// Auto grid: panels flow in `Vec` order; dragging a panel header over
    /// another panel reorders them.
    Tiling,
}

/// How much room a rendered surface has for panel management.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SurfaceClass {
    /// A narrow surface where panels tile without window-management chrome.
    Compact,
    /// A medium surface that supports window management.
    Tablet,
    /// A full-size surface that supports window management.
    Regular,
}

/// Input capabilities available on a rendered surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SurfaceCapabilities {
    /// Whether the primary pointer has coarse precision, such as touch.
    pub coarse_pointer: bool,
    /// Whether the surface can express pointer hover.
    pub hover: bool,
    /// Whether the surface has keyboard input.
    pub keyboard: bool,
}

/// The space and input capabilities a renderer actually provides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceProfile {
    /// Width-based surface class.
    pub class: SurfaceClass,
    /// Input capabilities available on the surface.
    pub caps: SurfaceCapabilities,
}

impl SurfaceProfile {
    /// Classify by logical width in the renderer's own units.
    ///
    /// Widths below `compact_max` are compact, widths below `tablet_max` are
    /// tablet-sized, and all larger widths are regular.
    pub fn from_logical_width(
        width: f64,
        compact_max: f64,
        tablet_max: f64,
        caps: SurfaceCapabilities,
    ) -> Self {
        let class = if width < compact_max {
            SurfaceClass::Compact
        } else if width < tablet_max {
            SurfaceClass::Tablet
        } else {
            SurfaceClass::Regular
        };
        Self { class, caps }
    }

    /// Whether move, resize, minimize, and maximize chrome is offered.
    pub fn window_management(&self) -> bool {
        self.class != SurfaceClass::Compact
    }

    /// Whether free floating placement is available.
    pub fn allows_floating(&self) -> bool {
        self.class != SurfaceClass::Compact
    }

    /// Whether a panel in `state` must expose a restore control even though
    /// this surface withholds window management.
    ///
    /// A tier may withhold the means to *enter* a window state; it must never
    /// withhold the means to *leave* one. [`SavedLayoutV2`] carries
    /// [`WinState`] across surfaces by design, so a panel maximized on a
    /// regular surface arrives maximized on a compact one — and a compact
    /// surface that also hid restore would strand the operator on a single
    /// full-bleed panel with every sibling unreachable and no affordance to
    /// undo it. Minimized panels already have their own escape hatch in the
    /// dock, so only [`WinState::Maximized`] needs this.
    pub fn must_offer_restore(&self, state: WinState) -> bool {
        !self.window_management() && state == WinState::Maximized
    }

    /// How many tiling columns this surface offers.
    ///
    /// This is surface policy, not renderer styling, so it lives here rather
    /// than being re-tabulated in each backend's stylesheet: a compact
    /// surface stacks in a single column, a tablet halves the regular grid,
    /// and a regular surface exposes the full [`TILE_W_MAX`] span range.
    /// Renderers clamp a panel's `tile_w` to this before placing it — a span
    /// wider than the grid has no meaning, and a DOM grid answers one by
    /// silently manufacturing zero-width implicit columns that swallow the
    /// panel.
    pub fn tile_columns(&self) -> u8 {
        match self.class {
            SurfaceClass::Compact => 1,
            SurfaceClass::Tablet => 2,
            SurfaceClass::Regular => TILE_W_MAX,
        }
    }
}

/// Compact-surface upper bound for web renderers, in CSS pixels.
pub const WEB_COMPACT_MAX: f64 = 760.0;
/// Tablet-surface upper bound for web renderers, in CSS pixels.
pub const WEB_TABLET_MAX: f64 = 1180.0;
/// Compact-surface upper bound for terminal renderers, in cells.
pub const CELLS_COMPACT_MAX: f64 = 60.0;
/// Tablet-surface upper bound for terminal renderers, in cells.
pub const CELLS_TABLET_MAX: f64 = 110.0;

/// Resolve the layout mode that a surface can actually provide.
pub fn effective_mode(preferred: Mode, profile: &SurfaceProfile) -> Mode {
    if profile.allows_floating() {
        preferred
    } else {
        Mode::Tiling
    }
}

/// Which surface owns the next key press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusContext<K> {
    /// The workspace itself owns keyboard commands.
    Workspace,
    /// A particular panel owns keyboard commands.
    Panel(K),
    /// A text-editing control owns bare key presses.
    TextInput,
}

/// A renderer-neutral key press.
///
/// Serializes in the authored external form: non-character keys are
/// snake_case strings (`"left"`, `"enter"`) and a printable key is a
/// one-field object (`{"char":"m"}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Key {
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Enter or return.
    Enter,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// A printable character.
    Char(char),
}

/// A key press plus its modifier state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct KeyChord {
    /// Pressed key.
    pub key: Key,
    /// Whether Shift is held.
    pub shift: bool,
    /// Whether Alt or Option is held.
    pub alt: bool,
    /// Whether Control is held.
    pub ctrl: bool,
    /// Whether Command or Meta is held.
    pub meta: bool,
}

/// A window-management intent, independent of how it was expressed.
///
/// Geometry commands returned by [`command_for`] carry canonical web
/// magnitudes: `16.0` marks a coarse step and `1.0` marks a fine step.
/// [`apply_command`] translates those markers through its [`CommandStep`].
///
/// Serializes internally tagged by `kind` in snake_case:
/// `{"kind":"move","dx":-16.0,"dy":0.0}`, `{"kind":"minimize"}`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PanelCommand {
    /// Move the focused panel in the given directions.
    Move {
        /// Horizontal direction.
        dx: f64,
        /// Vertical direction.
        dy: f64,
    },
    /// Resize the focused panel in the given directions.
    Resize {
        /// Width direction.
        dw: f64,
        /// Height direction.
        dh: f64,
    },
    /// Minimize the focused panel.
    Minimize,
    /// Toggle maximization of the focused panel.
    Maximize,
    /// Restore the focused panel to its normal state.
    Restore,
    /// Toggle between floating and tiling layout.
    ToggleMode,
    /// Raise the focused panel above every other panel.
    Raise,
    /// Focus the next visible panel.
    FocusNext,
    /// Focus the previous visible panel.
    FocusPrev,
}

/// Step sizes for keyboard geometry changes, in renderer units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CommandStep {
    /// Normal arrow-key step.
    pub coarse: f64,
    /// Alt-arrow precision step.
    pub fine: f64,
}

impl CommandStep {
    /// CSS-pixel steps for web renderers.
    pub const WEB: CommandStep = CommandStep {
        coarse: 16.0,
        fine: 1.0,
    };

    /// Character-cell steps for terminal renderers.
    pub const CELLS: CommandStep = CommandStep {
        coarse: 2.0,
        fine: 1.0,
    };
}

/// Map a renderer-neutral key chord to a window-management intent.
///
/// Bare keys are left to text inputs. Modified chords continue through the
/// command table so applications can offer deliberate window-management
/// shortcuts while an editor has focus.
pub fn command_for<K>(chord: KeyChord, focus: &FocusContext<K>) -> Option<PanelCommand> {
    let bare = !chord.shift && !chord.alt && !chord.ctrl && !chord.meta;
    if matches!(focus, FocusContext::TextInput) && bare {
        return None;
    }

    let coarse = CommandStep::WEB.coarse;
    let fine = CommandStep::WEB.fine;
    match chord.key {
        Key::Left if chord.shift => Some(PanelCommand::Resize {
            dw: -coarse,
            dh: 0.0,
        }),
        Key::Right if chord.shift => Some(PanelCommand::Resize {
            dw: coarse,
            dh: 0.0,
        }),
        Key::Up if chord.shift => Some(PanelCommand::Resize {
            dw: 0.0,
            dh: -coarse,
        }),
        Key::Down if chord.shift => Some(PanelCommand::Resize {
            dw: 0.0,
            dh: coarse,
        }),
        Key::Left => Some(PanelCommand::Move {
            dx: if chord.alt { -fine } else { -coarse },
            dy: 0.0,
        }),
        Key::Right => Some(PanelCommand::Move {
            dx: if chord.alt { fine } else { coarse },
            dy: 0.0,
        }),
        Key::Up => Some(PanelCommand::Move {
            dx: 0.0,
            dy: if chord.alt { -fine } else { -coarse },
        }),
        Key::Down => Some(PanelCommand::Move {
            dx: 0.0,
            dy: if chord.alt { fine } else { coarse },
        }),
        Key::Char('m') => Some(PanelCommand::Minimize),
        Key::Char('f') => Some(PanelCommand::Maximize),
        Key::Char('t') => Some(PanelCommand::ToggleMode),
        Key::Escape => Some(PanelCommand::Restore),
        Key::Tab if chord.shift => Some(PanelCommand::FocusPrev),
        Key::Tab => Some(PanelCommand::FocusNext),
        Key::Enter => Some(PanelCommand::Raise),
        Key::Char(_) => None,
    }
}

/// What an in-flight floating-mode drag is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragKind {
    /// Dragging the panel header: the panel follows the pointer.
    Move,
    /// Dragging the resize handle: the panel grows/shrinks.
    Resize,
}

/// Abstract pointer button understood by renderer shells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    /// The primary/select button: left mouse button, first touch contact, or
    /// equivalent activation control.
    Primary,
}

/// Abstract pointer event kind understood by renderer shells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEventKind {
    /// A pointer button was pressed.
    Down(PointerButton),
    /// A pointer button was released.
    Up(PointerButton),
    /// A pressed pointer moved.
    Drag(PointerButton),
    /// Pointer moved without an active drag.
    Moved,
    /// The scroll wheel turned by `delta_y` renderer units (positive scrolls
    /// the workspace content up, i.e. reveals content further down). Shells
    /// route this to the workspace-level vertical scroll.
    Scroll {
        /// Vertical wheel delta in renderer units.
        delta_y: f64,
    },
}

/// Renderer-neutral pointer event.
///
/// Units are deliberately abstract, matching [`PanelWin`]: CSS pixels for
/// the web shell, terminal cells for TUI shells, or any other coordinate
/// space the renderer consistently uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    /// Event kind.
    pub kind: PointerEventKind,
    /// X coordinate in renderer units.
    pub x: f64,
    /// Y coordinate in renderer units.
    pub y: f64,
}

/// Generic rectangle in renderer units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Region {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Region {
    /// Construct a region.
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
}

/// Shared workspace chrome regions.
///
/// This is the renderer-neutral shape of the panel surface: a top-level
/// container, an inner panel workspace, and a dock region. Renderers decide
/// how to draw these regions, but they should not invent different layout
/// semantics for the same workspace chrome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceChrome {
    /// Full top-level container.
    pub root: Region,
    /// Area where panels are laid out.
    pub workspace: Region,
    /// Dock area for minimized panels.
    pub dock: Region,
}

/// Unit-specific chrome metrics for [`workspace_chrome`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ChromeMetrics {
    /// Inset between the root border and the content.
    pub inset: f64,
    /// Height of the dock region.
    pub dock_h: f64,
}

impl ChromeMetrics {
    /// CSS-pixel defaults for the Dioxus backend.
    pub const WEB: ChromeMetrics = ChromeMetrics {
        inset: 0.0,
        dock_h: 30.0,
    };

    /// Character-cell defaults for TUI backends.
    pub const CELLS: ChromeMetrics = ChromeMetrics {
        inset: 1.0,
        dock_h: 3.0,
    };
}

/// Split a top-level renderer area into shared workspace chrome regions.
pub fn workspace_chrome(width: f64, height: f64, metrics: &ChromeMetrics) -> WorkspaceChrome {
    let root = Region::new(0.0, 0.0, width.max(0.0), height.max(0.0));
    let inset = metrics.inset.max(0.0);
    let inner_w = (root.w - inset * 2.0).max(0.0);
    let inner_h = (root.h - inset * 2.0).max(0.0);
    let dock_h = metrics.dock_h.clamp(0.0, inner_h);
    let workspace = Region::new(inset, inset, inner_w, (inner_h - dock_h).max(0.0));
    let dock = Region::new(inset, inset + workspace.h, inner_w, dock_h);
    WorkspaceChrome {
        root,
        workspace,
        dock,
    }
}

/// An in-flight drag: which panel, what kind, and the pointer + panel
/// geometry captured at pointer-down (deltas are applied against these).
///
/// For a tiling resize started with [`begin_tile_resize`], `start_w`/
/// `start_h` hold the *tile spans*, not length units — [`apply_drag`]
/// branches on `tiling`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// Index of the dragged panel in the workspace `Vec`.
    pub idx: usize,
    /// Move or resize.
    pub kind: DragKind,
    /// Pointer x at pointer-down.
    pub start_mx: f64,
    /// Pointer y at pointer-down.
    pub start_my: f64,
    /// Panel x at pointer-down.
    pub start_x: f64,
    /// Panel y at pointer-down.
    pub start_y: f64,
    /// Panel width (or tiling width span) at pointer-down.
    pub start_w: f64,
    /// Panel height (or tiling height span) at pointer-down.
    pub start_h: f64,
}

/// Tiling-mode row height unit in the web shell's px — kept here so both
/// shells agree on the persisted meaning of [`PanelWin::tile_h`].
pub const TILE_ROW_PX: f64 = 150.0;
/// Width span ceiling (quarter-row units).
pub const TILE_W_MAX: u8 = 4;
/// Height span ceiling (rows).
pub const TILE_H_MAX: u8 = 6;

fn default_tile_w() -> u8 {
    1
}
fn default_tile_h() -> u8 {
    2
}

/// One panel's geometry + window state. `z` is the floating-mode stacking
/// order.
///
/// Stored geometry is the user's *intent*: when the viewport shrinks,
/// panels are clamped on screen at render time ([`effective_rect`]) but the
/// stored rect is left untouched, so they spring back when the viewport
/// grows. Build defaults with [`LayoutBuilder`].
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanelWin<K> {
    /// Which panel this is (the app's [`PanelKey`], typically a `PanelKind`
    /// enum).
    pub kind: K,
    /// Left edge, relative to the workspace area (px on the web, cells in
    /// a terminal).
    pub x: f64,
    /// Top edge, relative to the workspace area.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
    /// Window state (normal / minimized to dock / maximized).
    pub state: WinState,
    /// Floating-mode stacking order; higher is in front.
    pub z: i32,
    /// Tiling-mode width span in quarter-row units (1–4): 1 ≈ a quarter of
    /// the row, 4 = the full row. Layouts saved before this field existed
    /// deserialize to 1.
    #[serde(default = "default_tile_w")]
    pub tile_w: u8,
    /// Tiling-mode height in rows (1–6). Defaults to 2.
    #[serde(default = "default_tile_h")]
    pub tile_h: u8,
}

impl<K> PanelWin<K> {
    /// Builder-style override of the tiling spans, clamped to their valid
    /// ranges (width 1–4 quarter-row units, height 1–6 rows).
    pub fn with_tile(mut self, w: u8, h: u8) -> Self {
        self.tile_w = w.clamp(1, TILE_W_MAX);
        self.tile_h = h.clamp(1, TILE_H_MAX);
        self
    }
}

/// Convenience builder for default layouts: hands out incrementing z values
/// so later panels stack in front of earlier ones.
pub struct LayoutBuilder {
    z: i32,
}

impl LayoutBuilder {
    /// Start a builder; the first panel gets `z = 1`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { z: 0 }
    }

    /// A [`PanelWin`] at the given rect, in [`WinState::Floating`], with the
    /// next z value.
    pub fn at<K>(&mut self, kind: K, x: f64, y: f64, w: f64, h: f64) -> PanelWin<K> {
        self.z += 1;
        PanelWin {
            kind,
            x,
            y,
            w,
            h,
            state: WinState::Floating,
            z: self.z,
            tile_w: default_tile_w(),
            tile_h: default_tile_h(),
        }
    }
}

fn default_max_frac() -> f64 {
    1.0
}

/// Viewport-clamp parameters for [`effective_rect`] — the unit-dependent
/// knobs that turn a raw viewport into a workspace area and keep panels
/// visible inside it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Clamp {
    /// Subtracted from viewport width to get the workspace width.
    pub outer_w: f64,
    /// Subtracted from viewport height (top bar + dock chrome).
    pub outer_h: f64,
    /// Workspace width floor.
    pub floor_w: f64,
    /// Workspace height floor.
    pub floor_h: f64,
    /// A panel must leave this much workspace visible when sized.
    pub inner: f64,
    /// Edge padding when pulling a panel back on screen.
    pub edge: f64,
    /// Minimum panel width.
    pub min_w: f64,
    /// Minimum panel height.
    pub min_h: f64,
    /// Maximum floating size as a fraction of the workspace on each axis.
    ///
    /// This render-time cap preserves stored geometry. Maximize bypasses it
    /// because maximized panels project directly to the whole workspace.
    #[serde(default = "default_max_frac")]
    pub max_frac: f64,
}

impl Clamp {
    /// The original panel-kit web (CSS px) behavior.
    pub const WEB: Clamp = Clamp {
        outer_w: 4.0,
        outer_h: 66.0, // topbar (~36) + dock (~30)
        floor_w: 220.0,
        floor_h: 180.0,
        inner: 12.0,
        edge: 6.0,
        min_w: 180.0,
        min_h: 110.0,
        max_frac: 0.75,
    };

    /// Character-cell defaults for terminal shells.
    pub const CELLS: Clamp = Clamp {
        outer_w: 0.0,
        outer_h: 0.0,
        floor_w: 24.0,
        floor_h: 8.0,
        inner: 2.0,
        edge: 0.0,
        min_w: 20.0,
        min_h: 5.0,
        max_frac: 0.75,
    };
}

/// Host-owned pointer snapping policy.
///
/// This is session UI policy rather than persisted layout state, so hosts pass
/// it through [`reducer::ReduceContext`] instead of storing it in a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SnapPolicy {
    /// Quantize floating resize dimensions to [`SnapPolicy::grid`].
    pub resize: bool,
    /// Quantize floating move coordinates and permit tiling reorder gestures.
    pub move_: bool,
    /// Floating snap interval in renderer units.
    pub grid: f64,
}

impl Default for SnapPolicy {
    fn default() -> Self {
        Self {
            resize: true,
            move_: true,
            grid: 32.0,
        }
    }
}

/// Tiling-mode metrics: how pointer deltas snap to tile spans.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TileMetrics {
    /// Height of one tile row.
    pub row: f64,
    /// Floor for the width of one quarter-row column.
    pub col_floor: f64,
    /// Horizontal chrome subtracted from the viewport before computing the
    /// column width.
    pub outer: f64,
}

impl TileMetrics {
    /// The original panel-kit web (CSS px) behavior.
    pub const WEB: TileMetrics = TileMetrics {
        row: TILE_ROW_PX,
        col_floor: 80.0,
        outer: 16.0,
    };

    /// Character-cell defaults for terminal shells. The row height is
    /// authoritative for both rendering and resize snapping; renderers must
    /// not keep a private copy.
    pub const CELLS: TileMetrics = TileMetrics {
        row: 4.0,
        col_floor: 12.0,
        outer: 0.0,
    };
}

/// Next z value that stacks in front of every panel.
pub fn front_z<K>(ps: &[PanelWin<K>]) -> i32 {
    ps.iter().map(|p| p.z).max().unwrap_or(0) + 1
}

/// The on-screen geometry for a floating panel: shrunk if larger than the
/// viewport, pulled back so the whole panel stays visible.
///
/// This is a render-time projection — the stored [`PanelWin`] keeps the
/// user's intended geometry untouched. Mutating state instead would make
/// clamping one-way: shrink the window once and every panel stays crushed
/// after it grows back.
pub fn effective_rect<K>(p: &PanelWin<K>, vw: f64, vh: f64, c: &Clamp) -> (f64, f64, f64, f64) {
    let ws_w = (vw - c.outer_w).max(c.floor_w);
    let ws_h = (vh - c.outer_h).max(c.floor_h);
    let w = p.w.min(ws_w - c.inner).min(ws_w * c.max_frac).max(c.min_w);
    let h = p.h.min(ws_h - c.inner).min(ws_h * c.max_frac).max(c.min_h);
    let x = p.x.min(ws_w - w - c.edge).max(0.0);
    let y = p.y.min(ws_h - h - c.edge).max(0.0);
    (x, y, w, h)
}

/// Total height the floating panels occupy from the workspace top: the
/// largest `y + h` across `indices` (using each panel's *stored* geometry, so
/// the figure reflects the user's intent rather than the on-screen clamp).
/// Returns `0.0` for an empty set. Shells feed this to [`max_scroll`] /
/// [`clamp_scroll`] to bound workspace-level vertical scrolling in floating
/// mode.
pub fn floating_content_height<K>(panels: &[PanelWin<K>], indices: &[usize]) -> f64 {
    indices
        .iter()
        .filter_map(|&i| panels.get(i))
        .map(|p| p.y + p.h)
        .fold(0.0_f64, f64::max)
}

/// The furthest the workspace can scroll vertically: how far the laid-out
/// content (`content_h`) overhangs the viewport (`viewport_h`), never below
/// zero. When content fits, the result is `0.0` and the shell pins the scroll
/// offset to the top.
///
/// Units are the renderer's own (CSS px on the web, character cells in a
/// terminal). Shells compute `content_h` from their own layout pass and clamp
/// their stored scroll offset with [`clamp_scroll`].
pub fn max_scroll(content_h: f64, viewport_h: f64) -> f64 {
    (content_h - viewport_h).max(0.0)
}

/// Clamp a candidate vertical scroll offset to `[0, max_scroll(..)]`, so the
/// workspace can never scroll above its top edge or past the bottom of its
/// laid-out content.
pub fn clamp_scroll(offset: f64, content_h: f64, viewport_h: f64) -> f64 {
    offset.clamp(0.0, max_scroll(content_h, viewport_h))
}

/// Start a floating-mode move/resize drag from a pointer-down at
/// `(mx, my)`, capturing panel geometry into a [`Drag`].
///
/// Normalize-on-grab: what the user grabbed is the *clamped* on-screen rect
/// ([`effective_rect`]), which can differ from the stored geometry after a
/// viewport shrink. Writing it back on grab keeps the drag math anchored to
/// what's visible — no jump on the first pointer-move.
#[allow(clippy::too_many_arguments)]
pub fn begin_drag<K>(
    panels: &mut [PanelWin<K>],
    idx: usize,
    kind: DragKind,
    mx: f64,
    my: f64,
    vw: f64,
    vh: f64,
    c: &Clamp,
) -> Option<Drag> {
    let p = panels.get_mut(idx)?;
    let (x, y, w, h) = effective_rect(p, vw, vh, c);
    (p.x, p.y, p.w, p.h) = (x, y, w, h);
    Some(Drag {
        idx,
        kind,
        start_mx: mx,
        start_my: my,
        start_x: p.x,
        start_y: p.y,
        start_w: p.w,
        start_h: p.h,
    })
}

/// Start a tiling-mode resize drag from the corner grip. Unlike
/// [`begin_drag`]'s free resize, pointer deltas snap the panel's tile spans
/// so tiles always land on fit sizes. `start_w`/`start_h` on the captured
/// [`Drag`] hold the *spans*, not lengths; [`apply_drag`] branches on
/// `tiling`.
pub fn begin_tile_resize<K>(panels: &[PanelWin<K>], idx: usize, mx: f64, my: f64) -> Option<Drag> {
    let p = panels.get(idx)?;
    Some(Drag {
        idx,
        kind: DragKind::Resize,
        start_mx: mx,
        start_my: my,
        start_x: 0.0,
        start_y: 0.0,
        start_w: p.tile_w as f64,
        start_h: p.tile_h as f64,
    })
}

/// Apply the in-flight [`Drag`] for a pointer now at `(mx, my)`.
///
/// `tiling` exclusively selects span resize captured by
/// [`begin_tile_resize`]. Floating moves and resizes quantize to
/// [`SnapPolicy::grid`] when their corresponding policy flag is enabled.
#[allow(clippy::too_many_arguments)]
pub fn apply_drag<K>(
    panels: &mut [PanelWin<K>],
    d: &Drag,
    mx: f64,
    my: f64,
    tiling: bool,
    snap: SnapPolicy,
    vw: f64,
    c: &Clamp,
    t: &TileMetrics,
) {
    let Some(p) = panels.get_mut(d.idx) else {
        return;
    };
    let quantize = |value: f64| {
        if snap.grid.is_finite() && snap.grid > 0.0 {
            (value / snap.grid).round() * snap.grid
        } else {
            value
        }
    };
    match d.kind {
        DragKind::Move => {
            let x = (d.start_x + (mx - d.start_mx)).max(0.0);
            let y = (d.start_y + (my - d.start_my)).max(0.0);
            p.x = if snap.move_ { quantize(x) } else { x };
            p.y = if snap.move_ { quantize(y) } else { y };
        }
        DragKind::Resize if tiling => {
            let col = ((vw - t.outer) / TILE_W_MAX as f64).max(t.col_floor);
            let dw = ((mx - d.start_mx) / col).round();
            let dh = ((my - d.start_my) / t.row).round();
            p.tile_w = (d.start_w + dw).clamp(1.0, TILE_W_MAX as f64) as u8;
            p.tile_h = (d.start_h + dh).clamp(1.0, TILE_H_MAX as f64) as u8;
        }
        DragKind::Resize => {
            let w = (d.start_w + (mx - d.start_mx)).max(c.min_w);
            let h = (d.start_h + (my - d.start_my)).max(c.min_h);
            p.w = if snap.resize { quantize(w) } else { w };
            p.h = if snap.resize { quantize(h) } else { h };
        }
    }
}

/// Tiling-mode reorder: move `dragged` into `target`'s slot. Moving down
/// the flow inserts after the target, moving up inserts before — the
/// classic sortable-list shuffle, so the dragged panel snaps into whichever
/// slot the pointer is over. `Vec` order is the tiling order.
pub fn reorder_tile<K: PartialEq + Copy>(panels: &mut Vec<PanelWin<K>>, dragged: K, target: K) {
    let Some(from) = panels.iter().position(|p| p.kind == dragged) else {
        return;
    };
    let Some(to) = panels.iter().position(|p| p.kind == target) else {
        return;
    };
    if from == to {
        return;
    }
    let p = panels.remove(from);
    let after_removal = if from < to { to - 1 } else { to };
    let insert_at = (if from < to {
        after_removal + 1
    } else {
        after_removal
    })
    .min(panels.len());
    panels.insert(insert_at, p);
}

/// Restore and raise the panel of `kind`: un-minimizes it and brings it to
/// the front. No-op when the layout holds no panel of that kind.
pub fn restore<K: PartialEq + Copy>(panels: &mut [PanelWin<K>], kind: K) {
    let z = front_z(panels);
    if let Some(p) = panels.iter_mut().find(|p| p.kind == kind) {
        if p.state == WinState::Minimized {
            p.state = WinState::Floating;
        }
        p.z = z;
    }
}

/// Indices of the panels a renderer should draw, honoring minimize and
/// maximize: a maximized panel hides all others. Returns
/// `(visible, maximized_index)`.
pub fn visible_panels<K>(panels: &[PanelWin<K>]) -> (Vec<usize>, Option<usize>) {
    let maximized = panels.iter().position(|p| p.state == WinState::Maximized);
    let visible = match maximized {
        Some(mi) => vec![mi],
        None => panels
            .iter()
            .enumerate()
            .filter(|(_, p)| p.state != WinState::Minimized)
            .map(|(i, _)| i)
            .collect(),
    };
    (visible, maximized)
}

fn command_delta(delta: f64, step: CommandStep) -> f64 {
    let magnitude = delta.abs();
    if magnitude == CommandStep::WEB.coarse {
        delta.signum() * step.coarse
    } else if magnitude == CommandStep::WEB.fine {
        delta.signum() * step.fine
    } else {
        delta
    }
}

/// Apply a window-management intent to the panel set.
///
/// `focused` selects the target panel. Mode changes and focus cycling write
/// their resulting values through `mode` and `focused`. Canonical coarse and
/// fine magnitudes produced by [`command_for`] are translated through `step`.
#[allow(clippy::too_many_arguments)]
pub fn apply_command<K: PanelKey>(
    panels: &mut [PanelWin<K>],
    mode: &mut Mode,
    focused: &mut Option<K>,
    cmd: PanelCommand,
    clamp: Clamp,
    step: CommandStep,
    vw: f64,
    vh: f64,
) {
    match cmd {
        PanelCommand::Move { dx, dy } => {
            let Some(kind) = *focused else {
                return;
            };
            let Some(panel) = panels.iter_mut().find(|panel| panel.kind == kind) else {
                return;
            };
            panel.x += command_delta(dx, step);
            panel.y += command_delta(dy, step);
            let (x, y, w, h) = effective_rect(panel, vw, vh, &clamp);
            (panel.x, panel.y, panel.w, panel.h) = (x, y, w, h);
        }
        PanelCommand::Resize { dw, dh } => {
            let Some(kind) = *focused else {
                return;
            };
            let Some(panel) = panels.iter_mut().find(|panel| panel.kind == kind) else {
                return;
            };
            panel.w += command_delta(dw, step);
            panel.h += command_delta(dh, step);
            let (x, y, w, h) = effective_rect(panel, vw, vh, &clamp);
            (panel.x, panel.y, panel.w, panel.h) = (x, y, w, h);
        }
        PanelCommand::Minimize => {
            let Some(kind) = *focused else {
                return;
            };
            if let Some(panel) = panels.iter_mut().find(|panel| panel.kind == kind) {
                panel.state = WinState::Minimized;
            }
        }
        PanelCommand::Maximize => {
            let Some(kind) = *focused else {
                return;
            };
            if let Some(panel) = panels.iter_mut().find(|panel| panel.kind == kind) {
                panel.state = if panel.state == WinState::Maximized {
                    WinState::Floating
                } else {
                    WinState::Maximized
                };
            }
        }
        PanelCommand::Restore => {
            let Some(kind) = *focused else {
                return;
            };
            if let Some(panel) = panels.iter_mut().find(|panel| panel.kind == kind) {
                if matches!(panel.state, WinState::Minimized | WinState::Maximized) {
                    panel.state = WinState::Floating;
                }
            }
        }
        PanelCommand::ToggleMode => {
            *mode = match *mode {
                Mode::Floating => Mode::Tiling,
                Mode::Tiling => Mode::Floating,
            };
        }
        PanelCommand::Raise => {
            let Some(kind) = *focused else {
                return;
            };
            let z = front_z(panels);
            if let Some(panel) = panels.iter_mut().find(|panel| panel.kind == kind) {
                panel.z = z;
            }
        }
        PanelCommand::FocusNext | PanelCommand::FocusPrev => {
            let (visible, _) = visible_panels(panels);
            if visible.is_empty() {
                *focused = None;
                return;
            }
            let current = (*focused)
                .and_then(|kind| visible.iter().position(|&index| panels[index].kind == kind));
            let next = match (cmd, current) {
                (_, None) => 0,
                (PanelCommand::FocusNext, Some(position)) => (position + 1) % visible.len(),
                (PanelCommand::FocusPrev, Some(0)) => visible.len() - 1,
                (PanelCommand::FocusPrev, Some(position)) => position - 1,
                _ => unreachable!(),
            };
            *focused = Some(panels[visible[next]].kind);
        }
    }
}

/// Legacy V1 persisted layout. This remains readable so renderers can
/// migrate the old `{ panels, tiling }` shape, but new layouts use
/// [`SavedLayoutV2`].
#[derive(Serialize, Deserialize)]
pub struct SavedLayout<K> {
    /// All panels with their geometry and window state, in tiling order.
    pub panels: Vec<PanelWin<K>>,
    /// Whether tiling mode was active when saved.
    pub tiling: bool,
}

/// Current persisted-layout schema version.
pub const LAYOUT_SCHEMA_VERSION: u32 = 2;

/// Coordinate space used by a saved floating rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
pub enum Units {
    /// CSS pixels.
    CssPx,
    /// Character cells.
    Cells,
}

/// A versioned persisted layout with explicit geometry units.
#[derive(Serialize, Deserialize)]
pub struct SavedLayoutV2<K> {
    /// Schema version. New records use [`LAYOUT_SCHEMA_VERSION`].
    pub version: u32,
    /// Coordinate space used by floating panel geometry.
    pub units: Units,
    /// Viewport the geometry was captured against, in `units`.
    pub viewport: (f64, f64),
    /// Layout mode active when saved.
    pub mode: Mode,
    /// All panels with their geometry and window state, in tiling order.
    pub panels: Vec<PanelWin<K>>,
}

/// A persisted layout in either the legacy or current schema.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum StoredLayout<K> {
    /// Current versioned schema.
    V2(SavedLayoutV2<K>),
    /// Legacy `{ panels, tiling }` schema.
    V1(SavedLayout<K>),
}

/// Upgrade a V1 record using the loading renderer's units and viewport.
pub fn migrate_v1<K>(old: SavedLayout<K>, units: Units, viewport: (f64, f64)) -> SavedLayoutV2<K> {
    SavedLayoutV2 {
        version: LAYOUT_SCHEMA_VERSION,
        units,
        viewport,
        mode: if old.tiling {
            Mode::Tiling
        } else {
            Mode::Floating
        },
        panels: old.panels,
    }
}

/// Rescale floating geometry into a renderer's local units and viewport.
///
/// Tile spans are unitless and remain unchanged.
pub fn reconcile_units<K>(
    mut layout: SavedLayoutV2<K>,
    to: Units,
    viewport: (f64, f64),
) -> SavedLayoutV2<K> {
    if layout.units == to && layout.viewport == viewport {
        return layout;
    }

    let scale_x = viewport.0 / layout.viewport.0;
    let scale_y = viewport.1 / layout.viewport.1;
    for panel in &mut layout.panels {
        panel.x *= scale_x;
        panel.w *= scale_x;
        panel.y *= scale_y;
        panel.h *= scale_y;
    }
    layout.units = to;
    layout.viewport = viewport;
    layout
}

/// Reconcile a loaded layout against the current panel set: panels added to
/// the app since the layout was saved are appended with their default
/// placement, so new features still show up for existing users.
pub fn merge_defaults<K: PartialEq + Copy>(
    panels: &mut Vec<PanelWin<K>>,
    defaults: &[PanelWin<K>],
) {
    for d in defaults {
        if !panels.iter().any(|p| p.kind == d.kind) {
            panels.push(*d);
        }
    }
}

/// CSS-safe slug of a panel title ("Filter Strip" -> "filter-strip").
pub fn kind_slug(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    enum TestPanel {
        First,
        Second,
        Third,
    }

    impl PanelKind for TestPanel {
        fn title(self) -> &'static str {
            match self {
                Self::First => "First",
                Self::Second => "Second",
                Self::Third => "Third",
            }
        }
    }

    fn capabilities() -> SurfaceCapabilities {
        SurfaceCapabilities {
            coarse_pointer: false,
            hover: true,
            keyboard: true,
        }
    }

    fn panel(kind: TestPanel) -> PanelWin<TestPanel> {
        PanelWin {
            kind,
            x: 10.0,
            y: 20.0,
            w: 30.0,
            h: 40.0,
            state: WinState::Floating,
            z: 1,
            tile_w: 2,
            tile_h: 3,
        }
    }

    #[test]
    fn effective_rect_caps_at_max_frac() {
        let mut layout = LayoutBuilder::new();
        let oversized = layout.at(TestPanel::First, 0.0, 0.0, 5000.0, 5000.0);
        let (_, _, w, h) = effective_rect(&oversized, 1000.0, 1000.0, &Clamp::WEB);

        assert!(w <= 996.0 * 0.75 + f64::EPSILON, "w={w}");
        assert!(h <= 934.0 * 0.75 + f64::EPSILON, "h={h}");

        let regular = layout.at(TestPanel::Second, 10.0, 10.0, 300.0, 200.0);
        let (_, _, w, h) = effective_rect(&regular, 1000.0, 1000.0, &Clamp::WEB);
        assert_eq!((w, h), (300.0, 200.0));
    }

    #[test]
    fn clamp_without_max_frac_deserializes_as_uncapped() {
        let json = r#"{
            "outer_w": 4.0,
            "outer_h": 66.0,
            "floor_w": 220.0,
            "floor_h": 180.0,
            "inner": 12.0,
            "edge": 6.0,
            "min_w": 180.0,
            "min_h": 110.0
        }"#;

        let clamp: Clamp = serde_json::from_str(json).unwrap();
        assert_eq!(clamp.max_frac, 1.0);
    }

    #[test]
    fn floating_move_quantizes_only_when_enabled() {
        let initial = panel(TestPanel::First);
        let drag = Drag {
            idx: 0,
            kind: DragKind::Move,
            start_mx: 0.0,
            start_my: 0.0,
            start_x: initial.x,
            start_y: initial.y,
            start_w: initial.w,
            start_h: initial.h,
        };
        let mut snapped = [initial];
        let mut free = [initial];

        apply_drag(
            &mut snapped,
            &drag,
            45.0,
            50.0,
            false,
            SnapPolicy::default(),
            1000.0,
            &Clamp::WEB,
            &TileMetrics::WEB,
        );
        apply_drag(
            &mut free,
            &drag,
            45.0,
            50.0,
            false,
            SnapPolicy {
                move_: false,
                ..SnapPolicy::default()
            },
            1000.0,
            &Clamp::WEB,
            &TileMetrics::WEB,
        );

        assert_eq!((snapped[0].x, snapped[0].y), (64.0, 64.0));
        assert_eq!((free[0].x, free[0].y), (55.0, 70.0));
    }

    #[test]
    fn floating_resize_quantizes_only_when_enabled() {
        let mut initial = panel(TestPanel::First);
        initial.w = 300.0;
        initial.h = 200.0;
        let drag = Drag {
            idx: 0,
            kind: DragKind::Resize,
            start_mx: 0.0,
            start_my: 0.0,
            start_x: initial.x,
            start_y: initial.y,
            start_w: initial.w,
            start_h: initial.h,
        };
        let mut snapped = [initial];
        let mut free = [initial];

        apply_drag(
            &mut snapped,
            &drag,
            45.0,
            50.0,
            false,
            SnapPolicy::default(),
            1000.0,
            &Clamp::WEB,
            &TileMetrics::WEB,
        );
        apply_drag(
            &mut free,
            &drag,
            45.0,
            50.0,
            false,
            SnapPolicy {
                resize: false,
                ..SnapPolicy::default()
            },
            1000.0,
            &Clamp::WEB,
            &TileMetrics::WEB,
        );

        assert_eq!((snapped[0].w, snapped[0].h), (352.0, 256.0));
        assert_eq!((free[0].w, free[0].h), (345.0, 250.0));
    }

    #[test]
    fn tiling_resize_uses_tile_spans_when_floating_snap_is_disabled() {
        let initial = panel(TestPanel::First);
        let drag = begin_tile_resize(&[initial], 0, 0.0, 0.0).unwrap();
        let mut panels = [initial];

        apply_drag(
            &mut panels,
            &drag,
            250.0,
            150.0,
            true,
            SnapPolicy {
                resize: false,
                ..SnapPolicy::default()
            },
            1000.0,
            &Clamp::WEB,
            &TileMetrics::WEB,
        );

        assert_eq!((panels[0].tile_w, panels[0].tile_h), (3, 4));
        assert_eq!((panels[0].w, panels[0].h), (30.0, 40.0));
    }

    #[test]
    fn compact_surface_forces_tiling() {
        let profile = SurfaceProfile::from_logical_width(
            759.0,
            WEB_COMPACT_MAX,
            WEB_TABLET_MAX,
            capabilities(),
        );

        assert!(matches!(
            effective_mode(Mode::Floating, &profile),
            Mode::Tiling
        ));
    }

    #[test]
    fn compact_surface_still_offers_restore_for_a_maximized_panel() {
        let compact = SurfaceProfile::from_logical_width(
            390.0,
            WEB_COMPACT_MAX,
            WEB_TABLET_MAX,
            capabilities(),
        );
        let regular = SurfaceProfile::from_logical_width(
            1440.0,
            WEB_COMPACT_MAX,
            WEB_TABLET_MAX,
            capabilities(),
        );

        // Compact withholds window management, so a maximized panel that
        // arrived through persistence would otherwise be inescapable.
        assert!(!compact.window_management());
        assert!(compact.must_offer_restore(WinState::Maximized));

        // Floating panels need nothing extra, and minimized panels escape
        // through the dock.
        assert!(!compact.must_offer_restore(WinState::Floating));
        assert!(!compact.must_offer_restore(WinState::Minimized));

        // A surface that already offers window management needs no override.
        assert!(regular.window_management());
        assert!(!regular.must_offer_restore(WinState::Maximized));
    }

    #[test]
    fn bare_arrow_is_preserved_for_text_input() {
        let chord = KeyChord {
            key: Key::Left,
            shift: false,
            alt: false,
            ctrl: false,
            meta: false,
        };

        assert!(command_for::<TestPanel>(chord, &FocusContext::TextInput).is_none());
        assert!(matches!(
            command_for::<TestPanel>(chord, &FocusContext::Workspace),
            Some(PanelCommand::Move { dx, dy })
                if dx == -CommandStep::WEB.coarse && dy == 0.0
        ));
    }

    #[test]
    fn focus_next_skips_minimized_panels_and_wraps() {
        let mut panels = vec![
            panel(TestPanel::First),
            PanelWin {
                state: WinState::Minimized,
                ..panel(TestPanel::Second)
            },
            panel(TestPanel::Third),
        ];
        let mut mode = Mode::Floating;
        let mut focused = Some(TestPanel::First);

        apply_command(
            &mut panels,
            &mut mode,
            &mut focused,
            PanelCommand::FocusNext,
            Clamp::WEB,
            CommandStep::WEB,
            1000.0,
            800.0,
        );
        assert_eq!(focused, Some(TestPanel::Third));

        apply_command(
            &mut panels,
            &mut mode,
            &mut focused,
            PanelCommand::FocusNext,
            Clamp::WEB,
            CommandStep::WEB,
            1000.0,
            800.0,
        );
        assert_eq!(focused, Some(TestPanel::First));
    }

    #[test]
    fn reconcile_units_round_trips_between_viewports() {
        let original = SavedLayoutV2 {
            version: LAYOUT_SCHEMA_VERSION,
            units: Units::CssPx,
            viewport: (100.0, 200.0),
            mode: Mode::Floating,
            panels: vec![panel(TestPanel::First)],
        };

        let cells = reconcile_units(original, Units::Cells, (50.0, 100.0));
        let restored = reconcile_units(cells, Units::CssPx, (100.0, 200.0));
        let restored_panel = &restored.panels[0];

        assert_eq!(
            (
                restored_panel.x,
                restored_panel.y,
                restored_panel.w,
                restored_panel.h
            ),
            (10.0, 20.0, 30.0, 40.0)
        );
        assert_eq!((restored_panel.tile_w, restored_panel.tile_h), (2, 3));
    }

    #[test]
    fn v1_json_deserializes_and_migrates_to_v2() {
        let json = r#"{
            "panels": [{
                "kind": "First",
                "x": 10.0,
                "y": 20.0,
                "w": 30.0,
                "h": 40.0,
                "state": "Floating",
                "z": 1,
                "tile_w": 2,
                "tile_h": 3
            }],
            "tiling": true
        }"#;
        let stored: StoredLayout<TestPanel> = serde_json::from_str(json).unwrap();
        let StoredLayout::V1(old) = stored else {
            panic!("legacy JSON parsed as the wrong schema");
        };

        let migrated = migrate_v1(old, Units::Cells, (120.0, 40.0));

        assert_eq!(migrated.version, LAYOUT_SCHEMA_VERSION);
        assert_eq!(migrated.units, Units::Cells);
        assert_eq!(migrated.viewport, (120.0, 40.0));
        assert!(matches!(migrated.mode, Mode::Tiling));
        assert_eq!(migrated.panels[0].kind, TestPanel::First);
    }
}
