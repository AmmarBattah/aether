//! Global replies never borrow the composer or the selected-agent target.
use aether_client::ClientError;
use aether_core::id::EventId;
use aether_protocol::{Command, Reply as DaemonReply};

use super::*;
use crate::app::Then;
use crate::voice::control::ControlAction;

const DEBOUNCE_TICKS: u64 = crate::app::TICKS_PER_SECOND / 2;
const ARM_TICKS: u64 = crate::app::TICKS_PER_SECOND * 120;

#[derive(Debug, Default)]
pub(super) struct ReplyState {
    pub(super) enabled: bool,
    pub(super) checking: Option<u64>,
    pub(super) armed: Option<Question>,
    pub(super) active: Option<ReplyCapture>,
    last_toggle: Option<u64>,
    unavailable: Option<(AgentId, u64, &'static str)>,
    last_outcome: Option<(&'static str, u64)>,
}

#[derive(Debug, PartialEq, Eq)]
enum Phase {
    Starting,
    Recording,
    Stopping,
    Transcribing,
    Sending,
    Cancelling,
}

#[derive(Debug)]
pub(super) struct ReplyCapture {
    pub(super) id: u64,
    question: Question,
    phase: Phase,
    authorized: bool,
    cancellation_reason: &'static str,
}

impl ReplyState {
    pub(super) fn status(&self) -> Option<&'static str> {
        if self.checking.is_some() {
            return Some("checking: validating STT/TTS configuration; no devices opened");
        }
        self.active.as_ref().map(|capture| match capture.phase {
            Phase::Starting => "starting: recording requested; microphone not yet confirmed; reply stops, cancel discards",
            Phase::Stopping => "stopping: stop requested; transcription not yet started; cancel discards",
            Phase::Recording => "recording: audio received; reply stops and sends without review; cancel discards",
            Phase::Transcribing => "transcribing: cancel discards; delivery not yet submitted",
            Phase::Sending => "sending: delivery requested; not yet confirmed; cannot retract",
            Phase::Cancelling if capture.cancellation_reason == "target_changed" => "cancelling: captured question changed or is no longer pending; nothing will be sent",
            Phase::Cancelling => "cancelling: discard requested; awaiting recorder cleanup",
        })
    }
}

impl App {
    /// Read-only, content-free snapshot shared by the CLI and /voice status.
    pub fn voice_reply_diagnostic(&self) -> String {
        let state = self.voice_key_hint(self.reply_eligibility());
        match self.voice.replies.last_outcome {
            Some((outcome, tick)) => format!(
                "{state}; last={outcome} ({}s ago)",
                self.tick.saturating_sub(tick) / crate::app::TICKS_PER_SECOND
            ),
            None => state,
        }
    }

    pub fn voice_control_ack(&self, action: ControlAction) -> String {
        if action == ControlAction::Status {
            return self.voice_reply_diagnostic();
        }
        if action == ControlAction::Cancel && self.voice.replies.active.is_none() {
            return "idle: no active recording; any armed reply cleared; nothing sent".into();
        }
        self.voice
            .replies
            .status()
            .unwrap_or("idle: nothing sent")
            .into()
    }

    fn reply_eligibility(&self) -> &'static str {
        if let Some(status) = self.voice.replies.status() {
            return status;
        }
        if !self.voice.available || !self.voice.replies.enabled || !self.voice.enabled {
            if let Some((_, missing)) = self.voice.missing {
                return missing.reply_diagnostic();
            }
            return "disabled: enable /voice replies on in the connected TUI";
        }
        if !self.connected || self.quit_requested {
            return "disconnected: reconnect the TUI and enable /voice replies on";
        }
        if self.voice.capture.is_some() {
            return "busy_dictation: F4 owns the microphone; finish or cancel it first";
        }
        if let Some(question) = &self.voice.replies.armed {
            if !self.reply_question_current(question) {
                return "target_changed: spoken question no longer current; wait for a new spoken question";
            }
            if self.tick >= question.expires {
                return "expired: 120s start window elapsed; answer in TUI or wait for a new question";
            }
            return "ready: newly spoken pending question; reply starts recording";
        }
        if let Some(speech) = &self.voice.speech
            && self.reply_question_current(&speech.question)
        {
            return if speech.cancelled {
                "speech_cancelled: playback did not complete; answer in TUI or wait for a new question"
            } else if speech.playing {
                "playback_pending: wait for successful playback completion; microphone not opened"
            } else {
                "synthesizing: wait for question speech and playback completion; microphone not opened"
            };
        }
        if self
            .voice
            .queue
            .iter()
            .any(|q| self.tick < q.expires && self.reply_question_current(q))
        {
            return "speech_queued: wait for question speech and playback completion";
        }
        if let Some((agent, seq, reason)) = self.voice.replies.unavailable
            && self.reply_target_current(agent, seq)
        {
            return reason;
        }
        if !self.transcripts.iter().any(|(agent, transcript)| {
            let transcript = transcript.live_state();
            transcript.cells.iter().any(|cell| match cell.model {
                CellModel::Question {
                    state: crate::app::transcript::QuestionState::Pending,
                    seq,
                    ..
                } => self.question_pending(*agent, seq),
                _ => false,
            })
        }) {
            return "no_pending_question: no pending ask_user; wait for an agent question";
        }
        "not_spoken: no eligible newly spoken question; answer in TUI or wait for a new question"
    }

    pub(super) fn reply_unavailable(&mut self, reason: &'static str) {
        if let Some(speech) = &self.voice.speech {
            self.voice.replies.unavailable =
                Some((speech.question.agent, speech.question.seq, reason));
        }
    }

    fn reply_outcome(&mut self, outcome: &'static str) {
        self.voice.replies.last_outcome = Some((outcome, self.tick));
    }

    pub fn voice_replies_enabled(&self) -> bool {
        self.voice.replies.enabled
    }

    pub(super) fn enable_voice_replies(&mut self) -> Vec<Effect> {
        if !self.connected {
            self.flash("connect to the daemon before enabling global voice replies");
            return Vec::new();
        }
        if self.voice.replies.enabled {
            self.flash("global voice replies already on; second shortcut sends without review");
            return Vec::new();
        }
        if self.voice.checking.is_some() || self.voice.replies.checking.is_some() {
            self.flash("voice configuration check already running");
            return Vec::new();
        }
        let effects = self.start_replies_check();
        if effects.is_empty() {
            self.flash("voice configuration directory unavailable");
            return effects;
        }
        self.flash(
            "checking STT and TTS; global reply will send without review on second invocation",
        );
        effects
    }

    /// Start the one check that validates dictation and speech together; the
    /// caller owns the notice, since a restored setting reads differently.
    pub(super) fn start_replies_check(&mut self) -> Vec<Effect> {
        debug_assert!(
            self.voice.checking.is_none() && self.voice.replies.checking.is_none(),
            "callers must reject a second concurrent check"
        );
        let Some(dir) = self.voice_dir() else {
            return Vec::new();
        };
        let id = self.voice.id();
        self.voice.replies.checking = Some(id);
        vec![Effect::Voice(VoiceEffect::CheckReplies { id, dir })]
    }

    /// `restored` marks a check started by a saved setting: it reports
    /// differently and saves nothing, since it matches what is already on file.
    pub(super) fn voice_replies_checked(
        &mut self,
        restored: bool,
        result: Result<String, CheckFailure>,
    ) {
        self.voice.replies.checking = None;
        match result {
            Ok(message) if self.connected && !self.quit_requested => {
                self.voice.enabled = true;
                self.voice.replies.enabled = true;
                self.voice.replies.armed = None;
                if restored {
                    self.voice.restored = true;
                    self.flash(format!("{message}; saved setting restored: global replies are armed for questions asked from now on; nothing is recorded or sent until you press the shortcut"));
                    return;
                }
                let saved = self.persist_voice(VoicePrefs {
                    questions: true,
                    replies: true,
                });
                self.flash(self.voice_key_hint(&format!("{message}; global replies ON for newly spoken questions: first shortcut records, second transcribes and sends WITHOUT review; global cancel discards; F4 unchanged{saved}")));
            }
            Ok(_) => self.flash("global voice replies not enabled: disconnected or quitting"),
            Err(error) if restored => self.flash_error(format!(
                "saved voice settings not restored: {error}; /voice opens settings and installs what is missing"
            )),
            Err(error) if error.missing.is_some() => {
                self.flash_error(format!("global voice replies stay off: {error}"))
            }
            Err(error) => self.flash_error(format!(
                "global voice replies configuration failed: {error}"
            )),
        }
    }

    /// Turn global replies off for this run without touching what was saved: an
    /// environment failure is not the user opting out.
    pub(crate) fn disarm_voice_replies_transiently(&mut self) -> Vec<Effect> {
        self.voice.restore = None;
        self.disarm_voice_replies()
    }

    pub(super) fn disarm_voice_replies(&mut self) -> Vec<Effect> {
        self.voice.replies.enabled = false;
        self.voice.replies.checking = None;
        self.voice.replies.armed = None;
        self.cancel_voice_reply()
    }

    pub(super) fn cancel_voice_reply(&mut self) -> Vec<Effect> {
        if let Some(question) = self.voice.replies.armed.take() {
            self.voice.replies.unavailable = Some((
                question.agent,
                question.seq,
                "cancelled: armed reply cleared; answer in TUI or wait for a new question",
            ));
        }
        let Some(capture) = &mut self.voice.replies.active else {
            return Vec::new();
        };
        match capture.phase {
            Phase::Sending => {
                self.flash("voice reply already submitted; cancellation cannot retract it; awaiting result");
                Vec::new()
            }
            Phase::Cancelling => Vec::new(),
            _ => {
                capture.phase = Phase::Cancelling;
                capture.authorized = false;
                let id = capture.id;
                self.flash("voice reply cancelling; composer unchanged");
                vec![Effect::Voice(VoiceEffect::Cancel(id))]
            }
        }
    }

    fn reply_question_live(&self, question: &Question) -> bool {
        self.tick < question.expires && self.reply_question_current(question)
    }

    fn reply_question_current(&self, question: &Question) -> bool {
        self.reply_target_current(question.agent, question.seq)
    }

    fn reply_target_current(&self, agent: AgentId, seq: u64) -> bool {
        self.question_pending(agent, seq)
            && self.transcripts.get(&agent).is_some_and(|transcript| {
                let transcript = transcript.live_state();
                !transcript.cells.iter().any(
                    |cell| matches!(cell.model, CellModel::Question { seq: later, .. } if later > seq),
                )
            })
    }

    pub(super) fn arm_spoken_reply(&mut self, id: u64, result: &Result<(), String>) {
        if !self.voice.replies.enabled || self.voice.replies.active.is_some() || result.is_err() {
            return;
        }
        let Some(speech) = &self.voice.speech else {
            return;
        };
        if speech.id != id || !speech.playing || speech.cancelled || !self.connected {
            return;
        }
        let mut question = speech.question.clone();
        question.expires = self.tick + ARM_TICKS;
        if self.reply_question_live(&question) {
            self.flash(format!("global voice reply ready for {} question {}; shortcut records, second sends without review", self.agent_name(question.agent), question.seq));
            self.voice.replies.armed = Some(question);
        }
    }

    pub(super) fn reconcile_voice_replies(&mut self) -> Vec<Effect> {
        if !self.connected
            || self.quit_requested
            || (self.voice.replies.enabled && !self.voice.enabled)
        {
            return self.disarm_voice_replies();
        }
        if self
            .voice
            .replies
            .armed
            .as_ref()
            .is_some_and(|q| !self.reply_question_live(q))
            && let Some(question) = self.voice.replies.armed.take()
        {
            let reason = if self.tick >= question.expires {
                "expired: 120s start window elapsed; answer in TUI or wait for a new question"
            } else {
                "target_changed: spoken question no longer current; wait for a new spoken question"
            };
            self.voice.replies.unavailable = Some((question.agent, question.seq, reason));
        }
        if self.voice.replies.active.as_ref().is_some_and(|c| {
            !matches!(c.phase, Phase::Sending | Phase::Cancelling)
                && !self.reply_question_current(&c.question)
        }) {
            if let Some(capture) = &mut self.voice.replies.active {
                capture.cancellation_reason = "target_changed";
            }
            let effects = self.cancel_voice_reply();
            self.flash(
                "voice reply cancelled: asking question expired, changed, answered, or agent ended",
            );
            return effects;
        }
        Vec::new()
    }

    /// Called by the private local control listener; success acknowledges accepted effects, not delivery.
    pub fn handle_voice_control(&mut self, action: ControlAction) -> Result<Vec<Effect>, String> {
        if action == ControlAction::Status {
            return Ok(Vec::new());
        }
        self.dirty = true;
        if action == ControlAction::Cancel {
            if self
                .voice
                .replies
                .active
                .as_ref()
                .is_some_and(|c| c.phase == Phase::Sending)
            {
                return Err(
                    "voice reply already submitted; cannot retract it; awaiting result".into(),
                );
            }
            let effects = self.cancel_voice_reply();
            self.flash(self.voice_control_ack(action));
            return Ok(effects);
        }
        if !self.voice.available
            || !self.voice.replies.enabled
            || !self.connected
            || self.quit_requested
        {
            return Err(self.voice_key_hint(self.reply_eligibility()));
        }
        if self.voice.capture.is_some() {
            return Err(self.voice_key_hint(self.reply_eligibility()));
        }
        if self
            .voice
            .replies
            .last_toggle
            .is_some_and(|last| self.tick.saturating_sub(last) < DEBOUNCE_TICKS)
        {
            self.voice.replies.last_toggle = Some(self.tick);
            return Err(
                "voice reply shortcut repeated too quickly; release it before toggling again"
                    .into(),
            );
        }
        if let Some(capture) = &self.voice.replies.active {
            if !matches!(capture.phase, Phase::Starting | Phase::Recording) {
                return Err(self.voice_key_hint(self.reply_eligibility()));
            }
            if !self.reply_question_current(&capture.question) {
                return Err("captured question is no longer pending; cancel this recording".into());
            }
            let Some(capture) = self.voice.replies.active.as_mut() else {
                return Err("voice reply capture no longer active".into());
            };
            capture.authorized = true;
            capture.phase = Phase::Stopping;
            let id = capture.id;
            self.voice.replies.last_toggle = Some(self.tick);
            self.flash(self.voice_control_ack(action));
            return Ok(vec![Effect::Voice(VoiceEffect::Stop(id))]);
        }
        let question = self
            .voice
            .replies
            .armed
            .clone()
            .filter(|q| self.reply_question_live(q))
            .ok_or_else(|| self.reply_eligibility().to_owned())?;
        let dir = self
            .voice_dir()
            .ok_or("voice configuration directory unavailable")?;
        let id = self.voice.id();
        self.voice.replies.armed = None;
        self.voice.replies.unavailable = Some((
            question.agent,
            question.seq,
            "consumed: reply attempt used this question; answer in TUI or wait for a new question",
        ));
        self.voice.replies.last_toggle = Some(self.tick);
        self.voice.replies.active = Some(ReplyCapture {
            id,
            question: question.clone(),
            phase: Phase::Starting,
            authorized: false,
            cancellation_reason: "cancelled",
        });
        self.voice.queue.clear();
        let mut effects = self.cancel_speech();
        effects.push(Effect::Voice(VoiceEffect::RecordReply { id, dir }));
        self.flash(self.voice_control_ack(action));
        Ok(effects)
    }

    pub(super) fn voice_reply_recording(&mut self, id: u64) {
        if let Some(capture) = &mut self.voice.replies.active
            && capture.id == id
            && capture.phase == Phase::Starting
        {
            capture.phase = Phase::Recording;
            self.flash(self.voice_control_ack(ControlAction::Reply));
        }
    }

    pub(super) fn voice_reply_transcribing(&mut self, id: u64) {
        if let Some(capture) = &mut self.voice.replies.active
            && capture.id == id
            && matches!(
                capture.phase,
                Phase::Starting | Phase::Recording | Phase::Stopping | Phase::Transcribing
            )
        {
            capture.phase = Phase::Transcribing;
        }
    }

    pub(super) fn voice_reply_recorded(
        &mut self,
        id: u64,
        result: Result<String, String>,
    ) -> Vec<Effect> {
        let Some(mut capture) = self.voice.replies.active.take() else {
            return Vec::new();
        };
        if capture.id != id || capture.phase == Phase::Sending {
            self.voice.replies.active = Some(capture);
            return Vec::new();
        }
        let text = match result {
            Ok(text) => text,
            Err(error) => {
                self.reply_outcome(
                    if capture.phase == Phase::Cancelling && error == "cancelled" {
                        capture.cancellation_reason
                    } else if capture.phase == Phase::Transcribing {
                        "transcription_failed"
                    } else {
                        "capture_failed"
                    },
                );
                self.flash_error(format!("voice reply not sent: {error}; composer unchanged"));
                return Vec::new();
            }
        };
        if capture.phase == Phase::Cancelling
            || !capture.authorized
            || !self.voice.replies.enabled
            || !self.connected
            || self.quit_requested
            || !self.reply_question_current(&capture.question)
        {
            self.reply_outcome(if capture.phase == Phase::Cancelling {
                capture.cancellation_reason
            } else {
                "discarded"
            });
            self.flash("voice reply discarded: cancelled, not explicitly stopped, or question no longer pending; nothing sent");
            return Vec::new();
        }
        if text.trim().is_empty() {
            self.reply_outcome("empty_transcription");
            self.flash("voice reply transcription was empty; nothing sent");
            return Vec::new();
        }
        let command = Command::ReplyToQuestion {
            to: capture.question.agent,
            question_seq: capture.question.seq,
            content: text,
            idempotency_key: Some(format!("voice-reply-{}", EventId::new())),
        };
        capture.phase = Phase::Sending;
        self.voice.replies.active = Some(capture);
        self.flash("voice reply sending to captured question; composer unchanged");
        vec![Effect::Request {
            command,
            then: Then::VoiceReply(id),
        }]
    }

    pub(crate) fn voice_send_result(
        &mut self,
        id: u64,
        result: Result<DaemonReply, ClientError>,
    ) -> Vec<Effect> {
        if !self
            .voice
            .replies
            .active
            .as_ref()
            .is_some_and(|c| c.id == id && c.phase == Phase::Sending)
        {
            return Vec::new();
        }
        let Some(capture) = self.voice.replies.active.take() else {
            return Vec::new();
        };
        let target = format!(
            "{} question {}",
            self.agent_name(capture.question.agent),
            capture.question.seq
        );
        self.reply_outcome(match &result {
            Ok(DaemonReply::MessageSent { .. }) => "sent",
            _ => "delivery_unconfirmed_check_TUI",
        });
        match result {
            Ok(DaemonReply::MessageSent { .. }) => self.flash(format!("voice reply sent to {target}")),
            Ok(_) => self.flash_error(format!("voice reply delivery to {target} unconfirmed: unexpected daemon response; not retried")),
            Err(error) => self.flash_error(format!("voice reply to {target} failed or delivery unconfirmed: {error}; not retried; check the conversation")),
        }
        Vec::new()
    }
}
