//! The optional semantic pass over memory.
//!
//! Structural and lexical retrieval are the store's job and always run. An
//! index sits behind this trait and adds one more signal: how close a memory
//! is in meaning to what the agent is doing now. It is optional by design -
//! a deployment with no index retrieves exactly as it did before.

use std::collections::BTreeMap;
use std::fmt;

use aether_core::id::MemoryId;
use aether_core::memory::MemoryRecord;

use crate::Result;

/// A semantic index over memory text.
///
/// `index` and `forget` are the seam a provider-backed implementation needs:
/// it embeds each record once, when it is admitted, and drops the vector when
/// the record is curated out, so a query does not pay for the whole corpus.
/// An index that computes cheaply enough to do it per query - like
/// [`LocalEmbeddingIndex`] - implements them as no-ops.
pub trait MemoryIndex: Send + Sync + fmt::Debug {
    /// Stable name, as it appears in config and in `--json` output.
    fn name(&self) -> &'static str;

    fn index(&self, record: &MemoryRecord) -> Result<()>;

    fn forget(&self, id: MemoryId) -> Result<()>;

    /// Similarity in 0..=1 of each record to `query`, in the order given.
    /// The returned vector always has one entry per record.
    fn similarity(&self, query: &str, records: &[MemoryRecord]) -> Result<Vec<f32>>;
}

/// Dimensions of the local embedding. Small enough to be free, wide enough
/// that unrelated trigrams rarely collide.
const DIMS: usize = 256;

/// A deterministic embedder with no network and no model: text is hashed into
/// a fixed vector over its character trigrams, and similarity is the cosine
/// between two such vectors.
///
/// Trigrams are the point. Lexical retrieval matches whole tokens, so it sees
/// nothing shared between "reindexing" and "rebuild the index"; trigrams do.
/// It is not a language model and does not pretend to be one - it is the
/// implementation that makes the seam real and testable without a key.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalEmbeddingIndex;

impl MemoryIndex for LocalEmbeddingIndex {
    fn name(&self) -> &'static str {
        "local"
    }

    fn index(&self, _record: &MemoryRecord) -> Result<()> {
        Ok(())
    }

    fn forget(&self, _id: MemoryId) -> Result<()> {
        Ok(())
    }

    fn similarity(&self, query: &str, records: &[MemoryRecord]) -> Result<Vec<f32>> {
        let q = embed(query);
        Ok(records
            .iter()
            .map(|r| cosine(&q, &embed(&r.text)))
            .collect())
    }
}

/// Sparse vector over `DIMS`, L2-normalized. Sparse because a memory is a
/// sentence, not a document: a dense array would be mostly zeroes.
type Vector = BTreeMap<u16, f32>;

fn embed(text: &str) -> Vector {
    let normalized: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();
    let mut counts: BTreeMap<u16, f32> = BTreeMap::new();
    for word in normalized.split_whitespace().filter(|w| carries_meaning(w)) {
        // Pad so a short word still yields the trigrams that carry it, and so
        // a word's start and end are distinguishable from its middle.
        let padded = format!(" {word} ");
        let chars: Vec<char> = padded.chars().collect();
        for window in chars.windows(3) {
            let slot = (fnv1a(window) % DIMS as u64) as u16;
            *counts.entry(slot).or_default() += 1.0;
        }
    }
    let norm = counts.values().map(|v| v * v).sum::<f32>().sqrt();
    if norm == 0.0 {
        return counts;
    }
    for value in counts.values_mut() {
        *value /= norm;
    }
    counts
}

fn cosine(a: &Vector, b: &Vector) -> f32 {
    // Both sides are already normalized, so the dot product is the cosine.
    a.iter()
        .filter_map(|(slot, x)| b.get(slot).map(|y| x * y))
        .sum::<f32>()
        .clamp(0.0, 1.0)
}

/// Function words are in every sentence, so their trigrams are noise that
/// drowns the few that actually match. Short words go too, except that three
/// letters is the floor rather than four: `git`, `npm` and `ssh` are facts.
pub(crate) fn carries_meaning(word: &str) -> bool {
    const STOPWORDS: [&str; 44] = [
        "the", "and", "for", "are", "but", "not", "you", "all", "any", "was", "our", "its", "has",
        "had", "her", "his", "him", "she", "who", "why", "how", "did", "does", "this", "that",
        "with", "from", "into", "when", "what", "then", "than", "there", "here", "your", "them",
        "they", "will", "would", "should", "could", "been", "were", "have",
    ];
    word.chars().count() >= 3 && !STOPWORDS.contains(&word)
}

/// FNV-1a, spelled out rather than taken from `DefaultHasher`: that one is
/// seeded per process, and a memory index that scores differently on every
/// restart is not an index.
fn fnv1a(chars: &[char]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for c in chars {
        for byte in (*c as u32).to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_text_embeds_the_same_way_every_time() {
        assert_eq!(
            embed("scripts/ci.sh runs the build"),
            embed("scripts/ci.sh runs the build")
        );
        assert!((cosine(&embed("the build"), &embed("the build")) - 1.0).abs() < 1e-5);
        assert_eq!(cosine(&embed("alpha"), &embed("")), 0.0);
    }

    #[test]
    fn a_shared_word_stem_scores_above_an_unrelated_sentence() {
        let related = cosine(
            &embed("how do I rebuild the index"),
            &embed("reindexing the shard"),
        );
        let unrelated = cosine(
            &embed("how do I rebuild the index"),
            &embed("the office coffee machine is on the third floor"),
        );
        assert!(
            related > unrelated,
            "related {related} should beat unrelated {unrelated}"
        );
    }
}
