//! Native crossterm backend for the shared topos/grammar demo host.
//!
//! Run through Nix: `nix build .#topos-tui-native`, then run the binary; topos,
//! grammar, and data JSON are embedded at build time. `--check-offscreen`
//! renders one frame per topos and fails when a panel body did not resolve.
//! Keys: `t` topos, `g` grammar, `n`/`x` append, `r` reset, `s`/space focus and
//! toggle a sort in the active subobject, `p` palette, `q` quit.

mod topos_app;

use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
};
use crossterm::execute;
use panel_kit_core::persist::LayoutError;
use panel_kit_core::spec::BackendKind;
use panel_kit_tui::input::crossterm_workspace_event;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use topos_app::{DemoKey, ToposHost};

const CHECK_WIDTH: u16 = 128;
const CHECK_HEIGHT: u16 = 52;
const PAGE_ROWS: f64 = 4.0;

fn main() -> std::io::Result<()> {
    if std::env::args().any(|arg| arg == "--check-offscreen") {
        return check_offscreen().map_err(std::io::Error::other);
    }

    let mut host = new_host().map_err(|error| std::io::Error::other(error.to_string()))?;
    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    loop {
        let mut draw_result = Ok(());
        terminal.draw(|frame| {
            draw_result = host.draw(frame);
        })?;
        draw_result.map_err(to_io_error)?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if let Some(intent) = demo_key(key) {
                        if host.handle_key(intent).map_err(to_io_error)? {
                            break;
                        }
                    }
                }
                Event::Mouse(mouse) => handle_mouse(&mut host, mouse).map_err(to_io_error)?,
                _ => {}
            }
        }
    }
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    Ok(())
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

/// Header controls get first refusal, then core owns the pointer gesture.
fn handle_mouse(
    host: &mut ToposHost,
    event: crossterm::event::MouseEvent,
) -> Result<(), LayoutError> {
    if event.kind == MouseEventKind::Down(MouseButton::Left)
        && host.press(Position::new(event.column, event.row))
    {
        return Ok(());
    }
    let (hits, panels) = host.routing();
    if let Some(workspace_event) = crossterm_workspace_event(hits, panels, event) {
        host.reduce(workspace_event)?;
    }
    Ok(())
}

fn new_host() -> Result<ToposHost, Box<dyn std::error::Error>> {
    Ok(ToposHost::new(BackendKind::Tui)?)
}

/// Render one frame per topos and assert every body resolved demo content.
fn check_offscreen() -> Result<(), String> {
    let mut host = ToposHost::new(BackendKind::Tui).map_err(|error| error.to_string())?;

    let mut diff = Vec::new();
    let mut titles: Option<Vec<String>> = None;
    for spec_id in host.topos_ids() {
        if !host.set_topos(spec_id) {
            diff.push(format!("{spec_id}: host rejected its own topos id"));
            continue;
        }
        let mut terminal = Terminal::new(TestBackend::new(CHECK_WIDTH, CHECK_HEIGHT))
            .map_err(infallible)?;
        let mut draw_result = Ok(());
        terminal
            .draw(|frame| {
                draw_result = host.draw(frame);
            })
            .map_err(infallible)?;
        draw_result.map_err(|error| error.to_string())?;

        let buffer = terminal.backend().buffer();
        let text = buffer_text(buffer);
        if text.contains("unbound:") {
            diff.push(format!("{spec_id}: a panel body painted an unbound binding"));
        }
        // The header clips the status line to its width; assert its stable head
        // (the regime name) renders. Counts and transport are checked elsewhere.
        let status_head = format!("topos {}", spec_id.trim_start_matches("topos-"));
        if !text.contains(&status_head) {
            diff.push(format!("{spec_id}: header strip is missing the status line"));
        }
        for control in ["[+ line]", "[+ bad]", "[reset]"] {
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
            if paint.rect.is_empty() || !body_has_content(buffer, paint.rect) {
                diff.push(format!(
                    "{spec_id}: panel {:?} body {:?} is blank",
                    paint.key, paint.rect
                ));
            }
        }

        let current = host.panel_titles();
        if titles.as_ref() == Some(&current) {
            diff.push(format!("{spec_id}: switching topos kept the same panel set"));
        }
        titles = Some(current);

        let records = host.demo().record_count();
        let diagnostics = host.demo().diagnostic_count();
        host.handle_key(DemoKey::Switch('x')).map_err(host_error)?;
        host.handle_key(DemoKey::Switch('n')).map_err(host_error)?;
        if host.demo().record_count() != records + 1 {
            diff.push(format!("{spec_id}: appending a valid line changed no record"));
        }
        if host.demo().diagnostic_count() != diagnostics + 1 {
            diff.push(format!("{spec_id}: appending a bad line changed no diagnostic"));
        }
        host.handle_key(DemoKey::Switch('r')).map_err(host_error)?;
        if host.demo().record_count() != records {
            diff.push(format!("{spec_id}: resetting the source kept the appended lines"));
        }
    }

    if diff.is_empty() {
        return Ok(());
    }
    Err(format!(
        "offscreen topos canary mismatch:\n{}",
        diff.join("\n")
    ))
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

fn infallible(error: std::convert::Infallible) -> String {
    match error {}
}

fn host_error(error: LayoutError) -> String {
    error.to_string()
}

fn to_io_error(error: LayoutError) -> std::io::Error {
    std::io::Error::other(error)
}
