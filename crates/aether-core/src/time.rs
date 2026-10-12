//! Time primitives. One alias so every crate agrees on the clock type.

use chrono::{DateTime, Utc};

/// UTC timestamp with millisecond precision in serialized form.
pub type Timestamp = DateTime<Utc>;

/// The current time.
pub fn now() -> Timestamp {
    Utc::now()
}
