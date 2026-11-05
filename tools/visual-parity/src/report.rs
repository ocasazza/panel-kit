//! `report.md` and per-panel side-by-side images (web | Bevy | diff × 4).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use image::{imageops, Rgb, RgbImage};

use crate::compare::{Comparison, Pair, DELTA_E_TOLERANCE, GEOMETRY_TOLERANCE, SCALE};

/// Write the report and side-by-sides into `out`.
pub fn write(
    out: &Path,
    comparison: &Comparison,
    pairs: &[Pair<'_>],
    web: &RgbImage,
    bevy: &RgbImage,
) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;
    let mut sides = Vec::new();
    for pair in pairs {
        let path = out.join(format!("{}-side.png", pair.title.to_lowercase()));
        side_by_side(web, bevy, pair)
            .save(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        sides.push((pair.title.as_str(), path));
    }

    let mut md = String::new();
    let _ = writeln!(md, "# Bevy vs web parity report\n");
    let _ = writeln!(
        md,
        "Web: Chromium, 1440×900 CSS px, DPR 2. Bevy: offscreen 1440×900 logical, scale 2.\n\
         Tolerances: geometry ±{GEOMETRY_TOLERANCE} px; colour exact sRGB \
         (anti-aliased samples: ΔE76 < {DELTA_E_TOLERANCE}).\n"
    );
    md.push_str("| panel | pass | fail |\n|---|---|---|\n");
    for (panel, pass, fail) in comparison.summary() {
        let _ = writeln!(md, "| {panel} | {pass} | {fail} |");
    }
    md.push_str("\n| panel | part | property | web | bevy | Δ | ok | cause |\n|---|---|---|---|---|---|---|---|\n");
    let mut causes: BTreeMap<&str, usize> = BTreeMap::new();
    for row in &comparison.rows {
        let cause = if row.ok { "" } else { row.exemption().unwrap_or("UNEXPLAINED") };
        if !row.ok {
            *causes.entry(cause).or_default() += 1;
        }
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {} | {cause} |",
            row.panel,
            row.part.replace('|', "\\|"),
            row.property,
            row.web,
            row.bevy,
            row.delta,
            if row.ok { "✓" } else { "✗" },
        );
    }
    md.push_str("\n## Failures by cause\n\n");
    for (cause, count) in &causes {
        let _ = writeln!(md, "- {count} × {cause}");
    }
    md.push_str("\n## Side-by-side (web | bevy | diff×4)\n\n");
    for (title, path) in &sides {
        let name = path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
        let _ = writeln!(md, "- {title}: `{name}`");
    }
    let path = out.join("report.md");
    std::fs::write(&path, md).map_err(|error| format!("{}: {error}", path.display()))
}

fn side_by_side(web: &RgbImage, bevy: &RgbImage, pair: &Pair<'_>) -> RgbImage {
    let device = |value: f64| (value * SCALE).round().max(0.0) as u32;
    let (w, h) = (device(pair.web.w), device(pair.web.h));
    let crop = |image: &RgbImage, x: f64, y: f64| {
        let (x, y) = (device(x).min(image.width()), device(y).min(image.height()));
        let w = w.min(image.width() - x);
        let h = h.min(image.height() - y);
        imageops::crop_imm(image, x, y, w, h).to_image()
    };
    let a = crop(web, pair.web.x, pair.web.y);
    let b = crop(bevy, pair.bevy.x, pair.bevy.y);
    let mut diff = RgbImage::new(a.width(), a.height());
    for (x, y, pixel) in diff.enumerate_pixels_mut() {
        let p = a.get_pixel(x, y).0;
        let q = b.get_pixel_checked(x, y).map_or([0; 3], |q| q.0);
        *pixel = Rgb([0, 1, 2].map(|i| (p[i].abs_diff(q[i]) as u32 * 4).min(255) as u8));
    }
    let mut side = RgbImage::from_pixel(a.width() * 3 + 8, a.height(), Rgb([40, 0, 40]));
    imageops::replace(&mut side, &a, 0, 0);
    imageops::replace(&mut side, &b, i64::from(a.width()) + 4, 0);
    imageops::replace(&mut side, &diff, i64::from(a.width()) * 2 + 8, 0);
    side
}
