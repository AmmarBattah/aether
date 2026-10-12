//! Balance enhanced key reporting across suspension, panic, and normal exit.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub(crate) struct KeyboardMode {
    active: AtomicBool,
}

impl KeyboardMode {
    pub fn enable(&self, out: &mut impl Write) -> io::Result<()> {
        // Windows reports modifiers through its native input API; crossterm's
        // enhancement command is unsupported there.
        #[cfg(unix)]
        if !self.active.load(Ordering::Relaxed) {
            use crossterm::event::{KeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
            crossterm::execute!(
                out,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
            self.active.store(true, Ordering::Relaxed);
        }
        #[cfg(not(unix))]
        let _ = out;
        Ok(())
    }

    pub fn disable(&self, out: &mut impl Write) -> io::Result<()> {
        if self.active.swap(false, Ordering::Relaxed) {
            crossterm::execute!(out, crossterm::event::PopKeyboardEnhancementFlags)?;
        }
        Ok(())
    }
}

/// Crossterm exposes OSC replies as Alt+] followed by printable keys. Never
/// let reply payloads reach the composer or the approval hotkeys.
#[derive(Debug, Default)]
pub(crate) struct TerminalReplyFilter {
    osc: bool,
}

impl TerminalReplyFilter {
    pub fn consume(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        if key.code == KeyCode::Char(']') && key.modifiers == KeyModifiers::ALT {
            self.osc = true;
            return true;
        }
        if !self.osc {
            return false;
        }
        if (key.code == KeyCode::Char('\\') && key.modifiers == KeyModifiers::ALT)
            || key.code == KeyCode::Char('\x07')
            || (key.code == KeyCode::Char('g') && key.modifiers == KeyModifiers::CONTROL)
            || key.code == KeyCode::Esc
        {
            self.osc = false;
        }
        true
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn reporting_is_enabled_and_restored_once_per_terminal_session() -> io::Result<()> {
        let mode = KeyboardMode::default();
        let mut bytes = Vec::new();
        mode.enable(&mut bytes)?;
        mode.enable(&mut bytes)?;
        assert_eq!(bytes, b"\x1b[>1u");
        mode.disable(&mut bytes)?;
        mode.disable(&mut bytes)?;
        assert_eq!(bytes, b"\x1b[>1u\x1b[<1u");
        mode.enable(&mut bytes)?;
        mode.disable(&mut bytes)?;
        assert_eq!(bytes, b"\x1b[>1u\x1b[<1u\x1b[>1u\x1b[<1u");
        Ok(())
    }
}
