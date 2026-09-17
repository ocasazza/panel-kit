//! Independently callable web panel composition parts.

use dioxus::events::{KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use panel_kit_core::frame::PanelProjection;
use panel_kit_core::panel::PanelMeta;
use panel_kit_core::reducer::{HitTarget, PanelPart, WorkspaceEvent};
use panel_kit_core::{PanelCommand, PanelKey, PointerButton, PointerEventKind};

use super::panel_layout::panel_class;
pub use super::panel_layout::panel_style;
use crate::input::{capture_pointer, core_pointer_event, keyboard_event, release_pointer};

/// Paint the positioned panel container and caller-selected child parts.
///
/// The shell is the slot API for faithful controller-era composition: hosts
/// place chrome, body, and resize parts inside this positioned `.panel`
/// ancestor so absolutely positioned CSS remains anchored correctly.
pub fn panel_shell<K: PanelKey>(
    panel: PanelProjection<K>,
    class: Option<&str>,
    children: Element,
) -> Element {
    let class = panel_class(panel, class);
    let style = panel_style(panel);

    rsx! {
        section {
            class: "{class}",
            style: "{style}",
            {children}
        }
    }
}

/// Paint the positioned panel container wired for host-owned event loops:
/// a primary pointer press on the panel surface emits a reducer event that
/// focuses the panel and, in floating mode, raises it to the front — the
/// window-manager behavior every controller-era workspace had. Chrome parts
/// (header, grip, lights) stop propagation, so only genuine body presses
/// reach this handler. [`panel_shell`] remains the passive painter for
/// static snapshots and SSR probes.
pub fn panel_shell_with_events<K: PanelKey>(
    panel: PanelProjection<K>,
    class: Option<&str>,
    children: Element,
    emit: EventHandler<WorkspaceEvent<K>>,
) -> Element {
    let class = panel_class(panel, class);
    let style = panel_style(panel);

    rsx! {
        section {
            class: "{class}",
            style: "{style}",
            onpointerdown: move |event: DioxusPointerEvent| {
                emit.call(pointer_workspace_event(
                    panel.key,
                    PanelPart::Surface,
                    PointerEventKind::Down(PointerButton::Primary),
                    &event,
                ));
            },
            {children}
        }
    }
}

/// Paint the native-overflow body for one projected panel.
pub fn panel_body(body: Element) -> Element {
    rsx! {
        div {
            class: "panel-body",
            style: "overflow:auto;",
            {body}
        }
    }
}

/// Paint a standalone panel surface with no implicit chrome, controls, or dock.
///
/// Use [`panel_shell`] plus [`panel_body`] when composing additional opt-in
/// parts inside the same positioned panel container.
pub fn panel_surface<K: PanelKey>(
    panel: PanelProjection<K>,
    class: Option<&str>,
    body: Element,
) -> Element {
    panel_shell(panel, class, panel_body(body))
}

/// Paint the focusable semantic panel toolbar and title row.
///
/// This preserves the existing public chrome part. It intentionally mounts no
/// traffic lights; use [`panel_chrome_with_controls`] when the host wants the
/// independent controls slotted inside the toolbar.
pub fn panel_chrome<K: PanelKey>(
    panel: PanelProjection<K>,
    meta: &PanelMeta<K>,
    header_actions: Option<Element>,
) -> Element {
    panel_chrome_with_controls(panel, meta, None, header_actions)
}

/// Paint the focusable panel toolbar with opt-in leading controls.
pub fn panel_chrome_with_controls<K: PanelKey>(
    panel: PanelProjection<K>,
    meta: &PanelMeta<K>,
    leading_controls: Option<Element>,
    header_actions: Option<Element>,
) -> Element {
    let title = &meta.title;
    let show_max_hint = panel.state == panel_kit_core::WinState::Maximized;

    rsx! {
        header {
            class: "panel-head",
            title: "{title}",
            tabindex: "0",
            role: "toolbar",
            {leading_controls.unwrap_or_else(|| rsx! {})}
            span { class: "panel-title", title: "{title}", "{title}" }
            if show_max_hint {
                span { class: "max-hint", "maximized" }
            }
            div {
                class: "panel-head-actions",
                onpointerdown: move |event: DioxusPointerEvent| event.stop_propagation(),
                onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
                {header_actions.unwrap_or_else(|| rsx! {})}
            }
        }
    }
}
/// Paint panel chrome that translates header gestures into reducer events.
///
/// This is the host-owned-loop variant used by examples and applications that
/// want faithful move/reorder keyboard behavior without the controller. The
/// plain [`panel_chrome_with_controls`] remains a passive painter for static
/// snapshots, SSR probes, and hosts that wire input elsewhere.
pub fn panel_chrome_with_events<K: PanelKey>(
    panel: PanelProjection<K>,
    meta: &PanelMeta<K>,
    emit: EventHandler<WorkspaceEvent<K>>,
    leading_controls: Option<Element>,
    header_actions: Option<Element>,
) -> Element {
    let title = &meta.title;
    let show_max_hint = panel.state == panel_kit_core::WinState::Maximized;

    rsx! {
        header {
            class: "panel-head",
            title: "{title}",
            tabindex: "0",
            role: "toolbar",
            onkeydown: move |event: KeyboardEvent| {
                event.stop_propagation();
                if let Some(workspace_event) = keyboard_event(&event, panel_kit_core::FocusContext::Panel(panel.key)) {
                    event.prevent_default();
                    emit.call(workspace_event);
                }
            },
            onpointerdown: move |event: DioxusPointerEvent| {
                event.stop_propagation();
                event.prevent_default();
                emit.call(pointer_workspace_event(
                    panel.key,
                    PanelPart::Header,
                    PointerEventKind::Down(PointerButton::Primary),
                    &event,
                ));
            },
            onpointermove: move |event: DioxusPointerEvent| {
                emit.call(pointer_workspace_event(
                    panel.key,
                    PanelPart::Header,
                    PointerEventKind::Moved,
                    &event,
                ));
            },
            {leading_controls.unwrap_or_else(|| rsx! {})}
            span { class: "panel-title", title: "{title}", "{title}" }
            if show_max_hint {
                span { class: "max-hint", "maximized" }
            }
            div {
                class: "panel-head-actions",
                onpointerdown: move |event: DioxusPointerEvent| event.stop_propagation(),
                onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
                {header_actions.unwrap_or_else(|| rsx! {})}
            }
        }
    }
}

/// Paint the mode/minimize/maximize controls for a projected panel.
pub fn traffic_lights<K: PanelKey>(
    panel: PanelProjection<K>,
    emit: EventHandler<WorkspaceEvent<K>>,
) -> Element {
    let lights = [
        (
            panel.chrome.mode_hit.is_some(),
            "light mode",
            "tiling / floating",
            PanelCommand::ToggleMode,
        ),
        (
            panel.chrome.minimize_hit.is_some(),
            "light yellow",
            "minimize",
            PanelCommand::Minimize,
        ),
        (
            panel.chrome.maximize_hit.is_some(),
            "light max",
            "maximize / restore",
            PanelCommand::Maximize,
        ),
    ];

    rsx! {
        div { class: "lights",
            for (enabled, class, title, command) in lights {
                if enabled {
                    button {
                        r#type: "button",
                        class: "{class}",
                        title: "{title}",
                        aria_label: "{title}",
                        onpointerdown: move |event: DioxusPointerEvent| event.stop_propagation(),
                        onkeydown: move |event: KeyboardEvent| event.stop_propagation(),
                        onclick: move |_| emit.call(WorkspaceEvent::Command { target: Some(panel.key), command }),
                    }
                }
            }
        }
    }
}

/// Paint the panel resize grip and translate pointer gestures to core events.
pub fn resize_grip<K: PanelKey>(
    panel: PanelProjection<K>,
    emit: EventHandler<WorkspaceEvent<K>>,
) -> Element {
    if panel.chrome.resize_hit.is_none() {
        return rsx! {};
    }

    rsx! {
        button {
            r#type: "button",
            class: "resize",
            aria_label: "Resize panel",
            tabindex: "0",
            onpointerdown: move |event: DioxusPointerEvent| {
                event.stop_propagation();
                event.prevent_default();
                capture_pointer(&event);
                emit.call(pointer_workspace_event(panel.key, PanelPart::ResizeGrip, PointerEventKind::Down(PointerButton::Primary), &event));
            },
            onpointermove: move |event: DioxusPointerEvent| {
                emit.call(pointer_workspace_event(panel.key, PanelPart::ResizeGrip, PointerEventKind::Drag(PointerButton::Primary), &event));
            },
            onpointerup: move |event: DioxusPointerEvent| {
                release_pointer(&event);
                emit.call(pointer_workspace_event(panel.key, PanelPart::ResizeGrip, PointerEventKind::Up(PointerButton::Primary), &event));
            },
            onpointercancel: move |event: DioxusPointerEvent| {
                release_pointer(&event);
                emit.call(pointer_workspace_event(panel.key, PanelPart::ResizeGrip, PointerEventKind::Up(PointerButton::Primary), &event));
            },
        }
    }
}

fn pointer_workspace_event<K: PanelKey>(
    key: K,
    part: PanelPart,
    kind: PointerEventKind,
    event: &DioxusPointerEvent,
) -> WorkspaceEvent<K> {
    WorkspaceEvent::Pointer {
        target: HitTarget::Panel { key, part },
        event: core_pointer_event(event, kind),
    }
}
