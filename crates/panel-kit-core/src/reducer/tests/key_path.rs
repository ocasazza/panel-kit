use super::fixtures::{chord, default_snapshot, web_context, TestPanel};
use crate::reducer::{reduce, ChangePhase, WorkspaceEvent};
use crate::{apply_command, command_for, Clamp, CommandStep, FocusContext, Key};

/// Design §15 slice 2: a key event reduced through [`reduce`] must produce
/// exactly the state `command_for` + `apply_command` produce when called
/// directly — the reducer owns no transition logic of its own.
#[test]
fn reduce_key_matches_command_for_and_apply_command() {
    let cases = [
        (chord(Key::Left, false, false), FocusContext::Workspace),
        (chord(Key::Right, false, true), FocusContext::Workspace),
        (chord(Key::Up, true, false), FocusContext::Workspace),
        (chord(Key::Down, true, false), FocusContext::Workspace),
        (chord(Key::Char('m'), false, false), FocusContext::Workspace),
        (chord(Key::Char('f'), false, false), FocusContext::Workspace),
        (chord(Key::Char('t'), false, false), FocusContext::Workspace),
        (chord(Key::Escape, false, false), FocusContext::Workspace),
        (chord(Key::Enter, false, false), FocusContext::Workspace),
        (chord(Key::Tab, false, false), FocusContext::Workspace),
        (chord(Key::Tab, true, false), FocusContext::Workspace),
        (
            chord(Key::Char('m'), false, false),
            FocusContext::Panel(TestPanel::Second),
        ),
        // An editor keeps bare keys, while deliberate window-management chords
        // still map (command_for's TextInput rule).
        (chord(Key::Left, false, false), FocusContext::TextInput),
        (chord(Key::Left, true, false), FocusContext::TextInput),
    ];

    for (chord, focus) in cases {
        let mut reduced = default_snapshot();
        reduced.focused = Some(TestPanel::Second);
        let mut direct = reduced.clone();

        let reduction = reduce(
            &mut reduced,
            WorkspaceEvent::Key { chord, focus },
            web_context(),
        );

        let before = direct.clone();
        if let Some(command) = command_for(chord, &focus) {
            apply_command(
                &mut direct.panels,
                &mut direct.preferred_mode,
                &mut direct.focused,
                command,
                Clamp::WEB,
                CommandStep::WEB,
                direct.viewport.width,
                direct.viewport.height,
            );
        }
        let direct_changed = direct != before;

        assert!(
            reduced.panels == direct.panels,
            "panels diverged for {chord:?} with {focus:?}"
        );
        assert_eq!(reduced.preferred_mode, direct.preferred_mode, "{chord:?}");
        assert_eq!(reduced.focused, direct.focused, "{chord:?}");
        assert_eq!(reduction.changed, direct_changed, "{chord:?}");
        assert_eq!(
            reduction.phase,
            if direct_changed {
                Some(ChangePhase::Settled)
            } else {
                None
            },
            "{chord:?}"
        );
        assert_eq!(reduction.focus_request, None, "{chord:?}");
    }
}

/// The pre-reducer keymap tests pinned two behaviors: bare keys stay with a
/// text input, and FocusNext skips minimized panels and wraps. Both must replay
/// identically through `reduce`.
#[test]
fn reduce_replays_existing_keymap_cases() {
    let mut snapshot = default_snapshot();
    let reduction = reduce(
        &mut snapshot,
        WorkspaceEvent::Key {
            chord: chord(Key::Left, false, false),
            focus: FocusContext::TextInput,
        },
        web_context(),
    );
    assert!(!reduction.changed);
    assert!(snapshot == default_snapshot());

    let mut snapshot = default_snapshot();
    snapshot.focused = Some(TestPanel::First);
    let reduction = reduce(
        &mut snapshot,
        WorkspaceEvent::Key {
            chord: chord(Key::Left, false, false),
            focus: FocusContext::Workspace,
        },
        web_context(),
    );
    assert!(reduction.changed);
    assert_eq!(reduction.phase, Some(ChangePhase::Settled));
    assert_eq!(snapshot.panels[0].x, 4.0); // 20.0 minus one coarse step

    let mut snapshot = default_snapshot();
    snapshot.focused = Some(TestPanel::First);
    snapshot.panels[1].state = crate::WinState::Minimized;
    reduce(
        &mut snapshot,
        WorkspaceEvent::Key {
            chord: chord(Key::Tab, false, false),
            focus: FocusContext::Workspace,
        },
        web_context(),
    );
    assert_eq!(snapshot.focused, Some(TestPanel::Third));
    reduce(
        &mut snapshot,
        WorkspaceEvent::Key {
            chord: chord(Key::Tab, false, false),
            focus: FocusContext::Workspace,
        },
        web_context(),
    );
    assert_eq!(snapshot.focused, Some(TestPanel::First));
}
