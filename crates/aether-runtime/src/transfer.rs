//! Moving a run between daemons: export to one file, import it elsewhere.
//!
//! The file is newline-delimited JSON - a manifest line, then one line per
//! record - so both directions stream and neither has to hold a run in
//! memory, and a person can read one with `head` or `grep`.
//!
//! Import recreates everything under fresh ids, with an event saying where
//! it came from. Ids are minted per store, so the imported run is a
//! different run that did the same things: `aether runs report` of the
//! import and of the export have the same body.

use crate::Result;
use crate::RuntimeError;
use aether_core::id::RunId;
use std::path::Path;

mod export;
mod import;
mod reader;

pub use export::export_run;
pub use import::import_run;

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Invalid(message.into())
}

/// Everything in the file belongs to the run the manifest names. A record
/// naming another run would be minted a run id of its own here - agents in a
/// run with no provenance event, absent from the summary - so the file is
/// refused instead.
fn same_run(path: &Path, what: &str, run: RunId, expected: RunId) -> Result<()> {
    if run == expected {
        return Ok(());
    }
    Err(invalid(format!(
        "{}: an {what} belongs to run {run}, which is not the {expected} the manifest names",
        path.display()
    )))
}
