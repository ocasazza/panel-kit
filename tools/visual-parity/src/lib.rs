//! Visual parity gate between the web (Dioxus) and Bevy workspace canaries.
//!
//! `visual-parity check` captures both backends at 1440×900 logical px,
//! scale 2, pairs every web panel with its Bevy frame, compares geometry,
//! sampled pixels, traffic lights, text runs and the data-table toolbar, and
//! fails on any difference no [`compare::Row::exemption`] explains. See the
//! README section "Visual parity check".

pub mod capture;
pub mod compare;
pub mod dump;
pub mod report;

use std::path::Path;

use compare::{Capture, Comparison};
use dump::{read_dump, BevyNode, WebNode};

/// Compare two existing captures (PNGs with their `.nodes.jsonl` dumps) and
/// write the report into `out`.
pub fn compare_files(web_png: &Path, bevy_png: &Path, out: &Path) -> Result<Comparison, String> {
    let open = |path: &Path| {
        image::open(path)
            .map(|image| image.to_rgb8())
            .map_err(|error| format!("{}: {error}", path.display()))
    };
    let (web_image, bevy_image) = (open(web_png)?, open(bevy_png)?);
    let web: Vec<WebNode> = read_dump(web_png)?;
    let bevy: Vec<BevyNode> = read_dump(bevy_png)?;
    if !web.iter().any(|node| node.cls.split_whitespace().next() == Some("panel")) {
        return Err(format!("{}: the web dump has no panel", web_png.display()));
    }
    let (comparison, pairs) = compare::compare(
        Capture {
            image: &web_image,
            nodes: &web,
        },
        Capture {
            image: &bevy_image,
            nodes: &bevy,
        },
    );
    report::write(out, &comparison, &pairs, &web_image, &bevy_image)?;
    Ok(comparison)
}
