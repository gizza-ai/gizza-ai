//! nostr-key-encoder core — convert Nostr identifiers between raw hex and their
//! NIP-19 bech32 forms. Pure compute, no external deps, shared verbatim by the
//! chat skill block, the CLI, and the web page (so it instantiates cleanly under
//! wafer/wasm).
//!
//! Bare 32-byte entities: `npub` (public key), `nsec` (private key), `note`
//! (event id). TLV entities: `nprofile` (pubkey + optional relays), `nevent`
//! (event id + optional relays/author/kind), plus decode-only support for
//! `naddr` / `nrelay`. Nostr uses the plain Bech32 (BIP 173) checksum — NOT
//! bech32m — and, unlike BIP 173, imposes NO 90-character length cap (relay
//! lists routinely exceed it); NIP-19 suggests a 5000-char soft limit instead,
//! which we enforce.

/// The Bech32 data-part alphabet (base-32). Index = 5-bit value 0..=31.
const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
/// Generator coefficients for the BCH checksum polymod (BIP 173).
const GEN: [u32; 5] = [0x3b6a_57b2, 0x2650_8e6d, 0x1ea1_19fa, 0x3d42_33dd, 0x2a14_62b3];
/// NIP-19 soft cap on the encoded string length.
const MAX_LEN: usize = 5000;

/// The BCH checksum polymod over a sequence of 5-bit values (BIP 173).
fn polymod(values: &[u8]) -> u32 {
    let mut chk: u32 = 1;
    for &v in values {
        let top = (chk >> 25) as u8;
        chk = ((chk & 0x1ff_ffff) << 5) ^ (v as u32);
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

/// Expand the HRP for the checksum: high 3 bits of each char, a 0, low 5 bits.
fn hrp_expand(hrp: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(hrp.len() * 2 + 1);
    v.extend(hrp.iter().map(|c| c >> 5));
    v.push(0);
    v.extend(hrp.iter().map(|c| c & 31));
    v
}

/// General power-of-two base conversion (BIP 173 `convertbits`), MSB-first.
/// On encode (8→5) a trailing partial group is zero-padded; on decode (5→8)
/// leftover bits must be zero and fewer than `from` or the input is rejected.
fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::with_capacity(data.len() * from as usize / to as usize + 1);
    let maxv: u32 = (1 << to) - 1;
    let max_acc: u32 = (1 << (from + to - 1)) - 1;
    for &value in data {
        let v = value as u32;
        if (v >> from) != 0 {
            return None;
        }
        acc = ((acc << from) | v) & max_acc;
        bits += from;
        while bits >= to {
            bits -= to;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            out.push(((acc << (to - bits)) & maxv) as u8);
        }
    } else if bits >= from || ((acc << (to - bits)) & maxv) != 0 {
        return None;
    }
    Some(out)
}

/// Encode an HRP + raw byte payload into a plain Bech32 (BIP 173) string.
fn bech32_encode(hrp: &str, data: &[u8]) -> Result<String, String> {
    let hrp_bytes = hrp.as_bytes();
    let five = convert_bits(data, 8, 5, true).ok_or("failed to pack payload into 5-bit groups")?;
    let mut values = hrp_expand(hrp_bytes);
    values.extend_from_slice(&five);
    values.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let m = polymod(&values) ^ 1;
    let checksum: Vec<u8> = (0..6).map(|i| ((m >> (5 * (5 - i))) & 31) as u8).collect();
    let mut s = String::with_capacity(hrp.len() + 1 + five.len() + 6);
    s.push_str(hrp);
    s.push('1');
    for &b in five.iter().chain(checksum.iter()) {
        s.push(CHARSET[b as usize] as char);
    }
    if s.len() > MAX_LEN {
        return Err(format!(
            "encoded string is {} chars; NIP-19 recommends a 5000-char limit (too many/too long relays?)",
            s.len()
        ));
    }
    Ok(s)
}

/// Decode a plain Bech32 (BIP 173) string into its HRP and raw byte payload.
/// No 90-char length cap (Nostr TLV strings are routinely longer); rejects
/// mixed case, bad characters, and checksum failures.
fn bech32_decode(s: &str) -> Result<(String, Vec<u8>), String> {
    let s = s.trim();
    if s.len() > MAX_LEN {
        return Err(format!("string is {} chars; over the 5000-char NIP-19 limit", s.len()));
    }
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    if has_lower && has_upper {
        return Err("mixed-case Bech32 string is invalid".into());
    }
    let lowered = s.to_ascii_lowercase();
    let pos = lowered.rfind('1').ok_or("not a Bech32 string: missing the '1' separator")?;
    if pos == 0 {
        return Err("empty human-readable prefix before the '1' separator".into());
    }
    let (hrp, rest) = (&lowered[..pos], &lowered[pos + 1..]);
    if rest.len() < 6 {
        return Err("Bech32 data part is too short to hold a checksum".into());
    }
    let mut data5 = Vec::with_capacity(rest.len());
    for c in rest.bytes() {
        let idx = CHARSET
            .iter()
            .position(|&x| x == c)
            .ok_or_else(|| format!("invalid Bech32 character {:?}", c as char))?;
        data5.push(idx as u8);
    }
    let mut values = hrp_expand(hrp.as_bytes());
    values.extend_from_slice(&data5);
    if polymod(&values) != 1 {
        return Err("invalid Bech32 checksum (typo or corrupted string?)".into());
    }
    let payload = convert_bits(&data5[..data5.len() - 6], 5, 8, false)
        .ok_or("invalid padding in the Bech32 data part")?;
    Ok((hrp.to_string(), payload))
}

/// Parse a hex string (optional `0x` prefix, whitespace ignored) into bytes.
fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if cleaned.is_empty() {
        return Err("no hex input provided".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex input has an odd number of digits ({}); each byte needs two",
            cleaned.len()
        ));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte {:?}", &cleaned[i..i + 2]))
        })
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Require exactly 32 bytes (a Nostr key or id), with a descriptive error.
fn require_32(bytes: &[u8], what: &str) -> Result<Vec<u8>, String> {
    if bytes.len() != 32 {
        return Err(format!(
            "a {what} must be exactly 32 bytes (64 hex chars); got {} bytes",
            bytes.len()
        ));
    }
    Ok(bytes.to_vec())
}

/// Split a relay list on commas / whitespace / newlines, dropping empties.
fn split_relays(relays: &str) -> Vec<String> {
    relays
        .split([',', '\n', '\r', '\t', ' '])
        .map(|r| r.trim())
        .filter(|r| !r.is_empty())
        .map(|r| r.to_string())
        .collect()
}

/// Append one TLV record (1-byte type, 1-byte length, value). Errors if the
/// value exceeds 255 bytes (the single-byte length field can't hold more).
fn push_tlv(out: &mut Vec<u8>, t: u8, value: &[u8]) -> Result<(), String> {
    if value.len() > 255 {
        return Err(format!(
            "TLV value for type {t} is {} bytes; the 1-byte length field allows at most 255",
            value.len()
        ));
    }
    out.push(t);
    out.push(value.len() as u8);
    out.extend_from_slice(value);
    Ok(())
}

/// Parse a TLV byte stream into (type, value) records.
fn parse_tlv(data: &[u8]) -> Result<Vec<(u8, Vec<u8>)>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if i + 2 > data.len() {
            return Err("truncated TLV record (missing length byte)".into());
        }
        let t = data[i];
        let l = data[i + 1] as usize;
        let start = i + 2;
        let end = start + l;
        if end > data.len() {
            return Err("truncated TLV record (value shorter than its length)".into());
        }
        out.push((t, data[start..end].to_vec()));
        i = end;
    }
    Ok(out)
}

/// The set of known bare (32-byte) prefixes.
fn is_bare_prefix(hrp: &str) -> bool {
    matches!(hrp, "npub" | "nsec" | "note")
}
/// The set of known TLV prefixes.
fn is_tlv_prefix(hrp: &str) -> bool {
    matches!(hrp, "nprofile" | "nevent" | "naddr" | "nrelay")
}

/// Does the input look like a NIP-19 bech32 string (known prefix + '1')?
fn looks_like_bech32(input: &str) -> bool {
    let lowered = input.trim().to_ascii_lowercase();
    for p in ["npub", "nsec", "note", "nprofile", "nevent", "naddr", "nrelay"] {
        if lowered.starts_with(&format!("{p}1")) {
            return true;
        }
    }
    false
}

/// Build the TLV payload for an nprofile: special(pubkey) + relays.
fn build_nprofile(input: &str, relays: &str) -> Result<Vec<u8>, String> {
    let pubkey = require_32(&parse_hex(input)?, "public key")?;
    let mut tlv = Vec::new();
    push_tlv(&mut tlv, 0, &pubkey)?;
    for r in split_relays(relays) {
        push_tlv(&mut tlv, 1, r.as_bytes())?;
    }
    Ok(tlv)
}

/// Build the TLV payload for an nevent: special(id) + relays + author + kind.
/// Records are emitted in NIP-19 order (0,1,2,3) so output matches reference
/// encoders. `kind < 0` omits the kind record.
fn build_nevent(input: &str, relays: &str, author: &str, kind: i64) -> Result<Vec<u8>, String> {
    let id = require_32(&parse_hex(input)?, "event id")?;
    let mut tlv = Vec::new();
    push_tlv(&mut tlv, 0, &id)?;
    for r in split_relays(relays) {
        push_tlv(&mut tlv, 1, r.as_bytes())?;
    }
    if !author.trim().is_empty() {
        let author = require_32(&parse_hex(author)?, "author public key")?;
        push_tlv(&mut tlv, 2, &author)?;
    }
    if kind >= 0 {
        if kind > u32::MAX as i64 {
            return Err(format!("kind {kind} is out of range (0..=4294967295)"));
        }
        push_tlv(&mut tlv, 3, &(kind as u32).to_be_bytes())?;
    }
    Ok(tlv)
}

/// Render a decoded TLV entity as a labeled, multi-line report.
fn render_tlv(hrp: &str, records: &[(u8, Vec<u8>)]) -> Result<String, String> {
    let mut lines = vec![format!("type: {hrp}")];
    // The special (type 0) label depends on the prefix.
    let special_label = match hrp {
        "nprofile" => "pubkey",
        "nevent" => "id",
        "naddr" => "identifier",
        "nrelay" => "relay",
        _ => "special",
    };
    for (t, v) in records {
        match t {
            0 => {
                if hrp == "naddr" || hrp == "nrelay" {
                    // special is an ASCII/UTF-8 string, not a 32-byte key.
                    let s = String::from_utf8_lossy(v);
                    lines.push(format!("{special_label}: {s}"));
                } else {
                    lines.push(format!("{special_label}: {}", to_hex(v)));
                }
            }
            1 => lines.push(format!("relay: {}", String::from_utf8_lossy(v))),
            2 => lines.push(format!("author: {}", to_hex(v))),
            3 => {
                if v.len() != 4 {
                    return Err("kind TLV must be a 4-byte big-endian integer".into());
                }
                let k = u32::from_be_bytes([v[0], v[1], v[2], v[3]]);
                lines.push(format!("kind: {k}"));
            }
            other => lines.push(format!("tlv-type-{other}: {}", to_hex(v))),
        }
    }
    Ok(lines.join("\n"))
}

/// Decode any NIP-19 bech32 string to hex (bare) or a labeled report (TLV).
fn decode(input: &str) -> Result<String, String> {
    let (hrp, payload) = bech32_decode(input)?;
    if is_bare_prefix(&hrp) {
        let bytes = require_32(&payload, "Nostr key/id")?;
        Ok(to_hex(&bytes))
    } else if is_tlv_prefix(&hrp) {
        let records = parse_tlv(&payload)?;
        render_tlv(&hrp, &records)
    } else {
        Err(format!(
            "unknown NIP-19 prefix {hrp:?}: expected one of npub, nsec, note, nprofile, nevent, naddr, nrelay"
        ))
    }
}

/// Encode raw hex into a NIP-19 bech32 string of the requested `type`.
fn encode(input: &str, type_: &str, relays: &str, author: &str, kind: i64) -> Result<String, String> {
    match type_.trim().to_ascii_lowercase().as_str() {
        "npub" | "nsec" | "note" => {
            let hrp = type_.trim().to_ascii_lowercase();
            let bytes = require_32(&parse_hex(input)?, "Nostr key/id")?;
            bech32_encode(&hrp, &bytes)
        }
        "nprofile" => bech32_encode("nprofile", &build_nprofile(input, relays)?),
        "nevent" => bech32_encode("nevent", &build_nevent(input, relays, author, kind)?),
        other => Err(format!(
            "invalid type {other:?}: expected npub, nsec, note, nprofile, or nevent"
        )),
    }
}

/// Top-level entry point shared by every surface.
///
/// * `mode` — `auto` (default: bech32 in → decode, hex in → encode), `encode`, or `decode`.
/// * `type_` — target prefix when encoding hex (npub/nsec/note/nprofile/nevent).
/// * `relays` — comma/space/newline-separated relay URLs for nprofile/nevent.
/// * `author` — hex author pubkey for nevent (optional).
/// * `kind` — event kind for nevent (`< 0` omits it).
pub fn convert(
    input: &str,
    mode: &str,
    type_: &str,
    relays: &str,
    author: &str,
    kind: i64,
) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("no input provided".into());
    }
    let mode = mode.trim().to_ascii_lowercase();
    let mode = if mode.is_empty() { "auto" } else { mode.as_str() };
    match mode {
        "decode" => decode(input),
        "encode" => encode(input, type_, relays, author, kind),
        "auto" => {
            if looks_like_bech32(input) {
                decode(input)
            } else {
                encode(input, type_, relays, author, kind)
            }
        }
        other => Err(format!(
            "invalid mode {other:?}: expected auto, encode, or decode"
        )),
    }
}

/// Thin wrapper kept so the scaffold's `run(&str)` name still resolves; not used
/// by the descriptor path (which calls `convert`). Treats the string as an
/// auto-detected npub target.
pub fn run(input: &str) -> Result<String, String> {
    convert(input, "auto", "npub", "", "", -1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // NIP-19 test vectors.
    const NPUB: &str = "npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg";
    const NPUB_HEX: &str = "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e";
    const NSEC: &str = "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5";
    const NSEC_HEX: &str = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa";
    const NPUB2_HEX: &str = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
    const NPUB2: &str = "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6";
    const NPROFILE: &str = "nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p";

    #[test]
    fn decode_npub_vector() {
        assert_eq!(convert(NPUB, "auto", "npub", "", "", -1).unwrap(), NPUB_HEX);
    }

    #[test]
    fn encode_npub_vector() {
        assert_eq!(convert(NPUB_HEX, "encode", "npub", "", "", -1).unwrap(), NPUB);
    }

    #[test]
    fn encode_npub_second_vector() {
        assert_eq!(convert(NPUB2_HEX, "encode", "npub", "", "", -1).unwrap(), NPUB2);
    }

    #[test]
    fn nsec_roundtrip_vector() {
        assert_eq!(convert(NSEC, "decode", "npub", "", "", -1).unwrap(), NSEC_HEX);
        assert_eq!(convert(NSEC_HEX, "encode", "nsec", "", "", -1).unwrap(), NSEC);
    }

    #[test]
    fn note_roundtrip() {
        let enc = convert(NPUB_HEX, "encode", "note", "", "", -1).unwrap();
        assert!(enc.starts_with("note1"), "got {enc}");
        assert_eq!(convert(&enc, "decode", "npub", "", "", -1).unwrap(), NPUB_HEX);
    }

    #[test]
    fn nprofile_matches_vector() {
        let enc = convert(
            NPUB2_HEX,
            "encode",
            "nprofile",
            "wss://r.x.com, wss://djbas.sadkb.com",
            "",
            -1,
        )
        .unwrap();
        assert_eq!(enc, NPROFILE);
    }

    #[test]
    fn nprofile_decode_reports_pubkey_and_relays() {
        let out = convert(NPROFILE, "decode", "npub", "", "", -1).unwrap();
        assert!(out.contains("type: nprofile"), "got {out}");
        assert!(out.contains(&format!("pubkey: {NPUB2_HEX}")), "got {out}");
        assert!(out.contains("relay: wss://r.x.com"), "got {out}");
        assert!(out.contains("relay: wss://djbas.sadkb.com"), "got {out}");
    }

    #[test]
    fn nevent_roundtrip_with_all_fields() {
        let enc = convert(
            NPUB_HEX,
            "encode",
            "nevent",
            "wss://relay.example.com",
            NPUB2_HEX,
            1,
        )
        .unwrap();
        assert!(enc.starts_with("nevent1"), "got {enc}");
        let dec = convert(&enc, "decode", "npub", "", "", -1).unwrap();
        assert!(dec.contains(&format!("id: {NPUB_HEX}")), "got {dec}");
        assert!(dec.contains("relay: wss://relay.example.com"), "got {dec}");
        assert!(dec.contains(&format!("author: {NPUB2_HEX}")), "got {dec}");
        assert!(dec.contains("kind: 1"), "got {dec}");
    }

    #[test]
    fn auto_detects_hex_as_encode() {
        // A bare 64-char hex with no prefix → encode to the default npub type.
        assert_eq!(convert(NPUB_HEX, "auto", "npub", "", "", -1).unwrap(), NPUB);
    }

    #[test]
    fn bad_checksum_errors() {
        let mut bad = NPUB.to_string();
        bad.pop();
        bad.push('x');
        assert!(convert(&bad, "decode", "npub", "", "", -1).is_err());
    }

    #[test]
    fn wrong_length_hex_errors() {
        assert!(convert("abcd", "encode", "npub", "", "", -1).is_err());
    }

    #[test]
    fn unknown_prefix_errors() {
        // Valid bech32 but not a Nostr prefix.
        assert!(convert("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4", "decode", "npub", "", "", -1).is_err());
    }
}
