//! Web input translation for host-owned panel-kit workspaces.

use dioxus::events::{Key as DioxusKey, KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use panel_kit_core::frame::ProjectedFrame;
use panel_kit_core::reducer::{HitTarget, WheelDisposition, WorkspaceEvent};
use panel_kit_core::{
    FocusContext, Key, KeyChord, PanelKey, PointerButton, PointerEvent, PointerEventKind,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Build a core key event from already-normalized web key parts.
pub fn key_event_from_parts<K: PanelKey>(
    chord: KeyChord,
    focus: FocusContext<K>,
) -> WorkspaceEvent<K> {
    WorkspaceEvent::Key { chord, focus }
}

/// Translate a Dioxus keyboard event into a core key chord.
///
/// Returns `None` for browser keys the core has no name for (lone modifiers,
/// function keys, dead keys), so hosts and the workspace share one translation.
pub fn key_chord(event: &KeyboardEvent) -> Option<KeyChord> {
    let key = key_from_dioxus(event.key())?;
    let modifiers = event.modifiers();
    Some(KeyChord {
        key,
        shift: modifiers.shift(),
        alt: modifiers.alt(),
        ctrl: modifiers.ctrl(),
        meta: modifiers.meta(),
    })
}

/// Translate a Dioxus keyboard event into a core workspace event.
///
/// Returns `None` for browser keys panel-kit does not own, leaving the host or
/// focused control free to handle them.
pub fn keyboard_event<K: PanelKey>(
    event: &KeyboardEvent,
    focus: FocusContext<K>,
) -> Option<WorkspaceEvent<K>> {
    Some(key_event_from_parts(key_chord(event)?, focus))
}

/// Give keyboard focus to the `.panel-head` containing the event target, so
/// keys pressed mid-gesture (Escape cancels a tile drag) reach that header.
/// The header press itself calls `preventDefault`, which would otherwise
/// leave focus where it was.
pub fn focus_panel_head(event: &DioxusPointerEvent) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = event;
    }

    #[cfg(target_arch = "wasm32")]
    {
        let Some(head) = event
            .data
            .downcast::<web_sys::PointerEvent>()
            .and_then(|web_event| web_event.target())
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
            .and_then(|element| element.closest(".panel-head").ok().flatten())
            .and_then(|head| head.dyn_into::<web_sys::HtmlElement>().ok())
        else {
            return;
        };
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        let _ = head.focus_with_options(&options);
    }
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

/// Translate a root `pointermove` during a held or lifted tile drag.
///
/// The target is the resting tile slot under the pointer, resolved by core's
/// [`hit_test`](panel_kit_core::frame::hit_test) against `frame` (the frame
/// the host painted) after mapping client coordinates into the frame through
/// the event target's `.ws` ancestor. A pointer outside the browser window
/// cancels the drag. Coordinates in the emitted event stay client
/// coordinates, the space the press was reported in.
pub fn tile_drag_pointer_event<K: PanelKey>(
    frame: &ProjectedFrame<'_, K>,
    event: &DioxusPointerEvent,
) -> WorkspaceEvent<K> {
    let client = event.client_coordinates();
    if !inside_window(client.x, client.y) {
        return pointer_event(HitTarget::Workspace, event, PointerEventKind::Cancel);
    }
    let target = workspace_point(frame, event)
        .and_then(|point| panel_kit_core::frame::hit_test(frame, point))
        .unwrap_or(HitTarget::Workspace);
    pointer_event(
        target,
        event,
        PointerEventKind::Drag(PointerButton::Primary),
    )
}

fn inside_window(x: f64, y: f64) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (x, y);
        true
    }

    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return true;
        };
        let width = window.inner_width().ok().and_then(|value| value.as_f64());
        let height = window.inner_height().ok().and_then(|value| value.as_f64());
        match (width, height) {
            (Some(width), Some(height)) => x >= 0.0 && y >= 0.0 && x < width && y < height,
            _ => true,
        }
    }
}

/// Client coordinates mapped into `frame` coordinates through the `.ws`
/// workspace element that contains the event target.
fn workspace_point<K: PanelKey>(
    frame: &ProjectedFrame<'_, K>,
    event: &DioxusPointerEvent,
) -> Option<(f64, f64)> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (frame, event);
        None
    }

    #[cfg(target_arch = "wasm32")]
    {
        let web_event = event.data.downcast::<web_sys::PointerEvent>()?;
        let workspace = web_event
            .target()?
            .dyn_into::<web_sys::Element>()
            .ok()?
            .closest(".ws")
            .ok()??;
        let rect = workspace.get_bounding_client_rect();
        let client = event.client_coordinates();
        let origin = frame.chrome.workspace;
        Some((
            client.x - rect.left() + f64::from(workspace.scroll_left()) + origin.x,
            client.y - rect.top() + f64::from(workspace.scroll_top()) + origin.y
                - frame.workspace_scroll,
        ))
    }
}

/// Drive tile-drag animation from the browser's frame clock.
///
/// While `animating` (see
/// [`TileDrag::animating`](panel_kit_core::tile_drag::TileDrag::animating)),
/// exactly one animation frame is pending; when it fires, the time since the
/// previous frame reaches `emit` as a
/// [`WorkspaceEvent::Tick`]. Under `prefers-reduced-motion: reduce` the tick
/// is `f64::INFINITY`, which completes the animation at once. Nothing is
/// scheduled while idle.
pub fn use_tile_drag_frames<K: PanelKey>(animating: bool, emit: EventHandler<WorkspaceEvent<K>>) {
    let pending = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(false)));
    let last_frame = use_hook(|| std::rc::Rc::new(std::cell::Cell::new(None::<f64>)));
    if !animating {
        last_frame.set(None);
        return;
    }
    if pending.replace(true) {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = emit;
    }

    #[cfg(target_arch = "wasm32")]
    spawn(async move {
        let now = next_animation_frame().await;
        pending.set(false);
        let dt_ms = if prefers_reduced_motion() {
            f64::INFINITY
        } else {
            const FIRST_FRAME_MS: f64 = 1000.0 / 60.0;
            last_frame
                .replace(Some(now))
                .map_or(FIRST_FRAME_MS, |previous| now - previous)
        };
        emit.call(WorkspaceEvent::Tick { dt_ms });
    });
}

/// Resolve with the timestamp of the next animation frame.
#[cfg(target_arch = "wasm32")]
async fn next_animation_frame() -> f64 {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.request_animation_frame(&resolve);
        }
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .ok()
        .and_then(|now| now.as_f64())
        .unwrap_or(0.0)
}

#[cfg(target_arch = "wasm32")]
fn prefers_reduced_motion() -> bool {
    web_sys::window()
        .and_then(|window| {
            window
                .match_media("(prefers-reduced-motion: reduce)")
                .ok()
                .flatten()
        })
        .is_some_and(|query| query.matches())
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
        DioxusKey::Backspace => Some(Key::Backspace),
        DioxusKey::Delete => Some(Key::Delete),
        DioxusKey::Home => Some(Key::Home),
        DioxusKey::End => Some(Key::End),
        DioxusKey::PageUp => Some(Key::PageUp),
        DioxusKey::PageDown => Some(Key::PageDown),
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
