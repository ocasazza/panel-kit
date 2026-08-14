//! Named views: several switchable panel layouts inside one workspace.
//!
//! Where [`use_workspace`](crate::use_workspace) persists a single layout
//! under one fixed localStorage key, [`use_views`] manages a registry of
//! named views — each with its own persisted layout — over the same
//! [`Workspace`] machinery. The app gets one workspace to render and a
//! `Copy` handle for enumerating, switching, creating, renaming, and
//! deleting views; the view switcher UI itself is app territory (see the
//! `views` example).
//!
//! # Storage-key scheme
//!
//! Given the base key `base` (what a viewless workspace would have used):
//!
//! - the view registry ([`SavedViews`]) persists at `"{base}:views"`,
//! - each view's layout persists at `"{base}:view:{name}"`.
//!
//! See [`panel_kit_core::views`] for the shared scheme. Migration: on first
//! run with no registry, a pre-views layout stored at `base` itself is
//! *copied* into the first view's key so existing users keep their
//! arrangement; the legacy key is left untouched, so rolling back to a
//! pre-views build loses nothing.

use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};
use panel_kit_core::views::{view_layout_key, views_registry_key};
use panel_kit_core::{Mode, PanelKind, PanelWin};

use crate::{load_layout, save_layout, use_workspace_state, Workspace};

pub use panel_kit_core::views::{SavedViews, ViewError};

/// The views handle: the shared [`Workspace`] (rendering the active view)
/// plus the view registry as `Copy` signals, safe to capture in event
/// handlers. Create one per app root with [`use_views`].
///
/// Like [`Workspace`], the signal fields are public so apps can drive the
/// registry directly (read-only enumeration is always safe; for mutations
/// prefer the methods, which keep the registry and the stored layouts
/// consistent).
pub struct Views<K: PanelKind> {
    /// The workspace showing the active view — everything
    /// [`Workspace`] offers (`render`, `dock`, `root_class`, the drag
    /// handlers, …) applies unchanged.
    pub workspace: Workspace<K>,
    /// All view names, in creation order.
    pub names: Signal<Vec<String>>,
    /// The active view's name.
    pub active: Signal<String>,
    base_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
}

impl<K: PanelKind> Clone for Views<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K: PanelKind> Copy for Views<K> {}

/// Set up a workspace with named views: loads (or seeds) the view registry
/// from localStorage, restores the active view's layout, and persists both
/// whenever they change. This is a Dioxus hook with the same call-site rules
/// as [`use_workspace`](crate::use_workspace).
///
/// `base_key` namespaces every key this hook writes (see the
/// [module docs](self) for the scheme); pick one per app. `defaults`
/// produces the layout a view starts with on its first visit (see
/// [`LayoutBuilder`](crate::LayoutBuilder)). `initial_views` seeds the
/// registry when none exists yet — e.g. `&["User", "Sessions"]`; empty or
/// all-blank input falls back to a single `"Default"` view.
///
/// First-run migration: if no registry exists but a pre-views single layout
/// is stored at `base_key` itself, it is copied into the first view's key
/// (never moved — the legacy key stays for older builds).
pub fn use_views<K: PanelKind>(
    base_key: &'static str,
    defaults: fn() -> Vec<PanelWin<K>>,
    initial_views: &[&str],
) -> Views<K> {
    let registry = load_registry(base_key, initial_views);
    migrate_legacy_layout(base_key, &registry);
    let saved = load_layout(&view_layout_key(base_key, &registry.active), &defaults());
    let workspace = use_workspace_state(saved, defaults);
    let names = use_signal(|| registry.views.clone());
    let active = use_signal(|| registry.active.clone());

    // Layout persistence, keyed by the active view. Same settle semantics as
    // use_workspace (not mid-drag); signal writes batch, so a view switch
    // lands panels+active together and this effect writes the just-restored
    // layout back under its own key — a no-op round-trip.
    use_effect(move || {
        let ps = workspace.panels.read().clone();
        let md = *workspace.mode.read();
        let view = active.read().clone();
        if workspace.drag.read().is_none() && workspace.tile_drag.read().is_none() {
            save_layout(&view_layout_key(base_key, &view), &ps, md);
        }
    });

    // Registry persistence (view list + active view).
    use_effect(move || {
        let reg = SavedViews { views: names.read().clone(), active: active.read().clone() };
        let _ = LocalStorage::set(views_registry_key(base_key), reg);
    });

    Views { workspace, names, active, base_key, defaults }
}

/// Load the registry, or seed + persist it from `initial_views` on first
/// run. A stored registry is sanitized before use (duplicates collapsed,
/// dangling active re-pointed) so a hand-edited or stale value can't wedge
/// the workspace.
fn load_registry(base_key: &'static str, initial_views: &[&str]) -> SavedViews {
    match LocalStorage::get::<SavedViews>(views_registry_key(base_key)) {
        Ok(stored) => stored.sanitize(),
        Err(_) => SavedViews::new(initial_views),
    }
}

/// First-run migration: with a freshly seeded registry, copy the pre-views
/// single layout at `base` into the first view's key. No-clobber on both
/// ends — the copy is skipped when the destination already exists, and the
/// legacy key is left in place for pre-views builds.
fn migrate_legacy_layout(base_key: &'static str, registry: &SavedViews) {
    let storage = LocalStorage::raw();
    let seeded = registry
        .views
        .first()
        .map(|first| view_layout_key(base_key, first));
    let Some(dest) = seeded else { return };
    let Ok(Some(raw)) = storage.get_item(base_key) else { return };
    if storage.get_item(&dest).ok().flatten().is_none() {
        let _ = storage.set_item(&dest, &raw);
    }
}

impl<K: PanelKind> Views<K> {
    fn registry(&self) -> SavedViews {
        SavedViews { views: self.names.read().clone(), active: self.active.read().clone() }
    }

    fn write_registry(&self, reg: SavedViews) {
        let mut names = self.names;
        names.set(reg.views);
        let mut active = self.active;
        active.set(reg.active);
    }

    /// Replace the workspace's panel/mode signals with the named view's
    /// stored layout — or the app defaults on the view's first visit — and
    /// drop any in-flight drag and scroll offset.
    fn restore_into_workspace(&self, name: &str) {
        let defaults = (self.defaults)();
        let (panels, mode) = load_layout(&view_layout_key(self.base_key, name), &defaults)
            .unwrap_or((defaults, Mode::Floating));
        let mut ws = self.workspace;
        ws.panels.set(panels);
        ws.mode.set(mode);
        ws.drag.set(None);
        ws.tile_drag.set(None);
        ws.ws_scroll.set(0.0);
    }

    /// Switch the active view: the outgoing view's layout is flushed to its
    /// key synchronously (the persist effect would get there too, but only
    /// after the panel signal is overwritten), then the incoming view's
    /// layout is restored. Switching to the current view is a no-op;
    /// switching to an unknown view returns [`ViewError::NotFound`].
    pub fn switch(&self, name: &str) -> Result<(), ViewError> {
        let mut reg = self.registry();
        if reg.active == name {
            return Ok(());
        }
        let outgoing = reg.active.clone();
        reg.activate(name)?;
        let ps = self.workspace.panels.read().clone();
        let md = *self.workspace.mode.read();
        save_layout(&view_layout_key(self.base_key, &outgoing), &ps, md);
        self.restore_into_workspace(name);
        self.write_registry(reg);
        Ok(())
    }

    /// Add a view without switching to it. Its layout materializes from the
    /// app defaults on the first [`switch`](Views::switch) to it.
    pub fn create(&self, name: &str) -> Result<(), ViewError> {
        let mut reg = self.registry();
        reg.add(name)?;
        self.write_registry(reg);
        Ok(())
    }

    /// Rename a view, moving its persisted layout to the new key (raw string
    /// move, so the layout follows even if it no longer deserializes against
    /// the current panel set). No-clobber: a layout already stored under the
    /// new key is never overwritten.
    pub fn rename(&self, old: &str, new: &str) -> Result<(), ViewError> {
        let new_name = new.trim().to_string();
        let mut reg = self.registry();
        reg.rename(old, &new_name)?;
        let storage = LocalStorage::raw();
        let old_key = view_layout_key(self.base_key, old);
        let new_key = view_layout_key(self.base_key, &new_name);
        if old_key != new_key {
            if let Ok(Some(raw)) = storage.get_item(&old_key) {
                if storage.get_item(&new_key).ok().flatten().is_none() {
                    let _ = storage.set_item(&new_key, &raw);
                }
                let _ = storage.remove_item(&old_key);
            }
        }
        self.write_registry(reg);
        Ok(())
    }

    /// Delete a view and its persisted layout. Deleting the active view
    /// activates the neighbor that slid into its slot (see
    /// [`SavedViews::remove`]); the last remaining view refuses to die
    /// ([`ViewError::LastView`]).
    pub fn delete(&self, name: &str) -> Result<(), ViewError> {
        let mut reg = self.registry();
        reg.remove(name)?;
        LocalStorage::delete(view_layout_key(self.base_key, name));
        let now_active = reg.active.clone();
        if *self.active.read() != now_active {
            self.restore_into_workspace(&now_active);
        }
        self.write_registry(reg);
        Ok(())
    }

    /// The localStorage key a view's layout persists under. Apps can use
    /// this for "reset this view" buttons (`LocalStorage::delete` + reload)
    /// or devtools introspection.
    pub fn layout_key(&self, name: &str) -> String {
        view_layout_key(self.base_key, name)
    }
}
