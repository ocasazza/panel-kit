{ lib, workspaceSpecSchema }:

let
  schemaValidator = import ./validateSchema.nix { inherit lib; };

  die = pointer: message: throw "mkWorkspaceSpec: ${pointer}: ${message}";
  join = pointer: segment: if pointer == "" then "/${toString segment}" else "${pointer}/${toString segment}";
  has = value: name: builtins.hasAttr name value;
  require = pointer: value: name: if has value name then value.${name} else die (join pointer name) "missing required field";
  allowed = pointer: names: value:
    if !(builtins.isAttrs value) || value == null then die pointer "expected attrset, got ${builtins.typeOf value}" else
    let unknown = lib.subtractLists names (builtins.attrNames value);
        checked = if unknown == [] then true else die (join pointer (builtins.head unknown))
          "unknown field; allowed fields: ${lib.concatStringsSep ", " names}";
    in builtins.seq checked value;
  asFloat = value: value + 0.0;
  mapIndex = f: values: lib.imap0 f values;
  unique = values: lib.unique (builtins.filter (value: value != null) values);

  defaultTheme = {
    colors = {
      bg = "#0a0a0a"; panel = "#0d0d0d"; fg = "#ededed"; dim = "#7a7a7a";
      line = "#262626"; line2 = "#5f5f5f"; inverse_bg = "#ededed"; inverse_fg = "#0a0a0a";
      accent = "#5ef38c"; red = "#ff5f56"; yellow = "#ffbd2e"; green = "#27c93f";
      blue = "#3b9bff"; pink = "#ff5fc3"; badge_info = "#83b7cc"; focus_ring = "#ededed";
    };
    typography = {
      family = "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", \"Consolas\", monospace";
      body_size = 13.0; body_line_height = 1.5; label_size = 0.72; label_weight = 700; label_tracking = 0.06;
    };
    density = { panel_radius = 4.0; badge_radius = 999.0; spacing_xs = 4.0; spacing_sm = 6.0; spacing_md = 8.0; };
  };

  normalizeWorkspace = spec:
    let input = allowed "" [ "spec_version" "id" "layout" "surface" "chrome" "input" "glyphs" "theme" "persistence" "panels" ] spec; in {
      spec_version = require "" input "spec_version";
      id = require "" input "id";
      layout = normalizeLayout "/layout" (require "" input "layout");
      surface = normalizeSurface "/surface" (require "" input "surface");
      chrome = normalizeChrome "/chrome" (require "" input "chrome");
      input = normalizeInput "/input" (require "" input "input");
      glyphs = require "" input "glyphs";
      theme = normalizeTheme "/theme" (require "" input "theme");
      persistence = normalizePersistence "/persistence" (require "" input "persistence");
      panels = mapIndex normalizePanel (require "" input "panels");
    };

  normalizeLayout = pointer: value:
    let input = allowed pointer [ "units" "viewport" "preferred_mode" "clamp" "tile" ] value; in {
      units = require pointer input "units";
      viewport = builtins.map asFloat (require pointer input "viewport");
      preferred_mode = require pointer input "preferred_mode";
      clamp = normalizeByFields (join pointer "clamp") [ "outer_w" "outer_h" "floor_w" "floor_h" "inner" "edge" "min_w" "min_h" "max_frac" ] asFloat (require pointer input "clamp");
      tile = normalizeByShape (join pointer "tile") [ "resize" "row_min" "gap" "padding" "fill_viewport" ] {
        resize = normalizeByFields (join pointer "tile/resize") [ "row" "col_floor" "outer" ] asFloat (require (join pointer "tile") (require pointer input "tile") "resize");
        row_min = asFloat (require (join pointer "tile") (require pointer input "tile") "row_min");
        gap = asFloat (require (join pointer "tile") (require pointer input "tile") "gap");
        padding = asFloat (require (join pointer "tile") (require pointer input "tile") "padding");
        fill_viewport = require (join pointer "tile") (require pointer input "tile") "fill_viewport";
      } (require pointer input "tile");
    };

  normalizeSurface = pointer: value:
    let input = allowed pointer [ "compact_max" "tablet_max" "fallback_capabilities" "resize_policy" ] value; in {
      compact_max = asFloat (require pointer input "compact_max");
      tablet_max = asFloat (require pointer input "tablet_max");
      fallback_capabilities = normalizeByFields (join pointer "fallback_capabilities") [ "coarse_pointer" "hover" "keyboard" ] (x: x) (require pointer input "fallback_capabilities");
      resize_policy = require pointer input "resize_policy";
    };

  normalizeChrome = pointer: value:
    let fields = [ "metrics" "hit_target_min" "panel_header_h" "panel_frame" "title_in_border" "mode_control" "minimize_control" "maximize_control" "resize_grip" "dock" "dock_label" ];
        input = allowed pointer fields value; in
    normalizeByShape pointer fields (input // {
      metrics = normalizeByFields (join pointer "metrics") [ "inset" "dock_h" ] asFloat input.metrics;
      hit_target_min = asFloat input.hit_target_min; panel_header_h = asFloat input.panel_header_h;
    }) value;

  normalizeInput = pointer: value:
    let input = allowed pointer [ "steps" "bindings" ] value; in {
      steps = normalizeByFields (join pointer "steps") [ "coarse" "fine" ] asFloat input.steps;
      bindings = mapIndex (index: binding: normalizeByShape (join (join pointer "bindings") index) [ "chord" "command" ] binding binding) input.bindings;
    };

  normalizePersistence = pointer: value:
    let input = allowed pointer [ "enabled" "key" "restore" "save_policy" ] value; in input;

  normalizeTheme = pointer: value:
    let input = allowed pointer [ "colors" "typography" "density" ] value; in {
      colors = normalizeByFields (join pointer "colors") [ "bg" "panel" "fg" "dim" "line" "line2" "inverse_bg" "inverse_fg" "accent" "red" "yellow" "green" "blue" "pink" "badge_info" "focus_ring" ] (x: x) input.colors;
      typography = normalizeByFields (join pointer "typography") [ "family" "body_size" "body_line_height" "label_size" "label_weight" "label_tracking" ] (x: x) input.typography;
      density = normalizeByFields (join pointer "density") [ "panel_radius" "badge_radius" "spacing_xs" "spacing_sm" "spacing_md" ] asFloat input.density;
    };

  normalizePanel = index: value:
    let pointer = join "/panels" index; input = allowed pointer [ "id" "title" "slug" "window" "content" ] value; in {
      id = input.id; title = input.title; slug = input.slug;
      window = normalizeWindow (join pointer "window") input.window;
      content = normalizeContent (join pointer "content") input.content;
    };

  normalizeWindow = pointer: value:
    let input = allowed pointer [ "x" "y" "w" "h" "state" "z" "tile_w" "tile_h" ] value; in
    input // { x = asFloat input.x; y = asFloat input.y; w = asFloat input.w; h = asFloat input.h; };

  normalizeContent = pointer: value:
    let
      input = if builtins.isAttrs value && value != null then value else allowed pointer [] value;
      kind = require pointer input "kind";
    in
    if kind == "custom" then
      let content = allowed pointer [ "kind" "binding" ] input; in
      { inherit kind; binding = require pointer content "binding"; }
    else if kind == "text" then
      let content = allowed pointer [ "kind" "source" "scroll" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeText (require pointer content "source"); scroll = require pointer content "scroll"; }
    else if kind == "editor" then
      let content = allowed pointer [ "kind" "binding" "multiline" "placeholder" ] input; in
      { inherit kind; binding = require pointer content "binding"; multiline = require pointer content "multiline"; placeholder = require pointer content "placeholder"; }
    else if kind == "badges" then
      let content = allowed pointer [ "kind" "source" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeBadges (require pointer content "source"); }
    else if kind == "table" then
      let content = allowed pointer [ "kind" "source" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeTable (require pointer content "source"); }
    else if kind == "time_series" then
      let content = allowed pointer [ "kind" "source" "unit" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeSeriesList (require pointer content "source"); unit = require pointer content "unit"; }
    else if kind == "gauges" then
      let content = allowed pointer [ "kind" "source" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeGaugeList (require pointer content "source"); }
    else if kind == "flamegraph" then
      let content = allowed pointer [ "kind" "source" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeFlameList (require pointer content "source"); }
    else if kind == "boxplot" then
      let content = allowed pointer [ "kind" "source" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeBoxList (require pointer content "source"); }
    else if kind == "meter" then
      let content = allowed pointer [ "kind" "source" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeMeter (require pointer content "source"); }
    else if kind == "status" then
      let content = allowed pointer [ "kind" "source" ] input; in
      { inherit kind; source = normalizeDataSource (join pointer "source") normalizeStatus (require pointer content "source"); }
    else if kind == "spinner" then
      let content = allowed pointer [ "kind" "label" ] input; in
      { inherit kind; label = normalizeDataSource (join pointer "label") normalizeString (require pointer content "label"); }
    else die (join pointer "kind") "unsupported content kind ${toString kind}";

  normalizeDataSource = pointer: normalizeValue: value:
    let
      sourceInput = if builtins.isAttrs value && value != null then value else allowed pointer [] value;
      source = require pointer sourceInput "source";
    in
    if source == "inline" then
      let input = allowed pointer [ "source" "value" ] sourceInput; in
      { inherit source; value = normalizeValue (join pointer "value") (require pointer input "value"); }
    else if source == "binding" then
      let input = allowed pointer [ "source" "id" ] sourceInput; in
      { inherit source; id = require pointer input "id"; }
    else die (join pointer "source") "expected inline or binding";

  normalizeString = _pointer: value: value;
  normalizeText = pointer: value: normalizeByShape pointer [ "text" ] value value;
  normalizeBadges = pointer: items: mapIndex (index: normalizeBadge (join pointer index)) items;
  normalizeBadge = pointer: value:
    let input = allowed pointer [ "kind" "field" "value" "active" "with_x" "with_plus" "small" "override_color" "accent_color" "click_kind" "emit_hover" ] value; in {
      kind = normalizeBadgeKind (require pointer input "kind");
      field = require pointer input "field";
      value = require pointer input "value";
      active = require pointer input "active";
      with_x = require pointer input "with_x";
      with_plus = require pointer input "with_plus";
      small = require pointer input "small";
      override_color = require pointer input "override_color";
      accent_color = require pointer input "accent_color";
      click_kind = normalizeBadgeClick (require pointer input "click_kind");
      emit_hover = require pointer input "emit_hover";
    };
  normalizeBadgeKind = kind: if builtins.isString kind then { tag = "Tag"; doctype = "Doctype"; folder = "Folder"; author = "Author"; date = "Date"; status = "Status"; generic = "Generic"; }.${kind} or kind else kind;
  normalizeBadgeClick = click: if click == "toggle" then "Toggle" else if click == "clicked" then "Clicked" else click;
  normalizeTable = pointer: value:
    let input = allowed pointer [ "columns" "rows" ] value; in { columns = require pointer input "columns"; rows = require pointer input "rows"; };
  normalizeSeriesList = _pointer: value: value;
  normalizeGaugeList = _pointer: value: value;
  normalizeFlameList = _pointer: value: value;
  normalizeBoxList = _pointer: value: value;
  normalizeMeter = _pointer: value: value;
  normalizeStatus = _pointer: value: value;

  normalizeByFields = pointer: names: normalize: value:
    let input = allowed pointer names value; in lib.genAttrs names (name: normalize (require pointer input name));
  normalizeByShape = pointer: names: normalized: original: builtins.deepSeq (allowed pointer names original) normalized;

  contentBindings = content:
    if content.kind == "custom" || content.kind == "editor" then [ content.binding ]
    else if content.kind == "spinner" then dataSourceBinding content.label
    else dataSourceBinding (content.source or null);
  dataSourceBinding = source: if source != null && source.source == "binding" then [ source.id ] else [];


  validateValue = value:
    let errors = schemaValidator.validate workspaceSpecSchema value; in
    if errors == [] then value else die "" (lib.concatStringsSep "; " errors);

  mkWorkspaceSpec = spec:
    let value = validateValue (normalizeWorkspace spec); in {
      inherit value;
      json = builtins.toJSON value;
      panel_ids = builtins.map (panel: panel.id) value.panels;
      bindings = unique (lib.concatMap (panel: contentBindings panel.content) value.panels);
    };
in
{
  inherit mkWorkspaceSpec defaultTheme;
}
