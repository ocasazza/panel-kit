use dioxus::events::{KeyboardEvent, PointerEvent as DioxusPointerEvent, WheelEvent};
use dioxus::prelude::*;
use panel_kit::input::{clear_selection, keyboard_event, pointer_event, release_pointer, wheel_event};
use panel_kit_core::persist::apply_save_decision;
use panel_kit_core::reducer::{reduce, HitTarget, ReduceContext, WorkspaceEvent};
use panel_kit_core::{
    Clamp, CommandStep, FocusContext, PanelKind, PointerButton, PointerEventKind, TileMetrics,
};

use super::{log_layout_error, DemoPanelState};

/// Build an event handler that reduces events and applies the example policy.
pub fn workspace_event_handler<K: PanelKind>(
    workspace: &DemoPanelState<K>,
) -> EventHandler<WorkspaceEvent<K>> {
    let workspace = workspace.clone();
    EventHandler::new(move |event| {
        reduce_workspace_event(&workspace, event);
    })
}

/// Reduce a root keyboard event after the example has declined first refusal.
pub fn handle_key<K: PanelKind>(workspace: &DemoPanelState<K>, event: &KeyboardEvent) {
    let focus = if panel_kit::input::is_editing() {
        FocusContext::TextInput
    } else if let Some(key) = workspace.snapshot.read().focused {
        FocusContext::Panel(key)
    } else {
        FocusContext::Workspace
    };

    let Some(workspace_event) = keyboard_event(event, focus) else {
        return;
    };
    if reduce_workspace_event(workspace, workspace_event) {
        event.prevent_default();
    }
}

/// Reduce a root pointer-move event for in-flight workspace gestures.
pub fn handle_pointer_move<K: PanelKind>(
    workspace: &DemoPanelState<K>,
    event: &DioxusPointerEvent,
) {
    let kind = if workspace.snapshot.read().drag.is_some() {
        PointerEventKind::Drag(PointerButton::Primary)
    } else {
        PointerEventKind::Moved
    };
    reduce_workspace_event(workspace, pointer_event(HitTarget::Workspace, event, kind));
}

/// Settle a pointer gesture and let the example policy persist settled layout.
pub fn handle_pointer_up<K: PanelKind>(workspace: &DemoPanelState<K>, event: &DioxusPointerEvent) {
    release_pointer(event);
    let changed = reduce_workspace_event(
        workspace,
        pointer_event(
            HitTarget::Workspace,
            event,
            PointerEventKind::Up(PointerButton::Primary),
        ),
    );
    if changed {
        clear_selection();
    }
}

/// Reduce a root wheel event after native panel body scroll gets first refusal.
pub fn handle_wheel<K: PanelKind>(workspace: &DemoPanelState<K>, event: &WheelEvent) {
    if reduce_workspace_event(workspace, wheel_event(event)) {
        event.prevent_default();
    }
}

/// Reduce one host-owned event and expose whether it changed the snapshot.
pub(super) fn reduce_workspace_event<K: PanelKind>(
    workspace: &DemoPanelState<K>,
    event: WorkspaceEvent<K>,
) -> bool {
    let mut snapshot_signal = workspace.snapshot;
    let mut snapshot = snapshot_signal.write();
    let context = reduce_context(panel_kit::surface::surface_profile(snapshot.viewport.width));
    let reduction = reduce(&mut snapshot, event, context);
    let decision = workspace.save_policy.decide(&reduction);
    let changed = reduction.changed;

    if let Err(error) = apply_save_decision(decision, &*workspace.store, &snapshot, &workspace.catalog) {
        log_layout_error("save layout", workspace.storage_key, &error);
    }

    changed
}

fn reduce_context(surface: panel_kit_core::SurfaceProfile) -> ReduceContext<'static> {
    ReduceContext {
        surface,
        clamp: &Clamp::WEB,
        command_step: CommandStep::WEB,
        tile: &TileMetrics::WEB,
    }
}
