# [1.0.0](https://github.com/ocasazza/panel-kit/compare/v0.3.0...v1.0.0) (2026-09-14)


* feat(core)!: unify web and terminal on one surface, layout, and token contract ([b5609e7](https://github.com/ocasazza/panel-kit/commit/b5609e78ae7ab4fadc2796e9626d7381dd470919)), closes [#09090a](https://github.com/ocasazza/panel-kit/issues/09090a) [#0a0a0a](https://github.com/ocasazza/panel-kit/issues/0a0a0a) [#efeff0](https://github.com/ocasazza/panel-kit/issues/efeff0) [#ededed](https://github.com/ocasazza/panel-kit/issues/ededed) [#242427](https://github.com/ocasazza/panel-kit/issues/242427) [#262626](https://github.com/ocasazza/panel-kit/issues/262626) [#77777d](https://github.com/ocasazza/panel-kit/issues/77777d) [#7a7a7a](https://github.com/ocasazza/panel-kit/issues/7a7a7a) [#389cf6](https://github.com/ocasazza/panel-kit/issues/389cf6) [#3b9bff](https://github.com/ocasazza/panel-kit/issues/3b9bff) [#f6be3c](https://github.com/ocasazza/panel-kit/issues/f6be3c) [#ffbd2e](https://github.com/ocasazza/panel-kit/issues/ffbd2e) [#ee60c2](https://github.com/ocasazza/panel-kit/issues/ee60c2) [#ff5fc3](https://github.com/ocasazza/panel-kit/issues/ff5fc3) [#3a3a3a](https://github.com/ocasazza/panel-kit/issues/3a3a3a) [#5f5f5f](https://github.com/ocasazza/panel-kit/issues/5f5f5f)


### Bug Fixes

* **editor:** dispose change subscription before editor/model teardown ([a221a1c](https://github.com/ocasazza/panel-kit/commit/a221a1cd45aaefd99ab0a8d49948220c65808d3a))
* **editor:** load monaco editor worker as module worker (bundle is ESM) ([7a19d9d](https://github.com/ocasazza/panel-kit/commit/7a19d9d2e76770df814ec38479d27221e15e58ce))
* **editor:** resolve monaco bundle URL against document.baseURI for dynamic import ([4029ac9](https://github.com/ocasazza/panel-kit/commit/4029ac9dc82c907f017df20329699469ee859855))
* **editor:** tolerate signal borrow conflict at mount via try_peek fallback ([d666c15](https://github.com/ocasazza/panel-kit/commit/d666c15aa523a3d56cbd6589de8d7babb53e6271))
* **flake:** include build.rs in the crane source fileset ([570f945](https://github.com/ocasazza/panel-kit/commit/570f9457cba0f1399d962fa730efa6c074e8fe9f))
* isolate panel header controls from dragging ([a38596a](https://github.com/ocasazza/panel-kit/commit/a38596a579216bdcb28ad03b4f4aadc2b9633958))
* keep static boot shell outside mount root ([b4974e6](https://github.com/ocasazza/panel-kit/commit/b4974e665e3f2110ed3e2f6736d146fd70414121))
* **loading:** root-scope store signals (browser ValueDroppedError) ([2f5b18b](https://github.com/ocasazza/panel-kit/commit/2f5b18bf030fccca8fb92b6617d92233cfe12b79))
* satisfy clippy 1.96 on core and the views example ([d444651](https://github.com/ocasazza/panel-kit/commit/d444651c31bd6269ef5f5f0be2145718ff04240c))
* **tui:** drop clone of Copy KeyCode so the release aggregate goes green ([14cf11f](https://github.com/ocasazza/panel-kit/commit/14cf11fe2eb894c66f565ccc89aa846416376d7b))


### Features

* add panel header actions ([f0b0388](https://github.com/ocasazza/panel-kit/commit/f0b0388b789abc2c24586722ef2233ba56a9e9fb))
* add workspace boot shell ([6f633b8](https://github.com/ocasazza/panel-kit/commit/6f633b8b33340fc4a253812e3585713caa7479de))
* Monaco editor component with pest grammar language ([c938642](https://github.com/ocasazza/panel-kit/commit/c938642b175ad6bb6616e9ae5082bad8d6b80985))
* named workspace views (use_views) with per-view layout persistence ([#6](https://github.com/ocasazza/panel-kit/issues/6)) ([2524fec](https://github.com/ocasazza/panel-kit/commit/2524fec6e731c6d41d1569058becaccd3c1b2adf))
* store-shaped async hydration with mandatory-percentage progress bars ([930c067](https://github.com/ocasazza/panel-kit/commit/930c067de8ba5a00a5b24823730317baf0b0f74d))


### BREAKING CHANGES

* consumers pinned to 0.3.x keep working on their current
revision and must migrate before moving the pin. MIGRATION.md documents every
change below with before/after edits and ends with the consumer notice.

* viewport_is_mobile() and Workspace::is_mobile are removed in favour of core
  SurfaceProfile with compact, tablet and regular tiers, and effective_mode.
* Workspace mouse entry points are renamed to begin_pointer_drag,
  begin_pointer_tile_resize, handle_pointer_move and handle_pointer_up, and
  consume pointer events; the mouse-named methods are gone with no aliases.
* Workspace gains handle_key, focused and surface_profile.
* The root class `mobile` is replaced by exactly one of compact, tablet or
  regular, plus `coarse` when the pointer is imprecise. Consumer CSS keyed on
  `.mobile`, or keyed on a tier for control sizing, must be rewritten to use
  var(--hit-min) or `.coarse`.
* Consumer CSS that hides .lights or .resize by tier must be deleted; it now
  suppresses the compact restore control.
* Tiled panels receive inline grid spans instead of flex and height, and
  .ws.tiling is a grid. Overrides that assumed flex have no effect.
* Persistence writes SavedLayoutV2 and reads the legacy V1 shape through
  StoredLayout with automatic migration, including per-view records.
* assets/panel-kit-boot.css is deleted and generated into OUT_DIR from
  assets/panel-kit-boot.css.in; patch the template, never the output.
* Colours must come from panel_kit_core::tokens rather than copied literals.
* Nix mkLayout takes { units, viewport, mode ? "Floating", panels } and emits
  V2; the `tiling` boolean is gone.
* panel-kit-tui removes the TuiMouseButton, TuiMouseEventKind and
  TuiMouseEvent aliases and its local ROW_CELLS, adds LayoutStore, and its
  widget colour parameters take panel_kit_core::badge::Rgb.
* TileMetrics::CELLS.row changes from 6.0 to 4.0 and is authoritative.

# [0.3.0](https://github.com/ocasazza/panel-kit/compare/v0.2.0...v0.3.0) (2026-07-28)


### Bug Fixes

* clear clippy warnings in web crate; correct example help text ([d292085](https://github.com/ocasazza/panel-kit/commit/d2920852c248a17876ac0a5aff43d8b6256df0a4))
* drop redundant deref in apply_drag call ([1709dc0](https://github.com/ocasazza/panel-kit/commit/1709dc0f32fa6ef160507da176c55daa495724f8))
* reliable resize in webviews + proportional floating-panel growth ([554180b](https://github.com/ocasazza/panel-kit/commit/554180b32d2ca885a432a63f9e8700052f6672c3))
* **tui:** empty capacity gauge reads empty ([c8df356](https://github.com/ocasazza/panel-kit/commit/c8df35647887b53d0bae32e9792248751ad94352))
* **tui:** restore green checks for panel-kit-tui ([1a0e4d6](https://github.com/ocasazza/panel-kit/commit/1a0e4d6cd75c21a22f446feaf963438cb3307850))
* **tui:** restore Unicode chrome by default, ASCII only for WebGL ([1236c5a](https://github.com/ocasazza/panel-kit/commit/1236c5a750ce9e71356c8e94f3905cc01c2d0591)), closes [#1](https://github.com/ocasazza/panel-kit/issues/1)


### Features

* **examples:** add Nodes table panel showcasing table/status/meter ([6784379](https://github.com/ocasazza/panel-kit/commit/67843794164434cf868062425730a22b9e7b9593))
* global vertical scrolling, flame/boxplot realtime charts, Nix layout DSL ([932f410](https://github.com/ocasazza/panel-kit/commit/932f410c77e8d8ef602f480917b568086aea48d7))
* hover glyphs on traffic lights, blue mode toggle, drag selection guard ([2e80295](https://github.com/ocasazza/panel-kit/commit/2e8029521d8802627a13e9b6832e759e3738053f))
* **hydra:** add hydraJobs, release aggregate, and semantic-release config ([0abae56](https://github.com/ocasazza/panel-kit/commit/0abae565b7dd5fdb96bf83ff7ca65504d2417415))
* printer-CMY lights — maximize goes pink, glyph tracks panel state ([c0ec98d](https://github.com/ocasazza/panel-kit/commit/c0ec98d5216aa90eb4741b728254c5cb46383004))
* **tui:** scrollable panel content helper ([27ffd8b](https://github.com/ocasazza/panel-kit/commit/27ffd8b6e6a12dc4a3685cdb68256c0544491040))
* **tui:** upstream table, meter, status, and text-wrap components ([4160e43](https://github.com/ocasazza/panel-kit/commit/4160e430ca8d34b5cf1f8bd37081b5f563032461))
