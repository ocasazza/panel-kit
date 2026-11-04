//! `ContentSpec` dispatcher for the Bevy widget set.
//!
//! Hosts hand one authored content model to [`spawn`]; the matching widget
//! module paints it. Inline data is painted directly, host bindings show a
//! deterministic placeholder (the host owns provider resolution).

use bevy::prelude::*;
use panel_kit_core::widgets::charts::SeriesView;
use panel_kit_core::widgets::{ContentSpec, DataSource};

use crate::theme::BevyTheme;

/// Paint one authored content model.
pub fn spawn(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, content: &ContentSpec) {
    match content {
        ContentSpec::Text { .. } | ContentSpec::Editor { .. } | ContentSpec::Custom { .. } => {
            super::text::spawn(parent, theme, content);
        }
        ContentSpec::Badges { source } => match source {
            DataSource::Inline { value } => super::badges(parent, theme, value),
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
        ContentSpec::Table { source } => match source {
            DataSource::Inline { value } => {
                super::table(parent, value);
            }
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
        ContentSpec::TimeSeries { source, unit } => match source {
            DataSource::Inline { value } => {
                let views: Vec<SeriesView<'_>> = value
                    .iter()
                    .map(|series| SeriesView {
                        name: &series.name,
                        points: &series.points,
                    })
                    .collect();
                super::time_series::spawn(parent, theme, &views, unit);
            }
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
        ContentSpec::Gauges { source } => match source {
            DataSource::Inline { value } => super::gauges::spawn(parent, theme, value),
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
        ContentSpec::Flamegraph { source } => match source {
            DataSource::Inline { value } => super::flamegraph::spawn(parent, theme, value),
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
        ContentSpec::Boxplot { source } => match source {
            DataSource::Inline { value } => super::boxplot::spawn_models(parent, theme, value),
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
        ContentSpec::Meter { source } => match source {
            DataSource::Inline { value } => super::meter(parent, theme, value),
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
        ContentSpec::Status { source } => match source {
            DataSource::Inline { value } => super::status(parent, theme, value),
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
        ContentSpec::Spinner { label } => match label {
            DataSource::Inline { value } => {
                super::spinner::spinner(parent, theme, value, 0, std::time::Duration::ZERO)
            }
            DataSource::Binding { id } => binding_placeholder(parent, theme, id),
        },
    }
}

fn binding_placeholder(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, id: &str) {
    super::label_text(parent, format!("binding: {id}"), 11.0, theme.text_muted);
}
