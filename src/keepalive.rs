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
    let panels = keepalive_slots(snapshot, catalog, frame, keep_mounted);
    rsx! {
        for (panel, meta, hidden) in panels {
            div {
                key: "{meta.stable_id}",
                class: if hidden {
                    "pk-keepalive-slot pk-keepalive-hidden"
                } else {
                    "pk-keepalive-slot"
                },
                {panel_shell(panel, Some(&format!("panel-{}", meta.slug)), rsx! {
                    {panel_chrome(panel, meta, None)}
                    {panel_body(body(panel.key, panel.placement == Placement::Maximized))}
                })}
            }
        }
    }
}

/// As [`render_keepalive`], but the host supplies the panel's shell too.
///
/// A host with its own interactive chrome (traffic lights, header actions, a
/// resize grip) and its own eventful shell cannot use the passive
/// [`panel_chrome`] above — but it still needs the keyed, `display: none`
/// retention that keeps an imperative descendant alive. The closure receives
/// the projection to render (a hidden panel gets a [`Placement::Floating`]
/// projection with the same region), the catalog metadata, and whether the
/// panel is hidden; it returns the whole slot content, shell included. This
/// helper contributes only the stable key and the hidden class.
pub fn render_keepalive_with<K, KeepMounted, Inner>(
    snapshot: &Snapshot<K>,
    catalog: &PanelCatalog<K>,
    frame: &ProjectedFrame<'_, K>,
    keep_mounted: KeepMounted,
    inner: Inner,
) -> Element
where
    K: PanelKey,
    KeepMounted: Fn(K) -> bool,
    Inner: Fn(PanelProjection<K>, &PanelMeta<K>, bool) -> Element,
{
    let panels = keepalive_slots(snapshot, catalog, frame, keep_mounted);
    rsx! {
        for (panel, meta, hidden) in panels {
            div {
                key: "{meta.stable_id}",
                class: if hidden {
                    "pk-keepalive-slot pk-keepalive-hidden"
                } else {
                    "pk-keepalive-slot"
                },
                {inner(panel, meta, hidden)}
            }
        }
    }
}

/// The slot list both renderers walk: projected panels, then kept panels that
/// the projection dropped, each with its catalog metadata and hidden flag.
fn keepalive_slots<'a, K: PanelKey, KeepMounted: Fn(K) -> bool>(
    snapshot: &'a Snapshot<K>,
    catalog: &'a PanelCatalog<K>,
    frame: &'a ProjectedFrame<'_, K>,
    keep_mounted: KeepMounted,
) -> Vec<(PanelProjection<K>, &'a PanelMeta<K>, bool)> {
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
    let panels: Vec<_> = visible
        .chain(hidden)
        .filter_map(|(panel, hidden)| catalog.get(panel.key).map(|meta| (panel, meta, hidden)))
        .collect();
    panels
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
