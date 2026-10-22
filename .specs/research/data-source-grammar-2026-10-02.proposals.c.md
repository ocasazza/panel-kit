# DataSource Grammar Abstraction Proposals

Designed for panel-kit-core to upstream jump-cannon's importer package model.

## Background

jump-cannon's importer package declares:
- **Envelope**: `metadata` (id, name, version), `limits` (nodes, edges, bytes), `schema` (fields + edge_types)
- **Parser**: `engine` (json/pest/tvix), `variables` (runtime-bound), `preflight` (existence check), `collections` (endpoints + projections)
- **Separation**: package is mechanism; `InstanceConfig` binds URL/credentials at runtime

panel-kit-core constraints:
- Pure data + traits; no I/O, no async, no transport
- `WorkspaceSpec` already handles layout/panel declarations
- `PanelKind` trait owns renderer-neutral identities
- Consumer owns domain types; core owns grammar format

---

## High-Probability Proposals (>0.80)

### 1. DataSourceSchema Only

**Summary:** Core declares only field + edge schema as pure serde types; collection/runtime binding lives entirely in shells.

**Description:**
This approach extracts the `schema` envelope from the importer package (fields, edge_types, limits, metadata) into core. Add `DataSourceSchema`, `FieldSpec`, `EdgeTypeSpec`, and `DataSourceLimits` types. A DataSource is declared in `WorkspaceSpec` alongside panels, with `panel_data_source_id` linking a `PanelKind` to a DataSource ID.

Shells implement:
- `DataSourceRuntime` trait with `resolve_variables`, `fetch_collection`, `map_to_graph`
- Runtime binding that binds a schema to endpoints/credentials
- Panel renderers that consume `Snapshot<GraphNode>` from the reducer

**Key Design Decisions:**
- **No parser/collection types in core**: Keeps core minimal; shells own transport
- **Schema-only validation**: Core validates field names/types, edge types, and limits
- **PanelKind integration**: Existing `PanelKind` enum gets `data_source: Option<String>` field via `WorkspaceSpec`

**Trade-offs:**
- Pro: Minimal core surface, no async/transport dependencies
- Pro: Shells own full transport stack (HTTP, filesystem, etc.)
- Con: Each shell re-implements collection pagination, caching, retry logic
- Con: Cross-shell behavior may diverge

**Probability:** 0.90  
**Complexity:** Low  
**Risks:**
- Shells may implement inconsistent pagination/retry strategies
- No shared validation for pointer paths or transforms
- Multi-panel data joins require shell-side coordination

---

### 2. Full Package Grammar (Abstracted)

**Summary:** Mirror jump-cannon's full package structure (schema + collections + variables) but strip engine-specific details from core.

**Description:**
Core exports `DataSourcePackage` with `metadata`, `limits`, `schema`, and `CollectionSpec` (path, pagination, node/edge rules). Variables remain runtime-bound. Remove engine-specific grammar/AST, parser state, and I/O from core.

The `ParserConfig` enum becomes `CollectionEngine` (json, pest, tvix) without implementation details. Shells implement `CollectionExecutor` trait that dispatches on engine kind.

**Key Design Decisions:**
- **CollectionSpec is declarative**: Paths, pagination, pointers, transforms live in core grammar
- **No engine implementation**: Core declares "what to fetch"; shells implement "how"
- **Variables abstracted**: `VariableSpec` declares name/description/default; runtime supplies values

**Trade-offs:**
- Pro: Full importer feature parity; shells only add transport
- Pro: Schema validation happens consistently in core
- Pro: Cross-shell compatibility via shared collection spec
- Con: Larger core surface area
- Con: Engine selection becomes a runtime decision

**Probability:** 0.85  
**Complexity:** Medium  
**Risks:**
- Core bloat if collection specs grow too complex
- Engine selection requires runtime state in shells
- Backward compatibility with existing WorkspaceSpec users

---

### 3. PanelSpec DataSource Extension

**Summary:** Extend `PanelSpec` with `data_source` config; DataSource grammar is embedded within panel declarations.

**Description:**
Add `DataSourceSpec` field to `PanelSpec` containing `source_id`, `collection`, `binding` (variable bindings). DataSource schema lives in `WorkspaceSpec`'s `data_sources` map keyed by ID.

This keeps DataSource grammar tightly coupled to panels. Each panel declares which DataSource feeds it, with `WorkspaceSpec` containing both panel layout and DataSource schema.

**Key Design Decisions:**
- **No top-level DataSource abstraction**: DataSource is a panel concern
- **Schema per DataSource**: One `DataSourceSpec` shared by multiple panels
- **Binding resolved at workspace resolution**: `ResolvedWorkspace` contains bound variables

**Trade-offs:**
- Pro: Minimal new abstractions; uses existing `WorkspaceSpec`
- Pro: Panel-centric view matches existing UX
- Con: DataSource can't exist independent of panels
- Con: Shared DataSource across panels requires explicit declaration

**Probability:** 0.82  
**Complexity:** Low  
**Risks:**
- Doesn't support global DataSource (non-panel data sources)
- DataSource schema duplication if multiple panels reference same ID
- Limited reusability across different WorkspaceSpecs

---

## Low-Probability Proposals (<0.10)

### 4. DSL/AST Grammar

**Summary:** DataSource is a declarative DSL parsed into core AST; shells compile AST to transport.

**Description:**
Define a DataSource DSL (TOML/YAML/JSON schema) with declarative syntax for fields, relationships, and collection queries. Core exports AST types (`DataSourceAST`, `FieldNode`, `EdgeNode`, `QueryNode`) and a parser/validator.

Shells implement `DataSourceCompiler` that transforms AST into runtime graph operations.

**Trade-offs:**
- Pro: DSL is human-readable; easy to write packages
- Pro: AST enables programmatic manipulation
- Con: Requires DSL parser in core or shell
- Con: Adds serialization/format complexity

**Probability:** 0.05  
**Complexity:** High  
**Risks:**
- DSL design is complex (pointer syntax, transforms, filters)
- Core parser increases surface area
- DSL evolution breaks backward compatibility

---

### 5. Graph Schema Only

**Summary:** Minimal graph model with nodes/edges/types; no collection/endpoint declarations.

**Description:**
Core defines `NodeSpec`, `EdgeSpec`, `GraphSchema` (node types, edge types, field indexes). DataSource binding happens entirely in shells via `GraphBuilder` trait.

This approach assumes DataSource schema is derived from external APIs (e.g., OpenAPI, GraphQL introspection) rather than authored packages.

**Trade-offs:**
- Pro: Minimal abstraction; maximum flexibility
- Pro: Easy to auto-generate from API schemas
- Con: No package validation
- Con: No shared collection semantics across shells

**Probability:** 0.03  
**Complexity:** Low  
**Risks:**
- Assumes schema discovery (not package-based)
- Doesn't align with jump-cannon's package model
- Requires external schema tooling

---

### 6. Runtime Plugin Registration

**Summary:** Shells register DataSource plugins via trait at runtime; core is purely an event bus.

**Description:**
Core provides `DataSourceRegistry` with `register(plugin: DataSourcePlugin)` and `resolve(id: &str)`. Plugin trait defines `schema()`, `fetch()`, `map()`. No DataSource grammar in core.

**Trade-offs:**
- Pro: Maximum flexibility; any shell can add plugins
- Pro: Zero core abstraction overhead
- Con: No validation of registered plugins
- Con: No standard schema or collection semantics

**Probability:** 0.02  
**Complexity:** High (runtime complexity)  
**Risks:**
- No compile-time or load-time validation
- Plugin discovery requires runtime coordination
- Contradicts "core owns grammar" constraint

---

## Recommendation

For practical adoption with jump-cannon alignment: **Proposal 2 (Full Package Grammar)** is recommended. It provides:

1. Feature parity with importer package model
2. Separation of mechanism (grammar) from instance (binding)
3. Validation in core; transport in shells
4. Composable with WorkspaceSpec via `data_sources` field

Follow with **Proposal 1 (Schema Only)** as a minimal v1 iteration, then expand to full grammar once shells demonstrate transport patterns.

---

## Implementation Notes

**Files to create:**
- `panel-kit-core/src/data_source.rs` (new module)
  - `DataSourceSchema`, `FieldSpec`, `EdgeTypeSpec`, `DataSourceLimits`
  - `DataSourcePackage` (for full grammar approach)
  - `DataSourceId`, `DataSourceBinding`

**Files to modify:**
- `panel-kit-core/src/spec.rs`
  - Add `data_sources: HashMap<DataSourceId, DataSourceSpec>` to `WorkspaceSpec`
  - Add `data_source_id: Option<String>` to `PanelSpec`

**Files to extend:**
- `panel-kit-core/src/persist.rs`
  - `DataSourceStore` trait for schema persistence
  - `restore_snapshot` extension to hydrate DataSource state

**Backward compatibility:**
- Existing single-spec consumers unaffected (data_sources is optional)
- `PanelKind` enum unchanged (use `WorkspaceSpec` linkage instead)
- `LayoutStore` unchanged (DataSource persistence separate)
