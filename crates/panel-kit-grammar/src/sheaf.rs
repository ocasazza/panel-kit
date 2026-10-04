//! Sheaves: declarative queries that turn a [`Site`] plus a regime's sortings,
//! stalks, and physics into panel-kit-core content models, one sheaf per
//! bindable content kind. Value expressions are strings:
//!
//! - object text/number: `id`, `title`, `kind` (regime sort label), `tag:<i>`,
//!   `field:<name>` (missing is an error), `field?:<name>` (missing reads
//!   empty), `degree`, `in_degree`, `out_degree`, `repulsion`, `mass`,
//!   `weighted:<field>` (= `field` * `repulsion`); physics values resolve
//!   through the base.
//! - morphism: `source`, `target`, `edge_kind` (regime sort label), `weight`,
//!   `rest_length`.
//!
//! "Group by kind" sheaves group by **regime sort**, so a non-injective sorting
//! merges site kinds (e.g. planner + reviewer → head).

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use panel_kit_core::badge::{BadgeKind, BadgeSpec, Rgb};
use panel_kit_core::widgets::charts::{BoxItemModel, FlameSpanModel, GaugeModel, SeriesModel};
use panel_kit_core::widgets::meter::MeterModel;
use panel_kit_core::widgets::status::{StatusModel, StatusState};
use panel_kit_core::widgets::table::{ColumnWidth, TableCell, TableColumn, TableModel, TableRow, TextAlign};
use panel_kit_core::widgets::TextModel;

use crate::error::SheafError;
use crate::edit::RowEdits;
use crate::site::{Morphism, Object, Site};
use crate::topos::{Resolver, Stages};

/// Resolved content keyed by binding id, plus the object and offered edits
/// behind each row of object-scoped tables that declare edits.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GlobalSections {
    map: BTreeMap<String, Section>,
    rows: BTreeMap<String, Vec<RowEdits>>,
}

impl GlobalSections {
    /// Build from resolved content and per-table row edits.
    pub(crate) fn from_parts(map: BTreeMap<String, Section>, rows: BTreeMap<String, Vec<RowEdits>>) -> Self {
        Self { map, rows }
    }

    /// The object and offered edits behind each row of a table binding, in row
    /// order; `None` when the binding's sheaf declares no edits.
    pub fn row_edits(&self, id: &str) -> Option<&[RowEdits]> {
        self.rows.get(id).map(Vec::as_slice)
    }

    /// Content for a binding id, or `None` when unbound.
    pub fn get(&self, id: &str) -> Option<&Section> {
        self.map.get(id)
    }

    /// Number of bound ids.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no ids are bound.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate `(id, content)` pairs in id order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Section)> {
        self.map.iter()
    }
}

/// One resolved content model, mirroring a bindable `ContentSpec` kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Section {
    /// Plain text.
    Text(TextModel),
    /// Badge strip.
    Badges(Vec<BadgeSpec>),
    /// Semantic table.
    Table(TableModel),
    /// Named point series.
    TimeSeries(Vec<SeriesModel>),
    /// Horizontal gauges.
    Gauges(Vec<GaugeModel>),
    /// Flamegraph spans in preorder.
    Flamegraph(Vec<FlameSpanModel>),
    /// Box-and-whisker distributions.
    Boxplot(Vec<BoxItemModel>),
    /// Single meter.
    Meter(MeterModel),
    /// Single status.
    Status(StatusModel),
}

impl Section {
    /// The `ContentSpec` kind name this content resolves.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Badges(_) => "badges",
            Self::Table(_) => "table",
            Self::TimeSeries(_) => "time_series",
            Self::Gauges(_) => "gauges",
            Self::Flamegraph(_) => "flamegraph",
            Self::Boxplot(_) => "boxplot",
            Self::Meter(_) => "meter",
            Self::Status(_) => "status",
        }
    }
}

/// One sheaf, tagged by the content kind it produces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Sheaf {
    /// Text model (heading, preface, optional stalks dump).
    Text(TextSheaf),
    /// Badge strip over object sorts or objects.
    Badges(BadgesSheaf),
    /// Table over objects or morphisms.
    Table(TableSheaf),
    /// Series over object numeric channels.
    TimeSeries(TimeSeriesSheaf),
    /// Gauges over objects or summed per sort.
    Gauges(GaugesSheaf),
    /// Flamegraph of a typed morphism call tree.
    Flamegraph(FlamegraphSheaf),
    /// Boxplot of an object numeric channel per sort.
    Boxplot(BoxplotSheaf),
    /// Aggregated single meter.
    Meter(MeterSheaf),
    /// Worst-of single status.
    Status(StatusSheaf),
    /// Assembly invariants as a table (one row per component).
    Assembly(AssemblySheaf),
    /// Assembly stage counts as a badge strip.
    AssemblyBadges(AssemblySheaf),
    /// Physics section as a table (objects or morphisms).
    Physics(PhysicsSheaf),
}

impl Sheaf {
    /// The content kind this sheaf produces.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Badges(_) => "badges",
            Self::Table(_) => "table",
            Self::TimeSeries(_) => "time_series",
            Self::Gauges(_) => "gauges",
            Self::Flamegraph(_) => "flamegraph",
            Self::Boxplot(_) => "boxplot",
            Self::Meter(_) => "meter",
            Self::Status(_) => "status",
            Self::Assembly(_) => "table",
            Self::AssemblyBadges(_) => "badges",
            Self::Physics(_) => "table",
        }
    }

    /// Γ over this sheaf's subobject: the site restricted to `restrict` (when
    /// set) and then evaluated.
    pub(crate) fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let restricted = self.restricted_site(resolver.object_sorting, resolver.morphism_sorting, site);
        let site = restricted.as_ref().unwrap_or(site);
        match self {
            Self::Text(sheaf) => sheaf.evaluate(resolver),
            Self::Badges(sheaf) => sheaf.evaluate(resolver, site),
            Self::Table(sheaf) => sheaf.evaluate(resolver, site),
            Self::TimeSeries(sheaf) => sheaf.evaluate(resolver, site),
            Self::Gauges(sheaf) => sheaf.evaluate(resolver, site),
            Self::Flamegraph(sheaf) => sheaf.evaluate(resolver, site),
            Self::Boxplot(sheaf) => sheaf.evaluate(resolver, site),
            Self::Meter(sheaf) => sheaf.evaluate(resolver, site),
            Self::Status(sheaf) => sheaf.evaluate(resolver, site),
            Self::Assembly(sheaf) => sheaf.evaluate_table(resolver, site),
            Self::AssemblyBadges(sheaf) => sheaf.evaluate_badges(resolver, site),
            Self::Physics(sheaf) => sheaf.evaluate(resolver, site),
        }
    }

    /// The site restricted to this sheaf's subobject, or `None` when the sheaf
    /// is unrestricted.
    pub(crate) fn restricted_site(
        &self,
        object_sorting: &BTreeMap<String, String>,
        morphism_sorting: &BTreeMap<String, String>,
        site: &Site,
    ) -> Option<Site> {
        self.restriction().map(|(sorts, space)| match space {
            SortSpace::Objects => site.restrict_objects(object_sorting, sorts),
            SortSpace::Morphisms => site.restrict_morphisms(morphism_sorting, sorts),
        })
    }

    /// The subobject `U` this sheaf's sections are taken over, and whether `U`
    /// names object or morphism sorts.
    pub(crate) fn restriction(&self) -> Option<(&BTreeSet<String>, SortSpace)> {
        match self {
            Self::Table(sheaf) => sheaf.restrict.as_ref().map(|sorts| {
                let space = match sheaf.scope {
                    RowScope::Objects => SortSpace::Objects,
                    RowScope::Morphisms => SortSpace::Morphisms,
                };
                (sorts, space)
            }),
            Self::Badges(BadgesSheaf { restrict, .. })
            | Self::Flamegraph(FlamegraphSheaf { restrict, .. })
            | Self::Status(StatusSheaf { restrict, .. }) => restrict.as_ref().map(|sorts| (sorts, SortSpace::Objects)),
            _ => None,
        }
    }
}

/// Whether a restriction names object sorts or morphism sorts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SortSpace {
    /// Regime object sorts.
    Objects,
    /// Regime morphism sorts.
    Morphisms,
}

/// Text sheaf: heading, authored preface lines, and an optional per-sort stalks
/// dump stating how the regime reads each sort.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSheaf {
    /// First line.
    pub heading: String,
    /// Authored lines after the heading.
    #[serde(default)]
    pub preface: Vec<String>,
    /// Append one line per object sort and morphism sort from the stalks.
    #[serde(default)]
    pub include_stalks: bool,
}

impl TextSheaf {
    fn evaluate(&self, resolver: &Resolver) -> Result<Section, SheafError> {
        let mut lines = vec![self.heading.clone()];
        lines.extend(self.preface.iter().cloned());
        if self.include_stalks {
            for (sort, stalk) in &resolver.stalks.objects {
                lines.push(format!("{sort} = {} [{}]", stalk.label, stalk.badge));
            }
            for (sort, stalk) in &resolver.stalks.morphisms {
                lines.push(format!("{sort} = {}", stalk.label));
            }
        }
        Ok(Section::Text(TextModel {
            text: lines.join("\n"),
        }))
    }
}

/// Badge strip over object sorts or objects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BadgesSheaf {
    /// Whether one badge is emitted per sort or per object.
    pub group: BadgeGroup,
    /// Regime object sorts to restrict to; absent means every object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restrict: Option<BTreeSet<String>>,
}

/// Badge grouping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeGroup {
    /// One badge per distinct object sort.
    Kinds,
    /// One badge per object.
    Objects,
}

impl BadgesSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let mut badges = Vec::new();
        match self.group {
            BadgeGroup::Kinds => {
                for sort in distinct_sorts(resolver, site) {
                    let badge = parse_badge(resolver.sort_object_badge(&sort))?;
                    let count = site
                        .objects
                        .iter()
                        .filter(|object| resolver.object_sort(&object.kind) == Some(sort.as_str()))
                        .count();
                    let value = format!("{} {count}", resolver.sort_object_label(&sort));
                    badges.push(BadgeSpec::new("kind", value, badge));
                }
            }
            BadgeGroup::Objects => {
                for object in &site.objects {
                    let badge = parse_badge(resolver.object_badge(&object.kind))?;
                    badges.push(BadgeSpec::new("object", object.title.clone(), badge));
                }
            }
        }
        Ok(Section::Badges(badges))
    }
}

/// Table over objects or morphisms.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableSheaf {
    /// Row source.
    pub scope: RowScope,
    /// Column definitions in order.
    pub columns: Vec<ColumnSpec>,
    /// Regime sorts to restrict to (object sorts, or morphism sorts for a
    /// morphism-scoped table); absent means every row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restrict: Option<BTreeSet<String>>,
    /// Section edits offered on each row's object (object scope only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<String>,
}

/// Table row source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowScope {
    /// One row per object.
    Objects,
    /// One row per morphism.
    Morphisms,
}

/// One table column.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnSpec {
    /// Stable column key.
    pub key: String,
    /// Display title.
    pub title: String,
    /// Width request.
    pub width: ColWidth,
    /// Text alignment.
    pub align: CellAlign,
    /// How each cell is produced.
    pub cell: CellSpec,
}

/// Renderer-neutral column width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColWidth {
    /// Flexible weighted width.
    Flex {
        /// Width weight.
        weight: u16,
    },
    /// Fixed width.
    Fixed {
        /// Width value.
        value: u16,
    },
}

impl ColWidth {
    fn to_core(self) -> ColumnWidth {
        match self {
            Self::Flex { weight } => ColumnWidth::Flex { weight },
            Self::Fixed { value } => ColumnWidth::Fixed { value },
        }
    }
}

/// Column text alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellAlign {
    /// Left-align.
    Left,
    /// Center-align.
    Center,
    /// Right-align.
    Right,
}

impl CellAlign {
    fn to_core(self) -> TextAlign {
        match self {
            Self::Left => TextAlign::Left,
            Self::Center => TextAlign::Center,
            Self::Right => TextAlign::Right,
        }
    }
}

/// How one cell is produced from a row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cell", rename_all = "snake_case")]
pub enum CellSpec {
    /// Text cell from a value expression.
    Text {
        /// Value expression.
        expr: String,
    },
    /// Status cell from a status-word expression (objects only).
    Status {
        /// Expression yielding a status word.
        from: String,
    },
    /// Meter cell: `value / scale` fill, with display `text`.
    Meter {
        /// Numeric value expression.
        value: String,
        /// Divisor applied to `value` for the fill ratio.
        #[serde(default = "one")]
        scale: f64,
        /// Display text expression.
        text: String,
    },
}

impl TableSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let columns = self
            .columns
            .iter()
            .map(|column| TableColumn {
                key: column.key.clone(),
                title: column.title.clone(),
                width: column.width.to_core(),
                align: column.align.to_core(),
            })
            .collect();

        let mut rows = Vec::new();
        match self.scope {
            RowScope::Objects => {
                for object in &site.objects {
                    let cells = self
                        .columns
                        .iter()
                        .map(|column| object_cell(&column.cell, object, site, resolver))
                        .collect::<Result<Vec<_>, _>>()?;
                    rows.push(TableRow { cells });
                }
            }
            RowScope::Morphisms => {
                for morphism in &site.morphisms {
                    let cells = self
                        .columns
                        .iter()
                        .map(|column| morphism_cell(&column.cell, morphism, resolver))
                        .collect::<Result<Vec<_>, _>>()?;
                    rows.push(TableRow { cells });
                }
            }
        }
        Ok(Section::Table(TableModel { columns, rows }))
    }
}

fn object_cell(cell: &CellSpec, object: &Object, site: &Site, resolver: &Resolver) -> Result<TableCell, SheafError> {
    match cell {
        CellSpec::Text { expr } => Ok(TableCell::Text(object_value(expr, object, site, resolver)?.into_text())),
        CellSpec::Status { from } => {
            let word = object_value(from, object, site, resolver)?.into_text();
            let (_, color) = status_of(&word);
            Ok(TableCell::Status { label: word, color })
        }
        CellSpec::Meter { value, scale, text } => Ok(TableCell::Meter {
            ratio: object_value(value, object, site, resolver)?.into_number(value)? / scale,
            text: object_value(text, object, site, resolver)?.into_text(),
            color: None,
        }),
    }
}

fn morphism_cell(cell: &CellSpec, morphism: &Morphism, resolver: &Resolver) -> Result<TableCell, SheafError> {
    match cell {
        CellSpec::Text { expr } => Ok(TableCell::Text(morphism_value(expr, morphism, resolver)?.into_text())),
        CellSpec::Meter { value, scale, text } => Ok(TableCell::Meter {
            ratio: morphism_value(value, morphism, resolver)?.into_number(value)? / scale,
            text: morphism_value(text, morphism, resolver)?.into_text(),
            color: None,
        }),
        CellSpec::Status { .. } => Err(SheafError::BadCell {
            scope: "edges",
            detail: "status",
        }),
    }
}

/// Series over object numeric channels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeSeriesSheaf {
    /// X value expression (object, numeric).
    pub x: String,
    /// Y value expression (object, numeric).
    pub y: String,
    /// One series per distinct sort (named by sort label).
    #[serde(default)]
    pub group_by_kind: bool,
    /// Series name used when not grouping by sort.
    #[serde(default = "series_default")]
    pub series_name: String,
}

impl TimeSeriesSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let mut series = Vec::new();
        if self.group_by_kind {
            for sort in distinct_sorts(resolver, site) {
                let mut points = Vec::new();
                for object in site
                    .objects
                    .iter()
                    .filter(|object| resolver.object_sort(&object.kind) == Some(sort.as_str()))
                {
                    points.push(self.point(object, site, resolver)?);
                }
                points.sort_by(|a, b| a.0.total_cmp(&b.0));
                series.push(SeriesModel {
                    name: resolver.sort_object_label(&sort),
                    points,
                });
            }
        } else {
            let mut points = Vec::new();
            for object in &site.objects {
                points.push(self.point(object, site, resolver)?);
            }
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
            series.push(SeriesModel {
                name: self.series_name.clone(),
                points,
            });
        }
        Ok(Section::TimeSeries(series))
    }

    fn point(&self, object: &Object, site: &Site, resolver: &Resolver) -> Result<(f64, f64), SheafError> {
        Ok((
            object_value(&self.x, object, site, resolver)?.into_number(&self.x)?,
            object_value(&self.y, object, site, resolver)?.into_number(&self.y)?,
        ))
    }
}

/// Gauges over objects, or one summed gauge per sort.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GaugesSheaf {
    /// Whether one gauge is emitted per object or summed per sort.
    pub scope: GroupScope,
    /// Numeric value expression.
    pub value: String,
    /// Divisor applied to the value for the fill ratio.
    #[serde(default = "one")]
    pub scale: f64,
    /// Label expression (per-object scope); empty uses the object title.
    #[serde(default)]
    pub label: String,
    /// Text expression (per-object scope); empty uses the value.
    #[serde(default)]
    pub text: String,
}

/// Grouping for gauge-like sheaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupScope {
    /// One entry per object.
    Objects,
    /// One entry per distinct object sort (summed).
    Kinds,
}

impl GaugesSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let mut gauges = Vec::new();
        match self.scope {
            GroupScope::Objects => {
                for object in &site.objects {
                    let raw = object_value(&self.value, object, site, resolver)?.into_number(&self.value)?;
                    let label = if self.label.is_empty() {
                        object.title.clone()
                    } else {
                        object_value(&self.label, object, site, resolver)?.into_text()
                    };
                    let text = if self.text.is_empty() {
                        fmt_num(raw)
                    } else {
                        object_value(&self.text, object, site, resolver)?.into_text()
                    };
                    gauges.push(GaugeModel {
                        label,
                        ratio: raw / self.scale,
                        text,
                    });
                }
            }
            GroupScope::Kinds => {
                for sort in distinct_sorts(resolver, site) {
                    let mut sum = 0.0;
                    for object in site
                        .objects
                        .iter()
                        .filter(|object| resolver.object_sort(&object.kind) == Some(sort.as_str()))
                    {
                        sum += object_value(&self.value, object, site, resolver)?.into_number(&self.value)?;
                    }
                    gauges.push(GaugeModel {
                        label: resolver.sort_object_label(&sort),
                        ratio: sum / self.scale,
                        text: fmt_num(sum),
                    });
                }
            }
        }
        Ok(Section::Gauges(gauges))
    }
}

/// Flamegraph of a typed-morphism call tree.
///
/// A span's value is the gluing of sections over the covering sieve generated
/// by the object's out-morphisms of the chosen kind: its own value plus the
/// glued values of its children.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlamegraphSheaf {
    /// Only follow morphisms of this kind.
    pub morphism_kind: String,
    /// Numeric value expression for an object's own cost; span width is the
    /// object's own value plus its subtree.
    pub value: String,
    /// Root object id; when absent, every source with no in-kind parent roots.
    #[serde(default)]
    pub root: Option<String>,
    /// Regime object sorts to restrict roots and traversed objects to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restrict: Option<BTreeSet<String>>,
}

impl FlamegraphSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let roots = match &self.root {
            Some(id) => {
                if !site.has_object(id) {
                    return Err(SheafError::MissingRoot(id.clone()));
                }
                vec![id.clone()]
            }
            None => site
                .objects
                .iter()
                .filter(|object| {
                    !site.morphisms.iter().any(|morphism| {
                        morphism.codomain == object.id
                            && morphism.kind.as_deref() == Some(self.morphism_kind.as_str())
                    })
                })
                .map(|object| object.id.clone())
                .collect(),
        };

        let mut spans = Vec::new();
        for root in roots {
            let mut path = vec![root.clone()];
            self.walk(&root, 0, &mut spans, &mut path, site, resolver)?;
        }
        Ok(Section::Flamegraph(spans))
    }

    /// Pushes the subtree rooted at `id` in preorder and returns its inclusive value.
    fn walk(
        &self,
        id: &str,
        depth: u16,
        spans: &mut Vec<FlameSpanModel>,
        path: &mut Vec<String>,
        site: &Site,
        resolver: &Resolver,
    ) -> Result<f64, SheafError> {
        let object = site
            .objects
            .iter()
            .find(|object| object.id == id)
            .ok_or_else(|| SheafError::MissingRoot(id.to_owned()))?;
        let own = object_value(&self.value, object, site, resolver)?.into_number(&self.value)?;
        let slot = spans.len();
        spans.push(FlameSpanModel {
            label: object.title.clone(),
            depth,
            value: own,
            color: None,
        });
        let mut total = own;
        for morphism in site
            .morphisms
            .iter()
            .filter(|morphism| morphism.domain == id && morphism.kind.as_deref() == Some(self.morphism_kind.as_str()))
        {
            if path.contains(&morphism.codomain) {
                continue;
            }
            path.push(morphism.codomain.clone());
            total += self.walk(&morphism.codomain, depth + 1, spans, path, site, resolver)?;
            path.pop();
        }
        // Summation noise (7240.000000000005) would leak into painted labels.
        total = (total * 1000.0).round() / 1000.0;
        spans[slot].value = total;
        Ok(total)
    }
}

/// Boxplot of an object numeric channel per sort.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxplotSheaf {
    /// Numeric value expression sampled per object.
    pub sample: String,
}

impl BoxplotSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let mut boxes = Vec::new();
        for sort in distinct_sorts(resolver, site) {
            let mut samples = Vec::new();
            for object in site
                .objects
                .iter()
                .filter(|object| resolver.object_sort(&object.kind) == Some(sort.as_str()))
            {
                samples.push(object_value(&self.sample, object, site, resolver)?.into_number(&self.sample)?);
            }
            boxes.push(BoxItemModel {
                label: resolver.sort_object_label(&sort),
                samples,
                color: None,
            });
        }
        Ok(Section::Boxplot(boxes))
    }
}

/// Aggregated single meter over all objects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeterSheaf {
    /// Meter label.
    pub label: String,
    /// Numeric value expression aggregated across objects.
    pub value: String,
    /// Aggregation applied to the per-object values.
    pub agg: Agg,
    /// Divisor applied to the aggregate for the fill ratio.
    #[serde(default = "one")]
    pub scale: f64,
    /// Unit appended to the display text.
    #[serde(default)]
    pub unit: String,
}

/// Aggregation function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agg {
    /// Sum of values.
    Sum,
    /// Arithmetic mean.
    Avg,
    /// Maximum value.
    Max,
    /// Minimum value.
    Min,
    /// Object count (value expression ignored).
    Count,
}

impl MeterSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let aggregate = match self.agg {
            Agg::Count => site.objects.len() as f64,
            _ => {
                let mut values = Vec::new();
                for object in &site.objects {
                    values.push(object_value(&self.value, object, site, resolver)?.into_number(&self.value)?);
                }
                match self.agg {
                    Agg::Sum => values.iter().sum(),
                    Agg::Avg => {
                        if values.is_empty() {
                            0.0
                        } else {
                            values.iter().sum::<f64>() / values.len() as f64
                        }
                    }
                    Agg::Max => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    Agg::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
                    Agg::Count => unreachable!("count handled above"),
                }
            }
        };
        let aggregate = if aggregate.is_finite() { aggregate } else { 0.0 };
        let text = if self.unit.is_empty() {
            fmt_num(aggregate)
        } else {
            format!("{} {}", fmt_num(aggregate), self.unit)
        };
        Ok(Section::Meter(MeterModel {
            label: self.label.clone(),
            ratio: aggregate / self.scale,
            text,
            color: None,
        }))
    }
}

/// Worst-of single status across objects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusSheaf {
    /// Status label.
    pub label: String,
    /// Expression yielding each object's status word.
    pub from: String,
    /// Regime object sorts to restrict to; absent means every object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restrict: Option<BTreeSet<String>>,
}

impl StatusSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let mut worst = String::from("unknown");
        let mut worst_rank = i32::MIN;
        for object in &site.objects {
            let word = object_value(&self.from, object, site, resolver)?.into_text();
            let rank = severity(&word);
            if rank > worst_rank {
                worst_rank = rank;
                worst = word;
            }
        }
        let (state, color) = status_of(&worst);
        Ok(Section::Status(StatusModel {
            label: self.label.clone(),
            state,
            color,
        }))
    }
}

/// Assembly invariants over the site restricted to chosen morphism sorts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblySheaf {
    /// Regime morphism sorts whose morphisms build the 1-skeleton.
    pub morphism_sorts: Vec<String>,
}

impl AssemblySheaf {
    fn selected(&self) -> BTreeSet<String> {
        self.morphism_sorts.iter().cloned().collect()
    }

    fn evaluate_table(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let components = assembly(site, resolver.morphism_sorting, &self.selected());
        let columns = assembly_columns();
        let mut rows = Vec::new();
        for component in &components {
            let cells = vec![
                TableCell::Text(component.representative.clone()),
                TableCell::Text(component.v.to_string()),
                TableCell::Text(component.e.to_string()),
                TableCell::Text(component.f.to_string()),
                TableCell::Text(component.b0.to_string()),
                TableCell::Text(component.b1.to_string()),
                TableCell::Text(component.chi.to_string()),
                TableCell::Text(component.boundary.to_string()),
                TableCell::Text(stage_label(&resolver.stalks.stages, component.stage).to_owned()),
            ];
            rows.push(TableRow { cells });
        }
        Ok(Section::Table(TableModel { columns, rows }))
    }

    fn evaluate_badges(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let components = assembly(site, resolver.morphism_sorting, &self.selected());
        let mut badges = Vec::new();
        for stage in STAGE_ORDER {
            let count = components.iter().filter(|component| component.stage == stage).count();
            if count == 0 {
                continue;
            }
            let value = format!("{} {count}", stage_label(&resolver.stalks.stages, stage));
            badges.push(BadgeSpec::new("stage", value, BadgeKind::Generic));
        }
        Ok(Section::Badges(badges))
    }
}

/// Physics section as a table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsSheaf {
    /// Objects or morphisms.
    pub scope: PhysicsScope,
}

/// Physics table scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicsScope {
    /// One row per object.
    Objects,
    /// One row per morphism.
    Morphisms,
}

impl PhysicsSheaf {
    fn evaluate(&self, resolver: &Resolver, site: &Site) -> Result<Section, SheafError> {
        let section = resolver.physics_section(site);
        let (columns, rows) = match self.scope {
            PhysicsScope::Objects => {
                let columns = vec![
                    physics_column("object", "Object", TextAlign::Left),
                    physics_column("type", "Type", TextAlign::Left),
                    physics_column("repulsion", "Repulsion", TextAlign::Right),
                    physics_column("mass", "Mass", TextAlign::Right),
                ];
                let rows = section
                    .objects
                    .iter()
                    .map(|(id, engine_type, repulsion, mass)| TableRow {
                        cells: vec![
                            TableCell::Text(id.clone()),
                            TableCell::Text(engine_type.clone()),
                            TableCell::Text(fmt_num(*repulsion)),
                            TableCell::Text(fmt_num(*mass)),
                        ],
                    })
                    .collect();
                (columns, rows)
            }
            PhysicsScope::Morphisms => {
                let columns = vec![
                    physics_column("from", "From", TextAlign::Left),
                    physics_column("to", "To", TextAlign::Left),
                    physics_column("type", "Type", TextAlign::Left),
                    physics_column("k", "k", TextAlign::Right),
                    physics_column("rest", "Rest", TextAlign::Right),
                ];
                let rows = section
                    .morphisms
                    .iter()
                    .map(|(from, to, engine_type, weight, rest_length)| TableRow {
                        cells: vec![
                            TableCell::Text(from.clone()),
                            TableCell::Text(to.clone()),
                            TableCell::Text(engine_type.clone()),
                            TableCell::Text(fmt_num(*weight)),
                            TableCell::Text(fmt_num(*rest_length)),
                        ],
                    })
                    .collect();
                (columns, rows)
            }
        };
        Ok(Section::Table(TableModel { columns, rows }))
    }
}

/// The six assembly stages, from topological invariants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Path (β1 = 0, max degree ≤ 2).
    Chain,
    /// Tree with a branch (β1 = 0).
    Branched,
    /// Single cycle (β1 = 1, F = 0).
    Ring,
    /// Triangulated patch with boundary (χ = 1).
    Sheet,
    /// Triangulated tube with boundary (χ = 0).
    Tube,
    /// Closed triangulated surface (boundary = 0).
    Membrane,
}

/// Canonical stage order for badge strips.
const STAGE_ORDER: [Stage; 6] = [
    Stage::Chain,
    Stage::Branched,
    Stage::Ring,
    Stage::Sheet,
    Stage::Tube,
    Stage::Membrane,
];

/// Topological invariants of one connected component.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentInvariants {
    /// Representative object id (first in site order).
    pub representative: String,
    /// Object (vertex) count.
    pub v: usize,
    /// Morphism (edge) count, undirected and deduplicated.
    pub e: usize,
    /// Triangle (3-clique) count.
    pub f: usize,
    /// β0 (always 1 per component).
    pub b0: usize,
    /// β1 = E − V + β0.
    pub b1: i64,
    /// Euler characteristic χ = V − E + F.
    pub chi: i64,
    /// Boundary morphisms: edges in exactly one triangle.
    pub boundary: usize,
    /// Classified stage.
    pub stage: Stage,
}

/// Per-connected-component topological invariants over the site restricted to
/// the chosen regime morphism sorts (edge-induced subgraph).
pub fn assembly(
    site: &Site,
    morphism_sorting: &BTreeMap<String, String>,
    morphism_sorts: &BTreeSet<String>,
) -> Vec<ComponentInvariants> {
    let index_of: BTreeMap<&str, usize> = site
        .objects
        .iter()
        .enumerate()
        .map(|(i, object)| (object.id.as_str(), i))
        .collect();

    let mut edge_set: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut adj: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut vertices: BTreeSet<usize> = BTreeSet::new();
    for morphism in &site.morphisms {
        let sort = match morphism.kind.as_deref().and_then(|kind| morphism_sorting.get(kind)) {
            Some(sort) => sort,
            None => continue,
        };
        if !morphism_sorts.contains(sort.as_str()) {
            continue;
        }
        let (Some(&a), Some(&b)) = (
            index_of.get(morphism.domain.as_str()),
            index_of.get(morphism.codomain.as_str()),
        ) else {
            continue;
        };
        if a == b {
            continue;
        }
        let pair = if a < b { (a, b) } else { (b, a) };
        if edge_set.insert(pair) {
            adj.entry(a).or_default().insert(b);
            adj.entry(b).or_default().insert(a);
        }
        vertices.insert(a);
        vertices.insert(b);
    }

    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut components: Vec<BTreeSet<usize>> = Vec::new();
    for &start in &vertices {
        if seen.contains(&start) {
            continue;
        }
        let mut component: BTreeSet<usize> = BTreeSet::new();
        let mut stack = vec![start];
        while let Some(vertex) = stack.pop() {
            if !seen.insert(vertex) {
                continue;
            }
            component.insert(vertex);
            if let Some(neighbors) = adj.get(&vertex) {
                for &neighbor in neighbors {
                    if !seen.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }
        components.push(component);
    }
    components.sort_by_key(|component| *component.iter().min().expect("component is non-empty"));

    components
        .iter()
        .map(|component| component_invariants(site, component, &edge_set, &adj))
        .collect()
}

fn component_invariants(
    site: &Site,
    component: &BTreeSet<usize>,
    edge_set: &BTreeSet<(usize, usize)>,
    adj: &BTreeMap<usize, BTreeSet<usize>>,
) -> ComponentInvariants {
    let verts: Vec<usize> = component.iter().copied().collect();
    let v = verts.len();

    let comp_edges: Vec<(usize, usize)> = edge_set
        .iter()
        .copied()
        .filter(|(a, b)| component.contains(a) && component.contains(b))
        .collect();
    let e = comp_edges.len();

    let is_edge = |x: usize, y: usize| adj.get(&x).is_some_and(|neighbors| neighbors.contains(&y));

    let mut f = 0usize;
    for i in 0..verts.len() {
        for j in (i + 1)..verts.len() {
            if !is_edge(verts[i], verts[j]) {
                continue;
            }
            for k in (j + 1)..verts.len() {
                if is_edge(verts[i], verts[k]) && is_edge(verts[j], verts[k]) {
                    f += 1;
                }
            }
        }
    }

    let mut boundary = 0usize;
    for (a, b) in &comp_edges {
        let common = match (adj.get(a), adj.get(b)) {
            (Some(na), Some(nb)) => na
                .intersection(nb)
                .filter(|vertex| component.contains(vertex))
                .count(),
            _ => 0,
        };
        if common == 1 {
            boundary += 1;
        }
    }

    let max_degree = verts
        .iter()
        .map(|vertex| {
            adj.get(vertex)
                .map_or(0, |neighbors| neighbors.iter().filter(|n| component.contains(n)).count())
        })
        .max()
        .unwrap_or(0);

    let b0 = 1usize;
    let b1 = e as i64 - v as i64 + b0 as i64;
    let chi = v as i64 - e as i64 + f as i64;
    let stage = classify_stage(f, b1, chi, boundary, max_degree);
    let representative = site.objects[*component.iter().min().expect("component is non-empty")]
        .id
        .clone();

    ComponentInvariants { representative, v, e, f, b0, b1, chi, boundary, stage }
}

fn classify_stage(f: usize, b1: i64, chi: i64, boundary: usize, max_degree: usize) -> Stage {
    if f == 0 {
        if b1 == 0 {
            if max_degree <= 2 {
                Stage::Chain
            } else {
                Stage::Branched
            }
        } else {
            Stage::Ring
        }
    } else if boundary == 0 {
        Stage::Membrane
    } else if chi == 0 {
        Stage::Tube
    } else {
        Stage::Sheet
    }
}

fn stage_label(stages: &Stages, stage: Stage) -> &str {
    match stage {
        Stage::Chain => &stages.chain,
        Stage::Branched => &stages.branched,
        Stage::Ring => &stages.ring,
        Stage::Sheet => &stages.sheet,
        Stage::Tube => &stages.tube,
        Stage::Membrane => &stages.membrane,
    }
}

fn assembly_columns() -> Vec<TableColumn> {
    [
        ("component", "Component", TextAlign::Left),
        ("v", "V", TextAlign::Right),
        ("e", "E", TextAlign::Right),
        ("f", "F", TextAlign::Right),
        ("b0", "β0", TextAlign::Right),
        ("b1", "β1", TextAlign::Right),
        ("chi", "χ", TextAlign::Right),
        ("boundary", "Boundary", TextAlign::Right),
        ("stage", "Stage", TextAlign::Left),
    ]
    .into_iter()
    .map(|(key, title, align)| physics_column(key, title, align))
    .collect()
}

fn physics_column(key: &str, title: &str, align: TextAlign) -> TableColumn {
    TableColumn {
        key: key.to_owned(),
        title: title.to_owned(),
        width: ColumnWidth::Flex { weight: 1 },
        align,
    }
}

/// A typed value produced by a value expression.
enum Value {
    Str(String),
    Num(f64),
}

impl Value {
    fn into_text(self) -> String {
        match self {
            Self::Str(text) => text,
            Self::Num(number) => fmt_num(number),
        }
    }

    fn into_number(self, expr: &str) -> Result<f64, SheafError> {
        match self {
            Self::Num(number) => Ok(number),
            Self::Str(text) => text.parse::<f64>().map_err(|_| SheafError::NotNumeric {
                expr: expr.to_owned(),
                value: text,
            }),
        }
    }
}

fn object_value(expr: &str, object: &Object, site: &Site, resolver: &Resolver) -> Result<Value, SheafError> {
    match expr {
        "id" => Ok(Value::Str(object.id.clone())),
        "title" => Ok(Value::Str(object.title.clone())),
        "kind" => Ok(Value::Str(resolver.object_label(&object.kind))),
        "degree" => Ok(Value::Num(site.degree(&object.id) as f64)),
        "in_degree" => Ok(Value::Num(site.in_degree(&object.id) as f64)),
        "out_degree" => Ok(Value::Num(site.out_degree(&object.id) as f64)),
        "repulsion" => Ok(Value::Num(resolver.object_repulsion(&object.kind)?)),
        "mass" => Ok(Value::Num(resolver.object_mass(&object.kind)?)),
        _ => {
            if let Some(index) = expr.strip_prefix("tag:") {
                let index: usize = index
                    .parse()
                    .map_err(|_| SheafError::BadExpression(expr.to_owned()))?;
                Ok(Value::Str(object.tags.get(index).cloned().unwrap_or_default()))
            } else if let Some(name) = expr.strip_prefix("field?:") {
                Ok(Value::Str(object.fields.get(name).cloned().unwrap_or_default()))
            } else if let Some(name) = expr.strip_prefix("field:") {
                let value = object.fields.get(name).ok_or_else(|| SheafError::MissingField {
                    object: object.id.clone(),
                    field: name.to_owned(),
                })?;
                Ok(Value::Str(value.clone()))
            } else if let Some(name) = expr.strip_prefix("weighted:") {
                let base = object.number(name).ok_or_else(|| SheafError::MissingField {
                    object: object.id.clone(),
                    field: name.to_owned(),
                })?;
                Ok(Value::Num(base * resolver.object_repulsion(&object.kind)?))
            } else {
                Err(SheafError::BadExpression(expr.to_owned()))
            }
        }
    }
}

fn morphism_value(expr: &str, morphism: &Morphism, resolver: &Resolver) -> Result<Value, SheafError> {
    match expr {
        "source" => Ok(Value::Str(morphism.domain.clone())),
        "target" => Ok(Value::Str(morphism.codomain.clone())),
        "edge_kind" => Ok(Value::Str(resolver.morphism_label(morphism.kind.as_deref()))),
        "weight" => Ok(Value::Num(resolver.morphism_weight(morphism.kind.as_deref())?)),
        "rest_length" => Ok(Value::Num(resolver.morphism_rest_length(morphism.kind.as_deref())?)),
        _ => Err(SheafError::BadExpression(expr.to_owned())),
    }
}

/// Distinct object sorts in first-appearance order.
fn distinct_sorts(resolver: &Resolver, site: &Site) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut sorts = Vec::new();
    for object in &site.objects {
        if let Some(sort) = resolver.object_sort(&object.kind) {
            if seen.insert(sort.to_owned()) {
                sorts.push(sort.to_owned());
            }
        }
    }
    sorts
}

/// Map an stalks badge string to a [`BadgeKind`].
fn parse_badge(badge: &str) -> Result<BadgeKind, SheafError> {
    Ok(match badge {
        "tag" => BadgeKind::Tag,
        "doctype" => BadgeKind::Doctype,
        "folder" => BadgeKind::Folder,
        "author" => BadgeKind::Author,
        "date" => BadgeKind::Date,
        "status" => BadgeKind::Status,
        "generic" => BadgeKind::Generic,
        "entity" => BadgeKind::Entity { ty: None },
        other => return Err(SheafError::BadBadge(other.to_owned())),
    })
}

/// Map a status word to a coarse state and a renderer-neutral colour.
fn status_of(word: &str) -> (StatusState, Rgb) {
    match word {
        "ok" => (StatusState::Ok, (39, 201, 63)),
        "warn" | "warning" => (StatusState::Warning, (255, 189, 46)),
        "error" | "err" => (StatusState::Error, (255, 95, 86)),
        "info" => (StatusState::Info, (59, 155, 255)),
        _ => (StatusState::Unknown, (127, 122, 122)),
    }
}

/// Severity rank of a status word; higher is worse.
fn severity(word: &str) -> i32 {
    match word {
        "error" | "err" => 3,
        "warn" | "warning" => 2,
        "info" => 1,
        "ok" => 0,
        _ => -1,
    }
}

/// Format a number as the shortest round-trip string (no trailing `.0`).
fn fmt_num(number: f64) -> String {
    format!("{number}")
}

fn one() -> f64 {
    1.0
}

fn series_default() -> String {
    "series".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morphism::Physics;
    use crate::topos::Stalks;

    fn object(id: &str, cost: &str) -> Object {
        Object {
            id: id.to_owned(),
            title: id.to_owned(),
            kind: "planner".to_owned(),
            tags: Vec::new(),
            fields: BTreeMap::from([("cost".to_owned(), cost.to_owned())]),
        }
    }

    fn call(source: &str, target: &str) -> Morphism {
        Morphism { domain: source.to_owned(), codomain: target.to_owned(), kind: Some("continues".to_owned()) }
    }

    #[test]
    fn flamegraph_spans_are_inclusive_of_their_subtree() {
        let site = Site {
            objects: vec![object("root", "1"), object("a", "2"), object("b", "4"), object("leaf", "8")],
            morphisms: vec![call("root", "a"), call("root", "b"), call("a", "leaf")],
        };
        let object_sorting = BTreeMap::new();
        let morphism_sorting = BTreeMap::new();
        let stalks = Stalks { objects: BTreeMap::new(), morphisms: BTreeMap::new(), stages: Stages::default() };
        let physics = Physics::empty();
        let resolver = Resolver {
            object_sorting: &object_sorting,
            morphism_sorting: &morphism_sorting,
            stalks: &stalks,
            physics: &physics,
        };
        let sheaf = FlamegraphSheaf {
            morphism_kind: "continues".to_owned(),
            value: "field:cost".to_owned(),
            root: None,
            restrict: None,
        };
        let Section::Flamegraph(spans) = sheaf.evaluate(&resolver, &site).unwrap() else {
            panic!("not a flamegraph");
        };
        let frames: Vec<(&str, u16, f64)> =
            spans.iter().map(|span| (span.label.as_str(), span.depth, span.value)).collect();
        assert_eq!(frames, [("root", 0, 15.0), ("a", 1, 10.0), ("leaf", 2, 8.0), ("b", 1, 4.0)]);
    }
}
