//! `ContentSpec` dispatcher for the Bevy widget set.
//!
//! Hosts hand one authored content model to [`spawn`]; the matching widget
//! module paints it. Inline data is painted directly (an inline external body
//! is a graph, solved here), host bindings show a deterministic placeholder
//! (the host owns provider resolution).

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
        ContentSpec::External { source: DataSource::Inline { value }, layout, .. } => {
            super::graph::spawn_solved(parent, theme, value, layout, None);
        }
        ContentSpec::External { component, .. } => {
            super::label_text(parent, format!("external: {component}"), 11.0, theme.text_muted);
        }
    }
}

fn binding_placeholder(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, id: &str) {
    super::label_text(parent, format!("binding: {id}"), 11.0, theme.text_muted);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::graph::GraphNodeDot;
    use bevy::ecs::system::RunSystemOnce;
    use panel_kit_core::theme::ThemeTokens;
    use panel_kit_core::widgets::graph::{GraphEdge, GraphLayout, GraphModel, GraphNode, LayoutAlgorithm};

    fn external(source: DataSource<GraphModel>) -> ContentSpec {
        ContentSpec::External {
            component: "graph".into(),
            source,
            layout: GraphLayout { algorithm: LayoutAlgorithm::Circle, ..GraphLayout::default() },
        }
    }

    fn graph() -> GraphModel {
        let node = |id: &str| GraphNode {
            id: id.into(),
            label: id.into(),
            sort: "metric".into(),
            color: (1, 2, 3),
            size: 3.0,
            mass: 1.0,
            repulsion: 1.0,
            species: String::new(),
        };
        GraphModel {
            nodes: vec![node("a"), node("b"), node("c")],
            edges: vec![GraphEdge {
                source: 0,
                target: 1,
                kind: String::new(),
                weight: 1.0,
                rest_length: 1.0,
            }],
        }
    }

    /// The painted nodes' dot count and every text string.
    fn painted(content: ContentSpec) -> (usize, Vec<String>) {
        let theme = BevyTheme::from_tokens(&ThemeTokens::dark());
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        world
            .run_system_once(move |mut commands: Commands| {
                commands
                    .entity(root)
                    .with_children(|parent| spawn(parent, &theme, &content));
            })
            .expect("content spawns");
        let dots = world.query::<&GraphNodeDot>().iter(&world).count();
        let texts = world.query::<&Text>().iter(&world).map(|text| text.0.clone()).collect();
        (dots, texts)
    }

    #[test]
    fn an_inline_external_body_paints_its_graph() {
        let (dots, texts) = painted(external(DataSource::Inline { value: graph() }));

        assert_eq!(dots, 3);
        assert!(texts.is_empty(), "{texts:?}");
    }

    #[test]
    fn a_bound_external_body_keeps_the_component_label() {
        let (dots, texts) = painted(external(DataSource::Binding { id: "host.graph".into() }));

        assert_eq!(dots, 0);
        assert_eq!(texts, ["external: graph"]);
    }
}
