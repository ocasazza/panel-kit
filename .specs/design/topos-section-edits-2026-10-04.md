# Section edits: writing back through the site

Status: approved by the panel-kit-grammar owner with the changes below folded in; implemented on
`feat/topos-section-edits` over phase 2A of `feat/topos-grammar-site`. v1 is set-only.

## Problem

A regime's global sections are read-only: `Topos::global_sections(&site, &physics)` is Γ of every
sheaf. Regimes need user actions — accept or overturn a record, set a value — that change the data
the site is presented from. The crate must stay pure, sync and wasm32-clean, carry no domain
vocabulary in its types, and keep grammars as the only IO contract.

## Semantics

- A **section edit** is a named change to the local section of object data at the objects it
  applies to. It is declared by a topos, over regime sorts.
- A set-only edit does not change the underlying category: objects and morphisms are identical. It
  replaces the local section at its **support** (the objects it changed) and re-glues. The result is
  a **site revision** `ρ: S → S′` that is the identity on generators; only object data changes.
- Restriction along `ρ` recovers Γ over `S`, so sections are functorial in the site: every regime's
  Γ and the physics section are recomputed from `S′`. The edit is regime-local; the revision is not.
- The grammar presenting `S` gains an **emitter** that serializes the support of `ρ` in the wire
  format it parses. The crate returns a **write intent** — grammar id, edit id, emitted bytes,
  support — and no transport detail; the host binds grammar id to a route and performs the IO.

Laws (tested for every engine and declared edit), with `U = support(ρ)`:

    let (S′, ρ) = S.revise(edit, x)                       // valid x
    parse(emit(S′, U)) restricted to U == S′ restricted to U
    emit(parse(emit(S′, U)), U) == emit(S′, U)            // byte-for-byte

An invalid `x` is an error with `S` unchanged.

## Spec shape (additive; serde `deny_unknown_fields`)

Topos document gains `edits`; sheaves reference them:

```json
"edits": {
  "<edit id>": {
    "label": "...",
    "sorts": ["<regime sort>"],
    "set": { "<field>": { "type": "enum", "values": ["a", "b"] } },
    "grammar": "<grammar id>"
  }
},
"sheaves": {
  "<binding id>": { "kind": "table", "...": "...", "edits": ["<edit id>"] }
}
```

Field types: `enum { values }`, `text { max }`, `int { min, max }`, `lines { max }`. Values are
validated against the type and stored in the site's string field model, so both engines round-trip
without new value types. A section carries, per object, the ids of the edits that apply to it, so a
host renders controls without knowing the regime.

Grammar package gains an `emitter`:

- `json` engine: `{ "engine": "json" }` — no settings. The emitter writes the support's objects as
  records at the parser's `nodes` pointer through the parser's own member map (the inverse by
  construction), with an empty array at `edges` when the parser reads one. An object with a field
  or tags the map does not carry is refused, since re-parsing would lose it.
- `pest` engine: `{ "engine": "pest", "object": "<line template>" }` with `{id}`, `{title}`,
  `{kind}`, `{tags}` and `{fields}` holes; `tags_separator` (`,`), `fields_separator` (`;`) and
  `field_assign` (`=`) default as shown. `{fields}` renders every field in key order: a per-key hole
  could not pass the field-loss check for objects carrying other fields. Pest grammars are not
  invertible in general, so emitters are not derived from the capture map. At package compile time
  the template is validated by parsing its output for a representative object (two tags, two
  fields); output that parses to a different object (field loss) is rejected. Set-only edits change
  no morphisms, so v1 has no morphism template.

## Crate surface (authoritative vocabulary)

```rust
pub struct SectionEdit { label, sorts: BTreeSet<String>, set: BTreeMap<String, FieldType>, grammar }
pub struct SiteRevision { pub edit: String, pub support: BTreeSet<String> }
impl Topos {
    pub fn revise(&self, site: &Site, edit: &str, input: &EditInput) -> Result<(Site, SiteRevision), EditError>;
    pub fn write_intent(&self, grammar: &CompiledGrammar, revised: &Site, revision: &SiteRevision)
        -> Result<WriteIntent, EditError>;
}
impl CompiledGrammar {
    pub fn emit(&self, site: &Site, support: &BTreeSet<String>) -> Result<Vec<u8>, GrammarError>;
}
pub struct WriteIntent { pub grammar: String, pub edit: String, pub bytes: Vec<u8>, pub support: BTreeSet<String> }
impl GlobalSections { pub fn row_edits(&self, binding: &str) -> Option<&[RowEdits]>; }
```

`revise` is on `Topos`, not `Site`: whether an edit applies depends on the object's regime sort,
which only the topos's sorting knows. Per-row edits sit beside the sections
(`GlobalSections::row_edits`) because panel-kit-core's `TableRow` carries no object id.

Validation at `Topos` decode: every edit's sorts exist, its fields are typed and can accept a value,
it names a grammar, and every table's offered edits exist and sit on an object-scoped table.
`write_intent` checks the grammar id; `emit` reports a grammar without an emitter.

## Restriction and gluing (same change set)

- `restrict` on Table/Badges/Status/Flamegraph sheaves: Γ over the subobject `U` of regime sorts.
  Object scope evaluates over the full subsite on objects sorted into `U` (so a Flamegraph's roots
  and traversed objects are both restricted); a morphism-scoped table keeps every object and the
  morphisms sorted into `U` (untyped morphisms lie outside every restriction). Laws tested:
  `restrict(U) ∘ restrict(V) = restrict(U ∩ V)`, `restrict(all sorts) = id`.
- `Site::glue(&a, &b, along_kind)`: the pushout over the discrete site of `along_kind` objects both
  sides present under one id. Identified objects must be equal (anything else is
  `GlueDisagrees`); any other shared id is `IdCollision`. Laws tested: associativity; restricting
  the glued site to one side's kinds returns that side.

## Tests

- `revise`: set within type, set outside type (error, site unchanged), unknown object, an object
  whose sort the edit does not declare.
- Both laws for the `json` and `pest` emitters, on the demo site and on a hand-built site per sort.
- Pest template validation rejects a template that loses a field.
- Sections expose edit ids per object; a sheaf referencing an unknown edit fails validation.

## Example regime (non-normative; lives in Nix specs, not crate types)

The omp auto-loop control regime: a `json` grammar over the loop's control site, one flat `objects`
array whose records name their sort (claim, proposal, queued_goal, repo, repo_policy, term, param,
prompt) plus typed `morphisms`. The member map is the union of every sort's fields, which the
server pins in a test. Repo objects are exactly the trace's (`id`, `title`, no tags, `root`), so
the control and trace sites glue along `repo`; what policy says about a repo lives on a
`repo_policy` object with a `governs` morphism to it. The emitter's records are accepted on the
server's write route and folded into its policy log. Edits: ratify a claim (`decision: enum
accepted|overturned`, `note: text`), decide a proposal, set a repo policy's gates (`lines`) and done
criteria (`text`), set a param (`int`), set a prompt (`text`). Effects beyond the record (steering a
session after an overturn) belong to that server.

## Future work

- `create` edits: an inclusion `ρ: S ↪ S′` (a monomorphism; `S` is a full subsite of `S′`), with an
  explicit host-supplied id policy, since the crate has no clock or randomness.
- Transporting an edit along a geometric morphism `f: E → F` (φ on sorts): the sound direction is
  `∀_f(U)`, not `∃_f(U)`. Offering an edit in `F` on sort `t` applies it to every object whose
  `E`-sort is in `φ⁻¹(t)`, which is valid only when `φ⁻¹(t) ⊆ U`, i.e. `t ∈ ∀_f(U)`. v1 keeps edits
  regime-local.
