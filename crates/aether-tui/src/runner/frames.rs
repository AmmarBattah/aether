//! When the loop draws (§9.7, §9.8). Frames are requested, never ticked: a
//! change marks the app dirty, animations ask for their next tick through
//! [`App::next_tick_in`], and with neither the loop sleeps until an event.

use std::time::{Duration, Instant};

use crate::app::{App, AppEvent, Effect, TICKS_PER_SECOND};

/// Shortest gap between two frames: the frame rate cap.
pub(crate) const FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / TICKS_PER_SECOND);

/// Shortest gap before a frame only background changes asked for: other
/// agents and status polls redraw at most this often.
pub(crate) const BACKGROUND_INTERVAL: Duration = Duration::from_millis(100);

/// The frame clock and the draw throttle, kept apart from the terminal so the
/// policy can be tested with a simulated clock.
#[derive(Debug)]
pub(crate) struct Frames {
    start: Instant,
    last_draw: Option<Instant>,
    /// When the loop first saw a change still undrawn: a backlog's grace runs
    /// from here, so a burst after a quiet spell is not drawn at its first event.
    requested_at: Option<Instant>,
}

impl Frames {
    pub(crate) fn new(start: Instant) -> Frames {
        Frames {
            start,
            last_draw: None,
            requested_at: None,
        }
    }

    /// Bring `app.tick` up to wall time. Rendering reads the tick and `now`
    /// rather than the clock, so this runs before every draw.
    pub(crate) fn advance_clock(&self, app: &mut App, now: Instant) -> Vec<Effect> {
        let due = self.frame_at(now);
        if due <= app.tick {
            return Vec::new();
        }
        app.reduce(AppEvent::Tick {
            frames: due - app.tick,
        })
    }

    /// Whether to draw at `now`. A `backlog` of events still queued holds the
    /// frame back so they land in one draw, but never past a frame's grace.
    pub(crate) fn should_draw(&mut self, app: &App, now: Instant, backlog: bool) -> bool {
        let Some(due) = self.draw_due(app) else {
            return false;
        };
        let requested = *self.requested_at.get_or_insert(now);
        now >= due && (!backlog || now >= due.max(requested) + FRAME_INTERVAL)
    }

    pub(crate) fn drew(&mut self, app: &mut App, at: Instant) {
        self.last_draw = Some(at);
        self.requested_at = None;
        app.dirty = false;
        app.background_dirty = false;
    }

    /// When the loop must wake with no event: the pending frame, or the tick
    /// an animation or deadline asked for. `None` sleeps until an event.
    pub(crate) fn next_wake(&self, app: &App) -> Option<Instant> {
        let tick = app
            .next_tick_in()
            .and_then(|frames| self.instant_of(app.tick.saturating_add(frames)));
        match (self.draw_due(app), tick) {
            (Some(draw), Some(tick)) => Some(draw.min(tick)),
            (draw, tick) => draw.or(tick),
        }
    }

    /// The earliest a frame may be drawn, if anything needs drawing.
    fn draw_due(&self, app: &App) -> Option<Instant> {
        let gap = if app.dirty {
            FRAME_INTERVAL
        } else if app.background_dirty {
            BACKGROUND_INTERVAL
        } else {
            return None;
        };
        Some(self.last_draw.map_or(self.start, |last| last + gap))
    }

    fn frame_at(&self, now: Instant) -> u64 {
        let nanos = now.saturating_duration_since(self.start).as_nanos();
        u64::try_from(nanos * u128::from(TICKS_PER_SECOND) / 1_000_000_000).unwrap_or(u64::MAX)
    }

    /// The first instant at which [`Frames::frame_at`] reaches `frame`;
    /// `None` past what the clock can represent.
    fn instant_of(&self, frame: u64) -> Option<Instant> {
        let nanos = (u128::from(frame) * 1_000_000_000).div_ceil(u128::from(TICKS_PER_SECOND));
        self.start
            .checked_add(Duration::from_nanos(u64::try_from(nanos).ok()?))
    }
}
