//! The two node dumps a capture writes next to its PNG (`<png>.nodes.jsonl`).

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Deserialize;

/// sRGB channels 0–255 plus alpha 0–1.
pub type Rgba = [f64; 4];

/// One rendered DOM element (`src/dump.js`). Geometry is CSS px.
#[derive(Clone, Debug, Deserialize)]
pub struct WebNode {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub tag: String,
    pub cls: String,
    /// The element's own text nodes, joined.
    pub text: String,
    /// Bounding box of `text`: x, y, w, h.
    pub text_rect: Option<[f64; 4]>,
    /// Title of the enclosing `.panel`.
    pub panel: Option<String>,
    pub font: f64,
    pub weight: String,
    /// `text-transform`.
    pub tt: String,
    /// `line-height`, as computed (`19.5px` or `normal`).
    pub lh: String,
    pub color: Option<Rgba>,
    pub bg: Option<Rgba>,
    pub border: Option<Rgba>,
    pub border_w: f64,
}

/// One visible Bevy UI node (the Bevy workspace example's capture dump).
/// Geometry is logical px.
#[derive(Clone, Debug, Deserialize)]
pub struct BevyNode {
    pub id: u64,
    pub parent: Option<u64>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Laid-out text width, for text nodes.
    pub tw: Option<f64>,
    pub text: String,
    /// Font size, for text nodes.
    pub font: Option<f64>,
    pub bold: bool,
    pub color: Option<Rgba>,
    pub bg: Option<Rgba>,
    pub border: Option<Rgba>,
    pub border_w: f64,
}

/// A rectangle in logical px.
pub trait Rect {
    fn rect(&self) -> (f64, f64, f64, f64);
}

impl Rect for WebNode {
    fn rect(&self) -> (f64, f64, f64, f64) {
        (self.x, self.y, self.w, self.h)
    }
}

impl Rect for BevyNode {
    fn rect(&self) -> (f64, f64, f64, f64) {
        (self.x, self.y, self.w, self.h)
    }
}

impl Rect for (f64, f64, f64, f64) {
    fn rect(&self) -> (f64, f64, f64, f64) {
        *self
    }
}

/// Whether two rectangles share any area.
pub fn overlaps(a: &impl Rect, b: &impl Rect) -> bool {
    let (ax, ay, aw, ah) = a.rect();
    let (bx, by, bw, bh) = b.rect();
    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
}

/// Whether `inner` lies within `outer`, give or take `slack` px.
pub fn inside(inner: &impl Rect, outer: &impl Rect, slack: f64) -> bool {
    let (x, y, w, h) = inner.rect();
    let (ox, oy, ow, oh) = outer.rect();
    ox - slack <= x && x + w <= ox + ow + slack && oy - slack <= y && y + h <= oy + oh + slack
}

/// Parse one dump: one JSON object per non-blank line.
pub fn parse_lines<T: DeserializeOwned>(text: &str) -> Result<Vec<T>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str(line).map_err(|error| format!("line {}: {error}", index + 1))
        })
        .collect()
}

/// Read and parse the dump beside `png`.
pub fn read_dump<T: DeserializeOwned>(png: &Path) -> Result<Vec<T>, String> {
    let path = png.with_extension("png.nodes.jsonl");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("reading {}: {error}", path.display()))?;
    parse_lines(&text).map_err(|error| format!("{}: {error}", path.display()))
}
