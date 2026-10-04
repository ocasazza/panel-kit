# The omp auto-loop regimes

Status: agreed with the panel-kit-grammar owner; authored as Nix specs after phase 2A
(`nix/specs/topos-autoloop*.nix`, `nix/grammars/autoloop-*.nix`). Domain words live only in those
specs, never in crate types.

## Sites, and how they become one

| Site | Grammar | Objects (site kinds) | Morphisms (site kinds) |
|---|---|---|---|
| trace | `autoloop-trace` (pest; the line format of jump-cannon's `omp-auto-loop.toml`) | `session`, `event`, `goal`, `repo`, `gate` | `in_repo`, `pursues`, `runs_gate`, `spawned`, `emitted`, `next` |
| control | `autoloop-control` (json; one flat `objects` array and a `morphisms` array from the loop's `GET /api/site`, written back through its emitter to `POST /api/site`) | `claim`, `proposal`, `queued_goal`, `repo`, `repo_policy`, `term`, `param`, `prompt` | `targets` (queued_goal → repo), `governs` (repo_policy → repo), `revises` (proposal → param or prompt) |

A `GeometricMorphism` relates topoi over the same site, so the two sites are first **glued** along
`repo`: the pushout `trace ← R → control`, where `R` is the discrete site of the repo objects both
present, identified by id. The loop's control site presents each repo the trace knows under the
trace's id with exactly the trace's data (`title`, no tags, `root`); repo policy lives on
`repo_policy`. Each grammar parses its own input; the host glues; both regimes are over the glued
site.

Crate addition (this work): `Site::glue(&a, &b, along_kind: &str) -> Result<Site, SiteError>` —
pure; objects of `along_kind` with equal ids are identified (they must be equal; anything else is
`GlueDisagrees`); any other id collision is `IdCollision`. Tests: glue is associative on the demo;
restricting the glued site to each side's kinds returns that side.

## `topos-autoloop-control`

Sorting: identity on control kinds; every trace kind except `repo` sorts to `elsewhere`, which has no
sheaves. Stalks: a label and a fixed badge kind per sort (badge kinds are per sort, not per value, so
a claim's decision and a repo policy's verification status are table columns, not badges).

Sheaves (each table `restrict`ed to one sort):

| Binding | Kind | `restrict` | Edits |
|---|---|---|---|
| `autoloop.claims` | table: title, session, reason, judge, decision | `claim` | `ratify` (`decision: enum accepted\|overturned`, `note: text`) |
| `autoloop.proposals` | table: title, before, after, rationale, status | `proposal` | `decide` (`status: enum accepted\|rejected`) |
| `autoloop.queue` | table: title, repo, priority, status, claimed_by | `queued_goal` | `prioritize` (`priority: int`), `retire` (`status: enum retired`) |
| `autoloop.repos` | table: title, verification, gates, criteria | `repo_policy` | `set_gates` (`gates: lines`), `set_criteria` (`criteria: text`) |
| `autoloop.limits` | table: title, value, min, max | `param` | `set_value` (`value: int`) |
| `autoloop.prompts` | table: title, text | `prompt` | `set_text` (`text: text`) |
| `autoloop.terms` | table: title, vocabulary, usage, count, hidden, description | `term` | `describe` (`description: text`), `hide` (`hidden: enum true\|false`) |
| `autoloop.kinds` | badges: object count per control sort | every control sort | — |

Edits set object fields only, so a term cannot be renamed in v1: its display name is the object's
`title`. Field limits match the loop's own validation (note and term description 400 characters,
criteria and prompt text 8000); `set_value` accepts the widest param range and the loop checks each
param's bounds.

Physics: `p_control` sends every control sort to `solvent` and every control morphism kind to `weak`.
Each physics morphism is total over its regime's sorts, so `elsewhere` maps too: to `solvent` and
`weak` in both regimes.

## `topos-autoloop-trace`

Sorting: identity on trace kinds; control kinds except `repo` sort to `elsewhere`.

Physics `p_trace`: `session` → `hydrophobic` (the assembling units), `goal`/`repo`/`gate` →
`hydrophilic` (anchors facing outward), `event` → `solvent`; `next`/`emitted`/`spawned` → `stiff`,
`in_repo`/`pursues` → `lateral`, `runs_gate` → `weak`. No morphism to `topos-agentic` is declared, so
no commuting square is required.

Sheaves: sessions table (title, goal state, agent, directory, continuations, max, heartbeats; only
props every session carries, since a missing field is an error), events table (time, session, class,
title), per-sort counts, assembly table and stage badges over `in_repo` + `pursues` (sessions on one
repo and goal assemble), physics table.

Verification: the `topos-autoloop-test` check builds both regimes from these specs and runs them over
fixtures written by the loop's own producers (`nix/data/autoloop-*`).

A geometric morphism between the two regimes is optional; if declared it must commute over the base.

## Host

`examples/autoloop` (web), separate from the static topos demo. It reads `GET /api/trace` and
`GET /api/site` same-origin, parses and glues them, and paints the active regime. A row of an
editable table opens its offered edits; writing one calls `revise` and `write_intent` and POSTs the
bytes to `/api/site`, then re-reads. Pure host logic is `examples/support/autoloop_host.rs`
(covered by `topos-autoloop-test`); section painting and the edit form are the shared
`examples/support/section_view.rs`. jump-cannon adopts the regimes once it consumes
`panel-kit-grammar`.
