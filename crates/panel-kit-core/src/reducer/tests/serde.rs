use crate::{Key, PanelCommand};

/// Design §10.1: `PanelCommand` serializes internally tagged by `kind` in
/// snake_case — the authored form Nix and command palettes emit. Additive only:
/// the command carried no wire form before.
#[test]
fn panel_command_serde_is_internally_tagged_snake_case() {
    assert_eq!(
        serde_json::to_value(PanelCommand::Move { dx: -16.0, dy: 0.0 }).unwrap(),
        serde_json::json!({ "kind": "move", "dx": -16.0, "dy": 0.0 }),
    );
    assert_eq!(
        serde_json::to_value(PanelCommand::Resize { dw: 0.0, dh: 16.0 }).unwrap(),
        serde_json::json!({ "kind": "resize", "dw": 0.0, "dh": 16.0 }),
    );
    assert_eq!(
        serde_json::to_value(PanelCommand::Minimize).unwrap(),
        serde_json::json!({ "kind": "minimize" }),
    );
    assert_eq!(
        serde_json::to_value(PanelCommand::ToggleMode).unwrap(),
        serde_json::json!({ "kind": "toggle_mode" }),
    );

    let every_command = [
        PanelCommand::Move { dx: -16.0, dy: 1.0 },
        PanelCommand::Resize { dw: 0.0, dh: 16.0 },
        PanelCommand::Minimize,
        PanelCommand::Maximize,
        PanelCommand::Restore,
        PanelCommand::ToggleMode,
        PanelCommand::Raise,
        PanelCommand::FocusNext,
        PanelCommand::FocusPrev,
    ];
    for command in every_command {
        let json = serde_json::to_string(&command).unwrap();
        assert_eq!(
            serde_json::from_str::<PanelCommand>(&json).unwrap(),
            command,
            "{json}"
        );
    }

    // Spellings outside the authored contract are rejected rather than
    // defaulted.
    assert!(serde_json::from_str::<PanelCommand>("{\"kind\":\"shrink\"}").is_err());
}

/// Design §10.1: `Key` serializes in the external authored form —
/// non-character keys are snake_case strings, a printable key is
/// `{ "char": "m" }`.
#[test]
fn key_serde_is_external_snake_case_strings_and_char_objects() {
    assert_eq!(
        serde_json::to_value(Key::Left).unwrap(),
        serde_json::json!("left")
    );
    assert_eq!(
        serde_json::to_value(Key::Enter).unwrap(),
        serde_json::json!("enter")
    );
    assert_eq!(
        serde_json::to_value(Key::Char('m')).unwrap(),
        serde_json::json!({ "char": "m" }),
    );

    let every_key = [
        Key::Left,
        Key::Right,
        Key::Up,
        Key::Down,
        Key::Enter,
        Key::Escape,
        Key::Tab,
        Key::Char('m'),
        Key::Char('M'),
    ];
    for key in every_key {
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(serde_json::from_str::<Key>(&json).unwrap(), key, "{json}");
    }

    // Unknown key names stay errors, not silent defaults.
    assert!(serde_json::from_str::<Key>("\"pageup\"").is_err());
}
