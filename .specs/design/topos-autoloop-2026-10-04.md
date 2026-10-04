# The omp auto-loop regimes

Status: agreed with the panel-kit-grammar owner; authored as Nix specs after phase 2A
(`nix/specs/topos-autoloop*.nix`, `nix/grammars/autoloop-*.nix`). Domain words live only in those
specs, never in crate types.

## Sites, and how they become one

| Site | Grammar | Objects (site kinds) | Morphisms (site kinds) |
|---|---|---|---|
| trace | `autoloop-trace` (pest; the line format of jump-cannon's `omp-auto-loop.toml`) | `session`, `event`, `goal`, `repo`, `gate` | `in_repo`, `pursues`, `runs_gate`, `spawned`, `emitted`, `next` |
| control | `autoloop-control` (json, multi-collection; the loop's `GET /api/site`, written back through its emitter to `POST /api/site`) | `claim`, `proposal`, `queued_goal`, `repo`, `term`, `param`, `prompt` | `targets` (queued_goal → repo), `revises` (proposal → param or prompt) |

A `GeometricMorphism` relates topoi over the same site, so the two sites are first **glued** along
`repo`: the pushout `trace ← R → control`, where `R` is the discrete site of the repo objects both
present, identified by id. The loop's control site reuses the trace's repo ids for every repo the
trace knows, so the identification is by equal ids. Each grammar parses its own input; the host
glues; both regimes are over the glued site.

Crate addition (this work): `Site::glue(&a, &b, along_kind: &str) -> Result<Site, SiteError>` —
pure; objects of `along_kind` with equal ids are identified (fields must agree; a disagreement is an
error); any other id collision is an error. Tests: glue is associative on the demo; restricting the
glued site to each side's kinds returns that side.

## `topos-autoloop-control`

Sorting: identity on control kinds; every trace kind except `repo` sorts to `elsewhere`, which has no
sheaves. Stalks: labels per sort; `claim` badge from `reason`; `repo` badge from its verification tag
(`gated` / `judge-checked` / `unverifiable`).

Sheaves (each `restrict`ed to one sort):

| Binding | Kind | `restrict` | Edits |
|---|---|---|---|
| `autoloop.claims` | table: title, session, reason, judge, decision | `claim` | `ratify` (`decision: enum accepted\|overturned`, `note: text`) |
| `autoloop.proposals` | table: title, before, after, rationale, status | `proposal` | `decide` (`status: enum accepted\|rejected`) |
| `autoloop.queue` | table: title, repo, priority, status, claimed_by | `queued_goal` | `prioritize` (`priority: int`), `retire` (`status: enum retired`) |
| `autoloop.repos` | table: title, verification, gates, criteria | `repo` | `set_gates` (`gates: lines`), `set_criteria` (`criteria: text`) |
| `autoloop.limits` | table: title, value, min, max | `param` | `set_value` (`value: int`) |
| `autoloop.prompts` | text per object | `prompt` | `set_text` (`text: text`) |
| `autoloop.terms` | table: title, vocabulary, usage, count, hidden | `term` | `relabel` (`title: text`, `description: text`), `hide` (`hidden: enum true\|false`) |
| `autoloop.verdicts` | badges by claim `decision` | `claim` | — |

Physics: `p_control` sends every control sort to `solvent` and both control morphism kinds to `weak`.

## `topos-autoloop-trace`

Sorting: identity on trace kinds; control kinds except `repo` sort to `elsewhere`.

Physics `p_trace`: `session` → `hydrophobic` (the assembling units), `goal`/`repo`/`gate` →
`hydrophilic` (anchors facing outward), `event` → `solvent`; `next`/`emitted`/`spawned` → `stiff`,
`in_repo`/`pursues` → `lateral`, `runs_gate` → `weak`. No morphism to `topos-agentic` is declared, so
no commuting square is required.

Sheaves: Sessions table (cwd, continuations / max, heartbeats, model), event stream, assembly table
and stage badges over `in_repo` + `pursues` (sessions on one repo and goal assemble), physics table.

A geometric morphism between the two regimes is optional; if declared it must commute over the base.

## Host

The panel-kit topos example host, extended with the write path: it parses both inputs, glues them,
and on an edit calls `revise`, `emit`, and POSTs the emitted bytes to the route bound to
`autoloop-control`, then re-reads. jump-cannon adopts the regimes once it consumes
`panel-kit-grammar`.
