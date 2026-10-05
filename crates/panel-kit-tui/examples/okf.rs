//! Native crossterm backend for the shared OKF graph-topos demo host.
//!
//! Run through Nix: `nix build .#okf-tui-native`, then run the binary; the OKF
//! snapshot, both regime topoi, `f`, and the physics base are embedded at build
//! time. `--check-offscreen` renders one frame per regime and asserts the
//! braille graph, every resolved body, and the visible effect of crossing `f`
//! and of selecting a node.
//! Keys: `t` cross topos along `f`, `n`/`p` next/previous node, `l` re-solve
//! layout, `r` reset, `q` quit.

mod okf_app;

use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
};
use crossterm::execute;
use okf_app::{DemoKey, OkfApp, TRANSITION_MS};
use panel_kit_core::persist::LayoutError;
use panel_kit_core::spec::BackendKind;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;

const CHECK_WIDTH: u16 = 128;
const CHECK_HEIGHT: u16 = 52;
const PAGE_ROWS: f64 = 4.0;

fn main() -> std::io::Result<()> {
    if std::env::args().any(|arg| arg == "--check-offscreen") {
        return check_offscreen().map_err(std::io::Error::other);
    }

    let mut host = OkfApp::new(BackendKind::Tui, false).map_err(to_io_error)?;
    let start = Instant::now();
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    loop {
        host.advance(elapsed_ms(start));
        let mut draw_result = Ok(());
        terminal.draw(|frame| {
            draw_result = host.draw(frame);
        })?;
        draw_result.map_err(to_io_error)?;
        if event::poll(Duration::from_millis(33))? {
            match event::read()? {
                Event::Key(key) => {
                    if let Some(intent) = demo_key(key) {
                        if host.handle_key(intent, elapsed_ms(start)).map_err(to_io_error)? {
                            break;
                        }
                    }
                }
                Event::Mouse(mouse) => handle_mouse(&mut host, mouse, elapsed_ms(start)),
                _ => {}
            }
        }
    }
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    Ok(())
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// Translate one crossterm key event into a demo intent.
fn demo_key(event: crossterm::event::KeyEvent) -> Option<DemoKey> {
    match event.code {
        KeyCode::PageUp => Some(DemoKey::Page(-PAGE_ROWS)),
        KeyCode::PageDown => Some(DemoKey::Page(PAGE_ROWS)),
        KeyCode::Char('q') => Some(DemoKey::Quit),
        _ => panel_kit_tui::input::crossterm_key_chord(event).map(DemoKey::from_chord),
    }
}

/// Header controls and graph nodes first; the graph's own cells are not
/// draggable, so nothing else needs the pointer.
fn handle_mouse(host: &mut OkfApp, event: crossterm::event::MouseEvent, now_ms: f64) {
    if event.kind == MouseEventKind::Down(MouseButton::Left) {
        host.press(ratatui::layout::Position::new(event.column, event.row), now_ms);
    }
}

/// Render one frame per regime and assert the graph, the bindings, and the
/// visible effects of crossing `f` and selecting a node.
fn check_offscreen() -> Result<(), String> {
    let mut host = OkfApp::new(BackendKind::Tui, true).map_err(|error| error.to_string())?;
    let mut diff = Vec::new();
    let mut titles: Option<Vec<String>> = None;
    let mut positions: Option<Vec<[f32; 2]>> = None;

    for spec_id in host.topos_ids() {
        // Reduced motion collapses the tween, so entering a regime lands on its
        // own arrangement in one step.
        host.set_topos(spec_id, 0.0);
        host.advance(TRANSITION_MS as f64);
        if host.topos_id() != spec_id {
            diff.push(format!("{spec_id}: the host stayed in {}", host.topos_id()));
            continue;
        }
        let mut terminal =
            Terminal::new(TestBackend::new(CHECK_WIDTH, CHECK_HEIGHT)).map_err(infallible)?;
        let mut draw_result = Ok(());
        terminal
            .draw(|frame| {
                draw_result = host.draw(frame);
            })
            .map_err(infallible)?;
        draw_result.map_err(|error| error.to_string())?;

        let buffer = terminal.backend().buffer().clone();
        let text = buffer_text(&buffer);
        if text.contains("unbound:") {
            diff.push(format!("{spec_id}: a panel body painted an unbound binding"));
        }
        let status_head = format!("okf-{}", spec_id.trim_start_matches("topos-okf-"));
        if !text.contains(&status_head) {
            diff.push(format!("{spec_id}: header strip is missing the status line"));
        }
        for control in ["[solve]", "[reset]"] {
            if !text.contains(control) {
                diff.push(format!("{spec_id}: header strip is missing {control}"));
            }
        }
        if host.paints().is_empty() {
            diff.push(format!("{spec_id}: no panel bodies were painted"));
        }
        for paint in host.paints() {
            if !paint.bound {
                diff.push(format!(
                    "{spec_id}: panel {:?} did not resolve its binding",
                    paint.key
                ));
            }
            if paint.rect.is_empty() || !body_has_content(&buffer, paint.rect) {
                diff.push(format!(
                    "{spec_id}: panel {:?} body {:?} is blank",
                    paint.key, paint.rect
                ));
            }
        }
        if !body_has_braille(&buffer, host.graph_rect()) {
            diff.push(format!("{spec_id}: the graph panel painted no braille"));
        }

        let current = host.panel_titles();
        if titles.as_ref() == Some(&current) {
            diff.push(format!("{spec_id}: the two regimes share one panel set"));
        }
        titles = Some(current);

        let current = host.positions_2d();
        if positions.as_ref() == Some(&current) {
            diff.push(format!("{spec_id}: the two regimes solved the same layout"));
        }
        positions = Some(current);

        // Selecting a node repaints the `okf.selected` panel.
        let before = host.selected_text();
        host.handle_key(DemoKey::Selection(1), 0.0).map_err(host_error)?;
        if host.selected().is_none() || before == host.selected_text() {
            diff.push(format!("{spec_id}: selecting a node left the selected panel unchanged"));
        }
    }

    if diff.is_empty() {
        return Ok(());
    }
    Err(format!("offscreen okf canary mismatch:\n{}", diff.join("\n")))
}

fn buffer_text(buffer: &Buffer) -> String {
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

fn body_has_content(buffer: &Buffer, rect: Rect) -> bool {
    (rect.y..rect.bottom()).any(|y| {
        (rect.x..rect.right()).any(|x| {
            buffer[(x, y)]
                .symbol()
                .chars()
                .any(|symbol| !symbol.is_whitespace())
        })
    })
}

/// Whether the graph body carries a braille code point (U+2800..=U+28FF).
fn body_has_braille(buffer: &Buffer, rect: Rect) -> bool {
    (rect.y..rect.bottom()).any(|y| {
        (rect.x..rect.right()).any(|x| {
            buffer[(x, y)]
                .symbol()
                .chars()
                .any(|symbol| ('\u{2800}'..='\u{28ff}').contains(&symbol))
        })
    })
}

fn infallible(error: std::convert::Infallible) -> String {
    match error {}
}

fn host_error(error: LayoutError) -> String {
    error.to_string()
}

fn to_io_error(error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(error.to_string())
}