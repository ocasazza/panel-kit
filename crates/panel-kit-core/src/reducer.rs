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

use crate::tile_drag::{occupies_slot, swap_tiles, TileDrag, TileDragPhase};
use crate::{
    apply_command, apply_drag, begin_drag, begin_tile_resize, clamp_scroll, command_for,
    effective_mode, restore, DragKind, FocusContext, Key, KeyChord, Mode, PanelCommand, PanelKey,
    PanelWin, PointerButton, PointerEvent, PointerEventKind, Units, WinState,
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
        WorkspaceEvent::Tick { dt_ms } => reduce_tick(snapshot, dt_ms),
    }
}

/// The keyboard path: [`command_for`] decides whether the chord is a
/// window-management intent at all, then the focused panel (if any)
/// receives it. Hosts that derive focus from the event origin (the TUI
/// focuses the panel a key was delivered through) mutate `focused` before
/// reducing — adoption is host policy, not a core transition.
///
/// Escape during a held or lifted tile drag cancels the drag and is not
/// also a restore command.
fn reduce_key<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    chord: KeyChord,
    focus: &FocusContext<K>,
) -> Reduction<K> {
    if chord.key == Key::Escape && snapshot.tile_drag.is_some_and(|tile| tile.tracks_pointer()) {
        return cancel_tile_drag(snapshot);
    }
    let Some(command) = command_for(chord, focus) else {
        return Reduction::unchanged();
    };
    apply_command_to_snapshot(snapshot, context, None, command)
}

/// Apply one resolved command exactly the way the controllers' direct-call
/// handoff does: resolve the target onto `focused`, delegate to
/// [`apply_command`], write `focused` back, and report what actually
/// changed.
///
/// A command ends any tile drag first, without a swap: commands may
/// minimize, maximize, or re-mode the dragged panel.
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

    let dropped_drag = snapshot.tile_drag.take().is_some();
    if !apply_command_with_report(snapshot, context, target, command) {
        if dropped_drag {
            return changed(ChangePhase::Transient, None);
        }
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
        PointerEventKind::Up(PointerButton::Primary) => reduce_pointer_up(snapshot, event),
        PointerEventKind::Cancel => reduce_pointer_cancel(snapshot),
        PointerEventKind::Scroll { delta_y } => reduce_wheel(
            snapshot,
            context,
            delta_y,
            WheelDisposition::BubbleToWorkspace,
        ),
    }
}

/// Start drag/reorder/restore gestures from pointer-down without duplicating
/// the transition math those gestures use.
///
/// A press while a tile drag is held or lifted is ignored (a second contact
/// or button). A press while a tile is landing finishes the landing at once
/// and is then handled normally.
fn reduce_pointer_down<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    target: HitTarget<K>,
    event: PointerEvent,
) -> Reduction<K> {
    let landed = match snapshot.tile_drag {
        Some(tile) if tile.tracks_pointer() => return Reduction::unchanged(),
        Some(_) => snapshot.tile_drag.take().is_some(),
        None => false,
    };
    let reduction = match target {
        HitTarget::Workspace => Reduction::unchanged(),
        HitTarget::Dock { key } => reduce_dock_restore(snapshot, key),
        HitTarget::Panel { key, part } => {
            reduce_panel_pointer_down(snapshot, context, key, part, event)
        }
    };
    if landed && !reduction.changed {
        return changed(ChangePhase::Transient, None);
    }
    reduction
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
        | PanelPart::MaximizeControl => focus_panel(
            snapshot,
            key,
            effective_mode(snapshot.preferred_mode, &context.surface),
        ),
    }
}

/// Start either a floating drag, a tiling resize, or a tile drag press.
///
/// A tile drag needs the move policy, a surface that offers window
/// management, and a panel that occupies a tile slot (not maximized).
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
        if context.snap.move_
            && context.surface.window_management()
            && snapshot.panels[index].state == WinState::Floating
        {
            snapshot.tile_drag = Some(TileDrag::pressed(key, (event.x, event.y)));
        }
        snapshot.focused = Some(key);
    } else if mode == Mode::Tiling {
        snapshot.drag = begin_tile_resize(&snapshot.panels, index, event.x, event.y);
        if snapshot.drag.is_some() {
            snapshot.focused = Some(key);
        }
    } else {
        raise_to_front(&mut snapshot.panels, key);
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

/// Focus a pointer-targeted panel. In floating mode a click also raises the
/// panel to the front of the stacking order — the window-manager behavior
/// every backend shared before the reducer recomposition. Tiling leaves
/// stacking untouched: z only matters when panels overlap, and mutating it
/// would re-render and swallow clicks on panel content.
fn focus_panel<K: PanelKey>(snapshot: &mut Snapshot<K>, key: K, mode: Mode) -> Reduction<K> {
    let focused_before = snapshot.focused;
    let raised = mode == Mode::Floating && raise_to_front(&mut snapshot.panels, key);

    snapshot.focused = Some(key);

    if snapshot.focused == focused_before && !raised {
        return Reduction::unchanged();
    }

    changed(ChangePhase::Continuous, None)
}

/// Bring a floating panel to the front of the stacking order, reporting
/// whether stacking changed. No-op when the panel already sits on top.
fn raise_to_front<K: PanelKey>(panels: &mut [PanelWin<K>], key: K) -> bool {
    let top = panels.iter().map(|panel| panel.z).max().unwrap_or(0);
    let Some(panel) = panels.iter_mut().find(|panel| panel.kind == key) else {
        return false;
    };
    if panel.z >= top {
        return false;
    }
    panel.z = top + 1;
    true
}

/// Apply in-flight drag motion: floating move/resize through the core drag
/// helpers, or a tile drag's lift and retarget.
fn reduce_pointer_motion<K: PanelKey>(
    snapshot: &mut Snapshot<K>,
    context: ReduceContext<'_>,
    target: HitTarget<K>,
    event: PointerEvent,
) -> Reduction<K> {
    if let Some(drag) = snapshot.drag {
        return apply_pointer_drag(snapshot, context, drag, event);
    }

    let Some(mut tile) = snapshot.tile_drag else {
        return Reduction::unchanged();
    };
    if !tile.tracks_pointer() || !occupies_slot(&snapshot.panels, &tile.key) {
        return Reduction::unchanged();
    }
    let before = tile;
    tile.pointer = (event.x, event.y);
    if tile.phase == TileDragPhase::Pressed && tile.past_threshold(snapshot.viewport.units) {
        tile.phase = TileDragPhase::Lifted;
    }
    if tile.phase == TileDragPhase::Lifted {
        tile.retarget(swap_target(&snapshot.panels, &tile, target));
    }
    if tile == before {
        return Reduction::unchanged();
    }
    snapshot.tile_drag = Some(tile);
    changed(ChangePhase::Transient, None)
}

/// The swap target a motion hit names for `tile`.
///
/// A hit on the dragged panel is its own slot (`None`). A hit on another
/// slotted panel targets it. Anything else keeps the current target, so a
/// backend that cannot resolve a slot never flickers the preview.
fn swap_target<K: PanelKey>(
    panels: &[PanelWin<K>],
    tile: &TileDrag<K>,
    hit: HitTarget<K>,
) -> Option<K> {
    match hit {
        HitTarget::Panel { key, .. } if key == tile.key => None,
        HitTarget::Panel { key, .. } if occupies_slot(panels, &key) => Some(key),
        _ => tile.target,
    }
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
        context.snap,
        snapshot.viewport.width,
        context.clamp,
        context.tile,
    );

    if !panel_changed(&snapshot.panels, before) {
        return Reduction::unchanged();
    }

    changed(ChangePhase::Continuous, None)
}

/// Settle an active pointer gesture.
///
/// A floating drag or tile resize clears its marker. A held tile was a
/// click and is dropped. A lifted tile commits the swap with its target to
/// the panel order — the one persisted change of the gesture — and starts
/// settling; with no target it flies home and nothing is persisted.
fn reduce_pointer_up<K: PanelKey>(snapshot: &mut Snapshot<K>, event: PointerEvent) -> Reduction<K> {
    if snapshot.drag.take().is_some() {
        return changed(ChangePhase::Settled, None);
    }
    let Some(mut tile) = snapshot.tile_drag else {
        return Reduction::unchanged();
    };
    match tile.phase {
        TileDragPhase::Pressed => {
            snapshot.tile_drag = None;
            changed(ChangePhase::Settled, None)
        }
        TileDragPhase::Lifted => {
            tile.pointer = (event.x, event.y);
            let target = tile
                .target
                .filter(|target| occupies_slot(&snapshot.panels, target));
            tile.retarget(target);
            tile.phase = TileDragPhase::Settling;
            tile.land = 0.0;
            let swapped =
                target.is_some_and(|target| swap_tiles(&mut snapshot.panels, tile.key, target));
            snapshot.tile_drag = Some(tile);
            changed(
                if swapped {
                    ChangePhase::Settled
                } else {
                    ChangePhase::Transient
                },
                None,
            )
        }
        TileDragPhase::Settling | TileDragPhase::Cancelling => Reduction::unchanged(),
    }
}

/// Abandon an active pointer gesture. A floating drag or tile resize
/// settles where it is, exactly like a release.
fn reduce_pointer_cancel<K: PanelKey>(snapshot: &mut Snapshot<K>) -> Reduction<K> {
    if snapshot.drag.take().is_some() {
        return changed(ChangePhase::Settled, None);
    }
    cancel_tile_drag(snapshot)
}

/// Cancel a tile drag: a held tile is dropped, a lifted tile flies home
/// with the panel order untouched.
fn cancel_tile_drag<K: PanelKey>(snapshot: &mut Snapshot<K>) -> Reduction<K> {
    let Some(mut tile) = snapshot.tile_drag else {
        return Reduction::unchanged();
    };
    match tile.phase {
        TileDragPhase::Pressed => snapshot.tile_drag = None,
        TileDragPhase::Lifted => {
            tile.cancel();
            snapshot.tile_drag = Some(tile);
        }
        TileDragPhase::Settling | TileDragPhase::Cancelling => return Reduction::unchanged(),
    }
    changed(ChangePhase::Transient, None)
}

/// Advance tile drag animation by host-supplied time; a finished landing
/// drops the drag.
fn reduce_tick<K: PanelKey>(snapshot: &mut Snapshot<K>, dt_ms: f64) -> Reduction<K> {
    if dt_ms.is_nan() || dt_ms <= 0.0 {
        return Reduction::unchanged();
    }
    let Some(mut tile) = snapshot.tile_drag else {
        return Reduction::unchanged();
    };
    let before = tile;
    if tile.advance(dt_ms) {
        snapshot.tile_drag = None;
        return changed(ChangePhase::Transient, None);
    }
    if tile == before {
        return Reduction::unchanged();
    }
    snapshot.tile_drag = Some(tile);
    changed(ChangePhase::Transient, None)
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
    let is_floating = effective_mode(snapshot.preferred_mode, &context.surface) == Mode::Floating;
    if !is_floating && snapshot.viewport.units != Units::Cells {
        return Reduction::unchanged();
    }

    let content_h = if is_floating {
        floating_scroll_extent(&snapshot.panels)
    } else {
        let mut tallest_row = 0.0f64;
        for p in &snapshot.panels {
            if p.state == WinState::Floating {
                tallest_row = tallest_row.max(p.tile_h as f64 * context.tile.row);
            }
        }
        tallest_row.max(snapshot.panels.len() as f64 * context.tile.row)
    };
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
    let panel_before =
        focused.and_then(|kind| panel_by_key(&snapshot.panels, kind).map(|panel| (kind, panel)));

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
pub(crate) mod tests;
