//! Host-owned workspace state and the pure event reducer.
//!
//! [`Snapshot`] is the plain-data workspace state a host can keep in a web
//! signal, a crossterm loop struct, or plain locals. Applications feed
//! [`WorkspaceEvent`]s through [`reduce`] and perform whatever effects the
//! returned [`Reduction`] reports.
//!
//! The reducer owns no transition logic of its own: key chords map through
//! [`command_for`], commands apply through [`apply_command`], pointer gestures
//! delegate to the existing drag/reorder/restore free functions, and wheel /
//! viewport policy stays renderer-neutral.
//!
//! [`reduce`] performs no I/O and invokes no callback: it reports effects
//! (did anything change, did the change settle, should focus move)
//! without performing them.

mod types;

pub use types::{
    ChangePhase, HitTarget, PanelPart, ReduceContext, Reduction, ResizePolicy, Snapshot, Viewport,
    WheelDisposition, WorkspaceEvent,
};

use crate::{
    apply_command, apply_drag, begin_drag, begin_tile_resize, clamp_scroll, command_for,
    effective_mode, reorder_tile, restore, DragKind, FocusContext, KeyChord, Mode, PanelCommand,
    PanelKey, PanelWin, PointerButton, PointerEvent, PointerEventKind, WinState,
};

/// Reduce one workspace event against host-owned state, purely.
///
/// Key chords map through [`command_for`] and commands apply through
/// [`apply_command`] — a reduced event produces exactly the state those
/// free functions produce when called directly. A command that changes
/// anything is
/// [`ChangePhase::Settled`]; unmapped chords, invalid targets, and
/// already-satisfied commands are no-ops (`changed: false`), never
/// panics.
///
/// Performs no I/O and invokes no callback; hosts act on the returned
/// [`Reduction`] (persistence policy, focus moves) themselves.
pub fn reduce<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    event: WorkspaceEvent<K>,
    context: ReduceContext<'_>,
) -> Reduction<K> {
    match event {
        WorkspaceEvent::Pointer { target, event } => {
            reduce_pointer(snapshot, context, target, event)
        }
        WorkspaceEvent::Key { chord, focus } => reduce_key(snapshot, context, chord, &focus),
        WorkspaceEvent::Command { target, command } => {
            apply_command_to_snapshot(snapshot, context, target, command)
        }
        WorkspaceEvent::Wheel {
            delta_y,
            disposition,
        } => reduce_wheel(snapshot, context, delta_y, disposition),
        WorkspaceEvent::ViewportChanged { size, policy } => {
            reduce_viewport_changed(snapshot, size, policy)
        }
    }
}

/// The keyboard path: [`command_for`] decides whether the chord is a
/// window-management intent at all, then the focused panel (if any)
/// receives it. Hosts that derive focus from the event origin (the TUI
/// focuses the panel a key was delivered through) mutate `focused` before
/// reducing — adoption is host policy, not a core transition.
fn reduce_key<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    chord: KeyChord,
    focus: &FocusContext<K>,
) -> Reduction<K> {
    let Some(command) = command_for(chord, focus) else {
        return Reduction::unchanged();
    };
    apply_command_to_snapshot(snapshot, context, None, command)
}

/// Apply one resolved command exactly the way the controllers' direct-call
/// handoff does: resolve the target onto `focused`, delegate to
/// [`apply_command`], write `focused` back, and report what actually
/// changed.
fn apply_command_to_snapshot<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    target: Option<K>,
    command: PanelCommand,
) -> Reduction<K> {
    // An authored target absent from the layout is invalid input — a
    // no-op, never a panic, and never a dangling `focused` write-back.
    if let Some(key) = target {
        if !snapshot.panels.iter().any(|panel| panel.kind == key) {
            return Reduction::unchanged();
        }
    }

    if !apply_command_with_report(snapshot, context, target, command) {
        return Reduction::unchanged();
    }

    Reduction {
        changed: true,
        phase: Some(ChangePhase::Settled),
        focus_request: None,
    }
}

/// Reduce pointer input by mapping backend hit targets to the existing
/// pointer transition functions.
fn reduce_pointer<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    target: HitTarget<K>,
    event: PointerEvent,
) -> Reduction<K> {
    match event.kind {
        PointerEventKind::Down(PointerButton::Primary) => {
            reduce_pointer_down(snapshot, context, target, event)
        }
        PointerEventKind::Drag(PointerButton::Primary) | PointerEventKind::Moved => {
            reduce_pointer_motion(snapshot, context, target, event)
        }
        PointerEventKind::Up(PointerButton::Primary) => reduce_pointer_up(snapshot),
        PointerEventKind::Scroll { delta_y } => {
            reduce_wheel(snapshot, context, delta_y, WheelDisposition::BubbleToWorkspace)
        }
    }
}

/// Start drag/reorder/restore gestures from pointer-down without duplicating
/// the transition math those gestures use.
fn reduce_pointer_down<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    target: HitTarget<K>,
    event: PointerEvent,
) -> Reduction<K> {
    match target {
        HitTarget::Workspace => Reduction::unchanged(),
        HitTarget::Dock { key } => reduce_dock_restore(snapshot, key),
        HitTarget::Panel { key, part } => {
            reduce_panel_pointer_down(snapshot, context, key, part, event)
        }
    }
}

/// Begin a panel gesture for a resolved target panel.
fn reduce_panel_pointer_down<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    key: K,
    part: PanelPart,
    event: PointerEvent,
) -> Reduction<K> {
    let Some(index) = panel_index(&snapshot.panels, key) else {
        return Reduction::unchanged();
    };

    match part {
        PanelPart::Header => begin_panel_drag(snapshot, context, index, key, DragKind::Move, event),
        PanelPart::ResizeGrip => {
            begin_panel_drag(snapshot, context, index, key, DragKind::Resize, event)
        }
        PanelPart::Surface
        | PanelPart::ModeControl
        | PanelPart::MinimizeControl
        | PanelPart::MaximizeControl => focus_panel(snapshot, key),
    }
}

/// Start either a floating drag or a tiling drag/reorder marker.
fn begin_panel_drag<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    index: usize,
    key: K,
    drag_kind: DragKind,
    event: PointerEvent,
) -> Reduction<K> {
    let mode = effective_mode(snapshot.preferred_mode, &context.surface);
    let before = panel_by_key(&snapshot.panels, key).map(|panel| (key, panel));
    let focused_before = snapshot.focused;
    let drag_before = snapshot.drag;
    let tile_drag_before = snapshot.tile_drag;

    if mode == Mode::Tiling && drag_kind == DragKind::Move {
        snapshot.tile_drag = Some(key);
        snapshot.focused = Some(key);
    } else if mode == Mode::Tiling {
        snapshot.drag = begin_tile_resize(&snapshot.panels, index, event.x, event.y);
        if snapshot.drag.is_some() {
            snapshot.focused = Some(key);
        }
    } else {
        snapshot.drag = begin_drag(
            &mut snapshot.panels,
            index,
            drag_kind,
            event.x,
            event.y,
            snapshot.viewport.width,
            snapshot.viewport.height,
            context.clamp,
        );
        if snapshot.drag.is_some() {
            snapshot.focused = Some(key);
        }
    }

    if snapshot.focused == focused_before
        && snapshot.drag == drag_before
        && snapshot.tile_drag == tile_drag_before
        && !panel_changed(&snapshot.panels, before)
    {
        return Reduction::unchanged();
    }

    changed(ChangePhase::Continuous, None)
}

/// Focus a pointer-targeted panel without inventing geometry behavior.
fn focus_panel<K: PanelKey>(snapshot: &mut Snapshot<K>, key: K) -> Reduction<K> {
    if snapshot.focused == Some(key) {
        return Reduction::unchanged();
    }

    snapshot.focused = Some(key);
    changed(ChangePhase::Continuous, None)
}

/// Apply in-flight drag/reorder motion through the core drag/reorder helpers.
fn reduce_pointer_motion<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    target: HitTarget<K>,
    event: PointerEvent,
) -> Reduction<K> {
    if let Some(drag) = snapshot.drag {
        return apply_pointer_drag(snapshot, context, drag, event);
    }

    let Some(dragged) = snapshot.tile_drag else {
        return Reduction::unchanged();
    };
    let HitTarget::Panel { key: target, .. } = target else {
        return Reduction::unchanged();
    };
    if dragged == target {
        return Reduction::unchanged();
    }
    if panel_index(&snapshot.panels, dragged).is_none()
        || panel_index(&snapshot.panels, target).is_none()
    {
        return Reduction::unchanged();
    }

    reorder_tile(&mut snapshot.panels, dragged, target);
    changed(ChangePhase::Continuous, None)
}

/// Apply a floating or span-snapped tile resize drag through [`apply_drag`].
fn apply_pointer_drag<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    drag: crate::Drag,
    event: PointerEvent,
) -> Reduction<K> {
    let before = snapshot
        .panels
        .get(drag.idx)
        .copied()
        .map(|panel| (panel.kind, panel));
    let tiling = effective_mode(snapshot.preferred_mode, &context.surface) == Mode::Tiling;

    apply_drag(
        &mut snapshot.panels,
        &drag,
        event.x,
        event.y,
        tiling,
        snapshot.viewport.width,
        context.clamp,
        context.tile,
    );

    if !panel_changed(&snapshot.panels, before) {
        return Reduction::unchanged();
    }

    changed(ChangePhase::Continuous, None)
}

/// Settle an active pointer gesture by clearing reducer-owned drag markers.
fn reduce_pointer_up<K: PanelKey>(snapshot: &mut Snapshot<K>) -> Reduction<K> {
    if snapshot.drag.is_none() && snapshot.tile_drag.is_none() {
        return Reduction::unchanged();
    }

    snapshot.drag = None;
    snapshot.tile_drag = None;
    changed(ChangePhase::Settled, None)
}

/// Restore a dock-targeted panel through the existing free function.
fn reduce_dock_restore<K: PanelKey>(snapshot: &mut Snapshot<K>, key: K) -> Reduction<K> {
    if panel_index(&snapshot.panels, key).is_none() {
        return Reduction::unchanged();
    }

    let focused_before = snapshot.focused;
    let before = panel_by_key(&snapshot.panels, key).map(|panel| (key, panel));
    restore(&mut snapshot.panels, key);
    snapshot.focused = Some(key);

    if snapshot.focused == focused_before && !panel_changed(&snapshot.panels, before) {
        return Reduction::unchanged();
    }

    changed(ChangePhase::Settled, Some(key))
}

/// Apply workspace wheel scrolling after the backend has decided content
/// precedence.
fn reduce_wheel<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    delta_y: f64,
    disposition: WheelDisposition,
) -> Reduction<K> {
    if disposition == WheelDisposition::ContentConsumed || !delta_y.is_finite() {
        return Reduction::unchanged();
    }
    if effective_mode(snapshot.preferred_mode, &context.surface) != Mode::Floating {
        return Reduction::unchanged();
    }

    let content_h = floating_scroll_extent(&snapshot.panels);
    let view_h = workspace_height(snapshot.viewport.height, context.clamp);
    let next = clamp_scroll(snapshot.workspace_scroll + delta_y, content_h, view_h);
    if next == snapshot.workspace_scroll {
        return Reduction::unchanged();
    }

    snapshot.workspace_scroll = next;
    changed(ChangePhase::Settled, None)
}

/// Apply a viewport measurement change according to the event's explicit
/// resize policy.
fn reduce_viewport_changed<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    size: Viewport,
    policy: ResizePolicy,
) -> Reduction<K> {
    if !valid_viewport(size) {
        return Reduction::unchanged();
    }
    if snapshot.viewport == size {
        return Reduction::unchanged();
    }

    if policy == ResizePolicy::ScaleFloating && valid_viewport(snapshot.viewport) {
        scale_floating_geometry(
            &mut snapshot.panels,
            size.width / snapshot.viewport.width,
            size.height / snapshot.viewport.height,
        );
    }
    snapshot.viewport = size;

    changed(ChangePhase::Settled, None)
}

/// Delegate to [`apply_command`] while retaining enough local state to report
/// whether that delegation changed anything.
///
/// The shared command mechanism mutates at most the selected panel plus
/// `mode`/`focused`. Tracking that panel by value avoids cloning the whole
/// workspace vector for every command while preserving CK-26's single source of
/// transition truth.
fn apply_command_with_report<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    target: Option<K>,
    command: PanelCommand,
) -> bool {
    let mode_before = snapshot.preferred_mode;
    let focused_before = snapshot.focused;
    let mut focused = target.or(snapshot.focused);
    let panel_before = focused
        .and_then(|kind| panel_by_key(&snapshot.panels, kind).map(|panel| (kind, panel)));

    apply_command(
        &mut snapshot.panels,
        &mut snapshot.preferred_mode,
        &mut focused,
        command,
        *context.clamp,
        context.command_step,
        snapshot.viewport.width,
        snapshot.viewport.height,
    );
    snapshot.focused = focused;

    snapshot.preferred_mode != mode_before
        || snapshot.focused != focused_before
        || panel_changed(&snapshot.panels, panel_before)
}

fn panel_by_key<K: PanelKey>(panels: &[PanelWin<K>], key: K) -> Option<PanelWin<K>> {
    panels.iter().find(|panel| panel.kind == key).copied()
}

fn panel_changed<K: PanelKey>(panels: &[PanelWin<K>], before: Option<(K, PanelWin<K>)>) -> bool {
    let Some((key, panel_before)) = before else {
        return false;
    };

    panels.iter().find(|panel| panel.kind == key) != Some(&panel_before)
}

/// Build a changed [`Reduction`] with an explicit phase and optional focus
/// request.
fn changed<K: PanelKey>(phase: ChangePhase, focus_request: Option<K>) -> Reduction<K> {
    Reduction {
        changed: true,
        phase: Some(phase),
        focus_request,
    }
}

/// Locate a panel by stable runtime key.
fn panel_index<K: PanelKey>(panels: &[PanelWin<K>], key: K) -> Option<usize> {
    panels.iter().position(|panel| panel.kind == key)
}

/// Compute workspace-level scroll extent for visible floating panels.
fn floating_scroll_extent<K: PanelKey>(panels: &[PanelWin<K>]) -> f64 {
    panels
        .iter()
        .filter(|panel| panel.state == WinState::Floating)
        .map(|panel| panel.y + panel.h)
        .fold(0.0, f64::max)
}

/// Convert total viewport height into the scrollable workspace area height.
fn workspace_height(height: f64, clamp: &crate::Clamp) -> f64 {
    (height - clamp.outer_h).max(clamp.floor_h)
}

/// Check the viewport dimensions core can safely use in ratio arithmetic.
fn valid_viewport(size: Viewport) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width > 0.0 && size.height > 0.0
}

/// Scale stored floating geometry by the viewport delta ratios.
fn scale_floating_geometry<K: PanelKey>(
    panels: &mut [PanelWin<K>],
    width_ratio: f64,
    height_ratio: f64,
) {
    if !width_ratio.is_finite() || !height_ratio.is_finite() {
        return;
    }

    for panel in panels {
        panel.x *= width_ratio;
        panel.y *= height_ratio;
        panel.w *= width_ratio;
        panel.h *= height_ratio;
    }
}

#[cfg(test)]
mod tests;
