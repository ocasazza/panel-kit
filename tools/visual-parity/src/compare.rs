//! Pair every web panel with its Bevy twin and compare geometry, pixels,
//! traffic lights, text runs, and the data-table toolbar.

use std::collections::{BTreeMap, HashMap, HashSet};

use image::RgbImage;

use crate::dump::{inside, overlaps, BevyNode, Rgba, WebNode};

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

/// A difference that is known and accepted, with why.
struct Exemption {
    applies: fn(&Row) -> bool,
    reason: &'static str,
}

const EXEMPTIONS: &[Exemption] = &[Exemption {
    applies: |row| row.part.contains('⠋'),
    reason: "animated spinner ring: the captures sample different rotation phases",
}];

impl Row {
    /// Why a failing row is accepted, or `None` when it is unexplained.
    pub fn exemption(&self) -> Option<&'static str> {
        EXEMPTIONS
            .iter()
            .find(|exemption| (exemption.applies)(self))
            .map(|exemption| exemption.reason)
    }
}

/// Every row of one comparison, in panel order.
#[derive(Debug, Default)]
pub struct Comparison {
    pub rows: Vec<Row>,
    /// Panel titles in comparison order.
    pub panels: Vec<String>,
}

impl Comparison {
    /// Failing rows no exemption explains.
    pub fn unexplained(&self) -> impl Iterator<Item = &Row> {
        self.rows
            .iter()
            .filter(|row| !row.ok && row.exemption().is_none())
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
    compare_columns(out, web.nodes, bevy.nodes, tree, title, wp, bp);
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

/// The data-table header: each column's cell and its resize zone (a 7px
/// `col-resize` strip overhanging the cell's right edge by 3px), paired left
/// to right.
fn compare_columns(
    out: &mut Rows,
    web: &[WebNode],
    bevy: &[BevyNode],
    tree: &BevyTree<'_>,
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
        &handles.len().to_string(),
        heads.len() == handles.len() && zones.len() == handles.len(),
    );
    for (index, head) in heads.iter().enumerate() {
        let handle = handles.get(index);
        let cell = handle
            .and_then(|h| h.parent)
            .and_then(|parent| tree.by_id.get(&parent));
        let part = format!("header column {}", index + 1);
        out.geo(&part, "x", Some(head.x - wp.x), cell.map(|b| b.x - bp.x));
        out.geo(&part, "y", Some(head.y - wp.y), cell.map(|b| b.y - bp.y));
        out.geo(&part, "w", Some(head.w), cell.map(|b| b.w));
        out.geo(&part, "h", Some(head.h), cell.map(|b| b.h));
        let zone = zones.get(index);
        let part = format!("header resize zone {}", index + 1);
        out.geo(&part, "x", zone.map(|z| z.x - wp.x), handle.map(|b| b.x - bp.x));
        out.geo(&part, "y", zone.map(|z| z.y - wp.y), handle.map(|b| b.y - bp.y));
        out.geo(&part, "w", zone.map(|z| z.w), handle.map(|b| b.w));
        out.geo(&part, "h", zone.map(|z| z.h), handle.map(|b| b.h));
    }
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
    fn identical_captures_have_no_unexplained_rows() {
        let comparison = run(40.0, BORDER);
        assert_eq!(comparison.panels, ["Notes"]);
        assert!(comparison.rows.iter().any(|row| row.part.starts_with("text")));
        assert_eq!(comparison.unexplained().count(), 0, "{:#?}", comparison.rows);
    }

    #[test]
    fn a_shift_past_the_tolerance_fails_and_within_it_passes() {
        let shifted = run(41.5, BORDER);
        let failing: Vec<_> = shifted.unexplained().collect();
        assert_eq!(failing.len(), 1, "{failing:#?}");
        assert_eq!((failing[0].part.as_str(), failing[0].property.as_str()), ("text \"NOTES\"", "x"));
        assert_eq!(run(40.9, BORDER).unexplained().count(), 0);
    }

    #[test]
    fn a_border_colour_change_fails_exactly() {
        let failing: Vec<String> = run(40.0, [96, 95, 95])
            .unexplained()
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
        let failing: Vec<_> = comparison.unexplained().collect();
        assert_eq!(failing.len(), 1);
        assert_eq!(failing[0].bevy, "missing");
    }

    #[test]
    fn only_the_spinner_ring_is_exempt() {
        let row = |part: &str| Row {
            panel: "Theme".into(),
            part: part.into(),
            property: "x".into(),
            web: "1".into(),
            bevy: "9".into(),
            delta: "8".into(),
            ok: false,
        };
        assert!(row("text \"⠋\"").exemption().is_some());
        assert!(row("text \"spinner painter\"").exemption().is_none());
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
}
