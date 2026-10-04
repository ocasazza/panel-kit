//! Browser (Ratzilla WebGL2) backend for the shared topos/grammar demo host.
//!
//! Build with trunk from this crate directory:
//!
//! ```sh
//! trunk serve browser_topos.html --example browser_topos --features spec-plan
//! ```
//!
//! The surface's topos, grammar, and data JSON are embedded at build time from
//! the environment paths; the ASCII-glyph specs match the canvas font atlas
//! Ratzilla can rasterize. Keys: `t` topos, `g` grammar, `n`/`x` append, `r`
//! reset, `s`/space focus and toggle a sort in the active subobject, `p` palette.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "browser_topos is a wasm/Ratzilla example. Build it with trunk from \
         crates/panel-kit-tui pointing at browser_topos.html."
    );
}

#[cfg(target_arch = "wasm32")]
mod topos_app;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::{cell::RefCell, rc::Rc};

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
    use ratzilla::web_sys::wasm_bindgen::JsValue;
    use ratzilla::{CursorShape, WebGl2Backend, WebRenderer};

    use crate::topos_app::{DemoKey, ToposHost};

    const CONTAINER_ID: &str = "panel-kit-topos";
    const PAGE_ROWS: f64 = 4.0;

    /// Browser entry host: the shared demo host plus ratzilla pointer state.
    struct BrowserHost {
        app: ToposHost,
        pointer: RatzillaPointerTranslator,
    }

    impl BrowserHost {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self {
                app: ToposHost::new(BackendKind::BrowserTui)?,
                pointer: RatzillaPointerTranslator::new(),
            })
        }

        fn handle_key(&mut self, event: KeyEvent) {
            let Some(intent) = demo_key(event) else {
                return;
            };
            let _ = self.app.handle_key(intent);
        }

        /// Header controls get first refusal, then core owns the gesture.
        fn handle_mouse(&mut self, event: WebMouseEvent) {
            let at = Position::new(event.col, event.row);
            if matches!(
                event.kind,
                WebMouseEventKind::ButtonDown(WebMouseButton::Left)
            ) && self.app.press(at)
            {
                return;
            }
            let Some(pointer) = self.pointer.pointer_event(event) else {
                return;
            };
            let _ = self.app.pointer_event(pointer);
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
        let host = Rc::new(RefCell::new(BrowserHost::new()?));

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
                let _ = host.app.draw(frame);
            }
        });
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(error) = browser::main() {
        ratzilla::web_sys::console::error_1(&format!("browser_topos error: {error:?}").into());
        return Err(error);
    }
    Ok(())
}
