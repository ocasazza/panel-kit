//! Browser wheel translation for hosts that draw through a WebGL canvas.
//!
//! Ratzilla/beamterm never report wheel events, so hosts install a DOM `wheel`
//! listener on the canvas element and translate it into terminal cells here.
//! Cell size comes from the canvas client rect divided by the grid the host is
//! rendering, so the mapping survives window resizes.

use ratzilla::web_sys::js_sys::{Function, Reflect};
use ratzilla::web_sys::wasm_bindgen::{closure::Closure, JsCast, JsValue};
use ratzilla::web_sys::{Element, MouseEvent};

use crate::input::WheelDelta;

/// One wheel gesture resolved into terminal cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelCell {
    /// Pointer column in terminal cells, relative to the canvas origin.
    pub col: f64,
    /// Pointer row in terminal cells, relative to the canvas origin.
    pub row: f64,
    /// Cells scrolled toward the right, from the DOM `deltaX`.
    pub delta_x: f64,
    /// Cells scrolled toward the bottom, from the DOM `deltaY`.
    pub delta_y: f64,
    /// Whether the shift key was held, which makes the gesture horizontal.
    pub shift: bool,
}

impl WheelCell {
    /// The gesture as wheel deltas, applying the shift modifier.
    pub fn delta(&self) -> WheelDelta {
        WheelDelta::from_deltas(self.delta_x, self.delta_y, self.shift)
    }
}

/// Install a `wheel` listener on `container_id` that reports terminal-cell
/// gestures, and return without detaching it.
///
/// `grid` supplies the live `(cols, rows)` grid so a resized canvas remaps on
/// the next gesture. `on_wheel` returns whether the gesture was consumed;
/// `preventDefault` is called only then, leaving page scrolling intact when a
/// gesture was ignored.
pub fn install_cell_wheel<F, G>(container_id: &str, grid: G, mut on_wheel: F) -> Result<(), JsValue>
where
    F: FnMut(WheelCell) -> bool + 'static,
    G: Fn() -> (f64, f64) + 'static,
{
    let document = ratzilla::web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
    let canvas = document
        .get_element_by_id(container_id)
        .ok_or_else(|| JsValue::from_str("wheel container unavailable"))?;
    let listener_canvas = canvas.clone();
    let listener = Closure::wrap(Box::new(move |event: JsValue| {
        let Some(gesture) = cell_wheel(&event, &listener_canvas, grid()) else {
            return;
        };
        if gesture.delta_x == 0.0 && gesture.delta_y == 0.0 {
            return;
        }
        if on_wheel(gesture) {
            if let Some(prevent) = method(&event, "preventDefault") {
                let _ = prevent.call0(&event);
            }
        }
    }) as Box<dyn FnMut(JsValue)>);
    canvas.add_event_listener_with_callback("wheel", listener.as_ref().unchecked_ref())?;
    listener.forget();
    Ok(())
}

/// Translate one DOM wheel event on `canvas` into terminal cells, or `None` when
/// the client rect or grid cannot produce a positive cell size.
fn cell_wheel(event: &JsValue, canvas: &Element, grid: (f64, f64)) -> Option<WheelCell> {
    let rect = canvas.get_bounding_client_rect();
    let (cols, rows) = grid;
    let cell_w = rect.width() / cols;
    let cell_h = rect.height() / rows;
    if !cell_w.is_finite() || !cell_h.is_finite() || cell_w <= 0.0 || cell_h <= 0.0 {
        return None;
    }

    let pointer = event.clone().dyn_into::<MouseEvent>().ok()?;
    Some(WheelCell {
        col: (f64::from(pointer.client_x()) - rect.left()) / cell_w,
        row: (f64::from(pointer.client_y()) - rect.top()) / cell_h,
        delta_x: number(event, "deltaX")? / cell_w,
        delta_y: number(event, "deltaY")? / cell_h,
        shift: boolean(event, "shiftKey"),
    })
}

fn number(event: &JsValue, key: &str) -> Option<f64> {
    Reflect::get(event, &JsValue::from_str(key))
        .ok()?
        .as_f64()
}

fn boolean(event: &JsValue, key: &str) -> bool {
    Reflect::get(event, &JsValue::from_str(key))
        .map(|value| value.is_truthy())
        .unwrap_or(false)
}

fn method(event: &JsValue, key: &str) -> Option<Function> {
    Reflect::get(event, &JsValue::from_str(key))
        .ok()?
        .dyn_into::<Function>()
        .ok()
}
