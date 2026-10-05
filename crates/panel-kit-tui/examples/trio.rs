//! Native crossterm backend for the shared trio demo host.
//!
//! Run through Nix: `nix build .#trio-tui-native`, then run the binary; both
//! sites, all three regimes, the three morphisms, and the grammars are embedded
//! at build time. `--check-offscreen` renders one frame per site/regime pair
//! and asserts the braille graph, every resolved body, and the four laws the
//! trio demo stands on: a regime switch keeps the positions and recolours, a
//! site switch changes the node set, and the trace site is refused in
//! okf-catalog by name.
//! Keys: `s` site, `t` next available regime, `g` grammar, `n`/`p` next and
//! previous node, `l` re-solve layout, `r` reset, `q` quit.

mod trio_app;

use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseButton, MouseEventKind,
};
use crossterm::execute;
use panel_kit_core::persist::LayoutError;
use panel_kit_core::spec::BackendKind;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;
use trio_app::{BodyPaint, DemoKey, RegimeId, SiteId, TrioApp, TrioHost, TRANSITION_MS};

const CHECK_WIDTH: u16 = 128;
const CHECK_HEIGHT: u16 = 52;
const PAGE_ROWS: f64 = 4.0;

fn main() -> std::io::Result<()> {
    if std::env::args().any(|arg| arg == "--check-offscreen") {
        return check_offscreen().map_err(std::io::Error::other);
    }

    let mut host = new_host(false).map_err(to_io_error)?;
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

fn new_host(reduced_motion: bool) -> Result<TrioApp, Box<dyn std::error::Error>> {
    Ok(TrioApp::new(BackendKind::Tui, reduced_motion)?)
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
fn handle_mouse(host: &mut TrioApp, event: crossterm::event::MouseEvent, now_ms: f64) {
    if event.kind == MouseEventKind::Down(MouseButton::Left) {
        host.press(ratatui::layout::Position::new(event.column, event.row), now_ms);
    }
}

/// Render one frame per available pair and assert the graph, the bindings, and
/// the visible effect of moving along the triangle.
fn check_offscreen() -> Result<(), String> {
    // Reduced motion collapses the tween, so a re-solve lands in one step.
    let mut host = new_host(true).map_err(|error| error.to_string())?;
    let mut diff = Vec::new();

    let site = host.site_id();
    let regime = host.regime();
    if (site, regime) != (SiteId::Traces, RegimeId::Agentic) {
        diff.push("the host did not start on the trace site in agentic".to_owned());
    }
    let mut frame = render(&mut host)?;
    if !frame.paints.iter().all(|paint| paint.bound) {
        diff.push("a panel body painted an unbound binding".to_owned());
    }
    if !frame.brailled {
        diff.push("the graph panel painted no braille".to_owned());
    }
    if frame.paints.is_empty() {
        diff.push("no panel bodies were painted".to_owned());
    }
    for control in ["[traces]", "[OKF]", "[agentic]", "[membrane]", "[lines]", "[json]"] {
        if !frame.text.contains(control) {
            diff.push(format!("the header strip is missing {control}"));
        }
    }
    if !frame.text.contains("agentic is native here") {
        diff.push("the status line is missing the native route".to_owned());
    }

    // A regime switch recolours the same nodes in place: physics is one
    // pullback, so no position may move and at least one colour must.
    let before_positions = host.positions_2d();
    let before_colors = host.node_colors();
    let before_ids = host.node_ids();
    host.handle_key(DemoKey::NextRegime, 0.0).map_err(host_error)?;
    host.advance(TRANSITION_MS as f64);
    if host.regime() == regime {
        diff.push("`t` did not leave the native regime".to_owned());
    }
    if host.positions_2d() != before_positions {
        diff.push("switching regime moved the graph positions".to_owned());
    }
    if host.node_ids() != before_ids {
        diff.push("switching regime changed the node set".to_owned());
    }
    if host.node_colors() == before_colors {
        diff.push("switching regime did not recolour the graph".to_owned());
    }

    if !host.route().is_ok_and(|route| !route.is_native()) {
        diff.push("the switched regime did not report a morphism route".to_owned());
    }
    frame = render(&mut host)?;
    if !frame.paints.iter().all(|paint| paint.bound) {
        diff.push("a panel body painted an unbound binding after `t`".to_owned());
    }
    if !frame.text.contains("--f--> membrane") {
        diff.push("the status line is missing the f route".to_owned());
    }

    // A site switch reads other objects, so the node set must change.
    host.handle_key(DemoKey::SwitchSite, 0.0).map_err(host_error)?;
    host.advance(TRANSITION_MS as f64);
    if host.site_id() == site {
        diff.push("`s` did not change the site".to_owned());
    }
    if host.node_ids() == before_ids {
        diff.push("switching site kept the same node set".to_owned());
    }
    frame = render(&mut host)?;
    if !frame.paints.iter().all(|paint| paint.bound) {
        diff.push("a panel body painted an unbound binding after `s`".to_owned());
    }
    if !frame.brailled {
        diff.push("the graph panel painted no braille after `s`".to_owned());
    }
    // The OKF site has one grammar, so `g` is a no-op there.
    let grammar = host.grammar_id();
    host.handle_key(DemoKey::Grammar, 0.0).map_err(host_error)?;
    host.advance(TRANSITION_MS as f64);
    if host.grammar_id() != grammar {
        diff.push("`g` changed the grammar of a site that has only one".to_owned());
    }

    // The trace site in okf-catalog is refused by name: no morphism exists, so
    // the demo says so instead of inventing one — `t` never walks into it and
    // the status line carries the reason.
    host.handle_key(DemoKey::SwitchSite, 0.0).map_err(host_error)?;
    host.advance(TRANSITION_MS as f64);
    host.handle_key(DemoKey::Grammar, 0.0).map_err(host_error)?;
    host.advance(TRANSITION_MS as f64);
    if host.site_id() != site {
        diff.push("the demo did not return to the trace site".to_owned());
    }
    let refusal = TrioHost::available(site, RegimeId::OkfCatalog)
        .expect_err("the trace site must be refused in okf-catalog");
    for _ in 0..RegimeId::ALL.len() {
        host.handle_key(DemoKey::NextRegime, 0.0).map_err(host_error)?;
        host.advance(TRANSITION_MS as f64);
        if host.regime() == RegimeId::OkfCatalog {
            diff.push("`t` entered okf-catalog on the trace site".to_owned());
            break;
        }
    }
    frame = render(&mut host)?;
    if !frame.text.contains(&refusal) {
        diff.push("the status line does not say why the pair is disabled".to_owned());
    }

    if diff.is_empty() {
        return Ok(());
    }
    Err(format!("offscreen trio canary mismatch:\n{}", diff.join("\n")))
}

/// One rendered frame, kept only for what the canary asserts.
struct Frame {
    text: String,
    brailled: bool,
    paints: Vec<BodyPaint>,
}

fn render(host: &mut TrioApp) -> Result<Frame, String> {
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
    Ok(Frame {
        brailled: body_has_braille(&buffer, host.graph_rect()),
        text: buffer_text(&buffer),
        paints: host.paints().to_vec(),
    })
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