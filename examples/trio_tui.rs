//! Native crossterm backend for the shared trio demo host.
//!
//! Run through Nix: `nix build .#trio-tui-native`, then run the binary; the one
//! site, all three regimes, the four morphisms, and the two grammars are
//! embedded at build time. `--check-offscreen` renders one frame per regime and
//! asserts the braille graph, every resolved body, the merged node set, and the
//! laws the trio demo stands on: a regime switch keeps the positions and the
//! node ids and recolours, and `g` re-reads the one site.
//! Keys: `t` next regime, `g` grammar, `n`/`p` next and previous node, `l`
//! re-solve layout, `r` reset, `q` quit.

#[path = "support/trio_app.rs"]
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
use trio_app::{BodyPaint, DemoKey, RegimeId, TrioApp, TRANSITION_MS};

const CHECK_WIDTH: u16 = 128;
const CHECK_HEIGHT: u16 = 52;
const PAGE_ROWS: f64 = 4.0;
/// Nodes the merged site graph carries.
const SITE_NODES: usize = 70;

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

/// Render one frame per regime and assert the graph, the bindings, and the
/// visible effect of moving along the triangle.
fn check_offscreen() -> Result<(), String> {
    // Reduced motion collapses the tween, so a re-solve lands in one step.
    let mut host = new_host(true).map_err(|error| error.to_string())?;
    let mut diff = Vec::new();

    let start = host.regime();
    if start != RegimeId::Agentic {
        diff.push("the host did not start in agentic".to_owned());
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
    if !host.unreadable().is_empty() {
        diff.push(format!("panels cannot read the site: {:?}", host.unreadable()));
    }
    for control in ["[agentic]", "[membrane]", "[okf-catalog]", "[lines]", "[json]"] {
        if !frame.text.contains(control) {
            diff.push(format!("the header strip is missing {control}"));
        }
    }
    for control in ["[solve]", "[reset]"] {
        if !frame.text.contains(control) {
            diff.push(format!("the header strip is missing {control}"));
        }
    }
    for absent in ["[traces]", "[OKF]", "unavailable"] {
        if frame.text.contains(absent) {
            diff.push(format!("the frame still names {absent}"));
        }
    }
    if host.node_ids().len() != SITE_NODES {
        diff.push(format!(
            "the merged site graph carries {} nodes, not {SITE_NODES}",
            host.node_ids().len()
        ));
    }
    let before_ids = host.node_ids();
    let before_positions = host.positions_2d();
    let mut previous_colors = host.node_colors();

    // A regime switch recolours the same nodes in place: physics is one
    // pullback, so no position may move and at least one colour must.
    for step in 1..=RegimeId::ALL.len() {
        let regime = RegimeId::ALL[step % RegimeId::ALL.len()];
        host.handle_key(DemoKey::NextRegime, step as f64).map_err(host_error)?;
        host.advance(TRANSITION_MS as f64);
        if host.regime() != regime {
            diff.push(format!("`t` did not enter {regime}"));
        }
        if host.node_ids() != before_ids {
            diff.push(format!("switching to {regime} changed the node set"));
        }
        if host.positions_2d() != before_positions {
            diff.push(format!("switching to {regime} moved the graph positions"));
        }
        if host.node_colors() == previous_colors {
            diff.push(format!("switching to {regime} did not recolour the graph"));
        }
        previous_colors = host.node_colors();
        if !host.unreadable().is_empty() {
            diff.push(format!("{regime} has panels that cannot read the site"));
        }
        for route in routes_into(regime) {
            if !host.route_summary().contains(route) {
                diff.push(format!("the {regime} routes are missing {route}"));
            }
        }
        frame = render(&mut host)?;
        if !frame.paints.iter().all(|paint| paint.bound) {
            diff.push(format!("a panel body painted an unbound binding in {regime}"));
        }
        if !frame.brailled {
            diff.push(format!("the graph panel painted no braille in {regime}"));
        }
        if frame.text.contains("cannot read") || frame.text.contains("unbound:") {
            diff.push(format!("the {regime} frame shows a panel that cannot read"));
        }
        if !frame.status.contains(regime.label()) {
            diff.push(format!("the status line does not name {regime}"));
        }
        for route in routes_into(regime) {
            if !frame.status.contains(route) {
                diff.push(format!("the {regime} status line is missing {route}"));
            }
        }
    }
    // Three switches from the first regime come back round to it.
    if host.regime() != start {
        diff.push("`t` did not cycle back to the starting regime".to_owned());
    }
    // `g` re-reads the one site: the node set survives, and the positions are
    // re-solved from the arrangement on screen.
    let grammar = host.grammar_id();
    let seeded_positions = host.positions_2d();
    host.handle_key(DemoKey::Grammar, 10.0).map_err(host_error)?;
    host.advance(TRANSITION_MS as f64);
    if host.grammar_id() == grammar {
        diff.push("`g` did not re-read the site with the next grammar".to_owned());
    }
    if host.node_ids() != before_ids {
        diff.push("`g` changed the merged node set".to_owned());
    }
    if host.positions_2d() == seeded_positions {
        diff.push("`g` did not re-solve the layout".to_owned());
    }
    frame = render(&mut host)?;
    if !frame.brailled {
        diff.push("the graph panel painted no braille after `g`".to_owned());
    }
    if !frame.paints.iter().all(|paint| paint.bound) {
        diff.push("a panel body painted an unbound binding after `g`".to_owned());
    }
    if !frame.text.contains(host.grammar_id()) {
        diff.push("the status line does not name the re-read grammar".to_owned());
    }

    // `l` solves from scratch, `r` clears the selection and solves again.
    host.handle_key(DemoKey::Selection(1), 20.0).map_err(host_error)?;
    if host.selected().is_none() {
        diff.push("`n` selected nothing".to_owned());
    }
    host.handle_key(DemoKey::Reset, 21.0).map_err(host_error)?;
    host.advance(TRANSITION_MS as f64);
    if host.selected().is_some() {
        diff.push("`r` did not clear the selection".to_owned());
    }
    if host.node_ids().len() != SITE_NODES {
        diff.push("`r` changed the merged node count".to_owned());
    }

    if diff.is_empty() {
        return Ok(());
    }
    Err(format!("offscreen trio canary mismatch:\n{}", diff.join("\n")))
}

/// One rendered frame, kept only for what the canary asserts.
struct Frame {
    text: String,
    status: String,
    brailled: bool,
    paints: Vec<BodyPaint>,
}

/// The routes every native regime takes into `regime`, as the status line
/// names them.
fn routes_into(regime: RegimeId) -> &'static [&'static str] {
    match regime {
        RegimeId::Agentic => &["is native here", "--χ--> agentic"],
        RegimeId::Membrane => &["--f--> membrane", "--ψ--> membrane"],
        RegimeId::OkfCatalog => &["is native here", "--ω--> okf-catalog"],
    }
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
    let text = buffer_text(&buffer);
    let status = text.lines().next().unwrap_or_default().trim_end().to_owned();
    Ok(Frame {
        brailled: body_has_braille(&buffer, host.graph_rect()),
        text,
        status,
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