use super::invalid;
use crate::Result;
use aether_store::StoreError;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Longest line an import will read, matching the daemon's own socket frame
/// cap (`aether-daemon`'s `MAX_LINE`): one artifact payload travels
/// base64-encoded on one line, so the bound has to admit a real payload
/// while still refusing a line invented to exhaust memory.
const MAX_LINE_BYTES: u64 = 64 << 20;

/// Records one import will read. This is what bounds the transaction: every
/// row is held until the single commit, so the count, not the byte total, is
/// the memory that matters.
pub(super) const MAX_RECORDS: u64 = 500_000;

/// Bytes one import will read in total, payloads included. Well under the
/// 2 GiB the default retention policy is willing to keep across every
/// artifact in the store, so an import cannot fill a disk that retention
/// then has to dig out of.
const MAX_TOTAL_BYTES: u64 = 512 << 20;

/// Reads an export's lines under the three caps, so a hostile file is
/// refused by name rather than by running the daemon out of memory or disk.
///
/// `BufRead::lines` allocates to the next newline however far away it is,
/// which is the hole: the reader is capped per line and the budget restored
/// after each, the way `aether-client`'s MCP stdio reader does it.
pub(super) struct BoundedLines<R: BufRead> {
    inner: std::io::Take<R>,
    path: PathBuf,
    caps: Caps,
    records: u64,
    bytes: u64,
}

/// The three bounds, as data rather than as constants read inside the loop,
/// so a test can prove each refusal without writing half a gigabyte.
#[derive(Debug, Clone, Copy)]
struct Caps {
    line_bytes: u64,
    records: u64,
    total_bytes: u64,
}

impl Caps {
    const DEFAULT: Caps = Caps {
        line_bytes: MAX_LINE_BYTES,
        records: MAX_RECORDS,
        total_bytes: MAX_TOTAL_BYTES,
    };
}

impl<R: BufRead> BoundedLines<R> {
    pub(super) fn new(reader: R, path: &Path) -> Self {
        Self::with_caps(reader, path, Caps::DEFAULT)
    }

    fn with_caps(reader: R, path: &Path, caps: Caps) -> Self {
        Self {
            inner: reader.take(caps.line_bytes + 1),
            path: path.to_path_buf(),
            caps,
            records: 0,
            bytes: 0,
        }
    }

    /// The next line without its newline, or `None` at end of file.
    pub(super) fn next_line(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        let read = self.inner.read_line(&mut line).map_err(StoreError::Io)? as u64;
        if read == 0 {
            return Ok(None);
        }
        if read > self.caps.line_bytes {
            return Err(invalid(format!(
                "{}: a line passed the {}-byte cap",
                self.path.display(),
                self.caps.line_bytes
            )));
        }
        self.inner.set_limit(self.caps.line_bytes + 1);
        self.records += 1;
        self.bytes += read;
        if self.records > self.caps.records {
            return Err(invalid(format!(
                "{}: more than {} records",
                self.path.display(),
                self.caps.records
            )));
        }
        if self.bytes > self.caps.total_bytes {
            return Err(invalid(format!(
                "{}: more than {} bytes",
                self.path.display(),
                self.caps.total_bytes
            )));
        }
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }
        Ok(Some(line))
    }
}

#[cfg(test)]
mod bounds {
    use super::*;

    fn read_all(text: &str, caps: Caps) -> Result<usize> {
        let mut lines = BoundedLines::with_caps(
            std::io::BufReader::new(text.as_bytes()),
            Path::new("test.aether"),
            caps,
        );
        let mut n = 0;
        while lines.next_line()?.is_some() {
            n += 1;
        }
        Ok(n)
    }

    #[test]
    fn a_line_past_the_cap_is_refused_by_name() {
        let caps = Caps {
            line_bytes: 16,
            ..Caps::DEFAULT
        };
        assert_eq!(read_all("short\nalso short\n", caps).expect("under"), 2);
        let error = read_all(&format!("{}\n", "x".repeat(64)), caps).expect_err("over the cap");
        assert!(error.to_string().contains("16-byte cap"), "{error}");
    }

    #[test]
    fn the_line_budget_is_restored_per_line() {
        // The bug this guards: one `take` over the whole stream would refuse
        // the second line for what the first one spent.
        let caps = Caps {
            line_bytes: 8,
            ..Caps::DEFAULT
        };
        assert_eq!(
            read_all("aaaa\nbbbb\ncccc\ndddd\n", caps).expect("each fits"),
            4
        );
    }

    #[test]
    fn too_many_records_are_refused_by_name() {
        let caps = Caps {
            records: 3,
            ..Caps::DEFAULT
        };
        assert_eq!(read_all("a\nb\nc\n", caps).expect("at the cap"), 3);
        let error = read_all("a\nb\nc\nd\n", caps).expect_err("over the cap");
        assert!(error.to_string().contains("more than 3 records"), "{error}");
    }

    #[test]
    fn too_many_bytes_are_refused_by_name() {
        let caps = Caps {
            total_bytes: 10,
            ..Caps::DEFAULT
        };
        assert_eq!(read_all("aaaa\nbbbb\n", caps).expect("at the cap"), 2);
        let error = read_all("aaaa\nbbbb\ncccc\n", caps).expect_err("over the cap");
        assert!(error.to_string().contains("more than 10 bytes"), "{error}");
    }
}
