//! First-class Bevy binding for panel-kit.
//!
//! This crate owns no panel state and no transition logic of its own. It
//! exposes the renderer-agnostic `panel-kit-core` workspace as a Bevy
//! resource and defines the component vocabulary the rendering and input
//! systems attach to workspace entities. State transitions flow through
//! core — [`reducer::reduce`](panel_kit_core::reducer::reduce), the drag and
//! command free functions, and the frame projection — so a Bevy workspace
//! behaves exactly like the Dioxus and ratatui renderers over the same
//! state machine.
//!
//! The [`BevyPanelKit`] resource holds everything later rendering and input
//! systems need against core's contracts:
//!
//! - [`Snapshot`](panel_kit_core::reducer::Snapshot) — the host-owned
//!   workspace state,
//! - [`PanelCatalog`] — panel keys mapped to the stable string IDs layouts
//!   persist,
//! - the [`SurfaceProfile`] and unit metrics ([`Clamp`], [`CommandStep`],
//!   [`TileMetrics`], [`SnapPolicy`]) that fill a
//!   [`ReduceContext`](panel_kit_core::reducer::ReduceContext).
//!
//! Rendering lives in the [`render`] module: [`render_workspace`] projects
//! the current snapshot through core's frame projection and reconciles a
//! stable set of `bevy_ui` entities — panel nodes ([`PanelFrame`]), chrome
//! headers ([`PanelChrome`]), body slots ([`PanelBody`]), and the minimized
//! dock ([`MinimizedDock`], [`DockChip`]). Register it with
//! [`PanelUiPlugin`] or add the system directly.
//!
//! Construct it once per Bevy app:
//!
//! ```
//! use panel_kit_bevy::BevyPanelKit;
//! use panel_kit_core::reducer::Viewport;
//! use panel_kit_core::{LayoutBuilder, Mode, PanelCatalog, Units};
//!
//! # #[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
//! # enum Demo { A }
//! # impl panel_kit_core::PanelKind for Demo {
//! #     fn title(self) -> &'static str { "Demo" }
//! # }
//! let mut layout = LayoutBuilder::new();
//! let panels = vec![layout.at(Demo::A, 40.0, 40.0, 640.0, 480.0)];
//! let catalog = PanelCatalog::from_panel_kind_layout(&panels).unwrap();
//!
//! let kit = BevyPanelKit::new(
//!     catalog,
//!     &panels,
//!     Mode::Floating,
//!     Viewport { width: 1280.0, height: 800.0, units: Units::CssPx },
//! )
//! .unwrap();
//!
//! let mut world = bevy::prelude::World::new();
//! world.insert_resource(kit);
//! assert_eq!(world.resource::<BevyPanelKit<Demo>>().snapshot.panels.len(), 1);
//! ```

#![warn(missing_docs)]

use bevy::prelude::{Component, Resource};
use panel_kit_core::reducer::{reduce, Snapshot, Viewport, WorkspaceEvent};
use panel_kit_core::{
    Clamp, CommandStep, Mode, PanelCatalog, PanelCommand, PanelWin, SnapPolicy,
    SurfaceCapabilities, SurfaceProfile, TileMetrics, WEB_COMPACT_MAX, WEB_TABLET_MAX,
};

pub use panel_kit_core::{PanelKey, PanelKind};

pub mod fonts;
pub mod input;
pub mod persist;
pub mod render;
pub mod theme;
pub mod widgets;

pub use input::{
    keyboard_input_system, pointer_input_system, resize_input_system, PanelInputPlugin,
};
pub use persist::{restore_layout, save_layout, FileLayoutStore};
pub use render::{
    render_workspace, DockChip, PanelBackdrop, PanelBodyHandles, PanelFrame, PanelUiPlugin,
};
pub use theme::BevyTheme;

/// The panel-identity bound the Bevy binding accepts.
///
/// Bevy requires every resource and component to be `Send + Sync + 'static`,
/// so this adds exactly those supertraits to core's [`PanelKey`]. It does not
/// restate [`PanelKind`]: every app panel enum satisfies the bound through
/// core's blanket impls, as do spec-resolved
/// [`SpecPanelId`](panel_kit_core::panel::SpecPanelId)s.
pub trait BevyPanelKey: PanelKey + Send + Sync {}

impl<K: PanelKey + Send + Sync> BevyPanelKey for K {}

/// Marks the UI entity that hosts one panel's body content.
///
/// Applications place their own UI inside this entity. The body owns its
/// content interaction: input translation must not treat pointer events
/// over a [`PanelBody`] as workspace-chrome input.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PanelBody;

/// Marks one panel's header chrome: the title and the traffic lights.
///
/// Dragging, resizing, and traffic-light input are chrome gestures; input
/// translation routes them through core's pointer events instead of letting
/// the body content consume them.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PanelChrome;

/// Marks the dock region that holds one chip per minimized panel.
///
/// Restoring a minimized panel is a dock gesture: clicking (or otherwise
/// activating) a chip inside a [`MinimizedDock`] entity translates to a
/// dock restore against core.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct MinimizedDock;

/// Marks the dock's host-owned trailing slot, right-aligned after the chips
/// (web `dock(.., extras)`). Hosts spawn workspace policy controls such as
/// snap toggles and layout reset into it; the dock never owns their state.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DockExtras;

/// Error returned when a [`BevyPanelKit`] cannot be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionError {
    /// The initial panel set references a panel the catalog does not
    /// declare. Persisting that panel would silently drop it from saved
    /// layouts (no stable ID maps it back), so construction refuses instead
    /// of deferring the failure to the first save.
    UnknownPanel,
}

impl core::fmt::Display for ConstructionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownPanel => {
                f.write_str("initial panel set references a panel not declared in the catalog")
            }
        }
    }
}

impl std::error::Error for ConstructionError {}

/// Bevy resource owning the panel workspace.
///
/// Fields are public data, matching core's renderer-neutral style: hosts
/// drive workspace state through core's reducer and free functions (e.g.
/// [`reducer::reduce`](panel_kit_core::reducer::reduce),
/// [`apply_drag`](panel_kit_core::apply_drag)), not through setters here.
#[derive(Resource)]
pub struct BevyPanelKit<K: BevyPanelKey> {
    /// Host-owned workspace state: panels with geometry and window state
    /// (in tiling order), the preferred mode, the viewport the geometry is
    /// expressed against, focus, in-flight gestures, and the workspace
    /// scroll offset.
    pub snapshot: Snapshot<K>,
    /// Panel metadata: keys mapped to stable persisted IDs, titles, and
    /// slugs.
    pub catalog: PanelCatalog<K>,
    /// The surface the workspace is rendered on: the surface class derived
    /// from the viewport width at construction, with the input capabilities
    /// of a native desktop window. Input systems refresh it on resize.
    pub surface: SurfaceProfile,
    /// Viewport-clamp parameters in renderer units.
    pub clamp: Clamp,
    /// Keyboard geometry step sizes in renderer units.
    pub command_step: CommandStep,
    /// Tiling span metrics in renderer units.
    pub tile: TileMetrics,
    /// Session snapping and tiling-reorder policy.
    pub snap: SnapPolicy,
    /// How window resizes treat floating geometry; hosts set it from the
    /// spec's `surface.resize_policy`, as the web host does.
    pub resize_policy: panel_kit_core::reducer::ResizePolicy,
}

impl<K: BevyPanelKey> BevyPanelKit<K> {
    /// Construct the workspace resource from an app-provided catalog, an
    /// initial panel set (typically built with
    /// [`LayoutBuilder`](panel_kit_core::LayoutBuilder)), a preferred
    /// [`Mode`], and the viewport the stored geometry is expressed against.
    ///
    /// Every initial panel must be declared in `catalog`. Renderer-unit
    /// defaults are seeded for the surface and the unit metrics: logical
    /// pixel units ([`Units::CssPx`], the coordinate space of a Bevy
    /// window) with the web-shell [`Clamp::WEB`], [`CommandStep::WEB`],
    /// [`TileMetrics::WEB`], and [`SnapPolicy::WEB`] magnitudes. Adjust the
    /// public fields after construction when an app needs different knobs.
    pub fn new(
        catalog: PanelCatalog<K>,
        initial: &[PanelWin<K>],
        preferred_mode: Mode,
        viewport: Viewport,
    ) -> Result<Self, ConstructionError> {
        if initial
            .iter()
            .any(|panel| catalog.get(panel.kind).is_none())
        {
            return Err(ConstructionError::UnknownPanel);
        }

        Ok(Self {
            snapshot: Snapshot::from_defaults(initial.to_vec(), preferred_mode, viewport),
            catalog,
            surface: SurfaceProfile::from_logical_width(
                viewport.width,
                WEB_COMPACT_MAX,
                WEB_TABLET_MAX,
                SurfaceCapabilities {
                    coarse_pointer: false,
                    hover: true,
                    keyboard: true,
                },
            ),
            clamp: Clamp::WEB,
            command_step: CommandStep::WEB,
            tile: TileMetrics::WEB,
            snap: SnapPolicy::WEB,
            resize_policy: panel_kit_core::reducer::ResizePolicy::PreserveIntent,
        })
    }

    /// Reduce one renderer-neutral event through core and return its report.
    ///
    /// This is the app-facing write path: every state transition goes through
    /// core's [`reduce`] and no Bevy-specific state is introduced.
    pub fn apply_workspace_event(
        &mut self,
        event: WorkspaceEvent<K>,
    ) -> panel_kit_core::reducer::Reduction<K> {
        let (surface, clamp, command_step, tile, snap) = (
            self.surface,
            self.clamp,
            self.command_step,
            self.tile,
            self.snap,
        );
        reduce(
            &mut self.snapshot,
            event,
            panel_kit_core::reducer::ReduceContext {
                surface,
                clamp: &clamp,
                command_step,
                tile: &tile,
                snap,
            },
        )
    }

    /// Send one panel-scoped or workspace-scoped window-management command.
    /// Returns whether the command changed the snapshot.
    pub fn send_command(&mut self, target: Option<K>, command: PanelCommand) -> bool {
        self.apply_workspace_event(WorkspaceEvent::Command { target, command })
            .changed
    }

    /// Replace the preferred layout mode without mutating panel geometry.
    pub fn set_preferred_mode(&mut self, mode: Mode) {
        self.snapshot.preferred_mode = mode;
    }

    /// Replace the panel catalog. Refuses if any existing snapshot panel is
    /// not declared by the new catalog, so persistence always has a stable ID.
    pub fn replace_catalog(&mut self, catalog: PanelCatalog<K>) -> Result<(), ConstructionError> {
        if self
            .snapshot
            .panels
            .iter()
            .any(|panel| catalog.get(panel.kind).is_none())
        {
            return Err(ConstructionError::UnknownPanel);
        }
        self.catalog = catalog;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::World;
    use panel_kit_core::{LayoutBuilder, Units};

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    enum TestPanel {
        Workspace,
        Inspector,
        Output,
        Timeline,
    }

    impl PanelKind for TestPanel {
        fn title(self) -> &'static str {
            match self {
                Self::Workspace => "Workspace",
                Self::Inspector => "Inspector",
                Self::Output => "Output",
                Self::Timeline => "Timeline",
            }
        }
    }

    fn test_panels() -> Vec<PanelWin<TestPanel>> {
        let mut layout = LayoutBuilder::new();
        vec![
            layout.at(TestPanel::Workspace, 40.0, 40.0, 640.0, 480.0),
            layout.at(TestPanel::Inspector, 700.0, 40.0, 400.0, 480.0),
            layout.at(TestPanel::Output, 40.0, 540.0, 640.0, 200.0),
            layout.at(TestPanel::Timeline, 700.0, 540.0, 400.0, 200.0),
        ]
    }

    fn test_viewport() -> Viewport {
        Viewport {
            width: 1280.0,
            height: 800.0,
            units: Units::CssPx,
        }
    }

    #[test]
    fn initializes_as_a_bevy_resource() {
        let panels = test_panels();
        let kit = BevyPanelKit::new(
            PanelCatalog::from_panel_kind_layout(&panels).expect("test panels serialize"),
            &panels,
            Mode::Floating,
            test_viewport(),
        )
        .expect("catalog covers every panel");

        let mut world = World::new();
        world.insert_resource(kit);

        let kit = world.resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(kit.snapshot.panels.len(), 4);
        assert_eq!(kit.snapshot.panels[0].kind, TestPanel::Workspace);
    }

    #[test]
    fn default_construction_produces_expected_panel_keys() {
        let panels = test_panels();
        let catalog = PanelCatalog::from_panel_kind_layout(&panels).expect("test panels serialize");
        let kit = BevyPanelKit::new(catalog, &panels, Mode::Floating, test_viewport())
            .expect("catalog covers every panel");

        let keys: Vec<TestPanel> = kit.snapshot.panels.iter().map(|p| p.kind).collect();
        assert_eq!(
            keys,
            vec![
                TestPanel::Workspace,
                TestPanel::Inspector,
                TestPanel::Output,
                TestPanel::Timeline
            ]
        );
        assert_eq!(kit.catalog.len(), 4);
        for kind in keys {
            assert_eq!(kit.catalog.stable_id(kind), Some(kind.title()));
            assert!(kit.catalog.get(kind).is_some());
        }
        assert!(!kit.snapshot.panels.is_empty());
    }

    #[test]
    fn mode_and_viewport_are_preserved() {
        let panels = test_panels();
        let viewport = Viewport {
            width: 1024.0,
            height: 768.0,
            units: Units::CssPx,
        };
        let kit = BevyPanelKit::new(
            PanelCatalog::from_panel_kind_layout(&panels).expect("test panels serialize"),
            &panels,
            Mode::Tiling,
            viewport,
        )
        .expect("catalog covers every panel");

        assert_eq!(kit.snapshot.preferred_mode, Mode::Tiling);
        assert_eq!(kit.snapshot.viewport, viewport);

        let mut world = World::new();
        world.insert_resource(kit);
        let kit = world.resource::<BevyPanelKit<TestPanel>>();
        assert_eq!(kit.snapshot.preferred_mode, Mode::Tiling);
        assert_eq!(kit.snapshot.viewport, viewport);
    }

    #[test]
    fn unknown_panels_are_rejected() {
        let panels = test_panels();
        // Catalog from a subset: Timeline is missing from it.
        let subset: Vec<PanelWin<TestPanel>> = panels
            .iter()
            .copied()
            .filter(|p| p.kind != TestPanel::Timeline)
            .collect();
        let catalog = PanelCatalog::from_panel_kind_layout(&subset).expect("test panels serialize");

        let result = BevyPanelKit::new(catalog, &panels, Mode::Floating, test_viewport());
        assert!(matches!(result, Err(ConstructionError::UnknownPanel)));
    }

    #[test]
    fn markers_are_queryable_components() {
        let mut world = World::new();
        let tagged = world.spawn((PanelBody, PanelChrome, MinimizedDock)).id();
        let plain = world.spawn_empty().id();

        let mut bodies = world.query::<&PanelBody>();
        let mut dock = world.query::<&MinimizedDock>();

        assert!(bodies.get(&world, tagged).is_ok());
        assert!(dock.get(&world, tagged).is_ok());
        assert!(bodies.get(&world, plain).is_err());
    }

    #[test]
    fn spec_panel_ids_satisfy_the_bevy_bound() {
        fn assert_bevy_key<K: BevyPanelKey>() {}
        assert_bevy_key::<panel_kit_core::panel::SpecPanelId>();
        assert_bevy_key::<TestPanel>();
    }

    #[test]
    fn app_api_applies_commands_and_modes_through_core() {
        let panels = test_panels();
        let mut kit = BevyPanelKit::new(
            PanelCatalog::from_panel_kind_layout(&panels).unwrap(),
            &panels,
            Mode::Floating,
            test_viewport(),
        )
        .unwrap();

        assert!(kit.send_command(Some(TestPanel::Workspace), PanelCommand::Minimize));
        assert_eq!(
            kit.snapshot.panels[0].state,
            panel_kit_core::WinState::Minimized
        );
        assert!(!kit.send_command(None, PanelCommand::Minimize));

        kit.set_preferred_mode(Mode::Tiling);
        assert_eq!(kit.snapshot.preferred_mode, Mode::Tiling);
    }

    #[test]
    fn catalog_replacement_rejects_unknown_snapshot_panels() {
        let panels = test_panels();
        let mut kit = BevyPanelKit::new(
            PanelCatalog::from_panel_kind_layout(&panels).unwrap(),
            &panels,
            Mode::Floating,
            test_viewport(),
        )
        .unwrap();

        let subset: Vec<PanelWin<TestPanel>> = panels
            .iter()
            .copied()
            .filter(|p| p.kind != TestPanel::Timeline)
            .collect();
        let catalog = PanelCatalog::from_panel_kind_layout(&subset).unwrap();
        assert_eq!(
            kit.replace_catalog(catalog),
            Err(ConstructionError::UnknownPanel)
        );
        assert_eq!(kit.catalog.len(), 4);
    }
}
