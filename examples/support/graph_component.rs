//! Host mounting for the `graph` external component contract.
//!
//! One component, three bodies, chosen by what the host can actually run: the
//! vendored WebGPU graph view when `component_satisfied` holds, otherwise the
//! spec's `fallback` — panel-kit's own SVG painter over the same positions, or
//! an honest notice. The host solves positions; every new target tweens from
//! the current arrangement, so a topos switch animates on every surface.
//!
//! `Force` arrangements arrive solved by `graph_layout::solve` and then keep
//! relaxing live: the GPU engine runs with every node's own mass and
//! repulsion and every edge's own weight and rest length ([`node_physics`],
//! [`edge_physics`]), under options derived from the CPU solve's constants
//! ([`force_options`]) so both reach the same equilibrium. A regime switch
//! re-tunes the running simulation in place; it never re-seeds it.

use dioxus::prelude::*;
use graph_layouts::{AmphiphileOptions, EdgePhysics, GpuForceOptions, NodePhysics, Species};
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

use graph_layout::{FORCE_LENGTH, FORCE_MIN_MASS, FORCE_SEED_RADIUS, Tween, fibonacci_sphere, project_2d};

/// Milliseconds the renderer spends walking from the old arrangement to the
/// codomain's, unless the page asks for reduced motion.
pub const TRANSITION_MS: u32 = 800;

/// Milliseconds between tween frames.
const FRAME_MS: i32 = 16;

/// Mount the `graph` contract for `component` over `model`.
///
/// `target` is the arrangement the host wants drawn — the codomain's solved
/// layout when the topos switches. Every new target tweens from the live
/// arrangement. On WebGPU a `Force` graph mounts on the solve's seed sphere
/// and assembles in front of the viewer under the model's physics; later
/// targets (a re-solve, a re-read site) tween from wherever the simulation is.
#[component]
pub fn GraphComponent(
    model: GraphModel,
    layout: GraphLayout,
    target: Vec<[f32; 3]>,
    selected: Option<String>,
    on_event: EventHandler<ComponentEvent>,
    component: ExternalComponentSpec,
) -> Element {
    let live: Signal<Vec<[f32; 3]>> = use_signal(|| target.clone());
    let mut webgpu = use_signal(|| false);
    // `use_reactive` re-runs each memo when its prop changes; a bare memo over
    // a moved prop would keep the first render's value forever.
    let model = use_memo(use_reactive((&model,), |(model,)| model));
    let layout = use_memo(use_reactive((&layout,), |(layout,)| layout));
    let wanted = use_memo(use_reactive((&target,), |(target,)| target));
    let chosen = use_memo(use_reactive((&selected,), |(selected,)| selected));
    let spec = use_memo(use_reactive((&component,), |(component,)| component));

    use_effect(move || {
        let request = wanted.read().clone();
        let from = render::with_host(|host| {
            host.pipes.positions_cpu().chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect::<Vec<_>>()
        })
        .filter(|positions| positions.len() == request.len())
        .unwrap_or_else(|| live.peek().clone());
        if request.is_empty() || request == from {
            return;
        }
        let tween = Tween::new(from, request, TRANSITION_MS, reduced_motion());
        drive(live, model, layout, tween, now_ms());
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
        render::set_after_mount(mounted);
        let graph = model.peek();
        let seed = match layout.peek().algorithm {
            LayoutAlgorithm::Force | LayoutAlgorithm::Amphiphile => fibonacci_sphere(graph.nodes.len(), FORCE_SEED_RADIUS),
            _ => live.peek().clone(),
        };
        mount(&graph, &seed);
    });
    // Colours, sizes and physics follow the model; a regime switch changes all
    // three without touching the positions, so the running sim is re-tuned
    // in place.
    use_effect(move || {
        let graph = model.read();
        let (colors, sizes) = (node_colors(&graph), node_sizes(&graph));
        render::with_host(|host| {
            let (pipes, queue) = host.pipes_and_queue();
            pipes.update_colors(queue, colors);
            pipes.update_sizes(queue, sizes);
        });
        settle(&graph, &layout.peek());
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
/// frame to the GPU through `RenderHost::apply_positions` with the simulation
/// paused; the landed arrangement is then settled per [`settle`].
fn drive(
    mut live: Signal<Vec<[f32; 3]>>,
    model: Memo<GraphModel>,
    layout: Memo<GraphLayout>,
    tween: Tween,
    start: u32,
) {
    let elapsed = now_ms().saturating_sub(start);
    let frame = tween.at(elapsed);
    live.set(frame.clone());
    let flat: Vec<f32> = frame.iter().flat_map(|p| p.to_vec()).collect();
    render::set_sim_running(false);
    let _ = render::with_host(|host| host.apply_positions(&flat));
    if tween.is_complete(elapsed) {
        settle(&model.read(), &layout.read());
        return;
    }
    let next = tween.clone();
    schedule_after(FRAME_MS, move || drive(live, model, layout, next, start));
}

/// Run `tick` after `delay_ms`, off the browser too.
fn schedule_after(delay_ms: i32, tick: impl FnOnce() + 'static) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let once = Closure::once(tick);
    if window
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            once.as_ref().unchecked_ref(),
            delay_ms,
        )
        .is_err()
    {
        return;
    }
    once.forget();
}

/// Mount the graph view's scene over `positions`, and publish
/// `window.panelKitGraphState()` — `{"running", "halted", "max_ke",
/// "positions": [x, y, z, …]}` from the renderer's live mirror, as JSON, for
/// headless checks that measure the arrangement and the simulation.
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
        node_physics: Some(node_physics(model)),
        edge_physics: Some(edge_physics(model)),
    });
    if let Some(window) = web_sys::window() {
        let read = Closure::<dyn Fn() -> String>::new(|| {
            render::with_host(|host| {
                format!(
                    "{{\"running\":{},\"halted\":{},\"max_ke\":{},\"settings\":{},\"positions\":{:?}}}",
                    host.pipes.sim_running(),
                    host.pipes.is_halted(),
                    host.pipes.last_max_ke(),
                    host.pipes.layout_settings_json().unwrap_or(serde_json::Value::Null),
                    host.pipes.positions_cpu()
                )
            })
            .unwrap_or_else(|| "{\"running\":false,\"halted\":false,\"max_ke\":0,\"positions\":[]}".into())
        });
        let _ = js_sys::Reflect::set(
            window.as_ref(),
            &wasm_bindgen::JsValue::from_str("panelKitGraphState"),
            read.as_ref(),
        );
        read.forget();
    }
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

/// Milliseconds after waking the simulation at which the camera is refitted:
/// the arrangement inflates fastest at first and has settled by the last.
const SETTLE_REFITS_MS: [i32; 3] = [1500, 5000, 12000];

/// Frame the landed arrangement. A static layout stays as solved; `Force`
/// hands the spring-electrical engine the model's physics, `Amphiphile`
/// swaps the engine to self-assembly over the model's species; either way
/// the simulation relaxes live from here. A model change on the same
/// algorithm re-tunes the running engine in place.
fn settle(model: &GraphModel, layout: &GraphLayout) {
    frame();
    match layout.algorithm {
        LayoutAlgorithm::Force => {
            render::with_host(|host| {
                host.pipes.update_layout_options(force_options());
                let _ = host.update_node_physics(node_physics(model));
                let _ = host.update_edge_physics(edge_physics(model));
                host.pipes.wake_physics_layout();
            });
        }
        LayoutAlgorithm::Amphiphile => {
            let species = species_of(model);
            render::with_host(|host| {
                if host.update_species(species.clone()).is_err() {
                    host.swap_to_amphiphile(AmphiphileOptions::default(), species);
                }
                host.pipes.wake_physics_layout();
            });
        }
        _ => {
            render::set_sim_running(false);
            return;
        }
    }
    render::set_sim_running(true);
    for delay in SETTLE_REFITS_MS {
        schedule_after(delay, frame);
    }
}

/// The engine species of every node from the base stalk the regime pulls
/// back: hydrophilic heads, hydrophobic tails, everything else solvent.
pub fn species_of(model: &GraphModel) -> Vec<Species> {
    model
        .nodes
        .iter()
        .map(|node| match node.species.as_str() {
            "hydrophilic" => Species::Head,
            "hydrophobic" => Species::Tail,
            _ => Species::Solvent,
        })
        .collect()
}

/// Per-node inertia and repulsion charge for the engine. Charges are
/// normalised to unit mean, as the CPU solve normalises them, so the
/// engine's `repulsion` option sets the scale and the model sets the ratios.
pub fn node_physics(model: &GraphModel) -> Vec<NodePhysics> {
    let declared: Vec<f64> = model
        .nodes
        .iter()
        .map(|node| if node.repulsion.is_finite() && node.repulsion > 0.0 { node.repulsion } else { 0.0 })
        .collect();
    let positive = declared.iter().filter(|value| **value > 0.0).count();
    let unit = if positive > 0 { declared.iter().sum::<f64>() / model.nodes.len() as f64 } else { 1.0 };
    model
        .nodes
        .iter()
        .zip(&declared)
        .map(|(node, repulsion)| NodePhysics {
            mass: if node.mass.is_finite() { node.mass.max(FORCE_MIN_MASS) } else { 1.0 } as f32,
            repulsion: if *repulsion > 0.0 { repulsion / unit } else { 1.0 } as f32,
        })
        .collect()
}

/// Per-edge spring multiplier and rest length (in canonical lengths) for the
/// engine, in the model's edge order.
pub fn edge_physics(model: &GraphModel) -> Vec<EdgePhysics> {
    model
        .edges
        .iter()
        .map(|edge| EdgePhysics {
            weight: if edge.weight.is_finite() { edge.weight.max(0.0) } else { 1.0 } as f32,
            rest_length: if edge.rest_length.is_finite() { edge.rest_length.max(0.0) } else { 1.0 } as f32,
        })
        .collect()
}

/// Spring stiffness the engine runs at, per unit of edge `weight`. The base's
/// heaviest bond (`weight` 5) then reaches the engine's default stiffness of 1,
/// which its integrator (`dt` 0.1, `damping` 0.9, one-rest-length velocity
/// clamp) is tuned for.
const ENGINE_STIFFNESS: f64 = 0.2;
/// Pull toward the origin per world unit. Weak enough that two assembled
/// components repel each other apart rather than being gathered into one blob,
/// strong enough that nothing drifts out of frame.
const ENGINE_GRAVITY: f64 = 2.0e-4;

/// Engine options for the model's physics in world units. The law differs from
/// the CPU solve's (Coulomb `1/d²` repulsion against Fruchterman-Reingold
/// `1/d`), so the two are made to agree where the engine is known to be
/// stable rather than at one distance: `spring_len` is one canonical length,
/// `spring_k` is [`ENGINE_STIFFNESS`], and `repulsion` keeps the engine's
/// default ratio of repulsion to spring force at the rest length
/// (`repulsion / (spring_k · spring_len³)`, 4000 / 400³ at the defaults), so
/// a shell inflates and neighbours separate as the engine's own graphs do.
/// Per-node charges and per-edge multipliers then carry the base's ratios.
pub fn force_options() -> GpuForceOptions {
    let defaults = GpuForceOptions::default();
    let ratio = f64::from(defaults.repulsion) / (f64::from(defaults.spring_k) * f64::from(defaults.spring_len).powi(3));
    GpuForceOptions {
        spring_k: ENGINE_STIFFNESS as f32,
        spring_len: FORCE_LENGTH as f32,
        repulsion: (ratio * ENGINE_STIFFNESS * FORCE_LENGTH.powi(3)) as f32,
        repulsion_radius: (4.0 * FORCE_LENGTH) as f32,
        gravity: ENGINE_GRAVITY as f32,
        ..defaults
    }
}

/// After every canvas mount: the engine options the solve's laws derive
/// (the host did not exist when `settle` first ran) and the camera.
fn mounted() {
    render::with_host(|host| host.pipes.update_layout_options(force_options()));
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
