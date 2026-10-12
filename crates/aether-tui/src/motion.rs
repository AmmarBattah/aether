//! VISION §9.7 as code: the only place the TUI decides how fast anything
//! moves.
//!
//! One clock drives everything — `App::tick` counts frames at [`FRAME_HZ`],
//! and every animation is a pure function of that count. Nothing here reads
//! the wall clock, so the whole interaction model stays testable, and the
//! runner can skip frames entirely while nothing animates (§9.8: no timers
//! when nothing moves).

/// Frames per second while something animates.
pub const FRAME_HZ: u64 = 60;

/// Ten braille frames at 80 ms, only while an agent is running.
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const SPINNER_MS: u64 = 80;
/// Shown instead of the animation under reduced motion: presence, not motion.
pub const SPINNER_STILL: &str = "⠿";

/// The caret blinks at 530 ms. The input's caret is the terminal's own
/// cursor, which the terminal blinks for free; this governs the caret that
/// trails streaming text.
pub const CARET_MS: u64 = 530;

/// Eased scrolling settles in 120 ms, so it needs every frame.
pub const SCROLL_MS: u64 = 1_000 / FRAME_HZ;
const SCROLL_EASE_MS: u64 = 120;
/// Below this many rows the remaining distance is not worth a frame.
const SCROLL_SNAP: f32 = 0.5;

/// Notices appear at once and clear after four seconds.
pub const NOTICE_MS: u64 = 4_000;

pub const fn frames(ms: u64) -> u64 {
    let n = ms * FRAME_HZ / 1_000;
    if n == 0 { 1 } else { n }
}

/// What a scroll was asked for by. Keys are exact and instant; the wheel and
/// following the tail are continuous, so they ease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollSource {
    Key,
    Wheel,
    Follow,
}

/// The motion rules, with reduced motion folded in so no caller has to
/// remember to check it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Motion {
    reduced: bool,
}

impl Motion {
    pub fn new(reduced: bool) -> Motion {
        Motion { reduced }
    }

    /// `reduced_motion` in `ui.toml`, or `AETHER_REDUCED_MOTION` in the
    /// environment (any non-empty value but `0` and `false`).
    pub fn env_reduced() -> Option<bool> {
        Self::reduced_from_env(&std::env::var("AETHER_REDUCED_MOTION").ok()?)
    }

    /// The parse on its own, so it can be tested without touching the
    /// process environment. An empty value is no answer, not "off".
    pub fn reduced_from_env(raw: &str) -> Option<bool> {
        let raw = raw.trim().to_ascii_lowercase();
        if raw.is_empty() {
            return None;
        }
        Some(!matches!(raw.as_str(), "0" | "false" | "no" | "off"))
    }

    pub fn reduced(&self) -> bool {
        self.reduced
    }

    pub fn label(&self) -> &'static str {
        if self.reduced { "reduced" } else { "full" }
    }

    /// The spinner frame for `tick`; a still glyph under reduced motion.
    pub fn spinner(&self, tick: u64) -> &'static str {
        if self.reduced {
            return SPINNER_STILL;
        }
        SPINNER[(tick / frames(SPINNER_MS)) as usize % SPINNER.len()]
    }

    /// Whether the streaming caret is currently drawn. Solid under reduced
    /// motion; absent when nothing is streaming.
    pub fn caret_on(&self, tick: u64, streaming: bool) -> bool {
        if !streaming {
            return false;
        }
        if self.reduced {
            return true;
        }
        (tick / frames(CARET_MS)).is_multiple_of(2)
    }

    /// Move `current` towards `target` for one frame. Returns whether it
    /// moved, so an idle screen can skip the redraw.
    pub fn ease_scroll(&self, current: &mut f32, target: f32, source: ScrollSource) -> bool {
        let instant = self.reduced || source == ScrollSource::Key;
        let diff = target - *current;
        if instant || diff.abs() < SCROLL_SNAP {
            let moved = *current != target;
            *current = target;
            return moved;
        }
        // Exponential approach that covers the distance within the budget:
        // after SCROLL_EASE_MS frames the remainder is under SCROLL_SNAP.
        *current += diff * ease_rate();
        true
    }
}

/// Per-frame fraction of the remaining distance, chosen so a scroll settles
/// within [`SCROLL_EASE_MS`].
fn ease_rate() -> f32 {
    // (1 - rate)^n = 0.02 over n frames leaves 2% of the distance.
    let n = frames(SCROLL_EASE_MS) as f32;
    1.0 - 0.02_f32.powf(1.0 / n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spinner_turns_at_eighty_milliseconds() {
        let m = Motion::new(false);
        let step = frames(SPINNER_MS);
        assert_eq!(step, 4, "80 ms is four frames at 60 Hz");
        assert_eq!(m.spinner(0), SPINNER[0]);
        assert_eq!(
            m.spinner(step - 1),
            SPINNER[0],
            "no frame within the period"
        );
        assert_eq!(m.spinner(step), SPINNER[1]);
        // Ten frames, then round.
        assert_eq!(m.spinner(step * 10), SPINNER[0]);
    }

    #[test]
    fn reduced_motion_freezes_the_spinner_and_the_caret() {
        let m = Motion::new(true);
        for tick in [0, 1, 7, 999] {
            assert_eq!(m.spinner(tick), SPINNER_STILL);
            assert!(m.caret_on(tick, true), "a still caret stays visible");
        }
        assert!(!m.caret_on(0, false), "no caret when nothing streams");
    }

    #[test]
    fn the_caret_blinks_only_while_text_streams() {
        let m = Motion::new(false);
        let half = frames(CARET_MS);
        assert!(m.caret_on(0, true));
        assert!(!m.caret_on(half, true));
        assert!(m.caret_on(half * 2, true));
        for tick in [0, half, half * 2] {
            assert!(!m.caret_on(tick, false));
        }
    }

    #[test]
    fn keys_scroll_instantly_and_the_wheel_eases_within_the_budget() {
        let m = Motion::new(false);
        let mut at = 0.0;
        assert!(m.ease_scroll(&mut at, 100.0, ScrollSource::Key));
        assert_eq!(at, 100.0, "a key press lands on the row it asked for");
        assert!(
            !m.ease_scroll(&mut at, 100.0, ScrollSource::Key),
            "no work left"
        );

        let mut at = 0.0;
        assert!(m.ease_scroll(&mut at, 100.0, ScrollSource::Wheel));
        assert!(at > 0.0 && at < 100.0, "the wheel eases, it does not jump");
        for _ in 1..frames(SCROLL_EASE_MS) {
            m.ease_scroll(&mut at, 100.0, ScrollSource::Wheel);
        }
        assert!(
            (100.0 - at) < 2.5,
            "a wheel scroll must be all but done after 120 ms, was {at}"
        );

        // Reduced motion makes even the wheel instant.
        let mut at = 0.0;
        Motion::new(true).ease_scroll(&mut at, 100.0, ScrollSource::Wheel);
        assert_eq!(at, 100.0);
    }
}
