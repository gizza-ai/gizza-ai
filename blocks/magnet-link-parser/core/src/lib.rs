//! gizza-ai/magnet-link-parser core — decode a BitTorrent `magnet:` URI into
//! its structured parts (info-hash, display name, trackers, web seeds, …) and
//! rebuild a magnet link from those parts. Pure-Rust, no wafer/wasm-bindgen
//! deps; shared by the chat block and the standalone page.
//!
//! Magnet links are `magnet:?` followed by `&`-separated query parameters
//! (BEP 9 / BEP 53 / the de-facto Magnet-URI scheme). The keys this tool
//! understands:
//!
//! | key   | meaning                                                  |
//! |-------|----------------------------------------------------------|
//! | `xt`  | exact topic — `urn:btih:<hash>` (v1) or `urn:btmh:<mh>` (v2) |
//! | `dn`  | display name (percent-/`+`-encoded)                      |
//! | `tr`  | tracker announce URL (repeatable)                        |
//! | `ws`  | web seed URL (BEP 19, repeatable)                        |
//! | `as`  | acceptable source — a fallback web link (repeatable)     |
//! | `xs`  | exact source — e.g. a `.torrent` link (repeatable)       |
//! | `kt`  | keyword topic — search keywords                          |
//! | `xl`  | exact length, in bytes                                   |
//! | `mt`  | manifest topic — a list of links                         |

use serde::Serialize;

/// One `xt` (exact-topic) entry, classified by its URN namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExactTopic {
    /// The raw `xt` value as it appeared (e.g. `urn:btih:<hash>`).
    pub urn: String,
    /// The URN namespace if recognised: `btih` (BitTorrent v1 info-hash),
    /// `btmh` (BitTorrent v2 multihash), `ed2k`, `sha1`, `tree:tiger`, …
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The hash/value after the namespace prefix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// The structured result of parsing a magnet URI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedMagnet {
    /// The primary BitTorrent v1 info-hash (`urn:btih:`), normalised to lower-case
    /// hex when it was given as 40 hex chars or 32 base32 chars; otherwise the raw value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
    /// How the source `btih` info-hash was encoded: `"hex"` (40 chars) or
    /// `"base32"` (32 chars), if a v1 info-hash was present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info_hash_encoding: Option<String>,
    /// The BitTorrent v2 info-hash multihash (`urn:btmh:`), if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info_hash_v2: Option<String>,
    /// The display name (`dn`), percent-/`+`-decoded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Tracker announce URLs (`tr`), decoded, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub trackers: Vec<String>,
    /// Web-seed URLs (`ws`), decoded, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub web_seeds: Vec<String>,
    /// Acceptable-source URLs (`as`), decoded, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub acceptable_sources: Vec<String>,
    /// Exact-source URLs (`xs`), decoded, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exact_sources: Vec<String>,
    /// Search keywords from `kt`, split on whitespace.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// The exact length in bytes (`xl`), if present and numeric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_length: Option<u64>,
    /// Every `xt` entry, classified (the first `btih` populates `info_hash`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exact_topics: Vec<ExactTopic>,
    /// Any other (key, decoded-value) parameters not covered above, in order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub other_params: Vec<KeyValue>,
}

/// A decoded `key=value` pair for parameters outside the well-known set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

/// Lenient percent-decode with `+`→space (form-style, as magnet `dn`/`kt` use).
/// Invalid `%` escapes are left literal; bytes are decoded as UTF-8 lossily.
fn decode(s: &str) -> String {
    let replaced = s.replace('+', " ");
    let bytes = replaced.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-encode a value for a magnet query parameter: keep the RFC 3986
/// unreserved set (`A-Z a-z 0-9 - _ . ~`), escape everything else as `%XX`.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// True if `s` is exactly 40 hexadecimal characters (a v1 info-hash in hex).
fn is_hex40(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// True if `s` is exactly 32 RFC 4648 base32 characters (a v1 info-hash in
/// base32). Padding is not expected for a 20-byte hash (32 chars, no `=`).
fn is_base32_32(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'2'..=b'7'))
}

/// Decode 32 base32 chars (RFC 4648, no padding) into 20 bytes, then hex-encode.
fn base32_to_hex(s: &str) -> Option<String> {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bits = 0u32;
    let mut nbits = 0u32;
    let mut out: Vec<u8> = Vec::with_capacity(20);
    for c in s.bytes() {
        let up = c.to_ascii_uppercase();
        let v = ALPHA.iter().position(|&a| a == up)? as u32;
        bits = (bits << 5) | v;
        nbits += 5;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    if out.len() != 20 {
        return None;
    }
    let mut hex = String::with_capacity(40);
    for b in out {
        hex.push_str(&format!("{b:02x}"));
    }
    Some(hex)
}

/// Normalise a raw `btih` value to lower-case hex; returns `(hex, encoding)`.
/// Accepts 40-char hex or 32-char base32; anything else is returned unchanged
/// with an encoding of `"raw"`.
fn normalise_btih(raw: &str) -> (String, String) {
    if is_hex40(raw) {
        (raw.to_ascii_lowercase(), "hex".to_string())
    } else if is_base32_32(raw) {
        match base32_to_hex(raw) {
            Some(hex) => (hex, "base32".to_string()),
            None => (raw.to_string(), "raw".to_string()),
        }
    } else {
        (raw.to_string(), "raw".to_string())
    }
}

/// Parse a magnet URI into its components. Accepts input with or without the
/// `magnet:?` prefix (a bare query string also works). Returns `Err` for empty
/// input or a non-magnet scheme.
pub fn parse(input: &str) -> Result<ParsedMagnet, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("empty input: provide a magnet: link to parse".into());
    }

    // Strip the scheme. `magnet:?a=b` and `magnet:a=b` both appear in the wild;
    // also tolerate a bare `a=b&c=d` query string.
    let query = if let Some(rest) = s.strip_prefix("magnet:") {
        rest.strip_prefix('?').unwrap_or(rest)
    } else if s.contains("://") {
        return Err("not a magnet link: expected a 'magnet:?…' URI".into());
    } else {
        s.strip_prefix('?').unwrap_or(s)
    };

    let mut m = ParsedMagnet {
        info_hash: None,
        info_hash_encoding: None,
        info_hash_v2: None,
        display_name: None,
        trackers: Vec::new(),
        web_seeds: Vec::new(),
        acceptable_sources: Vec::new(),
        exact_sources: Vec::new(),
        keywords: Vec::new(),
        exact_length: None,
        exact_topics: Vec::new(),
        other_params: Vec::new(),
    };

    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        // Keys may be indexed (e.g. `tr.1`, `xt.2`) per the multi-topic spec —
        // collapse the `.N` suffix to the base key.
        let base = key.split_once('.').map(|(b, _)| b).unwrap_or(key);
        let decoded = decode(value);

        match base {
            "xt" => {
                let topic = classify_xt(&decoded);
                if let (Some(k), Some(v)) = (&topic.kind, &topic.value) {
                    if k == "btih" && m.info_hash.is_none() {
                        let (hex, enc) = normalise_btih(v);
                        m.info_hash = Some(hex);
                        m.info_hash_encoding = Some(enc);
                    } else if k == "btmh" && m.info_hash_v2.is_none() {
                        m.info_hash_v2 = Some(v.clone());
                    }
                }
                m.exact_topics.push(topic);
            }
            "dn" if m.display_name.is_none() => m.display_name = Some(decoded),
            "tr" => m.trackers.push(decoded),
            "ws" => m.web_seeds.push(decoded),
            "as" => m.acceptable_sources.push(decoded),
            "xs" => m.exact_sources.push(decoded),
            "kt" => m
                .keywords
                .extend(decoded.split_whitespace().map(str::to_string)),
            "xl" if m.exact_length.is_none() => {
                m.exact_length = decoded.trim().parse::<u64>().ok();
            }
            _ => m.other_params.push(KeyValue {
                key: key.to_string(),
                value: decoded,
            }),
        }
    }

    if m.exact_topics.is_empty() && m.display_name.is_none() && m.trackers.is_empty() {
        return Err("no magnet parameters found (expected at least an xt= info-hash)".into());
    }

    Ok(m)
}

/// Classify an `xt` URN value (e.g. `urn:btih:<hash>`).
fn classify_xt(urn: &str) -> ExactTopic {
    // urn:<namespace>:<value> — split on the FIRST ':' after the `urn:` prefix.
    if let Some(rest) = urn.strip_prefix("urn:") {
        if let Some((ns, value)) = rest.split_once(':') {
            return ExactTopic {
                urn: urn.to_string(),
                kind: Some(ns.to_ascii_lowercase()),
                value: Some(value.to_string()),
            };
        }
    }
    ExactTopic {
        urn: urn.to_string(),
        kind: None,
        value: None,
    }
}

/// The parts used to build a magnet link. All fields are optional except that
/// at least an info-hash must be supplied.
#[derive(Debug, Clone, Default)]
pub struct MagnetParts<'a> {
    /// The BitTorrent v1 info-hash — 40 hex chars, 32 base32 chars, or a full
    /// `urn:btih:<hash>` (the prefix is added automatically when absent).
    pub info_hash: &'a str,
    pub display_name: &'a str,
    /// Tracker URLs, one per entry (already split by the caller).
    pub trackers: Vec<&'a str>,
    /// Web-seed URLs, one per entry.
    pub web_seeds: Vec<&'a str>,
    /// Exact length in bytes; `None` to omit.
    pub exact_length: Option<u64>,
}

/// Build a magnet URI from parts. The info-hash is required; a value that is
/// neither 40-hex nor 32-base32 nor an explicit `urn:` is rejected.
pub fn build(parts: &MagnetParts) -> Result<String, String> {
    let raw = parts.info_hash.trim();
    if raw.is_empty() {
        return Err("info_hash is required to build a magnet link".into());
    }

    // Accept an already-qualified URN, otherwise validate + prefix btih.
    let xt = if raw.to_ascii_lowercase().starts_with("urn:") {
        raw.to_string()
    } else if is_hex40(raw) {
        format!("urn:btih:{}", raw.to_ascii_lowercase())
    } else if is_base32_32(raw) {
        format!("urn:btih:{}", raw.to_ascii_uppercase())
    } else {
        return Err(format!(
            "invalid info_hash {raw:?}: expected 40 hex chars, 32 base32 chars, or a urn:btih:… value"
        ));
    };

    let mut params: Vec<String> = vec![format!("xt={xt}")];

    let dn = parts.display_name.trim();
    if !dn.is_empty() {
        params.push(format!("dn={}", encode(dn)));
    }
    if let Some(xl) = parts.exact_length {
        params.push(format!("xl={xl}"));
    }
    for tr in parts.trackers.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        params.push(format!("tr={}", encode(tr)));
    }
    for ws in parts.web_seeds.iter().map(|w| w.trim()).filter(|w| !w.is_empty()) {
        params.push(format!("ws={}", encode(ws)));
    }

    Ok(format!("magnet:?{}", params.join("&")))
}

/// Parse + return pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let parsed = parse(input)?;
    serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())
}

/// Split a multi-value text field into trimmed, non-empty entries. Accepts
/// either newline- or comma-separated lists (a tracker list pasted either way).
fn split_list(s: &str) -> Vec<&str> {
    s.split(['\n', ','])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .collect()
}

/// Unified entry point shared by every surface (chat / CLI / page).
///
/// - `mode` (`"parse"` default | `"build"`): direction.
/// - parse mode reads `magnet`; `human` chooses the rendering (`true` → the
///   page's aligned text view, `false` → pretty JSON for chat/CLI).
/// - build mode reads `info_hash` (required) + `display_name` / `trackers` /
///   `web_seeds` (newline- or comma-separated) / `exact_length`, and returns
///   the assembled `magnet:?…` URI (identical for both surfaces).
#[allow(clippy::too_many_arguments)]
pub fn dispatch(
    mode: &str,
    magnet: &str,
    info_hash: &str,
    display_name: &str,
    trackers: &str,
    web_seeds: &str,
    exact_length: Option<u64>,
    human: bool,
) -> Result<String, String> {
    match mode.trim() {
        "" | "parse" => {
            if human {
                render(magnet)
            } else {
                run(magnet)
            }
        }
        "build" => build(&MagnetParts {
            info_hash,
            display_name,
            trackers: split_list(trackers),
            web_seeds: split_list(web_seeds),
            exact_length,
        }),
        other => Err(format!(
            "invalid mode {other:?}: expected \"parse\" or \"build\""
        )),
    }
}

/// Human-readable rendering of a parsed magnet (used by the page in parse mode).
pub fn render(input: &str) -> Result<String, String> {
    let m = parse(input)?;
    let mut out = String::new();
    let row = |out: &mut String, label: &str, val: &str| {
        out.push_str(&format!("{label:<16}{val}\n"));
    };
    if let Some(h) = &m.info_hash {
        let enc = m.info_hash_encoding.as_deref().unwrap_or("");
        row(&mut out, "Info hash (v1):", &format!("{h}  ({enc})"));
    }
    if let Some(h) = &m.info_hash_v2 {
        row(&mut out, "Info hash (v2):", h);
    }
    if let Some(n) = &m.display_name {
        row(&mut out, "Display name:", n);
    }
    if let Some(xl) = m.exact_length {
        row(
            &mut out,
            "Exact length:",
            &format!("{xl} bytes ({})", human_bytes(xl)),
        );
    }
    if !m.keywords.is_empty() {
        row(&mut out, "Keywords:", &m.keywords.join(", "));
    }
    let list = |out: &mut String, label: &str, items: &[String]| {
        if !items.is_empty() {
            out.push_str(&format!("\n{label} ({}):\n", items.len()));
            for it in items {
                out.push_str(&format!("  {it}\n"));
            }
        }
    };
    list(&mut out, "Trackers", &m.trackers);
    list(&mut out, "Web seeds", &m.web_seeds);
    list(&mut out, "Acceptable sources", &m.acceptable_sources);
    list(&mut out, "Exact sources", &m.exact_sources);
    if !m.other_params.is_empty() {
        out.push_str(&format!("\nOther parameters ({}):\n", m.other_params.len()));
        for kv in &m.other_params {
            out.push_str(&format!("  {} = {}\n", kv.key, kv.value));
        }
    }
    if out.is_empty() {
        out.push_str("(no recognisable magnet parameters)\n");
    }
    Ok(out)
}

/// Format a byte count as a short human-readable size (e.g. `1.5 GB`).
fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} {}", UNITS[0])
    } else {
        format!("{v:.2} {}", UNITS[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_magnet() {
        let mag = "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=Some+File+Name&tr=udp%3A%2F%2Ftracker.example.com%3A1337&tr=http%3A%2F%2Ftrack2.org%2Fannounce&xl=123456";
        let m = parse(mag).unwrap();
        assert_eq!(
            m.info_hash.as_deref(),
            Some("c12fe1c06bba254a9dc9f519b335aa7c1367a88a")
        );
        assert_eq!(m.info_hash_encoding.as_deref(), Some("hex"));
        assert_eq!(m.display_name.as_deref(), Some("Some File Name"));
        assert_eq!(m.trackers.len(), 2);
        assert_eq!(m.trackers[0], "udp://tracker.example.com:1337");
        assert_eq!(m.trackers[1], "http://track2.org/announce");
        assert_eq!(m.exact_length, Some(123456));
    }

    #[test]
    fn base32_info_hash_is_normalised_to_hex() {
        let hex = "c12fe1c06bba254a9dc9f519b335aa7c1367a88a";
        // Build the base32 form from the hash bytes, then confirm parse() maps it back.
        let bytes: Vec<u8> = (0..20)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        let mut b32 = String::new();
        let mut bits = 0u32;
        let mut nbits = 0u32;
        for b in bytes {
            bits = (bits << 8) | b as u32;
            nbits += 8;
            while nbits >= 5 {
                nbits -= 5;
                b32.push(ALPHA[((bits >> nbits) & 0x1f) as usize] as char);
            }
        }
        if nbits > 0 {
            b32.push(ALPHA[((bits << (5 - nbits)) & 0x1f) as usize] as char);
        }
        assert_eq!(b32.len(), 32);
        let mag = format!("magnet:?xt=urn:btih:{b32}");
        let m = parse(&mag).unwrap();
        assert_eq!(m.info_hash.as_deref(), Some(hex));
        assert_eq!(m.info_hash_encoding.as_deref(), Some("base32"));
    }

    #[test]
    fn parses_v2_multihash_and_keywords() {
        let mag = "magnet:?xt=urn:btmh:1220caf1e1abc&kt=linux+iso+free";
        let m = parse(mag).unwrap();
        assert_eq!(m.info_hash_v2.as_deref(), Some("1220caf1e1abc"));
        assert_eq!(m.keywords, vec!["linux", "iso", "free"]);
        assert!(m.info_hash.is_none());
    }

    #[test]
    fn accepts_input_without_scheme() {
        let m = parse("xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=x").unwrap();
        assert_eq!(m.display_name.as_deref(), Some("x"));
        assert!(m.info_hash.is_some());
    }

    #[test]
    fn indexed_keys_collapse_to_base() {
        let m = parse("magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&tr.1=udp%3A%2F%2Fa&tr.2=udp%3A%2F%2Fb").unwrap();
        assert_eq!(m.trackers, vec!["udp://a", "udp://b"]);
    }

    #[test]
    fn unknown_params_go_to_other() {
        let m = parse(
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&so=0,1,2",
        )
        .unwrap();
        assert_eq!(m.other_params.len(), 1);
        assert_eq!(m.other_params[0].key, "so");
        assert_eq!(m.other_params[0].value, "0,1,2");
    }

    #[test]
    fn rejects_empty() {
        assert!(parse("   ").is_err());
    }

    #[test]
    fn rejects_non_magnet_url() {
        assert!(parse("https://example.com/file").is_err());
    }

    #[test]
    fn rejects_input_with_no_magnet_params() {
        assert!(parse("magnet:?foo=bar").is_err());
    }

    #[test]
    fn run_emits_valid_json() {
        let j =
            run("magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=hi").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["display_name"], "hi");
        assert_eq!(v["info_hash"], "c12fe1c06bba254a9dc9f519b335aa7c1367a88a");
    }

    #[test]
    fn build_from_hex_info_hash() {
        let parts = MagnetParts {
            info_hash: "C12FE1C06BBA254A9DC9F519B335AA7C1367A88A",
            display_name: "My File",
            trackers: vec!["udp://tracker.example.com:1337", " "],
            web_seeds: vec![],
            exact_length: Some(123456),
        };
        let mag = build(&parts).unwrap();
        assert_eq!(
            mag,
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=My%20File&xl=123456&tr=udp%3A%2F%2Ftracker.example.com%3A1337"
        );
    }

    #[test]
    fn build_round_trips_through_parse() {
        let parts = MagnetParts {
            info_hash: "c12fe1c06bba254a9dc9f519b335aa7c1367a88a",
            display_name: "Round Trip",
            trackers: vec!["http://a.org/announce"],
            web_seeds: vec!["https://seed.example/file"],
            exact_length: None,
        };
        let mag = build(&parts).unwrap();
        let m = parse(&mag).unwrap();
        assert_eq!(m.display_name.as_deref(), Some("Round Trip"));
        assert_eq!(m.trackers, vec!["http://a.org/announce"]);
        assert_eq!(m.web_seeds, vec!["https://seed.example/file"]);
    }

    #[test]
    fn build_rejects_missing_info_hash() {
        let parts = MagnetParts {
            info_hash: "   ",
            ..Default::default()
        };
        assert!(build(&parts).is_err());
    }

    #[test]
    fn build_rejects_bad_info_hash() {
        let parts = MagnetParts {
            info_hash: "not-a-hash",
            ..Default::default()
        };
        let err = build(&parts).unwrap_err();
        assert!(err.contains("invalid info_hash"), "got: {err}");
    }

    #[test]
    fn build_accepts_explicit_urn() {
        let parts = MagnetParts {
            info_hash: "urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a",
            ..Default::default()
        };
        let mag = build(&parts).unwrap();
        assert_eq!(
            mag,
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a"
        );
    }

    #[test]
    fn dispatch_parse_json_and_build() {
        // parse mode (non-human) → JSON
        let j = dispatch(
            "parse",
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=hi",
            "",
            "",
            "",
            "",
            None,
            false,
        )
        .unwrap();
        assert!(j.contains("\"display_name\": \"hi\""));
        // build mode from parts (trackers given as a comma list)
        let mag = dispatch(
            "build",
            "",
            "c12fe1c06bba254a9dc9f519b335aa7c1367a88a",
            "Demo",
            "udp://a, udp://b",
            "",
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            mag,
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=Demo&tr=udp%3A%2F%2Fa&tr=udp%3A%2F%2Fb"
        );
    }

    #[test]
    fn dispatch_rejects_bad_mode() {
        let err = dispatch("frobnicate", "", "", "", "", "", None, false).unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }

    #[test]
    fn render_is_human_readable() {
        let out = render(
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=Demo&tr=udp%3A%2F%2Fa&xl=1048576",
        )
        .unwrap();
        assert!(out.contains("Info hash (v1):"));
        assert!(out.contains("Display name:"));
        assert!(out.contains("Demo"));
        assert!(out.contains("Trackers (1):"));
        assert!(out.contains("1.00 MB"));
    }
}
