//! Bevy UI port of panel-kit core's `gauges` component.
//!
//! One row per [`GaugeModel`]: label | track+fill | value, matching the web
//! `.pk-gauges` grid layout at `.72rem`. The fill colour follows the CSS
//! `--meter-c` fallback chain: a model colour, else `--color-highlight`.

use bevy::prelude::*;
use panel_kit_core::widgets::charts::GaugeModel;

use super::TextStyle;
use crate::theme::BevyTheme;

/// Stack one row per [`GaugeModel`] in model order.
///
/// Every gauge row is a flex row: label (max 8rem), track (flex 1), value
/// (auto), with `.5rem` gap, matching the web `.pk-gauge` grid. The fill
/// colour is the model's colour or `theme.highlight`, and the ratio is
/// clamped to `0.0..=1.0` before it becomes a percentage width.
pub fn spawn(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, items: &[GaugeModel]) {
    let font = TextStyle::rem(theme, 0.72);
    let label_style = font.color(theme.text);
    let value_style = font.color(theme.text_muted);
    let gap = theme.metrics.rem(0.5);

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(theme.metrics.rem(0.45)),
            ..default()
        })
        .with_children(|column| {
            for item in items {
                column
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(gap),
                        ..default()
                    })
                    .with_children(|row| {
                        // Label cell: the grid column `minmax(0, 8rem)`, ellipsis-clipped.
                        row.spawn((
                            Text::new(item.label.clone()),
                            label_style.font(),
                            TextColor(label_style.color),
                            TextLayout::new_with_no_wrap(),
                            Node {
                                width: Val::Px(theme.metrics.rem(8.0)),
                                min_width: Val::Px(0.0),
                                height: Val::Px(label_style.line_box()),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                        ));

                        // Track: flex 1, .55rem tall, recessed background + 1px border.
                        let track_height = theme.metrics.rem(0.55);
                        let fill_percent = (item.ratio.clamp(0.0, 1.0) * 100.0) as f32;
                        let fill_color = theme.highlight;
                        row.spawn((
                            Node {
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                height: Val::Px(track_height),
                                border: UiRect::all(Val::Px(1.0)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(theme.background),
                            BorderColor(theme.border),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                Node {
                                    width: Val::Percent(fill_percent),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(fill_color),
                            ));
                        });

                        // Value: muted, no-wrap.
                        row.spawn((
                            Text::new(item.text.clone()),
                            value_style.font(),
                            TextColor(value_style.color),
                            TextLayout::new_with_no_wrap(),
                            Node {
                                height: Val::Px(value_style.line_box()),
                                ..default()
                            },
                        ));
                    });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(items: Vec<GaugeModel>) -> (App, Entity) {
        rendered_with(BevyTheme::default(), items)
    }

    fn rendered_with(theme: BevyTheme, items: Vec<GaugeModel>) -> (App, Entity) {
        let mut app = App::new();
        let root = app.world_mut().spawn(Node::default()).id();
        app.add_systems(Startup, move |mut commands: Commands| {
            commands
                .entity(root)
                .with_children(|parent| spawn(parent, &theme, &items));
        });
        app.update();
        (app, root)
    }

    fn gauge(label: &str, ratio: f64, text: &str) -> GaugeModel {
        GaugeModel { label: label.into(), ratio, text: text.into() }
    }

    fn children_of(app: &App, entity: Entity) -> Vec<Entity> {
        app.world().get::<Children>(entity).map(|c| c.to_vec()).unwrap_or_default()
    }

    /// Text contents in document order (depth-first through the hierarchy).
    fn texts(app: &App, root: Entity) -> Vec<String> {
        let mut found = Vec::new();
        collect_texts(app, root, &mut found);
        found
    }

    fn collect_texts(app: &App, entity: Entity, found: &mut Vec<String>) {
        if let Some(text) = app.world().get::<Text>(entity) {
            found.push(text.0.clone());
        }
        for child in children_of(app, entity) {
            collect_texts(app, child, found);
        }
    }

    /// The fill entity of the gauge at `index`. Root children: [column]. Column children are
    /// gauge rows. Each gauge row has [label, track, value]. The fill is the track's child.
    fn fill_of(app: &App, root: Entity, index: usize) -> Entity {
        let column = children_of(app, root)[0];
        let gauge_row = children_of(app, column)[index];
        let track = children_of(app, gauge_row)[1]; // label=0, track=1, value=2
        children_of(app, track)[0]
    }

    #[test]
    fn renders_label_and_usage_text_from_the_model() {
        let (app, root) = rendered(vec![gauge("queue", 0.375, "3/8")]);

        assert_eq!(texts(&app, root), ["queue", "3/8"]);

        let column = children_of(&app, root)[0];
        let gauge_row = children_of(&app, column)[0];
        let row_node = app.world().get::<Node>(gauge_row).expect("gauge row node");
        assert_eq!(row_node.flex_direction, FlexDirection::Row);
        assert_eq!(row_node.align_items, AlignItems::Center);

        let theme = BevyTheme::default();
        let row_children = children_of(&app, gauge_row);
        // Label: text colour, .72rem.
        assert_eq!(
            app.world().get::<TextColor>(row_children[0]).unwrap().0,
            theme.text
        );
        assert_eq!(
            app.world().get::<TextFont>(row_children[0]).unwrap().font_size,
            theme.metrics.rem(0.72)
        );
        // Track: background colour and border match theme tokens.
        assert_eq!(
            app.world().get::<BackgroundColor>(row_children[1]).unwrap().0,
            theme.background
        );
        assert_eq!(
            app.world().get::<BorderColor>(row_children[1]).unwrap().0,
            theme.border
        );
        // Value: muted colour, .72rem.
        assert_eq!(
            app.world().get::<TextColor>(row_children[2]).unwrap().0,
            theme.text_muted
        );
        assert_eq!(
            app.world().get::<TextFont>(row_children[2]).unwrap().font_size,
            theme.metrics.rem(0.72)
        );
    }

    #[test]
    fn ratio_is_clamped_into_the_fill_width() {
        let cases: [(f64, f32); 5] = [
            (-0.5, 0.0),
            (0.0, 0.0),
            (0.375, 37.5),
            (1.0, 100.0),
            (1.5, 100.0),
        ];
        let items = cases
            .iter()
            .map(|(ratio, _)| gauge("queue", *ratio, ""))
            .collect();
        let (app, root) = rendered(items);

        let column = children_of(&app, root)[0];
        assert_eq!(children_of(&app, column).len(), cases.len());

        for (index, (_, expected_percent)) in cases.iter().enumerate() {
            let node = app.world().get::<Node>(fill_of(&app, root, index)).unwrap();
            assert_eq!(node.width, Val::Percent(*expected_percent));
        }
    }

    #[test]
    fn track_is_recessed_with_full_height_fill_colored_highlight() {
        let (app, root) = rendered(vec![gauge("queue", 0.5, "50%")]);

        let column = children_of(&app, root)[0];
        let gauge_row = children_of(&app, column)[0];
        let track = children_of(&app, gauge_row)[1];
        let track_node = app.world().get::<Node>(track).unwrap();
        let theme = BevyTheme::default();
        assert_eq!(track_node.flex_grow, 1.0);
        assert_eq!(
            track_node.height,
            Val::Px(theme.metrics.rem(0.55))
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(track).unwrap().0,
            theme.background
        );

        let fill = fill_of(&app, root, 0);
        let fill_node = app.world().get::<Node>(fill).unwrap();
        assert_eq!(fill_node.height, Val::Percent(100.0));
        assert_eq!(
            app.world().get::<BackgroundColor>(fill).unwrap().0,
            theme.highlight
        );
    }

    #[test]
    fn gauge_order_follows_model_order() {
        let (app, root) = rendered(vec![
            gauge("vfs", 0.21, "31 / 148 files"),
            gauge("wasm", 0.63, "6.3 MB / 10 MB"),
            gauge("events", 0.78, "78% queue"),
            gauge("layout", 0.94, "94% stress"),
        ]);

        let expected = [
            ("vfs", "31 / 148 files"),
            ("wasm", "6.3 MB / 10 MB"),
            ("events", "78% queue"),
            ("layout", "94% stress"),
        ];
        let flattened: Vec<&str> = expected.into_iter().flat_map(|(l, t)| [l, t]).collect();

        assert_eq!(texts(&app, root), flattened);

        let column = children_of(&app, root)[0];
        assert_eq!(children_of(&app, column).len(), 4);
    }

    #[test]
    fn empty_stack_spawns_no_rows() {
        let (app, root) = rendered(Vec::new());
        let column = children_of(&app, root)[0];
        assert!(children_of(&app, column).is_empty());
        assert!(texts(&app, root).is_empty());
    }

    /// Sizes and gaps derive from the theme's rem, not literals.
    #[test]
    fn geometry_follows_theme_rem() {
        let mut theme = BevyTheme::default();
        theme.metrics.rem = 20.0;
        theme.metrics.body_size = 20.0;
        let (app, root) = rendered_with(theme.clone(), vec![gauge("a", 0.5, "b")]);

        let column = children_of(&app, root)[0];
        let column_node = app.world().get::<Node>(column).unwrap();
        assert_eq!(column_node.row_gap, Val::Px(theme.metrics.rem(0.45)));
        let gauge_row = children_of(&app, column)[0];
        let row_node = app.world().get::<Node>(gauge_row).unwrap();
        assert_eq!(row_node.column_gap, Val::Px(theme.metrics.rem(0.5)));
        let parts = children_of(&app, gauge_row);
        let label_node = app.world().get::<Node>(parts[0]).unwrap();
        assert_eq!(label_node.width, Val::Px(theme.metrics.rem(8.0)));
        let track_node = app.world().get::<Node>(parts[1]).unwrap();
        assert_eq!(track_node.height, Val::Px(theme.metrics.rem(0.55)));
        for part in [parts[0], parts[2]] {
            let size = app.world().get::<TextFont>(part).unwrap().font_size;
            assert_eq!(size, theme.metrics.rem(0.72));
        }
    }
}