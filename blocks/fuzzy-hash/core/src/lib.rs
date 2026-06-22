//! gizza-ai/fuzzy-hash core — pure-Rust SSDEEP context-triggered piecewise
//! hashing (CTPH / "fuzzy hashing"). Produces a similarity-comparable
//! fingerprint of the form `blocksize:sig1:sig2`, in the shape of the reference
//! ssdeep implementation, plus a 0..100 similarity score between two such
//! fingerprints.
//!
//! No wafer/wasm-bindgen deps; pure compute, wasm32-safe.
//!
//! Algorithm (Kornblum 2006, the ssdeep reference):
//! - A 7-byte rolling hash over the input fires a "reset" trigger whenever
//!   `roll % blocksize == blocksize - 1`.
//! - Two FNV-style sum hashes run in parallel, one at `blocksize` and one at
//!   `2*blocksize`; on a trigger the low 6 bits of the current sum select a
//!   base64 char appended to the corresponding signature, and that sum resets.
//! - The blocksize starts at `MIN_BLOCKSIZE` (3) and doubles until the signature
//!   fits in `SPAMSUM_LENGTH` (64) chars; if it ends up under half-full and can
//!   shrink, the blocksize is halved and the hash recomputed.
//! - Similarity is an edit-distance ratio between signatures sharing a (same or
//!   adjacent) blocksize, scaled to 0..100 and capped so short inputs can't
//!   claim a misleadingly high score.

const SPAMSUM_LENGTH: usize = 64;
const MIN_BLOCKSIZE: u32 = 3;
const ROLLING_WINDOW: usize = 7;
const HASH_PRIME: u32 = 0x0100_0193; // FNV prime
const HASH_INIT: u32 = 0x2802_1967;
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Rolling hash state — a fixed 7-byte window, matching the ssdeep reference.
#[derive(Default)]
struct Roll {
    window: [u8; ROLLING_WINDOW],
    h1: u32,
    h2: u32,
    h3: u32,
    n: usize,
}

impl Roll {
    #[inline]
    fn update(&mut self, c: u8) -> u32 {
        self.h2 = self
            .h2
            .wrapping_sub(self.h1)
            .wrapping_add((ROLLING_WINDOW as u32).wrapping_mul(c as u32));
        self.h1 = self.h1.wrapping_add(c as u32);
        let idx = self.n % ROLLING_WINDOW;
        self.h1 = self.h1.wrapping_sub(self.window[idx] as u32);
        self.window[idx] = c;
        self.n += 1;
        self.h3 = self.h3 << 5;
        self.h3 ^= c as u32;
        self.h1.wrapping_add(self.h2).wrapping_add(self.h3)
    }
}

#[inline]
fn sum_hash(c: u8, h: u32) -> u32 {
    h.wrapping_mul(HASH_PRIME) ^ (c as u32)
}

/// Compute the two piecewise signatures for a given blocksize.
fn build(data: &[u8], blocksize: u32) -> (Vec<u8>, Vec<u8>) {
    let mut roll = Roll::default();
    let mut h = HASH_INIT;
    let mut h2 = HASH_INIT;
    let mut sig: Vec<u8> = Vec::with_capacity(SPAMSUM_LENGTH);
    let mut sig2: Vec<u8> = Vec::with_capacity(SPAMSUM_LENGTH / 2);

    for &c in data {
        h = sum_hash(c, h);
        h2 = sum_hash(c, h2);
        let r = roll.update(c);

        if r % blocksize == blocksize - 1 {
            if sig.len() < SPAMSUM_LENGTH - 1 {
                sig.push(B64[(h % 64) as usize]);
                h = HASH_INIT;
            }
            if r % (blocksize * 2) == (blocksize * 2) - 1 && sig2.len() < SPAMSUM_LENGTH / 2 - 1 {
                sig2.push(B64[(h2 % 64) as usize]);
                h2 = HASH_INIT;
            }
        }
    }

    // Flush the final (partial) block if anything was consumed.
    if h != HASH_INIT {
        sig.push(B64[(h % 64) as usize]);
    }
    if h2 != HASH_INIT {
        sig2.push(B64[(h2 % 64) as usize]);
    }
    (sig, sig2)
}

/// Compute the ssdeep fuzzy hash of `data`, returning `blocksize:sig1:sig2`.
pub fn fuzzy_hash(data: &[u8]) -> String {
    let mut blocksize = MIN_BLOCKSIZE;
    while (blocksize as u64) * (SPAMSUM_LENGTH as u64) < data.len() as u64 {
        blocksize = blocksize.saturating_mul(2);
    }

    loop {
        let (sig, sig2) = build(data, blocksize);
        if blocksize > MIN_BLOCKSIZE && sig.len() < SPAMSUM_LENGTH / 2 {
            blocksize /= 2;
            continue;
        }
        let s1 = String::from_utf8(sig).unwrap();
        let s2 = String::from_utf8(sig2).unwrap();
        return format!("{blocksize}:{s1}:{s2}");
    }
}

/// Parse a `blocksize:sig1:sig2` fuzzy hash into its parts.
fn parse(s: &str) -> Option<(u32, &str, &str)> {
    let mut it = s.splitn(3, ':');
    let bs: u32 = it.next()?.parse().ok()?;
    let a = it.next()?;
    let b = it.next()?;
    if bs == 0 {
        return None;
    }
    Some((bs, a, b))
}

/// Collapse runs of 4+ identical chars down to 3 — ssdeep does this before
/// scoring so a long repeated sequence can't dominate the edit distance.
fn eliminate_sequences(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(b.len());
    for (i, &c) in b.iter().enumerate() {
        if i >= 3 && b[i - 1] == c && b[i - 2] == c && b[i - 3] == c {
            continue;
        }
        out.push(c as char);
    }
    out
}

/// Edit distance with insert/delete cost 1, substitute cost 2 (ssdeep variant).
fn edit_distance(a: &[u8], b: &[u8]) -> usize {
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let sub = if a[i - 1] == b[j - 1] { 0 } else { 2 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + sub);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// Score the similarity of two equal-blocksize signature strings, 0..100.
fn score_strings(s1: &str, s2: &str, blocksize: u32) -> u32 {
    let e1 = eliminate_sequences(s1);
    let e2 = eliminate_sequences(s2);
    let (a, b) = (e1.as_bytes(), e2.as_bytes());
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let dist = edit_distance(a, b);
    let len = (a.len() + b.len()) as u64;
    let mut score = (dist as u64 * SPAMSUM_LENGTH as u64) / len;
    score = (score * 100) / 64;
    let mut score = 100u64.saturating_sub(score);
    // Cap the score for small blocksizes so tiny inputs can't claim 100.
    let cap = (blocksize as u64 / MIN_BLOCKSIZE as u64) * a.len().min(b.len()) as u64;
    if score > cap {
        score = cap;
    }
    score.min(100) as u32
}

/// Compare two ssdeep fuzzy hashes, returning a similarity score 0..100
/// (0 = no match, 100 = near-identical). Returns 0 if either is unparseable or
/// the blocksizes are incompatible.
pub fn compare(h1: &str, h2: &str) -> u32 {
    if h1 == h2 && parse(h1).is_some() {
        return 100;
    }
    let (Some((b1, a1, c1)), Some((b2, a2, c2))) = (parse(h1), parse(h2)) else {
        return 0;
    };
    if b1 == b2 {
        score_strings(a1, a2, b1).max(score_strings(c1, c2, b1))
    } else if b1 == b2.wrapping_mul(2) {
        score_strings(a1, c2, b1)
    } else if b2 == b1.wrapping_mul(2) {
        score_strings(c1, a2, b2)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_short_have_blocksize_and_parse() {
        let h = fuzzy_hash(b"");
        assert!(parse(&h).is_some(), "empty hash must parse: {h}");
        assert!(h.starts_with("3:"), "empty input uses min blocksize: {h}");
    }

    #[test]
    fn deterministic() {
        let data = b"The quick brown fox jumps over the lazy dog. ".repeat(50);
        assert_eq!(fuzzy_hash(&data), fuzzy_hash(&data));
    }

    #[test]
    fn identical_inputs_score_100() {
        let data = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(40);
        let h = fuzzy_hash(&data);
        assert_eq!(compare(&h, &h), 100);
    }

    /// A varied (non-degenerate) text corpus so signatures aren't dominated by
    /// runs of one char — closer to real file contents.
    fn corpus(n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        let mut x: u32 = 0x1234_5678;
        let words: [&str; 8] = [
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
        ];
        while v.len() < n {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let w = words[(x >> 16) as usize % words.len()];
            v.extend_from_slice(w.as_bytes());
            v.push(b' ');
        }
        v.truncate(n);
        v
    }

    #[test]
    fn similar_inputs_score_high() {
        let base = corpus(20_000);
        let mut modified = base.clone();
        // Tweak a small contiguous region near the start; the bulk is shared.
        for i in 0..16 {
            modified[100 + i] = b'Z';
        }
        let h1 = fuzzy_hash(&base);
        let h2 = fuzzy_hash(&modified);
        let s = compare(&h1, &h2);
        assert!(s > 50, "near-identical inputs should score >50, got {s} ({h1} vs {h2})");
        assert!(s < 100, "modified input should not be a perfect match, got {s}");
    }

    #[test]
    fn unrelated_inputs_score_low() {
        let a = corpus(20_000);
        let mut b = Vec::with_capacity(20_000);
        let mut x: u32 = 0xDEAD_BEEF;
        while b.len() < 20_000 {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            b.push((x >> 16) as u8);
        }
        let s = compare(&fuzzy_hash(&a), &fuzzy_hash(&b));
        assert!(s < 50, "unrelated inputs should score low, got {s}");
    }

    #[test]
    fn compare_rejects_garbage() {
        assert_eq!(compare("not-a-hash", "3:abc:def"), 0);
        assert_eq!(compare("3:abc:def", ""), 0);
        assert_eq!(compare("", ""), 0);
    }

    #[test]
    fn blocksize_grows_with_size() {
        let small = fuzzy_hash(&b"x".repeat(100));
        let large = fuzzy_hash(&b"abcdefgh".repeat(200_000));
        let bs_small: u32 = small.split(':').next().unwrap().parse().unwrap();
        let bs_large: u32 = large.split(':').next().unwrap().parse().unwrap();
        assert!(
            bs_large > bs_small,
            "larger input -> larger blocksize: {bs_small} vs {bs_large}"
        );
    }
}
