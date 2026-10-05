//! Host mounting for the `graph` external component contract.
//!
//! One component, three bodies, chosen by what the host can actually run: the
//! vendored WebGPU graph view when `component_satisfied` holds, otherwise the
//! spec's `fallback` — panel-kit's own SVG painter over the same positions, or
//! an honest notice. The host solves positions; every new target tweens from
//! the current arrangement, so a topos switch animates on every surface.
//!
//! `Force` arrangements arrive solved by `graph_layout::solve`, which honours
//! each node's and edge's own physics. The GPU engine draws that arrangement
//! and never re-simulates it: its force options hold one global tuning, which
//! would flatten the per-sort physics a membrane assembles from.

use dioxus::prelude::*;
use graph_view::render;
use panel_kit::widgets::graph::{graph_notice, graph_svg};
use panel_kit_core::spec::{
    Capability, ComponentEvent, ComponentFallback, ExternalComponentSpec, component_satisfied,
};
use panel_kit_core::widgets::graph::{GraphLayout, GraphModel, GraphView, LayoutAlgorithm};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

#[path = "graph_layout.rs"]
pub mod graph_layout;

use graph_layout::{FORCE_SEED_RADIUS, Tween, fibonacci_sphere, project_2d};

/// Milliseconds the renderer spends walking from the old arrangement to the
/// codomain's, unless the page asks for reduced motion.
pub const TRANSITION_MS: u32 = 800;

/// Milliseconds between tween frames.
const FRAME_MS: i32 = 16;

/// Mount the `graph` contract for `component` over `model`.
///
/// `target` is the arrangement the host wants drawn — the codomain's solved
/// layout when the topos switches. Every change tweens towards it. A `Force`
/// graph first paints the solve's own seed sphere, so it visibly assembles.
#[component]
pub fn GraphComponent(
    model: GraphModel,
    layout: GraphLayout,
    target: Vec<[f32; 3]>,
    selected: Option<String>,
    on_event: EventHandler<ComponentEvent>,
    component: ExternalComponentSpec,
) -> Element {
    let live: Signal<Vec<[f32; 3]>> = use_signal(|| match layout.algorithm {
        LayoutAlgorithm::Force => fibonacci_sphere(target.len(), FORCE_SEED_RADIUS),
        _ => target.clone(),
    });
    let mut webgpu = use_signal(|| false);
    // `use_reactive` re-runs each memo when its prop changes; a bare memo over
    // a moved prop would keep the first render's value forever.
    let model = use_memo(use_reactive((&model,), |(model,)| model));
    let wanted = use_memo(use_reactive((&target,), |(target,)| target));
    let chosen = use_memo(use_reactive((&selected,), |(selected,)| selected));
    let spec = use_memo(use_reactive((&component,), |(component,)| component));

    use_effect(move || {
        let request = wanted.read().clone();
        let from = live.peek().clone();
        if request.is_empty() || request == from {
            return;
        }
        let tween = Tween::new(from, request, TRANSITION_MS, reduced_motion());
        drive(live, tween, now_ms());
    });

    // Mount once the WebGPU canvas is in the DOM and again only when the node
    // count changes: a topos switch keeps the node set, so it recolours and
    // resizes in place instead of remounting over the tween.
    let nodes = use_memo(move || model.read().nodes.len());
    use_effect(move || {
        if !webgpu() {
            return;
        }
        nodes();
        render::set_after_mount(frame);
        mount(&model.peek(), &live.peek());
    });
    use_effect(move || {
        let graph = model.read();
        let (colors, sizes) = (node_colors(&graph), node_sizes(&graph));
        render::with_host(|host| {
            let (pipes, queue) = host.pipes_and_queue();
            pipes.update_colors(queue, colors);
            pipes.update_sizes(queue, sizes);
        });
        settle();
    });

    use_effect(move || {
        let index = chosen
            .read()
            .as_deref()
            .and_then(|id| model.read().nodes.iter().position(|node| node.id == *id))
            .map(|index| index as u32);
        render::set_selected_node(index);
    });

    use_effect(move || {
        if component_satisfied(&spec.read(), &[Capability::WebGpu]) {
            spawn(async move {
                webgpu.set(detect_webgpu().await);
            });
        }
    });

    let view = GraphView {
        nodes: &model.read().nodes,
        edges: &model.read().edges,
    };

    if webgpu() && component_satisfied(&spec.read(), &[Capability::WebGpu]) {
        let ids: Vec<String> = model.read().nodes.iter().map(|node| node.id.clone()).collect();
        let hover_ids = ids.clone();
        let click_ids = ids.clone();
        let hover = on_event;
        let click = on_event;
        let leave = on_event;
        rsx! {
            style { "{graph_view::CSS}" }
            style { "{panel_kit::widgets::graph::CSS}" }
            graph_view::GraphView {
                node_count: ids.len() as u32,
                on_hover: move |(x, y): (f32, f32)| {
                    let hovered = render::pick(x, y);
                    render::set_hover_feedback(hovered, render::pick_edge(x, y));
                    let node = hovered.and_then(|index| hover_ids.get(index as usize).cloned());
                    hover.call(ComponentEvent::Hover { node });
                },
                on_click: move |(x, y): (f32, f32)| {
                    if let Some(node) = render::pick(x, y).and_then(|index| click_ids.get(index as usize).cloned()) {
                        click.call(ComponentEvent::Select { node });
                    }
                },
                on_leave: move |_| {
                    render::set_hover_feedback(None, None);
                    leave.call(ComponentEvent::Hover { node: None });
                },
            }
        }
    } else {
        match spec.read().fallback {
            ComponentFallback::Cells => rsx! {
                style { "{panel_kit::widgets::graph::CSS}" }
                {graph_svg(
                    view,
                    &project_2d(&live.read()),
                    chosen.read().as_deref(),
                    EventHandler::new(move |node: String| {
                        on_event.call(ComponentEvent::Select { node })
                    }),
                )}
            },
            ComponentFallback::Notice => rsx! {
                {graph_notice(
                    "Graph renderer unavailable",
                    "this browser has no WebGPU, and the graph contract's fallback is a notice",
                )}
            },
        }
    }
}

/// One tween step, then the next, until the arrangement lands.
///
/// Each step updates `live` (read by the SVG fallback) and writes the same
/// frame to the GPU through `RenderHost::apply_positions`; the landed
/// arrangement is then framed per [`settle`].
fn drive(mut live: Signal<Vec<[f32; 3]>>, tween: Tween, start: u32) {
    let elapsed = now_ms().saturating_sub(start);
    let frame = tween.at(elapsed);
    live.set(frame.clone());
    let flat: Vec<f32> = frame.iter().flat_map(|p| p.to_vec()).collect();
    render::set_sim_running(false);
    let _ = render::with_host(|host| host.apply_positions(&flat));
    if tween.is_complete(elapsed) {
        settle();
        return;
    }
    let next = tween.clone();
    schedule(move || drive(live, next, start));
}

/// Run `tick` on the next frame's timeout, off the browser too.
fn schedule(tick: impl FnOnce() + 'static) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let once = Closure::once(tick);
    if window
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            once.as_ref().unchecked_ref(),
            FRAME_MS,
        )
        .is_err()
    {
        return;
    }
    once.forget();
}

/// Mount the graph view's scene over `positions`.
fn mount(model: &GraphModel, positions: &[[f32; 3]]) {
    render::mount_canvas(render::Scene {
        positions: positions.iter().flat_map(|p| p.to_vec()).collect(),
        edges: model
            .edges
            .iter()
            .flat_map(|edge| [edge.source, edge.target])
            .collect(),
        colors: node_colors(model),
        sizes: node_sizes(model),
    });
}

/// RGBA per node from the model's encoding.
fn node_colors(model: &GraphModel) -> Vec<f32> {
    model
        .nodes
        .iter()
        .flat_map(|node| {
            let (r, g, b) = node.color;
            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
        })
        .collect()
}

/// Node sizes mapped onto jump-cannon's 2..10 px range, largest model size on top.
fn node_sizes(model: &GraphModel) -> Vec<f32> {
    let heaviest = model
        .nodes
        .iter()
        .map(|node| node.size)
        .fold(f32::MIN_POSITIVE, f32::max);
    model.nodes.iter().map(|node| 2.0 + 8.0 * node.size / heaviest).collect()
}

/// Frame the landed arrangement; the simulation stays paused, so what the host
/// solved is what stays on screen.
fn settle() {
    render::set_sim_running(false);
    frame();
}

/// Room around the framed graph, as a multiple of its extent.
const FRAME_MARGIN: f32 = 1.15;

/// Fit the camera to the graph's box and the panel's aspect. The vendored fit
/// frames the bounding sphere with generous padding, which leaves a wide panel
/// mostly empty and every assembled shape a speck.
fn frame() {
    render::with_host(|host| {
        let (width, height) = host.logical_size();
        let pipes = &mut host.pipes;
        pipes.fit_camera();
        let Some((min, max)) = pipes.bounds() else {
            return;
        };
        if pipes.camera.is_ortho() || width <= 0.0 || height <= 0.0 {
            return;
        }
        let half = (max - min) * 0.5;
        let tan = (pipes.camera.fov_y() * 0.5).tan();
        let reach = (half.y / tan).max(half.x / (tan * width / height));
        pipes.camera.look_at_point((min + max) * 0.5, FRAME_MARGIN * reach + half.z);
    });
}

/// Whether the page asks for reduced motion; false off the browser.
fn reduced_motion() -> bool {
    web_sys::window()
        .and_then(|window| window.match_media("(prefers-reduced-motion: reduce)").ok().flatten())
        .map(|query| query.matches())
        .unwrap_or(false)
}

/// `navigator.gpu` plus a real adapter request: the cheap presence probe
/// cannot tell a blocked adapter from a missing one.
async fn detect_webgpu() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(navigator) = js_sys::Reflect::get(window.as_ref(), &wasm_bindgen::JsValue::from_str("navigator")) else {
        return false;
    };
    let Ok(gpu) = js_sys::Reflect::get(&navigator, &wasm_bindgen::JsValue::from_str("gpu")) else {
        return false;
    };
    if gpu.is_null() || gpu.is_undefined() {
        return false;
    }
    let Ok(request) = js_sys::Reflect::get(&gpu, &wasm_bindgen::JsValue::from_str("requestAdapter")) else {
        return false;
    };
    let request: js_sys::Function = match request.dyn_into() {
        Ok(request) => request,
        Err(_) => return false,
    };
    let Ok(promise) = request.call0(&gpu) else {
        return false;
    };
    let Ok(adapter) = wasm_bindgen_futures::JsFuture::from(promise.unchecked_into::<js_sys::Promise>()).await else {
        return false;
    };
    !adapter.is_null() && !adapter.is_undefined()
}

fn now_ms() -> u32 {
    let Some(window) = web_sys::window() else {
        return 0;
    };
    let Ok(performance) = js_sys::Reflect::get(window.as_ref(), &wasm_bindgen::JsValue::from_str("performance")) else {
        return 0;
    };
    let Ok(now) = js_sys::Reflect::get(&performance, &wasm_bindgen::JsValue::from_str("now")) else {
        return 0;
    };
    let now: js_sys::Function = match now.dyn_into() {
        Ok(now) => now,
        Err(_) => return 0,
    };
    now.call0(&performance)
        .ok()
        .and_then(|value| value.as_f64())
        .map(|ms| ms.max(0.0) as u32)
        .unwrap_or_default()
}

/// `LayoutAlgorithm` params as authored, for the layout text panel.
pub fn describe(layout: &GraphLayout) -> String {
    let params: Vec<String> = layout
        .params
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    format!("{:?} / {:?}", layout.algorithm, params.join(", "))
}
