//! Core persistence port and shared layout restore/save lifecycle.
//!
//! Backends own only byte transport. This module owns the persisted JSON
//! contract: V1 migration, V2 unit reconciliation, stable panel-ID mapping, and
//! default-layout merging.

#[cfg(feature = "spec-json")]
mod codec;

use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(feature = "spec-json")]
use crate::reducer::Snapshot;
use crate::reducer::{ChangePhase, Reduction};
#[cfg(feature = "spec-json")]
use crate::PanelCatalog;
use crate::{CatalogError, PanelKey, Units};

/// Persistence transport for one logical layout record.
///
/// Implementations store and retrieve raw JSON from a backend-specific record:
/// a browser `localStorage` key, a JSON file path, or another host-owned slot.
/// Core interprets the JSON shape; adapters must not duplicate schema logic.
pub trait LayoutStore {
    /// Load the previously saved layout JSON, if a record exists.
    fn load(&self) -> Result<Option<String>, String>;

    /// Replace the saved layout record with `json`.
    fn save(&self, json: &str) -> Result<(), String>;

    /// Remove the saved layout record.
    fn clear(&self) -> Result<(), String>;
}

/// Host-selected timing for persisting changed workspace snapshots.
///
/// The policy is pure data: core never hides persistence side effects, and
/// hosts apply the returned [`SaveDecision`] after each reducer result.
/// `Manual` lets the host coalesce writes; `OnSettle` writes only completed
/// changes; `OnChange` writes every changed reduction. A clear/reset operation
/// is represented as [`SaveDecision::Clear`] so backends do not hand-code a
/// second persistence table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SavePolicy {
    /// Never save automatically after reductions.
    Manual,
    /// Save exactly once for settled changes.
    OnSettle,
    /// Save once for every continuous or settled change.
    OnChange,
}

/// First-class persistence action selected by [`SavePolicy`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveDecision {
    /// Leave the current store record unchanged.
    NoSave,
    /// Replace the store record with the current snapshot.
    Save,
    /// Remove the store record without immediately saving defaults back.
    Clear,
}

impl SavePolicy {
    /// Decide the persistence action after a reducer output.
    pub fn decide<K: PanelKey>(self, reduction: &Reduction<K>) -> SaveDecision {
        if !reduction.changed {
            return SaveDecision::NoSave;
        }

        match self {
            Self::Manual => SaveDecision::NoSave,
            Self::OnSettle if reduction.phase == Some(ChangePhase::Settled) => SaveDecision::Save,
            Self::OnChange if reduction.phase.is_some() => SaveDecision::Save,
            _ => SaveDecision::NoSave,
        }
    }

    /// Decide the persistence action for an explicit reset operation.
    ///
    /// Reset semantics are independent of timing policy: all policies clear the
    /// record and none immediately re-saves the just-restored defaults.
    pub fn reset_decision(self) -> SaveDecision {
        SaveDecision::Clear
    }
}

/// Renderer-local restore facts needed to migrate and reconcile a saved layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RestoreContext {
    /// Coordinate space the restored snapshot should use.
    pub units: Units,
    /// Current viewport in [`RestoreContext::units`].
    pub viewport: (f64, f64),
}

/// Error produced while loading, decoding, reconciling, encoding, or saving a layout.
#[derive(Debug, PartialEq)]
pub enum LayoutError {
    /// The backend transport failed.
    Store(String),
    /// The panel catalog could not represent the live layout.
    Catalog(CatalogError),
    /// JSON did not match any supported saved-layout schema.
    Decode(String),
    /// A snapshot could not be encoded as JSON.
    Encode(String),
    /// A versioned record used a schema version this library does not understand.
    UnsupportedVersion(u32),
    /// A saved or target viewport was non-finite or non-positive.
    InvalidViewport {
        /// The offending viewport pair.
        viewport: (f64, f64),
    },
    /// A live panel key had no catalog stable ID, so saving would corrupt user data.
    MissingStableId,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(f, "layout store failed: {error}"),
            Self::Catalog(error) => write!(f, "layout catalog is invalid: {error}"),
            Self::Decode(error) => write!(f, "layout JSON is invalid: {error}"),
            Self::Encode(error) => write!(f, "layout JSON could not be encoded: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported layout schema version {version}")
            }
            Self::InvalidViewport { viewport } => {
                write!(
                    f,
                    "invalid saved layout viewport ({}, {})",
                    viewport.0, viewport.1
                )
            }
            Self::MissingStableId => f.write_str("panel key is missing from the panel catalog"),
        }
    }
}

impl std::error::Error for LayoutError {}

/// Load, migrate, reconcile, and merge a saved layout into a host-owned snapshot.
///
/// Missing records return `defaults` unchanged. Unknown saved panel IDs are
/// ignored, while panels newly declared by `defaults` are appended through the
/// existing merge-defaults lifecycle.
#[cfg(feature = "spec-json")]
pub fn restore_snapshot<K: PanelKey>(
    store: &dyn LayoutStore,
    defaults: Snapshot<K>,
    catalog: &PanelCatalog<K>,
    context: RestoreContext,
) -> Result<Snapshot<K>, LayoutError> {
    codec::validate_viewport(context.viewport)?;

    let Some(json) = store.load().map_err(LayoutError::Store)? else {
        return Ok(defaults);
    };

    codec::restored_snapshot(defaults, &json, catalog, context)
}

/// Encode a snapshot as the V2 layout schema and replace the store record.
#[cfg(feature = "spec-json")]
pub fn persist_snapshot<K: PanelKey>(
    store: &dyn LayoutStore,
    snapshot: &Snapshot<K>,
    catalog: &PanelCatalog<K>,
) -> Result<(), LayoutError> {
    codec::validate_viewport((snapshot.viewport.width, snapshot.viewport.height))?;

    let saved = codec::saved_layout(snapshot, catalog)?;
    let json = serde_json::to_string_pretty(&saved)
        .map_err(|error| LayoutError::Encode(error.to_string()))?;

    store.save(&json).map_err(LayoutError::Store)
}

/// Execute a persistence action selected by [`SavePolicy`].
///
/// The call site still chooses when to apply the decision; this helper keeps
/// reset clear/write behavior identical for every host.
#[cfg(feature = "spec-json")]
pub fn apply_save_decision<K: PanelKey>(
    decision: SaveDecision,
    store: &dyn LayoutStore,
    snapshot: &Snapshot<K>,
    catalog: &PanelCatalog<K>,
) -> Result<(), LayoutError> {
    match decision {
        SaveDecision::NoSave => Ok(()),
        SaveDecision::Save => persist_snapshot(store, snapshot, catalog),
        SaveDecision::Clear => store.clear().map_err(LayoutError::Store),
    }
}

#[cfg(all(test, feature = "spec-json"))]
#[path = "persist/tests.rs"]
mod tests;
