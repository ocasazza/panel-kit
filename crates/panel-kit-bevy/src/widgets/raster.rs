//! Software rasteriser shared by the chart and graph painters: capsule
//! strokes into a coverage mask, source-over compositing, and the RGBA8
//! image a UI node displays.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Composite `ink` over `over` through `coverage` (premultiplied sRGB RGBA).
pub(super) fn composite(over: &mut [[f32; 4]], coverage: &[f32], ink: Color) {
    let ink = ink.to_srgba();
    for (pixel, &cover) in over.iter_mut().zip(coverage) {
        let alpha = cover * ink.alpha;
        if alpha <= 0.0 {
            continue;
        }
        let keep = 1.0 - alpha;
        pixel[0] = ink.red * alpha + pixel[0] * keep;
        pixel[1] = ink.green * alpha + pixel[1] * keep;
        pixel[2] = ink.blue * alpha + pixel[2] * keep;
        pixel[3] = alpha + pixel[3] * keep;
    }
}

/// Premultiplied pixels to straight-alpha sRGB RGBA8 bytes.
pub(super) fn flatten(over: &[[f32; 4]]) -> Vec<u8> {
    over.iter()
        .flat_map(|&[r, g, b, a]| {
            let unmul = |channel: f32| {
                if a > 0.0 {
                    (channel / a * 255.0).round().clamp(0.0, 255.0) as u8
                } else {
                    0
                }
            };
            [unmul(r), unmul(g), unmul(b), (a * 255.0).round() as u8]
        })
        .collect()
}

/// A nearest-sampled RGBA8 sRGB image of `size` over `pixels`.
pub(super) fn rgba_image(size: UVec2, pixels: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// Raise `coverage` to each pixel's overlap with the capsule of radius `half`
/// around the segment `from`–`to`, approximated by its distance to the pixel
/// centre. Non-finite endpoints paint nothing.
pub(super) fn stroke_segment(
    coverage: &mut [f32],
    width: usize,
    height: usize,
    from: (f64, f64),
    to: (f64, f64),
    half: f64,
) {
    if ![from.0, from.1, to.0, to.1].iter().all(|v| v.is_finite()) {
        return;
    }
    let reach = half + 0.5;
    let columns = (from.0.min(to.0) - reach).floor().max(0.0) as usize
        ..=((from.0.max(to.0) + reach).ceil().max(0.0) as usize).min(width.saturating_sub(1));
    let rows = (from.1.min(to.1) - reach).floor().max(0.0) as usize
        ..=((from.1.max(to.1) + reach).ceil().max(0.0) as usize).min(height.saturating_sub(1));
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length_sq = dx * dx + dy * dy;

    for row in rows {
        for column in columns.clone() {
            let (px, py) = (column as f64 + 0.5 - from.0, row as f64 + 0.5 - from.1);
            let along = if length_sq > 0.0 {
                ((px * dx + py * dy) / length_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance = (px - along * dx).hypot(py - along * dy);
            let cover = (reach - distance).clamp(0.0, 1.0) as f32;
            let cell = &mut coverage[row * width + column];
            *cell = cell.max(cover);
        }
    }
}
