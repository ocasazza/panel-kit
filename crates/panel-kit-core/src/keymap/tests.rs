use super::*;

fn c(ch: char) -> KeyChord {
    bare(Key::Char(ch))
}

fn shifted(ch: char) -> KeyChord {
    KeyChord {
        shift: true,
        ..bare(Key::Char(ch))
    }
}

fn rule(keys: &str, intent: &str) -> RuleSpec {
    RuleSpec {
        keys: keys.into(),
        intent: intent.into(),
        modes: None,
        enter: None,
        label: None,
        group: None,
    }
}

fn layer(id: &str, modes: &[&str], rules: Vec<RuleSpec>) -> KeymapSpec {
    KeymapSpec {
        spec_version: KEYMAP_SPEC_VERSION,
        id: id.into(),
        modes: modes.iter().map(|m| (*m).to_string()).collect(),
        rules,
    }
}

fn vi() -> Keymap<String> {
    Keymap::compile(&[layer(
        "vi",
        &["normal", "visual"],
        vec![
            rule("j", "down"),
            rule("g g", "first"),
            rule("G", "last"),
            rule("m {a-z}", "mark $1"),
            rule("<c-o>", "back"),
            rule("<space> x", "space-x"),
            RuleSpec {
                keys: "v".into(),
                intent: "visual".into(),
                modes: Some(vec!["normal".into()]),
                enter: Some("visual".into()),
                label: None,
                group: None,
            },
            RuleSpec {
                keys: "y".into(),
                intent: "yank".into(),
                modes: Some(vec!["visual".into()]),
                enter: Some("normal".into()),
                label: None,
                group: None,
            },
        ],
    )])
    .unwrap()
}

fn intent(map: &Keymap<String>, state: &mut KeymapState, chords: &[KeyChord]) -> Resolved<String> {
    let mut last = Resolved::Cancelled;
    for chord in chords {
        last = map.feed(state, *chord);
    }
    last
}

#[test]
fn single_key_fires_and_sequence_pends_then_fires() {
    let map = vi();
    let mut state = KeymapState::default();
    assert_eq!(
        map.feed(&mut state, c('j')),
        Resolved::Intent {
            intent: "down".into(),
            count: None
        }
    );
    assert_eq!(
        map.feed(&mut state, c('g')),
        Resolved::Pending {
            candidates: vec![Candidate { keys: "g g".into(), label: "first".into() }]
        }
    );
    assert_eq!(
        map.feed(&mut state, c('g')),
        Resolved::Intent {
            intent: "first".into(),
            count: None
        }
    );
    assert!(state.pending.is_empty());
}

#[test]
fn count_prefix_accumulates_and_is_delivered_once() {
    let map = vi();
    let mut state = KeymapState::default();
    assert_eq!(
        intent(&map, &mut state, &[c('1'), c('2'), c('j')]),
        Resolved::Intent {
            intent: "down".into(),
            count: Some(12)
        }
    );
    assert_eq!(
        map.feed(&mut state, c('j')),
        Resolved::Intent {
            intent: "down".into(),
            count: None
        }
    );
}

#[test]
fn leading_zero_is_unbound_not_a_count() {
    let map = vi();
    let mut state = KeymapState::default();
    assert_eq!(map.feed(&mut state, c('0')), Resolved::Unbound(c('0')));
    assert_eq!(state.count, None);
}

#[test]
fn uppercase_matches_with_or_without_shift_flag() {
    let map = vi();
    for chord in [c('G'), shifted('G')] {
        let mut state = KeymapState::default();
        assert_eq!(
            map.feed(&mut state, chord),
            Resolved::Intent {
                intent: "last".into(),
                count: None
            },
            "{chord:?}"
        );
    }
}

#[test]
fn capture_substitutes_into_intent() {
    let map = vi();
    let mut state = KeymapState::default();
    assert_eq!(
        intent(&map, &mut state, &[c('m'), c('q')]),
        Resolved::Intent {
            intent: "mark q".into(),
            count: None
        }
    );
    // A capture slot rejects anything but a bare lowercase letter.
    let mut state = KeymapState::default();
    assert_eq!(
        intent(&map, &mut state, &[c('m'), c('Q')]),
        Resolved::Unbound(c('Q'))
    );
}

#[test]
fn modifier_and_named_tokens_parse() {
    let map = vi();
    let mut state = KeymapState::default();
    let ctrl_o = KeyChord {
        ctrl: true,
        ..bare(Key::Char('o'))
    };
    assert_eq!(
        map.feed(&mut state, ctrl_o),
        Resolved::Intent {
            intent: "back".into(),
            count: None
        }
    );
    assert_eq!(
        intent(&map, &mut state, &[c(' '), c('x')]),
        Resolved::Intent {
            intent: "space-x".into(),
            count: None
        }
    );
}

#[test]
fn escape_cancels_pending_and_count_and_resets_mode() {
    let map = vi();
    let mut state = KeymapState::default();
    map.feed(&mut state, c('v'));
    assert_eq!(map.mode_name(&state), "visual");
    map.feed(&mut state, c('3'));
    assert_eq!(map.feed(&mut state, bare(Key::Escape)), Resolved::Cancelled);
    assert_eq!(state, KeymapState::default());
    // Escape with nothing pending is the host's.
    assert_eq!(
        map.feed(&mut state, bare(Key::Escape)),
        Resolved::Unbound(bare(Key::Escape))
    );
}

#[test]
fn rules_are_scoped_to_modes_and_enter_switches() {
    let map = vi();
    let mut state = KeymapState::default();
    assert_eq!(map.feed(&mut state, c('y')), Resolved::Unbound(c('y')));
    map.feed(&mut state, c('v'));
    assert_eq!(
        map.feed(&mut state, c('y')),
        Resolved::Intent {
            intent: "yank".into(),
            count: None
        }
    );
    assert_eq!(map.mode_name(&state), "normal");
}

#[test]
fn unbound_chord_drops_the_sequence() {
    let map = vi();
    let mut state = KeymapState::default();
    map.feed(&mut state, c('g'));
    assert_eq!(map.feed(&mut state, c('z')), Resolved::Unbound(c('z')));
    assert!(state.pending.is_empty());
}

#[test]
fn later_layer_shadows_same_sequence() {
    let base = layer("base", &["normal"], vec![rule("j", "down"), rule("k", "up")]);
    let app = layer("app", &["normal"], vec![rule("j", "app-down")]);
    let map: Keymap<String> = Keymap::compile(&[base, app]).unwrap();
    let mut state = KeymapState::default();
    assert_eq!(
        map.feed(&mut state, c('j')),
        Resolved::Intent {
            intent: "app-down".into(),
            count: None
        }
    );
    assert_eq!(
        map.feed(&mut state, c('k')),
        Resolved::Intent {
            intent: "up".into(),
            count: None
        }
    );
}

#[test]
fn prefix_conflict_is_a_compile_error() {
    let spec = layer("x", &["normal"], vec![rule("g", "a"), rule("g g", "b")]);
    assert_eq!(
        Keymap::<String>::compile(&[spec]).err(),
        Some(KeymapError::Prefix {
            mode: "normal".into(),
            prefix: "g".into(),
            longer: "g g".into()
        })
    );
}

#[test]
fn vocabulary_rejects_bad_intents_at_compile_including_templates() {
    #[derive(Clone, Debug, PartialEq)]
    enum V {
        Down,
        Mark(char),
    }
    impl IntentVocabulary for V {
        fn parse_intent(s: &str) -> Result<Self, String> {
            match s.split_whitespace().collect::<Vec<_>>().as_slice() {
                ["down"] => Ok(V::Down),
                ["mark", m] if m.len() == 1 => Ok(V::Mark(m.chars().next().unwrap())),
                _ => Err(format!("unknown `{s}`")),
            }
        }
    }
    let ok = layer("x", &["normal"], vec![rule("j", "down"), rule("m {a-z}", "mark $1")]);
    let map = Keymap::<V>::compile(&[ok]).unwrap();
    let mut state = KeymapState::default();
    assert_eq!(
        intent_v(&map, &mut state, &[c('m'), c('z')]),
        Resolved::Intent {
            intent: V::Mark('z'),
            count: None
        }
    );

    let bad = layer("x", &["normal"], vec![rule("j", "dwon")]);
    assert!(matches!(
        Keymap::<V>::compile(&[bad]),
        Err(KeymapError::Intent { .. })
    ));
    let bad_template = layer("x", &["normal"], vec![rule("m {a-z}", "mrak $1")]);
    assert!(matches!(
        Keymap::<V>::compile(&[bad_template]),
        Err(KeymapError::Intent { .. })
    ));

    fn intent_v(map: &Keymap<V>, state: &mut KeymapState, chords: &[KeyChord]) -> Resolved<V> {
        let mut last = Resolved::Cancelled;
        for chord in chords {
            last = map.feed(state, *chord);
        }
        last
    }
}

#[test]
fn bad_token_unknown_mode_and_version_are_errors() {
    let bad_token = layer("x", &["normal"], vec![rule("<nope>", "a")]);
    assert!(matches!(
        Keymap::<String>::compile(&[bad_token]),
        Err(KeymapError::Token { token, .. }) if token == "<nope>"
    ));
    let mut bad_mode = layer("x", &["normal"], vec![rule("j", "a")]);
    bad_mode.rules[0].enter = Some("insert".into());
    assert!(matches!(
        Keymap::<String>::compile(&[bad_mode]),
        Err(KeymapError::UnknownMode { mode, .. }) if mode == "insert"
    ));
    let mut bad_version = layer("x", &["normal"], vec![]);
    bad_version.spec_version = 0;
    assert!(matches!(
        Keymap::<String>::compile(&[bad_version]),
        Err(KeymapError::Version { found: 0, .. })
    ));
}

#[test]
fn spec_round_trips_through_json() {
    let spec = layer("vi", &["normal"], vec![rule("g g", "first")]);
    let json = serde_json::to_string(&spec).unwrap();
    assert_eq!(serde_json::from_str::<KeymapSpec>(&json).unwrap(), spec);
    assert!(serde_json::from_str::<KeymapSpec>(r#"{"spec_version":1,"id":"x","modes":[],"rules":[],"extra":1}"#).is_err());
}

#[test]
fn router_bypasses_keymap_for_bare_keys_in_text_input_and_falls_through_unbound() {
    use crate::reducer::tests::fixtures::TestPanel;
    let mut router = InputRouter::new(vi());
    // An editor keeps `j`; a modified chord still reaches the keymap.
    assert_eq!(
        router.route(c('j'), FocusContext::<TestPanel>::TextInput),
        Routed::Workspace(WorkspaceEvent::Key {
            chord: c('j'),
            focus: FocusContext::TextInput
        })
    );
    let ctrl_o = KeyChord {
        ctrl: true,
        ..bare(Key::Char('o'))
    };
    assert_eq!(
        router.route(ctrl_o, FocusContext::<TestPanel>::TextInput),
        Routed::Intent {
            intent: "back".into(),
            count: None
        }
    );
    // Outside an editor the keymap goes first; unbound chords reach the reducer.
    assert_eq!(
        router.route(c('j'), FocusContext::<TestPanel>::Workspace),
        Routed::Intent {
            intent: "down".into(),
            count: None
        }
    );
    assert_eq!(
        router.route(bare(Key::Tab), FocusContext::<TestPanel>::Workspace),
        Routed::Workspace(WorkspaceEvent::Key {
            chord: bare(Key::Tab),
            focus: FocusContext::Workspace
        })
    );
    assert_eq!(
        router.route(c('g'), FocusContext::<TestPanel>::Workspace),
        Routed::Pending {
            candidates: vec![Candidate { keys: "g g".into(), label: "first".into() }]
        }
    );
    assert_eq!(
        router.route(bare(Key::Escape), FocusContext::<TestPanel>::Workspace),
        Routed::Cancelled
    );
    assert!(router.state().pending.is_empty());
}
