//! Bevy UI port of the web `time_series` painter (`src/widgets/charts.rs`).
//!
//! The boxes follow the web CSS (`.pk-chart`, `.pk-chart-plot`,
//! `.pk-chart-scale`, `.pk-chart-xaxis`, `.pk-chart-legend*`): a column
//! figure whose plot is a two-column grid, `auto` scale column then a
//! `minmax(0, 1fr)` SVG area. Bevy UI has no polyline primitive, so
//! [`TimeSeriesPlugin`] rasterises every series into one image at the plot
//! box's physical pixel size, with the 1.5 logical px stroke the SVG's
//! `non-scaling-stroke` gives, and repaints it whenever that size changes.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use panel_kit_core::widgets::charts::SeriesView;

use super::{series_tints, styled_text, TextStyle};
use crate::theme::BevyTheme;

/// `stroke-width` of a series line, in logical pixels.
const STROKE_PX: f32 = 1.5;

/// One series as the rasteriser paints it.
struct PolylineSeries {
    points: Vec<(f64, f64)>,
    color: Color,
}

/// On the SVG area: the series to rasterise and the size last painted.
#[derive(Component)]
struct PolylinePlot {
    series: Vec<PolylineSeries>,
    x_span: Option<(f64, f64)>,
    y_span: Option<(f64, f64)>,
    painted: Option<(UVec2, f32)>,
}

/// The absolutely positioned child of a [`PolylinePlot`] that shows its image.
#[derive(Component)]
struct PolylineLayer;

/// Shared x span across every point, widened by one unit when flat.
fn x_span(series: &[SeriesView<'_>]) -> Option<(f64, f64)> {
    span(series.iter().flat_map(|item| item.points.iter().map(|point| point.0)))
}

/// Shared y span across every point, widened by one unit when flat.
fn y_span(series: &[SeriesView<'_>]) -> Option<(f64, f64)> {
    span(series.iter().flat_map(|item| item.points.iter().map(|point| point.1)))
}

fn span(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for value in values {
        lo = lo.min(value);
        hi = hi.max(value);
    }
    if lo > hi {
        return None;
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }

    Some((lo, hi))
}

/// Format an axis value without a trailing `.00`.
fn value_text(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

/// Format a value with its unit, skipping the space when there is none.
fn unit_text(value: f64, unit: &str) -> String {
    if unit.is_empty() {
        value_text(value)
    } else {
        format!("{} {unit}", value_text(value))
    }
}

/// Paint borrowed series as the web's line chart.
///
/// Every series shares one x span and one y span, so lines are comparable.
/// Add [`TimeSeriesPlugin`] to the app to see the lines; without it the
/// labels, axes and legend still paint.
pub fn spawn(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    series: &[SeriesView<'_>],
    unit: &str,
) {
    let metrics = &theme.metrics;
    let tints = series_tints(theme);
    let (xs, ys) = (x_span(series), y_span(series));
    let small = |factor: f32| TextStyle::rem(theme, factor).color(theme.text_muted);
    let axis_label = || small(0.6).line_height(1.0);
    let polylines = PolylinePlot {
        series: series
            .iter()
            .enumerate()
            .map(|(index, item)| PolylineSeries {
                points: item.points.to_vec(),
                color: tints[index % tints.len()],
            })
            .collect(),
        x_span: xs,
        y_span: ys,
        painted: None,
    };

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(metrics.rem(0.4)),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            min_height: Val::Px(0.0),
            ..default()
        })
        .with_children(|figure| {
            styled_text(
                figure,
                format!("Time series ({unit})"),
                small(0.62).tracked_caps(metrics.label_tracking),
            );

            figure
                .spawn(Node {
                    display: Display::Grid,
                    grid_template_columns: vec![
                        RepeatedGridTrack::auto(1),
                        RepeatedGridTrack::flex(1, 1.0),
                    ],
                    column_gap: Val::Px(metrics.rem(0.4)),
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    min_height: Val::Px(metrics.rem(3.5)),
                    ..default()
                })
                .with_children(|plot| {
                    // Max and min share grid cell 1/1, as `.pk-chart-scale` does.
                    for bound in ys.map(|(lo, hi)| [hi, lo]).into_iter().flatten() {
                        plot.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::FlexEnd,
                            grid_row: GridPlacement::start(1),
                            grid_column: GridPlacement::start(1),
                            ..default()
                        })
                        .with_children(|scale| {
                            let label = styled_text(scale, unit_text(bound, unit), axis_label());
                            scale.commands().entity(label).insert(TextLayout::new_with_no_wrap());
                        });
                    }
                    plot.spawn((
                        polylines,
                        Node {
                            grid_row: GridPlacement::start(1),
                            grid_column: GridPlacement::start(2),
                            min_height: Val::Px(0.0),
                            ..default()
                        },
                    ))
                    .with_children(|svg| {
                        svg.spawn((
                            PolylineLayer,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                        ));
                    });
                });

            figure
                .spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|axis| {
                    for bound in xs.map(|(lo, hi)| [lo, hi]).into_iter().flatten() {
                        styled_text(axis, value_text(bound), axis_label());
                    }
                });

            figure
                .spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(metrics.rem(0.15)),
                    column_gap: Val::Px(metrics.rem(0.6)),
                    ..default()
                })
                .with_children(|legend| {
                    for (index, item) in series.iter().enumerate() {
                        legend
                            .spawn(Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(metrics.rem(0.3)),
                                min_width: Val::Px(0.0),
                                ..default()
                            })
                            .with_children(|entry| {
                                entry.spawn((
                                    Node {
                                        width: Val::Px(metrics.rem(0.55)),
                                        height: Val::Px(metrics.rem(0.55)),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    BackgroundColor(tints[index % tints.len()]),
                                ));
                                let name = styled_text(entry, item.name, small(0.64));
                                entry.commands().entity(name).insert(TextLayout::new_with_no_wrap());
                            });
                    }
                });
        });
}

/// Paints the series lines into each time-series plot at its computed size.
pub struct TimeSeriesPlugin;

impl Plugin for TimeSeriesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, paint_polylines);
    }
}

/// Repaint a plot's image when its physical size or scale factor changed.
fn paint_polylines(
    mut commands: Commands,
    images: Option<ResMut<Assets<Image>>>,
    mut plots: Query<(&ComputedNode, &mut PolylinePlot, &Children)>,
    layers: Query<(), With<PolylineLayer>>,
) {
    let Some(mut images) = images else {
        return;
    };
    for (computed, mut plot, children) in &mut plots {
        let size = computed.size().round().as_uvec2();
        let scale = computed.inverse_scale_factor();
        if size.x == 0 || size.y == 0 || plot.painted == Some((size, scale)) {
            continue;
        }
        plot.painted = Some((size, scale));
        let (Some(xs), Some(ys)) = (plot.x_span, plot.y_span) else {
            continue;
        };

        let pixels = rasterize(&plot.series, xs, ys, size, STROKE_PX / scale);
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
        let handle = images.add(image);
        for layer in children.iter().filter(|child| layers.contains(*child)) {
            commands.entity(layer).insert(ImageNode::new(handle.clone()));
        }
    }
}

/// Rasterise `series` over `xs`/`ys` into straight-alpha sRGB RGBA8 pixels of
/// `size`, each line `stroke` pixels wide with round caps and joins.
///
/// A point maps to the plot box directly (x right, y down), which is what the
/// web's 320×140 viewBox stretched with `preserveAspectRatio="none"` yields.
fn rasterize(
    series: &[PolylineSeries],
    xs: (f64, f64),
    ys: (f64, f64),
    size: UVec2,
    stroke: f32,
) -> Vec<u8> {
    let (width, height) = (size.x as usize, size.y as usize);
    let (w, h) = (f64::from(size.x), f64::from(size.y));
    let half = f64::from(stroke) / 2.0;
    let mut over = vec![[0.0f32; 4]; width * height];
    let mut coverage = vec![0.0f32; width * height];

    for line in series {
        coverage.fill(0.0);
        let projected: Vec<(f64, f64)> = line
            .points
            .iter()
            .map(|&(x, y)| {
                (
                    (x - xs.0) / (xs.1 - xs.0) * w,
                    (1.0 - (y - ys.0) / (ys.1 - ys.0)) * h,
                )
            })
            .collect();
        for pair in projected.windows(2) {
            stroke_segment(&mut coverage, width, height, pair[0], pair[1], half);
        }

        let ink = line.color.to_srgba();
        for (pixel, &cover) in over.iter_mut().zip(&coverage) {
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

/// Raise `coverage` to each pixel's overlap with the capsule of radius `half`
/// around the segment `from`–`to`, approximated by its distance to the pixel
/// centre. Non-finite endpoints paint nothing.
fn stroke_segment(
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::text::LineHeight;
    use panel_kit_core::theme::ThemeTokens;
    use panel_kit_core::widgets::charts::SeriesModel;

    /// A theme whose rem, label tracking and colours differ from the
    /// defaults, so a literal in place of a token cannot pass.
    fn theme() -> BevyTheme {
        let mut tokens = ThemeTokens::dark();
        tokens.typography.body_size = 17.0;
        tokens.typography.label_tracking = 0.2;
        BevyTheme::from_tokens(&tokens)
    }

    fn model(name: &str, points: Vec<(f64, f64)>) -> SeriesModel {
        SeriesModel {
            name: name.to_string(),
            points,
        }
    }

    /// Spawn the chart under a root node and return the world.
    fn paint(theme: &BevyTheme, models: Vec<SeriesModel>, unit: &str) -> World {
        let (theme, unit) = (theme.clone(), unit.to_string());
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        world
            .run_system_once(move |mut commands: Commands| {
                let views: Vec<SeriesView<'_>> = models
                    .iter()
                    .map(|item| SeriesView {
                        name: &item.name,
                        points: &item.points,
                    })
                    .collect();
                commands
                    .entity(root)
                    .with_children(|parent| spawn(parent, &theme, &views, &unit));
            })
            .expect("the chart spawns");
        world
    }

    fn activity() -> Vec<SeriesModel> {
        vec![
            model("eval/ms", vec![(0.0, 19.24), (4.7, 34.3)]),
            model("frame/ms", vec![(0.0, 16.7), (4.7, 14.75)]),
        ]
    }

    /// Every text entity's string, font size, line height and colour, in
    /// spawn order.
    fn texts(world: &mut World) -> Vec<(String, f32, LineHeight, Color)> {
        let mut query = world.query::<(Entity, &Text, &TextFont, &TextColor)>();
        let mut found: Vec<_> = query
            .iter(world)
            .map(|(entity, text, font, color)| {
                (
                    entity,
                    (text.0.clone(), font.font_size, font.line_height, color.0),
                )
            })
            .collect();
        found.sort_by_key(|(entity, _)| *entity);
        found.into_iter().map(|(_, text)| text).collect()
    }

    /// The nodes whose layout matches `select`, in spawn order.
    fn nodes(world: &mut World, select: impl Fn(&Node) -> bool) -> Vec<Node> {
        let mut query = world.query::<(Entity, &Node)>();
        let mut found: Vec<_> = query
            .iter(world)
            .filter(|(_, node)| select(node))
            .map(|(entity, node)| (entity, node.clone()))
            .collect();
        found.sort_by_key(|(entity, _)| *entity);
        found.into_iter().map(|(_, node)| node).collect()
    }

    #[test]
    fn caption_is_the_tracked_muted_caps_figcaption() {
        let theme = theme();
        let mut world = paint(&theme, activity(), "ms");

        let glyphs: Vec<_> = texts(&mut world)
            .into_iter()
            .take("TIME SERIES (MS)".chars().count())
            .collect();
        let caption: String = glyphs.iter().map(|glyph| glyph.0.as_str()).collect();
        assert_eq!(caption, "TIME SERIES (MS)");
        for (_, size, line_height, color) in &glyphs {
            assert_eq!(*size, theme.metrics.rem(0.62));
            assert!(
                matches!(line_height, LineHeight::RelativeToFont(v) if *v == theme.metrics.line_height),
                "{line_height:?}"
            );
            assert_eq!(*color, theme.text_muted);
        }
        let tracked = nodes(&mut world, |node| {
            node.column_gap == Val::Px(theme.metrics.label_tracking * theme.metrics.rem(0.62))
        });
        assert_eq!(tracked.len(), 1);
    }

    #[test]
    fn figure_and_plot_boxes_follow_the_css_tokens() {
        let theme = theme();
        let mut world = paint(&theme, activity(), "ms");

        let figures = nodes(&mut world, |node| node.row_gap == Val::Px(theme.metrics.rem(0.4)));
        assert_eq!(figures.len(), 1);
        assert_eq!(figures[0].flex_direction, FlexDirection::Column);
        assert_eq!(figures[0].width, Val::Percent(100.0));
        assert_eq!(figures[0].height, Val::Percent(100.0));

        let plots = nodes(&mut world, |node| node.display == Display::Grid);
        assert_eq!(plots.len(), 1);
        assert_eq!(plots[0].column_gap, Val::Px(theme.metrics.rem(0.4)));
        assert_eq!(plots[0].min_height, Val::Px(theme.metrics.rem(3.5)));
        assert_eq!(plots[0].flex_grow, 1.0);
    }

    #[test]
    fn scale_and_axis_labels_are_muted_06rem_with_unit_line_height() {
        let theme = theme();
        let mut world = paint(&theme, activity(), "ms");

        let labels: Vec<_> = texts(&mut world)
            .into_iter()
            .filter(|(_, size, _, _)| *size == theme.metrics.rem(0.6))
            .collect();
        let shown: Vec<&str> = labels.iter().map(|label| label.0.as_str()).collect();
        assert_eq!(shown, ["34.30 ms", "14.75 ms", "0", "4.70"]);
        for (_, _, line_height, color) in &labels {
            assert!(matches!(line_height, LineHeight::RelativeToFont(v) if *v == 1.0), "{line_height:?}");
            assert_eq!(*color, theme.text_muted);
        }
    }

    #[test]
    fn scale_labels_share_one_grid_cell_right_aligned() {
        let theme = theme();
        let mut world = paint(&theme, activity(), "ms");

        let scales = nodes(&mut world, |node| {
            node.grid_column == GridPlacement::start(1) && node.grid_row == GridPlacement::start(1)
        });
        assert_eq!(scales.len(), 2);
        for scale in scales {
            assert_eq!(scale.flex_direction, FlexDirection::Column);
            assert_eq!(scale.justify_content, JustifyContent::SpaceBetween);
            assert_eq!(scale.align_items, AlignItems::FlexEnd);
        }
    }

    #[test]
    fn legend_boxes_and_swatches_follow_the_css_tokens() {
        let theme = theme();
        let mut world = paint(&theme, activity(), "ms");

        let legends = nodes(&mut world, |node| node.flex_wrap == FlexWrap::Wrap);
        assert_eq!(legends.len(), 1);
        assert_eq!(legends[0].row_gap, Val::Px(theme.metrics.rem(0.15)));
        assert_eq!(legends[0].column_gap, Val::Px(theme.metrics.rem(0.6)));
        let entries = nodes(&mut world, |node| node.column_gap == Val::Px(theme.metrics.rem(0.3)));
        assert_eq!(entries.len(), 2);

        let mut query = world.query::<(Entity, &Node, &BackgroundColor)>();
        let mut swatches: Vec<_> = query
            .iter(&world)
            .filter(|(_, _, background)| background.0 != Color::NONE)
            .collect();
        swatches.sort_by_key(|(entity, _, _)| *entity);
        let tints = series_tints(&theme);
        assert_eq!(swatches.len(), 2);
        for (index, (_, node, background)) in swatches.into_iter().enumerate() {
            assert_eq!(node.width, Val::Px(theme.metrics.rem(0.55)));
            assert_eq!(node.height, Val::Px(theme.metrics.rem(0.55)));
            assert_eq!(background.0, tints[index]);
        }

        let names: Vec<_> = texts(&mut world)
            .into_iter()
            .filter(|(_, size, _, _)| *size == theme.metrics.rem(0.64))
            .collect();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].0, "eval/ms");
        assert_eq!(names[1].0, "frame/ms");
        assert_eq!(names[0].3, theme.text_muted);
    }

    #[test]
    fn series_colours_cycle_through_the_six_tints() {
        let theme = theme();
        let models = (0..7)
            .map(|index| model(&format!("s{index}"), vec![(0.0, index as f64)]))
            .collect();
        let mut world = paint(&theme, models, "");

        let mut query = world.query::<&PolylinePlot>();
        let plot = query.single(&world).expect("one plot");
        let tints = series_tints(&theme);
        assert_eq!(plot.series.len(), 7);
        for (index, line) in plot.series.iter().enumerate() {
            assert_eq!(line.color, tints[index % 6]);
        }
    }

    #[test]
    fn empty_input_keeps_the_four_flow_boxes_and_paints_no_labels() {
        let theme = theme();
        let mut world = paint(&theme, vec![], "ms");

        let glyphs = texts(&mut world);
        assert_eq!(glyphs.len(), "TIME SERIES (MS)".chars().count());
        let mut query = world.query::<&PolylinePlot>();
        let plot = query.single(&world).expect("one plot");
        assert_eq!((plot.x_span, plot.y_span), (None, None));
    }

    #[test]
    fn spans_and_text_match_the_web_painter() {
        let series = [SeriesView {
            name: "a",
            points: &[(0.0, 5.0), (4.7, 5.0)],
        }];
        assert_eq!(x_span(&series), Some((0.0, 4.7)));
        assert_eq!(y_span(&series), Some((5.0, 6.0)));
        assert_eq!(value_text(5.0), "5");
        assert_eq!(value_text(34.3), "34.30");
        assert_eq!(unit_text(34.3, "ms"), "34.30 ms");
        assert_eq!(unit_text(7.0, ""), "7");
        assert_eq!(x_span(&[]), None);
    }

    fn alpha_at(pixels: &[u8], width: u32, x: u32, y: u32) -> u8 {
        pixels[((y * width + x) * 4 + 3) as usize]
    }

    /// One flat line at the vertical middle of a 20 px tall plot, plus a
    /// vertical edge series that only fixes the y span to 0..1.
    fn flat_line(stroke: f32) -> Vec<u8> {
        let ink = Color::srgb_u8(10, 20, 30);
        let series = [
            PolylineSeries {
                points: vec![(0.0, 0.5), (1.0, 0.5)],
                color: ink,
            },
            PolylineSeries {
                points: vec![(0.0, 0.0), (0.0, 1.0)],
                color: ink,
            },
        ];
        rasterize(&series, (0.0, 1.0), (0.0, 1.0), UVec2::new(40, 20), stroke)
    }

    #[test]
    fn stroke_covers_one_and_a_half_logical_pixels() {
        let column_alpha = |pixels: &[u8]| -> f32 {
            (0..20).map(|y| f32::from(alpha_at(pixels, 40, 20, y))).sum::<f32>() / 255.0
        };
        assert!((column_alpha(&flat_line(STROKE_PX)) - STROKE_PX).abs() < 0.02);
        // Scale factor 2: the same stroke is 3 physical pixels.
        assert!((column_alpha(&flat_line(STROKE_PX / 0.5)) - 3.0).abs() < 0.02);
    }

    #[test]
    fn rasterised_line_keeps_the_series_colour_and_flips_y() {
        let pixels = flat_line(STROKE_PX / 0.5);
        let at = |x: u32, y: u32| {
            let i = ((y * 40 + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        };
        assert_eq!(at(20, 9), [10, 20, 30, 255]);
        assert_eq!(at(20, 0)[3], 0);
        let rising = rasterize(
            &[PolylineSeries {
                points: vec![(0.0, 0.0), (1.0, 1.0)],
                color: Color::WHITE,
            }],
            (0.0, 1.0),
            (0.0, 1.0),
            UVec2::new(40, 20),
            STROKE_PX,
        );
        assert!(alpha_at(&rising, 40, 1, 19) > 0, "low y paints at the bottom");
        assert_eq!(alpha_at(&rising, 40, 1, 0), 0);
        assert!(alpha_at(&rising, 40, 38, 0) > 0, "high y paints at the top");
    }
}
