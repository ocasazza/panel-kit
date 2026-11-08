//! Pair every web panel with its Bevy twin and compare geometry, pixels,
//! traffic lights, text runs, and the data-table toolbar.

use std::collections::{BTreeMap, HashMap, HashSet};

use image::RgbImage;

use crate::dump::{inside, overlaps, Rect, BevyNode, Rgba, WebNode};

/// Geometry tolerance, logical px.
pub const GEOMETRY_TOLERANCE: f64 = 1.0;
/// Colour tolerance for anti-aliased samples, CIE76 ΔE.
pub const DELTA_E_TOLERANCE: f64 = 2.0;
/// Device pixels per logical px in both captures.
pub const SCALE: f64 = 2.0;
/// Panel surface fill shared by both backends' dark theme; Bevy frames are
/// found by it.
const PANEL_SURFACE: [f64; 3] = [13.0, 13.0, 13.0];

/// One compared property.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub panel: String,
    pub part: String,
    pub property: String,
    pub web: String,
    pub bevy: String,
    pub delta: String,
    pub ok: bool,
}

/// Every row of one comparison, in panel order.
#[derive(Debug, Default)]
pub struct Comparison {
    pub rows: Vec<Row>,
    /// Panel titles in comparison order.
    pub panels: Vec<String>,
}

impl Comparison {
    /// Rows outside tolerance.
    pub fn failures(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(|row| !row.ok)
    }

    /// Pass and fail counts per panel, in comparison order.
    pub fn summary(&self) -> Vec<(String, usize, usize)> {
        self.panels
            .iter()
            .map(|panel| {
                let rows = self.rows.iter().filter(|row| &row.panel == panel);
                let (pass, fail) = rows.fold((0, 0), |(p, f), row| {
                    if row.ok {
                        (p + 1, f)
                    } else {
                        (p, f + 1)
                    }
                });
                (panel.clone(), pass, fail)
            })
            .collect()
    }
}

/// One capture: its PNG (logical px × [`SCALE`]) and node dump.
pub struct Capture<'a, N> {
    pub image: &'a RgbImage,
    pub nodes: &'a [N],
}

/// A web panel paired with its Bevy frame.
pub struct Pair<'a> {
    pub title: String,
    pub web: &'a WebNode,
    pub bevy: &'a BevyNode,
}

/// Compare the two captures. Returns the rows and the panel pairs found.
pub fn compare<'a>(
    web: Capture<'a, WebNode>,
    bevy: Capture<'a, BevyNode>,
) -> (Comparison, Vec<Pair<'a>>) {
    let mut out = Rows::default();
    let mut pairs = Vec::new();
    let tree = BevyTree::new(bevy.nodes);
    let mut panels: Vec<&WebNode> = web
        .nodes
        .iter()
        .filter(|node| node.cls.split_whitespace().next() == Some("panel"))
        .collect();
    panels.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let frames: Vec<&BevyNode> = bevy
        .nodes
        .iter()
        .filter(|node| {
            node.border_w >= 0.99
                && node.bg.is_some_and(|bg| bg[..3] == PANEL_SURFACE)
                && node.w > 100.0
                && node.h > 60.0
        })
        .collect();

    for &wp in &panels {
        let title = web
            .nodes
            .iter()
            .find(|node| node.cls.contains("panel-title") && inside(*node, wp, 0.5))
            .map_or_else(|| "?".to_owned(), |node| node.text.clone());
        out.panel(&title);
        let distance = |b: &&BevyNode| (b.x - wp.x).abs() + (b.y - wp.y).abs();
        let Some(bp) = frames
            .iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            .filter(|b| distance(b) <= 40.0)
            .copied()
        else {
            out.same("frame", "rect", "present", "missing", false);
            continue;
        };
        compare_panel(&mut out, &web, &bevy, &tree, &title, wp, bp, &panels);
        pairs.push(Pair {
            title,
            web: wp,
            bevy: bp,
        });
    }
    (out.finish(), pairs)
}

fn compare_panel(
    out: &mut Rows,
    web: &Capture<'_, WebNode>,
    bevy: &Capture<'_, BevyNode>,
    tree: &BevyTree<'_>,
    title: &str,
    wp: &WebNode,
    bp: &BevyNode,
    panels: &[&WebNode],
) {
    for (key, w, b) in [("x", wp.x, bp.x), ("y", wp.y, bp.y), ("w", wp.w, bp.w), ("h", wp.h, bp.h)] {
        out.geo("frame", key, Some(w), Some(b));
    }

    let sample = |dx: f64, dy: f64| {
        (
            pixel(web.image, wp.x + dx, wp.y + dy),
            pixel(bevy.image, bp.x + dx, bp.y + dy),
        )
    };
    let samples = [
        ("frame", "border colour (left edge, mid)", 0.25, wp.h / 2.0, true),
        ("frame", "border colour (top edge, title zone)", 80.0, 0.25, true),
        ("frame", "corner pixel (radius 0)", 0.25, 0.25, true),
        ("frame", "surface (header, right of title)", wp.w - 30.0, 11.0, false),
        ("grip", "triangle ink (corner -3,-3)", wp.w - 3.0, wp.h - 3.0, true),
        ("grip", "open half (corner -13,-13)", wp.w - 13.0, wp.h - 13.0, true),
    ];
    // A sample within a pixel of a neighbouring panel measures that panel or
    // the sub-pixel overlap rounding, not this panel's chrome.
    let occluded = |dx: f64, dy: f64| {
        let point = (wp.x + dx - 1.0, wp.y + dy - 1.0, 2.0, 2.0);
        panels.iter().any(|other| !std::ptr::eq(*other, wp) && overlaps(&point, *other))
    };
    for (part, property, dx, dy, exact) in samples {
        if occluded(dx, dy) {
            continue;
        }
        let (w, b) = sample(dx, dy);
        out.colour(part, property, Some(w), Some(b), exact);
    }

    // Traffic lights: 12 px squares in the header, left to right.
    let mut web_lights: Vec<&WebNode> = web
        .nodes
        .iter()
        .filter(|n| n.cls.split_whitespace().next() == Some("light") && inside(*n, wp, 0.5))
        .collect();
    web_lights.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut bevy_lights: Vec<&BevyNode> = bevy
        .nodes
        .iter()
        .filter(|n| {
            n.bg.is_some()
                && (n.w - 12.0).abs() < 0.6
                && (n.h - 12.0).abs() < 0.6
                && inside(*n, bp, 0.5)
                && n.y - bp.y < 22.0
        })
        .collect();
    bevy_lights.sort_by(|a, b| a.x.total_cmp(&b.x));
    for (index, w) in web_lights.iter().enumerate() {
        let part = format!("light {index}");
        let b = bevy_lights.get(index);
        out.geo(&part, "x", Some(w.x - wp.x), b.map(|b| b.x - bp.x));
        out.geo(&part, "y", Some(w.y - wp.y), b.map(|b| b.y - bp.y));
        out.geo(&part, "w", Some(w.w), b.map(|b| b.w));
        out.geo(&part, "h", Some(w.h), b.map(|b| b.h));
        out.colour(
            &part,
            "fill",
            Some(pixel(web.image, w.x + 6.0, w.y + 6.0)),
            b.map(|b| pixel(bevy.image, b.x + 6.0, b.y + 6.0)),
            true,
        );
    }

    compare_text(out, web.nodes, tree, title, wp, bp);
    compare_toolbar(out, web.nodes, bevy.nodes, title, wp, bp);
    compare_columns(out, web.nodes, bevy.nodes, title, wp, bp);
    let ribbon = compare_ribbon(out, web, bevy, tree, title, wp, bp);
    compare_window(out, web, bevy, tree, title, wp, bp, &ribbon);
    compare_ring(out, web, bevy, tree, bp, &ribbon);
    compare_graph(out, web, bevy, title, wp, bp);
}

/// A text run normalised across backends.
#[derive(Clone, Debug)]
struct Run {
    text: String,
    x: f64,
    y: f64,
    w: f64,
    line_height: f64,
    /// Lines a wrapped web run spans.
    lines: f64,
    font: f64,
    bold: bool,
    color: Option<Rgba>,
}

fn normalise(text: &str) -> String {
    text.replace('\u{a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn web_runs(nodes: &[WebNode], title: &str, wp: &WebNode) -> Vec<Run> {
    let menus: Vec<&WebNode> = nodes
        .iter()
        .filter(|n| n.cls.starts_with("pk-dt-menu-body"))
        .collect();
    let mut out = Vec::new();
    for node in nodes {
        let Some([tx, ty, tw, th]) = node.text_rect else {
            continue;
        };
        if node.text.is_empty() || node.panel.as_deref() != Some(title) {
            continue;
        }
        let multiline = node.text.contains('\n');
        if (!multiline && !inside(&(tx, ty, tw, th), wp, 1.0))
            || node.cls.contains("dock")
            || node.cls.contains("snap")
            || node.cls.contains("pk-dt-menu")
            || menus.iter().any(|menu| !std::ptr::eq(*menu, node) && inside(node, *menu, 0.5))
        {
            continue;
        }
        let text = if node.tt == "uppercase" {
            node.text.to_uppercase()
        } else {
            node.text.clone()
        };
        let line_height = node
            .lh
            .strip_suffix("px")
            .and_then(|value| value.parse().ok())
            .unwrap_or(node.font * 1.2);
        let bold = node.weight.parse::<u32>().is_ok_and(|weight| weight >= 600);
        if multiline {
            // A `pre` block: one run per line on the line-height pitch.
            for (index, line) in node.text.split('\n').enumerate() {
                let top =
                    ty - (line_height - node.font * 15.0 / 13.0) / 2.0 + index as f64 * line_height;
                if line.trim().is_empty() || top + line_height > wp.y + wp.h {
                    continue;
                }
                out.push(Run {
                    text: normalise(line),
                    x: tx,
                    y: top,
                    w: line.chars().count() as f64 * node.font * 1233.0 / 2048.0,
                    line_height,
                    lines: 1.0,
                    font: node.font,
                    bold,
                    color: node.color,
                });
            }
            continue;
        }
        // A wrapped run's rect spans (lines - 1) line boxes plus one glyph box.
        let lines = ((th - node.font * 1.15) / line_height + 1.0).round_ties_even().max(1.0);
        let glyph_h = th - (lines - 1.0) * line_height;
        out.push(Run {
            text: normalise(&text),
            x: tx,
            y: ty - (line_height - glyph_h) / 2.0,
            w: tw,
            line_height,
            lines,
            font: node.font,
            bold,
            color: node.color,
        });
    }
    out
}

struct BevyTree<'a> {
    nodes: &'a [BevyNode],
    by_id: HashMap<u64, &'a BevyNode>,
    children: BTreeMap<u64, Vec<&'a BevyNode>>,
}

impl<'a> BevyTree<'a> {
    fn new(nodes: &'a [BevyNode]) -> Self {
        let by_id = nodes.iter().map(|node| (node.id, node)).collect();
        let mut children: BTreeMap<u64, Vec<&BevyNode>> = BTreeMap::new();
        for node in nodes {
            if let Some(parent) = node.parent {
                children.entry(parent).or_default().push(node);
            }
        }
        Self {
            nodes,
            by_id,
            children,
        }
    }

    /// Whether `node` is `root` or laid out inside its subtree. Clipped
    /// content keeps its scene place, so ancestry, not the frame rect, decides.
    fn descends_from(&self, node: &BevyNode, root: u64) -> bool {
        let mut at = Some(node.id);
        while let Some(id) = at {
            if id == root {
                return true;
            }
            at = self.by_id.get(&id).and_then(|n| n.parent);
        }
        false
    }

    /// Text runs inside `bp`. A row of single-glyph text children is one
    /// tracked run (letter-spacing is painted glyph by glyph).
    fn runs(&self, bp: &BevyNode) -> Vec<Run> {
        let mut out = Vec::new();
        let mut grouped = HashSet::new();
        for (parent, children) in &self.children {
            let mut glyphs: Vec<&BevyNode> = children
                .iter()
                .copied()
                .filter(|c| !c.text.is_empty() && c.font.is_some())
                .collect();
            if glyphs.len() < 2
                || glyphs.len() != children.len()
                || glyphs.iter().any(|c| c.text.chars().count() != 1)
            {
                continue;
            }
            glyphs.sort_by(|a, b| a.x.total_cmp(&b.x));
            let (first, last) = (glyphs[0], glyphs[glyphs.len() - 1]);
            let (x0, x1) = (first.x, last.x + last.w);
            if inside(&(x0, first.y, x1 - x0, first.h), bp, 1.0) {
                // CSS letter-spacing trails the last glyph; the row's right
                // padding carries it.
                let w = self.by_id.get(parent).map_or(x1 - x0, |row| row.w);
                out.push(Run {
                    text: normalise(&glyphs.iter().map(|c| c.text.as_str()).collect::<String>()),
                    x: x0,
                    y: first.y,
                    w,
                    line_height: first.h,
                    lines: 1.0,
                    font: first.font.unwrap_or_default(),
                    bold: first.bold,
                    color: first.color,
                });
            }
            grouped.extend(glyphs.iter().map(|c| c.id));
        }
        for node in self.nodes {
            let Some(font) = node.font else {
                continue;
            };
            if grouped.contains(&node.id) || node.text.is_empty() || !inside(node, bp, 1.0) {
                continue;
            }
            let lines = (node.text.matches('\n').count() + 1) as f64;
            out.push(Run {
                text: normalise(&node.text),
                x: node.x,
                y: node.y,
                w: node.tw.unwrap_or(node.w),
                line_height: node.h / lines,
                lines,
                font,
                bold: node.bold,
                color: node.color,
            });
        }
        out
    }
}

fn compare_text(
    out: &mut Rows,
    web: &[WebNode],
    tree: &BevyTree<'_>,
    title: &str,
    wp: &WebNode,
    bp: &BevyNode,
) {
    let bevy_runs = tree.runs(bp);
    let mut used = HashSet::new();
    for w in web_runs(web, title, wp) {
        let label = format!("text {:?}", w.text.chars().take(28).collect::<String>());
        let matching = |exact: bool| -> Vec<usize> {
            bevy_runs
                .iter()
                .enumerate()
                .filter(|(index, b)| {
                    !used.contains(index)
                        && if exact {
                            b.text == w.text
                        } else {
                            b.text.to_lowercase() == w.text.to_lowercase()
                        }
                })
                .map(|(index, _)| index)
                .collect()
        };
        let mut candidates = matching(true);
        if candidates.is_empty() {
            candidates = matching(false);
        }
        let offset = |b: &Run| ((b.x - bp.x) - (w.x - wp.x)).abs() + ((b.y - bp.y) - (w.y - wp.y)).abs();
        let Some(index) = candidates
            .into_iter()
            .min_by(|a, b| offset(&bevy_runs[*a]).total_cmp(&offset(&bevy_runs[*b])))
        else {
            out.same(&label, "present", "yes", "no", false);
            continue;
        };
        used.insert(index);
        let b = &bevy_runs[index];
        out.geo(&label, "x", Some(w.x - wp.x), Some(b.x - bp.x));
        out.geo(&label, "line top", Some(w.y - wp.y), Some(b.y - bp.y));
        out.geo(&label, "width", Some(w.w), Some(b.w));
        // A Bevy text node wraps in one box; compare its per-line height.
        let bevy_line = if b.lines > 1.0 { b.line_height } else { b.line_height / w.lines };
        out.geo(&label, "line height", Some(w.line_height), Some(bevy_line));
        out.geo(&label, "font size", Some(w.font), Some(b.font));
        out.same(&label, "bold", &w.bold.to_string(), &b.bold.to_string(), w.bold == b.bold);
        out.colour(&label, "colour", w.color.map(rgb), b.color.map(rgb), true);
    }
}

/// The data-table toolbar: bordered controls inside `.pk-dt-bar`, paired
/// left to right, and the row count's right edge.
fn compare_toolbar(
    out: &mut Rows,
    web: &[WebNode],
    bevy: &[BevyNode],
    title: &str,
    wp: &WebNode,
    bp: &BevyNode,
) {
    let in_panel = |n: &&WebNode| n.panel.as_deref() == Some(title);
    let Some(bar) = web
        .iter()
        .filter(in_panel)
        .find(|n| n.cls.split_whitespace().any(|c| c == "pk-dt-bar"))
    else {
        return;
    };
    let mut web_controls: Vec<&WebNode> = web
        .iter()
        .filter(in_panel)
        .filter(|n| {
            n.border_w >= 1.0
                && matches!(n.tag.as_str(), "input" | "button" | "summary" | "select")
                && bar.y - 0.5 <= n.y
                && n.y + n.h <= bar.y + bar.h + 0.5
        })
        .collect();
    web_controls.sort_by(|a, b| a.x.total_cmp(&b.x));
    let top = bp.y + (bar.y - wp.y);
    let mut bevy_controls: Vec<&BevyNode> = bevy
        .iter()
        .filter(|n| {
            n.border_w >= 0.99
                && inside(*n, bp, 0.5)
                && top - 0.5 <= n.y
                && n.y + n.h <= top + bar.h + 0.5
        })
        .collect();
    bevy_controls.sort_by(|a, b| a.x.total_cmp(&b.x));
    out.same(
        "toolbar",
        "control count",
        &web_controls.len().to_string(),
        &bevy_controls.len().to_string(),
        web_controls.len() == bevy_controls.len(),
    );
    for (index, w) in web_controls.iter().enumerate() {
        let name = match w.tag.as_str() {
            "input" => "quick filter".to_owned(),
            "select" => "group select".to_owned(),
            _ if !w.text.trim().is_empty() => w.text.trim().to_owned(),
            other => other.to_owned(),
        };
        let part = format!("toolbar {name}");
        let b = bevy_controls.get(index);
        out.geo(&part, "x", Some(w.x - wp.x), b.map(|b| b.x - bp.x));
        out.geo(&part, "y", Some(w.y - wp.y), b.map(|b| b.y - bp.y));
        out.geo(&part, "w", Some(w.w), b.map(|b| b.w));
        out.geo(&part, "h", Some(w.h), b.map(|b| b.h));
        out.colour(&part, "border colour", w.border.map(rgb), b.and_then(|b| b.border).map(rgb), true);
        out.colour(&part, "background", w.bg.map(rgb), b.and_then(|b| b.bg).map(rgb), true);
    }
    if let Some(tally) = web.iter().filter(in_panel).find(|n| n.cls.contains("pk-dt-tally")) {
        let bevy_edge = bevy
            .iter()
            .filter(|n| !n.text.is_empty() && inside(*n, bp, 0.5) && top <= n.y && n.y <= top + bar.h)
            .map(|n| n.x + n.w - bp.x)
            .max_by(f64::total_cmp);
        out.geo("toolbar row count", "right edge", Some(tally.x + tally.w - wp.x), bevy_edge);
    }
}

/// The `--color-border` edge ink of the dark theme both backends draw graph
/// edges in.
const EDGE_INK: [u8; 3] = [95, 95, 95];
/// Largest ΔE from [`EDGE_INK`] at which the closest pixel still counts as
/// edge ink: a 1-unit stroke is only a pixel or two wide, so its centre pixel
/// is blended with the host fill.
const EDGE_INK_TOLERANCE: f64 = 20.0;

/// The graph painter: the host box, each node's disc (geometry and fill), and
/// ink at each edge's midpoint. Skipped when the panel has no web graph.
fn compare_graph(
    out: &mut Rows,
    web: &Capture<'_, WebNode>,
    bevy: &Capture<'_, BevyNode>,
    title: &str,
    wp: &WebNode,
    bp: &BevyNode,
) {
    let in_panel = |n: &&WebNode| n.panel.as_deref() == Some(title);
    let has = |n: &&WebNode, class: &str| n.cls.split_whitespace().any(|c| c == class);
    let Some(host) = web.nodes.iter().filter(in_panel).find(|n| has(n, "pk-graph")) else {
        return;
    };
    let rel = |n: &WebNode| (n.x - wp.x, n.y - wp.y);
    let in_bevy = |logical: (f64, f64)| (bp.x + logical.0, bp.y + logical.1);

    // Host: the box filled with the theme background.
    let (hx, hy) = rel(host);
    let bevy_host = bevy
        .nodes
        .iter()
        .filter(|n| inside(*n, bp, 0.5) && n.bg.is_some_and(|bg| bg[3] > 0.99))
        .filter(|n| (n.w - host.w).abs() <= GEOMETRY_TOLERANCE && (n.h - host.h).abs() <= GEOMETRY_TOLERANCE)
        .min_by(|a, b| {
            let near = |n: &&BevyNode| (n.x - bp.x - hx).abs() + (n.y - bp.y - hy).abs();
            near(a).total_cmp(&near(b))
        });
    out.geo("graph host", "x", Some(hx), bevy_host.map(|b| b.x - bp.x));
    out.geo("graph host", "y", Some(hy), bevy_host.map(|b| b.y - bp.y));
    out.geo("graph host", "w", Some(host.w), bevy_host.map(|b| b.w));
    out.geo("graph host", "h", Some(host.h), bevy_host.map(|b| b.h));
    let corner = (hx + 1.0, hy + 1.0);
    out.colour(
        "graph host",
        "fill (top-left corner)",
        Some(pixel(web.image, wp.x + corner.0, wp.y + corner.1)),
        Some(pixel(bevy.image, in_bevy(corner).0, in_bevy(corner).1)),
        true,
    );

    let mut nodes: Vec<&WebNode> = web
        .nodes
        .iter()
        .filter(in_panel)
        .filter(|n| has(n, "pk-graph-node"))
        .collect();
    nodes.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    for (index, w) in nodes.iter().enumerate() {
        let part = format!("graph node {index}");
        let centre = (w.x + w.w / 2.0, w.y + w.h / 2.0);
        // The Bevy disc: a filled square of the same size, nearest the web centre.
        let disc = bevy
            .nodes
            .iter()
            .filter(|n| {
                (n.w - n.h).abs() <= GEOMETRY_TOLERANCE
                    && (n.w - w.w).abs() <= GEOMETRY_TOLERANCE
                    && n.bg.is_some_and(|bg| bg[3] > 0.99)
            })
            .min_by(|a, b| {
                let far = |n: &&BevyNode| {
                    (n.x + n.w / 2.0 - bp.x - (centre.0 - wp.x)).hypot(n.y + n.h / 2.0 - bp.y - (centre.1 - wp.y))
                };
                far(a).total_cmp(&far(b))
            });
        out.geo(&part, "x", Some(w.x - wp.x), disc.map(|b| b.x - bp.x));
        out.geo(&part, "y", Some(w.y - wp.y), disc.map(|b| b.y - bp.y));
        out.geo(&part, "w", Some(w.w), disc.map(|b| b.w));
        out.geo(&part, "h", Some(w.h), disc.map(|b| b.h));
        // A node centred on the plot edge is half clipped by the host, so its
        // centre is sampled a pixel inside the host box.
        let sample = (
            centre.0.clamp(host.x + 1.0, host.x + host.w - 1.0),
            centre.1.clamp(host.y + 1.0, host.y + host.h - 1.0),
        );
        let bevy_sample = in_bevy((sample.0 - wp.x, sample.1 - wp.y));
        out.colour(
            &part,
            "fill (sample inside host)",
            Some(pixel(web.image, sample.0, sample.1)),
            Some(pixel(bevy.image, bevy_sample.0, bevy_sample.1)),
            true,
        );
    }

    let edges = web.nodes.iter().filter(in_panel).filter(|n| has(n, "pk-graph-edge"));
    for (index, w) in edges.enumerate() {
        let part = format!("graph edge {index}");
        let middle = (w.x + w.w / 2.0, w.y + w.h / 2.0);
        let web_ink = closest_to(web.image, middle, EDGE_INK);
        let bevy_middle = in_bevy((middle.0 - wp.x, middle.1 - wp.y));
        let bevy_ink = closest_to(bevy.image, bevy_middle, EDGE_INK);
        let present = |ink: [u8; 3]| delta_e(ink, EDGE_INK) < EDGE_INK_TOLERANCE;
        out.same(
            &part,
            "ink at midpoint",
            &hex(web_ink),
            &hex(bevy_ink),
            present(web_ink) && present(bevy_ink),
        );
    }
}

/// The pixel within a logical pixel of `at` closest to `target`.
fn closest_to(image: &RgbImage, at: (f64, f64), target: [u8; 3]) -> [u8; 3] {
    let (cx, cy) = ((at.0 * SCALE).round() as i64, (at.1 * SCALE).round() as i64);
    let reach = SCALE as i64;
    (cy - reach..=cy + reach)
        .flat_map(|y| (cx - reach..=cx + reach).map(move |x| (x, y)))
        .filter(|&(x, y)| x >= 0 && y >= 0 && (x as u32) < image.width() && (y as u32) < image.height())
        .map(|(x, y)| image.get_pixel(x as u32, y as u32).0)
        .min_by(|a, b| delta_e(*a, target).total_cmp(&delta_e(*b, target)))
        .unwrap_or([0, 0, 0])
}

/// The data-table header: each column's cell and its resize zone (a 7px
/// `col-resize` strip overhanging the cell's right edge by 3px), paired left
/// to right.
fn compare_columns(
    out: &mut Rows,
    web: &[WebNode],
    bevy: &[BevyNode],
    title: &str,
    wp: &WebNode,
    bp: &BevyNode,
) {
    let in_panel = |n: &&WebNode| n.panel.as_deref() == Some(title);
    let has_class = |n: &WebNode, name: &str| n.cls.split_whitespace().any(|c| c == name);
    let mut heads: Vec<&WebNode> = web
        .iter()
        .filter(in_panel)
        .filter(|n| n.tag == "th" && has_class(n, "pk-dt-th"))
        .collect();
    let Some(top) = heads.iter().map(|n| n.y).min_by(f64::total_cmp) else {
        return;
    };
    heads.retain(|n| (n.y - top).abs() < 0.5);
    heads.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut zones: Vec<&WebNode> = web
        .iter()
        .filter(in_panel)
        .filter(|n| has_class(n, "pk-dt-resize"))
        .collect();
    zones.sort_by(|a, b| a.x.total_cmp(&b.x));
    let bevy_top = bp.y + (top - wp.y);
    // Header cells: the filled boxes on the header row. The tags gutter's
    // cell has no resize handle, so cells, not handles, pair with the `th`s.
    let mut cells: Vec<&BevyNode> = bevy
        .iter()
        .filter(|n| {
            n.bg.is_some_and(|bg| bg[3] > 0.5)
                && (n.y - bevy_top).abs() < 0.5
                && (20.0..32.0).contains(&n.h)
                && bp.x <= n.x
                && n.x + n.w <= bp.x + bp.w
        })
        .collect();
    cells.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut handles: Vec<&BevyNode> = bevy
        .iter()
        .filter(|n| {
            (n.w - 7.0).abs() < 0.1
                && (n.y - bevy_top).abs() < 0.5
                && bp.x <= n.x
                && n.x <= bp.x + bp.w
                && n.text.is_empty()
                && n.bg.is_none_or(|bg| bg[3] == 0.0)
        })
        .collect();
    handles.sort_by(|a, b| a.x.total_cmp(&b.x));
    out.same(
        "header",
        "column count",
        &heads.len().to_string(),
        &cells.len().to_string(),
        heads.len() == cells.len(),
    );
    out.same(
        "header",
        "resize zone count",
        &zones.len().to_string(),
        &handles.len().to_string(),
        zones.len() == handles.len(),
    );
    for (index, head) in heads.iter().enumerate() {
        let cell = cells.get(index);
        let part = format!("header column {}", index + 1);
        out.geo(&part, "x", Some(head.x - wp.x), cell.map(|b| b.x - bp.x));
        out.geo(&part, "y", Some(head.y - wp.y), cell.map(|b| b.y - bp.y));
        out.geo(&part, "w", Some(head.w), cell.map(|b| b.w));
        out.geo(&part, "h", Some(head.h), cell.map(|b| b.h));
    }
    for (index, zone) in zones.iter().enumerate() {
        let handle = handles.get(index);
        let part = format!("header resize zone {}", index + 1);
        out.geo(&part, "x", Some(zone.x - wp.x), handle.map(|b| b.x - bp.x));
        out.geo(&part, "y", Some(zone.y - wp.y), handle.map(|b| b.y - bp.y));
        out.geo(&part, "w", Some(zone.w), handle.map(|b| b.w));
        out.geo(&part, "h", Some(zone.h), handle.map(|b| b.h));
    }
}

/// The `--tag-N` fills both backends paint ribbon segments with, `--tag-0`
/// first.
const TAG_FILLS: [[u8; 3]; 8] = [
    [0x33, 0x6e, 0x09],
    [0x82, 0x5d, 0xf2],
    [0xc9, 0x50, 0xf1],
    [0x36, 0xae, 0x0e],
    [0xf4, 0x7d, 0xd7],
    [0xf7, 0x9d, 0xca],
    [0xc4, 0xc4, 0xfa],
    [0xe6, 0xd0, 0xfb],
];

/// The row tag ribbon: every visible segment's rect and painted fill, and the
/// `+N` tail. The web paints `.pk-dt-tag` buttons; Bevy paints 13px nodes in a
/// tag fill. Returns the grouped segment rows for [`compare_window`]; `None`
/// when the panel has no web table.
fn compare_ribbon<'a>(
    out: &mut Rows,
    web: &Capture<'a, WebNode>,
    bevy: &Capture<'a, BevyNode>,
    tree: &BevyTree<'_>,
    title: &str,
    wp: &WebNode,
    bp: &BevyNode,
) -> Option<(
    Vec<(usize, usize, &'a WebNode)>,
    Vec<(usize, usize, &'a BevyNode)>,
)> {
    let has_table = web.nodes.iter().any(|n| {
        n.panel.as_deref() == Some(title) && n.cls.split_whitespace().any(|c| c == "pk-dt")
    });
    if !has_table {
        return None;
    }
    let in_panel = |n: &WebNode| n.panel.as_deref() == Some(title);
    let web_segments: Vec<&WebNode> = web
        .nodes
        .iter()
        .filter(|n| {
            let classes: HashSet<&str> = n.cls.split_whitespace().collect();
            in_panel(n) && classes.contains("pk-dt-tag") && !classes.contains("pk-dt-tag-tail")
        })
        .collect();
    let web_tails: Vec<&WebNode> = web
        .nodes
        .iter()
        .filter(|n| in_panel(n) && n.cls.split_whitespace().any(|c| c == "pk-dt-tag-tail"))
        .collect();
    let in_frame = |n: &BevyNode| tree.descends_from(n, bp.id);
    let is_tag_fill = |bg: Rgba| TAG_FILLS.iter().any(|fill| near_fill(bg, fill));
    let bevy_segments: Vec<&BevyNode> = bevy
        .nodes
        .iter()
        .filter(|n| {
            in_frame(n)
                && (n.w - 13.0).abs() < 0.1
                && n.bg.is_some_and(|bg| is_tag_fill(bg))
        })
        .collect();
    let is_tail_text = |text: &str| {
        text.len() > 1
            && text.starts_with('+')
            && text[1..].bytes().all(|digit| digit.is_ascii_digit())
    };
    // The Bevy tail is the button; its `+N` is a text-span child, not its own.
    let tail_labels: HashMap<u64, String> = bevy
        .nodes
        .iter()
        .filter(|n| in_frame(n))
        .filter_map(|n| {
            let label = tree
                .children
                .get(&n.id)?
                .iter()
                .map(|child| &child.text)
                .find(|text| is_tail_text(text))?;
            Some((n.id, label.clone()))
        })
        .collect();
    let bevy_tails: Vec<&BevyNode> = bevy
        .nodes
        .iter()
        .filter(|n| tail_labels.contains_key(&n.id))
        .collect();
    out.same(
        "ribbon",
        "segments",
        &web_segments.len().to_string(),
        &bevy_segments.len().to_string(),
        web_segments.len() == bevy_segments.len(),
    );
    out.same(
        "ribbon",
        "tails",
        &web_tails.len().to_string(),
        &bevy_tails.len().to_string(),
        web_tails.len() == bevy_tails.len(),
    );
    let web_rows = ribbon_rows(web_segments);
    let bevy_rows = ribbon_rows(bevy_segments);
    for ((row, at, w), (_, _, b)) in web_rows.iter().zip(bevy_rows.iter())
    {
        let part = format!("ribbon row {row} segment {at}");
        out.geo(&part, "x", Some(w.x - wp.x), Some(b.x - bp.x));
        out.geo(&part, "y", Some(w.y - wp.y), Some(b.y - bp.y));
        out.geo(&part, "w", Some(w.w), Some(b.w));
        out.geo(&part, "h", Some(w.h), Some(b.h));
        // A row the scroller clips keeps its layout but paints nothing, so a
        // fill sample only means something where the segment is on screen.
        let web_at = (w.x + w.w / 2.0, w.y + 2.0);
        let bevy_at = (b.x + b.w / 2.0, b.y + 2.0);
        if inside(&(web_at.0, web_at.1, 0.0, 0.0), wp, 1.0)
            && inside(&(bevy_at.0, bevy_at.1, 0.0, 0.0), bp, 1.0)
        {
            out.colour(
                &part,
                "fill",
                Some(pixel(web.image, web_at.0, web_at.1)),
                Some(pixel(bevy.image, bevy_at.0, bevy_at.1)),
                true,
            );
        }
    }
    for ((row, _, w), (_, _, b)) in
        ribbon_rows(web_tails).into_iter().zip(ribbon_rows(bevy_tails))
    {
        let part = format!("ribbon row {row} tail");
        out.geo(&part, "x", Some(w.x - wp.x), Some(b.x - bp.x));
        out.geo(&part, "y", Some(w.y - wp.y), Some(b.y - bp.y));
        out.geo(&part, "w", Some(w.w), Some(b.w));
        out.geo(&part, "h", Some(w.h), Some(b.h));
        let label = tail_labels.get(&b.id).map(String::as_str).unwrap_or_default();
        out.same(&part, "count", &w.text, label, w.text == label);
    }
    Some((web_rows, bevy_rows))
}

/// The windowed table's mount: how many ribbon rows each side keeps in its
/// scene, the spacers standing in for the rest, and the first mounted row's
/// y. Skipped when the panel has no web table.
fn compare_window<'a>(
    out: &mut Rows,
    web: &Capture<'a, WebNode>,
    bevy: &Capture<'a, BevyNode>,
    tree: &BevyTree<'_>,
    title: &str,
    wp: &WebNode,
    bp: &BevyNode,
    ribbon: &Option<(
        Vec<(usize, usize, &'a WebNode)>,
        Vec<(usize, usize, &'a BevyNode)>,
    )>,
) {
    let Some((web_rows, bevy_rows)) = ribbon else {
        return;
    };
    let web_mounted = web_rows.last().map_or(0, |(row, _, _)| row + 1);
    let bevy_mounted = bevy_rows.last().map_or(0, |(row, _, _)| row + 1);
    out.same(
        "window",
        "mounted rows",
        &web_mounted.to_string(),
        &bevy_mounted.to_string(),
        web_mounted == bevy_mounted,
    );
    match (web_rows.first(), bevy_rows.first()) {
        (Some((_, _, w)), Some((_, _, b))) => {
            out.geo("window", "first mounted row y", Some(w.y - wp.y), Some(b.y - bp.y));
        }
        (w, b) => {
            let show = |present: bool| if present { "mounted" } else { "absent" };
            out.same(
                "window",
                "first mounted row y",
                show(w.is_some()),
                show(b.is_some()),
                w.is_some() == b.is_some(),
            );
        }
    }
    let in_panel = |n: &&WebNode| n.panel.as_deref() == Some(title);
    let web_spacers: Vec<&WebNode> = web
        .nodes
        .iter()
        .filter(|n| {
            in_panel(n)
                && n.tag == "tr"
                && n.cls.split_whitespace().any(|c| c == "pk-dt-spacer")
        })
        .collect();
    let in_frame = |n: &BevyNode| tree.descends_from(n, bp.id);
    let bevy_spacers: Vec<&BevyNode> = bevy
        .nodes
        .iter()
        .filter(|n| {
            in_frame(n)
                && !tree.children.contains_key(&n.id)
                && n.text.is_empty()
                // Bevy UI nodes carry transparent paint by default.
                && n.bg.is_none_or(|bg| bg[3] == 0.0)
                && n.border.is_none_or(|border| border[3] == 0.0)
                && n.w > 200.0
                && n.h > 0.0
        })
        .collect();
    // A spacer above every mounted row tops the window; one below every row
    // closes it.
    let web_top = web_spacers.iter().find(|s| above_rows(s.y, s.h, &web_rows)).map(|n| n.h);
    let bevy_top = bevy_spacers.iter().find(|s| above_rows(s.y, s.h, &bevy_rows)).map(|n| n.h);
    let web_bottom = web_spacers.iter().find(|s| below_rows(s.y, &web_rows)).map(|n| (n.h, n.y - wp.y));
    let bevy_bottom = bevy_spacers.iter().find(|s| below_rows(s.y, &bevy_rows)).map(|n| (n.h, n.y - bp.y));
    optional_geo(out, "window", "top spacer height", web_top, bevy_top);
    optional_geo(
        out,
        "window",
        "bottom spacer height",
        web_bottom.map(|(h, _)| h),
        bevy_bottom.map(|(h, _)| h),
    );
    optional_geo(
        out,
        "window",
        "bottom spacer y",
        web_bottom.map(|(_, y)| y),
        bevy_bottom.map(|(_, y)| y),
    );
}

/// Whether a band of `height` at `y` lies entirely above every grouped row.
fn above_rows<N: Rect>(y: f64, h: f64, rows: &[(usize, usize, &N)]) -> bool {
    rows.iter().all(|(_, _, n)| y + h <= n.rect().1 + 0.5)
}

/// Whether a band at `y` lies entirely below every grouped row.
fn below_rows<N: Rect>(y: f64, rows: &[(usize, usize, &N)]) -> bool {
    rows.iter().all(|(_, _, n)| y >= n.rect().1 + n.rect().3 - 0.5)
}

/// The `--color-focus-ring` ink both backends paint the cursor ring with.
const RING_INK: [u8; 3] = [0xed, 0xed, 0xed];

/// The seeded cursor ring: the first mounted row is the keyboard's row, so
/// both backends paint 1px focus-ring ink inset on its edges. The samples
/// catch the ink inside the row and its absence just outside. Skipped when
/// the panel has no web table.
fn compare_ring<'a>(
    out: &mut Rows,
    web: &Capture<'a, WebNode>,
    bevy: &Capture<'a, BevyNode>,
    tree: &BevyTree<'_>,
    bp: &BevyNode,
    ribbon: &Option<(
        Vec<(usize, usize, &'a WebNode)>,
        Vec<(usize, usize, &'a BevyNode)>,
    )>,
) {
    let Some((web_rows, bevy_rows)) = ribbon else {
        return;
    };
    let (Some((_, _, wseg)), Some((_, _, bseg))) = (web_rows.first(), bevy_rows.first()) else {
        return;
    };
    // The row height is the ribbon's plus the padding it hangs in; the widest
    // such container of the segment is the row, not the narrower gutter.
    let row_h = wseg.h + 6.0;
    let Some(wr) = ring_row(web.nodes.iter().filter(|n| n.tag == "tr"), wseg, row_h) else {
        return;
    };
    let Some(br) = ring_row(
        bevy.nodes.iter().filter(|n| tree.descends_from(n, bp.id)),
        bseg,
        row_h,
    ) else {
        return;
    };
    let samples = [
        ("top edge", (wr.x + 20.0, wr.y + 0.5), (br.x + 20.0, br.y + 0.5)),
        ("bottom edge", (wr.x + 20.0, wr.y + wr.h - 0.5), (br.x + 20.0, br.y + br.h - 0.5)),
        ("left edge", (wr.x + 0.5, wr.y + wr.h / 2.0), (br.x + 0.5, br.y + br.h / 2.0)),
        ("right edge", (wr.x + wr.w - 0.5, wr.y + wr.h / 2.0), (br.x + br.w - 0.5, br.y + br.h / 2.0)),
    ];
    for (property, (wx, wy), (bx, by)) in samples {
        // The backends snap a fractional row's 1px ink to different device
        // columns, so the sample takes the closest ink-like pixel in reach
        // and fails unless both sides actually paint the ring there.
        let web_ink = closest_to(web.image, (wx, wy), RING_INK);
        let bevy_ink = closest_to(bevy.image, (bx, by), RING_INK);
        let ok = delta_e(web_ink, RING_INK) < DELTA_E_TOLERANCE
            && delta_e(bevy_ink, RING_INK) < DELTA_E_TOLERANCE;
        out.push(
            "ring",
            property,
            hex(web_ink),
            hex(bevy_ink),
            format!("ΔE {:.2}", delta_e(web_ink, bevy_ink)),
            ok,
        );
    }
}

/// The widest node about `row_h` tall that contains `segment`: the row the
/// segment's ribbon sits on, not the narrower gutter cell beside it.
fn ring_row<'a, N: Rect>(
    nodes: impl Iterator<Item = &'a N>,
    segment: &N,
    row_h: f64,
) -> Option<&'a N> {
    nodes
        .filter(|n| inside(segment, *n, 0.5) && (n.rect().3 - row_h).abs() < 1.0)
        .max_by(|a, b| a.rect().2.total_cmp(&b.rect().2))
}

/// A height that may be absent on both sides: a spacer the window doesn't
/// need. Absent on one side only fails.
fn optional_geo(out: &mut Rows, part: &str, property: &str, web: Option<f64>, bevy: Option<f64>) {
    match (web, bevy) {
        (Some(w), Some(b)) => out.geo(part, property, Some(w), Some(b)),
        (None, None) => out.same(part, property, "absent", "absent", true),
        (w, b) => {
            let show = |v: Option<f64>| v.map_or_else(|| "absent".to_owned(), number);
            out.push(part, property, show(w), show(b), "absent".to_owned(), false);
        }
    }
}

/// Whether a dumped background is one of the tag fills, within half a unit.
fn near_fill(bg: Rgba, fill: &[u8; 3]) -> bool {
    bg[..3]
        .iter()
        .enumerate()
        .all(|(at, channel)| (channel - f64::from(fill[at])).abs() < 0.5)
}

/// Nodes grouped into their ribbon rows: `(row, index within the row, node)`,
/// rows top to bottom, segments left to right. A jump of more than half a row
/// height starts a new row.
fn ribbon_rows<'a, N: Rect>(mut nodes: Vec<&'a N>) -> Vec<(usize, usize, &'a N)> {
    nodes.sort_by(|a, b| {
        a.rect().1.total_cmp(&b.rect().1).then(a.rect().0.total_cmp(&b.rect().0))
    });
    let mut rows: Vec<Vec<&'a N>> = Vec::new();
    for node in nodes {
        match rows.last_mut() {
            Some(row) if (node.rect().1 - row[0].rect().1).abs() <= 13.0 => row.push(node),
            _ => rows.push(vec![node]),
        }
    }
    rows.into_iter()
        .enumerate()
        .flat_map(|(row, segments)| {
            segments
                .into_iter()
                .enumerate()
                .map(move |(at, node)| (row, at, node))
        })
        .collect()
}

/// The sRGB sample at logical (`x`, `y`), clamped to the image.
pub fn pixel(image: &RgbImage, x: f64, y: f64) -> [u8; 3] {
    let clamp = |value: f64, size: u32| {
        ((value * SCALE).round_ties_even().max(0.0) as u32).min(size.saturating_sub(1))
    };
    image.get_pixel(clamp(x, image.width()), clamp(y, image.height())).0
}

fn rgb(color: Rgba) -> [u8; 3] {
    color.map(|channel| channel.round().clamp(0.0, 255.0) as u8)[..3]
        .try_into()
        .expect("three channels")
}

/// CIE76 colour difference between two sRGB colours.
pub fn delta_e(a: [u8; 3], b: [u8; 3]) -> f64 {
    fn lab(color: [u8; 3]) -> [f64; 3] {
        let linear = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b] = color.map(linear);
        let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
        let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
        [116.0 * f(y) - 16.0, 500.0 * (f(x) - f(y)), 200.0 * (f(y) - f(z))]
    }
    let (a, b) = (lab(a), lab(b));
    a.iter().zip(b).map(|(p, q)| (p - q).powi(2)).sum::<f64>().sqrt()
}

fn hex(color: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

fn number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_owned() } else { text.to_owned() }
}

#[derive(Default)]
struct Rows {
    rows: Vec<Row>,
    panels: Vec<String>,
    panel: String,
}

impl Rows {
    fn panel(&mut self, title: &str) {
        self.panel = title.to_owned();
        self.panels.push(title.to_owned());
    }

    fn push(&mut self, part: &str, property: &str, web: String, bevy: String, delta: String, ok: bool) {
        self.rows.push(Row {
            panel: self.panel.clone(),
            part: part.to_owned(),
            property: property.to_owned(),
            web,
            bevy,
            delta,
            ok,
        });
    }

    fn geo(&mut self, part: &str, property: &str, web: Option<f64>, bevy: Option<f64>) {
        match (web, bevy) {
            (Some(w), Some(b)) => {
                let delta = b - w;
                let ok = delta.abs() <= GEOMETRY_TOLERANCE;
                self.push(part, property, number(w), number(b), number(delta), ok);
            }
            (w, b) => {
                let show = |v: Option<f64>| v.map_or_else(|| "missing".to_owned(), number);
                self.push(part, property, show(w), show(b), "missing".to_owned(), false);
            }
        }
    }

    /// `exact`: identical sRGB; otherwise within [`DELTA_E_TOLERANCE`].
    fn colour(&mut self, part: &str, property: &str, web: Option<[u8; 3]>, bevy: Option<[u8; 3]>, exact: bool) {
        match (web, bevy) {
            (Some(w), Some(b)) => {
                let distance = delta_e(w, b);
                let ok = if exact { w == b } else { distance < DELTA_E_TOLERANCE };
                self.push(part, property, hex(w), hex(b), format!("ΔE {distance:.2}"), ok);
            }
            (w, b) => {
                let show = |v: Option<[u8; 3]>| v.map_or_else(|| "missing".to_owned(), hex);
                self.push(part, property, show(w), show(b), "missing".to_owned(), false);
            }
        }
    }

    fn same(&mut self, part: &str, property: &str, web: &str, bevy: &str, ok: bool) {
        self.push(part, property, web.to_owned(), bevy.to_owned(), String::new(), ok);
    }

    fn finish(self) -> Comparison {
        Comparison {
            rows: self.rows,
            panels: self.panels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dump::parse_lines;

    const SURFACE: [u8; 3] = [13, 13, 13];
    const BORDER: [u8; 3] = [95, 95, 95];

    /// A 200×120 logical canvas with one bordered panel at (10, 10, 160, 90).
    fn canvas(border: [u8; 3]) -> RgbImage {
        let mut image = RgbImage::from_pixel(400, 240, image::Rgb([10, 10, 10]));
        for y in 20..200 {
            for x in 20..340 {
                let edge = x < 22 || y < 22 || x >= 338 || y >= 198;
                image.put_pixel(x, y, image::Rgb(if edge { border } else { SURFACE }));
            }
        }
        image
    }

    fn web_nodes(title_x: f64) -> Vec<WebNode> {
        parse_lines(&format!(
            r#"{{"x":10,"y":10,"w":160,"h":90,"tag":"section","cls":"panel","text":"","text_rect":null,"panel":"Notes","font":13,"weight":"400","tt":"none","lh":"normal","color":null,"bg":[13,13,13,1],"border":[95,95,95,1],"border_w":1}}
{{"x":{title_x},"y":12,"w":40,"h":12,"tag":"span","cls":"panel-title","text":"Notes","text_rect":[{title_x},13,40,10],"panel":"Notes","font":10,"weight":"700","tt":"uppercase","lh":"12px","color":[237,237,237,1],"bg":null,"border":null,"border_w":0}}"#
        ))
        .expect("web fixture parses")
    }

    fn bevy_nodes() -> Vec<BevyNode> {
        parse_lines(
            r#"{"id":1,"parent":null,"x":10,"y":10,"w":160,"h":90,"tw":null,"text":"","font":null,"bold":false,"color":null,"bg":[13,13,13,1],"border":[95,95,95,1],"border_w":1}
{"id":2,"parent":1,"x":40,"y":12,"w":40,"h":12,"tw":40,"text":"NOTES","font":10,"bold":true,"color":[237,237,237,1],"bg":null,"border":null,"border_w":0}"#,
        )
        .expect("bevy fixture parses")
    }

    fn run(web_title_x: f64, bevy_border: [u8; 3]) -> Comparison {
        let (web_image, bevy_image) = (canvas(BORDER), canvas(bevy_border));
        let (web, bevy) = (web_nodes(web_title_x), bevy_nodes());
        compare(
            Capture { image: &web_image, nodes: &web },
            Capture { image: &bevy_image, nodes: &bevy },
        )
        .0
    }

    #[test]
    fn identical_captures_have_no_failures() {
        let comparison = run(40.0, BORDER);
        assert_eq!(comparison.panels, ["Notes"]);
        assert!(comparison.rows.iter().any(|row| row.part.starts_with("text")));
        assert_eq!(comparison.failures().count(), 0, "{:#?}", comparison.rows);
    }

    #[test]
    fn a_shift_past_the_tolerance_fails_and_within_it_passes() {
        let shifted = run(41.5, BORDER);
        let failing: Vec<_> = shifted.failures().collect();
        assert_eq!(failing.len(), 1, "{failing:#?}");
        assert_eq!((failing[0].part.as_str(), failing[0].property.as_str()), ("text \"NOTES\"", "x"));
        assert_eq!(run(40.9, BORDER).failures().count(), 0);
    }

    #[test]
    fn a_border_colour_change_fails_exactly() {
        let failing: Vec<String> = run(40.0, [96, 95, 95])
            .failures()
            .map(|row| row.property.clone())
            .collect();
        assert!(failing.contains(&"border colour (left edge, mid)".to_owned()), "{failing:?}");
    }

    /// A grip sample under a neighbouring panel is not compared; the same
    /// sample on an uncovered corner is.
    #[test]
    fn samples_a_neighbouring_panel_covers_are_skipped() {
        let image = canvas(BORDER);
        let bevy = bevy_nodes();
        let open_half = |web: &[WebNode]| {
            compare(Capture { image: &image, nodes: web }, Capture { image: &image, nodes: &bevy })
                .0
                .rows
                .iter()
                .any(|row| row.panel == "Notes" && row.property.starts_with("open half"))
        };
        let mut web = web_nodes(40.0);
        assert!(open_half(&web));
        let mut neighbour = web[0].clone();
        (neighbour.x, neighbour.y) = (150.0, 80.0);
        web.push(neighbour);
        assert!(!open_half(&web));
    }

    #[test]
    fn a_missing_bevy_frame_fails() {
        let image = canvas(BORDER);
        let web = web_nodes(40.0);
        let comparison = compare(
            Capture { image: &image, nodes: &web },
            Capture { image: &image, nodes: &[] },
        )
        .0;
        let failing: Vec<_> = comparison.failures().collect();
        assert_eq!(failing.len(), 1);
        assert_eq!(failing[0].bevy, "missing");
    }

    /// A two-column header: the web `th` and `.pk-dt-resize` span, and the
    /// Bevy cell with its handle child.
    fn header_rows(bevy_cell_w: f64, bevy_handle_x: f64) -> Vec<Row> {
        let image = canvas(BORDER);
        let mut web = web_nodes(40.0);
        let th = |x: f64, w: f64| {
            format!(
                r#"{{"x":{x},"y":40,"w":{w},"h":25,"tag":"th","cls":"pk-dt-th","text":"","text_rect":null,"panel":"Notes","font":11,"weight":"700","tt":"uppercase","lh":"24px","color":null,"bg":null,"border":null,"border_w":0}}"#
            )
        };
        let zone = |x: f64| {
            format!(
                r#"{{"x":{x},"y":40,"w":7,"h":25,"tag":"span","cls":"pk-dt-resize","text":"","text_rect":null,"panel":"Notes","font":11,"weight":"400","tt":"none","lh":"normal","color":null,"bg":null,"border":null,"border_w":0}}"#
            )
        };
        web.extend(
            parse_lines::<WebNode>(&[th(20.0, 60.0), zone(76.0), th(80.0, 50.0), zone(126.0)].join("\n"))
                .expect("header fixture parses"),
        );
        let handle = |id: u32, parent: u32, x: f64| {
            format!(
                r#"{{"id":{id},"parent":{parent},"x":{x},"y":40,"w":7,"h":25,"tw":null,"text":"","font":null,"bold":false,"color":null,"bg":null,"border":[0,0,0,0],"border_w":0}}"#
            )
        };
        let cell = |id: u32, x: f64, w: f64| {
            format!(
                r#"{{"id":{id},"parent":1,"x":{x},"y":40,"w":{w},"h":25,"tw":null,"text":"","font":null,"bold":false,"color":null,"bg":[13,13,13,1],"border":null,"border_w":0}}"#
            )
        };
        let mut bevy = bevy_nodes();
        bevy.extend(
            parse_lines::<BevyNode>(
                &[
                    cell(3, 20.0, bevy_cell_w),
                    handle(4, 3, bevy_handle_x),
                    cell(5, 80.0, 50.0),
                    handle(6, 5, 126.0),
                ]
                .join("\n"),
            )
            .expect("bevy header fixture parses"),
        );
        compare(Capture { image: &image, nodes: &web }, Capture { image: &image, nodes: &bevy })
            .0
            .rows
            .into_iter()
            .filter(|row| row.part.starts_with("header"))
            .collect()
    }

    #[test]
    fn header_cells_and_resize_zones_are_compared_per_column() {
        let rows = header_rows(60.0, 76.0);
        assert!(rows.iter().any(|r| r.part == "header column 2" && r.property == "w"));
        assert!(rows.iter().any(|r| r.part == "header resize zone 1" && r.property == "x"));
        assert!(rows.iter().all(|r| r.ok), "{rows:#?}");
    }

    /// A Nodes-style ribbon row: two lettered segments and a `+2` tail on both
    /// backends, painted into the canvases. `bevy_second_x` moves the Bevy
    /// twin of the second segment; `drop_bevy_segment` removes it entirely.
    fn ribbon(bevy_second_x: f64, drop_bevy_segment: bool) -> Vec<Row> {
        let (y, h) = (50.0, 18.0);
        let mut web_image = canvas(BORDER);
        let mut bevy_image = canvas(BORDER);
        let rects = [(30.0, TAG_FILLS[0]), (40.0, TAG_FILLS[1]), (56.0, SURFACE)];
        for (x, color) in rects {
            fill(&mut web_image, x, y, 13.0, h, color);
        }
        if !drop_bevy_segment {
            fill(&mut bevy_image, bevy_second_x, y, 13.0, h, TAG_FILLS[1]);
        }
        for (x, color) in [(30.0, TAG_FILLS[0]), (56.0, SURFACE)] {
            fill(&mut bevy_image, x, y, 13.0, h, color);
        }
        let button = |x: f64, cls: &str, text: &str, bg: [u8; 3]| {
            format!(
                r#"{{"x":{x},"y":{y},"w":13,"h":{h},"tag":"button","cls":"{cls}","text":"{text}","text_rect":null,"panel":"Notes","font":11,"weight":"400","tt":"none","lh":"normal","color":null,"bg":[{},{},{},1],"border":null,"border_w":0}}"#,
                bg[0], bg[1], bg[2]
            )
        };
        let mut web = web_nodes(40.0);
        web.extend(
            parse_lines::<WebNode>(
                &[
                    r#"{"x":10,"y":30,"w":160,"h":60,"tag":"div","cls":"pk-widget pk-dt","text":"","text_rect":null,"panel":"Notes","font":13,"weight":"400","tt":"none","lh":"normal","color":null,"bg":null,"border":null,"border_w":0}"#.to_owned(),
                    button(30.0, "pk-dt-tag pk-dt-tag-0", "", TAG_FILLS[0]),
                    button(40.0, "pk-dt-tag pk-dt-tag-1", "", TAG_FILLS[1]),
                    button(56.0, "pk-dt-tag pk-dt-tag-tail", "+2", SURFACE),
                ]
                .join("\n"),
            )
            .expect("web ribbon fixture parses"),
        );
        let bevy_node = |id: u32, x: f64, text: &str, bg: [u8; 3]| {
            format!(
                r#"{{"id":{id},"parent":1,"x":{x},"y":{y},"w":13,"h":{h},"tw":null,"text":"{text}","font":11,"bold":false,"color":null,"bg":[{},{},{},1],"border":null,"border_w":0}}"#,
                bg[0], bg[1], bg[2]
            )
        };
        let mut bevy = bevy_nodes();
        let mut bevy_lines = vec![
            bevy_node(3, 30.0, "l", TAG_FILLS[0]),
            bevy_node(5, 56.0, "", SURFACE),
        ];
        if !drop_bevy_segment {
            bevy_lines.insert(1, bevy_node(4, bevy_second_x, "o", TAG_FILLS[1]));
        }
        // The `+N` label is a text child of the tail button, not its own.
        bevy_lines.push(
            r#"{"id":6,"parent":5,"x":59.0,"y":53.5,"w":13,"h":11,"tw":13.0,"text":"+2","font":11,"bold":false,"color":null,"bg":null,"border":null,"border_w":0}"#.to_owned(),
        );
        bevy.extend(
            parse_lines::<BevyNode>(&bevy_lines.join("\n"))
                .expect("bevy ribbon fixture parses"),
        );
        compare(
            Capture { image: &web_image, nodes: &web },
            Capture { image: &bevy_image, nodes: &bevy },
        )
        .0
        .rows
        .into_iter()
        .filter(|row| row.part.starts_with("ribbon"))
        .collect()
    }

    /// Paint a logical-px rect into a capture canvas.
    fn fill(image: &mut RgbImage, x: f64, y: f64, w: f64, h: f64, color: [u8; 3]) {
        for py in (y * SCALE) as u32..((y + h) * SCALE) as u32 {
            for px in (x * SCALE) as u32..((x + w) * SCALE) as u32 {
                image.put_pixel(px, py, image::Rgb(color));
            }
        }
    }

    #[test]
    fn a_drifted_ribbon_segment_fails_its_own_row() {
        let rows = ribbon(40.0, false);
        assert!(rows.iter().all(|row| row.ok), "{rows:#?}");
        let rows = ribbon(43.0, false);
        let failing: Vec<_> = rows
            .iter()
            .filter(|row| !row.ok)
            .map(|row| (row.part.as_str(), row.property.as_str()))
            .collect();
        assert_eq!(failing, [("ribbon row 0 segment 1", "x")]);
    }

    #[test]
    fn a_missing_ribbon_segment_fails_the_count_row() {
        let count = |rows: &[Row]| {
            rows.iter()
                .find(|row| (row.part.as_str(), row.property.as_str()) == ("ribbon", "segments"))
                .expect("segment count row")
                .ok
        };
        assert!(count(&ribbon(40.0, false)));
        assert!(!count(&ribbon(40.0, true)));
    }

    #[test]
    fn a_header_width_or_resize_zone_drift_fails_its_own_row() {
        let failing = |rows: Vec<Row>| -> Vec<(String, String)> {
            rows.into_iter().filter(|r| !r.ok).map(|r| (r.part, r.property)).collect()
        };
        assert_eq!(
            failing(header_rows(63.0, 76.0)),
            [("header column 1".to_owned(), "w".to_owned())]
        );
        assert_eq!(
            failing(header_rows(60.0, 79.0)),
            [("header resize zone 1".to_owned(), "x".to_owned())]
        );
    }

    /// Both canvases with a graph host, one node disc and one horizontal edge
    /// painted at the same place; `bevy_edge` and `bevy_disc` say whether the
    /// Bevy side draws them.
    fn graph_run(bevy_disc: bool, bevy_edge: bool) -> Comparison {
        let paint = |disc: bool, edge: bool| {
            let mut image = canvas(BORDER);
            for y in 80..180 {
                for x in 40..240 {
                    image.put_pixel(x, y, image::Rgb([10, 10, 10]));
                }
            }
            if disc {
                for y in 110..130 {
                    for x in 100..120 {
                        image.put_pixel(x, y, image::Rgb([40, 50, 60]));
                    }
                }
            }
            if edge {
                for x in 120..200 {
                    image.put_pixel(x, 120, image::Rgb(EDGE_INK));
                }
            }
            image
        };
        let host = |class: &str, x: f64, y: f64, w: f64, h: f64| {
            format!(
                r#"{{"x":{x},"y":{y},"w":{w},"h":{h},"tag":"div","cls":"{class}","text":"","text_rect":null,"panel":"Notes","font":13,"weight":"400","tt":"none","lh":"normal","color":null,"bg":null,"border":null,"border_w":0}}"#
            )
        };
        let mut web = web_nodes(40.0);
        web.extend(
            parse_lines::<WebNode>(&[
                host("pk-graph", 20.0, 40.0, 100.0, 50.0),
                host("pk-graph-node", 50.0, 55.0, 10.0, 10.0),
                host("pk-graph-edge", 60.0, 60.0, 40.0, 0.0),
            ]
            .join("\n"))
            .expect("web graph fixture parses"),
        );
        let bevy_node = |x: f64, y: f64, w: f64, h: f64, bg: &str| {
            format!(
                r#"{{"id":9,"parent":1,"x":{x},"y":{y},"w":{w},"h":{h},"tw":null,"text":"","font":null,"bold":false,"color":null,"bg":{bg},"border":null,"border_w":0}}"#
            )
        };
        let mut extra = vec![bevy_node(20.0, 40.0, 100.0, 50.0, "[10,10,10,1]")];
        if bevy_disc {
            extra.push(bevy_node(50.0, 55.0, 10.0, 10.0, "[40,50,60,1]"));
        }
        let mut nodes = bevy_nodes();
        nodes.extend(parse_lines::<BevyNode>(&extra.join("\n")).expect("bevy graph fixture parses"));
        let (web_image, bevy_image) = (paint(true, true), paint(bevy_disc, bevy_edge));
        compare(
            Capture { image: &web_image, nodes: &web },
            Capture { image: &bevy_image, nodes: &nodes },
        )
        .0
    }

    #[test]
    fn a_matching_graph_has_host_node_and_edge_rows_and_no_failures() {
        let comparison = graph_run(true, true);
        let parts: Vec<&str> = comparison.rows.iter().map(|row| row.part.as_str()).collect();
        for part in ["graph host", "graph node 0", "graph edge 0"] {
            assert!(parts.contains(&part), "{part} missing from {parts:?}");
        }
        assert_eq!(comparison.failures().count(), 0, "{:#?}", comparison.failures().collect::<Vec<_>>());
    }

    #[test]
    fn a_node_disc_the_bevy_side_does_not_draw_fails() {
        let comparison = graph_run(false, true);
        let failing: Vec<&str> = comparison.failures().map(|row| row.part.as_str()).collect();
        assert!(failing.iter().all(|part| *part == "graph node 0"), "{failing:?}");
        assert!(failing.contains(&"graph node 0"));
    }

    #[test]
    fn edge_ink_the_bevy_side_does_not_draw_fails() {
        let comparison = graph_run(true, false);
        let failing: Vec<(&str, &str)> = comparison
            .failures()
            .map(|row| (row.part.as_str(), row.property.as_str()))
            .collect();
        assert_eq!(failing, [("graph edge 0", "ink at midpoint")]);
    }
}
