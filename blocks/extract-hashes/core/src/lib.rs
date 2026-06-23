//! extract-hashes core — scan text for hexadecimal hash/digest strings and group
//! them by the algorithm implied by their length: MD5 (32 hex chars), SHA-1 (40),
//! SHA-256 (64), and SHA-512 (128). Pure-Rust (`regex` only); shared by the chat
//! skill block and the web page.

use regex::Regex;
use serde::Serialize;

/// One algorithm group of extracted hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    /// Algorithm label, e.g. "md5", "sha1", "sha256", "sha512".
    pub algorithm: String,
    /// Hex-digit length that identifies this algorithm (32/40/64/128).
    pub length: usize,
    /// Unique hashes for this algorithm, in first-seen order.
    pub hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    /// Total number of unique hashes across all groups.
    pub count: usize,
    /// Per-algorithm groups, ordered md5 → sha1 → sha256 → sha512; empty groups omitted.
    pub groups: Vec<Group>,
}

thread_local! {
    // A maximal run of hex digits, bounded by non-hex-word chars so that a 64-char
    // SHA-256 isn't mis-read as a 32-char MD5 (we only classify EXACT lengths).
    static HEXRE: Regex = Regex::new(r"(?i)\b[0-9a-f]{32,128}\b").unwrap();
}

/// The four supported hash lengths and their algorithm labels.
const KNOWN: &[(usize, &str)] = &[(32, "md5"), (40, "sha1"), (64, "sha256"), (128, "sha512")];

fn algo_for_len(len: usize) -> Option<&'static str> {
    KNOWN.iter().find(|(l, _)| *l == len).map(|(_, a)| *a)
}

/// Extract hex hash strings from `text`, grouped by detected algorithm.
///
/// `lowercase`: when true, normalize every hash to lowercase before output; either
/// way de-duplication is case-insensitive (`ABC…` and `abc…` count once).
pub fn extract(text: &str, lowercase: bool) -> Found {
    // Preserve algorithm ordering md5→sha1→sha256→sha512 with first-seen order within each.
    let mut buckets: Vec<(usize, &'static str, Vec<String>, std::collections::HashSet<String>)> =
        KNOWN.iter().map(|(l, a)| (*l, *a, Vec::new(), std::collections::HashSet::new())).collect();

    HEXRE.with(|re| {
        for m in re.find_iter(text) {
            let raw = m.as_str();
            if algo_for_len(raw.len()).is_none() {
                continue;
            }
            let value = if lowercase { raw.to_ascii_lowercase() } else { raw.to_string() };
            let key = value.to_ascii_lowercase();
            if let Some(b) = buckets.iter_mut().find(|b| b.0 == raw.len()) {
                if b.3.insert(key) {
                    b.2.push(value);
                }
            }
        }
    });

    let mut groups = Vec::new();
    let mut count = 0;
    for (len, algo, hashes, _) in buckets {
        if !hashes.is_empty() {
            count += hashes.len();
            groups.push(Group { algorithm: algo.to_string(), length: len, hashes });
        }
    }

    Found { count, groups }
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str, lowercase: bool) -> Result<String, String> {
    let r = extract(text, lowercase);
    if r.count == 0 {
        return Ok("No hashes found.".to_string());
    }
    let mut out = format!("{} unique hash(es):\n", r.count);
    for g in &r.groups {
        out.push_str(&format!(
            "\n{} ({} chars) — {} found:\n",
            g.algorithm.to_uppercase(),
            g.length,
            g.hashes.len()
        ));
        for h in &g.hashes {
            out.push_str(&format!("  {h}\n"));
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD5: &str = "d41d8cd98f00b204e9800998ecf8427e";
    const SHA1: &str = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
    const SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const SHA512: &str = "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e";

    #[test]
    fn groups_by_algorithm() {
        let t = format!("md5={MD5} sha1={SHA1} sha256={SHA256} sha512={SHA512}");
        let r = extract(&t, false);
        assert_eq!(r.count, 4);
        let algos: Vec<_> = r.groups.iter().map(|g| g.algorithm.as_str()).collect();
        assert_eq!(algos, vec!["md5", "sha1", "sha256", "sha512"]);
        assert_eq!(r.groups[0].hashes, vec![MD5]);
        assert_eq!(r.groups[3].length, 128);
    }

    #[test]
    fn dedupes_case_insensitively_when_lowercasing() {
        let t = format!("{MD5} {} repeat {MD5}", MD5.to_uppercase());
        let r = extract(&t, true);
        assert_eq!(r.count, 1);
        assert_eq!(r.groups[0].hashes, vec![MD5]);
    }

    #[test]
    fn keeps_case_when_not_lowercasing() {
        let up = MD5.to_uppercase();
        let t = format!("{up} {MD5}");
        let r = extract(&t, false);
        // Same hex, different case → de-duped (case-insensitive key), first-seen kept verbatim.
        assert_eq!(r.count, 1);
        assert_eq!(r.groups[0].hashes, vec![up]);
    }

    #[test]
    fn ignores_non_hash_hex_lengths() {
        // 8, 16, 48 hex chars are not a known digest length.
        let r = extract(
            "deadbeef cafebabedeadbeef 0123456789abcdef0123456789abcdef0123456789abcdef",
            false,
        );
        assert_eq!(r.count, 0);
        assert!(r.groups.is_empty());
    }

    #[test]
    fn does_not_truncate_longer_run() {
        // A 64-char SHA-256 must NOT be reported as a 32-char MD5 prefix.
        let r = extract(SHA256, false);
        assert_eq!(r.count, 1);
        assert_eq!(r.groups[0].algorithm, "sha256");
    }

    #[test]
    fn render_modes() {
        assert!(render(MD5, false).unwrap().contains("MD5 (32 chars)"));
        assert!(render("nothing here", false).unwrap().contains("No hashes"));
    }
}
