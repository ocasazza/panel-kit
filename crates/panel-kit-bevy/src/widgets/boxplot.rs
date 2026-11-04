//! Bevy UI port of panel-kit core's `boxplot` component.
//!
//! One row per distribution: a label, then a track holding the whisker span
//! (`min..max`), the interquartile box (`q1..q3`), and the median tick.
//! Every row shares one value scale, so boxes in the same chart are directly
//! comparable; the mapping mirrors the web painter's track percentages.
//!
//! Row layout (the order the tests rely on):
//!
//! ```text
//! figure column
//! └─ figcaption
//! └─ list ─┬─ item ─┬─ label cell ─ text
//!           │        ├─ track ─┬─ whisker ─── end caps
//!           │        │         ├─ interquartile box
//!           │        │         └─ median tick
//!           │        └─ summary row (spans both columns)
//!           └─ …
//! ```

use bevy::prelude::*;
use panel_kit_core::badge::Rgb;
use crate::theme::BevyTheme;
use panel_kit_core::widgets::charts::{five_num, BoxItemModel, BoxItemView, FiveNum};

use super::{styled_text, series_tints, TextStyle};

/// Spawn one box-and-whisker row per borrowed five-number summary.
///
/// The summaries share a single value scale, widened to one unit when every
/// value is identical. Rows stack in a full-width column container; an empty
/// slice spawns nothing at all.
pub fn spawn(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, items: &[BoxItemView<'_>]) {
    let Some(span) = shared_span(items) else {
        return;
    };

    let m = &theme.metrics;
    let row_gap = m.rem(0.35);

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(m.rem(0.4)),
            ..default()
        })
        .with_children(|figure| {
            // figcaption
            styled_text(
                figure,
                "Distribution",
                TextStyle {
                    size: m.rem(0.62),
                    line_height: 1.5,
                    uppercase: true,
                    tracking: m.label_tracking,
                    color: theme.text_muted,
                    ..TextStyle::body(theme)
                },
            );

            // list
            figure
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(row_gap),
                    ..default()
                })
                .with_children(|list| {
                    for (index, item) in items.iter().enumerate() {
                        let ink = ink(theme, item.color, index);
                        spawn_row(list, theme, item.label, &item.summary, ink, span);
                    }
                });
        });
}

/// Spawn rows from raw samples, summarising each with [`five_num`].
///
/// A distribution with no samples has no summary, so it contributes neither a
/// row nor a value to the shared scale.
pub fn spawn_models(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, models: &[BoxItemModel]) {
    let items: Vec<BoxItemView<'_>> = models
        .iter()
        .filter_map(|model| {
            five_num(&model.samples).map(|summary| BoxItemView {
                label: &model.label,
                summary,
                color: model.color,
            })
        })
        .collect();

    spawn(parent, theme, &items);
}

fn value_text(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn summary_text(summary: &FiveNum) -> String {
    format!(
        "min {}, q1 {}, median {}, q3 {}, max {}",
        value_text(summary.min),
        value_text(summary.q1),
        value_text(summary.median),
        value_text(summary.q3),
        value_text(summary.max),
    )
}

/// One `.pk-box-item`: `.66rem` label in a `6rem` column, `.5rem` gap, a
/// `1.1rem` track, then the `.6rem` summary line.
fn spawn_row(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    label: &str,
    summary: &FiveNum,
    ink: Color,
    span: (f64, f64),
) {
    let m = &theme.metrics;
    let label_size = m.rem(0.66);
    let summary_size = m.rem(0.6);
    let track_height = m.rem(1.1);
    let label_width = m.rem(6.0);
    let col_gap = m.rem(0.5);
    // Item: label + track row, then the summary line (grid row 2 on the web).
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|item| {
            item.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(col_gap),
                ..default()
            })
            .with_children(|line| {
                line.spawn(Node {
                    width: Val::Px(label_width),
                    flex_shrink: 0.0,
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|cell| {
                    styled_text(
                        cell,
                        label,
                        TextStyle {
                            size: label_size,
                            color: theme.text,
                            ..TextStyle::body(theme)
                        },
                    );
                });

                line.spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        height: Val::Px(track_height),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(theme.background),
                    BorderColor(theme.border),
                ))
                .with_children(|track| {
                    spawn_whisker(track, summary, ink, span);
                    spawn_interquartile(track, summary, ink, span);
                    spawn_median(track, summary, ink, span);
                });
            });

            styled_text(
                item,
                summary_text(summary),
                TextStyle {
                    size: summary_size,
                    line_height: 1.2,
                    color: theme.text_muted,
                    ..TextStyle::body(theme)
                },
            );
        });
}

/// The whisker from min to max at half the interior height, with 1px×7px end
/// caps matching the web `.pk-box-whisker::before,::after`.
fn spawn_whisker(
    parent: &mut ChildSpawnerCommands,
    summary: &FiveNum,
    ink: Color,
    span: (f64, f64),
) {
    let min = percent_on_scale(summary.min, span);
    let max = percent_on_scale(summary.max, span);

    let width = (max - min).max(0.0);

    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(min),
                top: Val::Percent(50.0),
                width: Val::Percent(width),
                height: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(ink),
        ))
        .with_children(|whisker| {
            // Start cap
            whisker.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-3.0),
                    left: Val::Px(0.0),
                    width: Val::Px(1.0),
                    height: Val::Px(7.0),
                    ..default()
                },
                BackgroundColor(ink),
            ));
            // End cap
            whisker.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-3.0),
                    right: Val::Px(0.0),
                    width: Val::Px(1.0),
                    height: Val::Px(7.0),
                    ..default()
                },
                BackgroundColor(ink),
            ));
        });
}

/// The interquartile box between Q1 and Q3, inset 1px from top/bottom.
fn spawn_interquartile(
    parent: &mut ChildSpawnerCommands,
    summary: &FiveNum,
    ink: Color,
    span: (f64, f64),
) {
    let q1 = percent_on_scale(summary.q1, span);
    let q3 = percent_on_scale(summary.q3, span);

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(q1),
            top: Val::Px(1.0),
            bottom: Val::Px(1.0),
            width: Val::Percent((q3 - q1).max(0.0)),
            border: UiRect::horizontal(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(ink.with_alpha(0.55)),
        BorderColor(ink),
    ));
}

/// The median tick spans the full interior height, with its left edge at
/// `median - 1px` matching CSS `left:calc(var(--box-median) - 1px)`.
fn spawn_median(
    parent: &mut ChildSpawnerCommands,
    summary: &FiveNum,
    ink: Color,
    span: (f64, f64),
) {
    let median = percent_on_scale(summary.median, span);

    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(median),
            top: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(2.0),
            margin: UiRect::left(Val::Px(-1.0)),
            ..default()
        },
        BackgroundColor(ink),
    ));
}

/// The value scale every box in the chart shares.
///
/// Non-finite summaries are skipped so one broken sample cannot poison the
/// scale for its neighbours. A chart whose values are all equal is widened by
/// one unit, matching the web and terminal painters, so its boxes still have a
/// place to draw.
fn shared_span(items: &[BoxItemView<'_>]) -> Option<(f64, f64)> {
    if items.is_empty() {
        return None;
    }

    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for item in items {
        let (min, max) = (item.summary.min, item.summary.max);
        if !min.is_finite() || !max.is_finite() {
            continue;
        }
        lo = lo.min(min);
        hi = hi.max(max);
    }

    if lo > hi {
        (lo, hi) = (0.0, 1.0);
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi = lo + 1.0;
    }

    Some((lo, hi))
}

/// Place one value on the shared scale as a clamped track percentage.
fn percent_on_scale(value: f64, (lo, hi): (f64, f64)) -> f32 {
    if !value.is_finite() || hi <= lo {
        return 0.0;
    }

    let ratio = ((value - lo) / (hi - lo)).clamp(0.0, 1.0);
    (ratio * 100.0) as f32
}

/// The ink for one distribution: an explicit colour, or the next palette slot.
fn ink(theme: &BevyTheme, color: Option<Rgb>, index: usize) -> Color {
    match color {
        Some((r, g, b)) => Color::srgb_u8(r, g, b),
        None => {
            let tints = series_tints(theme);
            tints[index % tints.len()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::text::LineHeight;

    #[derive(Resource)]
    enum Rows {
        Views(Vec<BoxItemView<'static>>),
        Models(Vec<BoxItemModel>),
    }

    #[derive(Resource)]
    struct Root(Entity);

    /// Non-default root size so rem-derived values cannot match a literal.
    fn test_theme() -> BevyTheme {
        let mut theme = BevyTheme::default();
        theme.metrics.rem = 20.0;
        theme.metrics.body_size = 20.0;
        theme
    }

    fn build(mut commands: Commands, root: Res<Root>, rows: Res<Rows>) {
        let theme = test_theme();
        commands
            .entity(root.0)
            .with_children(move |parent| match &*rows {
                Rows::Views(views) => spawn(parent, &theme, views),
                Rows::Models(models) => spawn_models(parent, &theme, models),
            });
    }

    fn render(rows: Rows) -> App {
        let mut app = App::new();
        let root = app.world_mut().spawn(Node::default()).id();
        app.insert_resource(Root(root));
        app.insert_resource(rows);
        app.add_systems(Startup, build);
        app.update();
        app
    }

    /// The figure entity, then the list inside it.
    fn figure_entity(app: &App) -> Entity {
        let world = app.world();
        let root = world.resource::<Root>().0;
        world
            .get::<Children>(root)
            .expect("root has children")[0]
    }

    /// Rows in spawn order, read through the list container.
    fn rows_of(app: &App) -> Vec<Entity> {
        let world = app.world();
        let root = world.resource::<Root>().0;
        if world.get::<Children>(root).is_none() {
            return Vec::new();
        }
        let figure = figure_entity(app);
        let fig_children = world.get::<Children>(figure).expect("figure has children");
        // [0] = figcaption text, [1] = list
        let list = fig_children[1];

        world
            .get::<Children>(list)
            .map(|rows| rows.to_vec())
            .unwrap_or_default()
    }

    /// The label/track line of one item.
    fn line_of(app: &App, row: Entity) -> Entity {
        app.world().get::<Children>(row).expect("item children")[0]
    }

    fn track_of(app: &App, row: Entity) -> Entity {
        let line = line_of(app, row);
        app.world().get::<Children>(line).expect("line children")[1]
    }

    fn summary_of(app: &App, row: Entity) -> Entity {
        app.world().get::<Children>(row).expect("item children")[1]
    }

    fn label_entity(app: &App, row: Entity) -> Entity {
        let world = app.world();
        let cell = world.get::<Children>(line_of(app, row)).expect("line children")[0];
        world.get::<Children>(cell).expect("label cell holds its text")[0]
    }

    fn label_of(app: &App, row: Entity) -> String {
        app.world()
            .get::<Text>(label_entity(app, row))
            .expect("label text")
            .0
            .clone()
    }

    struct Part {
        node: Node,
        color: Color,
    }

    /// The track's inner parts, in spawn order: whisker, interquartile box, median.
    fn parts_of(app: &App, row: Entity) -> (Part, Part, Part) {
        let world = app.world();
        let track = track_of(app, row);
        let entities = world.get::<Children>(track).expect("track holds its parts");

        (
            part(world, entities[0]),
            part(world, entities[1]),
            part(world, entities[2]),
        )
    }

    fn part(world: &World, entity: Entity) -> Part {
        Part {
            node: world.get::<Node>(entity).expect("part node").clone(),
            color: world
                .get::<BackgroundColor>(entity)
                .expect("part colour")
                .0,
        }
    }

    fn assert_color(actual: Color, expected: Color) {
        assert_eq!(actual.to_srgba(), expected.to_srgba());
    }

    fn assert_percent(value: Val, expected: f32) {
        let Val::Percent(actual) = value else {
            panic!("expected a percentage, got {value:?}");
        };
        assert!(
            (actual - expected).abs() < 1e-3,
            "expected {expected}%, got {actual}%"
        );
    }

    fn summary(min: f64, q1: f64, median: f64, q3: f64, max: f64) -> FiveNum {
        FiveNum {
            min,
            q1,
            median,
            q3,
            max,
        }
    }

    fn view(label: &'static str, s: FiveNum, color: Option<Rgb>) -> BoxItemView<'static> {
        BoxItemView {
            label,
            summary: s,
            color,
        }
    }

    fn model(label: &str, samples: Vec<f64>) -> BoxItemModel {
        BoxItemModel {
            label: label.to_string(),
            samples,
            color: None,
        }
    }

    #[test]
    fn five_num_vectors_keep_tui_parity() {
        assert_eq!(
            five_num(&[7.0, 1.0, 3.0, 9.0, 5.0]),
            Some(summary(1.0, 3.0, 5.0, 7.0, 9.0))
        );
        assert_eq!(
            five_num(&[10.0, 30.0, 20.0, 40.0]),
            Some(summary(10.0, 17.5, 25.0, 32.5, 40.0))
        );
    }

    #[test]
    fn empty_samples_and_empty_charts_spawn_no_rows() {
        let app = render(Rows::Models(vec![model("empty", Vec::new())]));
        assert!(rows_of(&app).is_empty());

        let app = render(Rows::Views(Vec::new()));
        assert!(rows_of(&app).is_empty());
    }

    #[test]
    fn rows_keep_input_order_with_one_row_per_distribution() {
        let app = render(Rows::Models(vec![
            model("ingest", vec![1.0, 2.0]),
            model("eval", vec![3.0, 4.0]),
            model("render", vec![5.0, 6.0]),
        ]));

        let rows = rows_of(&app);
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows.iter()
                .map(|&row| label_of(&app, row))
                .collect::<Vec<_>>(),
            vec!["ingest", "eval", "render"]
        );
    }

    /// Label font size is `0.66rem`; colour is `theme.text`.
    #[test]
    fn label_size_is_066rem_and_color_is_text() {
        let theme = test_theme();
        let app = render(Rows::Models(vec![model("p95", vec![1.0, 2.0, 3.0])]));
        let label = label_entity(&app, rows_of(&app)[0]);

        assert_eq!(label_of(&app, rows_of(&app)[0]), "p95");
        assert_eq!(
            app.world()
                .get::<TextFont>(label)
                .expect("label font")
                .font_size,
            theme.metrics.rem(0.66)
        );
        assert_color(
            app.world()
                .get::<TextColor>(label)
                .expect("label colour")
                .0,
            theme.text,
        );
    }

    /// Track height is `1.1rem` (`--box-track height`).
    #[test]
    fn track_height_is_11rem() {
        let theme = test_theme();
        let app = render(Rows::Views(vec![view("a", summary(0.0, 1.0, 2.0, 3.0, 4.0), None)]));
        let row = rows_of(&app)[0];
        let world = app.world();
        let track = track_of(&app, row);
        let track_node = world.get::<Node>(track).expect("track node");

        assert_eq!(track_node.height, Val::Px(theme.metrics.rem(1.1)));
        assert_color(
            world.get::<BackgroundColor>(track).expect("track bg").0,
            theme.background,
        );
        assert_color(
            world.get::<BorderColor>(track).expect("track border").0,
            theme.border,
        );
    }

    /// Summary text uses `0.6rem`, `line-height:1.2`, `text_muted`.
    #[test]
    fn summary_uses_06rem_and_text_muted() {
        let theme = test_theme();
        let app = render(Rows::Views(vec![view("a", summary(1.0, 2.0, 3.0, 4.0, 5.0), None)]));
        let row = rows_of(&app)[0];
        let world = app.world();
        let summary_entity = summary_of(&app, row);

        let font = world
            .get::<TextFont>(summary_entity)
            .expect("summary font");
        assert_eq!(font.font_size, theme.metrics.rem(0.6));
        assert!(matches!(font.line_height, LineHeight::RelativeToFont(h) if h == 1.2));
        assert_color(
            world
                .get::<TextColor>(summary_entity)
                .expect("summary colour")
                .0,
            theme.text_muted,
        );
    }

    #[test]
    fn boxes_share_one_scale_across_the_chart() {
        let app = render(Rows::Views(vec![
            view("a", summary(0.0, 2.5, 5.0, 7.5, 10.0), None),
            view("b", summary(5.0, 7.5, 10.0, 12.5, 15.0), None),
        ]));
        let rows = rows_of(&app);

        let (a_whisker, a_range, a_median) = parts_of(&app, rows[0]);
        assert_percent(a_whisker.node.left, 0.0);
        assert_percent(a_whisker.node.width, 66.667);
        assert_percent(a_range.node.left, 16.667);
        assert_percent(a_range.node.width, 33.333);
        assert_percent(a_median.node.left, 33.333);

        let (b_whisker, b_range, b_median) = parts_of(&app, rows[1]);
        assert_percent(b_whisker.node.left, 33.333);
        assert_percent(b_whisker.node.width, 66.667);
        assert_percent(b_range.node.left, 50.0);
        assert_percent(b_range.node.width, 33.333);
        assert_percent(b_median.node.left, 66.667);
    }

    #[test]
    fn values_outside_the_shared_span_clamp_to_the_track() {
        let app = render(Rows::Views(vec![view(
            "odd",
            summary(0.0, -5.0, 5.0, 15.0, 10.0),
            None,
        )]));
        let rows = rows_of(&app);
        let (whisker, range, median) = parts_of(&app, rows[0]);

        assert_percent(whisker.node.left, 0.0);
        assert_percent(whisker.node.width, 100.0);
        assert_percent(range.node.left, 0.0);
        assert_percent(range.node.width, 100.0);
        assert_percent(median.node.left, 50.0);
    }

    #[test]
    fn flat_distributions_widen_to_a_unit_span() {
        let app = render(Rows::Models(vec![model("steady", vec![5.0, 5.0, 5.0])]));
        let rows = rows_of(&app);
        let (whisker, range, median) = parts_of(&app, rows[0]);

        assert_percent(whisker.node.left, 0.0);
        assert_percent(whisker.node.width, 0.0);
        assert_percent(range.node.left, 0.0);
        assert_percent(range.node.width, 0.0);
        assert_percent(median.node.left, 0.0);
    }

    #[test]
    fn explicit_colour_overrides_the_palette_and_palette_cycles() {
        let app = render(Rows::Views(vec![
            view("first", summary(0.0, 1.0, 2.0, 3.0, 4.0), None),
            view("accent", summary(0.0, 1.0, 2.0, 3.0, 4.0), Some((10, 20, 30))),
            view("third", summary(0.0, 1.0, 2.0, 3.0, 4.0), None),
        ]));
        let rows = rows_of(&app);

        let theme = test_theme();
        let (_, first_range, first_median) = parts_of(&app, rows[0]);
        assert_color(first_median.color, theme.text);
        assert_color(first_range.color, theme.text.with_alpha(0.55));

        let (accent_whisker, accent_range, accent_median) = parts_of(&app, rows[1]);
        let accent = Color::srgb_u8(10, 20, 30);
        assert_color(accent_whisker.color, accent);
        assert_color(accent_range.color, accent.with_alpha(0.55));
        assert_color(accent_median.color, accent);

        let (_, _, third_median) = parts_of(&app, rows[2]);
        assert_color(third_median.color, theme.secondary);
    }

    #[test]
    fn non_finite_samples_do_not_poison_the_scale() {
        let app = render(Rows::Models(vec![
            model("broken", vec![f64::NAN]),
            model("healthy", vec![0.0, 10.0]),
        ]));
        let rows = rows_of(&app);
        assert_eq!(rows.len(), 2);

        let (_, _, broken_median) = parts_of(&app, rows[0]);
        assert_percent(broken_median.node.left, 0.0);

        let (whisker, range, median) = parts_of(&app, rows[1]);
        assert_percent(whisker.node.width, 100.0);
        assert_percent(range.node.left, 25.0);
        assert_percent(median.node.left, 50.0);
    }

    /// The whisker has 1px×7px end caps at top:-3px, matching the web.
    #[test]
    fn whisker_has_end_caps() {
        let app = render(Rows::Views(vec![view("a", summary(1.0, 2.0, 3.0, 4.0, 5.0), None)]));
        let row = rows_of(&app)[0];
        let world = app.world();
        let track = track_of(&app, row);
        let track_children = world.get::<Children>(track).expect("track children");
        let whisker = track_children[0];
        let caps = world
            .get::<Children>(whisker)
            .expect("whisker has end caps");

        assert_eq!(caps.len(), 2, "whisker has two end caps");
        for entity in caps.iter() {
            let node = world.get::<Node>(entity).expect("cap node");
            assert_eq!(node.width, Val::Px(1.0));
            assert_eq!(node.height, Val::Px(7.0));
            assert_eq!(node.top, Val::Px(-3.0));
        }
    }

    /// The figcaption is `0.62rem` uppercase tracked text.
    #[test]
    fn figcaption_is_tracked_uppercase() {
        let theme = test_theme();
        let app = render(Rows::Views(vec![view("a", summary(0.0, 1.0, 2.0, 3.0, 4.0), None)]));
        let figure = figure_entity(&app);
        let world = app.world();
        let caption = world.get::<Children>(figure).expect("figure children")[0];
        let glyphs = world.get::<Children>(caption).expect("tracked caption is a glyph row");
        let text: String = glyphs
            .iter()
            .map(|g| world.get::<Text>(g).expect("glyph text").0.clone())
            .collect();
        assert_eq!(text, "DISTRIBUTION");

        let first = glyphs[0];
        assert_eq!(
            world.get::<TextFont>(first).expect("caption font").font_size,
            theme.metrics.rem(0.62)
        );
        assert_color(
            world.get::<TextColor>(first).expect("caption colour").0,
            theme.text_muted,
        );
        assert_eq!(
            world.get::<Node>(caption).expect("caption node").column_gap,
            Val::Px(theme.metrics.label_tracking * theme.metrics.rem(0.62))
        );
    }
}