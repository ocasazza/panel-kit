use super::*;

use crate::{LayoutBuilder, PanelKind};
use serde::{Deserialize, Serialize};

/// Mirrors the TUI canary `Panel` enum
/// (`crates/panel-kit-tui/examples/workspace_canary.rs`): serde's default
/// variant names are the exact strings V1/V2 saves already contain, so
/// they are the stable IDs a catalog over this enum must use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum CanaryPanel {
    Workspace,
    Badges,
    Notes,
}

impl PanelKind for CanaryPanel {
    fn title(self) -> &'static str {
        match self {
            Self::Workspace => "Workspace",
            Self::Badges => "Badges",
            Self::Notes => "Notes",
        }
    }
}

fn spec_meta(key: SpecPanelId, stable_id: &str) -> PanelMeta<SpecPanelId> {
    PanelMeta {
        key,
        stable_id: stable_id.into(),
        title: stable_id.into(),
        slug: stable_id.to_lowercase().into(),
    }
}

#[test]
fn panel_catalog_resolves_spec_ids_once_and_rejects_duplicate_stable_ids() {
    // Resolution interns authored panels once, in document order, into
    // `SpecPanelId` indices; the catalog then resolves by index or stable
    // ID without re-interning anything.
    let catalog = PanelCatalog::try_new(vec![
        spec_meta(SpecPanelId(0), "Workspace"),
        spec_meta(SpecPanelId(1), "Badges"),
        spec_meta(SpecPanelId(2), "Notes"),
    ])
    .expect("distinct stable IDs form a valid catalog");

    // Index -> meta: ordered interning is stable and positional.
    let badges = catalog.get(SpecPanelId(1)).unwrap();
    assert_eq!(&*badges.stable_id, "Badges");
    assert_eq!(&*badges.title, "Badges");

    // Stable ID -> meta: the interned index comes back, not a new one.
    assert_eq!(catalog.get_by_stable_id("Badges").unwrap().key, SpecPanelId(1));

    // Key -> stable ID agrees with the entry lookup.
    assert_eq!(catalog.stable_id(SpecPanelId(2)), Some("Notes"));

    // Unknown identities resolve to None instead of panicking.
    assert!(catalog.get(SpecPanelId(3)).is_none());
    assert!(catalog.get_by_stable_id("Flame").is_none());
    assert_eq!(catalog.stable_id(SpecPanelId(3)), None);

    // A duplicate stable ID is ambiguous persistence state and must be
    // rejected at construction, naming the offender.
    let error = PanelCatalog::try_new(vec![
        spec_meta(SpecPanelId(0), "Workspace"),
        spec_meta(SpecPanelId(1), "Badges"),
        spec_meta(SpecPanelId(2), "Badges"),
    ])
    .expect_err("duplicate stable IDs must not form a catalog");

    assert_eq!(error, CatalogError::DuplicateStableId("Badges".into()));

    // An empty catalog is constructible and resolves nothing.
    let empty = PanelCatalog::<SpecPanelId>::try_new(Vec::new())
        .expect("no entries cannot conflict");
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert!(empty.get_by_stable_id("Workspace").is_none());
}

#[test]
fn panel_catalog_rejects_duplicate_keys_before_lookup_maps_can_diverge() {
    let error = PanelCatalog::try_new(vec![
        spec_meta(SpecPanelId(0), "Workspace"),
        spec_meta(SpecPanelId(0), "Badges"),
    ])
    .expect_err("duplicate keys must not form a catalog");

    assert_eq!(error, CatalogError::DuplicateKey);
}

#[test]
fn panel_kind_enums_satisfy_panel_key_with_serde_variant_stable_ids() {
    // Bounded by PanelKey only: passing a closed PanelKind enum through
    // this function proves the blanket impl keeps every existing enum
    // working without source changes.
    fn stable_id_of<K: PanelKey>(catalog: &PanelCatalog<K>, key: K) -> &str {
        catalog.stable_id(key).expect("key is in the catalog")
    }

    let catalog = PanelCatalog::try_new(vec![
        PanelMeta {
            key: CanaryPanel::Workspace,
            stable_id: "Workspace".into(),
            title: "Workspace".into(),
            slug: "workspace".into(),
        },
        PanelMeta {
            key: CanaryPanel::Badges,
            stable_id: "Badges".into(),
            title: "Badges".into(),
            slug: "badges".into(),
        },
        PanelMeta {
            key: CanaryPanel::Notes,
            stable_id: "Notes".into(),
            title: "Notes".into(),
            slug: "notes".into(),
        },
    ])
    .expect("distinct stable IDs form a valid catalog");

    // Each stable ID must equal the enum's serde variant name, so previously
    // saved layouts (which persist those names) survive the catalog round-trip
    // unchanged.
    for variant in [
        CanaryPanel::Workspace,
        CanaryPanel::Badges,
        CanaryPanel::Notes,
    ] {
        let json = serde_json::to_string(&variant).unwrap();
        let variant_name = json.trim_matches('"');

        assert_eq!(stable_id_of(&catalog, variant), variant_name);
        assert_eq!(catalog.get_by_stable_id(variant_name).unwrap().key, variant);
    }
}

#[test]
fn panel_catalog_builds_stable_ids_from_panel_kind_layout() {
    let mut layout = LayoutBuilder::new();
    let panels = vec![
        layout.at(CanaryPanel::Workspace, 1.0, 2.0, 30.0, 12.0),
        layout.at(CanaryPanel::Badges, 4.0, 5.0, 20.0, 8.0),
        layout.at(CanaryPanel::Notes, 6.0, 7.0, 25.0, 10.0),
    ];

    let catalog =
        PanelCatalog::from_panel_kind_layout(&panels).expect("panel kind layout forms a catalog");

    for panel in &panels {
        let meta = catalog.get(panel.kind).expect("panel is cataloged");
        let variant = serde_json::to_string(&panel.kind).unwrap();
        let stable_id = variant.trim_matches('"');

        assert_eq!(&*meta.stable_id, stable_id);
        assert_eq!(&*meta.title, panel.kind.title());
        assert_eq!(catalog.get_by_stable_id(stable_id).unwrap().key, panel.kind);
    }
}
