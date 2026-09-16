#[path = "support/composition.rs"]
mod support;

use panel_kit_core::frame::{PanelProjection, ProjectionBuffer};
use panel_kit_core::reducer::{HitTarget, PanelPart, WorkspaceEvent};
use panel_kit_core::{
    FocusContext, Key, KeyChord, Mode, PanelCommand, PointerButton, PointerEvent, PointerEventKind,
};
use panel_kit_tui::widgets::{self, TuiHitBuffer};
use panel_kit_tui::{Charset, ResolvedTuiTheme};
use ratatui::backend::TestBackend;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};
use ratatui::Terminal;

use support::{
    allocations_during, draw_composed_workspace, fixed_snapshot, project_cells, rect_from_region,
    TestPanel, ALLOCATION_TEST_LOCK,
};

#[test]
fn tui_hit_adapter_maps_drawn_parts_to_workspace_events() {
    let (snapshot, catalog) = fixed_snapshot(Mode::Floating);
    let mut scratch = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
    let projected = project_cells(&snapshot, &mut scratch);
    let alpha = projected
        .panels
        .iter()
        .find(|panel| panel.key == TestPanel::Alpha)
        .copied()
        .expect("alpha projects");
    let meta = catalog.get(alpha.key).expect("alpha metadata exists");
    let mut hit_buffer = TuiHitBuffer::with_capacity(1, 0);

    support::render_to_buffer(80, 24, |frame| {
        widgets::panel::draw_panel_surface(
            frame,
            alpha,
            &ResolvedTuiTheme::default(),
            Charset::Ascii,
            &mut hit_buffer,
        );
        widgets::panel::draw_panel_chrome(
            frame,
            alpha,
            meta,
            &ResolvedTuiTheme::default(),
            Charset::Ascii,
            &mut hit_buffer,
        );
        widgets::panel::draw_traffic_lights(
            frame,
            alpha,
            Mode::Floating,
            None,
            &ResolvedTuiTheme::default(),
            Charset::Ascii,
            &mut hit_buffer,
        );
    });

    let mode_hit = alpha.chrome.mode_hit.expect("mode hit projected");
    let event = panel_kit_tui::input::workspace_event_from_pointer(
        &hit_buffer,
        PointerEvent {
            kind: PointerEventKind::Down(PointerButton::Primary),
            x: mode_hit.x,
            y: mode_hit.y,
        },
    );

    assert_eq!(
        event,
        Some(WorkspaceEvent::Command {
            target: Some(TestPanel::Alpha),
            command: PanelCommand::ToggleMode,
        })
    );
    assert_eq!(
        panel_kit_tui::input::workspace_event_from_key(
            KeyChord {
                key: Key::Tab,
                shift: false,
                alt: false,
                ctrl: false,
                meta: false
            },
            FocusContext::<TestPanel>::Workspace,
        ),
        WorkspaceEvent::Key {
            chord: KeyChord {
                key: Key::Tab,
                shift: false,
                alt: false,
                ctrl: false,
                meta: false
            },
            focus: FocusContext::<TestPanel>::Workspace,
        }
    );
}

#[test]
fn drawing_over_borrowed_frame_uses_no_owned_scratch_vectors_after_reserve() {
    let _guard = ALLOCATION_TEST_LOCK
        .lock()
        .expect("allocation test lock is not poisoned");
    let (snapshot, catalog) = fixed_snapshot(Mode::Floating);
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test backend initializes");
    let mut projection = ProjectionBuffer::with_panel_capacity(snapshot.panels.len());
    let mut hits = TuiHitBuffer::with_capacity(snapshot.panels.len(), snapshot.panels.len());

    terminal.draw(|_| {}).expect("empty warm-up draw succeeds");
    hits.reserve(snapshot.panels.len(), snapshot.panels.len());
    let empty_draw_allocations = allocations_during(|| {
        terminal.draw(|_| {}).expect("empty measured draw succeeds");
    });
    let native_chrome_allocations = {
        let projected = project_cells(&snapshot, &mut projection);
        allocations_during(|| {
            draw_native_chrome_allocation_baseline(&mut terminal, projected.panels, &catalog);
        })
    };

    draw_composed_workspace(
        &mut terminal,
        &snapshot,
        &catalog,
        &mut projection,
        &mut hits,
    );
    {
        let projected = project_cells(&snapshot, &mut projection);
        assert_header_hit_was_recorded(&hits, projected.panels);
    }

    let allocations = allocations_during(|| {
        draw_composed_workspace(
            &mut terminal,
            &snapshot,
            &catalog,
            &mut projection,
            &mut hits,
        );
    });

    let allowed_allocations = empty_draw_allocations + native_chrome_allocations;
    assert!(
        allocations <= allowed_allocations,
        "borrowed-frame draw allocated {allocations} times; expected no panel-kit scratch allocation beyond {native_chrome_allocations} native chrome/title allocations and {empty_draw_allocations} empty-draw allocations"
    );

    let projected = project_cells(&snapshot, &mut projection);
    assert_eq!(
        rect_from_region(projected.chrome.root),
        terminal.get_frame().area()
    );
}

fn assert_header_hit_was_recorded(
    hits: &TuiHitBuffer<TestPanel>,
    panels: &[PanelProjection<TestPanel>],
) {
    let alpha = panels
        .iter()
        .find(|panel| panel.key == TestPanel::Alpha)
        .expect("alpha projects");
    let header_event = PointerEvent {
        kind: PointerEventKind::Down(PointerButton::Primary),
        x: alpha.chrome.header_hit.x + 10.0,
        y: alpha.chrome.header_hit.y,
    };

    assert_eq!(
        panel_kit_tui::input::workspace_event_from_pointer(hits, header_event),
        Some(WorkspaceEvent::Pointer {
            target: HitTarget::Panel {
                key: TestPanel::Alpha,
                part: PanelPart::Header
            },
            event: header_event,
        })
    );
}

fn draw_native_chrome_allocation_baseline(
    terminal: &mut Terminal<TestBackend>,
    panels: &[PanelProjection<TestPanel>],
    catalog: &panel_kit_core::PanelCatalog<TestPanel>,
) {
    let theme = ResolvedTuiTheme::default();
    terminal
        .draw(|frame| {
            for panel in panels.iter().copied() {
                let meta = catalog
                    .get(panel.key)
                    .expect("projected panel metadata exists");
                let outer = rect_from_region(panel.chrome.outer);
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_set(BorderType::Plain.to_border_set())
                    .border_style(Style::default().fg(theme.focus_ring))
                    .title(Line::from(Span::styled(
                        meta.title.as_ref(),
                        Style::default().fg(theme.fg),
                    )));

                let _ = block.inner(outer);
                frame.render_widget(block, outer);
            }
        })
        .expect("native chrome baseline draw succeeds");
}
