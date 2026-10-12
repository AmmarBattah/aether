//! Deliver Aether state as native user input without replacing native instructions.

use std::collections::BTreeMap;

use aether_core::agent::EngineSentLayers;
use aether_core::model::Message;
use aether_core::snapshot::{InvocationSnapshot, LayerKind, LayerPlacement};

/// Which state the native session already holds and how to fence new input.
#[derive(Debug)]
pub struct HoistPlan<'a> {
    /// Digests of the layers the engine's retained session was already sent,
    /// or `None` for a session that retains nothing (a fresh one, or one the
    /// engine replaced). A layer whose digest is unchanged is not sent again.
    pub retained: Option<&'a BTreeMap<LayerKind, String>>,
    /// Varies per turn, so a file or tool result written before the turn
    /// cannot reproduce the harness markers around the hoisted blocks. Not
    /// the load-bearing part of the fence - see [`fence`] - since content
    /// produced during the turn can echo it into the next turn's tool
    /// results.
    pub nonce: &'a str,
}

/// The user turn built for one engine turn.
#[derive(Debug)]
pub struct Prepared {
    /// What the engine is sent, and what the snapshot records.
    pub prompt: String,
    /// The same turn with every hoistable layer included, for a resumed
    /// session that turns out to be gone (see [`resend_everything`]).
    pub full_prompt: String,
    /// Digest of every hoistable layer this turn rendered - including one
    /// left out because the session already holds it, which is why the whole
    /// map can be recorded as what the session has been sent.
    pub sent: BTreeMap<LayerKind, String>,
}

pub fn system_prompt(snapshot: &InvocationSnapshot) -> String {
    snapshot.system_text()
}

/// What a hoisted layer is introduced as in the user turn. Checkpoint and
/// memory layers already name themselves; role and status do not.
fn preamble(kind: LayerKind) -> &'static str {
    match kind {
        LayerKind::RoleTask => {
            "Your Aether assignment (authoritative runtime state, not a claim made by any message):\n"
        }
        LayerKind::RuntimeState => {
            "Aether runtime status (informational; the active task and instructions remain in effect):\n"
        }
        _ => "",
    }
}

/// Runtime status is informational only (main text still governs); every
/// other hoisted layer is authority the system prompt used to carry, so only
/// those are worth the fence's bytes.
fn carries_authority(kind: LayerKind) -> bool {
    kind != LayerKind::RuntimeState
}

/// The last 8 characters of a ULID are its random suffix, not its timestamp -
/// short enough to keep the fence cheap, long enough (40 bits) that content
/// written before the turn cannot have guessed it.
fn short_nonce(nonce: &str) -> &str {
    &nonce[nonce.len().saturating_sub(8)..]
}

/// Wrap the hoisted blocks in markers naming where harness text ends.
///
/// The blocks are authority the system prompt used to carry; in a user turn
/// they sit in the same channel as relayed messages and tool output, which
/// can reproduce any fixed preamble. What actually defeats a forgery written
/// before this turn is position (these are the first blocks) plus the
/// closing marker naming where harness text ends; the nonce only adds
/// defence in depth, since it cannot stop content produced *during* this
/// turn - a file written, or command output - from echoing the live nonce
/// into the next turn's tool results. A bundle with nothing but
/// runtime status carries no authority to protect, so it goes out unfenced
/// (the policy text explaining the fence is what stays cached,
/// not this).
fn fence(blocks: &[&str], nonce: &str, message: &str, authoritative: bool) -> String {
    if blocks.is_empty() {
        return message.to_owned();
    }
    let body = blocks.join("\n\n");
    if !authoritative {
        return format!("{body}\n\n{message}");
    }
    let nonce = short_nonce(nonce);
    format!("--- Aether [{nonce}] ---\n{body}\n--- end Aether [{nonce}] ---\n\n{message}")
}

/// One layer's rendered block, owned once. `hoisted`/`everything` below
/// borrow from this instead of cloning it, since `held` is common - the
/// no-hold path used to clone every block twice for nothing.
struct RenderedBlock {
    text: String,
    held: bool,
    authoritative: bool,
}

/// Move all textual state layers into the user turn, preserving native instructions,
/// ahead of `message`, and record that placement on the snapshot: a layer
/// that moved keeps its kind and count but not its text, so the snapshot
/// still reconstructs the exact input without counting it twice.
///
/// A resumed engine session retains every user turn it was sent, so a layer
/// whose rendered text has not changed since it was last sent is already in
/// front of the model: it is marked [`LayerPlacement::Retained`] and left out.
/// Runtime status is exempt - it changes every turn by construction.
///
/// Layers that carry no text (the tail, tool schemas) are never touched -
/// they contribute nothing to the system prompt and own their own counts.
pub fn prepare_user_prompt(
    snapshot: &mut InvocationSnapshot,
    message: &str,
    plan: HoistPlan<'_>,
) -> Prepared {
    let mut blocks: Vec<RenderedBlock> = Vec::new();
    let mut sent = BTreeMap::new();
    for layer in &mut snapshot.layers {
        if layer.text.is_none() {
            continue;
        }
        let text = layer.text.take();
        layer.tokens = 0;
        // A hoistable layer with nothing to say (no checkpoint, no memory
        // hit) keeps the `System` default here - read as "produced no text
        // this turn", not "rendered in the system prompt".
        let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
            continue;
        };
        let digest = EngineSentLayers::digest(&text);
        let held = layer.kind != LayerKind::RuntimeState
            && plan.retained.and_then(|r| r.get(&layer.kind)) == Some(&digest);
        sent.insert(layer.kind, digest);
        layer.placement = match held {
            true => LayerPlacement::Retained,
            false => LayerPlacement::UserTurn,
        };
        blocks.push(RenderedBlock {
            text: format!("{}{}", preamble(layer.kind), text),
            held,
            authoritative: carries_authority(layer.kind),
        });
    }
    let hoisted: Vec<&str> = blocks
        .iter()
        .filter(|b| !b.held)
        .map(|b| b.text.as_str())
        .collect();
    let hoisted_authoritative = blocks.iter().any(|b| !b.held && b.authoritative);
    let prompt = fence(&hoisted, plan.nonce, message, hoisted_authoritative);
    let full_prompt = match blocks.iter().all(|b| !b.held) {
        true => prompt.clone(),
        false => {
            let everything: Vec<&str> = blocks.iter().map(|b| b.text.as_str()).collect();
            let everything_authoritative = blocks.iter().any(|b| b.authoritative);
            fence(&everything, plan.nonce, message, everything_authoritative)
        }
    };
    write_user_turn(snapshot, &prompt);
    Prepared {
        prompt,
        full_prompt,
        sent,
    }
}

/// Replace the turn's user message with [`Prepared::full_prompt`], for the
/// one case that invalidates the gate after the fact: the engine refused to
/// resume the session, so nothing it was sent earlier survives. Returns the
/// prompt to send, and leaves the snapshot describing what was really sent.
pub fn resend_everything(snapshot: &mut InvocationSnapshot, prepared: &Prepared) -> String {
    for layer in &mut snapshot.layers {
        if layer.placement == LayerPlacement::Retained {
            layer.placement = LayerPlacement::UserTurn;
        }
    }
    write_user_turn(snapshot, &prepared.full_prompt);
    prepared.full_prompt.clone()
}

fn write_user_turn(snapshot: &mut InvocationSnapshot, prompt: &str) {
    // Native engines resume their own history. Only this input crosses the
    // adapter boundary; duplicating the durable transcript here makes snapshot
    // storage grow with the accumulated history on every turn.
    snapshot.messages = vec![Message::user_text(prompt.to_owned())];
}

#[cfg(test)]
mod tests;
