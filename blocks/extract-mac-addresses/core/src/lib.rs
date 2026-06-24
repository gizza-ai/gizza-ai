//! gizza-ai/extract-mac-addresses core — find MAC (EUI-48 / EUI-64) addresses in
//! text written in any common notation and normalize them to a single chosen
//! format. Candidates are matched with `regex` over the common notations
//! (colon, hyphen, Cisco dotted-quad, and bare hex), de-duplicated by their
//! canonical bytes, and emitted in first-seen order. Pure-Rust, no I/O.

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    /// Number of unique MAC addresses found.
    pub count: usize,
    /// Unique MAC addresses, normalized to the requested format, first-seen order.
    pub macs: Vec<String>,
}

/// The output notation each found address is normalized to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    /// `00:1a:2b:3c:4d:5e` (IEEE 802 canonical, colon-separated).
    Colon,
    /// `00-1a-2b-3c-4d-5e` (Windows / IEEE hyphen form).
    Hyphen,
    /// `001a.2b3c.4d5e` (Cisco dotted-quad; EUI-64 → four groups).
    Cisco,
    /// `001a2b3c4d5e` (no separators).
    Bare,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "colon" => Ok(Format::Colon),
            "hyphen" => Ok(Format::Hyphen),
            "cisco" | "dot" => Ok(Format::Cisco),
            "bare" | "none" => Ok(Format::Bare),
            other => Err(format!(
                "invalid format {other:?}: expected \"colon\", \"hyphen\", \"cisco\", or \"bare\""
            )),
        }
    }
}

thread_local! {
    // Six (EUI-48) or eight (EUI-64) hex pairs separated by ':' or '-'. A
    // word boundary on each side avoids matching a longer hex run mid-string.
    static SEP: Regex =
        Regex::new(r"\b[0-9A-Fa-f]{2}(?:[:-][0-9A-Fa-f]{2}){5,7}\b").unwrap();
    // Cisco dotted-quad: three (EUI-48) or four (EUI-64) groups of four hex.
    static CISCO: Regex =
        Regex::new(r"\b[0-9A-Fa-f]{4}(?:\.[0-9A-Fa-f]{4}){2,3}\b").unwrap();
    // Bare hex run of 12 (EUI-48) to 16 (EUI-64) hex digits. The surrounding
    // non-hex guards (checked in code) keep it from matching inside a longer
    // hash / hex blob.
    static BARE: Regex = Regex::new(r"[0-9A-Fa-f]{12,16}").unwrap();
}

/// Collect the lowercase hex digits from a matched candidate, if its length is a
/// valid MAC width (12 = EUI-48, 16 = EUI-64). Separators are stripped.
fn hex_digits(tok: &str) -> Option<String> {
    let hex: String = tok
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if hex.len() == 12 || hex.len() == 16 {
        Some(hex)
    } else {
        None
    }
}

/// Format the 12- or 16-char lowercase hex string into the requested notation.
fn render_one(hex: &str, fmt: Format) -> String {
    match fmt {
        Format::Bare => hex.to_string(),
        Format::Colon | Format::Hyphen => {
            let sep = if fmt == Format::Colon { ':' } else { '-' };
            let bytes: Vec<&str> = (0..hex.len()).step_by(2).map(|i| &hex[i..i + 2]).collect();
            bytes.join(&sep.to_string())
        }
        Format::Cisco => {
            // Group the hex into chunks of 4 (Cisco dotted-quad).
            let quads: Vec<&str> = (0..hex.len()).step_by(4).map(|i| &hex[i..i + 4]).collect();
            quads.join(".")
        }
    }
}

/// Find, normalize and de-duplicate every MAC address in `text`.
///
/// - `format` (`"colon"` default | `"hyphen"` | `"cisco"` | `"bare"`): the output
///   notation. De-duplication is by the underlying bytes, so the same address
///   written two ways counts once.
///
/// Returns `Err` only on an invalid `format`.
pub fn extract(text: &str, format: &str) -> Result<Found, String> {
    let fmt = Format::parse(format)?;
    let mut macs: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let mut push = |hex: &str| {
        if seen.insert(hex.to_string()) {
            macs.push(render_one(hex, fmt));
        }
    };

    // Separator forms (colon/hyphen) — unambiguous, scan first.
    SEP.with(|re| {
        for m in re.find_iter(text) {
            if let Some(hex) = hex_digits(m.as_str()) {
                push(&hex);
            }
        }
    });
    // Cisco dotted-quad.
    CISCO.with(|re| {
        for m in re.find_iter(text) {
            if let Some(hex) = hex_digits(m.as_str()) {
                push(&hex);
            }
        }
    });
    // Bare hex runs — only when bounded by a non-hex char (so a 32-char MD5 or a
    // longer hex blob does NOT yield a 12/16-char false positive).
    BARE.with(|re| {
        for m in re.find_iter(text) {
            let (s, e) = (m.start(), m.end());
            let before_ok = s == 0
                || !text[..s]
                    .chars()
                    .next_back()
                    .map(|c| c.is_ascii_hexdigit())
                    .unwrap_or(false);
            let after_ok = e == text.len()
                || !text[e..]
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_hexdigit())
                    .unwrap_or(false);
            if before_ok && after_ok {
                if let Some(hex) = hex_digits(m.as_str()) {
                    push(&hex);
                }
            }
        }
    });

    Ok(Found { count: macs.len(), macs })
}

/// Human-readable rendering (used by the page).
pub fn render(text: &str, format: &str) -> Result<String, String> {
    let r = extract(text, format)?;
    if r.count == 0 {
        return Ok("No MAC addresses found.".to_string());
    }
    let mut out = format!("{} unique MAC address(es):\n", r.count);
    for m in &r.macs {
        out.push_str(&format!("  {m}\n"));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_colon_and_dedups() {
        let t = "host A 00:1A:2B:3C:4D:5E talks to 00-1a-2b-3c-4d-5e (same nic)";
        let r = extract(t, "colon").unwrap();
        // Both notations are the same address → one result.
        assert_eq!(r.macs, vec!["00:1a:2b:3c:4d:5e"]);
        assert_eq!(r.count, 1);
    }

    #[test]
    fn finds_cisco_dotted_quad() {
        let r = extract("router mac 001a.2b3c.4d5e up", "colon").unwrap();
        assert_eq!(r.macs, vec!["00:1a:2b:3c:4d:5e"]);
    }

    #[test]
    fn finds_bare_hex() {
        let r = extract("MAC=001A2B3C4D5E in dhcp lease", "colon").unwrap();
        assert_eq!(r.macs, vec!["00:1a:2b:3c:4d:5e"]);
    }

    #[test]
    fn normalizes_to_each_format() {
        let src = "aa:bb:cc:dd:ee:ff";
        assert_eq!(extract(src, "colon").unwrap().macs, vec!["aa:bb:cc:dd:ee:ff"]);
        assert_eq!(extract(src, "hyphen").unwrap().macs, vec!["aa-bb-cc-dd-ee-ff"]);
        assert_eq!(extract(src, "cisco").unwrap().macs, vec!["aabb.ccdd.eeff"]);
        assert_eq!(extract(src, "bare").unwrap().macs, vec!["aabbccddeeff"]);
    }

    #[test]
    fn finds_eui64() {
        // 8-byte EUI-64, colon form → Cisco four-group output.
        let r = extract("link-local 00:1a:2b:ff:fe:3c:4d:5e", "cisco").unwrap();
        assert_eq!(r.macs, vec!["001a.2bff.fe3c.4d5e"]);
    }

    #[test]
    fn ignores_md5_and_long_hex() {
        // 32-char MD5 must not yield a 12/16-char bare-hex MAC.
        let r = extract("hash d41d8cd98f00b204e9800998ecf8427e here", "colon").unwrap();
        assert_eq!(r.count, 0, "a 32-char hash is not a MAC");
    }

    #[test]
    fn ignores_partial_runs() {
        // Five pairs is too short (not 6=EUI-48 or 8=EUI-64); seven pairs is an
        // invalid width too. Neither yields a match.
        let r = extract(
            "only 00:11:22:33:44 here and 00:11:22:33:44:55:66",
            "colon",
        )
        .unwrap();
        assert_eq!(r.count, 0);
    }

    #[test]
    fn rejects_unknown_format() {
        let err = extract("aa:bb:cc:dd:ee:ff", "binary").unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }

    #[test]
    fn render_reports_and_empty() {
        assert!(render("aa:bb:cc:dd:ee:ff", "colon").unwrap().contains("1 unique MAC"));
        assert!(render("nothing here", "colon").unwrap().contains("No MAC"));
    }

    #[test]
    fn dedup_across_formats_first_seen() {
        // Two distinct addresses, second written bare; order preserved.
        let r = extract("11:22:33:44:55:66 then AABBCCDDEEFF", "bare").unwrap();
        assert_eq!(r.macs, vec!["112233445566", "aabbccddeeff"]);
    }
}
