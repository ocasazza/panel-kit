//! Browser localStorage adapter for the core layout persistence port.

use gloo_storage::{LocalStorage, Storage};
use panel_kit_core::persist::LayoutStore;

/// [`LayoutStore`] backed by one browser `localStorage` key.
///
/// The adapter stores the core-owned JSON string exactly as supplied. It does
/// not interpret, migrate, or wrap the layout schema; those responsibilities
/// remain in `panel-kit-core`.
pub struct LocalStorageLayoutStore {
    key: String,
}

impl LocalStorageLayoutStore {
    /// Create a store for one logical layout record.
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }
}

impl LayoutStore for LocalStorageLayoutStore {
    fn load(&self) -> Result<Option<String>, String> {
        LocalStorage::raw()
            .get_item(&self.key)
            .map_err(|error| format!("{error:?}"))
    }

    fn save(&self, json: &str) -> Result<(), String> {
        LocalStorage::raw()
            .set_item(&self.key, json)
            .map_err(|error| format!("{error:?}"))
    }

    fn clear(&self) -> Result<(), String> {
        LocalStorage::raw()
            .remove_item(&self.key)
            .map_err(|error| format!("{error:?}"))
    }
}
