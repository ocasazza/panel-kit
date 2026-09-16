#![allow(dead_code)]
use panel_kit_core::frame::{
    project_into, ChromeProjectionInput, ProjectionBuffer, ProjectionInput, TileLayoutMetrics,
};
use panel_kit_core::reducer::{Snapshot, Viewport};
use panel_kit_core::{
    ChromeMetrics, Clamp, LayoutBuilder, Mode, PanelCatalog, PanelKind, Region,
    SurfaceCapabilities, SurfaceProfile, TileMetrics, Units, WinState, CELLS_COMPACT_MAX,
    CELLS_TABLET_MAX,
};
use panel_kit_tui::widgets::{self, TuiHitBuffer};
use panel_kit_tui::{Charset, ResolvedTuiTheme};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use serde::{Deserialize, Serialize};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

pub(crate) struct CountingAllocator;

#[global_allocator]
static COUNTING_ALLOCATOR: CountingAllocator = CountingAllocator;

pub(crate) static ALLOCATION_TEST_LOCK: Mutex<()> = Mutex::new(());
static RECORDED_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNT_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record_allocation();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum TestPanel {
    Alpha,
    Beta,
    Docked,
}

impl PanelKind for TestPanel {
    fn title(self) -> &'static str {
        match self {
            Self::Alpha => "Alpha",
            Self::Beta => "Beta",
            Self::Docked => "Docked",
        }
    }
}

pub(crate) fn fixed_snapshot(mode: Mode) -> (Snapshot<TestPanel>, PanelCatalog<TestPanel>) {
    let mut layout = LayoutBuilder::new();
    let mut panels = vec![
        layout
            .at(TestPanel::Alpha, 2.0, 1.0, 20.0, 7.0)
            .with_tile(1, 1),
        layout
            .at(TestPanel::Beta, 24.0, 2.0, 20.0, 7.0)
            .with_tile(1, 1),
        layout
            .at(TestPanel::Docked, 46.0, 2.0, 20.0, 7.0)
            .with_tile(1, 1),
    ];
    panels[0].z = 20;
    panels[1].z = 10;
    panels[2].state = WinState::Minimized;
    let catalog = PanelCatalog::from_panel_kind_layout(&panels).expect("catalog builds");

    (
        Snapshot {
            panels,
            preferred_mode: mode,
            viewport: Viewport {
                width: 80.0,
                height: 24.0,
                units: Units::Cells,
            },
            focused: Some(TestPanel::Alpha),
            drag: None,
            tile_drag: None,
            workspace_scroll: 0.0,
        },
        catalog,
    )
}

pub(crate) fn project_cells<'a>(
    snapshot: &Snapshot<TestPanel>,
    scratch: &'a mut ProjectionBuffer<TestPanel>,
) -> panel_kit_core::frame::ProjectedFrame<'a, TestPanel> {
    let chrome = ChromeProjectionInput::full(ChromeMetrics::CELLS);
    let tile = TileLayoutMetrics::from_tile_metrics(TileMetrics::CELLS, regular_cells_surface());
    project_into(
        ProjectionInput {
            snapshot,
            surface: regular_cells_surface(),
            chrome: &chrome,
            clamp: &Clamp::CELLS,
            tile: &tile,
        },
        scratch,
    )
}

pub(crate) fn draw_composed_workspace(
    terminal: &mut Terminal<TestBackend>,
    snapshot: &Snapshot<TestPanel>,
    catalog: &PanelCatalog<TestPanel>,
    projection: &mut ProjectionBuffer<TestPanel>,
    hits: &mut TuiHitBuffer<TestPanel>,
) {
    let theme = ResolvedTuiTheme::default();
    terminal
        .draw(|frame| {
            let projected = project_cells(snapshot, projection);
            hits.clear();
            widgets::root::draw_root(
                frame,
                rect_from_region(projected.chrome.root),
                &theme,
                Charset::Ascii,
            );
            for panel in projected.panels.iter().copied() {
                let meta = catalog
                    .get(panel.key)
                    .expect("projected panel metadata exists");
                widgets::panel::draw_panel_surface(frame, panel, &theme, Charset::Ascii, hits);
                widgets::panel::draw_panel_chrome(frame, panel, meta, &theme, Charset::Ascii, hits);
                widgets::panel::draw_traffic_lights(
                    frame,
                    panel,
                    projected.mode,
                    None,
                    &theme,
                    Charset::Ascii,
                    hits,
                );
                widgets::panel::draw_resize_grip(frame, panel, None, &theme, hits);
            }
            widgets::dock::draw_dock(
                frame,
                rect_from_region(projected.chrome.dock),
                projected.dock,
                widgets::dock::DockRenderContext {
                    catalog,
                    label: "dock:",
                    theme: &theme,
                    charset: Charset::Ascii,
                },
                hits,
            );
            widgets::root::draw_workspace_scrollbar(frame, &projected, &theme);
        })
        .expect("draw succeeds");
}

pub(crate) fn regular_cells_surface() -> SurfaceProfile {
    SurfaceProfile::from_logical_width(
        80.0,
        CELLS_COMPACT_MAX,
        CELLS_TABLET_MAX,
        SurfaceCapabilities {
            coarse_pointer: false,
            hover: true,
            keyboard: true,
        },
    )
}

pub(crate) fn rect_from_region(region: Region) -> Rect {
    Rect::new(
        region.x as u16,
        region.y as u16,
        region.w as u16,
        region.h as u16,
    )
}

pub(crate) fn render_to_buffer(
    width: u16,
    height: u16,
    mut render: impl FnMut(&mut ratatui::Frame),
) -> ratatui::buffer::Buffer {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("test backend initializes");
    terminal
        .draw(|frame| render(frame))
        .expect("render succeeds");
    terminal.backend().buffer().clone()
}

pub(crate) fn buffer_contains(buffer: &ratatui::buffer::Buffer, needle: &str) -> bool {
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text.contains(needle)
}

pub(crate) fn allocations_during(run: impl FnOnce()) -> usize {
    RECORDED_ALLOCATIONS.store(0, Ordering::Relaxed);
    let previous = COUNT_ALLOCATIONS.with(|counting| {
        let previous = counting.get();
        counting.set(true);
        previous
    });
    run();
    COUNT_ALLOCATIONS.with(|counting| counting.set(previous));
    RECORDED_ALLOCATIONS.load(Ordering::Relaxed)
}

fn record_allocation() {
    COUNT_ALLOCATIONS.with(|counting| {
        if counting.get() {
            RECORDED_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
    });
}
