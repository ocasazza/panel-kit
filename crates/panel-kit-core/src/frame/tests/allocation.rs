use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

use crate::frame::FrameStatus;
use crate::Mode;

use super::fixtures::{project_snapshot, scratch_for, semantic_snapshot, TestPanel};

struct CountingAllocator;

#[global_allocator]
static COUNTING_ALLOCATOR: CountingAllocator = CountingAllocator;

// Count only the measuring thread. The lock keeps future allocation proofs
// from overlapping while unrelated parallel tests allocate freely.
static ALLOCATION_TEST_LOCK: Mutex<()> = Mutex::new(());
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

#[test]
fn project_into_allocates_zero_after_reserved_warmup() {
    let snapshot = semantic_snapshot(Mode::Tiling);
    let mut scratch = scratch_for(&snapshot);

    {
        let frame = project_snapshot(&snapshot, &mut scratch);
        assert_eq!(frame.status, FrameStatus::Ready);
        assert_eq!(frame.panels.len(), 3);
    }

    let allocations = allocations_during(|| {
        {
            let frame = project_snapshot(&snapshot, &mut scratch);
            assert_eq!(frame.status, FrameStatus::Ready);
            assert_eq!(frame.panels.len(), 3);
            assert_eq!(frame.dock.len(), 1);
        }

        {
            let frame = project_snapshot(&snapshot, &mut scratch);
            assert_eq!(frame.status, FrameStatus::Ready);
            assert_eq!(frame.panels[0].key, TestPanel::Alpha);
            assert_eq!(frame.panels[1].key, TestPanel::Beta);
            assert_eq!(frame.panels[2].key, TestPanel::Gamma);
        }
    });

    assert_eq!(allocations, 0);
}

fn record_allocation() {
    if COUNT_ALLOCATIONS.with(Cell::get) {
        RECORDED_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

fn allocations_during(run: impl FnOnce()) -> usize {
    let _serial = ALLOCATION_TEST_LOCK
        .lock()
        .expect("allocation proof lock poisoned");
    COUNT_ALLOCATIONS.with(|enabled| enabled.set(false));
    let scope = CountingScope::start();
    run();
    drop(scope);
    RECORDED_ALLOCATIONS.load(Ordering::SeqCst)
}

struct CountingScope {
    previous: bool,
}

impl CountingScope {
    fn start() -> Self {
        RECORDED_ALLOCATIONS.store(0, Ordering::SeqCst);
        let previous = COUNT_ALLOCATIONS.with(|enabled| enabled.replace(true));
        Self { previous }
    }
}

impl Drop for CountingScope {
    fn drop(&mut self) {
        COUNT_ALLOCATIONS.with(|enabled| enabled.set(self.previous));
    }
}
