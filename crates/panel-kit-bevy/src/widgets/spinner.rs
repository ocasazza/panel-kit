//! Bevy UI port of panel-kit core's `spinner` component.
//!
//! Mirrors the web `.spinner` painter: a 12px ring (`.spin-ring`, a 2px
//! border-colour circle whose top quarter is the highlight) carrying core's
//! frame glyph, then an optional highlight label (`.spin-label`) that is
//! omitted entirely when empty. The ring turns once per `SPIN_PERIOD`.

use std::f32::consts::TAU;
use std::time::Duration;

use bevy::asset::{weak_handle, RenderAssetUsages};
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use panel_kit_core::widgets::spinner::spinner_frame;

use super::{styled_text, TextStyle};
use crate::theme::BevyTheme;

/// The ring's outline (`border-color`), tinted per theme.
pub const RING_IMAGE: Handle<Image> = weak_handle!("4b8e2c6d-1f3a-4d5e-9a7b-c2d1e0f3a596");

/// The ring's top quarter (`border-top-color`), tinted per theme.
pub const RING_ARC_IMAGE: Handle<Image> = weak_handle!("7e1a9c3b-5d2f-4a6e-8b0c-d4f2e1a3b789");

/// Ring side and stroke in CSS pixels (`.spin-ring`).
const RING_PX: f32 = 12.0;
const RING_STROKE_PX: f32 = 2.0;

/// One turn of the ring (`animation: spin .7s`).
pub const SPIN_PERIOD: Duration = Duration::from_millis(700);

/// Texels per logical pixel of the ring images.
const RING_TEXELS: u32 = 4;

/// Register [`RING_IMAGE`] and [`RING_ARC_IMAGE`]: an anti-aliased white
/// annulus, and its top quarter between the 45° diagonals where a rounded
/// CSS border switches from `border-top-color` to the side colours.
pub fn insert_ring_images(images: Option<ResMut<Assets<Image>>>) {
    let Some(mut images) = images else {
        return;
    };
    images.insert(&RING_IMAGE, ring_image(false));
    images.insert(&RING_ARC_IMAGE, ring_image(true));
}

fn ring_image(top_only: bool) -> Image {
    const SAMPLES: u32 = 4;
    let side = RING_PX as u32 * RING_TEXELS;
    let outer = RING_PX / 2.0;
    let inner = outer - RING_STROKE_PX;
    let mut data = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let mut covered = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = (x as f32 + (sx as f32 + 0.5) / SAMPLES as f32) / RING_TEXELS as f32 - outer;
                    let py = (y as f32 + (sy as f32 + 0.5) / SAMPLES as f32) / RING_TEXELS as f32 - outer;
                    let r = (px * px + py * py).sqrt();
                    let top = py < 0.0 && px.abs() <= -py;
                    if r <= outer && r >= inner && (!top_only || top) {
                        covered += 1;
                    }
                }
            }
            let alpha = (covered * 255 / (SAMPLES * SAMPLES)) as u8;
            data.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Spawn a spinner ring and optional label for a monotonic tick counter.
///
/// The ring glyph is [`spinner_frame`]`(tick)`, so animation wraps with core's
/// frame set and any given tick renders deterministically; `elapsed` sets the
/// ring's turn. An empty `label` draws the ring alone, matching the web
/// component.
pub fn spinner(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    label: &str,
    tick: u64,
    elapsed: Duration,
) {
    let turn = elapsed.as_secs_f32() / SPIN_PERIOD.as_secs_f32();
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(theme.metrics.rem(0.45)),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Px(RING_PX),
                    height: Val::Px(RING_PX),
                    flex_shrink: 0.0,
                    ..default()
                },
                ImageNode::new(RING_IMAGE).with_color(theme.border),
                Transform::from_rotation(Quat::from_rotation_z(-turn.fract() * TAU)),
            ))
            .with_children(|ring| {
                ring.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::ZERO,
                        top: Val::ZERO,
                        width: Val::Px(RING_PX),
                        height: Val::Px(RING_PX),
                        ..default()
                    },
                    ImageNode::new(RING_ARC_IMAGE).with_color(theme.highlight),
                ));
                // The frame glyph starts at the ring's content box, inside
                // the stroke, as the web's inline ring text does.
                ring.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(RING_STROKE_PX),
                    top: Val::Px(RING_STROKE_PX),
                    ..default()
                })
                .with_children(|glyph| {
                    styled_text(glyph, spinner_frame(tick), TextStyle::body(theme));
                });
            });
            if !label.is_empty() {
                styled_text(
                    row,
                    label,
                    TextStyle::rem(theme, 0.78).color(theme.highlight),
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_kit_core::theme::ThemeTokens;
    use panel_kit_core::widgets::spinner::{SpinnerModel, FRAMES};

    /// Marker on the test root so tests can find the row the spinner built.
    #[derive(Component)]
    struct SpinnerRoot;

    /// One text node the spinner spawned, read back from the ECS.
    struct DrawnText {
        content: String,
        size: f32,
        color: Color,
    }

    /// A theme whose root size differs from the default, so size mappings
    /// are observable.
    fn theme() -> BevyTheme {
        let mut tokens = ThemeTokens::dark();
        tokens.typography.body_size = 17.0;
        BevyTheme::from_tokens(&tokens)
    }

    /// Render one spinner through a real `App`, exactly as an app would.
    fn render(label: &str, tick: u64) -> (App, Entity) {
        let mut app = App::new();
        let root = app.world_mut().spawn((Node::default(), SpinnerRoot)).id();
        let label = label.to_string();
        app.add_systems(Startup, move |mut commands: Commands| {
            let theme = theme();
            commands.entity(root).with_children(|parent| {
                spinner(parent, &theme, &label, tick, Duration::ZERO);
            });
        });
        app.update();
        (app, root)
    }

    /// The single row the spinner spawned under the test root.
    fn spinner_row(app: &App, root: Entity) -> Entity {
        let children = app
            .world()
            .get::<Children>(root)
            .expect("spinner spawns a row");
        children.first().copied().expect("exactly one row")
    }

    /// Every text node under the row, depth first in spawn order.
    fn drawn_texts(app: &App, row: Entity) -> Vec<DrawnText> {
        let world = app.world();
        let mut texts = Vec::new();
        let mut stack = vec![row];
        while let Some(entity) = stack.pop() {
            if let Some(text) = world.get::<Text>(entity) {
                texts.push(DrawnText {
                    content: text.0.clone(),
                    size: world.get::<TextFont>(entity).expect("font").font_size,
                    color: world.get::<TextColor>(entity).expect("colour").0,
                });
            }
            if let Some(children) = world.get::<Children>(entity) {
                stack.extend(children.iter().rev());
            }
        }
        texts
    }

    /// Port of the TUI test `spinner_uses_core_frame_selection`.
    #[test]
    fn spinner_uses_core_frame_selection() {
        for tick in 0..(FRAMES.len() as u64 * 2) {
            let (app, root) = render("", tick);
            let texts = drawn_texts(&app, spinner_row(&app, root));
            assert_eq!(texts.len(), 1, "empty label draws the ring alone");
            assert_eq!(texts[0].content, spinner_frame(tick), "tick {tick}");
        }
    }

    /// Port of the web snapshot expectation for `ContentView::Spinner` at tick
    /// 10 (`src/widgets/snapshot_expectations.rs`: ring `⠋`, label `indexing`).
    #[test]
    fn spinner_matches_web_snapshot_at_tick_ten() {
        let (app, root) = render("indexing", 10);
        let texts = drawn_texts(&app, spinner_row(&app, root));
        assert_eq!(
            texts.iter().map(|text| text.content.as_str()).collect::<Vec<_>>(),
            vec!["⠋", "indexing"]
        );
    }

    /// Port of core's `spinner_model_carries_optional_label`, observed through
    /// the rendered nodes.
    #[test]
    fn spinner_model_label_controls_label_presence() {
        let without_label = SpinnerModel { label: None };
        let (app, root) = render(without_label.label.as_deref().unwrap_or_default(), 0);
        assert_eq!(drawn_texts(&app, spinner_row(&app, root)).len(), 1);

        let with_label = SpinnerModel {
            label: Some("loading".into()),
        };
        let (app, root) = render(with_label.label.as_deref().unwrap_or_default(), 0);
        let texts = drawn_texts(&app, spinner_row(&app, root));
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[1].content, "loading");
    }

    /// The glyph is body text and the label a highlight `.78rem` caption,
    /// both resolved from the theme's root size; the row gap is `.45rem`.
    #[test]
    fn spinner_sizes_follow_the_theme_type_scale() {
        let theme = theme();
        let (app, root) = render("indexing", 1);
        let row = spinner_row(&app, root);
        let texts = drawn_texts(&app, row);
        assert_eq!(texts[0].size, theme.metrics.body_size);
        assert_eq!(texts[0].color, theme.text);
        assert_eq!(texts[1].size, theme.metrics.rem(0.78));
        assert_eq!(texts[1].color, theme.highlight);
        let node = app.world().get::<Node>(row).expect("row node");
        assert_eq!(node.column_gap, Val::Px(theme.metrics.rem(0.45)));
    }

    /// The ring images are an annulus and its top quarter only.
    #[test]
    fn ring_arc_is_the_top_quarter_of_the_annulus() {
        let side = RING_PX as u32 * RING_TEXELS;
        let alpha = |image: &Image, x: u32, y: u32| {
            image.data.as_ref().expect("pixels")[((y * side + x) * 4 + 3) as usize]
        };
        let ring = ring_image(false);
        let arc = ring_image(true);
        let mid = side / 2;
        let stroke = RING_TEXELS;
        assert_eq!(alpha(&ring, mid, stroke), 255, "top of the annulus");
        assert_eq!(alpha(&arc, mid, stroke), 255, "top quarter");
        assert_eq!(alpha(&ring, stroke, mid), 255, "left of the annulus");
        assert_eq!(alpha(&arc, stroke, mid), 0, "left is not the top quarter");
        assert_eq!(alpha(&ring, mid, mid), 0, "the centre is open");
    }
}
