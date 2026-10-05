//! Browser (Ratzilla WebGL2) backend for the shared OKF graph-topos demo host.
//!
//! Build with trunk from this crate directory:
//!
//! ```sh
//! trunk serve --port 8097 --address 127.0.0.1 --no-autoreload=true \
//!   --example browser_okf --features spec-plan browser_okf.html
//! ```
//!
//! The OKF snapshot, both regime topoi, `f`, and the physics base are embedded
//! at build time from the environment paths; the ASCII-glyph specs match the
//! canvas font atlas Ratzilla can rasterize, and the graph is painted with the
//! same braille canvas as the native host. Keys: `t` cross topos along `f`,
//! `n`/`p` next/previous node, `l` re-solve layout, `r` reset.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "browser_okf is a wasm/Ratzilla example. Build it with trunk from \
         crates/panel-kit-tui pointing at browser_okf.html."
    );
}

#[cfg(target_arch = "wasm32")]
mod okf_app;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{cell::RefCell, rc::Rc};

    use crate::okf_app::{DemoKey, OkfApp};
    use panel_kit_core::spec::BackendKind;
    use panel_kit_tui::input::{ratzilla_key_chord, RatzillaPointerTranslator};
    use panel_kit_tui::wheel::{self, WheelCell};
    use ratatui::layout::Position;
    use ratatui::style::Color;
    use ratzilla::backend::webgl2::{FontAtlasConfig, WebGl2BackendOptions};
    use ratzilla::event::{
        KeyCode, KeyEvent, MouseButton as WebMouseButton, MouseEvent as WebMouseEvent,
        MouseEventKind as WebMouseEventKind,
    };
    use ratzilla::web_sys::js_sys::{Function, Reflect};
    use ratzilla::web_sys::wasm_bindgen::{JsCast, JsValue};
    use ratzilla::{CursorShape, WebGl2Backend, WebRenderer};

    const CONTAINER_ID: &str = "panel-kit-okf";
    const PAGE_ROWS: f64 = 4.0;

    /// Browser entry host: the shared OKF host plus ratzilla pointer state.
    struct BrowserHost {
        app: OkfApp,
        pointer: RatzillaPointerTranslator,
        started: f64,
    }

    impl BrowserHost {
        fn new(started: f64) -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                app: OkfApp::new(BackendKind::BrowserTui, prefers_reduced_motion())?,
                pointer: RatzillaPointerTranslator::new(),
                started,
            })
        }

        /// Wall-clock milliseconds since the page load, the host's tween clock.
        fn now_ms(&self) -> f64 {
            now_ms() - self.started
        }

        fn handle_key(&mut self, event: KeyEvent) {
            let Some(intent) = demo_key(event) else {
                return;
            };
            let _ = self.app.handle_key(intent, self.now_ms());
        }

        /// Header controls and graph nodes first; the graph's own cells are not
        /// draggable, so nothing else needs the pointer.
        fn handle_mouse(&mut self, event: WebMouseEvent) {
            if matches!(
                event.kind,
                WebMouseEventKind::ButtonDown(WebMouseButton::Left)
            ) {
                let at = Position::new(event.col, event.row);
                if self.app.press(at, self.now_ms()) {
                    return;
                }
            }
            if let Some(pointer) = self.pointer.pointer_event(event) {
                let _ = self.app.pointer_event(pointer);
            }
        }

        /// Report whether the gesture moved a panel body or the workspace.
        fn handle_wheel(&mut self, cell: WheelCell) -> bool {
            self.app.wheel((cell.col, cell.row), cell.delta())
        }
    }

    fn demo_key(event: KeyEvent) -> Option<DemoKey> {
        match event.code.clone() {
            KeyCode::PageUp => Some(DemoKey::Page(-PAGE_ROWS)),
            KeyCode::PageDown => Some(DemoKey::Page(PAGE_ROWS)),
            _ => ratzilla_key_chord(event).map(DemoKey::from_chord),
        }
    }

    /// The page's wall clock in milliseconds.
    fn now_ms() -> f64 {
        ratzilla::web_sys::js_sys::Date::now()
    }

    /// Whether the page asks for reduced motion, so the layout tween collapses.
    fn prefers_reduced_motion() -> bool {
        let Some(window) = ratzilla::web_sys::window() else {
            return false;
        };
        let Ok(media) = Reflect::get(window.as_ref(), &JsValue::from_str("matchMedia")) else {
            return false;
        };
        let Ok(query) = media.dyn_into::<Function>() else {
            return false;
        };
        query
            .call1(
                window.as_ref(),
                &JsValue::from_str("(prefers-reduced-motion: reduce)"),
            )
            .ok()
            .and_then(|media| Reflect::get(&media, &JsValue::from_str("matches")).ok())
            .and_then(|matches| matches.as_bool())
            .unwrap_or(false)
    }

    fn install_wheel(host: Rc<RefCell<BrowserHost>>) -> Result<(), JsValue> {
        let grid_host = host.clone();
        wheel::install_cell_wheel(
            CONTAINER_ID,
            move || {
                grid_host
                    .try_borrow()
                    .map(|host| host.app.grid())
                    .unwrap_or((1.0, 1.0))
            },
            move |cell| match host.try_borrow_mut() {
                Ok(mut host) => host.handle_wheel(cell),
                Err(_) => false,
            },
        )
    }

    /// Boot the WebGL2 terminal and run the host's frame loop.
    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        let backend = WebGl2Backend::new_with_options(
            WebGl2BackendOptions::new()
                .grid_id(CONTAINER_ID)
                .cursor_shape(CursorShape::None)
                .canvas_padding_color(Color::Black)
                .font_atlas_config(FontAtlasConfig::dynamic(
                    &["Fira Code", "JetBrains Mono", "monospace"],
                    16.0,
                )),
        )?;
        let mut terminal = ratatui::Terminal::new(backend)?;
        let host = Rc::new(RefCell::new(BrowserHost::new(now_ms())?));

        install_wheel(host.clone())
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        terminal.on_key_event({
            let host = host.clone();
            move |key| {
                if let Ok(mut host) = host.try_borrow_mut() {
                    host.handle_key(key);
                }
            }
        })?;
        terminal.on_mouse_event({
            let host = host.clone();
            move |event| {
                if let Ok(mut host) = host.try_borrow_mut() {
                    host.handle_mouse(event);
                }
            }
        })?;

        terminal.draw_web(move |frame| {
            if let Ok(mut host) = host.try_borrow_mut() {
                let now = host.now_ms();
                host.app.advance(now);
                if let Err(error) = host.app.draw(frame) {
                    ratzilla::web_sys::console::error_1(&format!("browser_okf draw: {error:?}").into());
                }
            }
        });
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(error) = browser::main() {
        ratzilla::web_sys::console::error_1(&format!("browser_okf error: {error:?}").into());
        return Err(error);
    }
    Ok(())
}