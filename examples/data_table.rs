//! Searchable, sortable data table with a column-visibility menu.
//!
//! Run with: `dx serve --example data_table --platform web`

use dioxus::prelude::*;
use panel_kit::widgets::{DataColumnSpec, DataRow, DataTable};
use panel_kit::CSS;
use panel_kit_core::widgets::data_table::{SortDir, TableQuery};

const DEMO_CSS: &str = "
body { overflow:auto !important; }
.demo { max-width:64rem; margin:0 auto; padding:2rem; }
";

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let columns = vec![
        DataColumnSpec::new("policy", "Policy").pinned(),
        DataColumnSpec::new("reward", "Reward"),
        DataColumnSpec::new("steps", "Steps"),
        DataColumnSpec::new("notes", "Notes").hidden(),
    ];
    let rows: Vec<DataRow> = [
        ("ppo-baseline", Some(412.5), 1_000_000.0, "reference run"),
        ("ppo-curriculum", Some(655.0), 2_000_000.0, "warm-started"),
        ("sac-explore", None, 500_000.0, "diverged"),
        ("ppo-lr-sweep-3", Some(530.25), 1_500_000.0, "lr 3e-4"),
    ]
    .into_iter()
    .map(|(name, reward, steps, notes)| {
        DataRow::new(name)
            .text(name)
            .num(
                reward,
                reward
                    .map(|r| format!("{r:.1}"))
                    .unwrap_or_else(|| "\u{2014}".into()),
            )
            .num(Some(steps), format!("{steps:.0}"))
            .text_class(notes, "muted")
    })
    .collect();

    rsx! {
        style { {CSS} }
        style { {DEMO_CSS} }
        main { class: "demo",
            DataTable {
                columns,
                rows,
                initial: TableQuery::sorted("reward", SortDir::Desc),
                storage_key: Some("panel_kit_example_data_table".to_string()),
                placeholder: "filter policies…",
            }
        }
    }
}
