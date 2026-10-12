use super::{App, Effect, UiPrefs, VoicePrefs, saved_label};

impl App {
    /// Read the saved voice settings once, before the first frame. Nothing is
    /// enabled here; a saved opt-in only becomes a pending restoration.
    pub fn load_voice_prefs(&mut self) {
        assert!(
            !self.voice.enabled && !self.voice.replies.enabled,
            "preferences must load before anything can enable voice"
        );
        let Some(dir) = self.options.prefs_dir.as_deref() else {
            return;
        };
        match UiPrefs::load(dir) {
            Ok(prefs) => {
                let saved = prefs.voice();
                self.voice.saved = saved;
                self.voice.restore = saved.any().then_some(saved);
            }
            // A broken ui.toml must not silently become "voice off": this is
            // the only signal that a saved opt-in is being ignored.
            Err(error) => self.flash_error(format!(
                "saved voice settings unreadable: {}; voice stays off this run",
                error.reason
            )),
        }
    }

    /// Remember `saved` for the next launch; only explicit user intent reaches
    /// here. Returns a suffix naming a failed save, empty when it was written.
    pub(super) fn persist_voice(&mut self, saved: VoicePrefs) -> String {
        assert!(
            saved.questions || !saved.replies,
            "global replies must never outlive question speech"
        );
        debug_assert!(
            self.voice.available,
            "a TUI without local voice has nothing to save"
        );
        self.voice.restore = None;
        self.voice.overridden = true;
        match self.save_prefs(|prefs| prefs.set_voice(saved)) {
            Ok(()) => {
                self.voice.saved = saved;
                String::new()
            }
            // The cache still names what is on disk, because that is what the
            // next launch would restore.
            Err(reason) => {
                let notice = format!(
                    "NOT saved for next launch: {reason}; the next launch still restores {}",
                    saved_label(self.voice.saved)
                );
                self.flash_error(&notice);
                format!("; {notice}")
            }
        }
    }

    /// Declining replies must not cancel a saved question opt-in still being
    /// validated, so a combined check continues as the question-only one.
    pub(super) fn keep_question_restoration(&mut self) {
        let Some(id) = self.voice.replies.checking else {
            return;
        };
        if self.voice.restoring != Some(id) {
            return;
        }
        debug_assert!(self.voice.checking.is_none(), "one check at a time");
        self.voice.replies.checking = None;
        self.voice.checking = Some((id, true));
    }

    /// Question speech the user has asked for: on now, or a saved opt-in whose
    /// validation is still running.
    pub(super) fn questions_intended(&self) -> bool {
        let restoring = self.voice.restoring.is_some_and(|id| {
            self.voice.replies.checking == Some(id) || self.voice.checking == Some((id, true))
        });
        self.voice.enabled || restoring
    }

    /// A check still in flight must not enable voice once the reason for it is
    /// gone; the check task ignores cancellation, so intent is dropped here.
    pub(super) fn cancel_pending_enable(&mut self) {
        if let Some((_, enable)) = &mut self.voice.checking {
            *enable = false;
        }
        self.voice.restoring = None;
    }

    /// Stop speaking and disarm replies for this run. The caller decides
    /// whether this was a transient mute or a persistent opt-out.
    pub(super) fn voice_quiet(&mut self) -> Vec<Effect> {
        self.voice.recovered = None;
        self.voice.enabled = false;
        self.voice.queue.clear();
        self.cancel_pending_enable();
        self.voice.restore = None;
        self.voice.overridden = true;
        let mut effects = self.disarm_voice_replies();
        effects.extend(self.cancel_speech());
        effects
    }

    /// Apply a saved opt-in once, on a connected interactive TUI, through a
    /// typed command's validation. Never restores a recording or arms a reply.
    pub(super) fn restore_voice_prefs(&mut self) -> Vec<Effect> {
        if !self.voice.available || !self.connected || self.quit_requested {
            return Vec::new();
        }
        if self.voice.checking.is_some() || self.voice.replies.checking.is_some() {
            return Vec::new();
        }
        let Some(saved) = self.voice.restore else {
            return Vec::new();
        };
        debug_assert!(saved.any(), "an empty preference must never be pending");
        self.voice.restore = None;
        if self.voice_dir().is_none() {
            self.flash(
                "saved voice settings not restored: no configuration directory; see docs/voice.md",
            );
            return Vec::new();
        }
        // Replies validate speech and dictation together and turn question
        // speech on themselves, so exactly one check covers both settings.
        let effects = if saved.replies {
            self.start_replies_check()
        } else {
            self.voice_check(true)
        };
        let id = self
            .voice
            .checking
            .map(|(id, _)| id)
            .or(self.voice.replies.checking);
        assert_eq!(
            effects.len(),
            usize::from(id.is_some()),
            "a started restoration must emit exactly one check effect"
        );
        self.voice.restoring = id;
        if id.is_some() {
            self.flash(
                "restoring saved voice settings: checking the local backends; nothing is recorded or sent until you press a key",
            );
        }
        effects
    }

    /// After a successful install, retry a saved opt-in that failed to validate
    /// at launch. Never enables anything the user had not already saved.
    pub(super) fn rearm_saved_voice(&mut self) {
        if self.voice.overridden || !self.voice.saved.any() {
            return;
        }
        let satisfied =
            self.voice.enabled && (self.voice.replies.enabled || !self.voice.saved.replies);
        if !satisfied {
            self.voice.restore = Some(self.voice.saved);
        }
    }
}
