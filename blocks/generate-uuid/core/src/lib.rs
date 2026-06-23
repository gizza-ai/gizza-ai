//! gizza-ai/generate-uuid core — generate RFC 4122 / RFC 9562 UUIDs in
//! version 4 (random), 7 (unix-time-ordered), 1 (gregorian-time + node), and
//! 5/3 (namespace, SHA-1/MD5) forms, singly or in bulk. No wafer/wasm-bindgen
//! deps; pure compute, wasm32-safe.
//!
//! Randomness is via `getrandom` (WASI `random_get` on wasm32-wasip1 — it
//! instantiates in the wafer runtime; the page's wasm32-unknown-unknown build
//! uses getrandom's `js` backend). Time-based versions (v1, v7) take an explicit
//! millisecond timestamp so each surface supplies its own clock and the core is
//! deterministic given its inputs (the random bits aside). Namespace versions
//! (v3, v5) are fully deterministic functions of (namespace, name).

use md5::Md5;
use sha1::{Digest, Sha1};

/// Predefined RFC 4122 namespace UUIDs (Appendix C).
const NS_DNS: [u8; 16] = [
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
];
const NS_URL: [u8; 16] = [
    0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
];
const NS_OID: [u8; 16] = [
    0x6b, 0xa7, 0xb8, 0x12, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
];
const NS_X500: [u8; 16] = [
    0x6b, 0xa7, 0xb8, 0x14, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
];

/// Fill `buf` with cryptographic random bytes.
fn fill_random(buf: &mut [u8]) -> Result<(), String> {
    getrandom::getrandom(buf).map_err(|e| format!("randomness unavailable: {e}"))
}

/// Set the RFC 4122 version (high nibble of byte 6) and variant (top two bits of
/// byte 8 = 0b10), in place.
fn stamp(bytes: &mut [u8; 16], version: u8) {
    bytes[6] = (bytes[6] & 0x0f) | (version << 4);
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
}

/// Format 16 bytes as the canonical 8-4-4-4-12 hyphenated lowercase string.
fn format_uuid(b: &[u8; 16]) -> String {
    let mut s = String::with_capacity(36);
    for (i, byte) in b.iter().enumerate() {
        if i == 4 || i == 6 || i == 8 || i == 10 {
            s.push('-');
        }
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

/// Apply formatting options (uppercase, hyphens, braces, urn) to a canonical
/// hyphenated lowercase UUID string.
fn apply_format(canonical: &str, uppercase: bool, hyphens: bool, braces: bool, urn: bool) -> String {
    if urn {
        // RFC 4122 urn form is always hyphenated, lowercase by convention.
        return format!("urn:uuid:{canonical}");
    }
    let mut s = if hyphens {
        canonical.to_string()
    } else {
        canonical.replace('-', "")
    };
    if uppercase {
        s = s.to_uppercase();
    }
    if braces {
        s = format!("{{{s}}}");
    }
    s
}

/// nil UUID, all zeroes.
fn nil() -> [u8; 16] {
    [0u8; 16]
}

/// max UUID, all ones (RFC 9562 §5.10).
fn max() -> [u8; 16] {
    [0xffu8; 16]
}

/// version 4 — 122 random bits.
fn v4() -> Result<[u8; 16], String> {
    let mut b = [0u8; 16];
    fill_random(&mut b)?;
    stamp(&mut b, 4);
    Ok(b)
}

/// version 7 — 48-bit unix-millisecond timestamp, then random, monotonic-able.
/// `ms` is the unix epoch in milliseconds. Bytes 6-15 (rand_a + rand_b) random.
fn v7(ms: u64) -> Result<[u8; 16], String> {
    let mut b = [0u8; 16];
    let t = ms & 0xffff_ffff_ffff; // 48 bits
    b[0] = (t >> 40) as u8;
    b[1] = (t >> 32) as u8;
    b[2] = (t >> 24) as u8;
    b[3] = (t >> 16) as u8;
    b[4] = (t >> 8) as u8;
    b[5] = t as u8;
    let mut rand = [0u8; 10];
    fill_random(&mut rand)?;
    b[6..16].copy_from_slice(&rand);
    stamp(&mut b, 7);
    Ok(b)
}

/// version 1 — gregorian 100-ns timestamp + clock seq + node.
/// `ms` is unix epoch ms (converted to the gregorian epoch 1582-10-15).
/// clock_seq + node are randomized (node has the multicast bit set per RFC, so
/// it can't collide with a real MAC).
fn v1(ms: u64) -> Result<[u8; 16], String> {
    // 100-ns intervals since 1582-10-15 00:00:00 UTC. Offset = 122192928000000000.
    const GREG_OFFSET: u64 = 122_192_928_000_000_000;
    let ts = ms.saturating_mul(10_000).wrapping_add(GREG_OFFSET);
    let time_low = (ts & 0xffff_ffff) as u32;
    let time_mid = ((ts >> 32) & 0xffff) as u16;
    let time_hi = ((ts >> 48) & 0x0fff) as u16;

    let mut rand = [0u8; 8]; // 2 for clock_seq, 6 for node
    fill_random(&mut rand)?;
    let clock_seq = u16::from_be_bytes([rand[0], rand[1]]) & 0x3fff;

    let mut b = [0u8; 16];
    b[0..4].copy_from_slice(&time_low.to_be_bytes());
    b[4..6].copy_from_slice(&time_mid.to_be_bytes());
    b[6..8].copy_from_slice(&(time_hi | (1 << 12)).to_be_bytes()); // version 1
    b[8] = ((clock_seq >> 8) as u8 & 0x3f) | 0x80; // variant
    b[9] = clock_seq as u8;
    // node: 6 random bytes, set the multicast bit (RFC 4122 §4.5) to avoid MACs.
    b[10] = rand[2] | 0x01;
    b[11..16].copy_from_slice(&rand[3..8]);
    Ok(b)
}

/// Resolve a namespace string to its 16 raw bytes. Accepts the predefined
/// aliases dns/url/oid/x500 (case-insensitive) or any canonical UUID string.
fn resolve_namespace(ns: &str) -> Result<[u8; 16], String> {
    match ns.trim().to_ascii_lowercase().as_str() {
        "dns" => Ok(NS_DNS),
        "url" => Ok(NS_URL),
        "oid" => Ok(NS_OID),
        "x500" | "x.500" => Ok(NS_X500),
        _ => parse_uuid(ns)
            .ok_or_else(|| format!("namespace must be dns, url, oid, x500, or a UUID — got {ns:?}")),
    }
}

/// Parse a canonical (with or without hyphens/braces/urn) UUID into 16 bytes.
pub fn parse_uuid(s: &str) -> Option<[u8; 16]> {
    let mut hex = s.trim().to_string();
    if let Some(rest) = hex.strip_prefix("urn:uuid:") {
        hex = rest.to_string();
    }
    hex = hex.trim_matches(|c| c == '{' || c == '}').to_string();
    hex.retain(|c| c != '-');
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// version 5 — SHA-1(namespace || name), truncated to 16 bytes.
fn v5(ns: &[u8; 16], name: &str) -> [u8; 16] {
    let mut h = Sha1::new();
    h.update(ns);
    h.update(name.as_bytes());
    let digest = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest[..16]);
    stamp(&mut b, 5);
    b
}

/// version 3 — MD5(namespace || name).
fn v3(ns: &[u8; 16], name: &str) -> [u8; 16] {
    let mut h = Md5::new();
    h.update(ns);
    h.update(name.as_bytes());
    let digest = h.finalize();
    let mut b = [0u8; 16];
    b.copy_from_slice(&digest[..16]);
    stamp(&mut b, 3);
    b
}

/// Output of a generation request.
pub struct GenResult {
    pub uuids: Vec<String>,
    pub version: String,
}

/// Generate `count` UUIDs of `version`.
///
/// - `version`: "4" (random, default), "7" (time-ordered), "1" (time + node),
///   "5" (namespace, SHA-1), "3" (namespace, MD5), "nil", or "max".
/// - `count`: how many to produce (1..=1000).
/// - `now_ms`: unix epoch in milliseconds, used by v1/v7 (supplied by the
///   caller's clock). For v7/v1 each successive UUID in a batch is given an
///   incrementing timestamp so the batch stays sortable/monotonic.
/// - `namespace`/`name`: required for v3/v5; `namespace` is dns/url/oid/x500 or
///   a UUID string. Each name in a comma/newline-separated `name` list yields
///   one UUID for namespace versions (overriding `count`).
/// - format flags shape the output string.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    version: &str,
    count: usize,
    now_ms: u64,
    namespace: &str,
    name: &str,
    uppercase: bool,
    hyphens: bool,
    braces: bool,
    urn: bool,
) -> Result<GenResult, String> {
    let v = version.trim().to_ascii_lowercase();
    let v = v.strip_prefix('v').unwrap_or(&v).to_string();
    if !(1..=1000).contains(&count) {
        return Err(format!("count must be 1..=1000, got {count}"));
    }

    let fmt = |b: &[u8; 16]| apply_format(&format_uuid(b), uppercase, hyphens, braces, urn);

    let mut uuids = Vec::new();
    match v.as_str() {
        "4" | "" => {
            for _ in 0..count {
                uuids.push(fmt(&v4()?));
            }
        }
        "7" => {
            for i in 0..count {
                // increment the timestamp per item so a batch is sortable.
                uuids.push(fmt(&v7(now_ms.wrapping_add(i as u64))?));
            }
        }
        "1" => {
            for i in 0..count {
                uuids.push(fmt(&v1(now_ms.wrapping_add(i as u64))?));
            }
        }
        "5" | "3" => {
            let ns = resolve_namespace(namespace)?;
            // Split names on comma/newline; blank -> a single empty name.
            let names: Vec<&str> = name
                .split(['\n', ','])
                .map(|n| n.trim())
                .filter(|n| !n.is_empty())
                .collect();
            let names = if names.is_empty() { vec![""] } else { names };
            for nm in names {
                let b = if v == "5" { v5(&ns, nm) } else { v3(&ns, nm) };
                uuids.push(fmt(&b));
            }
        }
        "nil" | "0" => uuids.push(fmt(&nil())),
        "max" => uuids.push(fmt(&max())),
        other => {
            return Err(format!(
                "unknown version {other:?} (use 4, 7, 1, 5, 3, nil, or max)"
            ))
        }
    }

    Ok(GenResult {
        uuids,
        version: format!("v{v}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(v: &str) -> String {
        generate(v, 1, 1_700_000_000_000, "", "", false, true, false, false)
            .unwrap()
            .uuids
            .remove(0)
    }

    #[test]
    fn v4_has_correct_version_and_variant() {
        let s = one("4");
        let b = parse_uuid(&s).unwrap();
        assert_eq!(b[6] >> 4, 4, "version nibble");
        assert_eq!(b[8] >> 6, 0b10, "variant bits");
        assert_eq!(s.len(), 36);
    }

    #[test]
    fn v4_is_random() {
        assert_ne!(one("4"), one("4"));
    }

    #[test]
    fn v7_is_time_ordered_and_versioned() {
        let r = generate("7", 5, 1_700_000_000_000, "", "", false, true, false, false).unwrap();
        // Lexicographic order should match generation order (monotonic ts).
        let mut sorted = r.uuids.clone();
        sorted.sort();
        assert_eq!(sorted, r.uuids, "v7 batch must be lexicographically sortable");
        let b = parse_uuid(&r.uuids[0]).unwrap();
        assert_eq!(b[6] >> 4, 7);
        assert_eq!(b[8] >> 6, 0b10);
    }

    #[test]
    fn v1_versioned_with_multicast_node() {
        let b = parse_uuid(&one("1")).unwrap();
        assert_eq!(b[6] >> 4, 1);
        assert_eq!(b[8] >> 6, 0b10);
        assert_eq!(b[10] & 0x01, 0x01, "node multicast bit set");
    }

    #[test]
    fn v5_dns_matches_known_vector() {
        // uuid5(NAMESPACE_DNS, "www.example.com")
        let r = generate("5", 1, 0, "dns", "www.example.com", false, true, false, false).unwrap();
        assert_eq!(r.uuids[0], "2ed6657d-e927-568b-95e1-2665a8aea6a2");
    }

    #[test]
    fn v3_dns_matches_known_vector() {
        // uuid3(NAMESPACE_DNS, "www.example.com")
        let r = generate("3", 1, 0, "dns", "www.example.com", false, true, false, false).unwrap();
        assert_eq!(r.uuids[0], "5df41881-3aed-3515-88a7-2f4a814cf09e");
    }

    #[test]
    fn namespace_versions_are_deterministic() {
        let a = generate("5", 1, 0, "url", "https://gizza.ai", false, true, false, false).unwrap();
        let b = generate("5", 1, 0, "url", "https://gizza.ai", false, true, false, false).unwrap();
        assert_eq!(a.uuids, b.uuids);
    }

    #[test]
    fn namespace_accepts_uuid_string() {
        let r = generate(
            "5",
            1,
            0,
            "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "www.example.com",
            false,
            true,
            false,
            false,
        )
        .unwrap();
        // Same as the DNS alias.
        assert_eq!(r.uuids[0], "2ed6657d-e927-568b-95e1-2665a8aea6a2");
    }

    #[test]
    fn v5_name_list_yields_one_each() {
        let r = generate("5", 1, 0, "dns", "a,b,c", false, true, false, false).unwrap();
        assert_eq!(r.uuids.len(), 3);
        assert_ne!(r.uuids[0], r.uuids[1]);
    }

    #[test]
    fn count_produces_batch() {
        let r = generate("4", 10, 0, "", "", false, true, false, false).unwrap();
        assert_eq!(r.uuids.len(), 10);
    }

    #[test]
    fn format_options() {
        // no hyphens
        let nh = generate("4", 1, 0, "", "", false, false, false, false).unwrap().uuids.remove(0);
        assert!(!nh.contains('-'));
        assert_eq!(nh.len(), 32);
        // uppercase
        let up = generate("4", 1, 0, "", "", true, true, false, false).unwrap().uuids.remove(0);
        assert_eq!(up, up.to_uppercase());
        // braces
        let br = generate("4", 1, 0, "", "", false, true, true, false).unwrap().uuids.remove(0);
        assert!(br.starts_with('{') && br.ends_with('}'));
        // urn
        let urn = generate("4", 1, 0, "", "", false, true, false, true).unwrap().uuids.remove(0);
        assert!(urn.starts_with("urn:uuid:"));
    }

    #[test]
    fn nil_and_max() {
        assert_eq!(one("nil"), "00000000-0000-0000-0000-000000000000");
        assert_eq!(one("max"), "ffffffff-ffff-ffff-ffff-ffffffffffff");
    }

    #[test]
    fn rejects_bad_version_and_count() {
        assert!(generate("9", 1, 0, "", "", false, true, false, false).is_err());
        assert!(generate("4", 0, 0, "", "", false, true, false, false).is_err());
        assert!(generate("4", 1001, 0, "", "", false, true, false, false).is_err());
    }

    #[test]
    fn rejects_bad_namespace() {
        assert!(generate("5", 1, 0, "not-a-namespace", "x", false, true, false, false).is_err());
    }
}
