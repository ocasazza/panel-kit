//! Reusable panel-kit widget builders for Bevy UI.
//!
//! These mirror the renderer-neutral models in `panel-kit-core` so a Bevy app
//! uses the same badge, meter, status, and table vocabulary as the web and TUI
//! shells instead of inventing per-app widgets. Colour roles resolve from the
//! [`BevyTheme`] resource the same way `ResolvedTuiTheme` drives the terminal
//! painters.

use bevy::prelude::*;
pub mod boxplot;
pub mod cascade;
pub mod content;
pub mod data_table;
pub mod dropdown;
pub mod flamegraph;
pub mod gauges;
pub mod scroll;
pub mod spinner;
pub mod text;
pub mod time_series;
pub mod trace;

use crate::fonts::{fallback_font, BOLD_FONT};
use crate::theme::BevyTheme;
use bevy::text::LineHeight;
use panel_kit_core::badge::{display_label, BadgeKind, BadgeSpec};
use panel_kit_core::widgets::badge::{perceived_brightness, tint_over};
use panel_kit_core::theme::ThemeColor;
use panel_kit_core::widgets::meter::MeterModel;
use panel_kit_core::widgets::status::StatusModel;
use panel_kit_core::widgets::table::TableModel;

/// The badge stroke colour (`--badge-c`). A community tint strokes with its
/// 30% lift toward white (`tint_over`, as `src/badge.rs` `style_for` does);
/// otherwise the kind class picks its theme slot (`.badge-*`, CSS L452-460).
fn badge_color(spec: &BadgeSpec, theme: &BevyTheme) -> Color {
    if let Some(tint) = spec.override_color {
        let (r, g, b) = tint_over(tint, (255, 255, 255), 0.30);
        return Color::srgb_u8(r, g, b);
    }

    match &spec.kind {
        BadgeKind::Tag => theme.info,
        BadgeKind::Doctype | BadgeKind::Author => theme.info,
        BadgeKind::Folder => theme.text_muted,
        BadgeKind::Entity { .. } | BadgeKind::Date => theme.tertiary,
        BadgeKind::Wikilink { resolved: true, .. } => theme.info,
        BadgeKind::Wikilink { resolved: false, .. } => theme.error,
        BadgeKind::Url { .. } | BadgeKind::Status => theme.success,
        BadgeKind::Generic => theme.border,
    }
}

/// The badge label colour (`--badge-fg`): a community tint picks text or
/// background by its brightness; wikilink and URL kinds take their stroke
/// colour; everything else is `--color-text`.
fn badge_ink(spec: &BadgeSpec, theme: &BevyTheme) -> Color {
    if let Some(tint) = spec.override_color {
        return if perceived_brightness(tint) < 0.55 {
            theme.text
        } else {
            theme.background
        };
    }

    match &spec.kind {
        BadgeKind::Wikilink { .. } | BadgeKind::Url { .. } => badge_color(spec, theme),
        _ => theme.text,
    }
}

/// Per-channel sRGB lerp, the value of `color-mix(in srgb, a (1-t), b t)`.
fn mix_srgb(a: Color, b: Color, t: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
}

/// Horizontal badge padding: the literal `10px` of `.badge { padding:4px 10px }`.
/// No spacing token carries it.
const BADGE_PAD_X: f32 = 10.0;
/// `.badge { line-height:1.3 }`.
const BADGE_LINE_HEIGHT: f32 = 1.3;
/// `.badge-label { max-width:28ch }`.
const BADGE_LABEL_CH: f32 = 28.0;

/// Spawn one pill per shared [`BadgeSpec`], painted as the web `.badge`.
///
/// Padding is `4px 10px` (vertical is `--space-xs`, the same value as the
/// CSS literal), the stroke is 1px in [`badge_color`], the fill is the
/// community tint or `--color-background`, and the label is
/// `display_label(&kind, &value)` at `--label-size`. An active badge without a
/// tint mixes 25% `highlight` into the background; every active badge gets a
/// 2px `highlight` ring at 40% alpha. The `+`/`×` affordances are not painted:
/// the canary specs do not set `with_plus`/`with_x`.
pub fn badges(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, specs: &[BadgeSpec]) {
    let m = &theme.metrics;
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_content: AlignContent::FlexStart,
            column_gap: Val::Px(m.space_xs),
            row_gap: Val::Px(m.space_xs),
            ..default()
        })
        .with_children(|strip| {
            let height = m.label_size * BADGE_LINE_HEIGHT + 2.0 * m.space_xs + 2.0;
            for spec in specs {
                let fill = match (spec.override_color, spec.active) {
                    (Some((r, g, b)), _) => Color::srgb_u8(r, g, b),
                    (None, true) => mix_srgb(theme.background, theme.highlight, 0.25),
                    (None, false) => theme.background,
                };
                let mut chip = strip.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(BADGE_PAD_X), Val::Px(m.space_xs)),
                        border: UiRect::all(Val::Px(1.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(fill),
                    BorderColor(badge_color(spec, theme)),
                    BorderRadius::all(Val::Px(m.badge_radius.min(height / 2.0))),
                ));
                if spec.active {
                    chip.insert(BoxShadow::new(
                        theme.highlight.with_alpha(0.4),
                        Val::Px(0.0),
                        Val::Px(0.0),
                        Val::Px(2.0),
                        Val::Px(0.0),
                    ));
                }
                let style = TextStyle {
                    size: m.label_size,
                    line_height: BADGE_LINE_HEIGHT,
                    ..TextStyle::body(theme)
                }
                .color(badge_ink(spec, theme));
                chip.with_children(|chip| {
                    chip.spawn(Node {
                        max_width: Val::Px(BADGE_LABEL_CH * crate::fonts::ch_em() * m.label_size),
                        overflow: Overflow::clip(),
                        ..default()
                    })
                    .with_children(|cell| {
                        styled_text(cell, display_label(&spec.kind, &spec.value), style);
                    });
                });
            }
        });
}

/// The categorical series palette, derived from the theme.
///
/// The order the web and terminal painters cycle through, so a chart keeps the
/// same colour language in every shell.
pub(crate) fn series_tints(theme: &BevyTheme) -> [Color; 6] {
    [
        theme.color(ThemeColor::Text),
        theme.color(ThemeColor::Primary),
        theme.color(ThemeColor::Secondary),
        theme.color(ThemeColor::Tertiary),
        theme.color(ThemeColor::Info),
        theme.color(ThemeColor::Error),
    ]
}

/// Spawn a labelled meter bar from core's [`MeterModel`].
pub fn meter(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, model: &MeterModel) {
    row(parent, |row| {
        label_text(row, model.label.clone(), 11.0, theme.text_muted);
        label_text(row, model.text.clone(), 11.0, theme.text);
    });
    let ratio = model.ratio.clamp(0.0, 1.0) as f32;
    let tint = model
        .color
        .map(|rgb| Color::srgb_u8(rgb.0, rgb.1, rgb.2))
        .unwrap_or(theme.primary);
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(theme.background),
        ))
        .with_children(|track| {
            track.spawn((
                Node {
                    width: Val::Percent(ratio * 100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(tint),
            ));
        });
}

/// Spawn a status dot plus label and state.
pub fn status(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, model: &StatusModel) {
    row(parent, |row| {
        row.spawn((
            Node {
                width: Val::Px(9.0),
                height: Val::Px(9.0),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(model.color.0, model.color.1, model.color.2)),
        ));
        label_text(row, model.label.clone(), 11.0, theme.text);
        label_text(
            row,
            format!("{:?}", model.state).to_lowercase(),
            11.0,
            theme.text_muted,
        );
    });
}

/// Spawn the interactive data table over core's [`TableModel`], as the web
/// `Table` content paints it. [`data_table::DataTablePlugin`] paints and
/// drives it; returns the table entity.
pub fn table(parent: &mut ChildSpawnerCommands, model: &TableModel) -> Entity {
    data_table::spawn(parent, model)
}

/// Spawn a section heading.
pub fn section(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, label: &str) {
    label_text(parent, label, 10.0, theme.primary);
}

/// Spawn a label/value row.
pub fn key_value(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, key: &str, value: &str) {
    row(parent, |row| {
        label_text(row, key.to_uppercase(), 10.0, theme.text_muted);
        label_text(row, value.to_string(), 11.0, theme.text);
    });
}

/// Spawns a full-width row with children spaced between the edges.
pub fn row(parent: &mut ChildSpawnerCommands, build: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(build);
}

/// A row whose children sit side by side from the left, like inline spans in
/// a TUI line or web text.
pub fn inline_row(parent: &mut ChildSpawnerCommands, build: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(build);
}

/// Spawns a text label at the given px size and tint.
pub fn label_text(
    parent: &mut ChildSpawnerCommands,
    value: impl Into<String>,
    size: f32,
    tint: Color,
) {
    parent.spawn((
        Text::new(value),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(tint),
    ));
}

/// How a run of text is set: the CSS `font-size`, `line-height`,
/// `font-weight`, `letter-spacing`, `text-transform` and `color` of one
/// web text role, resolved from the theme's typography tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Font size in logical pixels.
    pub size: f32,
    /// Line height as a multiple of `size`.
    pub line_height: f32,
    /// Whether the run uses [`BOLD_FONT`].
    pub bold: bool,
    /// Letter spacing as a multiple of `size`.
    pub tracking: f32,
    /// Whether the run is set in capitals.
    pub uppercase: bool,
    /// Text colour.
    pub color: Color,
    /// Whether the run wraps to its container (CSS `white-space: normal`);
    /// otherwise it is one line box sized to its glyph advances.
    pub wrap: bool,
}

impl TextStyle {
    /// Body text (`--body-size`, `--body-line-height`) in `--color-text`.
    pub fn body(theme: &BevyTheme) -> Self {
        Self {
            size: theme.metrics.body_size,
            line_height: theme.metrics.line_height,
            bold: false,
            tracking: 0.0,
            uppercase: false,
            color: theme.text,
            wrap: false,
        }
    }

    /// Body line height at `factor` rem.
    pub fn rem(theme: &BevyTheme, factor: f32) -> Self {
        Self {
            size: theme.metrics.rem(factor),
            ..Self::body(theme)
        }
    }

    /// The tracked-caps label role (`--label-size`, `--label-weight`,
    /// `--label-tracking`, uppercase), as panel titles set it.
    pub fn label(theme: &BevyTheme) -> Self {
        Self {
            size: theme.metrics.label_size,
            bold: theme.metrics.label_bold,
            tracking: theme.metrics.label_tracking,
            uppercase: true,
            ..Self::body(theme)
        }
    }

    /// With `color`.
    pub fn color(self, color: Color) -> Self {
        Self { color, ..self }
    }

    /// With a line height of `line_height` times the size.
    pub fn line_height(self, line_height: f32) -> Self {
        Self { line_height, ..self }
    }

    /// In the bold face.
    pub fn bold(self) -> Self {
        Self { bold: true, ..self }
    }

    /// Tracked by `tracking` em and set in capitals.
    pub fn tracked_caps(self, tracking: f32) -> Self {
        Self {
            tracking,
            uppercase: true,
            ..self
        }
    }

    /// Wrapping to the container width.
    pub fn wrapping(self) -> Self {
        Self { wrap: true, ..self }
    }

    /// Height of one line box in logical pixels.
    pub fn line_box(&self) -> f32 {
        self.size * self.line_height
    }

    /// The Bevy font for this style.
    pub fn font(&self) -> TextFont {
        TextFont {
            font: if self.bold {
                BOLD_FONT
            } else {
                Handle::default()
            },
            font_size: self.size,
            line_height: LineHeight::RelativeToFont(self.line_height),
            ..default()
        }
    }

    /// Letter spacing in logical pixels.
    pub fn tracking_px(&self) -> f32 {
        self.tracking * self.size
    }
}

/// Spawn `value` set in `style`; returns the outermost entity.
///
/// Glyphs the theme face lacks are spans in their [`fallback_font`]. A
/// non-wrapping run is one line box as wide as its glyph advances, the
/// browser's monospace measure. Bevy text has no letter spacing, so tracked
/// text places one text node per glyph on the tracked pitch, trailing
/// spacing included as CSS does.
pub fn styled_text(
    parent: &mut ChildSpawnerCommands,
    value: impl Into<String>,
    style: TextStyle,
) -> Entity {
    let value = value.into();
    let value = if style.uppercase {
        value.to_uppercase()
    } else {
        value
    };
    let spacing = style.tracking_px();
    let advances = glyph_advances(&value, style.size);
    let line_box = style.line_box();
    if spacing != 0.0 {
        if let Some(advances) = advances {
            let width: f32 = advances.iter().map(|advance| advance + spacing).sum();
            return parent
                .spawn(Node {
                    width: Val::Px(width),
                    height: Val::Px(line_box),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|row| {
                    let mut x = 0.0;
                    for (glyph, advance) in value.chars().zip(advances) {
                        let mut cell = row.spawn(Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x),
                            top: Val::ZERO,
                            ..default()
                        });
                        insert_runs(&mut cell, &glyph.to_string(), style);
                        x += advance + spacing;
                    }
                })
                .id();
        }
        return parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_shrink: 0.0,
                column_gap: Val::Px(spacing),
                padding: UiRect::right(Val::Px(spacing)),
                ..default()
            })
            .with_children(|row| {
                for glyph in value.chars() {
                    let mut cell = row.spawn(Node::default());
                    insert_runs(&mut cell, &glyph.to_string(), style);
                }
            })
            .id();
    }
    let node = match (style.wrap, advances) {
        (true, _) => Node::default(),
        (false, Some(advances)) => Node {
            width: Val::Px(advances.iter().sum()),
            height: Val::Px(line_box),
            flex_shrink: 0.0,
            ..default()
        },
        (false, None) => Node {
            height: Val::Px(line_box),
            ..default()
        },
    };
    let mut text = parent.spawn(node);
    insert_runs(&mut text, &value, style);
    text.id()
}

/// Per-glyph advances at `size` from the installed theme font, or `None`
/// before one is installed or when the value has more than one line.
fn glyph_advances(value: &str, size: f32) -> Option<Vec<f32>> {
    if value.contains('\n') {
        return None;
    }
    value
        .chars()
        .map(|glyph| crate::fonts::glyph_advance_em(glyph).map(|em| em * size))
        .collect()
}

/// Make `entity` a text root for `value`: one run per font, glyphs the theme
/// face lacks as spans in their fallback face.
fn insert_runs(entity: &mut EntityCommands, value: &str, style: TextStyle) {
    let base = style.font();
    let layout = if style.wrap {
        TextLayout::default()
    } else {
        TextLayout::new_with_no_wrap()
    };
    let mut runs: Vec<(Handle<Font>, String)> = Vec::new();
    for glyph in value.chars() {
        let font = fallback_font(glyph).unwrap_or_else(|| base.font.clone());
        match runs.last_mut() {
            Some((run_font, run)) if *run_font == font => run.push(glyph),
            _ => runs.push((font, glyph.to_string())),
        }
    }
    if runs.len() <= 1 {
        let (font, run) = runs.pop().unwrap_or((base.font.clone(), String::new()));
        entity.insert((
            Text::new(run),
            TextFont { font, ..base },
            TextColor(style.color),
            layout,
        ));
        return;
    }
    entity
        .insert((Text::new(""), base.clone(), TextColor(style.color), layout))
        .with_children(|spans| {
            for (font, run) in runs {
                spans.spawn((
                    TextSpan::new(run),
                    TextFont {
                        font,
                        ..base.clone()
                    },
                    TextColor(style.color),
                ));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use panel_kit_core::badge::BadgeKind;
    use panel_kit_core::widgets::status::StatusState;

    /// Build `widget` under a fresh root node and return the world and root.
    ///
    /// A one-shot system applies its command queue, so the spawned hierarchy is
    /// queryable as soon as the helper returns: no plugin, render stack, or
    /// window is needed to assert on the UI entities.
    fn world_with(
        widget: impl FnOnce(&mut ChildSpawnerCommands, &BevyTheme) + Send + Sync + 'static,
    ) -> (World, Entity) {
        let mut widget = Some(widget);
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        world
            .run_system_once(move |mut commands: Commands| {
                let Some(widget) = widget.take() else {
                    return;
                };
                let theme = BevyTheme::default();
                commands
                    .entity(root)
                    .with_children(move |parent| widget(parent, &theme));
            })
            .expect("the one-shot widget system runs");
        (world, root)
    }

    fn children(world: &World, entity: Entity) -> Vec<Entity> {
        world
            .get::<Children>(entity)
            .map(|children| children.iter().collect())
            .unwrap_or_default()
    }

    fn only_child(world: &World, entity: Entity) -> Entity {
        let children = children(world, entity);
        assert_eq!(children.len(), 1, "expected exactly one child");
        children[0]
    }

    fn label(world: &World, entity: Entity) -> String {
        world
            .get::<Text>(entity)
            .expect("a widget label carries Text")
            .0
            .clone()
    }

    fn font_size(world: &World, entity: Entity) -> f32 {
        world
            .get::<TextFont>(entity)
            .expect("a widget label carries TextFont")
            .font_size
    }

    fn painted_text_color(world: &World, entity: Entity) -> Color {
        world
            .get::<TextColor>(entity)
            .expect("a widget label carries TextColor")
            .0
    }

    fn background(world: &World, entity: Entity) -> Color {
        world
            .get::<BackgroundColor>(entity)
            .expect("every bevy UI node carries a BackgroundColor")
            .0
    }

    fn node(world: &World, entity: Entity) -> Node {
        world
            .get::<Node>(entity)
            .expect("a spawned widget part is a UI node")
            .clone()
    }

    /// Compare by resolved sRGBA so `Color` variants can differ in spelling.
    fn assert_color(actual: Color, expected: Color) {
        assert_eq!(actual.to_srgba(), expected.to_srgba());
    }

    /// A fill width is a percentage of its track, so compare with float slack.
    fn assert_percent(width: Val, expected: f32) {
        match width {
            Val::Percent(actual) => {
                assert!(
                    (actual - expected).abs() < 1e-3,
                    "expected {expected}% fill, got {actual}%"
                );
            }
            other => panic!("expected a percentage width, got {other:?}"),
        }
    }

    fn spec(field: &str, value: &str, kind: BadgeKind) -> BadgeSpec {
        BadgeSpec::new(field, value, kind)
    }

    /// Non-default metrics so token-derived values cannot equal literals.
    fn badge_theme() -> BevyTheme {
        let mut theme = BevyTheme::default();
        theme.metrics.space_xs = 7.0;
        theme.metrics.label_size = 15.0;
        theme.metrics.badge_radius = 3.0;
        theme
    }

    /// Build `badges` for `specs` under `theme`; returns the world and chips.
    fn chips_with(theme: BevyTheme, specs: Vec<BadgeSpec>) -> (World, Vec<Entity>) {
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        world
            .run_system_once(move |mut commands: Commands| {
                let (theme, specs) = (theme.clone(), specs.clone());
                commands
                    .entity(root)
                    .with_children(move |parent| badges(parent, &theme, &specs));
            })
            .expect("the one-shot widget system runs");
        let strip = only_child(&world, root);
        let chips = children(&world, strip);
        (world, chips)
    }

    fn chip_text(world: &World, chip: Entity) -> Entity {
        only_child(world, only_child(world, chip))
    }

    fn border_color(world: &World, chip: Entity) -> Color {
        world.get::<BorderColor>(chip).expect("chip border colour").0
    }

    /// Each kind strokes in its `.badge-*` slot (CSS L452-460); wikilink and
    /// URL kinds also take it as label colour, the rest paint `text`.
    #[test]
    fn badges_map_each_kind_to_stroke_and_ink_slots() {
        let theme = badge_theme();
        let link = |resolved: bool| BadgeKind::Wikilink {
            resolved,
            target: "t".into(),
        };
        let specs = vec![
            spec("tag", "rust", BadgeKind::Tag),
            spec("doctype", "note", BadgeKind::Doctype),
            spec("author", "ada", BadgeKind::Author),
            spec("folder", "docs", BadgeKind::Folder),
            spec("entity", "ocasazza", BadgeKind::Entity { ty: None }),
            spec("date", "2026-11-02", BadgeKind::Date),
            spec("wikilink", "spec", link(true)),
            spec("wikilink", "ghost", link(false)),
            spec(
                "url",
                "panel-kit",
                BadgeKind::Url {
                    href: "https://example.com".into(),
                    host: String::new(),
                },
            ),
            spec("status", "done", BadgeKind::Status),
            spec("generic", "value", BadgeKind::Generic),
        ];
        let stroke = [
            theme.info,
            theme.info,
            theme.info,
            theme.text_muted,
            theme.tertiary,
            theme.tertiary,
            theme.info,
            theme.error,
            theme.success,
            theme.success,
            theme.border,
        ];
        let ink = [
            theme.text,
            theme.text,
            theme.text,
            theme.text,
            theme.text,
            theme.text,
            theme.info,
            theme.error,
            theme.success,
            theme.text,
            theme.text,
        ];
        let (world, chips) = chips_with(theme.clone(), specs);
        for (index, chip) in chips.iter().enumerate() {
            assert_color(border_color(&world, *chip), stroke[index]);
            assert_color(background(&world, *chip), theme.background);
            assert_color(painted_text_color(&world, chip_text(&world, *chip)), ink[index]);
        }
    }

    /// `.badge`: padding `4px 10px` (vertical is `--space-xs`), 1px border,
    /// `--label-size` text at line-height 1.3, strip gap `--space-xs`.
    #[test]
    fn badge_geometry_follows_the_theme_tokens() {
        let theme = badge_theme();
        let mut world_specs = vec![spec("tag", "a", BadgeKind::Tag)];
        world_specs.push(spec("tag", "b", BadgeKind::Tag));
        let (world, chips) = chips_with(theme.clone(), world_specs);
        assert_eq!(chips.len(), 2);

        let strip = world.get::<ChildOf>(chips[0]).expect("chip has a strip").parent();
        let strip_node = node(&world, strip);
        assert_eq!(strip_node.flex_direction, FlexDirection::Row);
        assert_eq!(strip_node.flex_wrap, FlexWrap::Wrap);
        assert_eq!(strip_node.column_gap, Val::Px(theme.metrics.space_xs));
        assert_eq!(strip_node.row_gap, Val::Px(theme.metrics.space_xs));

        let chip_node = node(&world, chips[0]);
        assert_eq!(
            chip_node.padding,
            UiRect::axes(Val::Px(BADGE_PAD_X), Val::Px(theme.metrics.space_xs))
        );
        assert_eq!(chip_node.border, UiRect::all(Val::Px(1.0)));

        let text = chip_text(&world, chips[0]);
        assert_eq!(font_size(&world, text), theme.metrics.label_size);
        assert!(matches!(
            world.get::<TextFont>(text).expect("label font").line_height,
            LineHeight::RelativeToFont(h) if h == 1.3
        ));
    }

    /// `border-radius:var(--badge-radius)` is clamped to half the pill height.
    #[test]
    fn badge_radius_is_the_token_clamped_to_a_pill() {
        let mut theme = badge_theme();
        let height = theme.metrics.label_size * 1.3 + 2.0 * theme.metrics.space_xs + 2.0;

        let (world, chips) = chips_with(theme.clone(), vec![spec("t", "a", BadgeKind::Tag)]);
        let radius = world.get::<BorderRadius>(chips[0]).expect("radius");
        assert_eq!(radius.top_left, Val::Px(theme.metrics.badge_radius));

        theme.metrics.badge_radius = 999.0;
        let (world, chips) = chips_with(theme, vec![spec("t", "a", BadgeKind::Tag)]);
        let radius = world.get::<BorderRadius>(chips[0]).expect("radius");
        assert_eq!(radius.top_left, Val::Px(height / 2.0));
    }

    /// A community tint owns the fill, strokes with its 30% lift toward
    /// white, and picks the label colour by perceived brightness.
    #[test]
    fn badge_override_color_owns_fill_stroke_and_ink() {
        let theme = badge_theme();
        let mut dark = spec("tag", "rust", BadgeKind::Tag);
        dark.override_color = Some((203, 77, 93));
        let mut light = spec("tag", "sand", BadgeKind::Tag);
        light.override_color = Some((230, 220, 180));

        let (world, chips) = chips_with(theme.clone(), vec![dark, light]);
        assert_color(background(&world, chips[0]), Color::srgb_u8(203, 77, 93));
        assert_color(border_color(&world, chips[0]), Color::srgb_u8(219, 130, 142));
        assert_color(painted_text_color(&world, chip_text(&world, chips[0])), theme.text);
        assert_color(
            painted_text_color(&world, chip_text(&world, chips[1])),
            theme.background,
        );
    }

    /// Active: 25% highlight mixed into the background plus a 2px highlight
    /// ring at 40% alpha; an inactive badge has no ring.
    #[test]
    fn badge_active_mixes_highlight_and_adds_a_ring() {
        let theme = badge_theme();
        let mut active = spec("status", "draft", BadgeKind::Status);
        active.active = true;
        let idle = spec("status", "idle", BadgeKind::Status);

        let (world, chips) = chips_with(theme.clone(), vec![active, idle]);
        let (bg, hl) = (theme.background.to_srgba(), theme.highlight.to_srgba());
        let fill = background(&world, chips[0]).to_srgba();
        assert!((fill.red - (bg.red + (hl.red - bg.red) * 0.25)).abs() < 1e-6);
        assert!((fill.green - (bg.green + (hl.green - bg.green) * 0.25)).abs() < 1e-6);
        assert!((fill.blue - (bg.blue + (hl.blue - bg.blue) * 0.25)).abs() < 1e-6);

        let ring = world.get::<BoxShadow>(chips[0]).expect("active ring");
        assert_eq!(ring.0.len(), 1);
        assert_eq!(ring.0[0].spread_radius, Val::Px(2.0));
        assert_eq!(ring.0[0].blur_radius, Val::Px(0.0));
        assert_color(ring.0[0].color, theme.highlight.with_alpha(0.4));
        assert!(world.get::<BoxShadow>(chips[1]).is_none());
        assert_color(background(&world, chips[1]), theme.background);
    }

    /// The label is `display_label`: wikilinks get the `⟶ ` prefix, URLs
    /// show their host.
    #[test]
    fn badge_label_is_the_display_label() {
        let link = spec(
            "link",
            "Spec",
            BadgeKind::Wikilink {
                resolved: true,
                target: "Spec".into(),
            },
        );
        let url = spec(
            "url",
            "x",
            BadgeKind::Url {
                href: "https://github.com/a".into(),
                host: "github.com".into(),
            },
        );
        let (world, chips) = chips_with(badge_theme(), vec![link, url]);
        assert_eq!(label(&world, chip_text(&world, chips[0])), "\u{27F6} Spec");
        assert_eq!(label(&world, chip_text(&world, chips[1])), "github.com");
    }

    /// Ports `bar_matches_tui_rounding_and_clamping`
    /// (`crates/panel-kit-core/src/widgets/meter.rs`), the `percent` clamp in
    /// `src/widgets/meter.rs`, and `meter_bar_reuses_core_fill_math`
    /// (`crates/panel-kit-tui/src/meter.rs`): the painted fill is the clamped
    /// ratio, the label and value text come from the model, and the optional
    /// colour becomes the fill colour.
    #[test]
    fn meter_clamps_ratio_and_paints_label_value_and_fill() {
        let theme = BevyTheme::default();
        for (ratio, filled) in [(-0.5, 0.0), (0.42, 42.0), (2.0, 100.0)] {
            let model = MeterModel {
                label: "cpu".into(),
                ratio,
                text: "42%".into(),
                color: Some((10, 20, 30)),
            };
            let (world, root) = world_with(move |parent, theme| meter(parent, theme, &model));

            let parts = children(&world, root);
            assert_eq!(parts.len(), 2, "a meter is a label row plus its track");
            let (labels, track) = (parts[0], parts[1]);

            let row_labels = children(&world, labels);
            assert_eq!(
                row_labels.iter().map(|e| label(&world, *e)).collect::<Vec<_>>(),
                ["cpu", "42%"]
            );
            assert_eq!(font_size(&world, row_labels[0]), 11.0);
            assert_color(painted_text_color(&world, row_labels[0]), theme.text_muted);
            assert_color(painted_text_color(&world, row_labels[1]), theme.text);

            let track_node = node(&world, track);
            assert_eq!(track_node.width, Val::Percent(100.0));
            assert_eq!(track_node.height, Val::Px(6.0));
            assert_color(background(&world, track), theme.background);

            let fill = only_child(&world, track);
            let fill_node = node(&world, fill);
            assert_percent(fill_node.width, filled);
            assert_eq!(fill_node.height, Val::Percent(100.0));
            assert_color(background(&world, fill), Color::srgb_u8(10, 20, 30));
        }
    }

    /// Ports `span_model` (`crates/panel-kit-tui/src/meter.rs`): a model
    /// without a colour still paints a fill, using the theme's primary tint.
    #[test]
    fn meter_without_a_color_keeps_the_default_fill() {
        let theme = BevyTheme::default();
        let model = MeterModel {
            label: "queue".into(),
            ratio: 0.5,
            text: "50%".into(),
            color: None,
        };
        let (world, root) = world_with(move |parent, theme| meter(parent, theme, &model));

        let track = children(&world, root)[1];
        let fill = only_child(&world, track);
        assert_percent(node(&world, fill).width, 50.0);
        assert_color(background(&world, fill), theme.primary);
    }

    /// Ports `state_name` (`src/widgets/status.rs`), the `●` dot assertion of
    /// `widget_painters_render_core_models_to_test_backend`
    /// (`crates/panel-kit-tui/tests/widget_painters.rs`), and
    /// `status_model_carries_label_state_and_color`
    /// (`crates/panel-kit-core/src/widgets/status.rs`): every state paints a
    /// dot in the model colour, the label, and its lowercase state name.
    #[test]
    fn status_paints_dot_label_and_lowercase_state() {
        let theme = BevyTheme::default();
        let states = [
            (StatusState::Ok, "ok"),
            (StatusState::Warning, "warning"),
            (StatusState::Error, "error"),
            (StatusState::Info, "info"),
            (StatusState::Unknown, "unknown"),
        ];
        for (state, name) in states {
            let model = StatusModel {
                label: "build".into(),
                state,
                color: (255, 200, 0),
            };
            let (world, root) = world_with(move |parent, theme| status(parent, theme, &model));

            let parts = children(&world, only_child(&world, root));
            assert_eq!(parts.len(), 3, "a status is a dot plus two labels");

            let dot = node(&world, parts[0]);
            assert_eq!((dot.width, dot.height), (Val::Px(9.0), Val::Px(9.0)));
            assert_color(background(&world, parts[0]), Color::srgb_u8(255, 200, 0));
            assert_eq!(label(&world, parts[1]), "build");
            assert_color(painted_text_color(&world, parts[1]), theme.text);
            assert_eq!(label(&world, parts[2]), name);
            assert_color(painted_text_color(&world, parts[2]), theme.text_muted);
        }
    }

}