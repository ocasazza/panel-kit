---
name: panel-kit
description: A unified tiling, floating, muxable window-manager shell — one near-black console, rendered on every platform.
# BEGIN GENERATED THEME TOKENS
colors:
  color-background: "#0a0a0a"
  color-surface: "#0d0d0d"
  color-text: "#ededed"
  color-text-muted: "#7a7a7a"
  color-divider: "#262626"
  color-border: "#5f5f5f"
  color-inverse-background: "#ededed"
  color-inverse-text: "#0a0a0a"
  color-focus-ring: "#ededed"
  color-primary: "#3b9bff"
  color-secondary: "#ff5fc3"
  color-tertiary: "#ffbd2e"
  color-highlight: "#5ef38c"
  color-success: "#27c93f"
  color-failure: "#ff5f56"
  color-error: "#ff5f56"
  color-warning: "#ffbd2e"
  color-info: "#83b7cc"
typography:
  title:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "0.8rem"
    fontWeight: 600
    lineHeight: 1.5
    letterSpacing: "0.06em"
  label:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "0.72rem"
    fontWeight: 700
    lineHeight: 1.5
    letterSpacing: "0.06em"
  body:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  caption:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "0.72rem"
    fontWeight: 400
    lineHeight: 1.3
    letterSpacing: "0.02em"
  micro:
    fontFamily: "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace"
    fontSize: "0.65rem"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
rounded:
  chip: "3px"
  panel: "0px"
  overlay: "5px"
  pill: "999px"
  dot: "50%"
spacing:
  hair: "2px"
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  xl: "16px"
# END GENERATED THEME TOKENS
components:
  panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.panel}"
    padding: "0"
    width: "min 180px"
    height: "min 110px"
  panel-body:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.body}"
    padding: "28px 8.8px 8.8px"
  panel-title:
    textColor: "{colors.text}"
    typography: "{typography.label}"
    padding: "0"
  light-mode:
    backgroundColor: "{colors.primary}"
    rounded: "{rounded.panel}"
    size: "12px"
  light-minimize:
    backgroundColor: "{colors.tertiary}"
    rounded: "{rounded.panel}"
    size: "12px"
  light-maximize:
    backgroundColor: "{colors.secondary}"
    rounded: "{rounded.panel}"
    size: "12px"
  dock-chip:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text}"
    rounded: "{rounded.chip}"
    typography: "{typography.caption}"
    padding: "3.2px 8px"
  dock-chip-hover:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text}"
    rounded: "{rounded.chip}"
    padding: "3.2px 8px"
  button:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text-muted}"
    rounded: "{rounded.panel}"
    padding: "1px 6px"
  button-hover:
    backgroundColor: "{colors.text}"
    textColor: "{colors.inverse-text}"
    rounded: "{rounded.panel}"
    padding: "1px 6px"
  button-pressed:
    backgroundColor: "{colors.background}"
    textColor: "{colors.highlight}"
    rounded: "{rounded.panel}"
    padding: "1px 6px"
  badge:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text}"
    rounded: "{rounded.pill}"
    typography: "{typography.caption}"
    padding: "4px 10px"
  badge-small:
    backgroundColor: "{colors.background}"
    textColor: "{colors.text}"
    rounded: "{rounded.pill}"
    typography: "{typography.caption}"
    padding: "2px 6px"
  badge-active:
    backgroundColor: "{colors.highlight}"
    textColor: "{colors.text}"
    rounded: "{rounded.pill}"
    padding: "4px 10px"
  spinner-label:
    textColor: "{colors.highlight}"
    typography: "{typography.caption}"
    padding: "0"
  tooltip-overlay:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.overlay}"
    typography: "{typography.caption}"
    padding: "8px 9.6px"
    width: "228px"
---

# Design System: panel-kit

## Overview

**Creative North Star: "The Lit Console"**

panel-kit is a near-black console in which the only saturated color is a lit
indicator. Everything structural is graphite — three stops of near-black for
surface, two hairline greys for seams, one bright grey for text, one mid grey
for de-emphasis. Against that, a small set of saturated roles exist and each
one *means* something: printer-CMY traffic lights that name a window
operation, a phosphor-green highlight that only ever says "this is live right
now," and status roles that report verdicts. The restraint is not minimalism
for its own sake; it is what makes a single lit pixel readable across a wall
of panels. Colour is addressed by **role**, never by hue: every colour is a
`--color-<role>` variable and any role can be assigned any value.

The system is a **window manager, not a page**. Its job is to render a unified
tileable, floatable, muxable workspace with the same experience and interface
on every platform it reaches — web and terminal today, iPad, Android, native
desktop, and embedded as stated direction. That ambition sets the hardest
constraint in this document: *the design language must be expressible in
cells.* A terminal has no shadows, no sub-pixel radii, no gradients, no
opacity. So the visual system is built from what every target can draw —
tonal layering, hairline borders, a fixed monospace grid, and color — and any
finish beyond that set is a web-only garnish, never load-bearing.

Density is the third pillar. This is an Operate surface: an operator arranges
panels around a long-running task and reads dense content inside them. Chrome
therefore yields to content. Panel controls and titles are inset *into* the
top border row rather than occupying a header band, following ratatui's
`Block::title` treatment, precisely to buy back vertical space. Nothing in
this system decorates; everything either labels, separates, or signals.

**Key Characteristics:**

- Near-black graphite base (`#0a0a0a` / `#0d0d0d`) with exactly one text role and one muted-text role.
- Monospace everywhere — no proportional font exists in the system.
- Saturated roles (primary, secondary, tertiary, highlight and the status roles), each semantically assigned; never used as decoration.
- Flat by rule: depth comes from tone and hairlines, not shadow.
- Chrome inset into borders; content gets the height.
- Every visual decision must survive translation into terminal cells.

## Colors

Colour is a set of **18 roles** under one `--color-*` CSS namespace. A role
names what a colour *does*; the value is whatever the theme assigns. Defaults
keep their hue names (Phosphor Green, Signal Blue, Amber, …) as shorthand for
the shipped value only. The palette is small enough to hold in your head and
every saturated entry is a signal, not a style.

**The role list, in canonical order:** `background`, `surface`, `text`,
`text-muted`, `divider`, `border`, `inverse-background`, `inverse-text`,
`focus-ring`, `primary`, `secondary`, `tertiary`, `highlight`, `success`,
`failure`, `error`, `warning`, `info`. The CSS variable is `--color-<role>`
(`--color-text-muted`, `--color-inverse-background`, …); the Rust name is
`ThemeColor::<Role>` / the snake_case field on `ColorTokens` (`text_muted`,
`inverse_background`); the spec JSON key is the same snake_case spelling.

**Every role is assignable.** Retheme by overriding `--color-<role>` on `:root`
in a stylesheet layered after `panel_kit::CSS`, or by supplying `ThemeTokens`
(which drives the terminal, the boot shell, and Monaco from the same values).
No two roles are required to match; where two default to the same value
(`failure`/`error`, `warning`/`tertiary`) they are still separate roles.

### Brand roles — The Control Cluster

Three roles that name the traffic-light cluster. They are deliberately **not**
macOS red/yellow/green, because none of panel-kit's lights destroys anything;
the cluster uses a printer-CMY scheme instead. The lights are `.light.mode`
(primary), `.light.min` (tertiary) and `.light.max` (secondary), and each sets
`--light-c` from its role.

- **Primary** `--color-primary`: the workspace-global floating ⇄ tiling mode light, and the boot progress fill. Default Signal Blue `#3b9bff` dark / `#1f5ec2` paper.
- **Secondary** `--color-secondary`: maximize / restore light. Default Magenta `#ff5fc3` dark / `#c21f8e` paper.
- **Tertiary** `--color-tertiary`: the third brand colour — minimize-to-dock light, and the entity and date badge tint. Default Amber `#ffbd2e` dark / `#b8860b` paper. Keeps its macOS minimize meaning, but it is *not* the warning status.

### Highlight — Live State

- **Highlight** `--color-highlight`: the live-system colour. It appears only where something is *happening or selected right now* — the spinner ring and label, the skeleton pulse, the progress fill, the topbar activity indicator, the active badge halo, and the dashed outline on a panel mid-reorder-drag. Default Phosphor Green `#5ef38c` dark / `#0c7a3d` paper. It is the brightest, most saturated value in the system, which is exactly why it must stay rare.

### Status roles

- **Success** `--color-success`: resolved/valid state and URL and status badges. Default Confirm Green `#27c93f` dark / `#1d7a33` paper. Distinct from highlight: this is a *verdict*, highlight is *activity*.
- **Failure** `--color-failure`: a failed check or failed verdict. New role; defaults to the same value as `error` (`#ff5f56` dark / `#c62828` paper) so the default look is unchanged.
- **Error** `--color-error`: errors, invalid input, critical gauge state, and unresolved references (`badge-wikilink.badge-unresolved`). Default Alert Red `#ff5f56` dark / `#c62828` paper.
- **Warning** `--color-warning`: warnings, alerts, and the 0.75 gauge threshold. Default Amber `#ffbd2e` dark / `#b8860b` paper — the same value as tertiary by default, but a status, not a brand colour.
- **Info** `--color-info`: informational metadata badges (doctype, author, tag, resolved wikilink), and the resting tint for tag badges — tags used to be highlight-colored, which spent a live-state signal on static metadata. Default Slate Info `#83b7cc` dark / `#2e6e8c` paper. Declared on `:root`, so web theming reaches it and the terminal mirror (`theme.rs`) stays in step.

Badge tones map onto roles: `ok` → success, `err` → error, `warn`/`alert` →
warning, `info` → info, `neutral` → text-muted. Badge kinds: tag, doctype,
author and resolved wikilink → info; folder → text-muted; entity and date →
tertiary; unresolved wikilink → error; url and status → success; generic →
border.

### Neutral roles

- **Background** `--color-background`: the page and the recessed field behind chips and badges. Default Console Black `#0a0a0a` dark / `#f4f1ea` paper.
- **Surface** `--color-surface`: every raised surface — panels, topbar, dock, tooltip. Default Panel Black `#0d0d0d` dark / `#fbf9f4` paper. Only two stops separate it from the page; that 3-value gap is the entire elevation system.
- **Text** `--color-text`: body and title text. Default Terminal White `#ededed` dark / `#1a1a1a` paper.
- **Text-muted** `--color-text-muted`: de-emphasized text — hints, dock labels, max-hints, folder badges. Default Graphite `#7a7a7a` dark / `#6e665c` paper.
- **Divider** `--color-divider`: structural hairlines that divide regions (topbar bottom, dock top). Default Seam `#262626` dark / `#d8d2c6` paper.
- **Border** `--color-border`: object outlines that bound a thing (panel border, chip border, scrollbar thumb, resize grip). Default Edge `#5f5f5f` dark / `#8c8578` paper. Chosen by measurement, not taste: the previous `#3a3a3a` computed **1.71:1** against `--color-surface`, below the 3:1 non-text threshold, which mattered doubly because the resize grip is drawn *only* in this color and so was invisible as a control. `#5f5f5f` computes **3.04:1** against `--color-surface` and **3.10:1** against `--color-background`.
- **Inverse-background** `--color-inverse-background`: the fill of an inverted pair (selection). Default `#ededed` dark / `#1a1a1a` paper.
- **Inverse-text** `--color-inverse-text`: the label on an inverted pair. Default `#0a0a0a` dark / `#f4f1ea` paper.
- **Focus-ring** `--color-focus-ring`: the keyboard focus ring and the traffic-light hover outline. Default `#ededed` dark / `#1a1a1a` paper.

### Theme-independent literals

Two literal families sit outside the role namespace on purpose.

- **Shadow ink** `--shadow-ink` (`#000`): the one theme-independent literal black. A shadow is darkness, not a theme colour, so it is declared outside `--color-*` and every shadow derives from it with `color-mix` (overlay lift `0 10px 30px` at 80%, drag lift at 46.67%, menu popups at 50%).
- **Tag slot palette** `--tag-0` … `--tag-7`: a literal categorical palette for tag hue slots (with `--tag-ink-N`); it is not role-assigned and stays a fixed palette.

### Named Rules

**The Lit Indicator Rule.** Saturation is reserved for meaning. The highlight
and the saturated brand and status roles together occupy ≤10% of any screen.
If a surface reads as green, something is wrong — not something is live.

**The Two Greys Rule.** There is one text grey (`--color-text`) and one muted
grey (`--color-text-muted`). A third intermediate grey is never introduced; if
text needs to sit between them, the answer is weight or case, not a new value.

**The Seam/Edge Rule.** `--color-divider` divides *regions*; `--color-border`
bounds *objects*. Never swap them — the two-step ramp is the only cue that says
"this is a thing" versus "this is a boundary between areas."

**The Role Rule.** Bind a use to the role it expresses, not to the hue it
happens to ship in. `failure` marks a failed check or verdict; `error` marks an
error or an unresolved reference. `warning` is status; `tertiary` is the third
brand colour. They share a default value and nothing else.

**The One Source Rule.** The editable palette, typography, and density defaults
live in `panel_kit_core::theme::ThemeTokens` and nowhere else. Every generated
consumer is mechanically derived from that source:

| Consumer | How it stays true |
|---|---|
| `assets/panel-kit.css` `:root` colour block | generated by `build.rs` from `ThemeTokens::dark()` (the 18 `--color-<role>` variables) and checked by the theme emitter test |
| `panel-kit-tui`'s `ResolvedTuiTheme` | converted from `ThemeTokens` with no backend-local preset literals |
| The pre-WASM boot stylesheet | generated by `build.rs` from `assets/panel-kit-boot.css.in`; its `{{placeholders}}` are the role keys (`{{background}}`, `{{text-muted}}`, `{{primary}}`, …) |
| Monaco's `panel-kit-dark` theme | built from token values plumbed in from Rust, not a JS table |

This rule was written after the failure it prevents. The palette had three
hand-maintained copies held together by discipline, and the discipline had
already failed: the boot shell carried `#09090a` where the token says
`#0a0a0a`, `#efeff0` for `#ededed`, `#242427` for `#262626`, `#77777d` for
`#7a7a7a`, `#389cf6` for `#3b9bff`, `#f6be3c` for `#ffbd2e`, and `#ee60c2`
for `#ff5fc3` — seven colours, every one wrong by an amount no reviewer would
catch by eye. Plus its own font stack.

Near-miss drift is the worst kind precisely because nothing looks broken. A
copy that is obviously wrong gets fixed on sight; a copy that is one hex digit
off just quietly stops being the same product. With four backends planned,
hand-copying does not scale past the copies that already existed, so the
values moved into the crate both renderers already depend on and a new
backend is now correct by construction.

## Typography

**Display Font:** none — the system has no display face by design.
**Body Font:** `ui-monospace, "SF Mono", "JetBrains Mono", "Menlo", "Consolas", monospace`
**Label/Mono Font:** the same stack. One family, no exceptions, including on `button`.

**Character:** Deliberately instrumental. A single monospace stack across every
role means the web surface shares a metric with the terminal surface, so the
same layout reasoning applies in both; it also means hierarchy has to be earned
through size, weight, case, and tracking rather than through contrast between
typefaces.

### Hierarchy

- **Title** (600, `.8rem`, tracking `.06em`): the topbar product name. The largest type in the system, and still under 11px at the 13px root.
- **Label** (700, `.72rem`, uppercase, tracking `.06em`): panel titles, inset into the border row. Uppercase plus heavy weight plus tracking is how a 9-10px string stays readable at panel scale.
- **Body** (400, `13px`, `1.5`): all panel content. The root size; everything else is a `rem` fraction of it.
- **Caption** (400, `.72rem`, tracking `.02em`): dock chips, badges, hints, spinner labels, tooltip text.
- **Micro** (400, `.65rem`): the maximize hint only. The floor of legibility; nothing smaller exists.

### Named Rules

**The Tracked-Caps Rule.** Any string set below `.75rem` that must be scanned
rather than read is uppercase with `.06em` tracking. Monospace at that size
loses word-shape; caps and tracking give it back.

**The One Family Rule.** No proportional font enters the system, and `button`
inherits `--mono` explicitly rather than falling back to the UA default. The
stack is a token (`tokens::MONO`), not a default, because it is a rule that
can be broken silently: the boot shell shipped `"Courier Prime", …` and the
embedded editor shipped Monaco's own stack until 1.0.0. Both now take the
canonical value, including the editor's portaled widgets.

## Layout

**The grid is the character cell.** Both renderers reason in a fixed monospace
advance; the web surface just happens to measure it in pixels. `TILE_ROW_PX`
carries that row quantum out of the web backend and into shared core math.

**Workspace shell.** A vertical flex column at exactly `100vh`: a 36px topbar
(`flex: 0 0 36px`), the workspace filling the remainder (`flex: 1; min-height: 0`),
and a 30px dock footer (`flex: 0 0 30px`). Both chrome bands are fixed-height
and hairline-separated from the field; the workspace is the only elastic region.

**Floating mode.** Panels are absolutely positioned with app-supplied geometry,
clamped so x/width/height never exceed the viewport while stored `y` is
preserved (floored at 0) so low-placed panels can extend below the fold and be
reached with workspace-level vertical scroll. Wheel events chain manually:
a hovered `.panel-body` that can still scroll in the wheel's direction absorbs
it; otherwise the workspace scrolls.

**Tiling mode.** A grid, not a wrapping flex line — because `tile_w` / `tile_h`
are *spans*, and only a grid can honour a span while still dividing a bounded
height among rows. Columns are `repeat(var(--tile-cols), minmax(0, 1fr))` and
rows are `grid-auto-rows: minmax(var(--tile-row-min), 1fr)`, with `8px` gap and
`8px` padding. A panel emits `grid-column: span <tile_w>` and
`grid-row: span <tile_h>`; the row height comes from the grid, never from the
panel, so a tile is free to be shorter than its content and `.panel-body`
scrolls instead of growing the row. The workspace fills exactly when the tiles
fit and scrolls once `--tile-row-min` is reached. Tiles stay
`position: relative` so the resize grip can anchor to them.

`--tile-cols` is **not** set per tier in CSS. It is written inline on `.ws`
from `SurfaceProfile::tile_columns()` — 1 compact, 2 tablet, `TILE_W_MAX`
regular — and the renderer clamps each emitted span to it. Column count is
surface policy, so core owns it and no stylesheet re-tabulates the tiers. A
span wider than the grid is meaningless, and a DOM grid answers one by
silently manufacturing zero-width implicit columns that swallow the panel.

**Spacing rhythm.** An informal 2/4/6/8/12/16 progression: `2px` badge internal
gap, `4px`/`6px` badge padding and light gap, `8px` tiling gap and padding,
`.55rem` (≈7px) panel-body inset, `.75rem` dock padding, `1rem` topbar padding.
`.panel-body` carries an asymmetric `1.75rem` top inset — that value exists to
clear the border-inset title row, and it is the one magic number in the layout.

**Density and responsive behavior.** Three tiers, not one binary. The surface
is classified in core by `SurfaceProfile::from_logical_width` — **compact**
below 760, **tablet** below 1180, **regular** above — and the root element
carries exactly one of `compact` / `tablet` / `regular`. Classification happens
in Rust and is re-evaluated on every resize rather than in a media query,
because the shell changes *behavior* and not just style; the stylesheet
contains zero `@media` queries. The same policy drives the terminal from cell
counts (`CELLS_COMPACT_MAX` / `CELLS_TABLET_MAX`), so both renderers agree on
what a cramped surface is.

- **Compact** withholds *geometry manipulation*: one column, spans collapse, a
  `200px` row floor, the topbar wraps, and the affordances that move or resize
  a panel by hand (header drag, corner grip, grab cursor) are gone. A phone
  stack is a list. It does **not** withhold the means to leave a window state
  — see the Escape Hatch Rule below.
- **Tablet** keeps the *complete* window-management surface — lights, resize
  grip, reorder, mode toggle — in two columns. This is the tier that makes
  iPad a real target rather than a degraded phone.
- **Regular** is the full four-column grid.

**Sizing follows the pointer, not the width.** `--hit-min` is `24px` by
default and `44px` under `.ws-root.coarse`, which the renderer emits from the
detected `SurfaceCapabilities::coarse_pointer`
(`matchMedia('(pointer: coarse)')`). Keying it to the tier got both ends
backwards: a 1400px kiosk touchscreen is a `regular` surface that still needs
44px, and a 900px desktop window driven by a mouse does not. Width describes
how much room there is; only the pointer describes how precisely it can be
hit. One token still moves every control together.

**The Escape Hatch Rule.** A tier may withhold the means to *enter* a window
state. It must never withhold the means to *leave* one. `SavedLayoutV2`
carries `WinState` across surfaces by design, so a panel maximized on a
regular surface arrives maximized on a compact one; if compact also hid
restore, the operator would be stranded on a single full-bleed panel with
every sibling unreachable and nothing to tap. Compact therefore renders the
magenta restore light — and only that one — for a maximized panel
(`SurfaceProfile::must_offer_restore`). Minimized panels already escape
through the dock, so they need no override.

## Elevation & Depth

**Flat by rule, parity-first.** Depth is expressed with tonal layering and
hairline borders, because those are the only two depth cues a terminal cell
grid can draw. A raised surface is `--color-surface` on `--color-background`
with a `--color-border` edge; a divided region is a `--color-divider` hairline.
That is the whole system, and it translates to cells without loss.

Shadow is a **web-only garnish reserved for genuinely floating things** — an
element that has left the document plane and is temporarily above everything
else. A tooltip overlay qualifies. A panel sitting at rest in a workspace does
not.

### Shadow Vocabulary

- **Overlay lift** (`box-shadow: 0 10px 30px` at 80% `--shadow-ink`): the fixed tooltip overlay only. It is genuinely detached and viewport-placed. `--shadow-ink` is the one theme-independent literal black, declared outside the colour namespace and mixed with `color-mix`.
- **Drag lift** (`box-shadow: 0 6px 24px` at 46.67% `--shadow-ink`): a panel genuinely lifted mid-drag (`.ws.floating .panel.dragging`).

Note that two further `box-shadow` declarations exist but are **rings, not elevation** — `0 0 0 2px` spreads used for badge hover/active halos. They borrow the shadow channel to draw a state ring. That is legitimate output but mis-housed: rings belong to a `--color-focus-ring` / state role, so that removing elevation never removes state feedback.

### Named Rules

**The Cell-Expressible Rule.** No depth cue may be load-bearing unless a
terminal can draw it. If removing shadows from the web surface makes z-order
illegible, the fix is a tonal or border treatment that both renderers share —
not a bigger shadow.

**Why this is the whole vocabulary.** The resting panel shadow
(`box-shadow: 0 6px 24px #0007`) was removed in 1.0.0: it shadowed every panel
whether floating, tiled, or docked, and the terminal renderer had no
counterpart for any of it. Z-order legibility now rides the tonal/border
channel both renderers can draw, and a shadow appears only on
`.ws.floating .panel.dragging` — a panel genuinely lifted mid-drag.

Two `0 0 0 2px` declarations remain in the shadow channel but are **rings,
not elevation**: badge hover/active. They draw state, so removing elevation
never removes feedback.

## Shapes

**Square corners, consistent stroke, one pill.** Every bounded object is a
1px-stroked rectangle at `--panel-radius` (`0`): panels, buttons, dock chips,
the tooltip overlay, chip-style badges, and the traffic lights, which are
square lit cells rather than circles. A square corner is what a terminal cell
draws, so web and terminal chrome are the same shape.

One shape breaks the rectangle: free-standing badges are full pills
(`border-radius: 999px`, which at their height is exactly a stadium), inherited
from the egui badge they port. Inside the data table they take the square chip
style. The spinner's rotating ring stays circular because it moves.
Borders do the work that fills would do elsewhere: a badge's *kind* is carried
by its border color with a near-black fill behind it, so ten badge kinds
coexist without ten colored blocks fighting for attention. The resize grip is
not a border at all but a 14px corner triangle drawn with a
`linear-gradient(135deg, transparent 50%, var(--color-border) 50%)` — the same
edge grey, folded.

## Components

### Panels

- **Character:** an instrument module bolted into a rack.
- **Shape:** square corners (`--panel-radius`), `1px` `--color-border` border, `--color-surface` fill, `overflow: hidden`, floors at `180x110px`.
- **Chrome:** controls and title are inset into the top border row, not a header band. The header's content starts on the body's content column (`--space-md` in); its `--space-xs` padding sits outside that column, where the top border stops. The body compensates with a `1.75rem` top inset.
- **States:** `.tile-dragging` drops to `.55` opacity with a `1px` dashed highlight outline inset `-1px` — the highlight's live-state role applied to drag.
- **Cursor:** `grab` on a tiling panel head, `grabbing` while active; `default` on mobile.

### Traffic Lights

- **Character:** lit indicators on a dark faceplate.
- **Shape:** `12px` squares at `--panel-radius`, `6px` apart, no border, fully opaque. The terminal draws `■`.
- **Color:** per-operation via `--light-c` — `.light.mode` primary (blue by default), `.light.min` tertiary (amber), `.light.max` secondary (magenta). The minimize class was `.light.yellow` before the role rename.
- **Hover:** a `1px` `--color-focus-ring` outline `1px` out; the terminal draws `▣`. **Focus:** the shared `2px` keyboard ring every control takes.
- **Never:** red/yellow/green macOS mimicry. None of these lights destroys anything.

### Dock Chips

- **Character:** a minimized panel's parked nameplate.
- **Shape:** `3px` radius, `1px` `--color-border` border, `--color-background` fill (recessed below the dock's `--color-surface`), `.2rem .5rem` padding, `nowrap`.
- **Hover:** border goes to `--color-text`. The fill never changes.
- **Empty state:** `.dock-empty` in `--color-text-muted` (4.53:1). It was the border role — a border token used as 9.1px text at 1.71:1 — until 1.0.0. The Seam/Edge Rule forbids structural tokens as text.

### Buttons

- **Character:** a content-area control — a toolbar action or toggle. Panel-header actions inset into the border row are `PanelHeaderButton` instead.
- **Component:** `panel_kit::widgets::Button` (`.pk-btn`); `compact` for controls inside a row or menu. A `<summary>` that opens a menu takes the same class.
- **Shape:** square, `1px` `--color-border` border, `--color-background` fill, `--color-text-muted` caps label at `11px` with `.06em` tracking.
- **Hover:** inverts — `--color-text` fill, `--color-inverse-text` label.
- **Pressed** (`aria-pressed="true"`): highlight border and label; a latched toggle is selection state. Disabled drops the border to `--color-divider`.

### Badges

- **Character:** a metadata chip that is also a filter control.
- **Shape:** stadium pill, `1px` border, `4px 10px` padding, label clamped to `28ch` with ellipsis and the full value in `title`.
- **Color model:** per-kind classes set `--badge-c` (border) and `--badge-fg` (text) over a `--color-background` fill; inline style overrides those plus `--badge-bg` for community tints.
- **Hover** (inactive only): fill drifts 18% toward the border color, plus a 2px 20% halo of the same.
- **Active:** 2px 40% highlight halo and a highlight-tinted fill, unless a community tint owns the fill.
- **Affordances:** trailing `+` / `×` buttons at 70% of the label color, going to full `--color-text` on hover.
- **Inside the data table** the badge takes its square chip style (`BadgeSpec::with_tone`, outline): cell values like a row's state, and applied filters with their `×`. The grid is square-edged, and an applied filter is query state, not live state, so neither earns the pill or the highlight halo. Tags tone `info`, other filters `neutral`.

### Spinner

- **Character:** the smallest possible "working."
- **Shape:** `12px` ring, `2px` `--color-border` border with a highlight top edge, rotating `.7s linear infinite`.
- **Label:** optional, highlight-colored caption; the span is omitted entirely when empty.

### Tooltip Overlay

- **Shape:** fixed, `228px` wide, `5px` radius, `1px` `--color-border` border, `var(--color-surface)` fill, the system's only true elevation shadow. The fill was a literal `#0d0d0d` until 1.0.0 — identical in the default theme, but it stopped following any retheme, so the overlay stayed near-black over a light workspace.
- **Placement:** viewport-aware and computed in Rust (`tip_pos`) — prefer left of the cursor, flip right without room, clamp inside the window. CSS anchor positioning would do this natively but WebKit does not support it yet.

### Content States

- **Empty** (`.empty`): centered `--color-text-muted` caption at `.78rem`, tracking `.03em`.
- **Loading** (`.skeleton`): the same box in highlight, pulsing `1.1s`.

### Progress (`.pk-progress`)

- **Character:** the honest report. A bar beats a spinner for anything measurable; the spinner is for tiny inline waits only.
- **Shape:** `.55rem` track, `3px` radius, `1px` `--color-border` border, `var(--color-background)` fill recessed below the surface it sits on.
- **Fill:** `--color-highlight` — loading is live state, so this is the highlight doing its actual job.
- **Honesty rule:** a determinate bar *always* shows its percentage; `None` stays indeterminate rather than inventing a number. The track was `rgba(255,255,255,.06)` until 1.0.0, which is a translucent surface and therefore unexpressible in cells; `var(--color-background)` reads identically and survives translation.
- **Reduced motion:** the indeterminate fill goes static at `40%` — not `100%`, which would claim completion, and not faded, which would reintroduce translucency.

### Embedded editor viewport

A third-party code editor is the one component panel-kit does not draw itself,
which makes it the sharpest test of whether the world is real. It is not
granted an exception: its theme is built from `tokens::DARK` values plumbed in
from Rust, so the surface is `--color-surface`, the text is `--color-text`,
guides and rulers are `--color-divider`/`--color-border`, syntax is drawn from
the existing palette (`--color-info`, `--color-success`, `--color-warning`,
`--color-error`, `--color-text-muted`), and it renders in `tokens::MONO` at the
console's own `13px`. The highlight appears on the cursor and on active/focused
matches — live state — and never on ordinary syntax.

The editor is also why pointer capture exists: a panel drag that crosses the
editor would otherwise die the moment the pointer entered it.

### The pre-WASM boot shell

The one surface that cannot use this design system at runtime, because it
paints before the WASM module exists and so cannot read the injected
stylesheet. It is still held to the system — by generation rather than by
discipline (see The One Source Rule); its `{{placeholders}}` are the role keys
(`{{background}}`, `{{text}}`, `{{primary}}`, …). It previews the real chrome:
the panel skeleton, the hairline borders, and the traffic-light cluster as
three square cells, as the mounted app and the terminal draw them.

## Do's and Don'ts

### Do:

- **Do** put anything both renderers must agree on in `panel-kit-core` first, then let each backend translate it. "Web and terminal both happen to do this" is not the same as "core owns it."
- **Do** keep every visual decision expressible in terminal cells: tone, hairline, monospace cell, color. Check a new treatment against the cell grid before shipping it.
- **Do** reserve `--color-highlight` for live state — activity, loading, selection, drag-in-flight — and keep it under ~10% of any screen.
- **Do** use `--color-divider` for region dividers and `--color-border` for object edges, never interchangeably.
- **Do** carry hierarchy with size, weight, case, and tracking, since there is only one font family.
- **Do** inset panel chrome into the border row and give reclaimed height to content.
- **Do** express depth as `--color-surface` on `--color-background` plus a `--color-border` edge.
- **Do** theme by overriding `--color-<role>` on `:root` in a stylesheet layered after `panel_kit::CSS` (or by supplying `ThemeTokens`), and mirror the change in `Theme` so the terminal stays the same product.
- **Do** bind a colour to the role it expresses: `failure` for a failed check, `error` for an error or unresolved reference, `warning` for a warning, `tertiary` for the third brand colour.

### Don't:

- **Don't** put `box-shadow` on a resting surface. Shadow belongs only to things that have genuinely left the plane, like the tooltip overlay.
- **Don't** use the highlight on static chrome — borders, titles, dividers, or decoration. A green that is always on cannot signal.
- **Don't** introduce a third grey between `--color-text` and `--color-text-muted`.
- **Don't** give the traffic lights macOS red/yellow/green semantics; none of them destroys anything, and the CMY assignment is the mapping.
- **Don't** add a proportional typeface, a gradient, a blur, or a translucent surface. None of them survives a cell grid.
- **Don't** set scannable text below `.65rem`, and don't set it at `.72rem` or below without caps and tracking.
- **Don't** hardcode a color, radius, or font stack that a `:root` token already names. The only literal colours are `--shadow-ink` and the `--tag-0..7` slot palette.
- **Don't** edit the generated boot stylesheet or a derived `Theme` field to change a colour; change the role and let both follow. Editing a derivative is how the three copies drifted in the first place.
- **Don't** solve a layout problem with a media query when the shell needs to change behavior — the breakpoint is evaluated in Rust so behavior and style can change together.
- **Don't** hand-copy a palette into a fourth place — a boot stylesheet, a JavaScript theme table, a native backend. Derive it from `panel_kit_core::tokens` or pin it with a test; those are the only two options.
- **Don't** grant an embedded third-party surface an exemption from the world. Theme it from the tokens; a foreign widget parked inside the console is more jarring than anything it saves.
- **Don't** express progress you have not measured. A determinate bar without a percentage, or a fabricated percentage, is worse than an honest indeterminate one.
