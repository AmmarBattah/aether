use super::{App, Effect, Install, ModelKind, VoiceEffect};

impl App {
    /// Starts installing `model`. The `/voice` confirm view is the explicit
    /// gate before this; nothing else emits `VoiceEffect::Setup`.
    pub(crate) fn voice_install(&mut self, model: ModelKind) -> Vec<Effect> {
        if !self.voice.available {
            self.flash(
                "local voice requires the interactive TUI on Linux or macOS; see docs/voice.md",
            );
            return Vec::new();
        }
        if self.voice.install.is_some() {
            self.flash("a voice install is already running; /voice cancel stops it");
            return Vec::new();
        }
        let Some(dir) = self.voice_dir() else {
            self.flash("voice configuration directory unavailable; see docs/voice.md");
            return Vec::new();
        };
        let id = self.voice.id();
        self.voice.install = Some(Install {
            id,
            model,
            stage: "starting".into(),
            cancelled: false,
        });
        self.dirty = true;
        vec![Effect::Voice(VoiceEffect::Setup { id, dir, model })]
    }

    /// Cancels the running install at most once.
    pub(super) fn cancel_install(&mut self) -> Vec<Effect> {
        let Some(install) = &mut self.voice.install else {
            return Vec::new();
        };
        if install.cancelled {
            return Vec::new();
        }
        install.cancelled = true;
        let id = install.id;
        self.flash("cancelling voice setup; any partial install is cleaned up");
        self.dirty = true;
        vec![Effect::Voice(VoiceEffect::Cancel(id))]
    }

    /// Finish the install `id`. Returns whether it actually succeeded.
    pub(super) fn setup_done(&mut self, id: u64, result: Result<String, String>) -> bool {
        if self.voice.install.as_ref().is_none_or(|i| i.id != id) {
            return false;
        }
        self.voice.install = None;
        match result {
            Ok(message) => {
                // What was missing may be installed now; the next check decides.
                self.voice.missing = None;
                self.flash(self.voice_key_hint(&message));
                true
            }
            Err(error) => {
                self.flash_error(format!("voice setup failed: {error}"));
                false
            }
        }
    }
}
