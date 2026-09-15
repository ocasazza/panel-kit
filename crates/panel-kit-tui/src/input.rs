//! Terminal and browser-cell input translation for host-owned workspaces.
//!
//! Backend hosts use these adapters after their own widgets/editors have had
//! first refusal. The functions translate platform events into core
//! `WorkspaceEvent` values; all state transitions remain in `panel-kit-core`.

use panel_kit_core::reducer::{HitTarget, PanelPart, WheelDisposition, WorkspaceEvent};
use panel_kit_core::{
    FocusContext, Key, KeyChord, PanelCommand, PanelKey, PointerButton, PointerEvent,
    PointerEventKind,
};
use crate::widgets::TuiHitBuffer;

/// Wrap a renderer-neutral key chord as a workspace event.
pub fn workspace_event_from_key<K: PanelKey>(
    chord: KeyChord,
    focus: FocusContext<K>,
) -> WorkspaceEvent<K> {
    WorkspaceEvent::Key { chord, focus }
}

/// Convert a pointer event plus recorded TUI hits into the workspace event core expects.
///
/// Clicked traffic lights become explicit command events so hosts preserve the
/// controller-era behavior where controls execute immediately. Body, header,
/// dock, drag, release, move, and wheel events remain renderer-neutral pointer
/// or wheel events for the core reducer to handle.
pub fn workspace_event_from_pointer<K: PanelKey>(
    hits: &TuiHitBuffer<K>,
    event: PointerEvent,
) -> Option<WorkspaceEvent<K>> {
    if let PointerEventKind::Scroll { delta_y } = event.kind {
        return Some(WorkspaceEvent::Wheel {
            delta_y,
            disposition: WheelDisposition::BubbleToWorkspace,
        });
    }

    let target = hits.hit_test((event.x, event.y)).unwrap_or(HitTarget::Workspace);
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
        shift: event.modifiers.contains(KeyModifiers::SHIFT) || matches!(event.code, KeyCode::BackTab),
        alt: event.modifiers.contains(KeyModifiers::ALT),
        ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
        meta: event.modifiers.contains(KeyModifiers::SUPER),
    })
}

/// Convert a crossterm mouse event into a renderer-neutral pointer event.
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
    pub fn pointer_event(
        &mut self,
        event: ratzilla::event::MouseEvent,
    ) -> Option<PointerEvent> {
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
            MouseEventKind::Moved if self.primary_down => PointerEventKind::Drag(PointerButton::Primary),
            MouseEventKind::Moved => PointerEventKind::Moved,
            MouseEventKind::SingleClick(MouseButton::Left) => PointerEventKind::Up(PointerButton::Primary),
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
