# DataSource Grammar Abstraction Proposals

**Date:** 2026-10-02  
**Goal:** Design an abstract DataSource grammar for panel-kit-core that upstreams jump-cannon's importer package model while maintaining core purity (no transport/async).

---

## Practical Approaches (>0.80 Probability)

### 1. Structured Spec (Schema-First)

**Summary:** Core defines `DataSourceSpec` struct mirroring the importer TOML schema (fields, edge_types, collections) and remains purely declarative.

**Description:** Core owns a Serde-serializable `DataSourceSpec` matching the importer package schema (metadata, limits, schema.fields, schema.edge_types, parser.variables). Panels declare a `DataSourceSpec` reference in their `ContentSpec`. Shells implement `DataSourceProvider` trait to load instance data at runtime, binding runtime variables to the parser block. No transport logic lives in core.

**Key Decisions:**
- `DataSourceSpec` is pure data with `Serialize + Deserialize` bounds (aligned with WorkspaceSpec).
- `parser` block is schema-only (variables declared, bindings supplied by shells).
- Collections and field types map to existing `ContentKind` where possible.

**Trade-offs:**
- High alignment with existing importer format (easy migration).
- Core schema evolves independently of runtime binding concerns.

**Probability:** 0.90  
**Complexity:** Medium  
**Risks:** Version drift between package format and core spec; validation burden on shells.

---

### 2. Trait-Based Provider Abstraction

**Summary:** Core defines `DataSourceProvider` trait (schema, edges, load) and specs just reference provider IDs.

**Description:** Instead of schema-in-core, core defines a `DataSourceProvider` trait exposing `fn schema() -> Spec`, `fn edges() -> EdgeSet`, and async-compatible `fn load()`. DataSource `DataSourceSpec` contains only a provider ID and instance config. Shells register concrete providers at runtime and resolve IDs to implementations.

**Key Decisions:**
- Core owns trait contracts; shells own implementations and transport.
- Specs use stable string IDs (e.g., `hindsight.memory-bank`) to reference providers.
- Schema metadata lives on the provider trait implementation.

**Trade-offs:**
- High flexibility for shells to swap implementations.
- Slightly more indirection than Structured Spec.

**Probability:** 0.85  
**Complexity:** Medium  
**Risks:** ID naming collisions; runtime provider registration complexity.

---

### 3. Minimal Graph Grammar

**Summary:** Core defines lean `Node`, `Edge`, and `Field` types capturing only essential graph structure; shells handle complex logic.

**Description:** Core owns a minimal grammar: `Node { id: String, fields: Vec<Field>, edges: Vec<Edge> }`, `Edge { from: String, to: String, kind: String }`, and `Field { key: String, type: FieldType }`. Parser blocks are replaced with shell-supplied fetchers. Specs reference a `GraphGrammar` and let shells resolve the full schema.

**Key Decisions:**
- Core grammar captures only what panels need for display/indexing.
- Complex logic (pagination, filtering) remains shell-side.
- Aligns with panel-kit's "pure data" philosophy.

**Trade-offs:**
- Simpler core schema; shells carry more implementation weight.
- May require additional shell-side mapping for importer features.

**Probability:** 0.88  
**Complexity:** Low  
**Risks:** Insufficient expressiveness for complex importer packages.

---

## Exploratory Approaches (<0.10 Probability)

### 4. DSL Scripting for Schema Logic

**Summary:** Core exposes a lightweight DSL (Lua/Wasm) allowing dynamic schema definitions at runtime.

**Description:** Instead of static Serde structs, core defines a `ScriptSchema` interface. Shells can load scripts defining field transformations, edge derivations, and collection filters. Core interprets the script output as a `DataSourceSpec`.

**Trade-offs:** Extremely flexible; allows runtime schema evolution without recompilation. Core complexity increases significantly; potential security/sandboxing concerns; deviates from "pure data" model.

**Probability:** 0.05  
**Complexity:** High  
**Risks:** Runtime evaluation overhead; scripting security surface.

---

### 5. Type-Driven Codegen (Build-Time)

**Summary:** DataSource specs are compile-time macros generating Rust types and serialization code.

**Description:** Shells declare DataSource specs in `build.rs` or proc-macros. Core provides derive macros (`#[derive(DataSource)]`) generating `DataSourceSpec`, `Node`, and `Edge` types per spec. No runtime schema storage; all schema becomes concrete Rust types.

**Trade-offs:** Zero runtime schema overhead; type-safe schema access. Violates core's runtime-agnostic design; requires rebuilds for schema changes; hard to support multi-tenant scenarios.

**Probability:** 0.02  
**Complexity:** High  
**Risks:** Breaks core/shell separation; inflexible for runtime bindings.

---

### 6. External Schema Service

**Summary:** DataSource schemas are fetched from an external HTTP service; core treats them as external resources.

**Description:** Core defines `DataSourceSpec` as a URL reference. On load, shells fetch and deserialize the schema from a service. Core validates structure but doesn't store schema definitions locally.

**Trade-offs:** Schema updates without app rebuilds; centralized schema management. Requires network access; dependency on external service availability; complicates offline/air-gapped use.

**Probability:** 0.08  
**Complexity:** Medium  
**Risks:** Network dependency; service availability; caching complexity.

---

</content>