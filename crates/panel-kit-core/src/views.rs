//! Named views: several panel layouts inside one workspace.
//!
//! A view is a named [`SavedLayout`](crate::SavedLayout) slot. This module
//! holds the renderer-neutral pieces — the persisted registry shape, the
//! storage-key scheme, and the registry state transitions — so every shell
//! (web localStorage, TUI JSON file, anything else) stores and switches
//! views the same way. The shell owns the actual storage I/O.
//!
//! # Storage-key scheme
//!
//! Given the app's base persistence key `base` (the same key a viewless
//! workspace would use, e.g. `"myapp_layout"`):
//!
//! - the registry ([`SavedViews`]) lives at [`views_registry_key`]
//!   (`"{base}:views"`),
//! - each view's [`SavedLayout`](crate::SavedLayout) lives at
//!   [`view_layout_key`] (`"{base}:view:{name}"`).
//!
//! The two namespaces cannot collide (`":views"` vs `":view:"` infixes), and
//! the legacy single-layout key `base` itself is never written by a
//! views-aware shell: on first run its value is *copied* into the initial
//! view's key (see the shell's migration step) and left in place, so a
//! downgrade back to a pre-views build finds the user's layout untouched.

use serde::{Deserialize, Serialize};

/// The storage key carrying the [`SavedViews`] registry for a workspace
/// whose base persistence key is `base`.
pub fn views_registry_key(base: &str) -> String {
    format!("{base}:views")
}

/// The storage key carrying one view's [`SavedLayout`](crate::SavedLayout).
/// `view` must be a name already normalized into a [`SavedViews`] registry
/// (trimmed, non-empty, unique — enforced by the registry's mutators).
pub fn view_layout_key(base: &str, view: &str) -> String {
    format!("{base}:view:{view}")
}

/// Why a view-registry mutation was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// The name is empty once trimmed.
    EmptyName,
    /// A view with that name already exists.
    Duplicate,
    /// No view with that name exists.
    NotFound,
    /// The last remaining view cannot be deleted.
    LastView,
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewError::EmptyName => f.write_str("view name is empty"),
            ViewError::Duplicate => f.write_str("a view with that name already exists"),
            ViewError::NotFound => f.write_str("no view with that name exists"),
            ViewError::LastView => f.write_str("cannot delete the last view"),
        }
    }
}

impl std::error::Error for ViewError {}

/// The persisted view registry: every view's name plus which one is active.
///
/// Invariants (established by [`SavedViews::new`]/[`SavedViews::sanitize`]
/// and preserved by the mutators): `views` is non-empty, trimmed, and
/// duplicate-free, and `active` is always a member of `views`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedViews {
    /// View names in creation order.
    pub views: Vec<String>,
    /// The active view's name.
    pub active: String,
}

impl SavedViews {
    /// Seed a fresh registry from the app's initial view names: trimmed,
    /// empties dropped, duplicates collapsed (first occurrence wins). Falls
    /// back to a single `"Default"` view when nothing usable is supplied.
    /// The first view starts active.
    pub fn new(initial: &[&str]) -> Self {
        let mut views: Vec<String> = Vec::new();
        for name in initial {
            let name = name.trim();
            if !name.is_empty() && !views.iter().any(|v| v == name) {
                views.push(name.to_string());
            }
        }
        if views.is_empty() {
            views.push("Default".to_string());
        }
        let active = views[0].clone();
        Self { views, active }
    }

    /// Repair a registry loaded from storage: normalize the view list like
    /// [`SavedViews::new`] does, then re-point `active` at the first view
    /// when it names a view that no longer exists. Never returns an empty
    /// registry.
    pub fn sanitize(self) -> Self {
        let mut fixed = Self::new(&self.views.iter().map(String::as_str).collect::<Vec<_>>());
        if self.views.contains(&self.active) && fixed.views.contains(&self.active) {
            fixed.active = self.active;
        }
        fixed
    }

    /// Add a view. The registry change is all that happens — the shell
    /// materializes the view's layout (from the app's defaults) on first
    /// activation, so no layout write is needed here.
    pub fn add(&mut self, name: &str) -> Result<(), ViewError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ViewError::EmptyName);
        }
        if self.views.iter().any(|v| v == name) {
            return Err(ViewError::Duplicate);
        }
        self.views.push(name.to_string());
        Ok(())
    }

    /// Make `name` the active view. The shell is responsible for persisting
    /// the outgoing view's layout and restoring the incoming one.
    pub fn activate(&mut self, name: &str) -> Result<(), ViewError> {
        if self.views.iter().any(|v| v == name) {
            self.active = name.to_string();
            Ok(())
        } else {
            Err(ViewError::NotFound)
        }
    }

    /// Rename `old` to `new`, keeping its slot in the view order (and its
    /// active status when it was active). The shell should move the stored
    /// layout to the new view key so the user's arrangement follows the
    /// name.
    pub fn rename(&mut self, old: &str, new: &str) -> Result<(), ViewError> {
        let new = new.trim();
        if new.is_empty() {
            return Err(ViewError::EmptyName);
        }
        if old != new && self.views.iter().any(|v| v == new) {
            return Err(ViewError::Duplicate);
        }
        let Some(slot) = self.views.iter_mut().find(|v| *v == old) else {
            return Err(ViewError::NotFound);
        };
        *slot = new.to_string();
        if self.active == old {
            self.active = new.to_string();
        }
        Ok(())
    }

    /// Remove a view. When it was active, the neighbor that slid into its
    /// slot (or the new last view) becomes active. The last remaining view
    /// refuses to die. The shell should delete the view's stored layout.
    pub fn remove(&mut self, name: &str) -> Result<(), ViewError> {
        let Some(pos) = self.views.iter().position(|v| v == name) else {
            return Err(ViewError::NotFound);
        };
        if self.views.len() == 1 {
            return Err(ViewError::LastView);
        }
        self.views.remove(pos);
        if self.active == name {
            let idx = pos.min(self.views.len() - 1);
            self.active = self.views[idx].clone();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_live_in_disjoint_namespaces() {
        assert_eq!(views_registry_key("app_layout"), "app_layout:views");
        assert_eq!(view_layout_key("app_layout", "Main"), "app_layout:view:Main");
        assert_ne!(views_registry_key("app_layout"), view_layout_key("app_layout", "views"));
    }

    #[test]
    fn new_normalizes_and_defaults() {
        let reg = SavedViews::new(&["  User ", "", "Sessions", "User"]);
        assert_eq!(reg.views, vec!["User".to_string(), "Sessions".to_string()]);
        assert_eq!(reg.active, "User");

        let reg = SavedViews::new(&[]);
        assert_eq!(reg.views, vec!["Default".to_string()]);
        assert_eq!(reg.active, "Default");
    }

    #[test]
    fn sanitize_repairs_loaded_registries() {
        // Active pointing at a gone view falls back to the first view.
        let reg = SavedViews {
            views: vec!["A".to_string(), " B ".to_string(), "A".to_string()],
            active: "gone".to_string(),
        }
        .sanitize();
        assert_eq!(reg.views, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(reg.active, "A");

        // A valid active view survives.
        let reg = SavedViews {
            views: vec!["A".to_string(), "B".to_string()],
            active: "B".to_string(),
        }
        .sanitize();
        assert_eq!(reg.active, "B");

        // An entirely empty stored registry still yields one view.
        let reg = SavedViews { views: vec![], active: String::new() }.sanitize();
        assert_eq!(reg.views, vec!["Default".to_string()]);
    }

    #[test]
    fn add_rejects_empty_and_duplicate_names() {
        let mut reg = SavedViews::new(&["A"]);
        assert_eq!(reg.add("  "), Err(ViewError::EmptyName));
        assert_eq!(reg.add("A"), Err(ViewError::Duplicate));
        assert_eq!(reg.add(" B "), Ok(()));
        assert_eq!(reg.views, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn activate_requires_membership() {
        let mut reg = SavedViews::new(&["A", "B"]);
        assert_eq!(reg.activate("C"), Err(ViewError::NotFound));
        assert_eq!(reg.activate("B"), Ok(()));
        assert_eq!(reg.active, "B");
    }

    #[test]
    fn rename_keeps_slot_and_active() {
        let mut reg = SavedViews::new(&["A", "B"]);
        assert_eq!(reg.rename("C", "D"), Err(ViewError::NotFound));
        assert_eq!(reg.rename("A", "B"), Err(ViewError::Duplicate));
        assert_eq!(reg.rename("A", ""), Err(ViewError::EmptyName));
        assert_eq!(reg.rename("A", " Main "), Ok(()));
        assert_eq!(reg.views, vec!["Main".to_string(), "B".to_string()]);
        assert_eq!(reg.active, "Main");
    }

    #[test]
    fn remove_repoints_active_and_guards_the_last_view() {
        let mut reg = SavedViews::new(&["A", "B", "C"]);
        assert_eq!(reg.remove("nope"), Err(ViewError::NotFound));

        // Removing an inactive view leaves active alone.
        assert_eq!(reg.remove("B"), Ok(()));
        assert_eq!(reg.active, "A");
        assert_eq!(reg.views, vec!["A".to_string(), "C".to_string()]);

        // Removing the active view activates the neighbor in its slot.
        assert_eq!(reg.remove("A"), Ok(()));
        assert_eq!(reg.active, "C");

        // The last view refuses to die.
        assert_eq!(reg.remove("C"), Err(ViewError::LastView));
        assert_eq!(reg.views, vec!["C".to_string()]);
    }
}
