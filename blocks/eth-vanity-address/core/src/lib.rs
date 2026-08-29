//! eth-vanity-address core — search for a secp256k1 private key whose Ethereum
//! address matches a chosen hex prefix and/or suffix.
//!
//! The walk is a plain incremental scan: a starting scalar `k0` is turned into
//! the curve point `k0·G`, and every following candidate is `k += 1` /
//! `P += G`, which costs one point addition instead of a fresh scalar
//! multiplication. Each candidate address is the last 20 bytes of
//! Keccak-256 over the uncompressed public key without its `0x04` tag; EIP-55
//! checksum casing is derived from the lowercase hex address.
//!
//! `k0` comes either from the operating system's CSPRNG (empty seed) or
//! deterministically from a caller-supplied seed string, so the same seed plus
//! the same pattern always reproduces the same key — that is what makes the
//! tool testable and shareable, and it is also why a guessable seed must never
//! hold real funds.

use k256::elliptic_curve::ops::Reduce;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::elliptic_curve::PrimeField;
use k256::{FieldBytes, ProjectivePoint, Scalar, U256};
use sha3::{Digest, Keccak256};

/// Hard ceiling on `max_attempts`. The scan is single-threaded and every
/// candidate costs a curve point addition, an affine normalization and a
/// Keccak-256 digest, so an unbounded loop would simply hang a surface.
pub const MAX_ATTEMPTS_CAP: u64 = 5_000_000;

/// Default number of candidates scanned when the caller does not say.
pub const DEFAULT_MAX_ATTEMPTS: u64 = 100_000;

/// Domain separator mixed into seeded start keys so the same seed text used
/// with some other tool does not derive the same private key here.
const SEED_DOMAIN: &[u8] = b"gizza-ai/eth-vanity-address:v1:";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    All,
    Address,
    PrivateKey,
    Json,
    Estimate,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Ok(Self::All),
            "address" => Ok(Self::Address),
            "private-key" | "private_key" | "privatekey" | "key" => Ok(Self::PrivateKey),
            "json" => Ok(Self::Json),
            "estimate" => Ok(Self::Estimate),
            other => Err(format!(
                "unknown output_format '{other}' (use all, address, private-key, json, or estimate)"
            )),
        }
    }
}

/// Everything a caller needs to know about a completed search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VanityMatch {
    /// EIP-55 checksummed `0x…` address.
    pub address: String,
    /// All-lowercase `0x…` address.
    pub address_lowercase: String,
    /// 32-byte private key as `0x…` hex.
    pub private_key: String,
    /// 65-byte uncompressed SEC1 public key as `0x04…` hex.
    pub public_key: String,
    /// 1-based index of the candidate that matched.
    pub attempts: u64,
}

/// Difficulty of a pattern, independent of any search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Difficulty {
    /// Number of fixed hex positions in the pattern.
    pub pattern_length: usize,
    /// Expected number of candidates per hit (1 / probability).
    pub expected_attempts: f64,
    /// Candidates needed for a 50% chance of at least one hit.
    pub fifty_percent_attempts: f64,
}

/// Derive a deterministic 32-byte starting key from a seed string.
pub fn start_key_from_seed(seed: &str) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(SEED_DOMAIN);
    hasher.update(seed.as_bytes());
    hasher.finalize().into()
}

/// Draw a 32-byte starting key from the platform CSPRNG (`crypto.getRandomValues`
/// in the browser, `random_get` under wasi, `getrandom(2)` natively).
pub fn random_start_key() -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key)
        .map_err(|e| format!("no secure random source available on this platform: {e}"))?;
    Ok(key)
}

/// Pick the starting key for a run: a blank seed means "use the CSPRNG".
pub fn resolve_start_key(seed: &str) -> Result<[u8; 32], String> {
    if seed.trim().is_empty() {
        random_start_key()
    } else {
        Ok(start_key_from_seed(seed.trim()))
    }
}

fn clean_pattern(raw: &str, field: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let body = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if let Some(bad) = body.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(format!(
            "{field} must use hex characters 0-9 and a-f only, but contains '{bad}' \
             (an Ethereum address is 40 hex characters after 0x)"
        ));
    }
    Ok(body.to_string())
}

fn keccak(bytes: &[u8]) -> [u8; 32] {
    Keccak256::digest(bytes).into()
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// EIP-55: uppercase hex letter `i` when nibble `i` of keccak256(lowercase hex)
/// is >= 8.
pub fn to_eip55(lowercase_40: &str) -> String {
    let hash = keccak(lowercase_40.as_bytes());
    lowercase_40
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let nibble = if i % 2 == 0 {
                hash[i / 2] >> 4
            } else {
                hash[i / 2] & 0x0f
            };
            if c.is_ascii_alphabetic() && nibble >= 8 {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect()
}

/// Difficulty for a prefix/suffix pair. `match_case` doubles the space for each
/// alphabetic position, because that position must also land on the right
/// EIP-55 case.
pub fn difficulty(prefix: &str, suffix: &str, match_case: bool) -> Difficulty {
    let pattern_length = prefix.len() + suffix.len();
    let letters = if match_case {
        prefix
            .chars()
            .chain(suffix.chars())
            .filter(|c| c.is_ascii_alphabetic())
            .count()
    } else {
        0
    };
    let expected_attempts = 16f64.powi(pattern_length as i32) * 2f64.powi(letters as i32);
    // n such that 1 - (1 - 1/d)^n = 0.5.
    let p = 1.0 / expected_attempts;
    let fifty_percent_attempts = if p >= 1.0 {
        1.0
    } else {
        (0.5f64.ln() / (1.0 - p).ln()).ceil()
    };
    Difficulty {
        pattern_length,
        expected_attempts,
        fifty_percent_attempts,
    }
}

/// Chance of at least one hit within `attempts` candidates.
pub fn probability_within(d: &Difficulty, attempts: u64) -> f64 {
    let p = 1.0 / d.expected_attempts;
    if p >= 1.0 {
        return 1.0;
    }
    1.0 - (1.0 - p).powf(attempts as f64)
}

/// Scan up to `max_attempts` consecutive keys starting at `start_key` and
/// return the first whose address matches.
pub fn search(
    prefix: &str,
    suffix: &str,
    match_case: bool,
    max_attempts: u64,
    start_key: &[u8; 32],
) -> Result<Option<VanityMatch>, String> {
    let want_prefix_lower = prefix.to_ascii_lowercase();
    let want_suffix_lower = suffix.to_ascii_lowercase();

    let mut scalar = <Scalar as Reduce<U256>>::reduce_bytes(FieldBytes::from_slice(start_key));
    if scalar == Scalar::ZERO {
        scalar = Scalar::ONE;
    }
    let mut point = ProjectivePoint::GENERATOR * scalar;

    for attempt in 1..=max_attempts {
        let encoded = point.to_affine().to_encoded_point(false);
        let uncompressed = encoded.as_bytes();
        let digest = keccak(&uncompressed[1..]);
        let lower = hex_lower(&digest[12..]);

        let hit = if match_case {
            let checksummed = to_eip55(&lower);
            checksummed.starts_with(prefix) && checksummed.ends_with(suffix)
        } else {
            lower.starts_with(&want_prefix_lower) && lower.ends_with(&want_suffix_lower)
        };

        if hit {
            let private_key: FieldBytes = scalar.to_repr();
            return Ok(Some(VanityMatch {
                address: format!("0x{}", to_eip55(&lower)),
                address_lowercase: format!("0x{lower}"),
                private_key: format!("0x{}", hex_lower(&private_key)),
                public_key: format!("0x{}", hex_lower(uncompressed)),
                attempts: attempt,
            }));
        }

        scalar += Scalar::ONE;
        point += ProjectivePoint::GENERATOR;
    }
    Ok(None)
}

fn thousands(n: f64) -> String {
    if !n.is_finite() {
        return "more than 1e308".to_string();
    }
    if n >= 1e15 {
        return format!("{n:.3e}");
    }
    let digits = format!("{}", n.round() as u128);
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Full tool entry point shared by the chat block, the CLI and the page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    prefix: &str,
    suffix: &str,
    match_case: bool,
    max_attempts: u64,
    output_format: &str,
    seed: &str,
    start_key: &[u8; 32],
) -> Result<String, String> {
    let format = OutputFormat::parse(output_format)?;
    let prefix = clean_pattern(prefix, "prefix")?;
    let suffix = clean_pattern(suffix, "suffix")?;

    if prefix.is_empty() && suffix.is_empty() {
        return Err(
            "give a prefix, a suffix, or both (for example prefix=dead) — with no pattern every \
             random key already matches"
                .into(),
        );
    }
    if prefix.len() + suffix.len() > 40 {
        return Err(format!(
            "prefix ({} chars) plus suffix ({} chars) is longer than the 40 hex characters of an \
             Ethereum address",
            prefix.len(),
            suffix.len()
        ));
    }
    if max_attempts == 0 || max_attempts > MAX_ATTEMPTS_CAP {
        return Err(format!(
            "max_attempts must be between 1 and {MAX_ATTEMPTS_CAP}, got {max_attempts}"
        ));
    }

    let diff = difficulty(&prefix, &suffix, match_case);
    let pattern_text = describe_pattern(&prefix, &suffix, match_case);

    if format == OutputFormat::Estimate {
        let chance = probability_within(&diff, max_attempts) * 100.0;
        return Ok(format!(
            "Pattern:         {pattern_text}\n\
             Fixed positions: {}\n\
             Difficulty:      1 in {}\n\
             50% chance at:   {} keys\n\
             Chance within {}: {chance:.2}%\n\
             (Estimate only — no keys were generated.)",
            diff.pattern_length,
            thousands(diff.expected_attempts),
            thousands(diff.fifty_percent_attempts),
            thousands(max_attempts as f64),
        ));
    }

    let found = search(&prefix, &suffix, match_case, max_attempts, start_key)?;
    let Some(hit) = found else {
        let chance = probability_within(&diff, max_attempts) * 100.0;
        return Err(format!(
            "no address matched {pattern_text} in {} keys (difficulty 1 in {}; a 50% chance needs \
             about {} keys, and {} keys only had a {chance:.2}% chance). Shorten the pattern, turn \
             off case matching, or raise max_attempts (cap {MAX_ATTEMPTS_CAP}).",
            thousands(max_attempts as f64),
            thousands(diff.expected_attempts),
            thousands(diff.fifty_percent_attempts),
            thousands(max_attempts as f64),
        ));
    };

    let origin = if seed.trim().is_empty() {
        "random (platform CSPRNG)".to_string()
    } else {
        format!("seed \"{}\" (reproducible)", seed.trim())
    };

    Ok(match format {
        OutputFormat::Address => hit.address,
        OutputFormat::PrivateKey => hit.private_key,
        OutputFormat::Json => format!(
            "{{\n  \"address\": \"{}\",\n  \"address_lowercase\": \"{}\",\n  \"private_key\": \"{}\",\n  \"public_key\": \"{}\",\n  \"prefix\": \"{}\",\n  \"suffix\": \"{}\",\n  \"match_case\": {},\n  \"attempts\": {},\n  \"max_attempts\": {},\n  \"difficulty\": {},\n  \"fifty_percent_attempts\": {},\n  \"start_key_source\": \"{}\"\n}}",
            hit.address,
            hit.address_lowercase,
            hit.private_key,
            hit.public_key,
            json_escape(&prefix),
            json_escape(&suffix),
            match_case,
            hit.attempts,
            max_attempts,
            diff.expected_attempts.round() as u128,
            diff.fifty_percent_attempts.round() as u128,
            json_escape(&origin),
        ),
        _ => format!(
            "Address:      {}\n\
             Private key:  {}\n\
             Public key:   {}\n\
             \n\
             Pattern:      {pattern_text}\n\
             Difficulty:   1 in {}\n\
             Found after:  {} of {} keys\n\
             Start key:    {origin}\n\
             \n\
             Keep the private key secret: anyone holding it controls the address.",
            hit.address,
            hit.private_key,
            hit.public_key,
            thousands(diff.expected_attempts),
            thousands(hit.attempts as f64),
            thousands(max_attempts as f64),
        ),
    })
}

fn describe_pattern(prefix: &str, suffix: &str, match_case: bool) -> String {
    let case = if match_case {
        "EIP-55 case-sensitive"
    } else {
        "case-insensitive"
    };
    match (prefix.is_empty(), suffix.is_empty()) {
        (false, false) => format!("0x{prefix}…{suffix}, {case}"),
        (false, true) => format!("0x{prefix}…, {case}"),
        (true, false) => format!("0x…{suffix}, {case}"),
        (true, true) => format!("(none), {case}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = "gizza-demo";

    #[test]
    fn finds_a_lowercase_prefix_match() {
        let start = start_key_from_seed(SEED);
        let hit = search("ab", "", false, 10_000, &start).unwrap().unwrap();
        assert!(hit.address_lowercase.starts_with("0xab"), "{hit:?}");
        assert_eq!(hit.address.to_ascii_lowercase(), hit.address_lowercase);
        assert_eq!(hit.private_key.len(), 66);
        assert_eq!(hit.public_key.len(), 132);
        assert!(hit.public_key.starts_with("0x04"));
    }

    #[test]
    fn same_seed_reproduces_the_same_key() {
        let start = start_key_from_seed(SEED);
        let a = search("7", "", false, 1_000, &start).unwrap().unwrap();
        let b = search("7", "", false, 1_000, &start).unwrap().unwrap();
        assert_eq!(a, b);
        let other = search("7", "", false, 1_000, &start_key_from_seed("other"))
            .unwrap()
            .unwrap();
        assert_ne!(a.private_key, other.private_key);
    }

    #[test]
    fn suffix_matching_works() {
        let start = start_key_from_seed(SEED);
        let hit = search("", "e", false, 10_000, &start).unwrap().unwrap();
        assert!(hit.address_lowercase.ends_with('e'), "{hit:?}");
    }

    #[test]
    fn case_sensitive_matching_respects_eip55() {
        let start = start_key_from_seed(SEED);
        let hit = search("AB", "", true, 200_000, &start).unwrap().unwrap();
        assert!(hit.address.starts_with("0xAB"), "{hit:?}");
    }

    #[test]
    fn derived_address_matches_the_public_key() {
        let start = start_key_from_seed(SEED);
        let hit = search("c", "", false, 10_000, &start).unwrap().unwrap();
        let pubkey = hex_decode(&hit.public_key[2..]);
        let digest = keccak(&pubkey[1..]);
        assert_eq!(hit.address_lowercase, format!("0x{}", hex_lower(&digest[12..])));
    }

    #[test]
    fn eip55_matches_the_reference_vector() {
        // From EIP-55's own test vectors.
        assert_eq!(
            to_eip55("5aaeb6053f3e94c9b9a09f33669435e7ef1beaed"),
            "5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
        );
        assert_eq!(
            to_eip55("fb6916095ca1df60bb79ce92ce3ea74c37c5d359"),
            "fB6916095ca1df60bB79Ce92cE3Ea74c37c5d359"
        );
    }

    #[test]
    fn known_private_key_derives_the_documented_address() {
        // Private key 1 → the well-known secp256k1 generator address.
        let mut start = [0u8; 32];
        start[31] = 1;
        let hit = search("", "", false, 1, &start).unwrap().unwrap();
        assert_eq!(
            hit.address,
            "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"
        );
    }

    #[test]
    fn rejects_non_hex_patterns() {
        let start = start_key_from_seed(SEED);
        let err = run("zz", "", false, 100, "all", SEED, &start).unwrap_err();
        assert!(err.contains("hex characters"), "{err}");
    }

    #[test]
    fn rejects_an_empty_pattern() {
        let start = start_key_from_seed(SEED);
        let err = run("", "", false, 100, "all", SEED, &start).unwrap_err();
        assert!(err.contains("prefix"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_attempts() {
        let start = start_key_from_seed(SEED);
        let err = run("a", "", false, 0, "all", SEED, &start).unwrap_err();
        assert!(err.contains("max_attempts"), "{err}");
        let err = run("a", "", false, MAX_ATTEMPTS_CAP + 1, "all", SEED, &start).unwrap_err();
        assert!(err.contains("max_attempts"), "{err}");
    }

    #[test]
    fn reports_a_miss_with_actionable_numbers() {
        let start = start_key_from_seed(SEED);
        let err = run("abcdef", "", false, 50, "all", SEED, &start).unwrap_err();
        assert!(err.contains("no address matched"), "{err}");
        assert!(err.contains("16,777,216"), "{err}");
    }

    #[test]
    fn estimate_mode_does_not_search() {
        let start = [0u8; 32];
        let out = run("dead", "", false, 100_000, "estimate", "", &start).unwrap();
        assert!(out.contains("1 in 65,536"), "{out}");
        assert!(out.contains("Estimate only"), "{out}");
    }

    #[test]
    fn estimate_counts_the_case_sensitive_penalty() {
        let start = [0u8; 32];
        let out = run("dead", "", true, 1_000, "estimate", "", &start).unwrap();
        // 16^4 * 2^4 (four alphabetic positions).
        assert!(out.contains("1 in 1,048,576"), "{out}");
    }

    #[test]
    fn output_formats_render_their_own_shapes() {
        let start = start_key_from_seed(SEED);
        let addr = run("ab", "", false, 10_000, "address", SEED, &start).unwrap();
        assert!(addr.starts_with("0x") && addr.len() == 42, "{addr}");
        let key = run("ab", "", false, 10_000, "private-key", SEED, &start).unwrap();
        assert!(key.starts_with("0x") && key.len() == 66, "{key}");
        let json = run("ab", "", false, 10_000, "json", SEED, &start).unwrap();
        assert!(json.contains("\"attempts\":"), "{json}");
        assert!(json.contains("\"difficulty\": 256"), "{json}");
        let all = run("ab", "", false, 10_000, "all", SEED, &start).unwrap();
        assert!(all.contains("Private key:"), "{all}");
        assert!(all.contains("seed \"gizza-demo\""), "{all}");
    }

    #[test]
    fn a_0x_prefixed_pattern_is_accepted() {
        let start = start_key_from_seed(SEED);
        let a = run("0xab", "", false, 10_000, "address", SEED, &start).unwrap();
        let b = run("ab", "", false, 10_000, "address", SEED, &start).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn random_start_keys_differ() {
        let a = random_start_key().unwrap();
        let b = random_start_key().unwrap();
        assert_ne!(a, b);
        assert_eq!(resolve_start_key("  seedy  ").unwrap(), start_key_from_seed("seedy"));
    }

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }
}
