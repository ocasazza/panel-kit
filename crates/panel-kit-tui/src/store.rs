//! JSON-file adapter for the core layout persistence port.

use std::path::PathBuf;

use panel_kit_core::persist::LayoutStore;

/// [`LayoutStore`] backed by one JSON file path.
///
/// The file contents are the exact JSON string encoded by `panel-kit-core`.
/// The TUI backend does not interpret or migrate the schema.
pub struct JsonFileLayoutStore {
    path: PathBuf,
}

impl JsonFileLayoutStore {
    /// Create a store for one logical layout file.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl LayoutStore for JsonFileLayoutStore {
    fn load(&self) -> Result<Option<String>, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(json) => Ok(Some(json)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn save(&self, json: &str) -> Result<(), String> {
        std::fs::write(&self.path, json).map_err(|error| error.to_string())
    }

    fn clear(&self) -> Result<(), String> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}
