//! Wall-clock budgets shared by the scale tests.

use std::time::Duration;

/// Fails when `took` is over `budget`. Budgets are calibrated on an M-series
/// laptop; `PANEL_KIT_BUDGETS=report` prints each timing without failing on
/// it, for builders whose speed is not known.
pub fn within(what: &str, took: Duration, budget: Duration) {
    eprintln!("{what}: {took:?} (budget {budget:?})");
    if std::env::var_os("PANEL_KIT_BUDGETS").is_some_and(|mode| mode == "report") {
        return;
    }
    assert!(
        took <= budget,
        "{what} took {took:?}, over its {budget:?} budget"
    );
}
