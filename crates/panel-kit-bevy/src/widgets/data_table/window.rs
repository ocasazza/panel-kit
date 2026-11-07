//! Row windowing for long tables: the web grid's `sync_window` and the
//! cursor-follows-focus effect.
//!
//! A windowed table mounts the rows core's `mounted` names and stands spacers
//! in for the rest. The scroller's offset and height feed `TableEvent::Window`
//! (or `TableEvent::Scroll` once the content outgrows what a scroller lays
//! out), exactly as the web `onscroll` does.

use bevy::prelude::*;
use panel_kit_core::widgets::data_table::{Entry, ScrollMap, TableEvent, OVERSCAN};

use super::ribbon;
use super::{RowMark, TableControl, TableRows, Tables};
use crate::theme::BevyTheme;

/// `.pk-dt-spacer`: the height of the entries a window leaves unmounted.
#[derive(Component, Clone, Copy, Debug)]
pub struct Spacer;

/// Mounts the rows each windowed table's scroller needs, and tells every
/// table how wide its scroller is.
pub(super) fn sync_windows(
    mut tables: NonSendMut<Tables>,
    theme: Res<BevyTheme>,
    mut rows: Query<(&ComputedNode, &mut ScrollPosition, &TableControl), With<TableRows>>,
) {
    let rem = theme.metrics.rem(1.0);
    for (node, mut position, control) in &mut rows {
        let Some(instance) = tables.0.get_mut(&control.table) else {
            continue;
        };
        let size = node.size() * node.inverse_scale_factor();
        let mode = instance.tags_mode;
        if instance.tagged()
            && size.x > 0.0
            && ribbon::look(mode, size.x, rem) != ribbon::look(mode, instance.measured_width, rem)
        {
            instance.dirty = true;
        }
        if size.x > 0.0 {
            instance.measured_width = size.x;
        }
        if !instance.windowed || instance.reveal {
            continue;
        }
        instance.scroll = Vec2::new(position.offset_x, position.offset_y);
        let total = instance.view.entries.len();
        if total == 0 {
            continue;
        }
        let row = instance.state.view.row_height(instance.state.density);
        let visible = f64::from(size.y);
        let mut top = f64::from(position.offset_y);
        if ScrollMap::new(total, row, 0.0).scaled() {
            let seen = instance.state.viewport;
            if seen.seek {
                top = seen.top;
                position.offset_y = top as f32;
                instance.scroll.y = position.offset_y;
            }
            if seen.seek || top != seen.top || visible != seen.visible {
                instance.apply(TableEvent::Scroll { top, visible });
            }
            continue;
        }
        let first = (top / row).floor() as usize;
        let shown = (visible / row).ceil() as usize + 1;
        let now = instance.state.mounted(total, true);
        let need_end = (first + shown).min(total);
        if first < now.start || need_end > now.end || now.end == now.start {
            instance.apply(TableEvent::Window {
                start: first.saturating_sub(OVERSCAN),
                count: shown + 2 * OVERSCAN,
                row_height: None,
            });
        }
    }
}

/// Scrolls the cursor row into view after the keyboard moved it. A windowed
/// row that is not mounted yet is reached by arithmetic first, then trimmed
/// by where it lands.
pub(super) fn reveal_cursor(
    mut tables: NonSendMut<Tables>,
    mut rows: Query<(&ComputedNode, &GlobalTransform, &mut ScrollPosition, &TableControl), With<TableRows>>,
    marks: Query<(&RowMark, &ComputedNode, &GlobalTransform)>,
) {
    for (viewport, viewport_at, mut position, control) in &mut rows {
        let Some(instance) = tables.0.get_mut(&control.table) else {
            continue;
        };
        if !instance.reveal {
            continue;
        }
        let Some(cursor) = instance.state.cursor.clone() else {
            instance.reveal = false;
            continue;
        };
        let scale = viewport.inverse_scale_factor();
        let view = viewport.size().y * scale;
        if view <= 0.0 {
            continue;
        }
        let mark = marks
            .iter()
            .find(|(mark, node, _)| mark.table == control.table && mark.id == cursor && node.size().y > 0.0);
        if let Some((_, node, at)) = mark {
            let height = node.size().y * scale;
            let from_top = (at.translation().y - node.size().y / 2.0
                - (viewport_at.translation().y - viewport.size().y / 2.0))
                * scale;
            let shift = if from_top < 0.0 {
                from_top
            } else if from_top + height > view {
                from_top + height - view
            } else {
                0.0
            };
            position.offset_y = (position.offset_y + shift).max(0.0);
            instance.scroll.y = position.offset_y;
            instance.reveal = false;
            continue;
        }
        if !instance.windowed {
            instance.reveal = false;
            continue;
        }
        let total = instance.view.entries.len();
        let entry = instance
            .view
            .entries
            .iter()
            .position(|e| matches!(e, Entry::Row(row) if instance.data.id(*row) == cursor));
        let row = instance.state.view.row_height(instance.state.density);
        let map = ScrollMap::new(total, row, f64::from(view));
        if let Some(top) = entry.and_then(|entry| map.reveal(f64::from(position.offset_y), entry)) {
            position.offset_y = top as f32;
            instance.scroll.y = position.offset_y;
        } else {
            instance.reveal = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::input::keyboard::Key;
    use panel_kit_core::widgets::data_table::{Selectable, WINDOW_COUNT};

    use super::super::testing::*;
    use super::super::{Action, DataTable};
    use super::*;

    const ROW: f32 = 26.0;

    fn windowed(rows: usize) -> DataTable {
        DataTable {
            windowed: true,
            ..DataTable::new(long(rows))
        }
    }

    fn spacers(app: &mut App) -> Vec<f32> {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Node, With<Spacer>>();
        let mut heights: Vec<(f32, f32)> = query
            .iter(world)
            .map(|node| match node.height {
                Val::Px(px) => (px, px),
                _ => (0.0, 0.0),
            })
            .collect();
        heights.sort_by(|a, b| a.0.total_cmp(&b.0));
        heights.into_iter().map(|h| h.0).collect()
    }

    fn scroll_to(app: &mut App, top: f32) {
        let world = app.world_mut();
        let mut rows = world.query_filtered::<&mut ScrollPosition, With<TableRows>>();
        for mut position in rows.iter_mut(world) {
            position.offset_y = top;
        }
        app.update();
    }

    fn scroll_top(app: &mut App) -> f32 {
        let world = app.world_mut();
        let mut rows = world.query_filtered::<&ScrollPosition, With<TableRows>>();
        rows.iter(world).next().map_or(-1.0, |p| p.offset_y)
    }

    #[test]
    fn an_unwindowed_table_mounts_every_row_and_no_spacer() {
        let (mut app, table) = app_with(DataTable::new(long(200)));
        assert_eq!(mounted_ids(&mut app, table).len(), 200);
        assert_eq!(spacers(&mut app), Vec::<f32>::new());
    }

    #[test]
    fn a_windowed_table_mounts_the_first_window_and_a_spacer_for_the_rest() {
        let (mut app, table) = app_with(windowed(500));
        let ids = mounted_ids(&mut app, table);
        assert_eq!(ids.len(), WINDOW_COUNT);
        assert_eq!((ids[0].as_str(), ids[WINDOW_COUNT - 1].as_str()), ("r000", "r079"));
        assert_eq!(spacers(&mut app), vec![(500 - WINDOW_COUNT) as f32 * ROW], "only the spacer below");
    }

    #[test]
    fn scrolling_mounts_the_viewport_rows_plus_overscan_between_two_spacers() {
        let (mut app, table) = app_with(windowed(500));
        scroll_to(&mut app, 200.0 * ROW);
        let ids = mounted_ids(&mut app, table);
        // an unmeasured viewport shows one row; 20 rows of overscan each side
        let start = 200 - OVERSCAN;
        assert_eq!(ids.first().map(String::as_str), Some(format!("r{start:03}").as_str()));
        assert_eq!(ids.len(), 1 + 2 * OVERSCAN);
        let above = start as f32 * ROW;
        let below = (500 - (start + ids.len())) as f32 * ROW;
        assert_eq!(spacers(&mut app), vec![above.min(below), above.max(below)]);
        assert_eq!(scroll_top(&mut app), 200.0 * ROW, "the offset survives the repaint");
    }

    #[test]
    fn scrolling_back_to_the_top_drops_the_top_spacer() {
        let (mut app, table) = app_with(windowed(500));
        scroll_to(&mut app, 300.0 * ROW);
        assert_eq!(spacers(&mut app).len(), 2, "scrolled down: a spacer on each side");
        scroll_to(&mut app, 0.0);
        let ids = mounted_ids(&mut app, table);
        assert_eq!(ids.first().map(String::as_str), Some("r000"));
        assert_eq!(spacers(&mut app).len(), 1);
    }

    #[test]
    fn a_scroll_inside_the_mounted_window_remounts_nothing() {
        let (mut app, table) = app_with(windowed(500));
        let before = with_instance(&app, table, |i| i.state.window);
        scroll_to(&mut app, 10.0 * ROW);
        assert_eq!(with_instance(&app, table, |i| i.state.window), before);
    }

    #[test]
    fn the_end_key_moves_the_window_to_the_last_row() {
        let mut table = windowed(500);
        table.defaults.selectable = Selectable::None;
        let (mut app, table) = app_with(table);
        press(&mut app, table, |a| matches!(a, Action::Row(id) if id == "r000"));
        key(&mut app, Key::End);
        let ids = mounted_ids(&mut app, table);
        assert!(ids.contains(&"r499".to_owned()), "the cursor row is mounted so it can be focused");
        assert!(ids.len() <= WINDOW_COUNT);
        assert_eq!(spacers(&mut app).len(), 1, "only a top spacer remains");
    }

    #[test]
    fn mounted_rows_stand_one_row_height_tall() {
        let (mut app, table) = app_with(windowed(100));
        let world = app.world_mut();
        let mut rows = world.query::<(&RowMark, &Node)>();
        let heights: Vec<Val> = rows.iter(world).filter(|(m, _)| m.table == table).map(|(_, n)| n.height).collect();
        assert!(!heights.is_empty());
        assert!(heights.iter().all(|h| *h == Val::Px(ROW)));
    }

    #[test]
    fn compact_density_changes_the_row_height_the_spacers_use() {
        let (mut app, table) = app_with(windowed(500));
        press(&mut app, table, |a| matches!(a, Action::Apply(panel_kit_core::widgets::data_table::TableEvent::ToggleDensity)));
        let mounted = mounted_ids(&mut app, table).len();
        assert_eq!(spacers(&mut app), vec![(500 - mounted) as f32 * 20.0]);
    }
}
