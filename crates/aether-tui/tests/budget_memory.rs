#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! §9.8's memory budget: 10 000 transcript cells under 100 MB.
//!
//! It has an integration test file to itself because that is a process to
//! itself. Resident set size is a property of the process, so a budget test
//! running beside other budget tests measures their fixtures as much as its
//! own — and because RSS does not shrink on free, a reading taken after
//! someone else's peak can come out as zero and pass having measured
//! nothing.

use aether_tui::testing::loaded_app;
use aether_tui::ui;
use aether_tui::ui::transcript::ViewCache;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

#[test]
fn ten_thousand_transcript_cells_stay_under_a_hundred_megabytes() {
    let Some(before) = resident_bytes() else {
        println!("resident memory unavailable on this platform; skipped");
        return;
    };
    let app = loaded_app(8, 15_000, 5_000);
    let mut terminal = Terminal::new(TestBackend::new(160, 200)).expect("terminal");
    let mut cache = ViewCache::default();
    terminal
        .draw(|f| ui::render(f, &app, &mut cache))
        .expect("draw");
    let after = resident_bytes().expect("resident memory");
    let used = after.saturating_sub(before);
    println!(
        "10 000 transcript cells: {:.1} MB resident",
        used as f64 / 1e6
    );
    assert_eq!(
        app.transcripts
            .values()
            .map(|t| t.cells.len())
            .sum::<usize>(),
        10_000,
        "the fixture must actually hold 10 000 cells"
    );
    assert!(
        used > 0,
        "measured no growth at all, so this measured nothing"
    );
    assert!(
        used < 100 * 1_000_000,
        "10 000 cells took {used} bytes, over the 100 MB budget"
    );
}

/// Resident set size in bytes, from `/proc/self/statm`.
fn resident_bytes() -> Option<u64> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4_096)
}
