use super::common::{catalog, defaults, MemoryStore, TestPanel};
use super::fixtures::v2_unknown_json;
use crate::persist::{apply_save_decision, LayoutStore, SaveDecision, SavePolicy};
use crate::reducer::{ChangePhase, Reduction};

#[test]
fn save_policy_fake_store_write_counts() {
    for (name, policy, reduction, expected_decision, expected_saves) in save_policy_cases() {
        let store = MemoryStore::empty();
        let decision = policy.decide(&reduction);

        assert_eq!(decision, expected_decision, "{name} decision");
        apply_save_decision(decision, &store, &defaults(), &catalog()).unwrap();

        assert_eq!(store.save_calls().len(), expected_saves, "{name}");
        assert_eq!(store.clear_calls(), 0, "{name} clear count");
    }

    assert_reset_clear_does_not_resave();
}

fn save_policy_cases() -> [(
    &'static str,
    SavePolicy,
    Reduction<TestPanel>,
    SaveDecision,
    usize,
); 9] {
    [
        (
            "Manual continuous",
            SavePolicy::Manual,
            continuous_change(),
            SaveDecision::NoSave,
            0,
        ),
        (
            "Manual settled",
            SavePolicy::Manual,
            settled_change(),
            SaveDecision::NoSave,
            0,
        ),
        (
            "Manual unchanged",
            SavePolicy::Manual,
            unchanged(),
            SaveDecision::NoSave,
            0,
        ),
        (
            "OnSettle continuous",
            SavePolicy::OnSettle,
            continuous_change(),
            SaveDecision::NoSave,
            0,
        ),
        (
            "OnSettle settled wheel-as-settled",
            SavePolicy::OnSettle,
            settled_change(),
            SaveDecision::Save,
            1,
        ),
        (
            "OnSettle unchanged",
            SavePolicy::OnSettle,
            unchanged(),
            SaveDecision::NoSave,
            0,
        ),
        (
            "OnChange continuous",
            SavePolicy::OnChange,
            continuous_change(),
            SaveDecision::Save,
            1,
        ),
        (
            "OnChange settled wheel-as-settled",
            SavePolicy::OnChange,
            settled_change(),
            SaveDecision::Save,
            1,
        ),
        (
            "OnChange unchanged",
            SavePolicy::OnChange,
            unchanged(),
            SaveDecision::NoSave,
            0,
        ),
    ]
}

fn continuous_change() -> Reduction<TestPanel> {
    reduction(true, Some(ChangePhase::Continuous))
}

fn settled_change() -> Reduction<TestPanel> {
    reduction(true, Some(ChangePhase::Settled))
}

fn unchanged() -> Reduction<TestPanel> {
    reduction(false, None)
}

fn reduction(changed: bool, phase: Option<ChangePhase>) -> Reduction<TestPanel> {
    Reduction {
        changed,
        phase,
        focus_request: None,
    }
}

fn assert_reset_clear_does_not_resave() {
    for policy in [
        SavePolicy::Manual,
        SavePolicy::OnSettle,
        SavePolicy::OnChange,
    ] {
        let store = MemoryStore::seeded(v2_unknown_json());

        let decision = policy.reset_decision();
        assert_eq!(decision, SaveDecision::Clear, "{policy:?} reset decision");
        apply_save_decision(decision, &store, &defaults(), &catalog()).unwrap();

        assert_eq!(store.clear_calls(), 1, "{policy:?} clear count");
        assert!(
            store.save_calls().is_empty(),
            "{policy:?} reset should not save"
        );
        assert_eq!(
            store.load().unwrap(),
            None,
            "{policy:?} clear remains visible"
        );
    }
}
