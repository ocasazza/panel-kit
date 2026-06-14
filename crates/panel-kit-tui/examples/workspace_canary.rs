use panel_kit_core::badge::{tag_hue, BadgeKind};
use panel_kit_core::{LayoutBuilder, PanelKind, PanelWin};
use panel_kit_tui::badge::{hue_color, Badge};
use panel_kit_tui::charts::{BoxItem, FlameSpan, GaugeItem, Series};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Panel {
    Workspace,
    Badges,
    Activity,
    Capacity,
    Flame,
    Distribution,
    Nodes,
    Notes,
    Theme,
}

impl PanelKind for Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Workspace => "Workspace",
            Panel::Badges => "Badges",
            Panel::Activity => "Activity",
            Panel::Capacity => "Capacity",
            Panel::Flame => "Flame",
            Panel::Distribution => "Distribution",
            Panel::Nodes => "Nodes",
            Panel::Notes => "Notes",
            Panel::Theme => "Theme",
        }
    }
}

pub fn defaults() -> Vec<PanelWin<Panel>> {
    let mut b = LayoutBuilder::new();
    vec![
        b.at(Panel::Workspace, 1.0, 0.0, 62.0, 11.0).with_tile(1, 2),
        b.at(Panel::Activity, 1.0, 12.0, 62.0, 14.0).with_tile(1, 3),
        b.at(Panel::Flame, 1.0, 27.0, 62.0, 11.0).with_tile(1, 3),
        b.at(Panel::Notes, 1.0, 39.0, 62.0, 13.0).with_tile(1, 3),
        b.at(Panel::Badges, 65.0, 0.0, 63.0, 11.0).with_tile(2, 3),
        b.at(Panel::Nodes, 65.0, 12.0, 63.0, 9.0).with_tile(2, 2),
        b.at(Panel::Capacity, 65.0, 22.0, 63.0, 8.0).with_tile(2, 2),
        b.at(Panel::Distribution, 65.0, 31.0, 63.0, 11.0)
            .with_tile(2, 3),
        b.at(Panel::Theme, 65.0, 43.0, 63.0, 9.0).with_tile(2, 2),
    ]
}

/// Demo rows for the [`Panel::Nodes`] table: `(name, healthy, load, detail)`,
/// where `load` is `0.0..=1.0`. Exercises [`panel_kit_tui::table`],
/// [`panel_kit_tui::status`], and [`panel_kit_tui::meter`] together.
pub fn node_rows() -> [(&'static str, bool, f64, &'static str); 4] {
    [
        ("pdx-01 *", true, 0.42, "leader"),
        ("pdx-02", true, 0.18, "ready"),
        ("pdx-03", true, 0.71, "busy"),
        ("gfr-01", false, 0.0, "stale"),
    ]
}

pub fn demo_badges() -> Vec<Badge> {
    let mut tag = Badge::new(BadgeKind::Tag, "tag", "browser-tui");
    tag.override_color = Some(hue_color(tag_hue("browser-tui")));
    let mut active = Badge::new(BadgeKind::Status, "status", "canary");
    active.active = true;
    vec![
        tag,
        Badge::new(BadgeKind::Doctype, "doctype", "example"),
        Badge::new(BadgeKind::Folder, "folder", "crates/panel-kit-tui"),
        Badge::new(BadgeKind::Author, "author", "olive"),
        Badge::new(
            BadgeKind::Entity {
                ty: Some("crate".into()),
            },
            "entity",
            "panel-kit-core",
        ),
        Badge::new(
            BadgeKind::Wikilink {
                resolved: true,
                target: "TuiWorkspace".into(),
            },
            "link",
            "TuiWorkspace",
        ),
        Badge::new(
            BadgeKind::Wikilink {
                resolved: false,
                target: "missing-doc".into(),
            },
            "link",
            "missing-doc",
        ),
        Badge::new(
            BadgeKind::Url {
                href: "https://github.com/ratatui/ratzilla".into(),
                host: "github.com".into(),
            },
            "url",
            "ratzilla",
        ),
        Badge::new(BadgeKind::Date, "date", "2026-06-13"),
        active,
        Badge::new(BadgeKind::Generic, "mode", "wasm"),
    ]
}

/// Number of points kept in each rolling time-series window.
pub const WINDOW: usize = 48;

/// Deterministic pseudo-random value in `0.0..1.0` from a counter — a small
/// hashed LCG so the canary animates without pulling in a `rand` dependency
/// (and stays reproducible across the terminal and wasm backends).
fn noise(seed: u64) -> f64 {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    (x % 10_000) as f64 / 10_000.0
}

/// Rolling, per-frame metrics that drive the realtime charts. The examples
/// hold one of these in their app/demo struct and call [`Metrics::tick`]
/// each frame to push fresh samples; the chart renderers read the buffers.
pub struct Metrics {
    /// Rolling `eval/ms` samples as `(x_seconds, ms)`.
    pub eval: Vec<(f64, f64)>,
    /// Rolling `frame/ms` samples as `(x_seconds, ms)`.
    pub frame: Vec<(f64, f64)>,
    /// Per-stage duration samples for the boxplot, newest last.
    pub stage_samples: [Vec<f64>; 5],
    /// Frame counter; also the x-axis clock (0.1 s per tick).
    n: u64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// The boxplot / flame stage names, color-matched to apple-notes' stages.
pub const STAGES: [&str; 5] = ["segment", "markdown", "synthesis", "charts", "assemble"];

impl Metrics {
    /// A fresh metrics buffer pre-filled with one window of history so the
    /// charts read full immediately.
    pub fn new() -> Self {
        let mut m = Metrics {
            eval: Vec::with_capacity(WINDOW),
            frame: Vec::with_capacity(WINDOW),
            stage_samples: Default::default(),
            n: 0,
        };
        for _ in 0..WINDOW {
            m.tick();
        }
        m
    }

    /// Push one fresh sample onto every rolling buffer, dropping the oldest
    /// to keep the window bounded. Call once per rendered frame.
    pub fn tick(&mut self) {
        let n = self.n;
        let x = n as f64 * 0.1;
        // eval/ms: a slow sine drift plus jitter, 6..36 ms.
        let eval = 20.0 + 12.0 * (x * 0.6).sin() + 6.0 * (noise(n) - 0.5);
        // frame/ms: tight around the 16.6 ms budget with rare spikes.
        let spike = if noise(n ^ 0xa5) > 0.92 { 9.0 } else { 0.0 };
        let frame = 16.6 + 1.4 * (x * 1.3).sin() + spike + 1.0 * (noise(n ^ 0x5a) - 0.5);
        push_window(&mut self.eval, (x, eval.max(1.0)));
        push_window(&mut self.frame, (x, frame.max(1.0)));
        // Per-stage durations: each stage has a characteristic mean/spread.
        let means = [2.4, 5.1, 3.3, 1.8, 0.9];
        let spreads = [0.8, 2.0, 1.1, 0.6, 0.3];
        for (s, samples) in self.stage_samples.iter_mut().enumerate() {
            let v =
                means[s] + spreads[s] * (noise(n ^ (s as u64 * 0x9e3) ^ 0x1234) - 0.5) * 2.0;
            if samples.len() >= WINDOW {
                samples.remove(0);
            }
            samples.push(v.max(0.05));
        }
        self.n = n.wrapping_add(1);
    }

    /// The two time-series for the Activity chart.
    pub fn series(&self) -> [Series<'_>; 2] {
        [
            Series {
                name: "eval/ms".into(),
                points: &self.eval,
            },
            Series {
                name: "frame/ms".into(),
                points: &self.frame,
            },
        ]
    }

    /// Box-and-whisker items, one per pipeline stage.
    pub fn boxes(&self) -> Vec<BoxItem> {
        self.stage_samples
            .iter()
            .enumerate()
            .map(|(i, s)| BoxItem {
                label: STAGES[i].into(),
                samples: s.clone(),
                color: None,
            })
            .collect()
    }

    /// A live flame/icicle tree: a "session" root over a handful of "job"
    /// branches, each split into the five stages, widths driven by the most
    /// recent per-stage sample so the graph breathes per frame.
    pub fn flame(&self) -> Vec<FlameSpan> {
        let latest = |i: usize| self.stage_samples[i].last().copied().unwrap_or(1.0);
        let mut spans = Vec::new();
        let jobs = 3usize;
        let mut session_total = 0.0;
        let mut job_spans: Vec<(f64, Vec<f64>)> = Vec::new();
        for j in 0..jobs {
            let weights: Vec<f64> = (0..5)
                .map(|s| latest(s) * (0.6 + 1.2 * noise(self.n ^ (j as u64 * 0x77) ^ s as u64)))
                .collect();
            let total: f64 = weights.iter().sum();
            session_total += total;
            job_spans.push((total, weights));
        }
        spans.push(FlameSpan {
            label: "session".into(),
            depth: 0,
            value: session_total,
            color: None,
        });
        for (j, (total, weights)) in job_spans.into_iter().enumerate() {
            spans.push(FlameSpan {
                label: format!("job-{j}"),
                depth: 1,
                value: total,
                color: None,
            });
            for (s, w) in weights.into_iter().enumerate() {
                spans.push(FlameSpan {
                    label: STAGES[s].into(),
                    depth: 2,
                    value: w,
                    color: None,
                });
            }
        }
        spans
    }
}

/// Push `p` onto a rolling window, dropping the oldest point past [`WINDOW`].
fn push_window(buf: &mut Vec<(f64, f64)>, p: (f64, f64)) {
    if buf.len() >= WINDOW {
        buf.remove(0);
    }
    buf.push(p);
}

pub fn capacity_items() -> [GaugeItem; 4] {
    [
        GaugeItem {
            label: "vfs".into(),
            ratio: 0.21,
            text: "31 / 148 files".into(),
        },
        GaugeItem {
            label: "wasm".into(),
            ratio: 0.63,
            text: "6.3 MB / 10 MB".into(),
        },
        GaugeItem {
            label: "events".into(),
            ratio: 0.78,
            text: "78% queue".into(),
        },
        GaugeItem {
            label: "layout".into(),
            ratio: 0.94,
            text: "94% stress".into(),
        },
    ]
}
