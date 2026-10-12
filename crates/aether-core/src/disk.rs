//! Free space where the daemon keeps its data. A full disk refuses the write
//! a turn has just made, so new turns wait while it runs low.

use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

/// The default floor is the smaller of these, so a small disk is not held
/// below a floor it could never clear.
const DEFAULT_FLOOR_BYTES: u64 = 5 << 30;
const DEFAULT_FLOOR_PER_TOTAL: u64 = 50;

/// Free space below which new turns wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskFloor {
    /// The smaller of 5 GiB and 2% of the filesystem.
    Default,
    Bytes(u64),
}

impl DiskFloor {
    /// The floor on a filesystem of `total` bytes.
    pub fn bytes(self, total: u64) -> u64 {
        match self {
            Self::Default => DEFAULT_FLOOR_BYTES.min(total / DEFAULT_FLOOR_PER_TOTAL),
            Self::Bytes(bytes) => bytes,
        }
    }
}

/// The filesystem holding the daemon's data is below its floor: new turns
/// wait until it has room again. Turns already running carry on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LowDisk {
    /// Where free space was measured: the store.
    pub path: String,
    /// As of the last measurement.
    pub available_bytes: u64,
    pub floor_bytes: u64,
    pub since: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_floor_is_five_gib_or_two_percent_whichever_is_smaller() {
        const GIB: u64 = 1 << 30;
        assert_eq!(DiskFloor::Default.bytes(2000 * GIB), 5 * GIB);
        assert_eq!(DiskFloor::Default.bytes(100 * GIB), 2 * GIB);
        assert_eq!(DiskFloor::Default.bytes(0), 0);
        assert_eq!(DiskFloor::Bytes(7).bytes(2000 * GIB), 7);
    }
}
