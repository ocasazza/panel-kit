//! Bevy canary: the shared canary content plus its Bevy painters.
//!
//! Every `canary.*` binding paints the same core model the TUI and web
//! canaries paint, through `panel-kit-bevy` widgets and the spec's theme.

#[allow(dead_code)]
#[path = "../../../examples/support/canary_content.rs"]
mod canary_content;
pub use canary_content::*;

use std::time::Duration;

use bevy::prelude::*;
use panel_kit_bevy::widgets::{
    self, boxplot, content, flamegraph, gauges, scroll, spinner, styled_text, time_series,
    TextStyle,
};
use panel_kit_bevy::BevyTheme;
use panel_kit_core::badge::BadgeSpec;
use panel_kit_core::widgets::ContentSpec;

/// Bindings whose content changes on every [`CanaryData::tick`].
pub const LIVE_BINDINGS: [&str; 4] = [
    "canary.activity",
    "canary.flame",
    "canary.distribution",
    "canary.theme",
];

/// Mutable provider state behind the canary bindings.
#[derive(Resource)]
pub struct CanaryData {
    pub metrics: Metrics,
    pub badges: Vec<BadgeSpec>,
    pub paper: bool,
    pub tick: u64,
}

impl Default for CanaryData {
    fn default() -> Self {
        Self {
            metrics: Metrics::new(),
            badges: demo_badges(),
            paper: false,
            tick: 0,
        }
    }
}

impl CanaryData {
    /// Interval between live samples.
    pub const TICK: Duration = Duration::from_millis(100);

    /// Advance the live metrics one sample.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.metrics.tick();
    }
}

/// The painted panel's facts the Workspace card reports.
#[derive(Clone, Copy, Debug)]
pub struct WorkspaceFacts<'a> {
    pub title: &'a str,
    pub maximized: bool,
}

/// Paint one panel body. Bindings outside the canary fall through to the
/// generic content painter, so a re-authored spec still renders its kind.
pub fn paint(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    data: &CanaryData,
    facts: WorkspaceFacts<'_>,
    binding: &str,
    content_spec: &ContentSpec,
) {
    match binding {
        "canary.workspace" => workspace_intro(parent, theme, facts),
        "canary.activity" => {
            let unit = match content_spec {
                ContentSpec::TimeSeries { unit, .. } => unit.as_str(),
                _ => "",
            };
            time_series::spawn(parent, theme, &data.metrics.series(), unit);
        }
        "canary.flame" => flamegraph::spawn(parent, theme, &data.metrics.flame()),
        "canary.notes" => notes(parent, theme),
        "canary.badges" => widgets::badges(parent, theme, &data.badges),
        "canary.nodes" => {
            widgets::table(parent, &node_rows());
        }
        "canary.capacity" => gauges::spawn(parent, theme, &capacity_items()),
        "canary.distribution" => boxplot::spawn(parent, theme, &data.metrics.boxes()),
        "canary.theme" => theme_panel(parent, theme, data),
        _ => content::spawn(parent, theme, content_spec),
    }
}

/// `.pk-custom-card`: a column of a tracked-caps heading and its items,
/// `.35rem` apart.
fn card(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    heading: &str,
    build: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(theme.metrics.rem(0.35)),
            ..default()
        })
        .with_children(|card| {
            // `.pk-custom-card h2`: .85rem, bold, .08em tracking, uppercase.
            styled_text(card, heading, TextStyle::rem(theme, 0.85).bold().tracked_caps(0.08));
            build(card);
        });
}

/// The shared Workspace card. Bevy panel headers carry no header actions,
/// so their click count is 0.
fn workspace_intro(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, facts: WorkspaceFacts<'_>) {
    let muted = TextStyle::body(theme).color(theme.text_muted).wrapping();
    card(parent, theme, WORKSPACE_CARD_HEADING, |card| {
        for line in workspace_card_lines(facts.title, facts.maximized, 0) {
            styled_text(card, line, muted);
        }
    });
}

/// The shared notes text in a scroll viewport, line for line, so the panel
/// scrolls under the wheel like the TUI and web canaries.
fn notes(parent: &mut ChildSpawnerCommands, theme: &BevyTheme) {
    let text = notes_text();
    scroll::spawn(parent, theme, &scroll::ScrollState::for_theme(theme), text.lines());
}

fn theme_panel(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, data: &CanaryData) {
    card(parent, theme, theme_preset_label(data.paper), |card| {
        // `.spec-list`: .25rem block margin, 1.1rem marker gutter, muted
        // items behind an outside `• ` marker.
        let gutter = theme.metrics.rem(1.1);
        let item = TextStyle::body(theme).color(theme.text_muted);
        card.spawn(Node {
            flex_direction: FlexDirection::Column,
            margin: UiRect::vertical(Val::Px(theme.metrics.rem(0.25))),
            padding: UiRect::left(Val::Px(gutter)),
            ..default()
        })
        .with_children(|list| {
            for (slot, caption) in THEME_SWATCHES {
                list.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|line| {
                    line.spawn(Node {
                        position_type: PositionType::Absolute,
                        right: Val::Percent(100.0),
                        width: Val::Px(gutter),
                        justify_content: JustifyContent::FlexEnd,
                        ..default()
                    })
                    .with_children(|marker| {
                        styled_text(marker, "• ", item);
                    });
                    styled_text(line, "## ", item.color(theme.color(slot)));
                    styled_text(line, caption, item);
                });
            }
        });
        spinner::spinner(
            card,
            theme,
            "spinner painter",
            data.tick / 2,
            CanaryData::TICK * data.tick as u32,
        );
    });
}
