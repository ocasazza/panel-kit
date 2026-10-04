//! omp auto-loop host — the loop's trace and control state as two topos
//! regimes over one glued site, with section edits written back to the loop.
//!
//! Run with: `dx serve --example autoloop --platform web`, with the loop's
//! `/api/` proxied to this origin (Dioxus.toml `[[web.proxy]]`, backend
//! `http://127.0.0.1:8798/api/`).
//!
//! - The host reads `GET /api/trace` (pest grammar `autoloop-trace`) and
//!   `GET /api/site` (json grammar `autoloop-control`), glues them along
//!   `repo`, and paints each panel from the active regime's global sections.
//! - A row of an editable control table opens its offered edits. Writing one
//!   runs `revise` → `write_intent` and POSTs the emitted bytes to
//!   `/api/site`; the loop validates and folds them, and the host reloads.
//! - `t` (or the top bar) switches regime; `r` reloads.

use std::sync::LazyLock;

use dioxus::events::{Key, KeyboardEvent, PointerEvent as DioxusPointerEvent};
use dioxus::prelude::*;
use panel_kit::CSS;
use panel_kit_core::reducer::{Viewport, WorkspaceEvent};
use panel_kit_core::widgets::ContentSpec;
use panel_kit_core::Units;
use panel_kit_grammar::Topos;
use wasm_bindgen::{JsCast, JsValue};

#[allow(dead_code, unused_imports)]
#[path = "support/composable_workspace.rs"]
mod composable_workspace;
#[allow(dead_code)]
#[path = "support/spec_workspace.rs"]
mod spec_workspace;
#[allow(dead_code)]
#[path = "support/autoloop_host.rs"]
mod autoloop_host;
#[allow(dead_code)]
#[path = "support/section_view.rs"]
mod section_view;

use autoloop_host::{AutoloopHost, Specs, CONTROL, TRACE};
use section_view::{EditDraft, RowSelect};

const BASE: &str = include_str!(env!("PANEL_KIT_TOPOS_PHYSICS"));
const TRACE_TOPOS: &str = include_str!(env!("PANEL_KIT_AUTOLOOP_TOPOS_TRACE"));
const CONTROL_TOPOS: &str = include_str!(env!("PANEL_KIT_AUTOLOOP_TOPOS_CONTROL"));
const TRACE_PHYSICS: &str = include_str!(env!("PANEL_KIT_AUTOLOOP_MORPHISM_TRACE_PHYSICS"));
const CONTROL_PHYSICS: &str = include_str!(env!("PANEL_KIT_AUTOLOOP_MORPHISM_CONTROL_PHYSICS"));
const TRACE_GRAMMAR: &str = include_str!(env!("PANEL_KIT_AUTOLOOP_GRAMMAR_TRACE"));
const CONTROL_GRAMMAR: &str = include_str!(env!("PANEL_KIT_AUTOLOOP_GRAMMAR_CONTROL"));

const HOST_CSS: &str = "
.topbar { flex-wrap: wrap; height: auto; min-height: 36px; gap: .45rem; }
.topbar h1 { font-size: .74rem; text-transform: uppercase; }
.topbar button { background: var(--bg); color: var(--fg); border: 1px solid var(--line2);
  border-radius: 3px; padding: .15rem .5rem; font-size: .72rem; cursor: pointer; }
.topbar button:hover { border-color: var(--fg); }
.topbar button.active { background: var(--fg); color: var(--bg); }
.topbar .axis-buttons { display: flex; gap: .35rem; }
.topbar .status { color: var(--accent); font-size: .7rem; }
.topbar .status.error { color: var(--red); }
.pk-content-unbound { color: var(--yellow); font-size: .78rem; }
";

static SPECS: Specs<'static> = Specs {
    base: BASE,
    trace_topos: TRACE_TOPOS,
    control_topos: CONTROL_TOPOS,
    trace_physics: TRACE_PHYSICS,
    control_physics: CONTROL_PHYSICS,
    trace_grammar: TRACE_GRAMMAR,
    control_grammar: CONTROL_GRAMMAR,
};

/// The trace regime's workspace spec, for the spec resolver hook, which
/// requires `&'static str`.
static INITIAL_WORKSPACE: LazyLock<String> =
    LazyLock::new(|| Topos::from_json_str(TRACE_TOPOS).expect("the trace topos is embedded").workspace_json());

type Host = Signal<Result<AutoloopHost, String>>;
/// `(is_error, message)` for the top bar.
type Status = Signal<(bool, String)>;

fn main() {
    dioxus::launch(App);
}

fn browser_viewport() -> Viewport {
    let (width, height) = panel_kit::surface::viewport_size();
    Viewport { width, height, units: Units::CssPx }
}

fn js_error(value: JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}

/// One same-origin request; a body makes it a JSON POST. Returns status and
/// body text.
async fn request(url: &str, body: Option<Vec<u8>>) -> Result<(u16, String), String> {
    let init = web_sys::RequestInit::new();
    if let Some(body) = body {
        let text = String::from_utf8(body).map_err(|e| e.to_string())?;
        let headers = web_sys::Headers::new().map_err(js_error)?;
        headers.set("content-type", "application/json").map_err(js_error)?;
        init.set_method("POST");
        init.set_headers(&headers);
        init.set_body(&JsValue::from_str(&text));
    }
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(url, &init))
        .await
        .map_err(js_error)?
        .dyn_into()
        .map_err(js_error)?;
    let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(js_error)?)
        .await
        .map_err(js_error)?
        .as_string()
        .unwrap_or_default();
    Ok((response.status(), text))
}

/// Read both inputs and load them into the host; on failure the last good
/// site stays painted and the status names the failure.
async fn reload(mut host: Host, mut status: Status) {
    let fetched = async {
        let (trace_status, trace) = request("/api/trace", None).await?;
        let (site_status, site) = request("/api/site", None).await?;
        if trace_status != 200 || site_status != 200 {
            return Err(format!("loop answered {trace_status} / {site_status}"));
        }
        Ok((trace, site))
    }
    .await;
    let loaded = fetched.and_then(|(trace, site)| {
        host.with_mut(|host| host.as_mut().map_err(|e| e.clone()).and_then(|host| host.load(&trace, &site)))
    });
    match loaded {
        Ok(()) => status.set((false, "loaded".to_owned())),
        Err(error) => status.set((true, error)),
    }
}

/// A draft of `edit` on `object` from the host's current state.
fn open_draft(host: Host, binding: &str, object: &str, edit: &str) -> Option<EditDraft> {
    let host = host.read();
    let host = host.as_ref().ok()?;
    Some(EditDraft::open(binding, host.object(object)?, edit, host.edit(edit)?))
}

/// Paint one binding from the active regime; rows of a table that offers
/// edits open a draft on their object.
fn host_content(host: Host, mut draft: Signal<Option<EditDraft>>, id: &str, content: &ContentSpec) -> Element {
    let state = host.read();
    let Ok(state) = state.as_ref() else {
        return rsx! { div { class: "pk-content pk-content-unbound", "host failed to start" } };
    };
    let Some(section) = state.resolve(id) else {
        let what = if state.is_loaded() { "unbound" } else { "loading" };
        return rsx! {
            div { class: "pk-content pk-content-unbound", role: "group", aria_label: "{what} binding {id}", "{what}: {id}" }
        };
    };
    let rows = state.row_edits(id).map(|rows| {
        let offered: Vec<(String, Option<String>)> =
            rows.iter().map(|row| (row.object.clone(), row.edits.first().cloned())).collect();
        let selected = draft
            .read()
            .as_ref()
            .filter(|d| d.binding == id)
            .and_then(|d| offered.iter().position(|(object, _)| *object == d.object));
        let binding = id.to_owned();
        RowSelect {
            selected,
            on_row: EventHandler::new(move |row: usize| {
                if let Some((object, Some(edit))) = offered.get(row) {
                    draft.set(open_draft(host, &binding, object, edit));
                }
            }),
        }
    });
    section_view::paint(section, content, rows, None)
}

/// The open draft's form; writing POSTs the emitted bytes and reloads.
fn draft_form(host: Host, mut draft: Signal<Option<EditDraft>>, mut status: Status) -> Element {
    let Some(open) = draft.read().clone() else { return rsx! {} };
    let state = host.read();
    let Ok(state) = state.as_ref() else { return rsx! {} };
    let Some(edit) = state.edit(&open.edit) else { return rsx! {} };
    let offered: Vec<(String, String)> = state
        .row_edits(&open.binding)
        .and_then(|rows| rows.iter().find(|row| row.object == open.object))
        .map(|row| {
            row.edits.iter().map(|id| (id.clone(), state.edit(id).map_or(id.clone(), |e| e.label.clone()))).collect()
        })
        .unwrap_or_default();
    let title = state.object(&open.object).map(|o| o.title.clone()).unwrap_or_default();
    let (binding, object) = (open.binding.clone(), open.object.clone());
    let on_choose = EventHandler::new(move |edit: String| draft.set(open_draft(host, &binding, &object, &edit)));
    let on_submit = EventHandler::new(move |submitted: EditDraft| {
        let written = host
            .read()
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|h| h.write(&submitted.edit, &submitted.object, submitted.values.clone()));
        let bytes = match written {
            Ok(bytes) => bytes,
            Err(error) => return status.set((true, error)),
        };
        spawn(async move {
            match request("/api/site", Some(bytes)).await {
                Ok((202, _)) => {
                    draft.set(None);
                    reload(host, status).await;
                    status.set((false, format!("{} written", submitted.edit)));
                }
                Ok((code, body)) => status.set((true, format!("loop refused ({code}): {body}"))),
                Err(error) => status.set((true, error)),
            }
        });
    });
    section_view::edit_form(draft, &title, &offered, edit, on_choose, on_submit)
}

#[component]
fn App() -> Element {
    let mut host = use_signal(|| AutoloopHost::new(&SPECS));
    let status = use_signal(|| (false, "loading".to_owned()));
    let mut draft = use_signal(|| None::<EditDraft>);
    let initial = spec_workspace::use_spec_workspace(INITIAL_WORKSPACE.as_str());
    let mut workspace = use_signal(move || initial.clone());
    let header_clicks = use_signal(|| 0_u32);

    use_hook(move || {
        spawn(reload(host, status));
    });

    // The viewport observer captures the workspace it mounted with; a regime
    // switch installs a new one, so the signal is read per event instead.
    let observed = workspace;
    let _viewport = panel_kit::surface::observe_viewport(EventHandler::new(move |size: Viewport| {
        let current = observed.read();
        spec_workspace::workspace_event_handler(&current).call(WorkspaceEvent::ViewportChanged {
            size,
            policy: current.resolved.surface.resize_policy,
        });
    }));

    let mut switch_topos = move |id: &str| {
        let Some(json) = host.read().as_ref().ok().and_then(|h| h.workspace_json(id)) else { return };
        if !host.with_mut(|h| h.as_mut().is_ok_and(|h| h.set_active(id))) {
            return;
        }
        draft.set(None);
        // Every read guard ends with its statement: the switch writes the
        // workspace signal, and a live read of it would panic the borrow.
        let switched = {
            let current = workspace.read();
            spec_workspace::switch_workspace_spec(&current, &json)
        };
        workspace.set(switched);
        let current = workspace.read();
        spec_workspace::workspace_event_handler(&current).call(WorkspaceEvent::ViewportChanged {
            size: browser_viewport(),
            policy: current.resolved.surface.resize_policy,
        });
    };

    let key_workspace = workspace.read().clone();
    let mut on_key = move |event: KeyboardEvent| {
        let mut handled = false;
        if !panel_kit::input::is_editing() {
            if let Key::Character(character) = event.key() {
                match character.as_str() {
                    "t" => {
                        let next = host.read().as_ref().ok().map(|h| if h.active_id() == TRACE { CONTROL } else { TRACE });
                        if let Some(next) = next {
                            switch_topos(next);
                        }
                        handled = true;
                    }
                    "r" => {
                        spawn(reload(host, status));
                        handled = true;
                    }
                    _ => {}
                }
            }
        }
        if handled {
            event.prevent_default();
        } else {
            spec_workspace::handle_key(&key_workspace, &event);
        }
    };

    let current = workspace.read();
    let emit = spec_workspace::workspace_event_handler(&current);
    let pointer_move_workspace = current.clone();
    let pointer_up_workspace = current.clone();
    let pointer_cancel_workspace = current.clone();
    let wheel_workspace = current.clone();

    let snapshot = current.snapshot.read();
    let mut scratch = current.scratch.borrow_mut();
    let frame = spec_workspace::project_workspace(&current, &snapshot, &mut scratch);
    let root_class = panel_kit::widgets::root::root_class(&frame);
    let workspace_class = spec_workspace::workspace_area_class(&frame);
    let workspace_style = frame.tile_grid.map(panel_kit::widgets::root::tile_grid_style).unwrap_or_default();

    let (active, startup_error) = match host.read().as_ref() {
        Ok(h) => (h.active_id().to_owned(), None),
        Err(error) => (String::new(), Some(error.clone())),
    };
    let (is_error, message) = match startup_error {
        Some(error) => (true, error),
        None => status.read().clone(),
    };
    let topos_buttons: Vec<Element> = [TRACE, CONTROL]
        .into_iter()
        .map(|id| {
            let mut switch = switch_topos;
            rsx! {
                button {
                    key: "topos-{id}",
                    class: if id == active { "active" },
                    aria_pressed: id == active,
                    onclick: move |_| switch(id),
                    "{id}"
                }
            }
        })
        .collect();

    let binding_content = move |id: &str, content: &ContentSpec| host_content(host, draft, id, content);

    rsx! {
        style { {CSS} }
        style { {HOST_CSS} }
        style { {section_view::EDIT_FORM_CSS} }
        div {
            class: "{root_class}",
            tabindex: "0",
            onpointermove: move |event: DioxusPointerEvent| spec_workspace::handle_pointer_move(&pointer_move_workspace, &event),
            onpointerup: move |event: DioxusPointerEvent| spec_workspace::handle_pointer_up(&pointer_up_workspace, &event),
            onpointercancel: move |event: DioxusPointerEvent| spec_workspace::handle_pointer_up(&pointer_cancel_workspace, &event),
            onkeydown: move |event: KeyboardEvent| on_key(event),
            header { class: "topbar",
                h1 { "omp auto-loop" }
                div { class: "axis-buttons", {topos_buttons.into_iter()} }
                button { onclick: move |_| { spawn(reload(host, status)); }, "reload" }
                span { class: if is_error { "status error" } else { "status" }, role: "status", "{message}" }
                span { class: "hint", "t regime · r reload · click a control row to edit it" }
            }
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                onwheel: move |event| spec_workspace::handle_wheel(&wheel_workspace, &event),
                {composable_workspace::web_canary::workspace_contents(&frame, &current.resolved, emit, header_clicks, &binding_content)}
            }
            if current.resolved.chrome.dock {
                {panel_kit::widgets::dock::dock(frame.dock, &current.resolved.catalog, emit, None)}
            }
            {draft_form(host, draft, status)}
        }
    }
}
