//! Declarative contract for panel bodies panel-kit does not render itself.
//!
//! A host mounts the component (an imperative canvas, a WebGL scene) and
//! panel-kit owns everything around it: which contract the body implements,
//! which capabilities it needs, whether it survives minimize/maximize, how
//! small it may get, which events it emits, and what to paint when the host
//! lacks a capability. Those are spec data here, not host policy.

use serde::{Deserialize, Serialize};

/// The closed set of bodies an external component can implement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ComponentContract {
    /// Nodes and edges over [`crate::widgets::graph::GraphModel`].
    Graph,
}

/// The closed set of host capabilities a component may require.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// The host can run a WebGPU compute/render pipeline.
    #[serde(rename = "webgpu")]
    WebGpu,
}

/// The closed set of events a component may emit at the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ComponentEventKind {
    /// The user selected a node.
    Select,
    /// The pointer entered or left a node; `Hover` clears with `None`.
    Hover,
}

/// What a backend paints when the host lacks a required capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ComponentFallback {
    /// The backend's built-in renderer-neutral painter over the same data.
    Cells,
    /// An honest "renderer unavailable" body.
    Notice,
}

/// One declared external component.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExternalComponentSpec {
    /// Stable identifier panels reference as `content.component`.
    pub id: String,
    /// Contract the mounted body implements.
    pub contract: ComponentContract,
    /// Capabilities the host must provide; empty means any host.
    pub requires: Vec<Capability>,
    /// Whether the panel stays mounted while minimized or maximized.
    pub keep_mounted: bool,
    /// Minimum width and height in the spec's units.
    pub min_size: [f64; 2],
    /// Events the component may emit.
    pub events: Vec<ComponentEventKind>,
    /// Body to paint when a required capability is missing.
    pub fallback: ComponentFallback,
}

/// One runtime event from a mounted external component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComponentEvent {
    /// The user selected the node with this id.
    Select {
        /// Selected node id.
        node: String,
    },
    /// The pointer entered a node, or left the graph entirely.
    Hover {
        /// Hovered node id, or `None` when the pointer left.
        node: Option<String>,
    },
}

/// Whether the host provides everything the component requires.
pub fn component_satisfied(spec: &ExternalComponentSpec, available: &[Capability]) -> bool {
    spec.requires
        .iter()
        .all(|capability| available.contains(capability))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(requires: Vec<Capability>) -> ExternalComponentSpec {
        ExternalComponentSpec {
            id: "graph".into(),
            contract: ComponentContract::Graph,
            requires,
            keep_mounted: true,
            min_size: [320.0, 240.0],
            events: vec![ComponentEventKind::Select, ComponentEventKind::Hover],
            fallback: ComponentFallback::Cells,
        }
    }

    #[test]
    fn capability_set_satisfies_only_when_every_requirement_is_present() {
        let none = component(Vec::new());
        let webgpu = component(vec![Capability::WebGpu]);

        assert!(component_satisfied(&none, &[]));
        assert!(!component_satisfied(&webgpu, &[]));
        assert!(component_satisfied(&webgpu, &[Capability::WebGpu]));
    }

    #[test]
    fn component_spec_encodes_snake_case_contract_data() {
        let encoded = serde_json::to_value(component(vec![Capability::WebGpu]))
            .expect("component spec serializes");

        assert_eq!(encoded["contract"], serde_json::json!("graph"));
        assert_eq!(encoded["requires"], serde_json::json!(["webgpu"]));
        assert_eq!(encoded["events"], serde_json::json!(["select", "hover"]));
        assert_eq!(encoded["fallback"], serde_json::json!("cells"));
        assert_eq!(encoded["keep_mounted"], serde_json::json!(true));
        assert_eq!(encoded["min_size"], serde_json::json!([320.0, 240.0]));
    }
}