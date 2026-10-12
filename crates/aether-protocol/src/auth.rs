//! The one rule for a token on a transport where a static string is the whole
//! authorization: how long it must be, and how it is compared.

/// Shortest token accepted. One string stands between the network and
/// `create_agent`, so a short one is refused rather than warned about. The
/// floor is length only: 32 characters of `aaaa...` passes it, and checking
/// for entropy in a string the operator chose would be guesswork. A token
/// Aether mints for a single turn is not held to this: it dies with the turn.
pub const MIN_TOKEN_CHARS: usize = 32;

/// Equal length and equal bytes, without returning early on the first
/// difference, so a caller cannot learn a prefix by timing. The comparison
/// is `subtle`'s rather than a hand-rolled fold: nothing in the language
/// stops a compiler short-circuiting the latter, and `subtle` carries the
/// optimization barrier that stops it.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq as _;
    let (a, b) = (a.as_bytes(), b.as_bytes());
    // Length is not a secret - it leaks through the frame anyway - and
    // `ct_eq` needs equal-length slices.
    a.len() == b.len() && bool::from(a.ct_eq(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_matches_str_equality() {
        for (a, b) in [
            ("", ""),
            ("token", "token"),
            ("token", "tokeN"),
            ("a", "ab"),
        ] {
            assert_eq!(constant_time_eq(a, b), a == b, "{a} vs {b}");
        }
    }
}
