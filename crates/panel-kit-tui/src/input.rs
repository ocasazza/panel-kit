//! Terminal and browser-cell input translation for host-owned workspaces.
//!
//! Backend hosts use these adapters after their own widgets/editors have had
//! first refusal. The functions translate platform events into core
//! `WorkspaceEvent` values; all state transitions remain in `panel-kit-core`.

use crate::scroll::PanelScroll;
use crate::widgets::TuiHitBuffer;
use panel_kit_core::reducer::{HitTarget, PanelPart, WheelDisposition, WorkspaceEvent};
use panel_kit_core::{
    FocusContext, Key, KeyChord, PanelCommand, PanelKey, PointerButton, PointerEvent,
    PointerEventKind,
};

/// A wheel gesture in terminal cells, positive toward the right and down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelDelta {
    /// Cells scrolled toward the right.
    pub x: f64,
    /// Cells scrolled toward the bottom.
    pub y: f64,
}

impl WheelDelta {
    /// Create a wheel gesture from raw cell deltas.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Reinterpret a vertical gesture as horizontal, as shift+wheel does.
    ///
    /// An already horizontal gesture is left alone.
    pub const fn horizontal(self) -> Self {
        if self.y == 0.0 {
            self
        } else {
            Self {
                x: self.y,
                y: 0.0,
            }
        }
    }

    /// Build a wheel gesture from raw renderer deltas, letting a horizontal
    /// modifier (shift+wheel) move the gesture onto the x axis.
    pub fn from_deltas(delta_x: f64, delta_y: f64, horizontal: bool) -> Self {
        let delta = Self::new(delta_x, delta_y);
        if horizontal {
            delta.horizontal()
        } else {
            delta
        }
    }
}

/// Route a wheel gesture to the panel body under `at`, falling back to the
/// workspace when the hit is not a body or the body is at its content edge.
pub fn route_wheel<K: PanelKey>(
    hits: &TuiHitBuffer<K>,
    panels: &mut PanelScroll<K>,
    at: (f64, f64),
    delta: WheelDelta,
) -> WorkspaceEvent<K> {
    let disposition = match hits.hit_test(at) {
        Some(HitTarget::Panel {
            key,
            part: PanelPart::Surface,
        }) => panels.absorb(&key, delta.x, delta.y),
        _ => WheelDisposition::BubbleToWorkspace,
    };

    wheel_event(delta.y, disposition)
}

/// Route a wheel gesture straight to one panel's body, for hosts that scroll
/// the focused panel instead of a hit-tested one.
pub fn route_wheel_to<K: PanelKey>(
    panels: &mut PanelScroll<K>,
    key: &K,
    delta: WheelDelta,
) -> WorkspaceEvent<K> {
    wheel_event(delta.y, panels.absorb(key, delta.x, delta.y))
}

fn wheel_event<K: PanelKey>(delta_y: f64, disposition: WheelDisposition) -> WorkspaceEvent<K> {
    WorkspaceEvent::Wheel {
        delta_y,
        disposition,
    }
}

/// Wrap a renderer-neutral key chord as a workspace event.
pub fn workspace_event_from_key<K: PanelKey>(
    chord: KeyChord,
    focus: FocusContext<K>,
) -> WorkspaceEvent<K> {
    WorkspaceEvent::Key { chord, focus }
}

/// Convert a non-wheel pointer event plus recorded TUI hits into the workspace
/// event core expects.
///
/// Clicked traffic lights become explicit command events so hosts preserve the
/// controller-era behavior where controls execute immediately. Body, header,
/// dock, drag, release, and move events remain renderer-neutral pointer events
/// for the core reducer to handle.
///
/// Wheel events return `None`: resolving content precedence needs the host's
/// mutable [`PanelScroll`], so hosts route them with [`route_wheel`] or
/// [`route_wheel_to`] instead.
pub fn workspace_event_from_pointer<K: PanelKey>(
    hits: &TuiHitBuffer<K>,
    event: PointerEvent,
) -> Option<WorkspaceEvent<K>> {
    if matches!(event.kind, PointerEventKind::Scroll { .. }) {
        return None;
    }

    let target = hits
        .hit_test((event.x, event.y))
        .unwrap_or(HitTarget::Workspace);
    control_command(target, event).or(Some(WorkspaceEvent::Pointer { target, event }))
}

/// Convert a crossterm key event into a renderer-neutral key chord.
#[cfg(not(target_arch = "wasm32"))]
pub fn crossterm_key_chord(event: crossterm::event::KeyEvent) -> Option<KeyChord> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let key = match event.code {
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Escape,
        KeyCode::Tab | KeyCode::BackTab => Key::Tab,
        KeyCode::Char(ch) => Key::Char(ch),
        _ => return None,
    };

    Some(KeyChord {
        key,
        shift: event.modifiers.contains(KeyModifiers::SHIFT)
            || matches!(event.code, KeyCode::BackTab),
        alt: event.modifiers.contains(KeyModifiers::ALT),
        ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
        meta: event.modifiers.contains(KeyModifiers::SUPER),
    })
}

/// Convert a crossterm mouse event into a workspace event, routing wheel
/// gestures through the panel scroll registry.
///
/// This is the native single entry point: clicks, drags, and moves become
/// pointer or command events, while every scroll notch becomes a wheel event
/// with the panel body under the pointer given first refusal.
#[cfg(not(target_arch = "wasm32"))]
pub fn crossterm_workspace_event<K: PanelKey>(
    hits: &TuiHitBuffer<K>,
    panels: &mut PanelScroll<K>,
    event: crossterm::event::MouseEvent,
) -> Option<WorkspaceEvent<K>> {
    let wheel = crossterm_wheel_delta(&event);
    let pointer = crossterm_pointer_event(event)?;
    match wheel {
        Some(delta) => Some(route_wheel(hits, panels, (pointer.x, pointer.y), delta)),
        None => workspace_event_from_pointer(hits, pointer),
    }
}

/// Wheel gesture in cells for a crossterm scroll event, `None` otherwise.
///
/// Notches are one cell each and shift turns a vertical notch horizontal.
#[cfg(not(target_arch = "wasm32"))]
pub fn crossterm_wheel_delta(event: &crossterm::event::MouseEvent) -> Option<WheelDelta> {
    use crossterm::event::{KeyModifiers, MouseEventKind};

    let delta = match event.kind {
        MouseEventKind::ScrollUp => WheelDelta::new(0.0, -1.0),
        MouseEventKind::ScrollDown => WheelDelta::new(0.0, 1.0),
        MouseEventKind::ScrollLeft => WheelDelta::new(-1.0, 0.0),
        MouseEventKind::ScrollRight => WheelDelta::new(1.0, 0.0),
        _ => return None,
    };

    Some(if event.modifiers.contains(KeyModifiers::SHIFT) {
        delta.horizontal()
    } else {
        delta
    })
}

/// Convert a crossterm mouse event into a renderer-neutral pointer event.
///
/// Scroll events keep their pointer position so hosts can hit-test them;
/// horizontal notches arrive as [`PointerEventKind::Scroll`], whose vertical
/// delta is zero.
#[cfg(not(target_arch = "wasm32"))]
pub fn crossterm_pointer_event(event: crossterm::event::MouseEvent) -> Option<PointerEvent> {
    use crossterm::event::{MouseButton, MouseEventKind};

    let kind = match event.kind {
        MouseEventKind::Down(MouseButton::Left) => PointerEventKind::Down(PointerButton::Primary),
        MouseEventKind::Up(MouseButton::Left) => PointerEventKind::Up(PointerButton::Primary),
        MouseEventKind::Drag(MouseButton::Left) => PointerEventKind::Drag(PointerButton::Primary),
        MouseEventKind::Moved => PointerEventKind::Moved,
        MouseEventKind::ScrollDown => PointerEventKind::Scroll { delta_y: 1.0 },
        MouseEventKind::ScrollUp => PointerEventKind::Scroll { delta_y: -1.0 },
        MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => {
            PointerEventKind::Scroll { delta_y: 0.0 }
        }
        _ => return None,
    };

    Some(PointerEvent {
        kind,
        x: event.column as f64,
        y: event.row as f64,
    })
}

/// Convert a ratzilla key event into a renderer-neutral key chord.
#[cfg(target_arch = "wasm32")]
pub fn ratzilla_key_chord(event: ratzilla::event::KeyEvent) -> Option<KeyChord> {
    use ratzilla::event::KeyCode;

    let key = match event.code {
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Char(ch) => Key::Char(ch),
        _ => return None,
    };

    Some(KeyChord {
        key,
        shift: event.shift,
        alt: event.alt,
        ctrl: event.ctrl,
        meta: false,
    })
}

/// Stateful ratzilla mouse translator for browsers that report drag as moved cells.
#[cfg(target_arch = "wasm32")]
#[derive(Default, Debug, Clone, Copy)]
pub struct RatzillaPointerTranslator {
    primary_down: bool,
}

#[cfg(target_arch = "wasm32")]
impl RatzillaPointerTranslator {
    /// Create a translator with no active pointer button.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert one ratzilla mouse event into a renderer-neutral pointer event.
    pub fn pointer_event(&mut self, event: ratzilla::event::MouseEvent) -> Option<PointerEvent> {
        use ratzilla::event::{MouseButton, MouseEventKind};

        let kind = match event.kind {
            MouseEventKind::ButtonDown(MouseButton::Left) => {
                self.primary_down = true;
                PointerEventKind::Down(PointerButton::Primary)
            }
            MouseEventKind::ButtonUp(MouseButton::Left) => {
                self.primary_down = false;
                PointerEventKind::Up(PointerButton::Primary)
            }
            MouseEventKind::Moved if self.primary_down => {
                PointerEventKind::Drag(PointerButton::Primary)
            }
            MouseEventKind::Moved => PointerEventKind::Moved,
            MouseEventKind::SingleClick(MouseButton::Left) => {
                PointerEventKind::Up(PointerButton::Primary)
            }
            _ => return None,
        };

        Some(PointerEvent {
            kind,
            x: event.col as f64,
            y: event.row as f64,
        })
    }
}

fn control_command<K: PanelKey>(
    target: HitTarget<K>,
    event: PointerEvent,
) -> Option<WorkspaceEvent<K>> {
    if !matches!(event.kind, PointerEventKind::Down(PointerButton::Primary)) {
        return None;
    }

    let HitTarget::Panel { key, part } = target else {
        return None;
    };

    let command = match part {
        PanelPart::ModeControl => PanelCommand::ToggleMode,
        PanelPart::MinimizeControl => PanelCommand::Minimize,
        PanelPart::MaximizeControl => PanelCommand::Maximize,
        _ => return None,
    };

    Some(WorkspaceEvent::Command {
        target: Some(key),
        command,
    })
}
