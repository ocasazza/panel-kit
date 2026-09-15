use crate::{
    Clamp, CommandStep, Drag, FocusContext, KeyChord, Mode, PanelCommand, PanelKey, PanelWin,
    PointerEvent, SurfaceProfile, TileMetrics, Units,
};
use serde::{Deserialize, Serialize};

/// The rendered viewport a [`Snapshot`]'s geometry is expressed against.
///
/// `units` pairs with the [`Clamp`]/[`CommandStep`] the host passes in
/// [`ReduceContext`]: CSS pixels on the web, character cells in a terminal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Visible width in [`Units`].
    pub width: f64,
    /// Visible height in [`Units`].
    pub height: f64,
    /// Coordinate space of `width`/`height` and of the panel geometry.
    pub units: Units,
}

/// Host-owned workspace state.
///
/// The fields are the renderer-neutral state needed by the reducer and frame
/// projection. Backend-local concerns stay out: the surface profile is an input
/// ([`ReduceContext::surface`]), persistence transports and render caches never
/// enter core state, and focus side effects are reported through
/// [`Reduction::focus_request`] rather than stored.
#[derive(Clone, PartialEq)]
pub struct Snapshot<K: PanelKey> {
    /// All panels with their geometry and window state; `Vec` order is the
    /// tiling order and persists with the layout.
    pub panels: Vec<PanelWin<K>>,
    /// The user-chosen layout [`Mode`]; renderers project it through
    /// [`effective_mode`](crate::effective_mode) against the surface in
    /// [`ReduceContext`].
    pub preferred_mode: Mode,
    /// Viewport the stored geometry is expressed against.
    pub viewport: Viewport,
    /// Panel that owns window-management commands, if any.
    pub focused: Option<K>,
    /// In-flight floating-mode drag, if any.
    pub drag: Option<Drag>,
    /// Tiling-mode reorder drag: the panel being dragged, if any.
    pub tile_drag: Option<K>,
    /// Workspace-level vertical scroll offset in renderer units.
    pub workspace_scroll: f64,
}

impl<K: PanelKey> Snapshot<K> {
    /// Seed a snapshot the way both controllers seed fresh state: the given
    /// panels, preferred mode, and viewport, with no focus, no drags, and the
    /// scroll offset pinned to the top.
    pub fn from_defaults(
        panels: Vec<PanelWin<K>>,
        preferred_mode: Mode,
        viewport: Viewport,
    ) -> Self {
        Self {
            panels,
            preferred_mode,
            viewport,
            focused: None,
            drag: None,
            tile_drag: None,
            workspace_scroll: 0.0,
        }
    }
}

/// Renderer-neutral regions that can receive pointer input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelPart {
    /// The panel body/surface.
    Surface,
    /// The draggable title/header chrome.
    Header,
    /// The control that toggles the workspace's layout mode.
    ModeControl,
    /// The control that minimizes a panel to the dock.
    MinimizeControl,
    /// The control that maximizes or restores a panel.
    MaximizeControl,
    /// The corner grip that starts a resize gesture.
    ResizeGrip,
}

/// Renderer-neutral pointer hit target reported by backend hit testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitTarget<K: PanelKey> {
    /// Pointer input landed in the workspace background.
    Workspace,
    /// Pointer input landed on a known panel part.
    Panel {
        /// Panel identity resolved by the backend hit-test/projection path.
        key: K,
        /// Specific panel part the pointer input targets.
        part: PanelPart,
    },
    /// Pointer input landed on a dock chip for a minimized panel.
    Dock {
        /// Panel identity represented by the dock chip.
        key: K,
    },
}

/// Policy that applies a viewport measurement change to stored panel intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ResizePolicy {
    /// Only the viewport measurement changes; stored panel geometry is the
    /// user's persistent intent and remains untouched.
    PreserveIntent,
    /// Stored floating geometry scales by the new/old viewport ratio, matching
    /// the web controller's reversible resize behavior.
    ScaleFloating,
}

/// Whether native content already consumed a wheel event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelDisposition {
    /// A panel body/editor consumed the wheel, so core must not scroll the
    /// workspace too.
    ContentConsumed,
    /// The wheel bubbled to workspace chrome and may affect workspace scroll.
    BubbleToWorkspace,
}

/// One renderer-neutral workspace input event.
///
/// Backends translate native input (DOM, crossterm) into these at their
/// boundary, after the host's first refusal — an event the host consumed for
/// its own editors never reaches [`crate::reducer::reduce`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorkspaceEvent<K: PanelKey> {
    /// Pointer input over a backend-projected hit target.
    Pointer {
        /// The hit target resolved by the backend.
        target: HitTarget<K>,
        /// Renderer-neutral pointer coordinates and kind.
        event: PointerEvent,
    },
    /// A key press plus its modifier state and who owned keyboard focus when it
    /// arrived.
    Key {
        /// The pressed chord.
        chord: KeyChord,
        /// Who owned the key: the workspace, a panel, or a text input.
        focus: FocusContext<K>,
    },
    /// A window-management command, already resolved from a chord, a traffic
    /// light, or a command palette.
    ///
    /// `target` is the panel the command acts on: `Some(key)` for a
    /// panel-scoped origin (the clicked light targets — and focuses — that
    /// panel, like the web controller's `execute_command`), `None` for
    /// workspace-scoped origins, which act on the focused panel.
    Command {
        /// Panel the command targets, if scoped to one.
        target: Option<K>,
        /// The window-management intent.
        command: PanelCommand,
    },
    /// Workspace-level wheel input after backend content-precedence handling.
    Wheel {
        /// Positive values scroll down/reveal lower content.
        delta_y: f64,
        /// Whether the wheel was already consumed by native content.
        disposition: WheelDisposition,
    },
    /// The rendered viewport changed size under an explicit host resize policy.
    ViewportChanged {
        /// New validated size reported by the renderer adapter.
        size: Viewport,
        /// Policy for applying the measurement change to stored geometry.
        policy: ResizePolicy,
    },
}

/// The unit system and surface facts one [`crate::reducer::reduce`] call
/// reduces against.
#[derive(Clone, Copy)]
pub struct ReduceContext<'a> {
    /// The surface the input was measured on. Pointer and wheel paths use it to
    /// choose the same effective mode the backend controllers used.
    pub surface: SurfaceProfile,
    /// Viewport clamp parameters in the host's units.
    pub clamp: &'a Clamp,
    /// Keyboard geometry step sizes in the host's units.
    pub command_step: CommandStep,
    /// Tiling span metrics in the host's units.
    pub tile: &'a TileMetrics,
}

/// When a reduction's change should be observed by a persistence policy.
///
/// Pointer motion is [`ChangePhase::Continuous`] (mid-gesture, hold writes);
/// commands and gesture ends are [`ChangePhase::Settled`] (one
/// policy-qualified write). The design's save-policy write-count table is
/// exact over these phases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangePhase {
    /// Mid-gesture: more changes are expected imminently.
    Continuous,
    /// The change is complete.
    Settled,
}

/// What one [`crate::reducer::reduce`] call did — reported, never performed.
#[derive(Clone, Debug, PartialEq)]
pub struct Reduction<K: PanelKey> {
    /// Whether the event changed any snapshot field. A mapped command whose
    /// application left every field identical reports `false`.
    pub changed: bool,
    /// When the change should be observed: `Some(phase)` if and only if
    /// `changed` is `true`.
    pub phase: Option<ChangePhase>,
    /// Panel whose chrome or dock representation should receive input focus
    /// next, if the reduction requests it.
    pub focus_request: Option<K>,
}

impl<K: PanelKey> Reduction<K> {
    /// The reduction for an event that changed nothing.
    pub(super) fn unchanged() -> Self {
        Self {
            changed: false,
            phase: None,
            focus_request: None,
        }
    }
}
