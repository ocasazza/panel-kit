use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use panel_kit_core::reducer::Viewport;
use wasm_bindgen::{closure::Closure, JsCast};

use super::{viewport_from_size, viewport_size};

/// Registration state for one browser viewport observer primitive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewportObserverState {
    /// The hook has not attempted browser registration yet.
    Pending,
    /// The browser primitive was registered and will be cleaned up on unmount.
    Registered,
    /// The browser primitive is unavailable in this environment.
    Unavailable(&'static str),
    /// The browser primitive rejected registration.
    Failed(String),
}

/// Observable registration state for [`observe_viewport`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewportObserverStatus {
    /// State of the `<html>` [`web_sys::ResizeObserver`] registration.
    pub resize_observer: ViewportObserverState,
    /// State of the fallback `window.resize` listener registration.
    pub window_resize_listener: ViewportObserverState,
}

impl ViewportObserverStatus {
    /// Return the initial state before browser registration runs.
    pub const fn pending() -> Self {
        Self {
            resize_observer: ViewportObserverState::Pending,
            window_resize_listener: ViewportObserverState::Pending,
        }
    }

    /// Whether both viewport change mechanisms registered successfully.
    pub fn is_fully_registered(&self) -> bool {
        matches!(self.resize_observer, ViewportObserverState::Registered)
            && matches!(self.window_resize_listener, ViewportObserverState::Registered)
    }
}

/// Observe browser viewport changes and emit validated core viewport values.
///
/// The adapter reports measurements only. Hosts decide the resize policy when
/// translating the emitted [`Viewport`] into `WorkspaceEvent::ViewportChanged`.
/// The returned signal exposes observer/listener registration failures so hosts
/// can log degraded browser environments instead of losing them silently.
pub fn observe_viewport(emit: EventHandler<Viewport>) -> Signal<ViewportObserverStatus> {
    let mut status = use_signal(ViewportObserverStatus::pending);
    let handles = use_hook(|| Rc::new(RefCell::new(None::<ViewportObserverHandles>)));
    let handles_for_drop = handles.clone();
    use_drop(move || {
        handles_for_drop.borrow_mut().take();
    });

    use_hook({
        let handles = handles.clone();
        move || {
            let registration = register_viewport_observer(emit);
            status.set(registration.status);
            *handles.borrow_mut() = registration.handles;
        }
    });

    status
}

struct ViewportObserverRegistration {
    status: ViewportObserverStatus,
    handles: Option<ViewportObserverHandles>,
}

struct ViewportObserverHandles {
    observer: Option<web_sys::ResizeObserver>,
    _observer_callback: Closure<dyn FnMut()>,
    _window_listener: Option<WindowResizeListener>,
}

impl Drop for ViewportObserverHandles {
    fn drop(&mut self) {
        if let Some(observer) = &self.observer {
            observer.disconnect();
        }
    }
}

struct WindowResizeListener {
    window: web_sys::Window,
    callback: Closure<dyn FnMut(web_sys::Event)>,
}

impl Drop for WindowResizeListener {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            "resize",
            self.callback.as_ref().unchecked_ref(),
        );
    }
}

fn register_viewport_observer(emit: EventHandler<Viewport>) -> ViewportObserverRegistration {
    let recompute = Rc::new(move || {
        let (width, height) = viewport_size();
        if let Some(viewport) = viewport_from_size(width, height) {
            emit.call(viewport);
        }
    });

    let observer_callback = Closure::wrap({
        let recompute = recompute.clone();
        Box::new(move || recompute()) as Box<dyn FnMut()>
    });
    let (observer_state, observer) = register_resize_observer(&observer_callback);

    let window_callback = Closure::wrap(
        Box::new(move |_event: web_sys::Event| recompute()) as Box<dyn FnMut(web_sys::Event)>
    );
    let (listener_state, window_listener) = register_window_resize_listener(window_callback);

    ViewportObserverRegistration {
        status: ViewportObserverStatus {
            resize_observer: observer_state,
            window_resize_listener: listener_state,
        },
        handles: Some(ViewportObserverHandles {
            observer,
            _observer_callback: observer_callback,
            _window_listener: window_listener,
        }),
    }
}

fn register_resize_observer(
    callback: &Closure<dyn FnMut()>,
) -> (ViewportObserverState, Option<web_sys::ResizeObserver>) {
    let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return (ViewportObserverState::Unavailable("document element"), None);
    };

    match web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()) {
        Ok(observer) => {
            observer.observe(&element);
            (ViewportObserverState::Registered, Some(observer))
        }
        Err(error) => (ViewportObserverState::Failed(js_error(error)), None),
    }
}

fn register_window_resize_listener(
    callback: Closure<dyn FnMut(web_sys::Event)>,
) -> (ViewportObserverState, Option<WindowResizeListener>) {
    let Some(window) = web_sys::window() else {
        return (ViewportObserverState::Unavailable("window"), None);
    };

    match window.add_event_listener_with_callback("resize", callback.as_ref().unchecked_ref()) {
        Ok(()) => (
            ViewportObserverState::Registered,
            Some(WindowResizeListener { window, callback }),
        ),
        Err(error) => (ViewportObserverState::Failed(js_error(error)), None),
    }
}

fn js_error(error: wasm_bindgen::JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
