//! Renderer-neutral widget content models.
//!
//! Core owns only semantic data and small pure calculations. Web and terminal
//! crates translate these models into DOM or ratatui primitives at their paint
//! boundaries.

pub mod badge;
pub mod cascade;
pub mod charts;
pub mod dropdown;
pub mod meter;
pub mod scroll;
pub mod spinner;
pub mod status;
pub mod table;

use serde::{Deserialize, Serialize};

use crate::badge::BadgeSpec;
use charts::{BoxItemModel, BoxItemView, FlameSpanModel, GaugeModel, SeriesModel, SeriesView};
use meter::MeterModel;
use status::StatusModel;
use table::{TableModel, TableView};

/// Authored or runtime-supplied data for a content model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum DataSource<T> {
    /// The complete value is present in the authored spec.
    Inline {
        /// Owned content value.
        value: T,
    },
    /// A host/application provider supplies the data at runtime.
    Binding {
        /// Stable binding identifier resolved by the host application.
        id: String,
    },
}

impl<T> DataSource<T> {
    /// Whether this source owns its value inline.
    pub fn is_inline(&self) -> bool {
        matches!(self, Self::Inline { .. })
    }

    /// The binding identifier when this source is provider-backed.
    pub fn binding_id(&self) -> Option<&str> {
        match self {
            Self::Binding { id } => Some(id),
            Self::Inline { .. } => None,
        }
    }
}

/// Scroll behavior requested by an authored text content spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ScrollPolicy {
    /// Clip content to the visible viewport.
    Clip,
    /// Hard-wrap text to the content width.
    Wrap,
    /// Let the backend use its native overflow behavior.
    Auto,
}

/// Owned text content authored in a workspace spec.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TextModel {
    /// Text body to present.
    pub text: String,
}

/// The complete closed set of renderer-neutral panel content kinds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContentSpec {
    /// Application-native content, addressed by a host binding.
    Custom {
        /// Host binding identifier.
        binding: String,
    },
    /// Plain text content with a renderer-neutral scroll policy.
    Text {
        /// Authored or bound text model.
        source: DataSource<TextModel>,
        /// Requested scrolling/wrapping policy.
        scroll: ScrollPolicy,
    },
    /// Native editor binding; renderer support stays backend-specific.
    Editor {
        /// Host binding identifier.
        binding: String,
        /// Whether the editor accepts multiple lines.
        multiline: bool,
        /// Empty-state prompt text.
        placeholder: String,
    },
    /// Badge strip content.
    Badges {
        /// Authored or bound badge strip.
        source: DataSource<Vec<BadgeSpec>>,
    },
    /// Semantic table content.
    Table {
        /// Authored or bound table model.
        source: DataSource<TableModel>,
    },
    /// Named point series with a unit label.
    TimeSeries {
        /// Authored or bound series.
        source: DataSource<Vec<SeriesModel>>,
        /// Unit shown on the value axis.
        unit: String,
    },
    /// Horizontal gauge stack.
    Gauges {
        /// Authored or bound gauges.
        source: DataSource<Vec<GaugeModel>>,
    },
    /// Flamegraph spans in flattened preorder.
    Flamegraph {
        /// Authored or bound flamegraph spans.
        source: DataSource<Vec<FlameSpanModel>>,
    },
    /// Box-and-whisker distributions.
    Boxplot {
        /// Authored or bound boxplot distributions.
        source: DataSource<Vec<BoxItemModel>>,
    },
    /// Single labeled meter.
    Meter {
        /// Authored or bound meter.
        source: DataSource<MeterModel>,
    },
    /// Single labeled status value.
    Status {
        /// Authored or bound status.
        source: DataSource<StatusModel>,
    },
    /// Small loading indicator.
    Spinner {
        /// Authored or bound spinner label.
        label: DataSource<String>,
    },
}

impl ContentSpec {
    /// Stable normalized name for this authored content variant.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Custom { .. } => "custom",
            Self::Text { .. } => "text",
            Self::Editor { .. } => "editor",
            Self::Badges { .. } => "badges",
            Self::Table { .. } => "table",
            Self::TimeSeries { .. } => "time_series",
            Self::Gauges { .. } => "gauges",
            Self::Flamegraph { .. } => "flamegraph",
            Self::Boxplot { .. } => "boxplot",
            Self::Meter { .. } => "meter",
            Self::Status { .. } => "status",
            Self::Spinner { .. } => "spinner",
        }
    }
}

/// Borrowed runtime content passed from providers to painters.
pub enum ContentView<'a> {
    /// Application-native content binding.
    Custom {
        /// Host binding identifier.
        binding: &'a str,
    },
    /// Plain text body.
    Text {
        /// Borrowed text body.
        text: &'a str,
        /// Renderer-neutral scroll policy.
        scroll: ScrollPolicy,
    },
    /// Native editor binding.
    Editor {
        /// Host binding identifier.
        binding: &'a str,
    },
    /// Borrowed badge strip.
    Badges(&'a [BadgeSpec]),
    /// Borrowed semantic table.
    Table(TableView<'a>),
    /// Borrowed time-series data and unit label.
    TimeSeries {
        /// Borrowed time-series data.
        series: &'a [SeriesView<'a>],
        /// Unit shown on the value axis.
        unit: &'a str,
    },
    /// Borrowed gauge stack.
    Gauges(&'a [GaugeModel]),
    /// Borrowed flamegraph spans.
    Flamegraph(&'a [FlameSpanModel]),
    /// Borrowed boxplot summaries.
    Boxplot(&'a [BoxItemView<'a>]),
    /// Borrowed meter.
    Meter(&'a MeterModel),
    /// Borrowed status.
    Status(&'a StatusModel),
    /// Borrowed spinner label.
    Spinner {
        /// Borrowed spinner label.
        label: &'a str,
    },
}

impl ContentView<'_> {
    /// Normalized content kind name for this borrowed view.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Custom { .. } => "custom",
            Self::Text { .. } => "text",
            Self::Editor { .. } => "editor",
            Self::Badges(_) => "badges",
            Self::Table(_) => "table",
            Self::TimeSeries { .. } => "time_series",
            Self::Gauges(_) => "gauges",
            Self::Flamegraph(_) => "flamegraph",
            Self::Boxplot(_) => "boxplot",
            Self::Meter(_) => "meter",
            Self::Status(_) => "status",
            Self::Spinner { .. } => "spinner",
        }
    }

    /// Item count when the view naturally exposes a collection.
    pub fn len(&self) -> Option<usize> {
        match self {
            Self::Badges(items) => Some(items.len()),
            Self::TimeSeries { series, .. } => Some(series.len()),
            Self::Gauges(items) => Some(items.len()),
            Self::Flamegraph(items) => Some(items.len()),
            Self::Boxplot(items) => Some(items.len()),
            Self::Table(table) => Some(table.rows.len()),
            _ => None,
        }
    }

    /// Whether this borrowed view naturally exposes an empty collection.
    pub fn is_empty(&self) -> Option<bool> {
        self.len().map(|len| len == 0)
    }
}
#[cfg(test)]
mod tests;
