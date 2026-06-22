//! hash-identifier core — pure compute, shared by the chat skill block and the web page.
//!
//! Given a single hash/digest string, identify the likely hash or MAC scheme(s)
//! that could have produced it. Recognition is by **structure** alone: prefixed
//! formats (bcrypt `$2*$`, Argon2 `$argon2*$`, sha-crypt `$5$`/`$6$`, PHPass `$P$`,
//! NetNTLM, etc.) are matched by their unambiguous prefix; bare hex/base64 digests
//! are matched by length (and so map to a *family* of same-width algorithms — e.g.
//! a 32-hex string is MD5 **or** NTLM **or** MD4 **or** MD2 — all are reported).
//!
//! No I/O, no deps beyond std. Deterministic.

/// One candidate scheme for an input hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Human name, e.g. "bcrypt", "SHA-256", "MD5".
    pub name: &'static str,
    /// Rough confidence: a prefixed/structured format is High; a bare
    /// fixed-width hex/base64 digest that many algorithms share is Low.
    pub confidence: Confidence,
    /// One-line note (what it's typically used for / how to tell it apart).
    pub note: &'static str,
    /// The Hashcat `-m` mode number for this scheme, when there is a single
    /// well-known one (so the result can feed straight into a crack); `None`
    /// for grouped/ambiguous candidates that don't map to one mode.
    pub hashcat_mode: Option<u32>,
}

/// Build a Candidate. Helper so the long table below stays readable.
const fn cand(
    name: &'static str,
    confidence: Confidence,
    note: &'static str,
    hashcat_mode: Option<u32>,
) -> Candidate {
    Candidate {
        name,
        confidence,
        note,
        hashcat_mode,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl Confidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        }
    }
}

/// Result of identifying one input string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identification {
    /// The normalized input that was analyzed (trimmed).
    pub input: String,
    /// Candidate schemes, best-confidence first.
    pub candidates: Vec<Candidate>,
}

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_lower_hex(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_base64ish(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || b == b'+'
                || b == b'/'
                || b == b'='
                || b == b'-'
                || b == b'_'
        })
}

/// Identify the structured (prefixed) formats. These have an unambiguous shape,
/// so a match here is High/Medium confidence and short-circuits the hex/base64
/// length heuristics. Returns `None` if no prefixed format matches.
fn identify_prefixed(s: &str) -> Option<Vec<Candidate>> {
    // bcrypt: $2$, $2a$, $2b$, $2x$, $2y$ followed by cost + 53 chars salt+hash.
    if s.starts_with("$2a$")
        || s.starts_with("$2b$")
        || s.starts_with("$2y$")
        || s.starts_with("$2x$")
        || s.starts_with("$2$")
    {
        return Some(vec![cand(
            "bcrypt",
            Confidence::High,
            "Blowfish-based password hash (Modular Crypt Format). Format: $2<a|b|x|y>$<cost>$<22-char salt><31-char hash>.",
            Some(3200),
        )]);
    }
    // Argon2 (PHC string): $argon2i$, $argon2d$, $argon2id$
    if s.starts_with("$argon2") {
        let (name, note, mode): (&'static str, &'static str, Option<u32>) =
            if s.starts_with("$argon2id$") {
                ("Argon2id", "Hybrid Argon2 (recommended), PHC string: $argon2id$v=19$m=...,t=...,p=...$<salt>$<hash>.", None)
            } else if s.starts_with("$argon2i$") {
                ("Argon2i", "Argon2i (data-independent), PHC string format.", None)
            } else if s.starts_with("$argon2d$") {
                ("Argon2d", "Argon2d (data-dependent), PHC string format.", None)
            } else {
                ("Argon2", "Argon2 password hash, PHC string format.", None)
            };
        return Some(vec![cand(name, Confidence::High, note, mode)]);
    }
    // scrypt PHC string
    if s.starts_with("$scrypt$") || s.starts_with("$7$") {
        return Some(vec![cand(
            "scrypt",
            Confidence::High,
            "scrypt memory-hard password hash (PHC / crypt format).",
            Some(8900),
        )]);
    }
    // sha512crypt / sha256crypt / md5crypt / sha1crypt
    if s.starts_with("$6$") {
        return Some(vec![cand(
            "sha512crypt ($6$)",
            Confidence::High,
            "SHA-512 crypt — default Linux /etc/shadow password hash. $6$<salt>$<86-char hash>.",
            Some(1800),
        )]);
    }
    if s.starts_with("$5$") {
        return Some(vec![cand(
            "sha256crypt ($5$)",
            Confidence::High,
            "SHA-256 crypt password hash (/etc/shadow). $5$<salt>$<43-char hash>.",
            Some(7400),
        )]);
    }
    if s.starts_with("$1$") {
        return Some(vec![cand(
            "md5crypt ($1$)",
            Confidence::High,
            "MD5-based crypt password hash (legacy Linux/BSD /etc/shadow).",
            Some(500),
        )]);
    }
    if s.starts_with("$sha1$") {
        return Some(vec![cand(
            "sha1crypt ($sha1$)",
            Confidence::High,
            "SHA-1 based crypt password hash (NetBSD).",
            Some(15100),
        )]);
    }
    // PHPass / phpBB3 / WordPress
    if s.starts_with("$P$") || s.starts_with("$H$") {
        return Some(vec![cand(
            "PHPass (WordPress/phpBB)",
            Confidence::High,
            "Portable PHP password hash (WordPress $P$, phpBB3 $H$). 34 chars total.",
            Some(400),
        )]);
    }
    // Drupal 7 ($S$)
    if s.starts_with("$S$") {
        return Some(vec![cand(
            "Drupal 7 ($S$)",
            Confidence::High,
            "Drupal 7 SHA-512-based password hash.",
            Some(7900),
        )]);
    }
    // Cisco IOS type 8 / 9
    if s.starts_with("$8$") {
        return Some(vec![cand(
            "Cisco IOS type 8",
            Confidence::High,
            "Cisco type 8 password (PBKDF2-HMAC-SHA256).",
            Some(9200),
        )]);
    }
    if s.starts_with("$9$") {
        return Some(vec![cand(
            "Cisco IOS type 9",
            Confidence::High,
            "Cisco type 9 password (scrypt).",
            Some(9300),
        )]);
    }
    // Apache apr1 md5
    if s.starts_with("$apr1$") {
        return Some(vec![cand(
            "Apache MD5 (apr1)",
            Confidence::High,
            "Apache-specific MD5 password hash (htpasswd).",
            Some(1600),
        )]);
    }
    // PBKDF2 (Django / passlib style)
    if s.starts_with("pbkdf2_sha256$")
        || s.starts_with("pbkdf2_sha1$")
        || s.starts_with("pbkdf2-sha256$")
        || s.starts_with("$pbkdf2")
    {
        return Some(vec![cand(
            "PBKDF2",
            Confidence::High,
            "PBKDF2 password hash (Django/passlib): <algo>$<iterations>$<salt>$<hash>.",
            Some(10000),
        )]);
    }
    // LDAP / SSHA schemes: {SHA}, {SSHA}, {MD5}, {SSHA512}, {CRYPT}
    if let Some(rest) = s.strip_prefix('{') {
        if let Some(end) = rest.find('}') {
            let up = rest[..end].to_ascii_uppercase();
            let name: &'static str = match up.as_str() {
                "SHA" => "LDAP {SHA} (base64 SHA-1)",
                "SSHA" => "LDAP {SSHA} (salted SHA-1)",
                "MD5" => "LDAP {MD5} (base64 MD5)",
                "SMD5" => "LDAP {SMD5} (salted MD5)",
                "SHA256" => "LDAP {SHA256}",
                "SSHA256" => "LDAP {SSHA256} (salted SHA-256)",
                "SHA512" => "LDAP {SHA512}",
                "SSHA512" => "LDAP {SSHA512} (salted SHA-512)",
                "CRYPT" => "LDAP {CRYPT} (Unix crypt)",
                _ => "",
            };
            if !name.is_empty() {
                return Some(vec![cand(
                    name,
                    Confidence::High,
                    "RFC 2307 LDAP password storage scheme; the digest after '}' is base64.",
                    None,
                )]);
            }
        }
    }
    // NetNTLMv2 / NetNTLMv1: user::domain:...:... (colon-delimited capture).
    // Require at least one long hex field (NTProofStr is 32 hex chars, the blob
    // longer) so a plain IPv6 address (hextets are <=4 hex digits, e.g.
    // `fe80::1:2:3:4`) is not misidentified as a captured NTLM response.
    if s.contains("::")
        && s.matches(':').count() >= 4
        && s.split(':').any(|seg| seg.len() >= 16 && is_hex(seg))
    {
        return Some(vec![cand(
            "NetNTLMv2 / NetNTLMv1",
            Confidence::Medium,
            "Captured NTLM network authentication response (user::domain:challenge:response...). NetNTLMv2 = hashcat -m 5600, NetNTLMv1 = -m 5500.",
            None,
        )]);
    }
    // MySQL 4.1+ tagged: *HEX(40)
    if let Some(rest) = s.strip_prefix('*') {
        if rest.len() == 40 && is_hex(rest) {
            return Some(vec![cand(
                "MySQL 4.1+ (SHA1(SHA1(pw)))",
                Confidence::High,
                "MySQL 4.1+ PASSWORD() hash — a leading '*' then 40 uppercase hex chars.",
                Some(300),
            )]);
        }
    }
    None
}

/// Candidates for a bare hex digest of the given length.
fn hex_candidates(len: usize) -> Vec<Candidate> {
    match len {
        8 => vec![
            cand("CRC-32", Confidence::Low, "32-bit checksum (not cryptographic); also Adler-32, FNV-1/1a 32-bit.", None),
            cand("CRC-32B", Confidence::Low, "Common CRC-32 variant (zlib/PNG).", None),
        ],
        16 => vec![
            cand("CRC-64", Confidence::Low, "64-bit checksum.", None),
            cand("MySQL323", Confidence::Low, "MySQL <4.1 OLD_PASSWORD() hash (16 hex chars).", Some(200)),
        ],
        32 => vec![
            cand("MD5", Confidence::Medium, "128-bit MD5 digest — by far the most common 32-hex hash.", Some(0)),
            cand("NTLM", Confidence::Medium, "Windows NT password hash (MD4 of UTF-16LE password) — same width as MD5.", Some(1000)),
            cand("MD4", Confidence::Low, "128-bit MD4 digest.", Some(900)),
            cand("MD2", Confidence::Low, "128-bit MD2 digest (legacy).", None),
            cand("RIPEMD-128 / Tiger-128 / Snefru-128", Confidence::Low, "Other 128-bit digests with the same 32-hex width.", None),
        ],
        40 => vec![
            cand("SHA-1", Confidence::Medium, "160-bit SHA-1 digest — most common 40-hex hash (also git object IDs).", Some(100)),
            cand("RIPEMD-160", Confidence::Low, "160-bit RIPEMD digest (Bitcoin addresses).", Some(6000)),
            cand("HAS-160 / Tiger-160 / Haval-160", Confidence::Low, "Other 160-bit digests of the same width.", None),
        ],
        56 => vec![
            cand("SHA-224", Confidence::Medium, "224-bit SHA-2 digest.", Some(1300)),
            cand("SHA3-224", Confidence::Low, "224-bit SHA-3 (Keccak) digest.", Some(17300)),
            cand("Haval-224 / Tiger-192-truncated", Confidence::Low, "Other 224-bit digests.", None),
        ],
        64 => vec![
            cand("SHA-256", Confidence::Medium, "256-bit SHA-2 digest — most common 64-hex hash.", Some(1400)),
            cand("SHA3-256", Confidence::Low, "256-bit SHA-3 (Keccak) digest.", Some(17400)),
            cand("BLAKE2s-256", Confidence::Low, "256-bit BLAKE2s digest.", None),
            cand("RIPEMD-256 / GOST / Snefru-256", Confidence::Low, "Other 256-bit digests of the same width.", None),
        ],
        96 => vec![
            cand("SHA-384", Confidence::Medium, "384-bit SHA-2 digest.", Some(10800)),
            cand("SHA3-384", Confidence::Low, "384-bit SHA-3 (Keccak) digest.", Some(17500)),
        ],
        128 => vec![
            cand("SHA-512", Confidence::Medium, "512-bit SHA-2 digest — most common 128-hex hash.", Some(1700)),
            cand("SHA3-512", Confidence::Low, "512-bit SHA-3 (Keccak) digest.", Some(17600)),
            cand("BLAKE2b-512", Confidence::Low, "512-bit BLAKE2b digest.", Some(600)),
            cand("Whirlpool", Confidence::Low, "512-bit Whirlpool digest.", Some(6100)),
        ],
        _ => Vec::new(),
    }
}

/// Identify one trimmed input string. Always returns an `Identification`
/// (with an empty candidate list if nothing matched).
pub fn identify(input: &str) -> Identification {
    let s = input.trim();
    let mut candidates: Vec<Candidate> = Vec::new();

    if !s.is_empty() {
        if let Some(p) = identify_prefixed(s) {
            candidates = p;
        } else if is_hex(s) {
            candidates = hex_candidates(s.len());
            if candidates.is_empty() && is_lower_hex(s) {
                candidates.push(cand(
                    "Unrecognized hex digest",
                    Confidence::Low,
                    "A hex string that doesn't match a known fixed-width digest.",
                    None,
                ));
            }
        } else if is_base64ish(s) {
            let stripped = s.trim_end_matches('=');
            let approx_bytes = stripped.len() * 6 / 8;
            let mapped: Option<(&'static str, &'static str)> = match approx_bytes {
                16 => Some(("Base64 MD5 / 128-bit digest", "~16 decoded bytes — a base64-encoded 128-bit digest (e.g. MD5).")),
                20 => Some(("Base64 SHA-1 / 160-bit digest", "~20 decoded bytes — a base64-encoded 160-bit digest (e.g. SHA-1, LDAP {SHA}).")),
                28 => Some(("Base64 SHA-224", "~28 decoded bytes — a base64-encoded 224-bit digest.")),
                32 => Some(("Base64 SHA-256 / 256-bit digest", "~32 decoded bytes — a base64-encoded 256-bit digest (e.g. SHA-256).")),
                48 => Some(("Base64 SHA-384", "~48 decoded bytes — a base64-encoded 384-bit digest.")),
                64 => Some(("Base64 SHA-512 / 512-bit digest", "~64 decoded bytes — a base64-encoded 512-bit digest (e.g. SHA-512).")),
                _ => None,
            };
            if let Some((name, n)) = mapped {
                candidates.push(cand(name, Confidence::Low, n, None));
            }
        }
    }

    Identification {
        input: s.to_string(),
        candidates,
    }
}

/// Render an `Identification` as a human-readable plain-text report (used by the
/// page and as the chat/CLI message). Returns an error string for empty input.
pub fn run(input: &str) -> Result<String, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("input is empty: provide a hash string to identify".into());
    }
    let id = identify(s);
    if id.candidates.is_empty() {
        return Ok(format!(
            "No known hash scheme matched (length {} chars). It may be salted, truncated, encrypted ciphertext, or an unsupported format.",
            s.chars().count()
        ));
    }
    let mut out = format!(
        "Possible hash schemes for the {}-char input:\n",
        s.chars().count()
    );
    for c in &id.candidates {
        let mode = match c.hashcat_mode {
            Some(m) => format!(" (hashcat -m {m})"),
            None => String::new(),
        };
        out.push_str(&format!(
            "- {} [{}]{}: {}\n",
            c.name,
            c.confidence.as_str(),
            mode,
            c.note
        ));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_bcrypt() {
        let id = identify("$2b$12$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW");
        assert_eq!(id.candidates[0].name, "bcrypt");
        assert_eq!(id.candidates[0].confidence, Confidence::High);
    }

    #[test]
    fn identifies_argon2id() {
        let id = identify("$argon2id$v=19$m=65536,t=3,p=4$c29tZXNhbHQ$RdescudvJCsgt3ub+b+dWRWJTmaaJObG");
        assert_eq!(id.candidates[0].name, "Argon2id");
    }

    #[test]
    fn identifies_sha512crypt() {
        let id = identify("$6$rounds=5000$salt$hashhashhash");
        assert_eq!(id.candidates[0].name, "sha512crypt ($6$)");
    }

    #[test]
    fn md5_reports_md5_and_ntlm() {
        let id = identify("5f4dcc3b5aa765d61d8327deb882cf99");
        let names: Vec<_> = id.candidates.iter().map(|c| c.name).collect();
        assert!(names.contains(&"MD5"));
        assert!(names.contains(&"NTLM"));
    }

    #[test]
    fn sha1_is_top_for_40_hex() {
        let id = identify("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
        assert_eq!(id.candidates[0].name, "SHA-1");
    }

    #[test]
    fn sha256_is_top_for_64_hex() {
        let id =
            identify("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08");
        assert_eq!(id.candidates[0].name, "SHA-256");
    }

    #[test]
    fn mysql_tagged_star_hash() {
        let id = identify("*2470C0C06DEE42FD1618BB99005ADCA2EC9D1E19");
        assert_eq!(id.candidates[0].name, "MySQL 4.1+ (SHA1(SHA1(pw)))");
    }

    #[test]
    fn ldap_ssha() {
        let id = identify("{SSHA}HrYQK7KSqL2j8I7Yc1234567890abcdEF");
        assert_eq!(id.candidates[0].name, "LDAP {SSHA} (salted SHA-1)");
    }

    #[test]
    fn empty_input_is_error() {
        assert!(run("   ").is_err());
    }

    #[test]
    fn unknown_returns_no_match_message() {
        let out = run("not_a_hash!!!").unwrap();
        assert!(out.contains("No known hash scheme matched"));
    }

    #[test]
    fn run_trims_and_reports() {
        let out = run("  5f4dcc3b5aa765d61d8327deb882cf99  ").unwrap();
        assert!(out.contains("MD5"));
    }

    #[test]
    fn crc32_for_8_hex() {
        let id = identify("a9b7ba70");
        assert_eq!(id.candidates[0].name, "CRC-32");
    }

    #[test]
    fn reports_hashcat_mode() {
        // MD5 -> hashcat -m 0; bcrypt -> -m 3200.
        let md5 = run("5f4dcc3b5aa765d61d8327deb882cf99").unwrap();
        assert!(md5.contains("hashcat -m 0"), "{md5}");
        let bc = run("$2b$12$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW").unwrap();
        assert!(bc.contains("hashcat -m 3200"), "{bc}");
    }

    #[test]
    fn argon2_has_no_hashcat_mode() {
        let id = identify("$argon2id$v=19$m=65536,t=3,p=4$c29tZXNhbHQ$RdescudvJCsgt3ub+b+dWRWJTmaaJObG");
        assert_eq!(id.candidates[0].hashcat_mode, None);
    }

    #[test]
    fn netntlm_detected_but_ipv6_is_not() {
        // A genuine NetNTLMv2 capture (long hex NTProofStr + blob) is still detected.
        let nt = identify("admin::CORP:1122334455667788:00112233445566778899aabbccddeeff:0101000000000000000000000000000000000000");
        assert_eq!(nt.candidates[0].name, "NetNTLMv2 / NetNTLMv1");
        // A plain IPv6 address (only short hextets) must NOT be misidentified.
        let ip = identify("fe80::1:2:3:4");
        assert!(
            !ip.candidates.iter().any(|c| c.name.starts_with("NetNTLM")),
            "IPv6 misidentified as NetNTLM: {:?}",
            ip.candidates.iter().map(|c| c.name).collect::<Vec<_>>()
        );
    }
}
