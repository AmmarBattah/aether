//! Compiled by tests/allocations.rs as a standalone probe. Production and
//! ordinary integration tests retain unsafe_code=forbid.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use aether_tui::app::transcript::ScrollIntent;
use aether_tui::testing::app_with_transcript;
use aether_tui::ui::{self, transcript::ViewCache};
use ratatui::{Terminal, backend::TestBackend};

struct Counting;

static ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

fn record(bytes: usize) {
    if ENABLED.load(Ordering::Relaxed) {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
}

// SAFETY: every allocation operation forwards the unchanged pointer and layout
// to System. Counters observe requests without touching allocated memory.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[derive(Debug, Clone, Copy)]
struct Allocations {
    count: usize,
    bytes: usize,
}

fn measure(rows: usize, reading: bool) -> Allocations {
    let reply = format!(
        "```\n{}```",
        "a typical output row with enough text to own storage\n".repeat(rows)
    );
    let mut app = app_with_transcript(&["show output", &reply]);
    app.board.open = false;
    if reading {
        let agent = app.selected.expect("selected agent");
        app.scroll_log.push(agent, ScrollIntent::Top);
        app.scroll_log.push(agent, ScrollIntent::LineDown(40));
    }
    let mut cache = ViewCache::default();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("terminal");
    // Warm both terminal buffers and the layout; pending scrolling is resolved
    // before counting so this measures repeated frames, not initial layout.
    for _ in 0..2 {
        terminal
            .draw(|f| ui::render(f, &app, &mut cache))
            .expect("warm draw");
    }
    let inserts = cache.layout.inserts();
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    ENABLED.store(true, Ordering::Relaxed);
    let result = terminal.draw(|f| ui::render(f, &app, &mut cache));
    ENABLED.store(false, Ordering::Relaxed);
    let measured = Allocations {
        count: ALLOCS.load(Ordering::Relaxed),
        bytes: BYTES.load(Ordering::Relaxed),
    };
    result.expect("measured draw");
    assert_eq!(
        cache.layout.inserts(),
        inserts,
        "warm frame must reuse layouts"
    );
    measured
}

#[test]
fn cached_large_cells_allocate_only_for_visible_rows() {
    // A single test and sequential draws isolate the process-wide counters.
    for reading in [false, true] {
        let small = measure(100, reading);
        let large = measure(10_000, reading);
        println!("warm 100x30, reading={reading}: 100 rows {small:?}; 10,000 rows {large:?}");
        assert!(
            small.count > 0 && small.bytes > 0,
            "counter must observe the draw"
        );
        assert!(
            large.count <= small.count * 2,
            "allocation count grows with hidden rows: {small:?} -> {large:?}"
        );
        assert!(
            large.bytes <= small.bytes * 2,
            "allocated bytes grow with hidden rows: {small:?} -> {large:?}"
        );
    }
}
