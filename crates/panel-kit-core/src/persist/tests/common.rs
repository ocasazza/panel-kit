use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::persist::{LayoutStore, RestoreContext};
use crate::reducer::{Snapshot, Viewport};
use crate::{LayoutBuilder, Mode, PanelCatalog, PanelKind, PanelMeta, Units};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(super) enum TestPanel {
    First,
    Second,
    Third,
}

impl PanelKind for TestPanel {
    fn title(self) -> &'static str {
        match self {
            Self::First => "First",
            Self::Second => "Second",
            Self::Third => "Third",
        }
    }
}

pub(super) struct MemoryStore {
    value: RefCell<Option<String>>,
    save_calls: RefCell<Vec<String>>,
    clear_calls: RefCell<usize>,
}

impl MemoryStore {
    pub(super) fn empty() -> Self {
        Self {
            value: RefCell::new(None),
            save_calls: RefCell::new(Vec::new()),
            clear_calls: RefCell::new(0),
        }
    }

    pub(super) fn seeded(json: impl Into<String>) -> Self {
        Self {
            value: RefCell::new(Some(json.into())),
            save_calls: RefCell::new(Vec::new()),
            clear_calls: RefCell::new(0),
        }
    }

    pub(super) fn saved_json(&self) -> Option<String> {
        self.value.borrow().clone()
    }

    pub(super) fn save_calls(&self) -> Vec<String> {
        self.save_calls.borrow().clone()
    }

    pub(super) fn clear_calls(&self) -> usize {
        *self.clear_calls.borrow()
    }
}

impl LayoutStore for MemoryStore {
    fn load(&self) -> Result<Option<String>, String> {
        Ok(self.saved_json())
    }

    fn save(&self, json: &str) -> Result<(), String> {
        self.save_calls.borrow_mut().push(json.to_owned());
        self.value.replace(Some(json.to_owned()));
        Ok(())
    }

    fn clear(&self) -> Result<(), String> {
        *self.clear_calls.borrow_mut() += 1;
        self.value.replace(None);
        Ok(())
    }
}

pub(super) fn catalog() -> PanelCatalog<TestPanel> {
    PanelCatalog::try_new(vec![
        meta(TestPanel::First, "First"),
        meta(TestPanel::Second, "Second"),
        meta(TestPanel::Third, "Third"),
    ])
    .expect("test catalog is unique")
}

pub(super) fn defaults() -> Snapshot<TestPanel> {
    let mut builder = LayoutBuilder::new();
    Snapshot::from_defaults(
        vec![
            builder.at(TestPanel::First, 10.0, 20.0, 100.0, 80.0),
            builder.at(TestPanel::Second, 160.0, 20.0, 120.0, 90.0),
            builder.at(TestPanel::Third, 320.0, 20.0, 140.0, 100.0),
        ],
        Mode::Floating,
        Viewport {
            width: 400.0,
            height: 300.0,
            units: Units::CssPx,
        },
    )
}

pub(super) fn restore_context() -> RestoreContext {
    RestoreContext {
        units: Units::CssPx,
        viewport: (400.0, 300.0),
    }
}

fn meta(kind: TestPanel, stable_id: &'static str) -> PanelMeta<TestPanel> {
    PanelMeta {
        key: kind,
        stable_id: stable_id.into(),
        title: kind.title().into(),
        slug: kind.title().to_ascii_lowercase().into_boxed_str(),
    }
}
