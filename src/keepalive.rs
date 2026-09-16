//! Retained-DOM composition for panels with imperative browser state.
//!
//! A normal [`ProjectedFrame`] contains only panels that should be painted in
//! the current frame: minimized panels are absent, and a maximized panel hides
//! its siblings. [`render_keepalive`] joins that borrowed projection with the
//! host's full [`Snapshot`] so selected, non-projected panels stay mounted
//! under `display: none`. This preserves imperative descendants such as a
//! WebGL canvas while keeping layout state and the keep-alive policy in the
//! host.
//!
//! This helper is intentionally web-only. A browser has a retained DOM whose
//! node identity can outlive a paint; the ratatui backend repaints cells and
//! has no retained element for a corresponding TUI helper to preserve.

use dioxus::prelude::*;
use panel_kit_core::frame::{PanelChromeProjection, PanelProjection, Placement, ProjectedFrame};
use panel_kit_core::panel::{PanelCatalog, PanelMeta};
use panel_kit_core::reducer::Snapshot;
use panel_kit_core::{PanelKey, PanelWin, Region};

use crate::widgets::panel::{panel_body, panel_chrome, panel_shell};

/// Render projected panels while retaining selected off-layout panel bodies.
///
/// Panels in `frame.panels` render normally. A snapshot panel absent from the
/// projected frame is also rendered when `keep_mounted` returns `true`, but a
/// keyed wrapper gives it `display: none`; the same wrapper and panel shell are
/// reused when it becomes visible again. The hidden ancestor suppresses the
/// panel's chrome without conditionally removing siblings around `body`, so an
/// imperative descendant is not remounted during minimize/restore or while a
/// different panel is maximized.
///
/// `snapshot` remains the source of the complete panel set, `frame` remains the
/// source of visible geometry, and `keep_mounted` remains host policy. This
/// function stores no state. A host-owned set can be used with a predicate such
/// as `|key| kept.contains(&key)`.
///
/// The helper paints the passive [`panel_chrome`] surface. Hosts that need
/// interactive chrome can compose eventful controls outside this convenience
/// surface; the kept-alive body itself must remain under this helper's stable
/// keyed shell.
///
/// # Example
///
/// ```no_run
/// use dioxus::prelude::*;
/// use panel_kit::keepalive::render_keepalive;
/// use panel_kit_core::frame::ProjectedFrame;
/// use panel_kit_core::panel::{PanelCatalog, PanelKey};
/// use panel_kit_core::reducer::Snapshot;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Panel {
///     Scene,
///     Inspector,
/// }
///
/// impl PanelKey for Panel {}
///
/// #[component]
/// fn BevyCanvas() -> Element {
///     rsx! { canvas { id: "bevy-canvas" } }
/// }
///
/// fn workspace_panels(
///     snapshot: &Snapshot<Panel>,
///     catalog: &PanelCatalog<Panel>,
///     frame: &ProjectedFrame<'_, Panel>,
/// ) -> Element {
///     render_keepalive(
///         snapshot,
///         catalog,
///         frame,
///         |key| key == Panel::Scene,
///         |key, maximized| match key {
///             Panel::Scene => rsx! { BevyCanvas {} },
///             Panel::Inspector => rsx! { "Inspector (maximized: {maximized})" },
///         },
///     )
/// }
/// ```
pub fn render_keepalive<K, KeepMounted, Body>(
    snapshot: &Snapshot<K>,
    catalog: &PanelCatalog<K>,
    frame: &ProjectedFrame<'_, K>,
    keep_mounted: KeepMounted,
    body: Body,
) -> Element
where
    K: PanelKey,
    KeepMounted: Fn(K) -> bool,
    Body: Fn(K, bool) -> Element,
{
    let visible = frame.panels.iter().copied().map(|panel| (panel, false));
    let hidden = snapshot
        .panels
        .iter()
        .copied()
        .enumerate()
        .filter(|(source_index, panel)| {
            keep_mounted(panel.kind)
                && frame
                    .panels
                    .iter()
                    .all(|projected| projected.source_index != *source_index)
        })
        .map(|(source_index, panel)| (hidden_projection(snapshot, source_index, panel), true));

    rsx! {
        for (panel, hidden) in visible.chain(hidden) {
            if let Some(meta) = catalog.get(panel.key) {
                {keepalive_panel(
                    panel,
                    meta,
                    hidden,
                    body(panel.key, panel.placement == Placement::Maximized),
                )}
            }
        }
    }
}

fn keepalive_panel<K: PanelKey>(
    panel: PanelProjection<K>,
    meta: &PanelMeta<K>,
    hidden: bool,
    body: Element,
) -> Element {
    let key = meta.stable_id.as_ref();
    let panel_class = format!("panel-{}", meta.slug);
    let slot_class = if hidden {
        "pk-keepalive-slot pk-keepalive-hidden"
    } else {
        "pk-keepalive-slot"
    };

    rsx! {
        div {
            key: "{key}",
            class: "{slot_class}",
            {panel_shell(panel, Some(&panel_class), rsx! {
                {panel_chrome(panel, meta, None)}
                {panel_body(body)}
            })}
        }
    }
}

fn hidden_projection<K: PanelKey>(
    snapshot: &Snapshot<K>,
    source_index: usize,
    panel: PanelWin<K>,
) -> PanelProjection<K> {
    let region = Region::new(panel.x, panel.y, panel.w, panel.h);
    PanelProjection {
        source_index,
        key: panel.kind,
        region,
        placement: Placement::Floating,
        z: panel.z,
        state: panel.state,
        focused: snapshot.focused == Some(panel.kind),
        pointer_dragging: snapshot.drag.map(|drag| drag.idx) == Some(source_index),
        tile_dragging: snapshot.tile_drag == Some(panel.kind),
        chrome: PanelChromeProjection {
            outer: region,
            body: region,
            header_hit: Region::default(),
            mode_hit: None,
            minimize_hit: None,
            maximize_hit: None,
            resize_hit: None,
        },
    }
}
