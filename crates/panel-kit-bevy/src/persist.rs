//! File-backed layout persistence for native Bevy applications.
//!
//! The store owns byte transport only; schema migration, stable-ID mapping,
//! and default merging stay in [`panel_kit_core::persist`].

use std::path::{Path, PathBuf};

use panel_kit_core::persist::{
    persist_snapshot, restore_snapshot, LayoutError, LayoutStore, RestoreContext,
};

use crate::{BevyPanelKey, BevyPanelKit};

/// [`LayoutStore`] backed by one JSON file path.
///
/// The file contents are the exact JSON encoded by `panel-kit-core`; this
/// adapter never interprets or migrates the schema.
pub struct FileLayoutStore {
    path: PathBuf,
}

impl FileLayoutStore {
    /// Create a store for one logical layout file.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The file path backing this store.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl LayoutStore for FileLayoutStore {
    fn load(&self) -> Result<Option<String>, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(json) => Ok(Some(json)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn save(&self, json: &str) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
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

/// Encode and save the kit's current snapshot and catalog.
pub fn save_layout<K: BevyPanelKey>(
    kit: &BevyPanelKit<K>,
    store: &dyn LayoutStore,
) -> Result<(), LayoutError> {
    persist_snapshot(store, &kit.snapshot, &kit.catalog)
}

/// Restore the kit's snapshot from `store`, keeping the current viewport and
/// units as the restore context.
///
/// A missing record leaves the snapshot unchanged. Unknown saved panel IDs are
/// ignored, while panels newly declared by the catalog are appended to the
/// default layout by core's restore lifecycle.
pub fn restore_layout<K: BevyPanelKey>(
    kit: &mut BevyPanelKit<K>,
    store: &dyn LayoutStore,
) -> Result<(), LayoutError> {
    let context = RestoreContext {
        units: kit.snapshot.viewport.units,
        viewport: (kit.snapshot.viewport.width, kit.snapshot.viewport.height),
    };
    let restored = restore_snapshot(store, kit.snapshot.clone(), &kit.catalog, context)?;
    kit.snapshot = restored;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_kit_core::reducer::Viewport;
    use panel_kit_core::{LayoutBuilder, Mode, PanelCatalog, Units};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    enum TestPanel {
        Workspace,
        Inspector,
    }

    impl panel_kit_core::PanelKind for TestPanel {
        fn title(self) -> &'static str {
            match self {
                Self::Workspace => "Workspace",
                Self::Inspector => "Inspector",
            }
        }
    }

    fn kit(x: f64) -> BevyPanelKit<TestPanel> {
        let mut layout = LayoutBuilder::new();
        let panels = vec![
            layout.at(TestPanel::Workspace, x, 40.0, 640.0, 480.0),
            layout.at(TestPanel::Inspector, 700.0, 40.0, 400.0, 480.0),
        ];
        let catalog = PanelCatalog::from_panel_kind_layout(&panels).unwrap();
        BevyPanelKit::new(
            catalog,
            &panels,
            Mode::Floating,
            Viewport {
                width: 1280.0,
                height: 800.0,
                units: Units::CssPx,
            },
        )
        .unwrap()
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("panel-kit-bevy-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn layout_round_trips_through_a_file() {
        let path = temp_path("roundtrip");
        let store = FileLayoutStore::new(&path);
        let source = kit(48.0);

        save_layout(&source, &store).unwrap();
        assert!(path.exists());

        let mut restored = kit(40.0);
        restore_layout(&mut restored, &store).unwrap();
        assert_eq!(restored.snapshot.panels[0].x, 48.0);
        assert_eq!(restored.snapshot.panels.len(), source.snapshot.panels.len());
        assert_eq!(restored.snapshot.preferred_mode, Mode::Floating);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_layout_leaves_defaults_unchanged() {
        let path = temp_path("missing");
        let store = FileLayoutStore::new(&path);
        let mut kit = kit(72.0);
        restore_layout(&mut kit, &store).unwrap();
        assert_eq!(kit.snapshot.panels[0].x, 72.0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn clear_removes_an_existing_record() {
        let path = temp_path("clear");
        let store = FileLayoutStore::new(&path);
        let kit = kit(96.0);
        save_layout(&kit, &store).unwrap();
        assert!(path.exists());
        store.clear().unwrap();
        assert!(!path.exists());
    }
}
