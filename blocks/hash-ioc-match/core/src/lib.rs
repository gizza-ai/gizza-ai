//! hash-ioc-match core — hash an input (a file's bytes) and flag it if any of
//! its digests appears in a pasted blocklist of known-bad hashes.
//!
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps. The input is hashed with the four hash families used
//! by threat-intel IOC feeds — MD5, SHA-1, SHA-256 and SHA-512 — and each
//! resulting lowercase-hex digest is looked up in a set built from the
//! blocklist. Blocklist lines are normalized leniently: any hex run whose length
//! matches a supported digest width (32/40/64/128 hex chars) is extracted, so
//! labelled (`MD5: …`), CSV (`hash,filename`), `0x`-prefixed and `#`-commented
//! lines all work, case-insensitively.
//!
//! All hashers are pure-Rust (RustCrypto) so the tool runs on every backend,
//! including the chat Service Worker.

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

/// A hash algorithm this tool computes for the input. Ordered as reported.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    /// Canonical lowercase identifier.
    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Md5 => "md5",
            Algorithm::Sha1 => "sha1",
            Algorithm::Sha256 => "sha256",
            Algorithm::Sha512 => "sha512",
        }
    }

    /// Digest width in hex characters (2 × byte width).
    fn hex_len(self) -> usize {
        match self {
            Algorithm::Md5 => 32,
            Algorithm::Sha1 => 40,
            Algorithm::Sha256 => 64,
            Algorithm::Sha512 => 128,
        }
    }
}

/// The algorithms computed for every input, in report order.
pub const ALGORITHMS: &[Algorithm] =
    &[Algorithm::Md5, Algorithm::Sha1, Algorithm::Sha256, Algorithm::Sha512];

/// How to interpret the input string before hashing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputEncoding {
    /// Hash the UTF-8 bytes of the text as-is (default).
    Text,
    /// Decode the text from hexadecimal first, then hash the raw bytes.
    Hex,
    /// Decode the text from base64 (standard alphabet) first, then hash the bytes.
    Base64,
}

fn parse_input_encoding(s: &str) -> Result<InputEncoding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" | "utf8" | "utf-8" => Ok(InputEncoding::Text),
        "hex" => Ok(InputEncoding::Hex),
        "base64" | "b64" => Ok(InputEncoding::Base64),
        other => Err(format!(
            "invalid input_encoding '{other}': expected 'text', 'hex', or 'base64'"
        )),
    }
}

/// Decode `text` into raw bytes per `encoding`.
fn decode_input(text: &str, encoding: InputEncoding) -> Result<Vec<u8>, String> {
    match encoding {
        InputEncoding::Text => Ok(text.as_bytes().to_vec()),
        InputEncoding::Hex => {
            let cleaned: String = text.split_whitespace().collect();
            let cleaned = cleaned
                .strip_prefix("0x")
                .or_else(|| cleaned.strip_prefix("0X"))
                .unwrap_or(&cleaned);
            hex::decode(cleaned).map_err(|e| format!("input is not valid hex: {e}"))
        }
        InputEncoding::Base64 => {
            let cleaned: String = text.split_whitespace().collect();
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(cleaned.as_bytes())
                .map_err(|e| format!("input is not valid base64: {e}"))
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Compute the lowercase-hex digest of `data` with `alg`.
pub fn digest_hex(data: &[u8], alg: Algorithm) -> String {
    match alg {
        Algorithm::Md5 => hex_encode(&Md5::digest(data)),
        Algorithm::Sha1 => hex_encode(&Sha1::digest(data)),
        Algorithm::Sha256 => hex_encode(&Sha256::digest(data)),
        Algorithm::Sha512 => hex_encode(&Sha512::digest(data)),
    }
}

/// The set of digest-width hex-character lengths we recognise in the blocklist.
const HEX_WIDTHS: [usize; 4] = [32, 40, 64, 128];

/// Normalize a pasted blocklist into a set of lowercase hex hashes.
///
/// Scans every maximal run of hex characters in the text and keeps those whose
/// length matches a supported digest width (32/40/64/128), lowercased. This
/// tolerates labels (`MD5: <hash>`), CSV columns (`<hash>,malware.exe`), `0x`
/// prefixes, `#`/`;` comments and arbitrary separators — a `0x`-prefixed hash
/// still yields the trailing digest-width run. Duplicates collapse.
pub fn normalize_blocklist(text: &str) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    let mut run = String::new();
    // Append a sentinel non-hex char so the final run is flushed by the loop.
    for ch in text.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_hexdigit() {
            run.push(ch.to_ascii_lowercase());
        } else {
            if HEX_WIDTHS.contains(&run.len()) {
                set.insert(std::mem::take(&mut run));
            } else {
                run.clear();
            }
        }
    }
    set
}

/// A single algorithm→digest that was found in the blocklist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashMatch {
    pub algorithm: &'static str,
    pub hash: String,
}

/// One computed digest of the input (always reported, matched or not).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComputedHash {
    pub algorithm: &'static str,
    pub hash: String,
    pub matched: bool,
}

/// Outcome of hashing an input and matching it against a blocklist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchOutcome {
    /// True iff at least one computed digest was found in the blocklist.
    pub flagged: bool,
    /// Every computed digest, in MD5/SHA-1/SHA-256/SHA-512 order.
    pub computed: Vec<ComputedHash>,
    /// The subset of `computed` that hit the blocklist.
    pub matches: Vec<HashMatch>,
    /// Size of the hashed input in bytes (after any hex/base64 decode).
    pub input_bytes: usize,
    /// Number of distinct known-bad hashes parsed from the blocklist.
    pub blocklist_entries: usize,
    /// Human-readable summary line.
    pub summary: String,
}

/// Hash `input` (interpreted per `input_encoding`) and flag it against
/// `blocklist`. Returns a [`MatchOutcome`]; errors only on an undecodable input
/// or an invalid encoding — an empty blocklist is a clean (non-flagged) result,
/// not an error.
pub fn analyze(
    input: &str,
    blocklist: &str,
    input_encoding: &str,
) -> Result<MatchOutcome, String> {
    let enc = parse_input_encoding(input_encoding)?;
    let data = decode_input(input, enc)?;
    let bad = normalize_blocklist(blocklist);

    let mut computed = Vec::with_capacity(ALGORITHMS.len());
    let mut matches = Vec::new();
    for &alg in ALGORITHMS {
        debug_assert_eq!(alg.hex_len(), digest_hex(b"", alg).len());
        let hash = digest_hex(&data, alg);
        let hit = bad.contains(&hash);
        if hit {
            matches.push(HashMatch { algorithm: alg.name(), hash: hash.clone() });
        }
        computed.push(ComputedHash { algorithm: alg.name(), hash, matched: hit });
    }

    let flagged = !matches.is_empty();
    let summary = if flagged {
        let algs: Vec<&str> = matches.iter().map(|m| m.algorithm).collect();
        format!(
            "FLAGGED — the file's {} hash matched the blocklist of known-bad hashes.",
            algs.join(" and ")
        )
    } else if bad.is_empty() {
        "CLEAN — no known-bad hashes were supplied, so nothing to match against.".to_string()
    } else {
        "CLEAN — none of the file's hashes appear in the blocklist.".to_string()
    };

    Ok(MatchOutcome {
        flagged,
        computed,
        matches,
        input_bytes: data.len(),
        blocklist_entries: bad.len(),
        summary,
    })
}

/// Render a [`MatchOutcome`] as a multi-line text report (used by the page +
/// CLI/chat string surface).
pub fn report(input: &str, blocklist: &str, input_encoding: &str) -> Result<String, String> {
    let o = analyze(input, blocklist, input_encoding)?;
    let mut s = String::new();
    s.push_str(&o.summary);
    s.push('\n');
    s.push_str(&format!("status: {}\n", if o.flagged { "FLAGGED" } else { "CLEAN" }));
    if o.flagged {
        s.push_str("matched:\n");
        for m in &o.matches {
            s.push_str(&format!("  {:<7} {}\n", m.algorithm, m.hash));
        }
    } else {
        s.push_str("matched: none\n");
    }
    s.push_str(&format!("input bytes: {}\n", o.input_bytes));
    s.push_str(&format!("blocklist entries: {}\n", o.blocklist_entries));
    s.push_str("computed:\n");
    for c in &o.computed {
        s.push_str(&format!(
            "  {:<7} {}{}\n",
            c.algorithm,
            c.hash,
            if c.matched { "  <-- MATCH" } else { "" }
        ));
    }
    Ok(s.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known digests of the empty input.
    const EMPTY_MD5: &str = "d41d8cd98f00b204e9800998ecf8427e";
    // Known digests of "abc".
    const ABC_MD5: &str = "900150983cd24fb0d6963f7d28e17f72";
    const ABC_SHA1: &str = "a9993e364706816aba3e25717850c26c9cd0d89d";
    const ABC_SHA256: &str =
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn flags_when_md5_in_blocklist() {
        let o = analyze("abc", ABC_MD5, "text").unwrap();
        assert!(o.flagged);
        assert_eq!(o.matches.len(), 1);
        assert_eq!(o.matches[0].algorithm, "md5");
        assert_eq!(o.matches[0].hash, ABC_MD5);
        assert_eq!(o.blocklist_entries, 1);
        assert_eq!(o.input_bytes, 3);
    }

    #[test]
    fn clean_when_no_match() {
        let o = analyze("abc", EMPTY_MD5, "text").unwrap();
        assert!(!o.flagged);
        assert!(o.matches.is_empty());
        // Every algorithm still reported.
        assert_eq!(o.computed.len(), 4);
        assert!(o.computed.iter().all(|c| !c.matched));
    }

    #[test]
    fn matches_are_case_insensitive() {
        let o = analyze("abc", &ABC_SHA1.to_uppercase(), "text").unwrap();
        assert!(o.flagged);
        assert_eq!(o.matches[0].algorithm, "sha1");
    }

    #[test]
    fn tolerates_labels_csv_and_comments() {
        let list = "# known bad\nMD5:  900150983CD24FB0D6963F7D28E17F72 , dropper.exe\n\
                    sha256=0xBA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD ; note";
        let o = analyze("abc", list, "text").unwrap();
        assert!(o.flagged);
        // Both the MD5 and the SHA-256 lines should have been parsed and matched.
        assert_eq!(o.blocklist_entries, 2);
        let algs: Vec<&str> = o.matches.iter().map(|m| m.algorithm).collect();
        assert!(algs.contains(&"md5"));
        assert!(algs.contains(&"sha256"));
    }

    #[test]
    fn hex_input_encoding_matches_text() {
        // "abc" == hex 616263 — decode then hash equals hashing the bytes.
        let o = analyze("616263", ABC_SHA256, "hex").unwrap();
        assert!(o.flagged);
        assert_eq!(o.matches[0].algorithm, "sha256");
    }

    #[test]
    fn ignores_non_digest_width_hex_runs() {
        // "dead" is 4 hex chars — not a digest width — so nothing is parsed and
        // the result is clean, not flagged.
        let o = analyze("abc", "dead beef cafe", "text").unwrap();
        assert_eq!(o.blocklist_entries, 0);
        assert!(!o.flagged);
    }

    #[test]
    fn empty_blocklist_is_clean_not_error() {
        let o = analyze("abc", "", "text").unwrap();
        assert!(!o.flagged);
        assert_eq!(o.blocklist_entries, 0);
    }

    #[test]
    fn bad_encoding_errors() {
        let err = analyze("abc", "", "rot13").unwrap_err();
        assert!(err.contains("invalid input_encoding"));
    }

    #[test]
    fn bad_hex_input_errors() {
        let err = analyze("zzz", "", "hex").unwrap_err();
        assert!(err.contains("not valid hex"));
    }

    #[test]
    fn report_contains_status_and_match_marker() {
        let r = report("abc", ABC_MD5, "text").unwrap();
        assert!(r.contains("status: FLAGGED"));
        assert!(r.contains("<-- MATCH"));
        assert!(r.contains(ABC_MD5));
    }

    #[test]
    fn report_clean_has_no_match_marker() {
        let r = report("abc", EMPTY_MD5, "text").unwrap();
        assert!(r.contains("status: CLEAN"));
        assert!(r.contains("matched: none"));
        assert!(!r.contains("<-- MATCH"));
    }

    #[test]
    fn duplicate_blocklist_entries_collapse() {
        let list = format!("{ABC_MD5}\n{ABC_MD5}\n{}", ABC_MD5.to_uppercase());
        let o = analyze("abc", &list, "text").unwrap();
        assert_eq!(o.blocklist_entries, 1);
    }
}
