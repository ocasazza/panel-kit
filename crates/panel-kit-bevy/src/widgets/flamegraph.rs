//! Bevy UI port of the web `flamegraph` painter (`src/widgets/charts.rs`).
//!
//! One row per preorder span, as the `.pk-flamegraph` CSS lays them out: a
//! column with a 1px gap at `.68rem`, each frame indented by `depth * 1.25ch`
//! and ending at its share of the summed root values.

use bevy::prelude::*;
use panel_kit_core::widgets::charts::FlameSpanModel;

use super::TextStyle;
use crate::theme::BevyTheme;

/// `.pk-flame-frame` indent per depth level, in `ch`.
const INDENT_CH: f32 = 1.25;
/// `.pk-flame-frame` `min-width`, in `ch`.
const MIN_WIDTH_CH: f32 = 3.0;
/// `.pk-flame-frame` left border width in logical pixels.
const BORDER_LEFT: f32 = 2.0;
/// `.pk-flamegraph` row gap in logical pixels.
const ROW_GAP: f32 = 1.0;
/// Share of the accent colour in the frame background (`color-mix` 35%).
const ACCENT_MIX: f32 = 0.35;

/// Marks the chart container node.
#[derive(Component)]
struct FlameChart;
/// Marks one frame (the node carrying background and left border).
#[derive(Component)]
struct FlameFrame;

/// Per-channel sRGB mix: `accent` at `amount`, the rest `base`
/// (`color-mix(in srgb, accent amount, base)`).
fn mix_srgb(accent: Color, base: Color, amount: f32) -> Color {
    let a = accent.to_srgba();
    let b = base.to_srgba();
    let lerp = |from: f32, to: f32| from + (to - from) * amount;
    Color::srgba(
        lerp(b.red, a.red),
        lerp(b.green, a.green),
        lerp(b.blue, a.blue),
        lerp(b.alpha, a.alpha),
    )
}

/// A frame's share of the summed root values as the web prints it
/// (`--flame-w:{:.1}%`): clamped to `0..=1`, rounded to one decimal percent.
fn share_percent(value: f64, total: f64) -> f32 {
    let share = if total > 0.0 {
        (value / total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((share * 1000.0).round() / 10.0) as f32
}

/// Spawn the flamegraph for preorder `spans`; empty input spawns nothing.
pub fn spawn(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, spans: &[FlameSpanModel]) {
    if spans.is_empty() {
        return;
    }

    let total: f64 = spans
        .iter()
        .filter(|span| span.depth == 0)
        .map(|span| span.value)
        .sum();
    let label_style = TextStyle::rem(theme, 0.68);
    let value_style = label_style.color(theme.text_muted);
    let ch = label_style.size * crate::fonts::ch_em();
    let pad_x = theme.metrics.rem(0.35);
    let pad_y = theme.metrics.rem(0.05);

    parent
        .spawn((
            FlameChart,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(ROW_GAP),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .with_children(|chart| {
            for span in spans {
                let accent = span
                    .color
                    .map(|(r, g, b)| Color::srgb_u8(r, g, b))
                    .unwrap_or(theme.highlight);
                // The wrapper ends at the share boundary; the spacer indents
                // the frame, as `width: share - depth * 1.25ch` does in CSS.
                chart
                    .spawn(Node {
                        width: Val::Percent(share_percent(span.value, total)),
                        flex_direction: FlexDirection::Row,
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn(Node {
                            width: Val::Px(f32::from(span.depth) * INDENT_CH * ch),
                            flex_shrink: 0.0,
                            ..default()
                        });
                        line.spawn((
                            FlameFrame,
                            Node {
                                flex_grow: 1.0,
                                min_width: Val::Px(MIN_WIDTH_CH * ch),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                column_gap: Val::Px(theme.metrics.rem(0.5)),
                                padding: UiRect::axes(Val::Px(pad_x), Val::Px(pad_y)),
                                border: UiRect::left(Val::Px(BORDER_LEFT)),
                                ..default()
                            },
                            BackgroundColor(mix_srgb(accent, theme.background, ACCENT_MIX)),
                            BorderColor(accent),
                        ))
                        .with_children(|frame| {
                            // Bevy clips a node's descendants, not its own
                            // text, so the shrinking box holds the label.
                            frame
                                .spawn(Node {
                                    min_width: Val::Px(0.0),
                                    height: Val::Px(label_style.line_box()),
                                    overflow: Overflow::clip(),
                                    ..default()
                                })
                                .with_children(|clip| {
                                    clip.spawn((
                                        Text::new(span.label.clone()),
                                        label_style.font(),
                                        TextColor(label_style.color),
                                        TextLayout::new_with_no_wrap(),
                                        Node {
                                            flex_shrink: 0.0,
                                            height: Val::Px(label_style.line_box()),
                                            ..default()
                                        },
                                    ));
                                });
                            frame.spawn((
                                Text::new(format!("{}", span.value)),
                                value_style.font(),
                                TextColor(value_style.color),
                                TextLayout::new_with_no_wrap(),
                                Node {
                                    flex_shrink: 0.0,
                                    height: Val::Px(value_style.line_box()),
                                    ..default()
                                },
                            ));
                        });
                    });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(label: &str, depth: u16, value: f64) -> FlameSpanModel {
        FlameSpanModel { label: label.into(), depth, value, color: None }
    }

    fn app_for(theme: BevyTheme, spans: Vec<FlameSpanModel>) -> App {
        let mut app = App::new();
        let root = app.world_mut().spawn(Node::default()).id();
        app.add_systems(Startup, move |mut commands: Commands| {
            commands
                .entity(root)
                .with_children(|parent| spawn(parent, &theme, &spans));
        });
        app.update();
        app
    }

    fn children_of(world: &World, entity: Entity) -> Vec<Entity> {
        world.get::<Children>(entity).map(|c| c.to_vec()).unwrap_or_default()
    }

    /// `(wrapper, spacer, frame)` per span in order.
    fn lines(world: &mut World) -> Vec<(Entity, Entity, Entity)> {
        let mut query = world.query_filtered::<Entity, With<FlameChart>>();
        let Some(chart) = query.iter(world).next() else {
            return Vec::new();
        };
        children_of(world, chart)
            .into_iter()
            .map(|wrapper| {
                let parts = children_of(world, wrapper);
                (wrapper, parts[0], parts[1])
            })
            .collect()
    }

    fn themed() -> BevyTheme {
        let mut theme = BevyTheme::default();
        theme.metrics.rem = 20.0;
        theme.metrics.body_size = 20.0;
        theme
    }

    #[test]
    fn empty_spans_spawn_nothing() {
        let mut app = app_for(BevyTheme::default(), Vec::new());
        assert!(lines(app.world_mut()).is_empty());
    }

    /// Share is value over summed roots, rounded to one decimal percent.
    #[test]
    fn width_is_root_share_rounded_to_a_tenth() {
        let mut app = app_for(
            BevyTheme::default(),
            vec![span("a", 0, 3.0), span("b", 1, 1.0), span("c", 0, 4.0)],
        );
        let world = app.world_mut();
        let widths: Vec<Val> = lines(world)
            .iter()
            .map(|(wrapper, _, _)| world.get::<Node>(*wrapper).unwrap().width)
            .collect();
        assert_eq!(
            widths,
            [Val::Percent(42.9), Val::Percent(14.3), Val::Percent(57.1)]
        );
    }

    #[test]
    fn zero_total_and_overshare_clamp() {
        assert_eq!(share_percent(1.0, 0.0), 0.0);
        assert_eq!(share_percent(5.0, 2.0), 100.0);
        assert_eq!(share_percent(-1.0, 2.0), 0.0);
    }

    /// Indent, min width, padding, gap and text size follow the theme rem.
    #[test]
    fn geometry_follows_theme_rem() {
        let theme = themed();
        let mut app = app_for(theme.clone(), vec![span("root", 0, 1.0), span("kid", 2, 1.0)]);
        let world = app.world_mut();
        let rows = lines(world);
        let size = theme.metrics.rem(0.68);
        let ch = size * crate::fonts::ch_em();

        let spacer = world.get::<Node>(rows[1].1).unwrap();
        assert_eq!(spacer.width, Val::Px(2.0 * 1.25 * ch));
        let frame = world.get::<Node>(rows[1].2).unwrap();
        assert_eq!(frame.min_width, Val::Px(3.0 * ch));
        assert_eq!(frame.column_gap, Val::Px(theme.metrics.rem(0.5)));
        assert_eq!(
            frame.padding,
            UiRect::axes(Val::Px(theme.metrics.rem(0.35)), Val::Px(theme.metrics.rem(0.05)))
        );
        assert_eq!(frame.border, UiRect::left(Val::Px(2.0)));
        let clip = children_of(world, rows[1].2)[0];
        let label = children_of(world, clip)[0];
        assert_eq!(world.get::<TextFont>(label).unwrap().font_size, size);
    }

    /// Fill is `color-mix(accent 35%, background)`, border is the accent,
    /// label is text and value is muted; a span colour overrides highlight.
    #[test]
    fn colours_follow_theme_and_span_override() {
        let theme = BevyTheme::default();
        let mut app = app_for(
            theme.clone(),
            vec![
                span("plain", 0, 1.0),
                FlameSpanModel { label: "hot".into(), depth: 0, value: 1.0, color: Some((200, 0, 0)) },
            ],
        );
        let world = app.world_mut();
        let rows = lines(world);

        let accent = world.get::<BorderColor>(rows[0].2).unwrap().0;
        assert_eq!(accent, theme.highlight);
        let fill = world.get::<BackgroundColor>(rows[0].2).unwrap().0.to_srgba();
        let (h, b) = (theme.highlight.to_srgba(), theme.background.to_srgba());
        assert!((fill.green - (b.green + (h.green - b.green) * 0.35)).abs() < 1e-6);
        assert_eq!(world.get::<BorderColor>(rows[1].2).unwrap().0, Color::srgb_u8(200, 0, 0));

        let parts = children_of(world, rows[0].2);
        let label = children_of(world, parts[0])[0];
        assert_eq!(world.get::<TextColor>(label).unwrap().0, theme.text);
        assert_eq!(world.get::<TextColor>(parts[1]).unwrap().0, theme.text_muted);
    }

    /// The value prints as the raw `f64` Display string, like the web.
    #[test]
    fn value_prints_raw_f64() {
        let mut app = app_for(BevyTheme::default(), vec![span("s", 0, 35.672404120799996)]);
        let world = app.world_mut();
        let frame = lines(world)[0].2;
        let value = children_of(world, frame)[1];
        assert_eq!(world.get::<Text>(value).unwrap().0, "35.672404120799996");
    }
}
