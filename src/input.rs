//! Web input translation for host-owned panel-kit workspaces.

use dioxus::events::{Key as DioxusKey, KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use panel_kit_core::reducer::{HitTarget, WheelDisposition, WorkspaceEvent};
use panel_kit_core::{FocusContext, Key, KeyChord, PanelKey, PointerEvent, PointerEventKind};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Build a core key event from already-normalized web key parts.
pub fn key_event_from_parts<K: PanelKey>(
    chord: KeyChord,
    focus: FocusContext<K>,
) -> WorkspaceEvent<K> {
    WorkspaceEvent::Key { chord, focus }
}

/// Translate a Dioxus keyboard event into a core workspace event.
///
/// Returns `None` for browser keys panel-kit does not own, leaving the host or
/// focused control free to handle them.
pub fn keyboard_event<K: PanelKey>(
    event: &KeyboardEvent,
    focus: FocusContext<K>,
) -> Option<WorkspaceEvent<K>> {
    let key = key_from_dioxus(event.key())?;
    let modifiers = event.modifiers();
    Some(key_event_from_parts(
        KeyChord {
            key,
            shift: modifiers.shift(),
            alt: modifiers.alt(),
            ctrl: modifiers.ctrl(),
            meta: modifiers.meta(),
        },
        focus,
    ))
}

/// Build a core pointer event from already-resolved hit target and coordinates.
pub fn pointer_event_from_parts<K: PanelKey>(
    target: HitTarget<K>,
    event: PointerEvent,
) -> WorkspaceEvent<K> {
    WorkspaceEvent::Pointer { target, event }
}

/// Translate a Dioxus pointer event into a core workspace event.
pub fn pointer_event<K: PanelKey>(
    target: HitTarget<K>,
    event: &DioxusPointerEvent,
    kind: PointerEventKind,
) -> WorkspaceEvent<K> {
    pointer_event_from_parts(target, core_pointer_event(event, kind))
}

/// Build a core wheel event after the caller decides native content precedence.
pub fn wheel_event_from_parts<K: PanelKey>(
    delta_y: f64,
    body_absorbs_wheel: bool,
) -> WorkspaceEvent<K> {
    let disposition = if body_absorbs_wheel {
        WheelDisposition::ContentConsumed
    } else {
        WheelDisposition::BubbleToWorkspace
    };
    WorkspaceEvent::Wheel {
        delta_y,
        disposition,
    }
}

/// Translate a Dioxus wheel delta into a core wheel event.
pub fn wheel_event<K: PanelKey>(event: &dioxus::events::WheelEvent) -> WorkspaceEvent<K> {
    let delta_y = event.data().delta().strip_units().y;
    wheel_event_from_parts(delta_y, panel_body_absorbs_wheel(delta_y))
}

/// Convert a Dioxus pointer event into renderer-neutral coordinates.
pub fn core_pointer_event(event: &DioxusPointerEvent, kind: PointerEventKind) -> PointerEvent {
    let coordinates = event.client_coordinates();
    PointerEvent {
        kind,
        x: coordinates.x,
        y: coordinates.y,
    }
}

/// Capture the browser pointer for a drag gesture, if the event exposes one.
pub fn capture_pointer(event: &DioxusPointerEvent) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = event;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let Some(web_event) = event.data.downcast::<web_sys::PointerEvent>() else {
            return;
        };
        let Some(target) = web_event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        let _ = target.set_pointer_capture(web_event.pointer_id());
    }
}

/// Release pointer capture for a completed or cancelled drag gesture.
pub fn release_pointer(event: &DioxusPointerEvent) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = event;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let Some(web_event) = event.data.downcast::<web_sys::PointerEvent>() else {
            return;
        };
        let Some(target) = web_event
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        let _ = target.release_pointer_capture(web_event.pointer_id());
    }
}

/// Clear browser text selection after a drag gesture.
pub fn clear_selection() {
    #[cfg(target_arch = "wasm32")]
    if let Some(selection) =
        web_sys::window().and_then(|window| window.get_selection().ok().flatten())
    {
        let _ = selection.remove_all_ranges();
    }
}

/// True while an `<input>` or `<textarea>` has browser focus.
pub fn is_editing() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.active_element())
            .map(|element| {
                let tag = element.tag_name();
                tag.eq_ignore_ascii_case("input") || tag.eq_ignore_ascii_case("textarea")
            })
            .unwrap_or(false)
    }
}

/// Whether the hovered panel body can still scroll in the wheel direction.
///
/// This preserves the controller's manual scroll chaining: native panel body
/// overflow consumes the wheel until it reaches a boundary, then the workspace
/// receives a `BubbleToWorkspace` wheel event.
pub fn panel_body_absorbs_wheel(delta_y: f64) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = delta_y;
        false
    }

    #[cfg(target_arch = "wasm32")]
    {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return false;
        };
        let Ok(hovered) = document.query_selector_all(".panel-body:hover") else {
            return false;
        };
        let Some(node) = hovered
            .length()
            .checked_sub(1)
            .and_then(|index| hovered.item(index))
        else {
            return false;
        };
        let Ok(element) = node.dyn_into::<web_sys::Element>() else {
            return false;
        };

        let max_scroll = element.scroll_height() - element.client_height();
        if max_scroll <= 0 {
            return false;
        }
        if delta_y > 0.0 {
            return element.scroll_top() < max_scroll;
        }
        if delta_y < 0.0 {
            return element.scroll_top() > 0;
        }
        false
    }
}

fn key_from_dioxus(key: DioxusKey) -> Option<Key> {
    match key {
        DioxusKey::ArrowLeft => Some(Key::Left),
        DioxusKey::ArrowRight => Some(Key::Right),
        DioxusKey::ArrowUp => Some(Key::Up),
        DioxusKey::ArrowDown => Some(Key::Down),
        DioxusKey::Enter => Some(Key::Enter),
        DioxusKey::Escape => Some(Key::Escape),
        DioxusKey::Tab => Some(Key::Tab),
        DioxusKey::Character(value) => one_character(&value).map(Key::Char),
        _ => None,
    }
}

fn one_character(value: &str) -> Option<char> {
    let mut characters = value.chars();
    let character = characters.next()?;
    if characters.next().is_none() {
        Some(character)
    } else {
        None
    }
}
