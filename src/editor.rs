//! Monaco code-editor component — a Dioxus wrapper around a vendored,
//! minified ESM build of `monaco-editor`, with a Monarch tokenizer for
//! `.pest` grammar files (plus a minimal `toml`, which Monaco does not
//! ship) and a `panel-kit-dark` theme mapped onto the crate's palette.
//!
//! This widget is inherently web-only (Monaco renders to the DOM), so unlike
//! the badge there is nothing renderer-neutral to push into `panel-kit-core`.
//!
//! # Assets
//!
//! The vendored bundle lives in `assets/vendor/monaco-editor-0.56.0/`
//! (see [`MONACO_ASSET_DIR`]): `monaco.esm.js` + `monaco.esm.css` +
//! `editor.worker.js` + the codicon font, built from the upstream npm
//! package with esbuild (`--bundle --minify --format=esm`). The Rust side
//! injects the small loader shim (`panel-kit-loader.js` in the same
//! directory) as an inline script; the shim then pulls in the bundle with a
//! dynamic `import()` and serves the worker as a classic `Worker`, so the
//! same static files work under `trunk build` and in a Tauri webview — the
//! consumer only has to serve the directory (for trunk:
//! `<link data-trunk rel="copy-dir" href="assets/vendor" />` after copying
//! `vendor/` into the app's own `assets/`). Override the location before the
//! first editor mounts with [`set_monaco_asset_base`].
//!
//! # Interfaces
//!
//! [`MonacoEditor`] supports three usage styles, combinable freely:
//!
//! - two-way bound: pass `value: Signal<String>` — edits write the signal,
//!   external signal writes update the editor preserving the cursor;
//! - pull/event: `on_change: EventHandler<String>` fires on every edit;
//! - push/pull imperative: [`EditorHandle`] (handed out via `on_ready`)
//!   exposes `set_value`, `value`, `set_language`, `set_read_only`,
//!   `layout`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

use dioxus::prelude::*;
use wasm_bindgen::{JsCast, JsValue};

/// Version of the vendored `monaco-editor` bundle. Bumped together with the
/// files under `assets/vendor/monaco-editor-<version>/`.
pub const MONACO_ASSET_VERSION: &str = "0.56.0";

/// Default directory the editor loads its bundle from, resolved against the
/// document base URL. Copy `assets/vendor/monaco-editor-0.56.0/` from this
/// crate into the consuming app's served assets keeping this relative path,
/// or point elsewhere with [`set_monaco_asset_base`].
pub const MONACO_ASSET_DIR: &str = "assets/vendor/monaco-editor-0.56.0";

/// Monaco theme matching the panel-kit palette (`--bg`/`--panel`/`--fg`/
/// `--dim`/`--line*`/accent colours) on the Courier Prime monospace stack.
/// Registered by [`MonacoEditor`] on first mount; also the default theme.
pub const PANEL_KIT_DARK_THEME: &str = "panel-kit-dark";

/// Monaco `fontFamily` — Courier Prime first (the consuming apps ship the
/// webfont), then the `--mono` fallback chain from the base stylesheet.
const EDITOR_FONT_STACK: &str =
    "\"Courier Prime\", ui-monospace, \"SF Mono\", \"JetBrains Mono\", Menlo, Consolas, monospace";

/// The loader shim, injected inline on first use (see module docs). Kept
/// with the vendored assets: it is integration glue, not application logic.
const LOADER_JS: &str = include_str!("../assets/vendor/monaco-editor-0.56.0/panel-kit-loader.js");

// The only wasm-bindgen boundary that talks to handwritten JS: typed
// externs onto the object the loader shim installs as
// `globalThis.__panelKitMonaco`. No eval, no stringly-typed calls — every
// function is declared with real argument and return types. The namespace
// lookup is lazy, so injecting the shim before the first call is enough.
mod interop {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = load)]
        pub fn load(base: &str) -> js_sys::Promise;

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = create, catch)]
        pub fn create(el: &web_sys::HtmlElement, options: &JsValue) -> Result<u32, JsValue>;

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = dispose)]
        pub fn dispose(id: u32);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = setValue)]
        pub fn set_value(id: u32, value: &str);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = setValueKeepCursor)]
        pub fn set_value_keep_cursor(id: u32, value: &str);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = getValue)]
        pub fn get_value(id: u32) -> String;

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = setLanguage)]
        pub fn set_language(id: u32, language: &str);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = setReadOnly)]
        pub fn set_read_only(id: u32, read_only: bool);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = layout)]
        pub fn layout(id: u32);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = setTheme)]
        pub fn set_theme(name: &str);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = onChange)]
        pub fn on_change(id: u32, callback: &js_sys::Function);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = registerLanguage)]
        pub fn register_language(id: &str, monarch: &JsValue, configuration: &JsValue);

        #[wasm_bindgen(js_namespace = ["__panelKitMonaco"], js_name = defineTheme)]
        pub fn define_theme(name: &str, data: &JsValue);
    }
}

thread_local! {
    /// Editors currently mounted — `EditorHandle` methods no-op once an
    /// editor is gone instead of throwing across the wasm boundary.
    static LIVE_EDITORS: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
    /// Rust change closures, one per live editor; dropped on dispose.
    static CHANGE_CALLBACKS: RefCell<HashMap<u32, Closure>> = RefCell::new(HashMap::new());
    static MONACO_BASE: RefCell<Option<String>> = const { RefCell::new(None) };
    static LOADER_INJECTED: Cell<bool> = const { Cell::new(false) };
    static LANGUAGES_READY: Cell<bool> = const { Cell::new(false) };
}

/// wasm-bindgen closures of the shape the shim's `onChange` expects. Named
/// so the thread-local map above stays readable.
type Closure = wasm_bindgen::closure::Closure<dyn FnMut(String)>;

/// Error from loading the Monaco bundle or creating an editor instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorError(String);

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EditorError {}

fn js_err(e: &JsValue) -> String {
    if let Some(s) = e.as_string() {
        return s;
    }
    if let Ok(err) = e.clone().dyn_into::<js_sys::Error>() {
        return err.message().into();
    }
    "unknown js error".to_string()
}

/// Override where the editor loads its vendored bundle from (default
/// [`MONACO_ASSET_DIR`], relative to the document base URL). Must be called
/// before the first [`MonacoEditor`] mounts — the loader caches the first
/// successful load.
pub fn set_monaco_asset_base(path: &str) {
    MONACO_BASE.with(|b| *b.borrow_mut() = Some(path.trim_end_matches('/').to_string()));
}

fn monaco_asset_base() -> String {
    MONACO_BASE
        .with(|b| b.borrow().clone())
        .unwrap_or_else(|| MONACO_ASSET_DIR.to_string())
}

/// Inject the loader shim as an inline script (idempotent). Inline, not
/// `src=`: the shim is small and this way only the multi-MB bundle needs to
/// be a served file.
fn inject_loader() {
    if LOADER_INJECTED.with(|i| i.replace(true)) {
        return;
    }
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        LOADER_INJECTED.with(|i| i.set(false));
        return;
    };
    let Ok(script) = doc.create_element("script") else {
        LOADER_INJECTED.with(|i| i.set(false));
        return;
    };
    script.set_text_content(Some(LOADER_JS));
    let Some(head) = doc.head() else {
        LOADER_INJECTED.with(|i| i.set(false));
        return;
    };
    // An inline script executes synchronously on insertion, so the
    // `__panelKitMonaco` global exists as soon as append_child returns.
    let _ = head.append_child(&script);
}

async fn load_monaco() -> Result<(), EditorError> {
    inject_loader();
    let base = monaco_asset_base();
    wasm_bindgen_futures::JsFuture::from(interop::load(&base))
        .await
        .map(|_| ())
        .map_err(|e| EditorError(format!("monaco bundle failed to load from {base}: {}", js_err(&e))))
}

/// Load Monaco (if needed) and register the `pest` grammar language —
/// Monarch tokenizer for `.pest` files (line `//` and block `/* */`
/// comments, rule definitions, `'x'` / `"x"` / `^"x"` literals, the
/// `~ | * + ? ! & @{ ${ _{ !{ ( ) , = ..` operators, and pest's built-in
/// rules `ANY SOI EOI NEWLINE PEEK PEEK_ALL POP POP_ALL DROP PUSH ASCII_*`
/// as keywords) with bracket matching and comment/bracket language
/// configuration — plus a minimal `toml` language for package manifests
/// (Monaco ships `ini` but no `toml`). Idempotent; [`MonacoEditor`] calls
/// it on mount, so calling it directly is only needed outside the
/// component.
pub async fn register_pest_language() -> Result<(), EditorError> {
    load_monaco().await?;
    register_languages();
    Ok(())
}

fn register_languages() {
    if LANGUAGES_READY.with(|r| r.replace(true)) {
        return;
    }
    interop::register_language("pest", &pest_monarch(), &pest_language_configuration());
    interop::register_language("toml", &toml_monarch(), &toml_language_configuration());
    interop::define_theme(PANEL_KIT_DARK_THEME, &panel_kit_dark_theme());
}

fn editor_alive(id: u32) -> bool {
    LIVE_EDITORS.with(|live| live.borrow().contains(&id))
}

fn drop_editor(id: u32) {
    if LIVE_EDITORS.with(|live| live.borrow_mut().remove(&id)) {
        interop::dispose(id);
        CHANGE_CALLBACKS.with(|cbs| cbs.borrow_mut().remove(&id));
    }
}

/// Imperative handle to a mounted [`MonacoEditor`] — `Copy`, so it is
/// trivially storable in a signal or context. Obtained from the
/// component's `on_ready` callback. Every method no-ops once the editor is
/// disposed (panel closed, component unmounted), so stale handles are safe
/// to keep around.
///
/// # Examples
///
/// ```no_run
/// # use dioxus::prelude::*;
/// # use panel_kit::editor::{EditorHandle, MonacoEditor};
/// # fn panel() -> Element {
/// let mut editor = use_signal(|| None::<EditorHandle>);
/// rsx! {
///     MonacoEditor {
///         language: "pest",
///         on_ready: move |h: EditorHandle| editor.set(Some(h)),
///     }
///     button {
///         onclick: move |_| {
///             if let Some(h) = *editor.read() {
///                 h.set_value("main = { SOI ~ \"x\" ~ EOI }");
///             }
///         },
///         "replace contents"
///     }
/// }
/// # }
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EditorHandle {
    id: u32,
}

impl EditorHandle {
    /// Replace the whole document (resets the undo stack; fires the
    /// component's `on_change` and writes the bound `value` signal).
    pub fn set_value(&self, value: &str) {
        if editor_alive(self.id) {
            interop::set_value(self.id, value);
        }
    }

    /// Current document text; `""` once the editor is disposed.
    pub fn value(&self) -> String {
        if editor_alive(self.id) {
            interop::get_value(self.id)
        } else {
            String::new()
        }
    }

    /// Re-tokenize the open model as another language (`"pest"`, `"toml"`,
    /// `"plaintext"`, any Monaco built-in).
    pub fn set_language(&self, language: &str) {
        if editor_alive(self.id) {
            interop::set_language(self.id, language);
        }
    }

    /// Toggle Monaco's `readOnly` option on the instance.
    pub fn set_read_only(&self, read_only: bool) {
        if editor_alive(self.id) {
            interop::set_read_only(self.id, read_only);
        }
    }

    /// Re-measure the container. The editor is created with
    /// `automaticLayout`, so this is only needed after changes Monaco's own
    /// ResizeObserver cannot see (e.g. `display: none` round-trips).
    pub fn layout(&self) {
        if editor_alive(self.id) {
            interop::layout(self.id);
        }
    }
}

/// Loading state behind the component's status overlay.
#[derive(Clone, PartialEq)]
enum EditorStatus {
    Loading,
    Ready,
    Failed(String),
}

/// Monaco code editor panel widget. See the [module docs](self) for the
/// three usage styles and the asset-serving contract.
///
/// The editor fills its container (`.pk-monaco` is `100% × 100%` with
/// `min-height: 0`, so it works directly as a `.panel-body` child) and
/// re-layouts itself on resize. Styling hooks live in [`crate::CSS`].
///
/// # Examples
///
/// ```no_run
/// use dioxus::prelude::*;
/// use panel_kit::editor::MonacoEditor;
///
/// # fn grammar_panel() -> Element {
/// // Two-way bound: edits write the signal, signal writes update the
/// // editor (cursor preserved).
/// let grammar = use_signal(|| "main = { SOI ~ \"x\" ~ EOI }".to_string());
/// rsx! {
///     MonacoEditor {
///         value: grammar,
///         language: "pest",
///         on_change: move |text: String| { /* latest document */ },
///     }
/// }
/// # }
/// ```
#[component]
pub fn MonacoEditor(
    /// Two-way bound editor contents. Editor edits write the signal;
    /// external signal writes are applied with cursor/selection preserved
    /// (and do not fire `on_change`). When omitted the editor is
    /// uncontrolled and starts from `initial`.
    #[props(default)]
    value: Option<Signal<String>>,
    /// Fired with the full document text after every edit — typed or
    /// pushed through [`EditorHandle::set_value`], but not from external
    /// `value`-signal writes.
    #[props(default)]
    on_change: Option<EventHandler<String>>,
    /// Monaco language id: `"pest"` and `"toml"` are registered by this
    /// module, everything Monaco ships (e.g. `"json"`, `"rust"`,
    /// `"plaintext"`) also works. Reactive — changing it re-tokenizes the
    /// open model.
    #[props(default = "plaintext".to_string())]
    language: String,
    /// Reactive read-only flag.
    #[props(default)]
    read_only: bool,
    /// Monaco theme name; defaults to [`PANEL_KIT_DARK_THEME`]. Reactive.
    /// Monaco themes are global, so one value applies to all open editors.
    #[props(default)]
    theme: Option<String>,
    /// Starting text when `value` is not bound; ignored otherwise.
    #[props(default)]
    initial: String,
    /// Fired once when the editor instance is ready, handing out the
    /// imperative [`EditorHandle`].
    #[props(default)]
    on_ready: Option<EventHandler<EditorHandle>>,
) -> Element {
    let handle: Signal<Option<EditorHandle>> = use_signal(|| None);
    let status: Signal<EditorStatus> = use_signal(|| EditorStatus::Loading);
    // Set while an external `value`-signal write is being applied to the
    // editor, so the resulting model-content event is not echoed back into
    // the signal / `on_change` (feedback-loop guard).
    let suppress = use_hook(|| Rc::new(Cell::new(false)));

    // External signal writes -> editor. Editor-originated writes
    // short-circuit in the shim because the texts already match.
    let suppress_ext = suppress.clone();
    use_effect(use_reactive!(|value| {
        let Some(sig) = value else { return };
        let desired = sig.read().clone();
        if let Some(h) = *handle.read() {
            suppress_ext.set(true);
            interop::set_value_keep_cursor(h.id, &desired);
            suppress_ext.set(false);
        }
    }));

    // Reactive props -> live editor options.
    use_effect(use_reactive!(|language, read_only| {
        if let Some(h) = *handle.read() {
            interop::set_language(h.id, &language);
            interop::set_read_only(h.id, read_only);
        }
    }));
    use_effect(use_reactive!(|theme| {
        if handle.read().is_some() {
            interop::set_theme(theme.as_deref().unwrap_or(PANEL_KIT_DARK_THEME));
        }
    }));

    use_drop(move || {
        if let Some(h) = *handle.peek() {
            drop_editor(h.id);
        }
    });

    let mount_initial = initial.clone();
    let mount_language = language.clone();
    let mount_theme = theme.clone().unwrap_or_else(|| PANEL_KIT_DARK_THEME.to_string());
    let mount_read_only = read_only;

    rsx! {
        div { class: "pk-monaco",
            // Monaco owns everything inside this div once mounted; Dioxus
            // only ever touches the status overlay sibling.
            div {
                class: "pk-monaco-host",
                onmounted: move |evt: MountedEvent| {
                    let mut status = status;
                    let mut handle = handle;
                    let suppress = suppress.clone();
                    let el = evt.data().downcast::<web_sys::Element>().cloned();
                    let Some(el) = el else {
                        status.set(EditorStatus::Failed(
                            "onmounted did not yield a web_sys::Element".to_string(),
                        ));
                        return;
                    };
                    let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() else {
                        status.set(EditorStatus::Failed("host is not an HtmlElement".to_string()));
                        return;
                    };
                    let initial = mount_initial.clone();
                    let language = mount_language.clone();
                    let theme = mount_theme.clone();
                    spawn(async move {
                        if let Err(e) = load_monaco().await {
                            status.set(EditorStatus::Failed(e.to_string()));
                            return;
                        }
                        register_languages();
                        let text = match value {
                            Some(v) => v.try_peek().map(|g| g.clone()).unwrap_or_else(|_| {
                                // The consumer is mid-write on this signal (e.g. an
                                // on_change echo); mount with `initial` and let the
                                // subscription apply the live value after.
                                initial.clone()
                            }),
                            None => initial,
                        };
                        let options = editor_options(&text, &language, mount_read_only, &theme);
                        let id = match interop::create(&el, options.as_ref()) {
                            Ok(id) => id,
                            Err(e) => {
                                status.set(EditorStatus::Failed(format!(
                                    "monaco editor.create failed: {}",
                                    js_err(&e)
                                )));
                                return;
                            }
                        };
                        LIVE_EDITORS.with(|live| live.borrow_mut().insert(id));
                        let change: Closure = Closure::new(move |text: String| {
                            if suppress.get() {
                                return;
                            }
                            if let Some(mut sig) = value {
                                sig.set(text.clone());
                            }
                            if let Some(cb) = on_change {
                                cb.call(text);
                            }
                        });
                        interop::on_change(id, change.as_ref().unchecked_ref());
                        CHANGE_CALLBACKS.with(|cbs| cbs.borrow_mut().insert(id, change));
                        let h = EditorHandle { id };
                        handle.set(Some(h));
                        status.set(EditorStatus::Ready);
                        if let Some(cb) = on_ready {
                            cb.call(h);
                        }
                    });
                },
            }
            match &*status.read() {
                EditorStatus::Loading => rsx! {
                    div { class: "pk-monaco-status", "loading editor…" }
                },
                EditorStatus::Ready => rsx! {},
                EditorStatus::Failed(msg) => rsx! {
                    div { class: "pk-monaco-status pk-monaco-error", "{msg}" }
                },
            }
        }
    }
}

/// Monaco `IStandaloneEditorConstructionOptions` as a plain JS object —
/// built via `Reflect` so no JSON round-trip is involved. Options not
/// exposed as props are the panel-kit defaults: `automaticLayout` (panels
/// resize freely), no minimap (panels are small), `fixedOverflowWidgets`
/// (`.panel` clips overflow; suggestion/hover widgets must escape it),
/// Courier Prime stack.
fn editor_options(value: &str, language: &str, read_only: bool, theme: &str) -> js_sys::Object {
    let options = js_sys::Object::new();
    let set = |key: &str, val: JsValue| {
        let _ = js_sys::Reflect::set(options.as_ref(), &JsValue::from_str(key), &val);
    };
    set("value", value.into());
    set("language", language.into());
    set("theme", theme.into());
    set("readOnly", read_only.into());
    set("automaticLayout", true.into());
    set("fontFamily", EDITOR_FONT_STACK.into());
    set("fontSize", 12.into());
    set("scrollBeyondLastLine", false.into());
    set("fixedOverflowWidgets", true.into());
    set("renderWhitespace", "selection".into());
    let minimap = js_sys::Object::new();
    let _ = js_sys::Reflect::set(minimap.as_ref(), &JsValue::from_str("enabled"), &false.into());
    set("minimap", minimap.into());
    options
}

fn json_to_js(value: &serde_json::Value) -> JsValue {
    // serde_json's output is always valid JSON, so parse cannot fail.
    js_sys::JSON::parse(&value.to_string()).unwrap_or(JsValue::UNDEFINED)
}

/// Monarch tokenizer for pest grammars. Token classes: `comment`,
/// `string` / `string.escape` / `string.quote`, `keyword` (built-in rules),
/// `type.identifier` (rule definitions), `identifier` (rule references),
/// `number` (repetition counts), `operator` (incl. the `@{`/`${`/`_{`/`!{`
/// modifiers), `delimiter.*` via `@brackets`.
fn pest_monarch() -> JsValue {
    json_to_js(&serde_json::json!({
        "defaultToken": "identifier",
        "builtins": [
            "ANY", "SOI", "EOI", "NEWLINE",
            "PUSH", "PEEK", "PEEK_ALL", "POP", "POP_ALL", "DROP",
            "ASCII_DIGIT", "ASCII_NONZERO_DIGIT", "ASCII_BIN_DIGIT",
            "ASCII_OCT_DIGIT", "ASCII_HEX_DIGIT", "ASCII_ALPHA_LOWER",
            "ASCII_ALPHA_UPPER", "ASCII_ALPHA", "ASCII_ALPHANUMERIC",
            "ASCII", "WHITE_SPACE"
        ],
        "brackets": [
            { "open": "(", "close": ")", "token": "delimiter.parenthesis" },
            { "open": "{", "close": "}", "token": "delimiter.curly" },
            { "open": "[", "close": "]", "token": "delimiter.square" }
        ],
        "tokenizer": {
            "root": [
                ["//.*$", "comment"],
                ["/\\*", "comment", "@block_comment"],
                ["\\^?\"", "string.quote", "@string_double"],
                ["'", "string.quote", "@string_single"],
                ["[a-zA-Z_][a-zA-Z0-9_]*(?=\\s*=\\s*[!@$_]?\\s*\\{)", "type.identifier"],
                {
                    "regex": "[a-zA-Z_][a-zA-Z0-9_]*",
                    "action": { "cases": { "@builtins": "keyword", "@default": "identifier" } }
                },
                ["\\d+", "number"],
                ["\\.\\.", "operator"],
                ["[@$!_](?=\\{)", "operator"],
                ["[~|*+?&=!^,]", "operator"],
                ["[(){}\\[\\]]", "@brackets"],
                ["\\s+", "white"]
            ],
            "block_comment": [
                ["\\*/", "comment", "@pop"],
                ["[^*]+", "comment"],
                ["\\*", "comment"]
            ],
            "string_double": [
                ["\\\\.", "string.escape"],
                ["\"", "string.quote", "@pop"],
                ["[^\\\\\"]+", "string"]
            ],
            "string_single": [
                ["\\\\.", "string.escape"],
                ["'", "string.quote", "@pop"],
                ["[^\\\\']+", "string"]
            ]
        }
    }))
}

fn pest_language_configuration() -> JsValue {
    json_to_js(&serde_json::json!({
        "comments": { "lineComment": "//", "blockComment": ["/*", "*/"] },
        "brackets": [["(", ")"], ["{", "}"], ["[", "]"]],
        "autoClosingPairs": [
            { "open": "(", "close": ")" },
            { "open": "{", "close": "}" },
            { "open": "[", "close": "]" },
            { "open": "\"", "close": "\"", "notIn": ["string", "comment"] },
            { "open": "'", "close": "'", "notIn": ["string", "comment"] }
        ],
        "surroundingPairs": [
            { "open": "(", "close": ")" },
            { "open": "{", "close": "}" },
            { "open": "[", "close": "]" },
            { "open": "\"", "close": "\"" },
            { "open": "'", "close": "'" }
        ]
    }))
}

/// Minimal Monarch tokenizer for TOML (package manifests) — Monaco ships
/// `ini` but no `toml`. Covers comments, basic/literal strings (single and
/// multiline), booleans, dates/times, numbers (incl. hex/oct/bin/inf/nan),
/// bare keys, and table headers.
fn toml_monarch() -> JsValue {
    json_to_js(&serde_json::json!({
        "defaultToken": "",
        "brackets": [
            { "open": "[", "close": "]", "token": "delimiter.square" },
            { "open": "{", "close": "}", "token": "delimiter.curly" }
        ],
        "tokenizer": {
            "root": [
                ["#.*$", "comment"],
                ["\\[\\[[^\\]\\n]+\\]\\]", "type.identifier"],
                ["\\[[^\\]\\n]+\\]", "type.identifier"],
                ["\"\"\"", "string.quote", "@string_ml_double"],
                ["'''", "string.quote", "@string_ml_single"],
                ["\"", "string.quote", "@string_double"],
                ["'", "string.quote", "@string_single"],
                ["\\b(true|false)\\b", "keyword"],
                ["\\b\\d{4}-\\d{2}-\\d{2}([Tt ][0-9:.]+([Zz]|[+-]\\d{2}:?\\d{2})?)?\\b", "number"],
                ["\\b\\d{2}:\\d{2}:\\d{2}(\\.\\d+)?\\b", "number"],
                ["[+-]?(0x[0-9a-fA-F_]+|0o[0-7_]+|0b[01_]+|[0-9][0-9_]*(\\.[0-9_]+)?([eE][+-]?[0-9]+)?|inf|nan)\\b", "number"],
                ["[A-Za-z0-9_-]+(?=\\s*=)", "identifier"],
                ["[=,.]", "operator"],
                ["[\\[\\]{}]", "@brackets"],
                ["\\s+", "white"]
            ],
            "string_double": [
                ["\\\\.", "string.escape"],
                ["\"", "string.quote", "@pop"],
                ["[^\\\\\"\\n]+", "string"]
            ],
            "string_single": [
                ["'", "string.quote", "@pop"],
                ["[^'\\n]+", "string"]
            ],
            "string_ml_double": [
                ["\\\\.", "string.escape"],
                ["\"\"\"", "string.quote", "@pop"],
                ["[^\\\\\"]+", "string"],
                ["\"", "string"]
            ],
            "string_ml_single": [
                ["'''", "string.quote", "@pop"],
                ["[^']+", "string"],
                ["'", "string"]
            ]
        }
    }))
}

fn toml_language_configuration() -> JsValue {
    json_to_js(&serde_json::json!({
        "comments": { "lineComment": "#" },
        "brackets": [["[", "]"], ["{", "}"]],
        "autoClosingPairs": [
            { "open": "[", "close": "]" },
            { "open": "{", "close": "}" },
            { "open": "\"", "close": "\"", "notIn": ["string", "comment"] },
            { "open": "'", "close": "'", "notIn": ["string", "comment"] }
        ],
        "surroundingPairs": [
            { "open": "[", "close": "]" },
            { "open": "{", "close": "}" },
            { "open": "\"", "close": "\"" },
            { "open": "'", "close": "'" }
        ]
    }))
}

/// `panel-kit-dark` theme: the crate's `:root` palette mapped onto Monaco
/// theme keys (colours are duplicated here as hex — Monaco themes are data
/// handed to JS, so CSS variables cannot be referenced).
fn panel_kit_dark_theme() -> JsValue {
    json_to_js(&serde_json::json!({
        "base": "vs-dark",
        "inherit": true,
        "rules": [
            { "token": "comment", "foreground": "7a7a7a" },
            { "token": "string", "foreground": "5ef38c" },
            { "token": "string.quote", "foreground": "5ef38c" },
            { "token": "string.escape", "foreground": "27c93f" },
            { "token": "keyword", "foreground": "ff5fc3" },
            { "token": "number", "foreground": "ffbd2e" },
            { "token": "type.identifier", "foreground": "ffbd2e" },
            { "token": "operator", "foreground": "3b9bff" },
            { "token": "delimiter", "foreground": "7a7a7a" }
        ],
        "colors": {
            "editor.background": "#0d0d0d",
            "editor.foreground": "#ededed",
            "editor.lineHighlightBackground": "#26262640",
            "editor.lineHighlightBorder": "#00000000",
            "editorLineNumber.foreground": "#3a3a3a",
            "editorLineNumber.activeForeground": "#7a7a7a",
            "editorCursor.foreground": "#5ef38c",
            "editor.selectionBackground": "#3a3a3a80",
            "editor.inactiveSelectionBackground": "#3a3a3a40",
            "editorWhitespace.foreground": "#3a3a3a",
            "editorIndentGuide.background1": "#262626",
            "editorIndentGuide.activeBackground1": "#3a3a3a",
            "editorBracketMatch.border": "#5ef38c",
            "editorBracketMatch.background": "#5ef38c22",
            "editorWidget.background": "#0d0d0d",
            "editorWidget.border": "#3a3a3a",
            "editorHoverWidget.background": "#0d0d0d",
            "editorHoverWidget.border": "#3a3a3a",
            "editorSuggestWidget.background": "#0d0d0d",
            "editorSuggestWidget.border": "#3a3a3a",
            "focusBorder": "#5ef38c",
            "input.background": "#0a0a0a",
            "input.border": "#3a3a3a"
        }
    }))
}
