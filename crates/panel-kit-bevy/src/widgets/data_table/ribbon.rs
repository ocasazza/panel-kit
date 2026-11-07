//! The row tag ribbon: web `paint_ribbon` and the `.pk-dt-tag*` rules.
//!
//! Which tags a row shows, how a segment is named and which colour slot it
//! fills all come from core (`RibbonMode::shown`, `segment_label`,
//! `tag_slot`); this module only lays the segments out.

use bevy::prelude::*;
use panel_kit_core::widgets::data_table::{
    ribbon_label, segment_label, tag_letter, tag_slot, RibbonMode,
};

use super::{styled_text, Action, Paint, TextStyle};

/// `--tag-seg`: a lettered segment's width.
const SEG: f32 = 13.0;
/// `--tag-overlap`: how far a segment sits under the next.
const OVERLAP: f32 = 3.0;
/// `--tag-inset`: the ribbon's left margin.
const INSET: f32 = 2.0;
/// `--tag-rake`: the trailing edge's whole-pixel slant.
const RAKE: f32 = 2.0;
/// The `+N` tail's budget in the gutter, and its minimum width.
const TAIL_BUDGET: f32 = 14.0;
const TAIL_MIN: f32 = 13.0;
const TAIL_PAD_X: f32 = 2.0;
/// `data-tags-mode="flat"`: a 3px stripe and a 1px gap.
const FLAT_SEG: f32 = 3.0;
const FLAT_GAP: f32 = 1.0;
/// `--text-label` as the segment letter's size; its line height is 1.
const LETTER_SIZE: f32 = 11.0;
/// `.pk-dt-tag-expanded` gap.
const EXPANDED_GAP: f32 = 4.0;
/// The scroller's container-query breakpoints, in rem: at or below the first
/// the ribbon goes flat, at or below the second it keeps only its first tag.
const FLAT_BELOW_REM: f32 = 34.0;
const FLAG_BELOW_REM: f32 = 22.0;
/// Texels per pixel of the rake mask, and its coverage samples per axis.
const RAKE_TEXELS: u32 = 16;
const RAKE_SAMPLES: u32 = 4;

/// The tags gutter's width: inset, four overlapped segments and the tail.
pub(super) const GUTTER: f32 = INSET + SEG * 4.0 - OVERLAP * 3.0 + TAIL_BUDGET;

/// `--tag-0` … `--tag-7`.
const FILLS: [[u8; 3]; 8] = [
    [0x33, 0x6e, 0x09],
    [0x82, 0x5d, 0xf2],
    [0xc9, 0x50, 0xf1],
    [0x36, 0xae, 0x0e],
    [0xf4, 0x7d, 0xd7],
    [0xf7, 0x9d, 0xca],
    [0xc4, 0xc4, 0xfa],
    [0xe6, 0xd0, 0xfb],
];

/// The mask the last segment's raked corner is cut with: the wedge below the
/// line from the bottom-left to the top-right, in white, tinted at use.
pub(super) const RAKE_IMAGE: Handle<Image> =
    bevy::asset::weak_handle!("5b1c7d4e-2a90-4f63-8e1d-9c3b6a7f0e52");

/// What a ribbon draws once the table's width is known: the segment style,
/// and whether only the first tag shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Look {
    pub style: RibbonMode,
    pub first_only: bool,
}

/// `mode` as the stylesheet resolves it at `width` pixels of scroller, `rem`
/// the root font size. An unmeasured width leaves `mode` alone.
pub(super) fn look(mode: RibbonMode, width: f32, rem: f32) -> Look {
    let mut look = Look {
        style: match mode {
            RibbonMode::Flag => RibbonMode::Full,
            other => other,
        },
        first_only: mode == RibbonMode::Flag,
    };
    if width > 0.0 {
        if width <= FLAT_BELOW_REM * rem {
            look.style = RibbonMode::Flat;
        }
        if width <= FLAG_BELOW_REM * rem {
            look.first_only = true;
        }
    }
    look
}

/// `--tag-N` and its ink: slot 0 is set in `--color-text`, the rest in
/// `--color-inverse-text`.
fn slot_colors(slot: u32, text: Color, inverse: Color) -> (Color, Color) {
    let [r, g, b] = FILLS[slot as usize % FILLS.len()];
    (
        Color::srgb_u8(r, g, b),
        if slot == 0 { text } else { inverse },
    )
}

/// The rake mask: alpha 1 where the corner is cut away (`x + y >= 1`).
pub(super) fn rake_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let (width, height) = (RAKE_TEXELS, RAKE_TEXELS);
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let mut covered = 0;
            for sy in 0..RAKE_SAMPLES {
                for sx in 0..RAKE_SAMPLES {
                    let u = (x as f32 + (sx as f32 + 0.5) / RAKE_SAMPLES as f32) / width as f32;
                    let v = (y as f32 + (sy as f32 + 0.5) / RAKE_SAMPLES as f32) / height as f32;
                    if u + v >= 1.0 {
                        covered += 1;
                    }
                }
            }
            let alpha = (covered * 255 / (RAKE_SAMPLES * RAKE_SAMPLES)) as u8;
            data.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width,
            height,
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

/// A painted ribbon segment.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TagSegment {
    /// The row the segment sits on.
    pub row: String,
    /// The canonical tag.
    pub tag: String,
    /// Whether the tag is in the filter.
    pub on: bool,
}

impl Paint<'_> {
    /// The ribbon's height: `--dt-row` less the cell's block padding.
    fn ribbon_height(&self) -> f32 {
        let state = &self.instance.state;
        let row = state.density.row_height() as f32;
        match state.density {
            panel_kit_core::widgets::data_table::Density::Normal => row - 6.0,
            panel_kit_core::widgets::data_table::Density::Compact => row - 2.0,
        }
    }

    /// One row's tags in the gutter: a layered ribbon, or spelled out when the
    /// cursor row is expanded. `ground` is the colour behind the row.
    pub(super) fn ribbon(
        &self,
        gutter: &mut ChildSpawnerCommands,
        row: &str,
        labels: &[String],
        cursor: bool,
        ground: Color,
    ) {
        if labels.is_empty() {
            return;
        }
        let theme = self.theme;
        let state = &self.instance.state;
        let height = self.ribbon_height();
        let name = ribbon_label(labels);
        if state.tags_expand && cursor {
            gutter
                .spawn((
                    Node {
                        height: Val::Px(height),
                        margin: UiRect::left(Val::Px(INSET)),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(EXPANDED_GAP),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    Name::new(name),
                ))
                .with_children(|spelled| {
                    let style = self.letter_style().color(theme.text_muted);
                    for tag in labels {
                        styled_text(spelled, tag.clone(), style);
                    }
                });
            return;
        }
        let look = look(
            self.instance.tags_mode,
            self.instance.measured_width,
            theme.metrics.rem(1.0),
        );
        let policy = if look.first_only { RibbonMode::Flag } else { self.instance.tags_mode };
        let shown = policy.shown(labels, self.instance.tags_limit);
        let hidden = labels.len() - shown.len();
        gutter
            .spawn((
                Node {
                    height: Val::Px(height),
                    margin: UiRect::left(Val::Px(INSET)),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Stretch,
                    flex_shrink: 0.0,
                    ..default()
                },
                Name::new(name),
            ))
            .with_children(|ribbon| {
                for (at, tag) in shown.iter().enumerate() {
                    let last = at + 1 == shown.len() && hidden == 0;
                    self.segment(ribbon, row, tag, cursor, look.style, last, ground);
                }
                if hidden > 0 {
                    self.tail(ribbon, row, hidden, labels.len());
                }
            });
    }

    fn letter_style(&self) -> TextStyle {
        TextStyle {
            size: LETTER_SIZE,
            line_height: 1.0,
            ..self.body()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn segment(
        &self,
        ribbon: &mut ChildSpawnerCommands,
        row: &str,
        tag: &str,
        cursor: bool,
        style: RibbonMode,
        last: bool,
        ground: Color,
    ) {
        let theme = self.theme;
        let state = &self.instance.state;
        let on = state.tag_filter.contains(tag);
        let filtering = !state.tag_filter.is_empty();
        let flat = style == RibbonMode::Flat;
        let raked = style == RibbonMode::Full;
        let (fill, ink) = slot_colors(tag_slot(tag), theme.text, theme.inverse_text);
        let outline = filtering && !on;
        let (background, ink, border, edge) = if on {
            (fill, ink, 2.0, theme.highlight)
        } else if outline {
            (theme.background, fill, 1.0, fill)
        } else {
            (fill, ink, 0.0, fill)
        };
        let focused = cursor
            && self.grid_focused
            && state.tag_focus.as_ref().map(|(r, t)| (r.as_str(), t.as_str())) == Some((row, tag));
        let mut segment = ribbon.spawn((
            Node {
                width: Val::Px(if flat { FLAT_SEG } else { SEG }),
                flex_shrink: 0.0,
                margin: UiRect::right(Val::Px(if flat { FLAT_GAP } else { -OVERLAP })),
                border: UiRect::all(Val::Px(border)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(background),
            BorderColor(edge),
            BorderRadius::all(Val::Px(theme.metrics.panel_radius)),
            Button,
            super::TableControl {
                table: self.table,
                action: Action::Tag(tag.to_owned()),
            },
            TagSegment {
                row: row.to_owned(),
                tag: tag.to_owned(),
                on,
            },
            Name::new(segment_label(tag, &state.tag_filter)),
        ));
        if focused {
            segment.insert(Outline {
                width: Val::Px(1.0),
                offset: Val::Px(1.0),
                color: theme.focus_ring,
            });
        }
        segment.with_children(|seg| {
            if !flat {
                seg.spawn((
                    Node {
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Name::new("letter"),
                ))
                .with_children(|letter| {
                    styled_text(letter, tag_letter(tag), self.letter_style().color(ink));
                });
            }
            if on {
                seg.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor(theme.background),
                ));
            } else if !outline && !flat {
                seg.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(theme.background),
                ));
                seg.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        right: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        height: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(theme.divider),
                ));
            }
            if raked && last {
                seg.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(RAKE),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    ImageNode::new(RAKE_IMAGE).with_color(ground),
                ));
            }
        });
    }

    fn tail(&self, ribbon: &mut ChildSpawnerCommands, row: &str, hidden: usize, total: usize) {
        let theme = self.theme;
        ribbon
            .spawn((
                Node {
                    min_width: Val::Px(TAIL_MIN),
                    flex_shrink: 0.0,
                    padding: UiRect::axes(Val::Px(TAIL_PAD_X), Val::Px(0.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(theme.surface),
                BorderColor(theme.divider),
                BorderRadius::all(Val::Px(theme.metrics.panel_radius)),
                Button,
                super::TableControl {
                    table: self.table,
                    action: Action::Expand(row.to_owned()),
                },
                Name::new(format!("show all {total} tags on this row")),
            ))
            .with_children(|tail| {
                styled_text(
                    tail,
                    format!("+{hidden}"),
                    self.letter_style().color(theme.text_muted),
                );
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gutter_budgets_four_segments_and_a_tail() {
        assert_eq!(GUTTER, 59.0);
    }

    #[test]
    fn the_scroller_width_steps_the_ribbon_down_like_the_container_queries() {
        let rem = 15.0;
        let wide = look(RibbonMode::Full, 34.0 * rem + 1.0, rem);
        assert_eq!((wide.style, wide.first_only), (RibbonMode::Full, false));
        let mid = look(RibbonMode::Full, 34.0 * rem, rem);
        assert_eq!((mid.style, mid.first_only), (RibbonMode::Flat, false));
        let narrow = look(RibbonMode::Tight, 22.0 * rem, rem);
        assert_eq!((narrow.style, narrow.first_only), (RibbonMode::Flat, true));
        let unmeasured = look(RibbonMode::Flag, 0.0, rem);
        assert_eq!((unmeasured.style, unmeasured.first_only), (RibbonMode::Full, true));
    }

    #[test]
    fn slot_zero_is_set_in_text_and_the_rest_in_inverse_text() {
        let (text, inverse) = (Color::WHITE, Color::BLACK);
        assert_eq!(slot_colors(0, text, inverse).1, text);
        assert_eq!(slot_colors(3, text, inverse).1, inverse);
        assert_eq!(slot_colors(1, text, inverse).0, Color::srgb_u8(0x82, 0x5d, 0xf2));
    }
}

#[cfg(test)]
mod painting {
    use bevy::input::keyboard::Key;
    use panel_kit_core::widgets::data_table::TagMode;

    use super::super::testing::*;
    use super::super::{DataTable, GridFocus};
    use super::*;
    use crate::theme::BevyTheme;

    fn rows() -> panel_kit_core::widgets::table::TableModel {
        tagged(&[
            ("r1", &["Beta", "alpha"]),
            ("r2", &["beta"]),
            ("r3", &["alpha", "beta"]),
            ("r4", &[]),
        ])
    }

    /// The text of every segment letter node.
    fn letters(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let mut query = world.query::<(&Name, &Children)>();
        let letter_nodes: Vec<Entity> = query
            .iter(world)
            .filter(|(name, _)| name.as_str() == "letter")
            .flat_map(|(_, children)| children.iter())
            .collect();
        letter_nodes
            .into_iter()
            .filter_map(|child| world.get::<Text>(child).map(|text| text.0.clone()))
            .collect()
    }

    /// `(tag, fill, on)` of a row's painted segments.
    fn segments(app: &mut App, row: &str) -> Vec<(String, Color, bool)> {
        let world = app.world_mut();
        let mut query = world.query::<(&TagSegment, &BackgroundColor)>();
        let mut found: Vec<(String, Color, bool)> = query
            .iter(world)
            .filter(|(segment, _)| segment.row == row)
            .map(|(segment, fill)| (segment.tag.clone(), fill.0, segment.on))
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found
    }

    #[test]
    fn a_row_paints_one_segment_per_tag_in_its_slot_colour_and_letter() {
        let (mut app, _) = app_with(DataTable::new(rows()));
        let theme = BevyTheme::default();
        let found = segments(&mut app, "r1");
        assert_eq!(found.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(), ["alpha", "beta"]);
        for (tag, fill, on) in &found {
            assert_eq!(*fill, slot_colors(tag_slot(tag), theme.text, theme.inverse_text).0, "{tag}");
            assert!(!on);
        }
        assert_eq!(segments(&mut app, "r4"), Vec::new(), "an untagged row paints no ribbon");
        let mut letters = letters(&mut app);
        letters.sort();
        assert_eq!(letters, ["A", "A", "B", "B", "B"], "one letter per segment: r1 and r3 carry alpha and beta, r2 beta");
    }

    #[test]
    fn a_ribbon_is_named_by_core_s_label() {
        let (mut app, _) = app_with(DataTable::new(rows()));
        let world = app.world_mut();
        let mut names = world.query::<&Name>();
        let names: Vec<String> = names.iter(world).map(|n| n.as_str().to_owned()).collect();
        assert!(names.contains(&"tags: alpha, beta".to_owned()));
        assert!(names.contains(&"alpha, filter".to_owned()));
    }

    #[test]
    fn a_tagged_table_gains_a_59px_gutter_and_an_untagged_one_does_not() {
        let gutters = |app: &mut App| {
            let world = app.world_mut();
            let mut nodes = world.query::<&Node>();
            nodes.iter(world).filter(|n| n.flex_basis == Val::Px(GUTTER)).count()
        };
        let (mut tagged_app, _) = app_with(DataTable::new(rows()));
        // header + one per row
        assert_eq!(gutters(&mut tagged_app), 1 + 4);
        let (mut plain, _) = app();
        assert_eq!(gutters(&mut plain), 0);
    }

    #[test]
    fn a_segment_press_filters_rows_by_tag_and_outlines_the_rest() {
        let (mut app, table) = app_with(DataTable::new(rows()));
        let theme = BevyTheme::default();
        press(&mut app, table, |a| matches!(a, Action::Tag(tag) if tag == "alpha"));
        assert_eq!(shown_rows(app.world(), table), Some(vec!["r1".to_owned(), "r3".to_owned()]));
        let alpha = segments(&mut app, "r1");
        let (on, off): (Vec<_>, Vec<_>) = alpha.iter().partition(|s| s.2);
        assert_eq!(on.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(), ["alpha"]);
        assert_eq!(off[0].1, theme.background, "a segment outside the filter drops to an outline");
        let world = app.world_mut();
        let mut ringed = world.query::<(&TagSegment, &Node, &BorderColor)>();
        let ring = ringed.iter(world).find(|(s, ..)| s.on).expect("an on segment");
        assert_eq!((ring.1.border, ring.2 .0), (UiRect::all(Val::Px(2.0)), theme.highlight));
    }

    #[test]
    fn shift_on_a_segment_switches_to_all_of() {
        let (mut app, table) = app_with(DataTable::new(rows()));
        press(&mut app, table, |a| matches!(a, Action::Tag(tag) if tag == "alpha"));
        hold_key(&mut app, KeyCode::ShiftLeft);
        press(&mut app, table, |a| matches!(a, Action::Tag(tag) if tag == "beta"));
        release_key(&mut app, KeyCode::ShiftLeft);
        assert_eq!(with_instance(&app, table, |i| i.state.tag_mode), TagMode::All);
        assert_eq!(shown_rows(app.world(), table), Some(vec!["r1".to_owned(), "r3".to_owned()]));
    }

    #[test]
    fn hidden_tags_fold_into_a_counted_tail_that_expands_the_cursor_row() {
        let tags = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"];
        let (mut app, table) = app_with(DataTable::new(tagged(&[("r1", &tags), ("r2", &["alpha"])])));
        assert_eq!(segments(&mut app, "r1").len(), 4, "four segments before the tail");
        assert!(painted_texts(&mut app).iter().any(|t| t == "+2"));
        press(&mut app, table, |a| matches!(a, Action::Expand(row) if row == "r1"));
        assert!(with_instance(&app, table, |i| i.state.tags_expand));
        assert_eq!(segments(&mut app, "r1"), Vec::new(), "the cursor row spells its tags instead");
        let texts = painted_texts(&mut app);
        for tag in ["echo", "foxtrot"] {
            assert!(texts.iter().any(|t| t == tag), "{tag} spelled out");
        }
        assert_eq!(segments(&mut app, "r2").len(), 1, "other rows keep the ribbon");
    }

    #[test]
    fn the_flag_mode_keeps_only_the_first_tag_and_the_tail() {
        let table = DataTable {
            tags_mode: RibbonMode::Flag,
            ..DataTable::new(tagged(&[("r1", &["alpha", "beta", "gamma"])]))
        };
        let (mut app, _) = app_with(table);
        assert_eq!(segments(&mut app, "r1").len(), 1);
        assert!(painted_texts(&mut app).iter().any(|t| t == "+2"));
    }

    #[test]
    fn the_flat_mode_paints_unlettered_three_pixel_stripes() {
        let table = DataTable {
            tags_mode: RibbonMode::Flat,
            ..DataTable::new(tagged(&[("r1", &["alpha", "beta"])]))
        };
        let (mut app, _) = app_with(table);
        let world = app.world_mut();
        let mut query = world.query::<(&TagSegment, &Node)>();
        let widths: Vec<(Val, Val)> = query.iter(world).map(|(_, n)| (n.width, n.margin.right)).collect();
        assert_eq!(widths, vec![(Val::Px(3.0), Val::Px(1.0)); 2]);
        assert_eq!(letters(&mut app), Vec::<String>::new(), "no letters at the flat tier");
    }

    #[test]
    fn full_segments_overlap_by_three_pixels() {
        let (mut app, _) = app_with(DataTable::new(rows()));
        let world = app.world_mut();
        let mut query = world.query::<(&TagSegment, &Node)>();
        assert!(query
            .iter(world)
            .all(|(_, n)| n.width == Val::Px(13.0) && n.margin.right == Val::Px(-3.0)));
    }

    #[test]
    fn a_table_without_a_ribbon_paints_no_segments_and_ignores_tags() {
        let table = DataTable {
            ribbon: false,
            ..DataTable::new(rows())
        };
        let (mut app, table) = app_with(table);
        assert_eq!(segments(&mut app, "r1"), Vec::new());
        assert_eq!(shown_rows(app.world(), table).map(|r| r.len()), Some(4));
    }

    #[test]
    fn t_walks_a_row_s_segments_by_keyboard_and_enter_filters() {
        let (mut app, table) = app_with(DataTable::new(rows()));
        press(&mut app, table, |a| matches!(a, Action::Row(id) if id == "r1"));
        assert_eq!(app.world().resource::<GridFocus>().0, Some(table));
        key(&mut app, Key::Character("t".into()));
        assert_eq!(
            with_instance(&app, table, |i| i.state.tag_focus.clone()),
            Some(("r1".to_owned(), "alpha".to_owned()))
        );
        key(&mut app, Key::ArrowRight);
        assert_eq!(
            with_instance(&app, table, |i| i.state.tag_focus.clone()),
            Some(("r1".to_owned(), "beta".to_owned()))
        );
        key(&mut app, Key::Enter);
        assert!(with_instance(&app, table, |i| i.state.tag_filter.contains("beta")));
        key(&mut app, Key::Escape);
        assert_eq!(with_instance(&app, table, |i| i.state.tag_focus.clone()), None);
    }
}
