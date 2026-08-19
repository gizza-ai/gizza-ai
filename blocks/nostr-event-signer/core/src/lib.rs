//! nostr-event-signer core — build a Nostr event from its parts, hash it into a
//! NIP-01 event id, and sign that id with a BIP-340 Schnorr signature over
//! secp256k1. Pure compute, no I/O, shared verbatim by the chat skill block, the
//! CLI, and the web page (so it instantiates cleanly under wafer/wasm).
//!
//! NIP-01 fixes the id preimage as a whitespace-free JSON array
//! `[0, <pubkey hex>, <created_at>, <kind>, <tags>, <content>]`; `id` is the
//! lowercase hex SHA-256 of that string and `sig` is the 64-byte Schnorr
//! signature over the raw 32-byte id. The public key is the 32-byte **x-only**
//! form, which is why an odd-Y secret key is silently negated by BIP-340 — the
//! same `nsec` therefore always yields the same `npub`.

use k256::schnorr::signature::hazmat::{PrehashSigner, PrehashVerifier};
use k256::schnorr::{Signature, SigningKey};
use sha2::{Digest, Sha256};

/// The Bech32 data-part alphabet (base-32). Index = 5-bit value 0..=31.
const CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
/// Generator coefficients for the BCH checksum polymod (BIP 173).
const GEN: [u32; 5] = [0x3b6a_57b2, 0x2650_8e6d, 0x1ea1_19fa, 0x3d42_33dd, 0x2a14_62b3];

/// Largest accepted `content` length, in bytes. Relays reject far smaller
/// events; this only exists so a runaway paste can't exhaust the wasm sandbox.
pub const MAX_CONTENT_BYTES: usize = 256 * 1024;
/// Largest accepted number of tags on one event.
pub const MAX_TAGS: usize = 2000;
/// Highest proof-of-work difficulty (leading zero bits) this tool will mine.
pub const MAX_POW_BITS: u32 = 20;
/// Safety valve for the mining loop, so a pathological run still terminates.
const MAX_POW_ATTEMPTS: u64 = 40_000_000;

/// A fully signed Nostr event. Field order is the canonical wire order, and
/// serde serializes struct fields in declaration order, so the emitted JSON
/// matches what relays and other clients print.
#[derive(Debug, serde::Serialize)]
pub struct SignedEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: i64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

// ---------------------------------------------------------------------------
// Bech32 (BIP 173) — only what NIP-19 needs: decode an `nsec`, encode `npub` /
// `note`. Nostr uses the plain bech32 checksum, not bech32m.
// ---------------------------------------------------------------------------

fn polymod(values: &[u8]) -> u32 {
    let mut chk: u32 = 1;
    for &v in values {
        let top = (chk >> 25) as u8;
        chk = ((chk & 0x01ff_ffff) << 5) ^ (v as u32);
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

fn hrp_expand(hrp: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(hrp.len() * 2 + 1);
    v.extend(hrp.iter().map(|c| c >> 5));
    v.push(0);
    v.extend(hrp.iter().map(|c| c & 31));
    v
}

/// General power-of-two base conversion (BIP 173 `convertbits`), MSB-first.
fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let maxv: u32 = (1 << to) - 1;
    let max_acc: u32 = (1 << (from + to - 1)) - 1;
    let mut out = Vec::with_capacity(data.len() * from as usize / to as usize + 2);
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

/// Encode 32 raw bytes under `hrp` as a bech32 string (`npub1…`, `note1…`).
fn bech32_encode(hrp: &str, data: &[u8]) -> String {
    let five = convert_bits(data, 8, 5, true).expect("8->5 conversion always succeeds");
    let mut values = hrp_expand(hrp.as_bytes());
    values.extend_from_slice(&five);
    values.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let polymod = polymod(&values) ^ 1;
    let mut out = String::with_capacity(hrp.len() + 1 + five.len() + 6);
    out.push_str(hrp);
    out.push('1');
    for v in &five {
        out.push(CHARSET[*v as usize] as char);
    }
    for i in 0..6 {
        let v = ((polymod >> (5 * (5 - i))) & 31) as usize;
        out.push(CHARSET[v] as char);
    }
    out
}

/// Decode a bech32 string into `(hrp, payload bytes)`.
fn bech32_decode(s: &str) -> Result<(String, Vec<u8>), String> {
    if s.chars().any(|c| !c.is_ascii() || (c as u32) < 33 || (c as u32) > 126) {
        return Err("invalid NIP-19 string: contains a non-printable or non-ASCII character".into());
    }
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    if has_lower && has_upper {
        return Err("invalid NIP-19 string: mixed upper and lower case".into());
    }
    let lowered = s.to_ascii_lowercase();
    let pos = lowered
        .rfind('1')
        .ok_or_else(|| "invalid NIP-19 string: missing the '1' separator".to_string())?;
    if pos == 0 || pos + 7 > lowered.len() {
        return Err("invalid NIP-19 string: malformed prefix or truncated checksum".into());
    }
    let hrp = &lowered[..pos];
    let mut values: Vec<u8> = Vec::with_capacity(lowered.len() - pos - 1);
    for c in lowered[pos + 1..].bytes() {
        match CHARSET.iter().position(|&x| x == c) {
            Some(i) => values.push(i as u8),
            None => {
                return Err(format!(
                    "invalid NIP-19 string: {:?} is not a Bech32 character",
                    c as char
                ))
            }
        }
    }
    let mut check = hrp_expand(hrp.as_bytes());
    check.extend_from_slice(&values);
    if polymod(&check) != 1 {
        return Err("invalid Bech32 checksum: the key was mistyped or truncated".into());
    }
    let payload = convert_bits(&values[..values.len() - 6], 5, 8, false)
        .ok_or_else(|| "invalid NIP-19 string: bad 5-to-8-bit padding".to_string())?;
    Ok((hrp.to_string(), payload))
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// Parse the signing key: `nsec1…` bech32 or a 64-character hex secret.
fn parse_secret_key(raw: &str) -> Result<[u8; 32], String> {
    let s: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if s.is_empty() {
        return Err("no private key given: paste an nsec1… string or a 64-character hex secret key"
            .into());
    }
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("npub1") || lower.starts_with("note1") {
        return Err(format!(
            "{} is a public identifier, not a private key — signing needs your nsec1… (or hex) secret key",
            &lower[..lower.len().min(5)]
        ));
    }
    if lower.starts_with("ncryptsec1") {
        return Err("ncryptsec (NIP-49) keys are password-encrypted and are not supported — decrypt it in your signer first, then paste the nsec".into());
    }
    if lower.starts_with("nsec1") {
        let (hrp, payload) = bech32_decode(&s)?;
        if hrp != "nsec" {
            return Err(format!("expected an nsec key, got prefix {hrp:?}"));
        }
        if payload.len() != 32 {
            return Err(format!(
                "invalid nsec: decodes to {} bytes, expected 32",
                payload.len()
            ));
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&payload);
        return Ok(out);
    }
    let hexpart = lower.strip_prefix("0x").unwrap_or(&lower);
    if hexpart.len() != 64 || !hexpart.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "unrecognized private key: expected an nsec1… string or 64 hex characters, got {} character(s)",
            s.chars().count()
        ));
    }
    let bytes = hex::decode(hexpart).map_err(|e| format!("invalid hex private key: {e}"))?;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// Parse the `tags` field. Accepts either the JSON array-of-arrays form
/// (`[["e","<id>"],["p","<pubkey>"]]`) or a shorthand where each tag sits on its
/// own line (or is comma-separated) as `name=value1;value2`.
fn parse_tags(raw: &str) -> Result<Vec<Vec<String>>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        let v: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("tags is not valid JSON: {e}"))?;
        return json_tags(&v);
    }
    let mut out = Vec::new();
    for entry in trimmed.split(['\n', ',']) {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (name, rest) = match entry.split_once('=') {
            Some((n, r)) => (n.trim(), Some(r)),
            None => (entry, None),
        };
        if name.is_empty() {
            return Err(format!("tag {entry:?} has an empty name — write it as name=value"));
        }
        let mut tag = vec![name.to_string()];
        if let Some(rest) = rest {
            tag.extend(rest.split(';').map(|v| v.trim().to_string()));
        }
        out.push(tag);
    }
    check_tag_count(out.len())?;
    Ok(out)
}

/// Validate a `serde_json` value as a NIP-01 tag list (array of arrays of strings).
fn json_tags(v: &serde_json::Value) -> Result<Vec<Vec<String>>, String> {
    let arr = v
        .as_array()
        .ok_or_else(|| "tags JSON must be an array of arrays, e.g. [[\"e\",\"<event id>\"]]".to_string())?;
    check_tag_count(arr.len())?;
    let mut out = Vec::with_capacity(arr.len());
    for (i, tag) in arr.iter().enumerate() {
        let items = tag.as_array().ok_or_else(|| {
            format!("tag #{} is not an array — each tag must look like [\"e\",\"<event id>\"]", i + 1)
        })?;
        if items.is_empty() {
            return Err(format!("tag #{} is empty — a tag needs at least a name", i + 1));
        }
        let mut row = Vec::with_capacity(items.len());
        for item in items {
            match item.as_str() {
                Some(s) => row.push(s.to_string()),
                None => {
                    return Err(format!(
                        "tag #{} contains a non-string value ({item}) — NIP-01 tags are arrays of strings, so quote it",
                        i + 1
                    ))
                }
            }
        }
        out.push(row);
    }
    Ok(out)
}

fn check_tag_count(n: usize) -> Result<(), String> {
    if n > MAX_TAGS {
        return Err(format!("too many tags: {n} (limit {MAX_TAGS})"));
    }
    Ok(())
}

/// Fields an event `template` may override.
#[derive(Default)]
struct Template {
    kind: Option<u32>,
    content: Option<String>,
    tags: Option<Vec<Vec<String>>>,
    created_at: Option<i64>,
}

/// Parse an optional unsigned-event JSON object. Any `id`/`sig`/`pubkey` present
/// is ignored — they are always recomputed from the signing key.
fn parse_template(raw: &str) -> Result<Template, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Template::default());
    }
    let v: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("template is not valid JSON: {e}"))?;
    let obj = v.as_object().ok_or_else(|| {
        "template must be a JSON object like {\"kind\":1,\"content\":\"hello\",\"tags\":[]}"
            .to_string()
    })?;
    let mut t = Template::default();
    if let Some(k) = obj.get("kind") {
        let n = k
            .as_i64()
            .ok_or_else(|| format!("template kind must be a number, got {k}"))?;
        t.kind = Some(check_kind(n as f64)?);
    }
    if let Some(c) = obj.get("content") {
        t.content = Some(
            c.as_str()
                .ok_or_else(|| format!("template content must be a string, got {c}"))?
                .to_string(),
        );
    }
    if let Some(tags) = obj.get("tags") {
        t.tags = Some(json_tags(tags)?);
    }
    if let Some(ts) = obj.get("created_at") {
        t.created_at = Some(
            ts.as_i64()
                .ok_or_else(|| format!("template created_at must be a number, got {ts}"))?,
        );
    }
    Ok(t)
}

fn check_kind(kind: f64) -> Result<u32, String> {
    if !kind.is_finite() || kind.fract() != 0.0 || !(0.0..=65535.0).contains(&kind) {
        return Err(format!(
            "invalid kind {kind}: a Nostr event kind is a whole number from 0 to 65535 (1 = text note)"
        ));
    }
    Ok(kind as u32)
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

/// The NIP-01 id preimage: a whitespace-free JSON array.
fn serialize_for_id(
    pubkey: &str,
    created_at: i64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Result<String, String> {
    serde_json::to_string(&(0u8, pubkey, created_at, kind, tags, content))
        .map_err(|e| format!("could not serialize the event: {e}"))
}

fn event_id(serialized: &str) -> [u8; 32] {
    let digest = Sha256::digest(serialized.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Leading zero BITS of an event id — the NIP-13 proof-of-work difficulty.
fn leading_zero_bits(id: &[u8; 32]) -> u32 {
    let mut bits = 0;
    for b in id {
        if *b == 0 {
            bits += 8;
        } else {
            bits += b.leading_zeros();
            break;
        }
    }
    bits
}

/// Format a unix timestamp as `YYYY-MM-DDTHH:MM:SSZ` without pulling a date
/// crate (Howard Hinnant's civil-from-days algorithm).
fn format_utc(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Build, mine (optionally), and Schnorr-sign a Nostr event.
///
/// Returns the signed event plus the difficulty actually achieved.
pub fn build_signed_event(
    nsec: &str,
    content: &str,
    kind: f64,
    tags: &str,
    created_at: f64,
    template: &str,
    pow: f64,
    now_unix: i64,
) -> Result<(SignedEvent, u32), String> {
    let secret = parse_secret_key(nsec)?;
    let signing_key = SigningKey::from_bytes(&secret).map_err(|_| {
        "invalid secp256k1 private key: the value is zero or not below the curve order — check the key you pasted".to_string()
    })?;
    let pubkey = hex::encode(signing_key.verifying_key().to_bytes());

    let tmpl = parse_template(template)?;
    let mut kind = check_kind(kind)?;
    let mut content = content.to_string();
    let mut tags = parse_tags(tags)?;
    let mut created_at = if !created_at.is_finite() || created_at.fract() != 0.0 {
        return Err(format!(
            "invalid created_at {created_at}: pass a whole number of seconds since 1970-01-01, or 0 to stamp the event with the current time"
        ));
    } else {
        created_at as i64
    };

    if let Some(v) = tmpl.kind {
        kind = v;
    }
    if let Some(v) = tmpl.content {
        content = v;
    }
    if let Some(v) = tmpl.tags {
        tags = v;
    }
    if let Some(v) = tmpl.created_at {
        created_at = v;
    }
    if created_at <= 0 {
        created_at = now_unix;
    }

    if content.len() > MAX_CONTENT_BYTES {
        return Err(format!(
            "content is {} bytes, over the {MAX_CONTENT_BYTES}-byte limit — relays reject events far smaller than this",
            content.len()
        ));
    }
    check_tag_count(tags.len())?;

    let target = if !pow.is_finite() || pow.fract() != 0.0 || pow < 0.0 {
        return Err(format!("invalid pow {pow}: pass a whole number of leading zero bits from 0 to {MAX_POW_BITS}"));
    } else {
        pow as u32
    };
    if target > MAX_POW_BITS {
        return Err(format!(
            "pow {target} is above the {MAX_POW_BITS}-bit limit — mining that far takes a dedicated miner, not a browser tab"
        ));
    }

    // NIP-13: proof of work lives in a ["nonce", <counter>, <target>] tag. Drop
    // any nonce the caller supplied so we own the one we mine.
    if target > 0 {
        tags.retain(|t| t.first().map(|n| n != "nonce").unwrap_or(true));
        tags.push(vec!["nonce".to_string(), "0".to_string(), target.to_string()]);
    }

    let (serialized, id) = if target == 0 {
        let s = serialize_for_id(&pubkey, created_at, kind, &tags, &content)?;
        let id = event_id(&s);
        (s, id)
    } else {
        let nonce_idx = tags.len() - 1;
        let mut counter: u64 = 0;
        loop {
            tags[nonce_idx][1] = counter.to_string();
            let s = serialize_for_id(&pubkey, created_at, kind, &tags, &content)?;
            let id = event_id(&s);
            if leading_zero_bits(&id) >= target {
                break (s, id);
            }
            counter += 1;
            if counter >= MAX_POW_ATTEMPTS {
                return Err(format!(
                    "gave up mining {target} bits of proof of work after {MAX_POW_ATTEMPTS} attempts — try a lower difficulty"
                ));
            }
        }
    };

    let signature: Signature = signing_key
        .sign_prehash(&id)
        .map_err(|e| format!("Schnorr signing failed: {e}"))?;
    // Self-check: never emit a signature our own verifier rejects.
    signing_key
        .verifying_key()
        .verify_prehash(&id, &signature)
        .map_err(|_| "internal error: the produced signature failed verification".to_string())?;

    let achieved = leading_zero_bits(&id);
    let _ = serialized;
    Ok((
        SignedEvent {
            id: hex::encode(id),
            pubkey,
            created_at,
            kind,
            tags,
            content,
            sig: hex::encode(signature.to_bytes()),
        },
        achieved,
    ))
}

/// Serialize any serde value, indented or compact.
fn to_json<T: serde::Serialize + ?Sized>(v: &T, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(v)
    } else {
        serde_json::to_string(v)
    }
    .map_err(|e| format!("could not render JSON: {e}"))
}

/// Render a signed event in the requested output shape.
///
/// Everything serializes the `SignedEvent` STRUCT directly rather than going
/// through `serde_json::Value`: serde emits struct fields in declaration order,
/// while a `Value` object is a sorted map and would alphabetize the keys.
fn render(event: &SignedEvent, achieved: u32, output: &str, pretty: bool) -> Result<String, String> {
    match output {
        "event" => to_json(event, pretty),
        "relay-message" => to_json(&("EVENT", event), pretty),
        "report" => {
            let id_bytes = hex::decode(&event.id).map_err(|e| e.to_string())?;
            let pk_bytes = hex::decode(&event.pubkey).map_err(|e| e.to_string())?;
            let mut s = String::new();
            s.push_str(&format!("id: {}\n", event.id));
            s.push_str(&format!("note: {}\n", bech32_encode("note", &id_bytes)));
            s.push_str(&format!("pubkey: {}\n", event.pubkey));
            s.push_str(&format!("npub: {}\n", bech32_encode("npub", &pk_bytes)));
            s.push_str(&format!(
                "created_at: {} ({})\n",
                event.created_at,
                format_utc(event.created_at)
            ));
            s.push_str(&format!("kind: {} ({})\n", event.kind, kind_label(event.kind)));
            s.push_str(&format!("tags: {}\n", event.tags.len()));
            s.push_str(&format!("content: {} byte(s)\n", event.content.len()));
            s.push_str(&format!("pow: {achieved} leading zero bit(s)\n"));
            s.push_str(&format!("sig: {}\n", event.sig));
            s.push_str("signature check: valid\n\n");
            s.push_str(&to_json(event, pretty)?);
            s.push('\n');
            Ok(s)
        }
        other => Err(format!(
            "invalid output {other:?}: expected event, relay-message, or report"
        )),
    }
}

/// A short human label for a kind number, using the NIP-01 ranges.
fn kind_label(kind: u32) -> &'static str {
    match kind {
        0 => "profile metadata",
        1 => "text note",
        3 => "follow list",
        4 => "encrypted direct message",
        5 => "deletion request",
        6 => "repost",
        7 => "reaction",
        1059 => "gift wrap",
        30023 => "long-form article",
        0..=2 | 4..=44 | 1000..=9999 => "regular",
        10000..=19999 => "replaceable",
        20000..=29999 => "ephemeral",
        30000..=39999 => "addressable",
        _ => "unclassified",
    }
}

/// Full entry point: build, sign, and render. `now_unix` is supplied by each
/// surface (the block/CLI read the system clock, the page reads `Date.now()`),
/// which keeps the core deterministic and testable.
#[allow(clippy::too_many_arguments)]
pub fn sign_event(
    nsec: &str,
    content: &str,
    kind: f64,
    tags: &str,
    created_at: f64,
    template: &str,
    pow: f64,
    output: &str,
    pretty: bool,
    now_unix: i64,
) -> Result<String, String> {
    if !matches!(output, "event" | "relay-message" | "report") {
        return Err(format!(
            "invalid output {output:?}: expected event, relay-message, or report"
        ));
    }
    let (event, achieved) =
        build_signed_event(nsec, content, kind, tags, created_at, template, pow, now_unix)?;
    render(&event, achieved, output, pretty)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BIP-340 test-vector key: secret 3, whose x-only public key is well known.
    const SK_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000003";
    /// A disposable throwaway key used across the tests below.
    const SK_NSEC: &str = "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5";

    fn sign(content: &str, kind: f64, tags: &str) -> SignedEvent {
        build_signed_event(SK_HEX, content, kind, tags, 1_700_000_000.0, "", 0.0, 0)
            .unwrap()
            .0
    }

    #[test]
    fn signs_a_text_note_with_a_stable_id_and_pubkey() {
        let ev = sign("hello nostr", 1.0, "");
        // BIP-340: the x-only pubkey for secret key 3.
        assert_eq!(
            ev.pubkey,
            "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
        );
        // The id is a pure function of the NIP-01 serialization.
        let expect_serialized = format!(
            r#"[0,"{}",1700000000,1,[],"hello nostr"]"#,
            ev.pubkey
        );
        assert_eq!(
            ev.id,
            hex::encode(event_id(&expect_serialized)),
            "id must be sha256 of the NIP-01 array"
        );
        assert_eq!(ev.created_at, 1_700_000_000);
        assert_eq!(ev.kind, 1);
        assert_eq!(ev.sig.len(), 128);
        assert!(ev.tags.is_empty());
    }

    #[test]
    fn signature_verifies_against_the_public_key() {
        let ev = sign("verify me", 1.0, "");
        let vk = k256::schnorr::VerifyingKey::from_bytes(&hex::decode(&ev.pubkey).unwrap()).unwrap();
        let sig = Signature::try_from(hex::decode(&ev.sig).unwrap().as_slice()).unwrap();
        let id = hex::decode(&ev.id).unwrap();
        assert!(vk.verify_prehash(&id, &sig).is_ok());
    }

    #[test]
    fn accepts_an_nsec_and_a_hex_key_interchangeably() {
        let a = build_signed_event(SK_NSEC, "same", 1.0, "", 1.0, "", 0.0, 0)
            .unwrap()
            .0;
        let hexkey = hex::encode(parse_secret_key(SK_NSEC).unwrap());
        let b = build_signed_event(&hexkey, "same", 1.0, "", 1.0, "", 0.0, 0)
            .unwrap()
            .0;
        assert_eq!(a.pubkey, b.pubkey);
        assert_eq!(a.id, b.id);
        assert_eq!(a.sig, b.sig, "signing is deterministic (zero aux randomness)");
    }

    #[test]
    fn parses_shorthand_and_json_tags_identically() {
        let short = sign("x", 1.0, "e=abc;wss://relay.example.com;root\np=def");
        let json = sign(
            "x",
            1.0,
            r#"[["e","abc","wss://relay.example.com","root"],["p","def"]]"#,
        );
        assert_eq!(short.tags, json.tags);
        assert_eq!(
            short.tags,
            vec![
                vec!["e", "abc", "wss://relay.example.com", "root"],
                vec!["p", "def"]
            ]
        );
        assert_eq!(short.id, json.id);
    }

    #[test]
    fn comma_separated_shorthand_tags_work_for_deep_links() {
        let ev = sign("x", 1.0, "t=nostr,t=rust");
        assert_eq!(ev.tags, vec![vec!["t", "nostr"], vec!["t", "rust"]]);
    }

    #[test]
    fn bare_tag_name_without_a_value_is_allowed() {
        let ev = sign("x", 1.0, "-");
        assert_eq!(ev.tags, vec![vec!["-"]]);
    }

    #[test]
    fn template_overrides_the_individual_fields() {
        let (ev, _) = build_signed_event(
            SK_HEX,
            "ignored",
            1.0,
            "",
            1_700_000_000.0,
            r#"{"kind":30023,"content":"from template","tags":[["d","slug"]],"created_at":1234567890,"id":"junk","sig":"junk"}"#,
            0.0,
            0,
        )
        .unwrap();
        assert_eq!(ev.kind, 30023);
        assert_eq!(ev.content, "from template");
        assert_eq!(ev.tags, vec![vec!["d", "slug"]]);
        assert_eq!(ev.created_at, 1_234_567_890);
        assert_ne!(ev.id, "junk");
    }

    #[test]
    fn created_at_zero_uses_the_supplied_clock() {
        let (ev, _) =
            build_signed_event(SK_HEX, "now", 1.0, "", 0.0, "", 0.0, 1_800_000_000).unwrap();
        assert_eq!(ev.created_at, 1_800_000_000);
    }

    #[test]
    fn proof_of_work_mines_a_nonce_tag_to_the_target() {
        let (ev, achieved) =
            build_signed_event(SK_HEX, "pow", 1.0, "", 1_700_000_000.0, "", 12.0, 0).unwrap();
        assert!(achieved >= 12, "achieved {achieved} bits");
        let nonce = ev.tags.iter().find(|t| t[0] == "nonce").expect("nonce tag");
        assert_eq!(nonce[2], "12");
        let id = hex::decode(&ev.id).unwrap();
        assert_eq!(id[0], 0, "12 leading zero bits means the first byte is zero");
    }

    #[test]
    fn relay_message_output_is_an_event_envelope() {
        let out = sign_event(
            SK_HEX,
            "hi",
            1.0,
            "",
            1_700_000_000.0,
            "",
            0.0,
            "relay-message",
            false,
            0,
        )
        .unwrap();
        assert!(out.starts_with(r#"["EVENT",{"id":"#), "got {out}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0], "EVENT");
        assert_eq!(v[1]["kind"], 1);
    }

    #[test]
    fn report_output_carries_nip19_forms_and_a_verification_line() {
        let out = sign_event(
            SK_HEX,
            "hi",
            1.0,
            "",
            1_700_000_000.0,
            "",
            0.0,
            "report",
            true,
            0,
        )
        .unwrap();
        assert!(out.contains("\nnote: note1"), "got {out}");
        assert!(out.contains("\nnpub: npub1"), "got {out}");
        assert!(out.contains("created_at: 1700000000 (2023-11-14T22:13:20Z)"), "got {out}");
        assert!(out.contains("kind: 1 (text note)"), "got {out}");
        assert!(out.contains("signature check: valid"), "got {out}");
    }

    #[test]
    fn pretty_false_emits_compact_json() {
        let compact =
            sign_event(SK_HEX, "hi", 1.0, "", 1.0, "", 0.0, "event", false, 0).unwrap();
        let pretty = sign_event(SK_HEX, "hi", 1.0, "", 1.0, "", 0.0, "event", true, 0).unwrap();
        assert!(!compact.contains('\n'));
        assert!(pretty.contains("\n  \"id\":"));
    }

    #[test]
    fn emitted_event_keys_are_in_canonical_wire_order() {
        let out = sign_event(SK_HEX, "hi", 1.0, "", 1.0, "", 0.0, "event", false, 0).unwrap();
        let keys: Vec<&str> = ["id", "pubkey", "created_at", "kind", "tags", "content", "sig"]
            .into_iter()
            .collect();
        let mut at = 0usize;
        for k in keys {
            let needle = format!("\"{k}\":");
            let pos = out[at..].find(&needle).unwrap_or_else(|| panic!("missing {k} after {at}"));
            at += pos;
        }
    }

    #[test]
    fn utc_formatting_handles_epoch_and_a_leap_day() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    // ---- error paths ----

    #[test]
    fn rejects_an_npub_as_the_signing_key() {
        let err = build_signed_event(
            "npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg",
            "x",
            1.0,
            "",
            1.0,
            "",
            0.0,
            0,
        )
        .unwrap_err();
        assert!(err.contains("public identifier"), "got {err}");
    }

    #[test]
    fn rejects_a_bad_bech32_checksum() {
        let mut bad = SK_NSEC.to_string();
        bad.pop();
        bad.push('q');
        let err = build_signed_event(&bad, "x", 1.0, "", 1.0, "", 0.0, 0).unwrap_err();
        assert!(err.contains("checksum"), "got {err}");
    }

    #[test]
    fn rejects_a_short_hex_key() {
        let err = build_signed_event("abc123", "x", 1.0, "", 1.0, "", 0.0, 0).unwrap_err();
        assert!(err.contains("64 hex characters"), "got {err}");
    }

    #[test]
    fn rejects_an_all_zero_key() {
        let err = build_signed_event(&"0".repeat(64), "x", 1.0, "", 1.0, "", 0.0, 0).unwrap_err();
        assert!(err.contains("invalid secp256k1 private key"), "got {err}");
    }

    #[test]
    fn rejects_an_out_of_range_kind() {
        let err = build_signed_event(SK_HEX, "x", 70000.0, "", 1.0, "", 0.0, 0).unwrap_err();
        assert!(err.contains("0 to 65535"), "got {err}");
    }

    #[test]
    fn rejects_non_string_values_inside_json_tags() {
        let err =
            build_signed_event(SK_HEX, "x", 1.0, r#"[["e",1]]"#, 1.0, "", 0.0, 0).unwrap_err();
        assert!(err.contains("non-string value"), "got {err}");
    }

    #[test]
    fn rejects_a_pow_target_above_the_cap() {
        let err = build_signed_event(SK_HEX, "x", 1.0, "", 1.0, "", 32.0, 0).unwrap_err();
        assert!(err.contains("20-bit limit"), "got {err}");
    }

    #[test]
    fn rejects_an_unknown_output_mode() {
        let err = sign_event(SK_HEX, "x", 1.0, "", 1.0, "", 0.0, "yaml", true, 0).unwrap_err();
        assert!(err.contains("expected event, relay-message, or report"), "got {err}");
    }

    #[test]
    fn rejects_a_non_object_template() {
        let err = build_signed_event(SK_HEX, "x", 1.0, "", 1.0, "[1,2]", 0.0, 0).unwrap_err();
        assert!(err.contains("must be a JSON object"), "got {err}");
    }
}
