# [2.0.0](https://github.com/ocasazza/panel-kit/compare/v1.1.1...v2.0.0) (2026-11-21)


* feat(core)!: tiling drag lifts, follows, and swaps tiles on every backend ([ab3fd80](https://github.com/ocasazza/panel-kit/commit/ab3fd80093f50cf5a9a0a5f994a249a8d949c3ff))


### Bug Fixes

* avoid is_multiple_of to keep the wasm consumers' 1.85 toolchain working ([8e4de76](https://github.com/ocasazza/panel-kit/commit/8e4de76ff1092d5919793adde1fe2554c51b42f2))
* **bevy:** lay repainted panel bodies out in the frame they are painted ([1ce8806](https://github.com/ocasazza/panel-kit/commit/1ce880643a66e2e97482f742af9377b9da5d80f2))
* **build:** exclude the patched beamterm-renderer from the workspace ([6c96943](https://github.com/ocasazza/panel-kit/commit/6c9694341c4cbfa607e320c85e85efd7786c6afe))
* **chrome:** square traffic lights and terminal borders; header content on the body's column ([7177b9f](https://github.com/ocasazza/panel-kit/commit/7177b9fff1fb96c8134cd5ead3e14d913917348a))
* **chrome:** the light cluster hugs the frame, the title keeps the text column ([0df08a9](https://github.com/ocasazza/panel-kit/commit/0df08a93d265978dc3df61139d8def3e15894492))
* **clippy:** clear the -D warnings backlog (auto-deref, Copy clone, defaults) ([9cfd693](https://github.com/ocasazza/panel-kit/commit/9cfd69316da6532690e472b1f1e52fe7a222e904))
* **core:** clear the clippy lints that fail checks.clippy ([2e8fafa](https://github.com/ocasazza/panel-kit/commit/2e8fafa382be5dc9f2e4077669ff6c2bb457f863))
* **core:** silence clippy too_many_arguments on the pointer seams ([800f616](https://github.com/ocasazza/panel-kit/commit/800f6162e8cb6c2a2b234403fe3eb030c4e0dd08))
* **css:** resize grip z-index above embedded BevyCanvas layers ([1426e25](https://github.com/ocasazza/panel-kit/commit/1426e25f73712d6d70afc7e018b02ebb306a3614))
* **css:** the gate reads declarations, and the stylesheet uses the role tokens ([ab8fdf9](https://github.com/ocasazza/panel-kit/commit/ab8fdf92227dc1c0bde8fe47f6e3a4ee58693d75))
* **css:** transparent dock toggle bg + right-justify toggles ([9bc1fbe](https://github.com/ocasazza/panel-kit/commit/9bc1fbef706e2d786a3818a2ed0514e94c7c4278))
* **data-table:** draw the group select's chevron in CSS ([da8fd7f](https://github.com/ocasazza/panel-kit/commit/da8fd7f487c163b21b7fdff9e224387696510538))
* **data:** add parked_blocked term and parkAfter param to the control fixture ([42ebdf7](https://github.com/ocasazza/panel-kit/commit/42ebdf7ada797cb3bfaabf04528f78c411a94a63))
* **dock:** move minimized chips before snap toggles ([ec685ac](https://github.com/ocasazza/panel-kit/commit/ec685ac6d59ed1937b1d5eb8c71be5610c73924a))
* **docs:** intra-doc links the rustdoc gate rejects ([ae3569a](https://github.com/ocasazza/panel-kit/commit/ae3569a58098df5b3bfa2ea99b58d52fe5622b10))
* **docs:** plain-code the private references in vendored module docs ([8aad712](https://github.com/ocasazza/panel-kit/commit/8aad7126f5eb2eb2d43b176b21664a46f83f362f))
* **docs:** resolve the render_keepalive intra-doc link ([922036d](https://github.com/ocasazza/panel-kit/commit/922036d4712c08fa7a9d7f408dc99ce43e1ea9af))
* **dropdown:** give triggers a visible affordance ([98486c6](https://github.com/ocasazza/panel-kit/commit/98486c61db07f98f0f45546b992f911bfb9d9947))
* **dropdown:** opaque sticky group label background ([08a0521](https://github.com/ocasazza/panel-kit/commit/08a05215bfd6ac4cf7b3f018a99348354f67c67a))
* **editor:** a read-only editor follows its bound value ([36d4588](https://github.com/ocasazza/panel-kit/commit/36d45887af43ecf13dd03259d1a8b7a83308f9a0))
* **editor:** the theme reads canonical token names, and grammar errors surface ([58749d9](https://github.com/ocasazza/panel-kit/commit/58749d9e8debfd28151df8e29abfaa931ee69789))
* **frame:** stop one tile resize from resizing its neighbours ([7517e67](https://github.com/ocasazza/panel-kit/commit/7517e673bc095794ff4a3905a9dcd0268e259a86))
* **grafana:** drop loading=lazy, it strands the new spinner ([#3](https://github.com/ocasazza/panel-kit/issues/3)) ([b3ceb75](https://github.com/ocasazza/panel-kit/commit/b3ceb75e6479276f6ce7d3270859a26558b86854))
* **grammar:** emit re-parse guard, glue duplicate refusal, legacy sessions ([f5d5425](https://github.com/ocasazza/panel-kit/commit/f5d5425d13a458aaf3299cbf71d86b09eec63830))
* **grammar:** table row ids are unique and name their morphism ([5cdd324](https://github.com/ocasazza/panel-kit/commit/5cdd3244b94a8d066f0ddd879c325c16a191dec4))
* mobile layout improvements + reset button ([bb51837](https://github.com/ocasazza/panel-kit/commit/bb518374d8cdc79bbca2405a21bc4c1848b2e71b))
* **nix:** fetch the OKF corpus at eval time; lint okf with its env ([b3eaced](https://github.com/ocasazza/panel-kit/commit/b3eaced10d78755ce5d9d3cae1f6d57487a04180))
* **nix:** give theme-parity check the topos env ([a5fe251](https://github.com/ocasazza/panel-kit/commit/a5fe25143086cbbde181d67afdf73391cd2ef44f))
* resolve redundancy collisions (Viewport, Charset, Align/TextAlign, stale re-exports) ([ebf9080](https://github.com/ocasazza/panel-kit/commit/ebf90808408742395d942f1c9c1d38138e6f22b5))
* **snap:** gate tile-span resize on tiling mode, not just snap_resize ([310fa93](https://github.com/ocasazza/panel-kit/commit/310fa931076d55870b874fb6d6cd93bf59b4cd7c))
* **snap:** grid snapping for tiling resize + larger floating grid ([b0d5071](https://github.com/ocasazza/panel-kit/commit/b0d5071b8c77257130d159bd7acf7ea75da9df48))
* **snap:** tiling resize works with snap off; column overflow hidden ([dac9f00](https://github.com/ocasazza/panel-kit/commit/dac9f0061b73fa8fdb554a9575985a413facaebb))
* **spec:** accept the amphiphile layout in the strict validator ([984560b](https://github.com/ocasazza/panel-kit/commit/984560b087dce76373296f7b89fa1a38cf0f0e35))
* **spec:** workspace schema, strict table fields and reference fixture match TableColumn.kind and TableRow id/tags ([72d58dc](https://github.com/ocasazza/panel-kit/commit/72d58dce130e918d90804dfd3aac08bd1d4e3ed3))
* **spinner:** rest the ring under reduced motion; parity check has no exemptions ([0431d11](https://github.com/ocasazza/panel-kit/commit/0431d1121b2f9fe2e2dc99fc9cfacc9b1173081d))
* **table:** rows a replacement drops leave the selection, anchor, cursor and tag focus ([f563a6b](https://github.com/ocasazza/panel-kit/commit/f563a6bc460a38cdab749b364e595d29ed84b36c))
* **table:** rows of edit tables open their draft; clippy clean ([f4375d9](https://github.com/ocasazza/panel-kit/commit/f4375d9fd884624279706761bf11cef1b128a035))
* **test:** regenerate the workspace schema and give the frontmatter fixture a value over the chip budget ([5242252](https://github.com/ocasazza/panel-kit/commit/5242252fbe27c603e99ec53fae0fc374f9b66c17))
* **tiling:** increase column-flow vertical resize sensitivity 4x ([263dd5c](https://github.com/ocasazza/panel-kit/commit/263dd5cfdca4d7e3c4f4a34eec554dfb0a2f0800))
* **tiling:** the grid declares only the columns a tile reaches, and a column stretches its members ([d4b5afe](https://github.com/ocasazza/panel-kit/commit/d4b5afeac5fb149ad4217af06fdf0ea6348b4783))
* **topos:** keep the topos demo on its own trace data and compare physics tuning without the regime sort ([f0bd17f](https://github.com/ocasazza/panel-kit/commit/f0bd17f4dbe58188317adf41258609b785ed6fa1))
* **trace:** labels ellipsise in the column they are given ([6910d1b](https://github.com/ocasazza/panel-kit/commit/6910d1bdd282d766181e1b7baece3d1780a1eaa8))
* **trace:** the canvas takes the room its host leaves, not the viewport ([e0bdc14](https://github.com/ocasazza/panel-kit/commit/e0bdc14cbd7b760191199b9a23dc300182c05150))
* **trio:** adapt the graph and trio work to main's theme roles, resolver, and env names ([583b7ed](https://github.com/ocasazza/panel-kit/commit/583b7ed21b565585f37fc785dd78609e49ac92ad))
* **trio:** follow main's table, theme-role, and canary APIs; paint external bodies in Bevy ([e3d7c6e](https://github.com/ocasazza/panel-kit/commit/e3d7c6e602c8d841e208e81f8513b7431b2c86ed))
* **tui:** clip panel painting and hits to the workspace ([97a5d51](https://github.com/ocasazza/panel-kit/commit/97a5d516275a6b9ad9497ed5f12ba39f098e8956))
* **tui:** the topos offscreen canary names a web-surface build and runs as a flake check ([8b17b98](https://github.com/ocasazza/panel-kit/commit/8b17b985359b0378b1d64ce4fd05b5b01438ee02))


### Features

* add GrafanaPanel and IdePanel reusable components ([#1](https://github.com/ocasazza/panel-kit/issues/1)) ([f281edb](https://github.com/ocasazza/panel-kit/commit/f281edb1423aea76eccbc1bb0f11bc3bc4816569))
* add multi_spec topos demo with grammar switching ([d4d6522](https://github.com/ocasazza/panel-kit/commit/d4d6522cd83204633bc1ba3009d7f916e36326f7))
* **bevy:** copy table rows with Ctrl/Cmd-C and paint card tag chips ([fbd849d](https://github.com/ocasazza/panel-kit/commit/fbd849d0438593fb491e83be25e0cad3f267f2c6))
* **bevy:** match the web canary, convert cell specs in core, gate visual parity ([da02ac2](https://github.com/ocasazza/panel-kit/commit/da02ac26fe7a3d0f04f3dd534203a6bd970889ad))
* **bevy:** paint External graph bodies; add a graph panel to the canary ([afa2a33](https://github.com/ocasazza/panel-kit/commit/afa2a33755cc2784b5c10217cb9c862871cede2f))
* **bevy:** port the data table's tag ribbon, column drag/resize, keyboard and windowing ([8f7a893](https://github.com/ocasazza/panel-kit/commit/8f7a8933609477fec1668420cc44bb68fc6b2d98))
* **bevy:** render and read input through a named target camera ([d3acf03](https://github.com/ocasazza/panel-kit/commit/d3acf03aaf74345e03240538e3ca281ca41bcd95))
* **bevy:** select graph nodes by pressing them ([8e32119](https://github.com/ocasazza/panel-kit/commit/8e32119a9cd8daf50db3aff2965cf0e6953aa981))
* **canary:** tagged, windowed Nodes table with a seeded keyboard cursor ([2f418e8](https://github.com/ocasazza/panel-kit/commit/2f418e825c23f54d2b098af5615b7115c6ef8f94))
* **cascade:** cascading Miller-columns dropdown component ([6557db7](https://github.com/ocasazza/panel-kit/commit/6557db784f280932c623baa0b6b674988a67357e))
* **cascade:** fit-to-text sizing and optional breadcrumb ([afee000](https://github.com/ocasazza/panel-kit/commit/afee00095555496c2f8348e5febd5a6f73050274))
* **core:** external-component contract and renderer-neutral graph model ([57c3379](https://github.com/ocasazza/panel-kit/commit/57c3379ab5e5f863c2697f77a9e9cb600631944a))
* **core:** floating panels cap at a viewport fraction ([d9cab37](https://github.com/ocasazza/panel-kit/commit/d9cab3760fe9b584c2f23c21a32d06091c28d435))
* **core:** vendored widget models — frame ring, badge detectors, form parsing, hints, selection pills, build feed ([af4e0ae](https://github.com/ocasazza/panel-kit/commit/af4e0aef757192e8cde405ccfec00112f2407832))
* **dropdown:** grouped searchable single-select component ([2351de7](https://github.com/ocasazza/panel-kit/commit/2351de7463dc82a2e0e0e83fe4dc6740bdb744fb))
* **editor:** a Nix Monarch grammar and a submit action ([807e911](https://github.com/ocasazza/panel-kit/commit/807e911f8eab5184865a2b0235080fd7f3c27250))
* **editor:** a word-wrap prop for wide read-only values ([9b271d7](https://github.com/ocasazza/panel-kit/commit/9b271d74cba8a5a2d3228a8028f6a67f6729f7d6))
* **event-graph:** expandable node facts on rails, graph inspector, standalone example ([fcd4f2d](https://github.com/ocasazza/panel-kit/commit/fcd4f2d748c009eb0234cef89cc472b70eda5f83))
* **event-graph:** node facts and expansion model ([990db28](https://github.com/ocasazza/panel-kit/commit/990db28d42a393f1f7eae5a3e03bf190b1f55699))
* **event-graph:** rail and swimlane event graph with query, legend and sample data ([c7d312c](https://github.com/ocasazza/panel-kit/commit/c7d312c3c0cb5544f109262e1c85b09d221426e2))
* **event-graph:** sequence-diagram lanes, collab tables, windowed graph, 1M-event layout ([c4a08b4](https://github.com/ocasazza/panel-kit/commit/c4a08b4bf20983d0eb261560b8b5e0aba57e5a0d))
* **examples:** auto-loop graph panel on jump-cannon's graph view ([b3a6ca2](https://github.com/ocasazza/panel-kit/commit/b3a6ca288e29dd4f3d828f22d35bd2bcd7fbed2e))
* **examples:** every topos panel is a section over the active subobject ([ce55736](https://github.com/ocasazza/panel-kit/commit/ce5573627c789f870bc503aa171e3d57c45abf69))
* **examples:** okf topos demo on the web with the WebGPU graph ([bf16712](https://github.com/ocasazza/panel-kit/commit/bf16712ca97781d48999b878850b84a21c0affa8))
* **examples:** omp auto-loop web host with section-edit write path ([8f73319](https://github.com/ocasazza/panel-kit/commit/8f7331963bef40d6230461f034c37095a5372df1))
* **forms:** form controls on every renderer and an Inputs canary panel ([4303e84](https://github.com/ocasazza/panel-kit/commit/4303e845cd2769368547f00ec1a4e225ac7d32db))
* **grafana:** split the embed into GrafanaDashboard + GrafanaPanel ([#2](https://github.com/ocasazza/panel-kit/issues/2)) ([39b7881](https://github.com/ocasazza/panel-kit/commit/39b788186eb137d9207866d16b62de45791ad212))
* **grammar:** add panel-kit-grammar topos/grammar layer and demos ([90003c5](https://github.com/ocasazza/panel-kit/commit/90003c5bca4cc1d03eb1ef209ad7549b7d61e251))
* **grammar:** okf engine over a pinned Open Knowledge Format snapshot ([6faf07b](https://github.com/ocasazza/panel-kit/commit/6faf07b9f07cf63679297fa8a7349adc82ee6cf4))
* **grammar:** parse_append extends a site by appended lines without reparsing ([3518b13](https://github.com/ocasazza/panel-kit/commit/3518b13c2a0681b065256fa22b4b47349265d957))
* **grammar:** section edits, sheaf restriction, and site gluing ([895111f](https://github.com/ocasazza/panel-kit/commit/895111f0b09c28cd40b588eaf6e0cb74c8c7db89))
* **graph:** amphiphile self-assembly layout and per-node species ([e46274f](https://github.com/ocasazza/panel-kit/commit/e46274f2a298790ea7fa67617d61ba3897421aa3))
* **graph:** render the graph contract on WebGPU, SVG, and braille ([11dee67](https://github.com/ocasazza/panel-kit/commit/11dee6790b6d7c9a76bdcc0ccfa3566b1d2c7768))
* **keepalive:** let the host supply the panel shell ([#21](https://github.com/ocasazza/panel-kit/issues/21)) ([d0948db](https://github.com/ocasazza/panel-kit/commit/d0948db22ff9a8b4c2431eaf3fc3d9697166683b))
* **keymap:** data-authored keymaps, graph navigation cursor, and tour ([72b0510](https://github.com/ocasazza/panel-kit/commit/72b0510a7b9dee1416d78555d87a313cc4127614))
* **keys:** add keymap vocabulary, graph navigation, and extended key set ([2877fef](https://github.com/ocasazza/panel-kit/commit/2877fefa24459c4742154973ffc311860dfd2e03))
* **nix:** omp auto-loop trace and control regimes ([94c0ce5](https://github.com/ocasazza/panel-kit/commit/94c0ce5b635436f344766621888e145f1bce994c))
* **panel:** add loading progress controls ([01e5ce1](https://github.com/ocasazza/panel-kit/commit/01e5ce1cd1bf21137157aa8ebf7066ebdbfeac77))
* responsive mobile/desktop layout support ([186f482](https://github.com/ocasazza/panel-kit/commit/186f482c3705f11632f1d72b2309552dfc0e4220))
* **snap:** grid snapping for both floating and tiling modes ([3928cf6](https://github.com/ocasazza/panel-kit/commit/3928cf6f1f0f7212c78194a20e679ade790931fe))
* **table): data table component; fix(cascade:** readable long labels ([ab95f6e](https://github.com/ocasazza/panel-kit/commit/ab95f6e5f99e7360410678db7bf53ce7da303bbf))
* **table:** a replaced table's painter prunes stale selection and raises RowsReplaced ([71e5ba5](https://github.com/ocasazza/panel-kit/commit/71e5ba569c68c0eb69f2aec524ac0afb92f71b7e))
* **table:** a scaled table re-reads its scroller when it resizes ([9669d78](https://github.com/ocasazza/panel-kit/commit/9669d78ce93127c8b9d0b1344750b121f0420a19))
* **table:** host-owned sort, filter, focus and selection keyed by row id ([d8aca0a](https://github.com/ocasazza/panel-kit/commit/d8aca0a771b54da3f8e4fc7112e2492a795c87f8))
* **table:** paint scaled scrolling ([64a1067](https://github.com/ocasazza/panel-kit/commit/64a1067890a8f1a4a1ad3b3187288906c9cd9316))
* **table:** port the LiveView data table in full, with its test suites ([2065988](https://github.com/ocasazza/panel-kit/commit/2065988c83ff9e4c1ef1fae88da0387462351a5f))
* **table:** scroll map for entries taller than a browser allows an element ([fb6d128](https://github.com/ocasazza/panel-kit/commit/fb6d1289dcdb45ca321399d9a5623601e59a6bb4))
* **table:** typed columns, one table id per binding, a memoized table component ([0baa35c](https://github.com/ocasazza/panel-kit/commit/0baa35cb0eb2db01c1808e6a845b7aa7e4e74bf9))
* **theme:** rename colour slots to role names (--color-*) ([98d2ebe](https://github.com/ocasazza/panel-kit/commit/98d2ebefcaa55ff82345f9cd391ac82bfb8d0b47))
* **tiling:** size tiled panels to their content by default ([1b31b19](https://github.com/ocasazza/panel-kit/commit/1b31b197c517ed067347f2ec1174262d4c068929))
* **topos:** keep the subobject topos demo beside the trio demos ([6248f90](https://github.com/ocasazza/panel-kit/commit/6248f9067230568f0b4735eaa5373dd0a7cee03e))
* **topos:** okf catalog and lineage regimes over the knowledge catalog ([fdc2cb5](https://github.com/ocasazza/panel-kit/commit/fdc2cb575ba0a7f7850267b16d91e884e7390c97))
* **topos:** trio host over traces and OKF in three regimes ([57cb756](https://github.com/ocasazza/panel-kit/commit/57cb756ef0a043d5081715ee54049516dc3e77bc))
* **topos:** trio morphisms between agentic, membrane, and OKF ([75c0b79](https://github.com/ocasazza/panel-kit/commit/75c0b795b7c1c8468e902d8bc7a4d254dc2de89f))
* **trace:** derive fork lanes from the events when the tree has none ([100c8ff](https://github.com/ocasazza/panel-kit/commit/100c8ff87dac956482519b6ed46d835fb34c35ec))
* **trace:** rename the event graph to trace, size buttons and memory header in the example ([9aa33b7](https://github.com/ocasazza/panel-kit/commit/9aa33b70a4a53d0d9f97a554f322d29d73640238))
* **trio:** grammar and morphism editor panels, dock hints, Pages site ([51bc904](https://github.com/ocasazza/panel-kit/commit/51bc9046f0c21437bc7fc874f7d6e738d6d6712c))
* **trio:** one panel grid for every regime; physics rows by title ([b07e0f8](https://github.com/ocasazza/panel-kit/commit/b07e0f88553a993acb2be4c69f4db43346180b74))
* **trio:** one site, live pulled-back physics, examples under the root only ([ee62328](https://github.com/ocasazza/panel-kit/commit/ee62328df609c1883035aeb40aac5be175076347))
* **trio:** trio demos on every surface; force layouts assemble per component ([c4ce8ac](https://github.com/ocasazza/panel-kit/commit/c4ce8ac3612f1e86bb3e36c7e5452d2b961a70fe))
* **tui:** okf topos demo on crossterm and ratzilla ([5a1c2f1](https://github.com/ocasazza/panel-kit/commit/5a1c2f10f931d7e892406bfa746f790bf78da05c))
* **views:** fold the event graph into the views demo as the Events view ([6b5865d](https://github.com/ocasazza/panel-kit/commit/6b5865d041e0a1ad41b3284031abe26641cbd557))
* **web:** expose panel tiling grow control ([a29eb7f](https://github.com/ocasazza/panel-kit/commit/a29eb7fa333ffbc6acafb3945a51279259c057b4))
* **web:** vendored painters — selectable card, form rows, hint bar, build progress, pills, frontmatter grid ([3abc237](https://github.com/ocasazza/panel-kit/commit/3abc23785b728aa3d65c5b49e23ddf2a4015e974))
* **widgets:** Button component; table filters and demo states as square badge chips ([85dae0c](https://github.com/ocasazza/panel-kit/commit/85dae0cca8d18bbd1f41cbb58a1ff4515b086f34))
* **widgets:** searchable sortable DataTable with column visibility ([c1049fe](https://github.com/ocasazza/panel-kit/commit/c1049fe99e4f90667f0c253dcff82ba2063e6c31))
* **widgets:** searchable sortable DataTable with column visibility ([f59ca25](https://github.com/ocasazza/panel-kit/commit/f59ca2581f678ec721709895cc199677d11580ec))
* **workspace:** add snap-to-grid toggles for resize and move ([b314b79](https://github.com/ocasazza/panel-kit/commit/b314b797febbb0c2042bc445143df178b70f9790))
* **workspace:** own the host glue once for every web consumer ([32e168c](https://github.com/ocasazza/panel-kit/commit/32e168c748af0dc6fda7e0afe0e9925e081c6709))
* **workspace:** own the host glue once for every web consumer ([d770132](https://github.com/ocasazza/panel-kit/commit/d770132add41c3158660cbd450bd7382babb3c05))
* **workspace:** render_keepalive for panels hosting imperative DOM ([6b520d6](https://github.com/ocasazza/panel-kit/commit/6b520d684ecb4444e66ddc5df875a1eb9ea72f97))


### Performance Improvements

* **demo:** an appended line extends the site instead of reparsing the source ([74f92d8](https://github.com/ocasazza/panel-kit/commit/74f92d8b48a1ad387817ecb4fa55ef83a56a5655))
* **sheaf:** a flamegraph indexes objects and calls instead of rescanning the site ([b311ba6](https://github.com/ocasazza/panel-kit/commit/b311ba6ac0b3663b99bc999580be2e176c67f442))
* **table:** a million rows, with filter feedback ([e252060](https://github.com/ocasazza/panel-kit/commit/e2520603d2f8d65d9a880c6b858a210c9d3eab0f))
* **table:** append rows without rebuilding, rank long shared prefixes by segment ([d511a01](https://github.com/ocasazza/panel-kit/commit/d511a01253c21581b2a5471d371f85344f991e1a))
* **table:** check every-row-selected once per selection and view; demo ids unique ([0d8aea8](https://github.com/ocasazza/panel-kit/commit/0d8aea82671fcc308b172ca7ce638c590a3b971b))
* **table:** copy-on-write selection and cursor position hints ([350024b](https://github.com/ocasazza/panel-kit/commit/350024b691726e2e57c84947cb41c2f091da786c))
* **table:** keep the rows both windows mount; reveal a clicked row by the scroll map ([3e56f37](https://github.com/ocasazza/panel-kit/commit/3e56f37590ac656e3ca5493ac1828e4b55d5df07))
* **table:** look for quick words in a column's text at once ([37a7b45](https://github.com/ocasazza/panel-kit/commit/37a7b45a11adec3084cc9038b3664d930dbdeac6))
* **table:** painter shares the selection ([bbdb158](https://github.com/ocasazza/panel-kit/commit/bbdb158aaacc465f1f4b636098206d54b5d1e7c6))
* **table:** refine and extend an order instead of rebuilding it, fold non-ASCII text without allocating, group numbers by value ([d5def2d](https://github.com/ocasazza/panel-kit/commit/d5def2d147f172061e6e6a791a5457bd134f4a04))
* **table:** refine and extend the cached order; demo exercises each scale weakness ([ead25a4](https://github.com/ocasazza/panel-kit/commit/ead25a4d075ff34d1a18d78dd250099e83e91733))
* **table:** stream exports and fix each view mode's entry height ([8d167ee](https://github.com/ocasazza/panel-kit/commit/8d167ee3ccf8424df3f22285417b21b0a91a75c3))
* **topos:** the badge base resolves its one binding, not every sheaf ([2a42a51](https://github.com/ocasazza/panel-kit/commit/2a42a51ae49b924878b7507657cc54c8c9bed4c9))


### Reverts

* **views:** the event graph gets its own example ([04fb43f](https://github.com/ocasazza/panel-kit/commit/04fb43ff207c81ace71d67435f021d2af3c16784))


### BREAKING CHANGES

* Snapshot::tile_drag is Option<TileDrag<K>>;
PanelProjection gains slot, tile_role, and lift (tile_dragging removed);
reorder_tile is removed; hosts send Tick and route pointercancel to
PointerEventKind::Cancel. See MIGRATION.md.

Also includes the working tree's in-progress Bevy backend, canary content
sharing, theme paint, and spec-parity changes.

## [1.2.0](https://github.com/ocasazza/panel-kit/compare/v1.1.1...v1.2.0) (2026-11-02)


### Features

* **core:** vendored widget models — frame ring, badge detectors, form parsing, hints, selection, build feed ([af4e0ae](https://github.com/ocasazza/panel-kit/commit/af4e0aef757192e8cde405ccfec00112f2407832))
* **hosts:** promote the example support glue into the library ([fb1af12](https://github.com/ocasazza/panel-kit/commit/fb1af123ad4af1efa88d93aa337437a07d2f267b))
* **web:** vendored painters — selectable card, form rows, hint bar, build progress, pills, frontmatter grid ([3abc237](https://github.com/ocasazza/panel-kit/commit/3abc23785b728aa3d65c5b49e23ddf2a4015e974))

## [1.1.1](https://github.com/ocasazza/panel-kit/compare/v1.1.0...v1.1.1) (2026-09-17)


### Bug Fixes

* **core:** restore expanding tiling tracks and click-to-raise stacking ([2bea80c](https://github.com/ocasazza/panel-kit/commit/2bea80cc5e6c68a434a64acd0767fce34fffbc55))
* **core:** use minimal row count for column-major tile fills ([58e0be3](https://github.com/ocasazza/panel-kit/commit/58e0be34ce39f6a273ea8ed0a24ef569872a127a))
* **web:** wire panel body presses to focus/raise through the reducer ([83d0b41](https://github.com/ocasazza/panel-kit/commit/83d0b41c6e45b87590e5ad2f78d0278fdcb20a90))

# [1.1.0](https://github.com/ocasazza/panel-kit/compare/v1.0.0...v1.1.0) (2026-09-16)


### Features

* port fork widgets and policies; fix inset-chrome regression ([e216fa6](https://github.com/ocasazza/panel-kit/commit/e216fa6ddbf2f4783af1fbcb76c61d5cb9b3d23b))

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
* named workspace views with per-view layout persistence ([#6](https://github.com/ocasazza/panel-kit/issues/6)) ([2524fec](https://github.com/ocasazza/panel-kit/commit/2524fec6e731c6d41d1569058becaccd3c1b2adf))
* store-shaped async hydration with mandatory-percentage progress bars ([930c067](https://github.com/ocasazza/panel-kit/commit/930c067de8ba5a00a5b24823730317baf0b0f74d))


### BREAKING CHANGES

* 1.0 is a clean cutover to host-owned composition. Git-pinned consumers keep
  their existing revision until their migration branch builds, then update the
  pin; no compatibility facades or deprecated aliases are shipped.
* The web workspace controller, hook, all render/dock methods, and the named
  views controller composite are removed. Applications now own snapshots,
  reducer calls, projection scratch, persistence timing, and Dioxus part
  composition.
* The TUI workspace controller and its render/input/persistence methods are
  removed. Terminal hosts now project through core and draw `panel-kit-tui`
  root, panel, dock, and scrollbar parts explicitly.
* Persistence ports live in `panel_kit_core::persist`: `LayoutStore` now
  includes `clear`, `SavePolicy` owns write timing, and web/TUI stores are
  byte transports only.
* Theme defaults move to `panel_kit_core::theme::ThemeTokens`; web CSS and TUI
  colours are emitted or converted from that source, including focus-ring
  coverage.
* Badge and widget data models move to core, while web and TUI modules retain
  native painters over the shared semantics.
* `mkLayout` remains the unchanged layout-only `SavedLayoutV2` producer. Full
  declarative workspaces use `mkWorkspaceSpec` and the generated schema/parity
  checks.

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
