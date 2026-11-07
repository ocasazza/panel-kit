//! Strict authored workspace specifications.
//!
//! `WorkspaceSpec` is normalized authored configuration, distinct from saved
//! operator layout state. JSON decoding intentionally runs through a
//! `serde_json::Value` walk before typed deserialization so structural errors
//! (unknown fields, missing fields, and nullable-field absence) accumulate with
//! document-ordered JSON pointers instead of stopping at serde's first error.

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::persist::SavePolicy;
use crate::reducer::{ResizePolicy, Snapshot, Viewport};
use crate::theme::ThemeTokens;
use crate::widgets::ContentSpec;
use crate::keymap::{Keymap, KeymapError, KeymapSpec};
use crate::{
    ChromeMetrics, Clamp, CommandStep, Mode, PanelCatalog, PanelMeta, PanelWin, SpecPanelId,
    SurfaceCapabilities, TileMetrics, Units, WinState, TILE_H_MAX, TILE_W_MAX,
};

mod logical_px;

#[cfg(feature = "spec-json")]
mod strict;

pub mod component;

pub use component::{
    Capability, ComponentContract, ComponentEvent, ComponentEventKind, ComponentFallback,
    ExternalComponentSpec, component_satisfied,
};

/// Current authored workspace-spec version.
pub const WORKSPACE_SPEC_VERSION: u32 = 1;

/// Author-selected glyph family for terminal-like renderers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Charset {
    /// Unicode box drawing and traffic-light glyphs.
    Unicode,
    /// ASCII-compatible fallback glyphs.
    Ascii,
}

/// Authoritative strict workspace specification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSpec {
    /// Spec format version.
    pub spec_version: u32,
    /// Stable workspace identifier.
    pub id: String,
    /// Initial layout policy and geometry.
    pub layout: LayoutSpec,
    /// Surface classification policy.
    pub surface: SurfaceSpec,
    /// Shared chrome toggles and metrics.
    pub chrome: ChromeSpec,
    /// Keyboard movement steps and keymap layers.
    pub input: InputSpec,
    /// Glyph family requested by terminal-like backends.
    pub glyphs: Charset,
    /// Complete theme token set.
    pub theme: ThemeTokens,
    /// Persistence policy requested by the authored workspace.
    pub persistence: PersistenceSpec,
    /// Authored panels in declaration order.
    pub panels: Vec<PanelSpec>,
    /// Declared external components; empty when every body is panel-kit's.
    #[serde(default)]
    pub components: Vec<ExternalComponentSpec>,
}

/// Layout defaults for a workspace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LayoutSpec {
    /// Coordinate units used by authored rectangles.
    pub units: Units,
    /// Authored viewport width and height.
    pub viewport: [f64; 2],
    /// Initial preferred layout mode.
    pub preferred_mode: Mode,
    /// Floating clamp constants.
    pub clamp: Clamp,
    /// Tiling layout constants.
    pub tile: TileLayoutSpec,
}

/// Tiling layout constants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TileLayoutSpec {
    /// Existing span-resize metrics reused by reducers/projectors.
    pub resize: TileMetrics,
    /// Minimum row height.
    pub row_min: f64,
    /// Gap between tiled panels.
    pub gap: f64,
    /// Padding around the tile grid.
    pub padding: f64,
    /// Whether the tile grid fills the viewport.
    pub fill_viewport: bool,
}

/// Surface classification policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SurfaceSpec {
    /// Upper bound for compact surfaces.
    pub compact_max: f64,
    /// Upper bound for tablet surfaces.
    pub tablet_max: f64,
    /// Capabilities assumed when a backend cannot observe them directly.
    pub fallback_capabilities: SurfaceCapabilities,
    /// How viewport changes affect authored geometry.
    pub resize_policy: ResizePolicy,
}

/// Chrome metrics and feature toggles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ChromeSpec {
    /// Shared workspace chrome metrics.
    pub metrics: ChromeMetrics,
    /// Minimum interactive target size.
    pub hit_target_min: f64,
    /// Header height inside each panel.
    pub panel_header_h: f64,
    /// Whether the outer panel frame is drawn.
    pub panel_frame: bool,
    /// Whether the title appears in the border/header.
    pub title_in_border: bool,
    /// Whether the mode toggle is exposed.
    pub mode_control: bool,
    /// Whether the minimize control is exposed.
    pub minimize_control: bool,
    /// Whether the maximize control is exposed.
    pub maximize_control: bool,
    /// Whether the resize grip is exposed.
    pub resize_grip: bool,
    /// Whether the minimized-panel dock is exposed.
    pub dock: bool,
    /// Accessible/visible dock label.
    pub dock_label: String,
}

impl ChromeSpec {
    /// Only panel bodies/surfaces are enabled; chrome and dock are disabled.
    pub fn surface_only() -> Self {
        Self {
            metrics: ChromeMetrics::WEB,
            hit_target_min: 0.0,
            panel_header_h: 0.0,
            panel_frame: false,
            title_in_border: false,
            mode_control: false,
            minimize_control: false,
            maximize_control: false,
            resize_grip: false,
            dock: false,
            dock_label: String::new(),
        }
    }

    /// Full chrome with the supplied shared metrics.
    pub fn full(metrics: ChromeMetrics) -> Self {
        Self {
            metrics,
            hit_target_min: 24.0,
            panel_header_h: 28.0,
            panel_frame: true,
            title_in_border: true,
            mode_control: true,
            minimize_control: true,
            maximize_control: true,
            resize_grip: true,
            dock: true,
            dock_label: "Dock".into(),
        }
    }
}

/// Keyboard command configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InputSpec {
    /// Geometry step sizes for keyboard movement/resizing.
    pub steps: CommandStep,
    /// Keymap layers in precedence order; later layers shadow earlier ones.
    /// Validated by compiling with the untyped (`String`) vocabulary, so
    /// token, mode, and prefix errors surface at spec load.
    pub keymaps: Vec<KeymapSpec>,
}

/// Authored persistence policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PersistenceSpec {
    /// Whether this workspace participates in persistence.
    pub enabled: bool,
    /// Logical persistence key.
    pub key: String,
    /// Whether hosts should restore an existing record on startup.
    pub restore: bool,
    /// Save timing selected by the host after reductions.
    pub save_policy: SavePolicy,
}

/// One authored panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelSpec {
    /// Stable authored panel ID.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Stable CSS/provider slug.
    pub slug: String,
    /// Initial window geometry and state.
    pub window: WindowSpec,
    /// Renderer-neutral content model.
    pub content: ContentSpec,
}

/// Initial panel window placement.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WindowSpec {
    /// Left edge in authored units.
    pub x: f64,
    /// Top edge in authored units.
    pub y: f64,
    /// Width in authored units.
    pub w: f64,
    /// Height in authored units.
    pub h: f64,
    /// Initial window state.
    pub state: WinState,
    /// Floating stacking order.
    pub z: i32,
    /// Tiling width span.
    pub tile_w: u8,
    /// Tiling height span.
    pub tile_h: u8,
}

/// Backend family used by a provider manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// Dioxus web backend.
    Web,
    /// Native ratatui backend.
    Tui,
    /// Browser-hosted terminal backend.
    BrowserTui,
    /// Native Bevy UI backend.
    Bevy,
}

/// Stable content kind names used by provider manifests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    /// Application-native content binding.
    Custom,
    /// Text panel.
    Text,
    /// Editor panel.
    Editor,
    /// Badge strip.
    Badges,
    /// Table.
    Table,
    /// Time series chart.
    TimeSeries,
    /// Gauges.
    Gauges,
    /// Flamegraph.
    Flamegraph,
    /// Boxplot.
    Boxplot,
    /// Meter.
    Meter,
    /// Status.
    Status,
    /// Spinner.
    Spinner,
    /// Host-mounted component declared by `WorkspaceSpec::components`.
    External,
}

/// Provider-side declaration for one panel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelProviderDeclaration {
    /// Authored panel ID served by this provider.
    pub panel_id: String,
    /// Content kind the provider implements.
    pub content_kind: ContentKind,
    /// Optional runtime binding ID. The field is required but nullable.
    pub binding: Option<String>,
}

/// Provider manifest supplied by a backend/app.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "spec-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BindingManifest {
    /// Backend family this manifest serves.
    pub backend: BackendKind,
    /// Provider declarations.
    pub panels: Vec<PanelProviderDeclaration>,
}

/// Immutable resolved workspace data plus host-owned initial snapshot.
pub struct ResolvedWorkspace {
    /// Spec format version.
    pub spec_version: u32,
    /// Stable workspace identifier.
    pub id: String,
    /// Interned panel catalog.
    pub catalog: PanelCatalog<SpecPanelId>,
    /// Initial host-owned snapshot.
    pub initial: Snapshot<SpecPanelId>,
    /// Authored layout policy.
    pub layout: LayoutSpec,
    /// Authored surface policy.
    pub surface: SurfaceSpec,
    /// Authored chrome policy.
    pub chrome: ChromeSpec,
    /// Authored input policy.
    pub input: InputSpec,
    /// Authored glyph family.
    pub glyphs: Charset,
    /// Authored theme tokens.
    pub theme: ThemeTokens,
    /// Authored persistence policy.
    pub persistence: PersistenceSpec,
    /// Authored panel content and metadata in declaration order.
    pub panels: Vec<PanelSpec>,
    /// Declared external components in declaration order.
    pub components: Vec<ExternalComponentSpec>,
}

/// One strict-spec diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecDiagnostic {
    pointer: String,
    message: String,
}

impl SpecDiagnostic {
    /// JSON pointer to the offending field or value.
    pub fn pointer(&self) -> &str {
        &self.pointer
    }

    /// Human-readable diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Ordered collection of spec diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecErrors {
    diagnostics: Vec<SpecDiagnostic>,
}

impl SpecErrors {
    /// Whether no diagnostics were accumulated.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Iterate in document/validation order.
    pub fn iter(&self) -> impl Iterator<Item = &SpecDiagnostic> {
        self.diagnostics.iter()
    }

    fn one(pointer: impl Into<String>, message: impl Into<String>) -> Self {
        let mut errors = ErrorSink::new();
        errors.push(pointer, message);
        errors.finish().expect_err("one error exists")
    }
}

impl fmt::Display for SpecErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{}: {}", diagnostic.pointer, diagnostic.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for SpecErrors {}

impl WorkspaceSpec {
    /// Decode from a JSON value with all strict diagnostics accumulated first.
    #[cfg(feature = "spec-json")]
    pub fn from_json_value(value: serde_json::Value) -> Result<Self, SpecErrors> {
        let mut errors = ErrorSink::new();
        strict::walk_workspace(&value, "", &mut errors);
        errors.finish()?;

        let spec: Self = serde_json::from_value(value)
            .map_err(|error| SpecErrors::one("", error.to_string()))?;
        spec.validate()?;
        Ok(spec)
    }

    /// Decode from JSON text with all strict diagnostics accumulated first.
    #[cfg(feature = "spec-json")]
    pub fn from_json_str(text: &str) -> Result<Self, SpecErrors> {
        let value =
            serde_json::from_str(text).map_err(|error| SpecErrors::one("", error.to_string()))?;
        Self::from_json_value(value)
    }

    /// Validate typed authored configuration.
    pub fn validate(&self) -> Result<(), SpecErrors> {
        let mut errors = ErrorSink::new();
        validate_workspace(self, &mut errors);
        errors.finish()
    }

    /// Validate, bind providers, intern IDs, and produce initial host state.
    pub fn resolve(self, providers: &BindingManifest) -> Result<ResolvedWorkspace, SpecErrors> {
        self.validate()?;
        validate_manifest(providers)?;
        validate_providers(&self, providers)?;

        let entries = self
            .panels
            .iter()
            .enumerate()
            .map(|(index, panel)| PanelMeta {
                key: SpecPanelId::from_index(index),
                stable_id: panel.id.clone().into_boxed_str(),
                title: panel.title.clone().into_boxed_str(),
                slug: panel.slug.clone().into_boxed_str(),
            })
            .collect();
        let catalog = PanelCatalog::try_new(entries)
            .map_err(|error| SpecErrors::one("/panels", error.to_string()))?;
        let initial = Snapshot::from_defaults(
            self.panel_windows(),
            self.layout.preferred_mode,
            self.viewport(),
        );

        Ok(ResolvedWorkspace {
            spec_version: self.spec_version,
            id: self.id,
            catalog,
            initial,
            layout: self.layout,
            surface: self.surface,
            chrome: self.chrome,
            input: self.input,
            glyphs: self.glyphs,
            theme: self.theme,
            persistence: self.persistence,
            panels: self.panels,
            components: self.components,
        })
    }

    /// The declared external component with this id, if any.
    pub fn component(&self, id: &str) -> Option<&ExternalComponentSpec> {
        self.components.iter().find(|component| component.id == id)
    }

    fn panel_windows(&self) -> Vec<PanelWin<SpecPanelId>> {
        self.panels
            .iter()
            .enumerate()
            .map(|(index, panel)| panel.window.to_panel_win(SpecPanelId::from_index(index)))
            .collect()
    }

    fn viewport(&self) -> Viewport {
        Viewport {
            width: self.layout.viewport[0],
            height: self.layout.viewport[1],
            units: self.layout.units,
        }
    }
}

impl BindingManifest {
    /// Decode a provider manifest with required-nullable checking.
    #[cfg(feature = "spec-json")]
    pub fn from_json_value(value: serde_json::Value) -> Result<Self, SpecErrors> {
        let mut errors = ErrorSink::new();
        strict::walk_manifest(&value, "", &mut errors);
        errors.finish()?;
        serde_json::from_value(value).map_err(|error| SpecErrors::one("", error.to_string()))
    }
}

impl PanelSpec {
    /// Build the provider declaration required to resolve this authored panel.
    pub fn provider_declaration(&self) -> PanelProviderDeclaration {
        PanelProviderDeclaration {
            panel_id: self.id.clone(),
            content_kind: ContentKind::from_spec(&self.content),
            binding: self.content.binding_id().map(str::to_owned),
        }
    }

    /// The external component this panel's body names, if it has one.
    pub fn external_component(&self) -> Option<&str> {
        match &self.content {
            ContentSpec::External { component, .. } => Some(component),
            _ => None,
        }
    }
}

impl ContentSpec {
    /// Return the host binding ID carried directly by this content, if any.
    pub fn binding_id(&self) -> Option<&str> {
        match self {
            Self::Custom { binding } | Self::Editor { binding, .. } => Some(binding),
            Self::Text { source, .. } => source.binding_id(),
            Self::Badges { source } => source.binding_id(),
            Self::Table { source } => source.binding_id(),
            Self::TimeSeries { source, .. } => source.binding_id(),
            Self::Gauges { source } => source.binding_id(),
            Self::Flamegraph { source } => source.binding_id(),
            Self::Boxplot { source } => source.binding_id(),
            Self::Meter { source } => source.binding_id(),
            Self::Status { source } => source.binding_id(),
            Self::Spinner { label } => label.binding_id(),
            Self::External { source, .. } => source.binding_id(),
        }
    }
}

impl WindowSpec {
    fn to_panel_win(self, kind: SpecPanelId) -> PanelWin<SpecPanelId> {
        PanelWin {
            kind,
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
            state: self.state,
            z: self.z,
            tile_w: self.tile_w,
            tile_h: self.tile_h,
        }
    }
}

impl ContentKind {
    /// Classify an authored content spec into the provider-manifest kind.
    pub fn from_spec(content: &ContentSpec) -> Self {
        match content {
            ContentSpec::Custom { .. } => Self::Custom,
            ContentSpec::Text { .. } => Self::Text,
            ContentSpec::Editor { .. } => Self::Editor,
            ContentSpec::Badges { .. } => Self::Badges,
            ContentSpec::Table { .. } => Self::Table,
            ContentSpec::TimeSeries { .. } => Self::TimeSeries,
            ContentSpec::Gauges { .. } => Self::Gauges,
            ContentSpec::Flamegraph { .. } => Self::Flamegraph,
            ContentSpec::Boxplot { .. } => Self::Boxplot,
            ContentSpec::Meter { .. } => Self::Meter,
            ContentSpec::Status { .. } => Self::Status,
            ContentSpec::Spinner { .. } => Self::Spinner,
            ContentSpec::External { .. } => Self::External,
        }
    }
}

impl ResolvedWorkspace {
    /// The declaration a panel's external body names, if it has one.
    pub fn panel_component(&self, panel: &PanelSpec) -> Option<&ExternalComponentSpec> {
        panel
            .external_component()
            .and_then(|id| self.components.iter().find(|component| component.id == id))
    }
}

pub(super) struct ErrorSink {
    diagnostics: Vec<SpecDiagnostic>,
}

impl ErrorSink {
    fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    pub(super) fn push(&mut self, pointer: impl Into<String>, message: impl Into<String>) {
        self.diagnostics.push(SpecDiagnostic {
            pointer: pointer.into(),
            message: message.into(),
        });
    }

    fn finish(self) -> Result<(), SpecErrors> {
        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(SpecErrors {
                diagnostics: self.diagnostics,
            })
        }
    }
}

fn validate_workspace(spec: &WorkspaceSpec, errors: &mut ErrorSink) {
    if spec.spec_version != WORKSPACE_SPEC_VERSION {
        errors.push("/spec_version", "unsupported workspace spec version");
    }
    if spec.id.is_empty() {
        errors.push("/id", "workspace id must not be empty");
    }
    validate_layout(&spec.layout, errors);
    validate_surface(&spec.surface, errors);
    validate_input(&spec.input, errors);
    validate_panels(&spec.panels, errors);
    validate_components(&spec.components, &spec.panels, errors);
}

fn validate_layout(layout: &LayoutSpec, errors: &mut ErrorSink) {
    for (index, dimension) in layout.viewport.iter().enumerate() {
        if !dimension.is_finite() || *dimension <= 0.0 {
            errors.push(
                format!("/layout/viewport/{index}"),
                "viewport dimension must be finite and positive",
            );
        }
    }
    if layout.tile.row_min <= 0.0 || !layout.tile.row_min.is_finite() {
        errors.push(
            "/layout/tile/row_min",
            "row_min must be finite and positive",
        );
    }
    if layout.tile.gap < 0.0 || !layout.tile.gap.is_finite() {
        errors.push("/layout/tile/gap", "gap must be finite and non-negative");
    }
    if layout.tile.padding < 0.0 || !layout.tile.padding.is_finite() {
        errors.push(
            "/layout/tile/padding",
            "padding must be finite and non-negative",
        );
    }
}

fn validate_surface(surface: &SurfaceSpec, errors: &mut ErrorSink) {
    if surface.compact_max <= 0.0 || !surface.compact_max.is_finite() {
        errors.push(
            "/surface/compact_max",
            "compact_max must be finite and positive",
        );
    }
    if surface.tablet_max <= surface.compact_max || !surface.tablet_max.is_finite() {
        errors.push(
            "/surface/tablet_max",
            "tablet_max must be finite and greater than compact_max",
        );
    }
}

fn validate_input(input: &InputSpec, errors: &mut ErrorSink) {
    let mut ids = HashSet::new();
    for (index, layer) in input.keymaps.iter().enumerate() {
        if !ids.insert(layer.id.as_str()) {
            errors.push(format!("/input/keymaps/{index}/id"), "duplicate keymap id");
        }
    }
    if let Err(error) = Keymap::<String>::compile(&input.keymaps) {
        let layer = match &error {
            KeymapError::Version { layer, .. }
            | KeymapError::NoModes { layer }
            | KeymapError::Token { layer, .. }
            | KeymapError::UnknownMode { layer, .. }
            | KeymapError::Intent { layer, .. } => Some(layer.as_str()),
            KeymapError::Prefix { .. } => None,
        };
        let pointer = match layer.and_then(|id| input.keymaps.iter().position(|l| l.id == id)) {
            Some(index) => format!("/input/keymaps/{index}"),
            None => "/input/keymaps".to_owned(),
        };
        errors.push(pointer, error.to_string());
    }
}

fn validate_panels(panels: &[PanelSpec], errors: &mut ErrorSink) {
    let mut ids = HashSet::new();
    let mut slugs = HashSet::new();
    let mut maximized = None;
    for (index, panel) in panels.iter().enumerate() {
        let base = format!("/panels/{index}");
        if panel.id.is_empty() {
            errors.push(format!("{base}/id"), "panel id must not be empty");
        }
        if !ids.insert(panel.id.as_str()) {
            errors.push(format!("{base}/id"), "duplicate panel id");
        }
        if panel.slug.is_empty() {
            errors.push(format!("{base}/slug"), "panel slug must not be empty");
        }
        if !slugs.insert(panel.slug.as_str()) {
            errors.push(format!("{base}/slug"), "duplicate panel slug");
        }
        validate_window(&panel.window, &format!("{base}/window"), errors);
        validate_panel_content(&panel.content, &format!("{base}/content"), errors);
        if panel.window.state == WinState::Maximized && maximized.replace(index).is_some() {
            errors.push(
                format!("{base}/window/state"),
                "only one panel may start maximized",
            );
        }
    }
}

fn validate_window(window: &WindowSpec, pointer: &str, errors: &mut ErrorSink) {
    for (field, value) in [
        ("x", window.x),
        ("y", window.y),
        ("w", window.w),
        ("h", window.h),
    ] {
        if !value.is_finite() {
            errors.push(
                format!("{pointer}/{field}"),
                "window dimension must be finite",
            );
        }
    }
    if window.w <= 0.0 {
        errors.push(format!("{pointer}/w"), "window width must be positive");
    }
    if window.h <= 0.0 {
        errors.push(format!("{pointer}/h"), "window height must be positive");
    }
    if !(1..=TILE_W_MAX).contains(&window.tile_w) {
        errors.push(format!("{pointer}/tile_w"), "tile_w must be in 1..=4");
    }
    if !(1..=TILE_H_MAX).contains(&window.tile_h) {
        errors.push(format!("{pointer}/tile_h"), "tile_h must be in 1..=6");
    }
}

fn validate_panel_content(content: &ContentSpec, pointer: &str, errors: &mut ErrorSink) {
    let ContentSpec::External { layout, .. } = content else {
        return;
    };
    for (name, value) in &layout.params {
        if !value.is_finite() {
            errors.push(
                format!("{pointer}/layout/params/{name}"),
                "layout parameter must be finite",
            );
        }
    }
}

fn validate_components(
    components: &[ExternalComponentSpec],
    panels: &[PanelSpec],
    errors: &mut ErrorSink,
) {
    let mut declared = HashSet::new();
    for (index, component) in components.iter().enumerate() {
        let base = format!("/components/{index}");
        if component.id.is_empty() {
            errors.push(format!("{base}/id"), "component id must not be empty");
        }
        if !declared.insert(component.id.as_str()) {
            errors.push(format!("{base}/id"), "duplicate component id");
        }
        for (axis, extent) in component.min_size.iter().enumerate() {
            if !extent.is_finite() || *extent <= 0.0 {
                errors.push(
                    format!("{base}/min_size/{axis}"),
                    "min_size extent must be finite and positive",
                );
            }
        }
    }

    let mut referenced = HashSet::new();
    for (index, panel) in panels.iter().enumerate() {
        let Some(id) = panel.external_component() else {
            continue;
        };
        if declared.contains(id) {
            referenced.insert(id);
        } else {
            errors.push(
                format!("/panels/{index}/content/component"),
                format!("undeclared component {id:?}"),
            );
        }
    }

    for (index, component) in components.iter().enumerate() {
        if !referenced.contains(component.id.as_str()) {
            errors.push(
                format!("/components/{index}/id"),
                format!("component {:?} is not referenced by any panel", component.id),
            );
        }
    }
}

fn validate_manifest(manifest: &BindingManifest) -> Result<(), SpecErrors> {
    let mut errors = ErrorSink::new();
    let mut seen = HashSet::new();
    for (index, panel) in manifest.panels.iter().enumerate() {
        if panel.panel_id.is_empty() {
            errors.push(
                format!("/panels/{index}/panel_id"),
                "provider panel_id must not be empty",
            );
        }
        if !seen.insert(panel.panel_id.as_str()) {
            errors.push(
                format!("/panels/{index}/panel_id"),
                "duplicate provider panel_id",
            );
        }
    }
    errors.finish()
}

fn validate_providers(spec: &WorkspaceSpec, providers: &BindingManifest) -> Result<(), SpecErrors> {
    let mut errors = ErrorSink::new();
    let mut provider_by_panel = HashMap::new();
    for provider in &providers.panels {
        provider_by_panel.insert(provider.panel_id.as_str(), provider);
    }

    for (index, panel) in spec.panels.iter().enumerate() {
        match provider_by_panel.remove(panel.id.as_str()) {
            Some(provider) if provider.content_kind == ContentKind::from_spec(&panel.content) => {}
            Some(_) => errors.push(
                format!("/panels/{index}/content/kind"),
                "provider content kind disagrees with spec",
            ),
            None => errors.push(format!("/panels/{index}/id"), "provider missing for panel"),
        }
    }

    for provider in provider_by_panel.values() {
        errors.push(
            "/panels",
            format!("provider {:?} is not referenced", provider.panel_id),
        );
    }
    errors.finish()
}

#[cfg(all(test, feature = "spec-json"))]
mod tests {
    #[test]
    fn input_keymaps_validate_by_pointer() {
        use crate::keymap::{KeymapSpec, RuleSpec, KEYMAP_SPEC_VERSION};
        let rule = |keys: &str, intent: &str| RuleSpec {
            keys: keys.into(),
            intent: intent.into(),
            modes: None,
            enter: None,
            label: None,
            group: None,
        };
        let layer = |id: &str, rules: Vec<RuleSpec>| KeymapSpec {
            spec_version: KEYMAP_SPEC_VERSION,
            id: id.into(),
            modes: vec!["normal".into()],
            rules,
        };
        let diagnostics = |keymaps: Vec<KeymapSpec>| {
            let mut errors = ErrorSink::new();
            validate_input(
                &InputSpec {
                    steps: CommandStep::WEB,
                    keymaps,
                },
                &mut errors,
            );
            errors
                .finish()
                .err()
                .map(|e| {
                    e.iter()
                        .map(|d| (d.pointer().to_string(), d.message().to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };

        assert!(diagnostics(vec![layer("a", vec![rule("g g", "first")])]).is_empty());

        let bad_token = diagnostics(vec![layer("a", vec![]), layer("b", vec![rule("<nope>", "x")])]);
        assert_eq!(bad_token[0].0, "/input/keymaps/1");
        assert!(bad_token[0].1.contains("bad token `<nope>`"), "{}", bad_token[0].1);

        let prefix = diagnostics(vec![layer("a", vec![rule("g", "x"), rule("g g", "y")])]);
        assert_eq!(prefix[0].0, "/input/keymaps");
        assert!(prefix[0].1.contains("`g` is a prefix of `g g`"), "{}", prefix[0].1);

        let dup = diagnostics(vec![layer("a", vec![]), layer("a", vec![])]);
        assert_eq!(dup[0], ("/input/keymaps/1/id".to_string(), "duplicate keymap id".to_string()));
    }

    use super::*;
    use crate::theme::ThemeTokens;
    use crate::widgets::{DataSource, TextModel};
    use crate::{ChromeMetrics, Clamp, CommandStep, Key, TileMetrics};
    use serde_json::json;

    pub(super) fn valid_spec_json() -> serde_json::Value {
        json!({
            "spec_version": WORKSPACE_SPEC_VERSION,
            "id": "workspace-canary",
            "layout": {
                "units": "Cells",
                "viewport": [120.0, 40.0],
                "preferred_mode": "Floating",
                "clamp": Clamp::CELLS,
                "tile": { "resize": TileMetrics::CELLS, "row_min": 1.0, "gap": 1.0, "padding": 0.0, "fill_viewport": true }
            },
            "surface": {
                "compact_max": 60.0,
                "tablet_max": 110.0,
                "fallback_capabilities": { "coarse_pointer": false, "hover": true, "keyboard": true },
                "resize_policy": "preserve_intent"
            },
            "chrome": {
                "metrics": ChromeMetrics::CELLS,
                "hit_target_min": 2.0,
                "panel_header_h": 1.0,
                "panel_frame": true,
                "title_in_border": true,
                "mode_control": true,
                "minimize_control": true,
                "maximize_control": true,
                "resize_grip": true,
                "dock": true,
                "dock_label": "Dock"
            },
            "input": {
                "steps": CommandStep::CELLS,
                "keymaps": [{
                    "spec_version": 1,
                    "id": "canary-vi",
                    "modes": ["normal"],
                    "rules": [{ "keys": "g g", "intent": "nav.first", "modes": null, "enter": null, "label": null, "group": null }]
                }]
            },
            "glyphs": "unicode",
            "theme": ThemeTokens::dark(),
            "persistence": { "enabled": true, "key": "workspace-canary", "restore": true, "save_policy": "on_settle" },
            "panels": [{
                "id": "notes",
                "title": "Notes",
                "slug": "notes",
                "window": { "x": 2.0, "y": 2.0, "w": 36.0, "h": 12.0, "state": "Floating", "z": 1, "tile_w": 2, "tile_h": 2 },
                "content": { "kind": "text", "source": { "source": "inline", "value": { "text": "hello" } }, "scroll": "wrap" }
            }]
        })
    }

    fn valid_badge_json() -> serde_json::Value {
        json!({
            "kind": "Tag",
            "field": "tag",
            "value": "nix",
            "active": false,
            "with_x": false,
            "with_plus": true,
            "small": false,
            "override_color": [80, 180, 130],
            "accent_color": null,
            "click_kind": "Toggle",
            "emit_hover": false
        })
    }

    fn valid_content_json(kind: &str) -> serde_json::Value {
        match kind {
            "custom" => json!({ "kind": "custom", "binding": "canary.custom" }),
            "text" => {
                json!({ "kind": "text", "source": { "source": "inline", "value": { "text": "hello" } }, "scroll": "wrap" })
            }
            "editor" => {
                json!({ "kind": "editor", "binding": "canary.editor", "multiline": true, "placeholder": "Write" })
            }
            "badges" => {
                json!({ "kind": "badges", "source": { "source": "inline", "value": [valid_badge_json()] } })
            }
            "table" => json!({
                "kind": "table",
                "source": {
                    "source": "inline",
                    "value": {
                        "columns": [{ "key": "name", "title": "Name", "width": { "Fixed": { "value": 12 } }, "align": "left", "kind": "number" }],
                        "rows": [{ "id": "alpha", "cells": [{ "kind": "text", "value": "alpha" }] }]
                    }
                }
            }),
            "time_series" => {
                json!({ "kind": "time_series", "source": { "source": "inline", "value": [{ "name": "load", "points": [[0.0, 1.0]] }] }, "unit": "req/s" })
            }
            "gauges" => {
                json!({ "kind": "gauges", "source": { "source": "inline", "value": [{ "label": "cpu", "ratio": 0.42, "text": "42%" }] } })
            }
            "flamegraph" => {
                json!({ "kind": "flamegraph", "source": { "source": "inline", "value": [{ "label": "root", "depth": 0, "value": 1.0, "color": null }] } })
            }
            "boxplot" => {
                json!({ "kind": "boxplot", "source": { "source": "inline", "value": [{ "label": "latency", "samples": [1.0, 2.0, 3.0], "color": null }] } })
            }
            "meter" => {
                json!({ "kind": "meter", "source": { "source": "inline", "value": { "label": "memory", "ratio": 0.5, "text": "50%", "color": null } } })
            }
            "status" => {
                json!({ "kind": "status", "source": { "source": "inline", "value": { "label": "build", "state": "ok", "color": [39, 201, 63] } } })
            }
            "spinner" => {
                json!({ "kind": "spinner", "label": { "source": "inline", "value": "Loading" } })
            }
            "external" => json!({
                "kind": "external",
                "component": "graph",
                "source": { "source": "binding", "id": "okf.graph" },
                "layout": { "algorithm": "dagre", "group_by": "sort", "params": { "rank_sep": 2.0 } }
            }),
            _ => panic!("unknown content kind fixture: {kind}"),
        }
    }

    fn spec_with_content(content: serde_json::Value) -> serde_json::Value {
        let mut spec = valid_spec_json();
        spec["panels"][0]["content"] = content;
        spec
    }

    fn valid_component_json() -> serde_json::Value {
        json!({
            "id": "graph",
            "contract": "graph",
            "requires": ["webgpu"],
            "keep_mounted": true,
            "min_size": [320.0, 240.0],
            "events": ["select", "hover"],
            "fallback": "cells"
        })
    }

    fn spec_with_external_panel() -> serde_json::Value {
        let mut spec = spec_with_content(valid_content_json("external"));
        spec["components"] = json!([valid_component_json()]);
        spec
    }

    fn decode_error_pointers(doc: serde_json::Value) -> Vec<String> {
        WorkspaceSpec::from_json_value(doc)
            .expect_err("malformed spec must be rejected")
            .iter()
            .map(|error| error.pointer().to_owned())
            .collect()
    }

    #[test]
    fn content_variants_reject_unknown_variant_fields_by_pointer() {
        for kind in [
            "custom",
            "text",
            "editor",
            "badges",
            "table",
            "time_series",
            "gauges",
            "flamegraph",
            "boxplot",
            "meter",
            "status",
            "spinner",
            "external",
        ] {
            let mut content = valid_content_json(kind);
            content["unexpected"] = json!(true);

            let pointers = decode_error_pointers(spec_with_content(content));

            assert_eq!(pointers[0], "/panels/0/content/unexpected", "{kind}");
        }
    }

    #[test]
    fn data_source_variants_reject_ambiguous_and_missing_fields_by_pointer() {
        let cases = [
            (
                json!({ "kind": "text", "source": { "source": "inline", "value": { "text": "hello" }, "id": "ambiguous" }, "scroll": "wrap" }),
                "/panels/0/content/source/id",
            ),
            (
                json!({ "kind": "text", "source": { "source": "inline" }, "scroll": "wrap" }),
                "/panels/0/content/source/value",
            ),
            (
                json!({ "kind": "text", "source": { "source": "binding", "id": "canary.text", "value": { "text": "hello" } }, "scroll": "wrap" }),
                "/panels/0/content/source/value",
            ),
            (
                json!({ "kind": "text", "source": { "source": "binding" }, "scroll": "wrap" }),
                "/panels/0/content/source/id",
            ),
        ];

        for (content, expected_pointer) in cases {
            let pointers = decode_error_pointers(spec_with_content(content));

            assert_eq!(pointers[0], expected_pointer);
        }
    }

    #[test]
    fn nested_content_diagnostics_accumulate_in_panel_order() {
        let mut doc = valid_spec_json();
        doc["panels"][0]["content"] = json!({
            "kind": "text",
            "source": { "source": "inline", "value": { "text": "hello" }, "id": "ambiguous" },
            "scroll": "wrap"
        });
        let mut second = doc["panels"][0].clone();
        second["id"] = json!("badges");
        second["slug"] = json!("badges");
        second["content"] = json!({
            "kind": "badges",
            "source": { "source": "inline", "value": [{
                "kind": "Tag",
                "field": "tag",
                "value": "nix",
                "active": false,
                "with_x": false,
                "with_plus": true,
                "small": false,
                "override_color": null,
                "click_kind": "Toggle",
                "emit_hover": false
            }] }
        });
        let mut third = doc["panels"][0].clone();
        third["id"] = json!("spinner");
        third["slug"] = json!("spinner");
        third["content"] = json!({
            "kind": "spinner",
            "label": { "source": "binding", "id": "canary.spinner", "value": "ambiguous" }
        });
        doc["panels"]
            .as_array_mut()
            .unwrap()
            .extend([second, third]);

        let pointers = decode_error_pointers(doc);

        assert_eq!(
            pointers,
            [
                "/panels/0/content/source/id",
                "/panels/1/content/source/value/0/accent_color",
                "/panels/2/content/label/value",
            ]
        );
    }

    #[test]
    fn workspace_spec_reports_strict_errors_by_pointer() {
        let mut doc = valid_spec_json();
        doc["chrome"]["new_field"] = json!(true);
        doc["panels"][0].as_object_mut().unwrap().remove("title");
        doc["layout"]["viewport"] = json!([0.0, 40.0]);
        doc["surface"]["tablet_max"] = json!(50.0);
        doc["panels"][0]["window"]["tile_w"] = json!(0);
        let mut duplicate = doc["panels"][0].clone();
        duplicate["title"] = json!("Notes");
        doc["panels"].as_array_mut().unwrap().push(duplicate);
        doc["input"]["keymaps"][0]["rules"][0]["count"] = json!(true);

        let errors = WorkspaceSpec::from_json_value(doc).expect_err("invalid spec reports errors");
        let diagnostics: Vec<_> = errors
            .iter()
            .map(|error| (error.pointer(), error.message()))
            .collect();

        assert_eq!(diagnostics[0].0, "/layout/viewport/0");
        assert_eq!(diagnostics[1].0, "/surface/tablet_max");
        assert_eq!(diagnostics[2].0, "/chrome/new_field");
        assert!(diagnostics[2].1.contains("unknown field; allowed fields:"));
        assert_eq!(diagnostics[3].0, "/input/keymaps/0/rules/0/count");
        assert_eq!(diagnostics[4].0, "/panels/0/title");
        assert_eq!(diagnostics[5].0, "/panels/0/window/tile_w");
        assert_eq!(diagnostics[6].0, "/panels/1/id");
    }

    #[test]
    fn required_nullable_manifest_binding_distinguishes_null_from_missing() {
        let present_null = json!({
            "backend": "web",
            "panels": [{ "panel_id": "notes", "content_kind": "text", "binding": null }]
        });
        assert!(BindingManifest::from_json_value(present_null).is_ok());

        let missing = json!({
            "backend": "web",
            "panels": [{ "panel_id": "notes", "content_kind": "text" }]
        });
        let errors = BindingManifest::from_json_value(missing)
            .expect_err("missing nullable field is rejected");
        assert_eq!(errors.iter().next().unwrap().pointer(), "/panels/0/binding");
    }

    #[test]
    fn color_codec_accepts_only_canonical_lowercase_rrggbb() {
        let mut spec = valid_spec_json();
        spec["theme"]["colors"]["highlight"] = json!("#5ef38c");
        assert!(WorkspaceSpec::from_json_value(spec.clone()).is_ok());

        spec["theme"]["colors"]["highlight"] = json!("#5EF38C");
        assert_eq!(
            WorkspaceSpec::from_json_value(spec.clone())
                .unwrap_err()
                .iter()
                .next()
                .unwrap()
                .pointer(),
            "/theme/colors/highlight"
        );

        spec["theme"]["colors"]["highlight"] = json!("fff");
        assert_eq!(
            WorkspaceSpec::from_json_value(spec)
                .unwrap_err()
                .iter()
                .next()
                .unwrap()
                .pointer(),
            "/theme/colors/highlight"
        );
    }

    #[test]
    fn workspace_spec_round_trips_losslessly() {
        let spec = WorkspaceSpec::from_json_value(valid_spec_json()).expect("valid spec decodes");
        let encoded = serde_json::to_value(&spec).expect("spec encodes");
        let decoded = WorkspaceSpec::from_json_value(encoded).expect("encoded spec decodes");

        assert_eq!(decoded, spec);
    }

    #[test]
    fn resolved_workspace_preserves_panel_order_and_rejects_provider_drift() {
        let spec = WorkspaceSpec::from_json_value(valid_spec_json()).expect("valid spec decodes");
        let providers = BindingManifest {
            backend: BackendKind::Web,
            panels: vec![PanelProviderDeclaration {
                panel_id: "notes".into(),
                content_kind: ContentKind::Text,
                binding: None,
            }],
        };
        let resolved = spec.resolve(&providers).expect("providers match spec");

        assert_eq!(resolved.catalog.len(), 1);
        assert_eq!(resolved.initial.panels[0].kind.index(), 0);

        let mut drifted = providers.clone();
        drifted.panels[0].content_kind = ContentKind::Custom;
        let spec = WorkspaceSpec::from_json_value(valid_spec_json()).expect("valid spec decodes");
        let errors = match spec.resolve(&drifted) {
            Ok(_) => panic!("provider kind drift is rejected"),
            Err(errors) => errors,
        };
        assert_eq!(
            errors.iter().next().unwrap().pointer(),
            "/panels/0/content/kind"
        );
    }

    /// Resolve a spec document against providers declared from its own panels.
    pub(super) fn resolved_from(doc: serde_json::Value) -> ResolvedWorkspace {
        let spec = WorkspaceSpec::from_json_value(doc).expect("valid spec decodes");
        let providers = BindingManifest {
            backend: BackendKind::Web,
            panels: spec.panels.iter().map(PanelSpec::provider_declaration).collect(),
        };
        spec.resolve(&providers).expect("providers match spec")
    }

    #[test]
    fn external_panel_with_declared_component_round_trips_losslessly() {
        let spec = WorkspaceSpec::from_json_value(spec_with_external_panel())
            .expect("external panel with a declared component decodes");
        let encoded = serde_json::to_value(&spec).expect("spec encodes");
        let decoded = WorkspaceSpec::from_json_value(encoded).expect("encoded spec decodes");

        assert_eq!(decoded, spec);
        assert_eq!(spec.components.len(), 1);
        assert_eq!(spec.component("graph"), spec.components.first());
        assert_eq!(spec.panels[0].external_component(), Some("graph"));
        assert_eq!(spec.panels[0].content.binding_id(), Some("okf.graph"));
    }

    #[test]
    fn resolved_workspace_exposes_the_declaration_an_external_panel_names() {
        let spec = WorkspaceSpec::from_json_value(spec_with_external_panel())
            .expect("external panel decodes");
        let providers = BindingManifest {
            backend: BackendKind::Web,
            panels: vec![spec.panels[0].provider_declaration()],
        };

        let resolved = spec.resolve(&providers).expect("providers match spec");

        assert_eq!(resolved.components.len(), 1);
        assert_eq!(
            resolved.panel_component(&resolved.panels[0]).map(|c| c.id.as_str()),
            Some("graph")
        );
        assert!(component_satisfied(
            &resolved.components[0],
            &[Capability::WebGpu]
        ));
    }

    #[test]
    fn external_component_diagnostics_report_their_pointer() {
        let mut undeclared = spec_with_content(valid_content_json("external"));
        undeclared["components"] = json!([valid_component_json()]);
        undeclared["panels"][0]["content"]["component"] = json!("missing");
        assert_eq!(
            decode_error_pointers(undeclared),
            ["/panels/0/content/component", "/components/0/id"],
            "an unknown reference leaves its declaration unreferenced"
        );

        let mut duplicate = spec_with_external_panel();
        duplicate["components"]
            .as_array_mut()
            .expect("components is an array")
            .push(valid_component_json());
        assert!(decode_error_pointers(duplicate).contains(&"/components/1/id".into()));

        let mut unreferenced = valid_spec_json();
        unreferenced["components"] = json!([valid_component_json()]);
        assert_eq!(decode_error_pointers(unreferenced), ["/components/0/id"]);

        let mut negative = spec_with_external_panel();
        negative["components"][0]["min_size"] = json!([0.0, 240.0]);
        assert_eq!(
            decode_error_pointers(negative),
            ["/components/0/min_size/0"]
        );
    }

    #[test]
    fn closed_external_enum_sets_report_unsupported_names_by_pointer() {
        for (field, value, pointer) in [
            ("contract", json!("canvas"), "/components/0/contract"),
            ("fallback", json!("iframe"), "/components/0/fallback"),
        ] {
            let mut doc = spec_with_external_panel();
            doc["components"][0][field] = value;
            assert_eq!(decode_error_pointers(doc), [pointer], "{field}");
        }

        let mut requires = spec_with_external_panel();
        requires["components"][0]["requires"] = json!(["webgl"]);
        assert_eq!(decode_error_pointers(requires), ["/components/0/requires/0"]);

        let mut events = spec_with_external_panel();
        events["components"][0]["events"] = json!(["drag"]);
        assert_eq!(decode_error_pointers(events), ["/components/0/events/0"]);

        let mut algorithm = spec_with_external_panel();
        algorithm["panels"][0]["content"]["layout"]["algorithm"] = json!("spring");
        assert_eq!(
            decode_error_pointers(algorithm),
            ["/panels/0/content/layout/algorithm"]
        );
    }

    #[test]
    fn non_finite_layout_parameter_is_rejected_by_pointer() {
        let mut doc = spec_with_external_panel();
        doc["panels"][0]["content"]["layout"]["params"] = json!({ "rank_sep": "NaN" });

        assert_eq!(
            decode_error_pointers(doc),
            ["/panels/0/content/layout/params/rank_sep"]
        );
    }

    #[test]
    fn inline_external_graph_body_decodes_with_its_model() {
        let mut doc = spec_with_external_panel();
        doc["panels"][0]["content"]["source"] = json!({
            "source": "inline",
            "value": {
                "nodes": [{
                    "id": "acme/gross-margin",
                    "label": "Gross margin",
                    "sort": "metric",
                    "color": [94, 243, 140],
                    "size": 3.0,
                    "mass": 1.0,
                    "repulsion": 40.0
                }],
                "edges": [{
                    "source": 0, "target": 0, "kind": "references",
                    "weight": 1.0, "rest_length": 120.0
                }]
            }
        });

        let spec = WorkspaceSpec::from_json_value(doc.clone()).expect("inline graph decodes");

        assert_eq!(spec.panels[0].content.binding_id(), None);
        assert_eq!(doc["components"][0]["contract"], json!("graph"));
    }

    #[test]
    fn plain_rust_enum_path_has_no_spec_schema_dependency() {
        let content = ContentSpec::Text {
            source: DataSource::Inline {
                value: TextModel {
                    text: "plain".into(),
                },
            },
            scroll: crate::widgets::ScrollPolicy::Clip,
        };

        assert_eq!(Key::Left, Key::Left);
        assert_eq!(content.kind(), "text");
    }
}
