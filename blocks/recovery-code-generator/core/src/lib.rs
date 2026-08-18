//! recovery-code-generator core — generate a set of one-time account-recovery
//! ("2FA backup") codes: `count` codes, each made of `blocks` groups of
//! `chars_per_block` characters joined by a `separator`, drawn uniformly from a
//! transcription-friendly alphabet, plus optional SHA-256 digests a service can
//! store instead of the codes themselves. No wafer/wasm-bindgen deps.
//!
//! Randomness comes from `getrandom` (WASI `random_get` on wasm32-wasip1, JS
//! crypto on the page). Supplying `seed_hex` switches to a deterministic
//! SHA-256 counter stream so a run is reproducible (tests, page deep-links,
//! regenerating an identical sheet) — that path is only as secret as the seed.
//!
//! BIP39 recovery PHRASES are a different deliverable and live in the
//! `bip39-mnemonic-generator` block; this one is codes only.

use sha2::{Digest, Sha256};

/// Alphabets. Each is deliberately punctuation-free: recovery codes get read
/// off paper and typed into a login box.
const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz0123456789";
const ALPHANUMERIC: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const NUMERIC: &str = "0123456789";
/// Letters + digits with every look-alike pair removed (no 0/O, 1/I/L, and no
/// U so a slip can't spell something unfortunate) — Crockford-style.
const UNAMBIGUOUS: &str = "23456789ABCDEFGHJKMNPQRSTVWXYZ";
const HEX: &str = "0123456789abcdef";

/// Caps. `blocks * chars_per_block` is the code length, so codes span 2-96
/// characters and a sheet is at most 50 codes.
pub const MAX_COUNT: usize = 50;
pub const MAX_BLOCKS: usize = 6;
pub const MAX_CHARS_PER_BLOCK: usize = 16;
pub const MIN_CHARS_PER_BLOCK: usize = 2;
pub const MAX_SEPARATOR_CHARS: usize = 3;
/// Salt length for `hash = sha256-salted`, in bytes.
const SALT_BYTES: usize = 16;

/// Everything the generator needs. Strings are the raw surface values (chat,
/// CLI and page all hand over text) and are parsed here.
#[derive(Debug, Clone)]
pub struct Options {
    pub count: usize,
    pub blocks: usize,
    pub chars_per_block: usize,
    pub charset: String,
    pub separator: String,
    pub output: String,
    pub hash: String,
    pub seed_hex: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            count: 10,
            blocks: 2,
            chars_per_block: 5,
            charset: "lowercase".into(),
            separator: "-".into(),
            output: "numbered".into(),
            hash: "none".into(),
            seed_hex: String::new(),
        }
    }
}

/// A generated sheet of recovery codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeSet {
    /// The codes, formatted with the separator exactly as they are shown.
    pub codes: Vec<String>,
    /// One digest per code, empty when `hash = none`. For `sha256-salted` each
    /// entry is `<salt_hex>:<digest_hex>`.
    pub hashes: Vec<String>,
    /// Code length in characters, separators excluded.
    pub code_length: usize,
    /// Size of the alphabet the characters were drawn from.
    pub alphabet_size: usize,
    /// True when the set came from `seed_hex` instead of the system CSPRNG.
    pub deterministic: bool,
}

impl CodeSet {
    /// Entropy of a single code, in bits.
    pub fn bits_per_code(&self) -> f64 {
        let bits = self.code_length as f64 * (self.alphabet_size as f64).log2();
        (bits * 100.0).round() / 100.0
    }
    /// Entropy of the whole sheet, in bits.
    pub fn bits_total(&self) -> f64 {
        let bits = self.codes.len() as f64
            * self.code_length as f64
            * (self.alphabet_size as f64).log2();
        (bits * 100.0).round() / 100.0
    }
}

/// Resolve an alphabet name to (alphabet, human label).
fn alphabet_for(name: &str) -> Result<(&'static str, &'static str), String> {
    Ok(match name.trim().to_ascii_lowercase().as_str() {
        "" | "lowercase" | "lower" => (LOWERCASE, "a-z0-9"),
        "alphanumeric" | "mixedcase" | "mixed" | "base62" => (ALPHANUMERIC, "A-Za-z0-9"),
        "uppercase" | "upper" => (UPPERCASE, "A-Z0-9"),
        "numeric" | "digits" => (NUMERIC, "0-9"),
        "unambiguous" | "safe" | "no-ambiguous" => {
            (UNAMBIGUOUS, "A-Z0-9 without look-alikes")
        }
        "hex" | "hexadecimal" => (HEX, "0-9a-f"),
        other => {
            return Err(format!(
                "unknown charset {other:?} — use lowercase, alphanumeric, uppercase, numeric, unambiguous, or hex"
            ))
        }
    })
}

/// Which digest, if any, to publish next to each code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HashKind {
    None,
    Sha256,
    Sha256Salted,
}

impl HashKind {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "off" => HashKind::None,
            "sha256" | "sha-256" => HashKind::Sha256,
            "sha256-salted" | "sha256_salted" | "salted" => HashKind::Sha256Salted,
            other => {
                return Err(format!(
                    "unknown hash {other:?} — use none, sha256, or sha256-salted"
                ))
            }
        })
    }
    /// Column / field name used in the csv and json renderings.
    fn field(self) -> &'static str {
        match self {
            HashKind::None => "",
            HashKind::Sha256 => "sha256",
            HashKind::Sha256Salted => "sha256_salted",
        }
    }
}

/// How the sheet is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    Numbered,
    Plain,
    Csv,
    Json,
}

impl OutputKind {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "numbered" | "list" => OutputKind::Numbered,
            "plain" | "codes" => OutputKind::Plain,
            "csv" => OutputKind::Csv,
            "json" => OutputKind::Json,
            other => {
                return Err(format!(
                    "unknown output {other:?} — use numbered, plain, csv, or json"
                ))
            }
        })
    }
}

/// Random-byte source: the platform CSPRNG, or a deterministic SHA-256 counter
/// stream keyed by `seed_hex`.
struct Rng {
    seed: Vec<u8>,
    seeded: bool,
    counter: u64,
    buf: Vec<u8>,
    pos: usize,
}

impl Rng {
    fn os() -> Self {
        Rng { seed: Vec::new(), seeded: false, counter: 0, buf: Vec::new(), pos: 0 }
    }

    fn seeded(seed: Vec<u8>) -> Self {
        Rng { seed, seeded: true, counter: 0, buf: Vec::new(), pos: 0 }
    }

    fn fill(&mut self, out: &mut [u8]) -> Result<(), String> {
        if !self.seeded {
            return getrandom::getrandom(out).map_err(|e| format!("RNG failure: {e}"));
        }
        for slot in out.iter_mut() {
            if self.pos >= self.buf.len() {
                let mut h = Sha256::new();
                h.update(b"gizza recovery-code-generator v1");
                h.update(&self.seed);
                h.update(self.counter.to_be_bytes());
                self.buf = h.finalize().to_vec();
                self.counter += 1;
                self.pos = 0;
            }
            *slot = self.buf[self.pos];
            self.pos += 1;
        }
        Ok(())
    }

    /// Uniform index in `0..n` by rejection sampling — no modulo bias, even for
    /// a 30- or 36-character alphabet.
    fn index(&mut self, n: usize) -> Result<usize, String> {
        if n == 0 {
            return Err("empty alphabet".into());
        }
        let nn = n as u64;
        let zone = (u64::from(u32::MAX) + 1) / nn * nn;
        loop {
            let mut b = [0u8; 4];
            self.fill(&mut b)?;
            let v = u64::from(u32::from_be_bytes(b));
            if v < zone {
                return Ok((v % nn) as usize);
            }
        }
    }
}

fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if t.len() % 2 != 0 {
        return Err(format!(
            "seed_hex must have an even number of hex digits (got {})",
            t.len()
        ));
    }
    if !(8..=128).contains(&t.len()) {
        return Err(format!(
            "seed_hex must be 8-128 hex digits (4-64 bytes), got {}",
            t.len()
        ));
    }
    (0..t.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&t[i..i + 2], 16)
                .map_err(|_| format!("seed_hex is not valid hex near {:?}", &t[i..i + 2]))
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Validate the options and generate the code set.
pub fn generate(o: &Options) -> Result<CodeSet, String> {
    if o.count == 0 || o.count > MAX_COUNT {
        return Err(format!(
            "count must be between 1 and {MAX_COUNT} codes, got {}",
            o.count
        ));
    }
    if o.blocks == 0 || o.blocks > MAX_BLOCKS {
        return Err(format!(
            "blocks must be between 1 and {MAX_BLOCKS}, got {}",
            o.blocks
        ));
    }
    if o.chars_per_block < MIN_CHARS_PER_BLOCK || o.chars_per_block > MAX_CHARS_PER_BLOCK {
        return Err(format!(
            "chars_per_block must be between {MIN_CHARS_PER_BLOCK} and {MAX_CHARS_PER_BLOCK}, got {}",
            o.chars_per_block
        ));
    }
    let (alphabet, _) = alphabet_for(&o.charset)?;
    let alpha: Vec<char> = alphabet.chars().collect();

    // The separator must stay distinguishable from the code characters,
    // otherwise a printed code cannot be read back unambiguously. Commas,
    // quotes and whitespace would also break the csv rendering.
    if o.separator.chars().count() > MAX_SEPARATOR_CHARS {
        return Err(format!(
            "separator must be at most {MAX_SEPARATOR_CHARS} characters, got {}",
            o.separator.chars().count()
        ));
    }
    for c in o.separator.chars() {
        if alpha.contains(&c) {
            return Err(format!(
                "separator character {c:?} is also in the {} alphabet — pick a separator that cannot be mistaken for part of a code",
                o.charset.trim().to_ascii_lowercase()
            ));
        }
        if c.is_whitespace() || c == ',' || c == '"' {
            return Err(format!(
                "separator character {c:?} is not allowed — whitespace, commas and quotes would break the printed and csv output"
            ));
        }
    }

    let hash = HashKind::parse(&o.hash)?;
    let _ = OutputKind::parse(&o.output)?;

    let deterministic = !o.seed_hex.trim().is_empty();
    let mut rng = if deterministic {
        Rng::seeded(parse_hex(&o.seed_hex)?)
    } else {
        Rng::os()
    };

    let mut codes = Vec::with_capacity(o.count);
    let mut hashes = Vec::new();
    for _ in 0..o.count {
        let mut parts = Vec::with_capacity(o.blocks);
        for _ in 0..o.blocks {
            let mut block = String::with_capacity(o.chars_per_block);
            for _ in 0..o.chars_per_block {
                block.push(alpha[rng.index(alpha.len())?]);
            }
            parts.push(block);
        }
        let code = parts.join(&o.separator);
        match hash {
            HashKind::None => {}
            HashKind::Sha256 => {
                let mut h = Sha256::new();
                h.update(code.as_bytes());
                hashes.push(hex(&h.finalize()));
            }
            HashKind::Sha256Salted => {
                let mut salt = [0u8; SALT_BYTES];
                rng.fill(&mut salt)?;
                let mut h = Sha256::new();
                h.update(salt);
                h.update(code.as_bytes());
                hashes.push(format!("{}:{}", hex(&salt), hex(&h.finalize())));
            }
        }
        codes.push(code);
    }

    Ok(CodeSet {
        codes,
        hashes,
        code_length: o.blocks * o.chars_per_block,
        alphabet_size: alpha.len(),
        deterministic,
    })
}

/// Render a generated set per `o.output`.
pub fn render(set: &CodeSet, o: &Options) -> Result<String, String> {
    let kind = OutputKind::parse(&o.output)?;
    let hash = HashKind::parse(&o.hash)?;
    let (_, label) = alphabet_for(&o.charset)?;

    let summary = format!(
        "{} code{} · {} characters each ({}, {}-character alphabet) · {:.1} bits per code · {:.1} bits total{}",
        set.codes.len(),
        if set.codes.len() == 1 { "" } else { "s" },
        set.code_length,
        label,
        set.alphabet_size,
        set.bits_per_code(),
        set.bits_total(),
        if set.deterministic {
            " · derived from seed_hex (reproducible, NOT secret unless the seed is)"
        } else {
            ""
        }
    );

    Ok(match kind {
        OutputKind::Numbered => {
            let mut out = String::new();
            for (i, code) in set.codes.iter().enumerate() {
                out.push_str(&format!("{:>2}. {}", i + 1, code));
                if let Some(h) = set.hashes.get(i) {
                    out.push_str(&format!("  {h}"));
                }
                out.push('\n');
            }
            format!("{out}\n{summary}")
        }
        OutputKind::Plain => {
            let mut out = String::new();
            for (i, code) in set.codes.iter().enumerate() {
                out.push_str(code);
                if let Some(h) = set.hashes.get(i) {
                    out.push_str(&format!("  {h}"));
                }
                out.push('\n');
            }
            format!("{out}\n{summary}")
        }
        OutputKind::Csv => {
            let mut out = String::from("index,code");
            if hash != HashKind::None {
                out.push(',');
                out.push_str(hash.field());
            }
            out.push('\n');
            for (i, code) in set.codes.iter().enumerate() {
                out.push_str(&format!("{},{}", i + 1, code));
                if let Some(h) = set.hashes.get(i) {
                    out.push(',');
                    out.push_str(h);
                }
                out.push('\n');
            }
            out.trim_end().to_string()
        }
        OutputKind::Json => {
            let mut v = serde_json::json!({
                "count": set.codes.len(),
                "code_length": set.code_length,
                "alphabet_size": set.alphabet_size,
                "bits_per_code": set.bits_per_code(),
                "bits_total": set.bits_total(),
                "deterministic": set.deterministic,
                "codes": set.codes,
            });
            if hash != HashKind::None {
                v[hash.field()] = serde_json::json!(set.hashes);
            }
            serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?
        }
    })
}

/// Generate + render in one call — the entry point every surface uses.
pub fn run(o: &Options) -> Result<String, String> {
    let set = generate(o)?;
    render(&set, o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded(seed: &str) -> Options {
        Options { seed_hex: seed.into(), ..Options::default() }
    }

    #[test]
    fn seeded_run_is_deterministic_and_shaped() {
        let o = seeded("00112233445566778899aabbccddeeff");
        let a = generate(&o).expect("generates");
        let b = generate(&o).expect("generates");
        assert_eq!(a, b, "same seed → same sheet");
        assert_eq!(a.codes.len(), 10);
        assert_eq!(a.code_length, 10);
        assert_eq!(a.alphabet_size, 36);
        assert!(a.deterministic);
        for code in &a.codes {
            assert_eq!(code.chars().count(), 11, "5 + '-' + 5");
            let (l, r) = code.split_once('-').expect("one separator");
            assert_eq!(l.len(), 5);
            assert_eq!(r.len(), 5);
            assert!(
                code.chars().all(|c| c == '-' || LOWERCASE.contains(c)),
                "only alphabet characters: {code}"
            );
        }
        // Ten 10-character codes over a 36-character alphabet.
        assert_eq!(a.bits_per_code(), 51.7);
        assert_eq!(a.bits_total(), 516.99);
    }

    #[test]
    fn random_run_differs_between_calls() {
        let o = Options::default();
        let a = generate(&o).expect("generates");
        let b = generate(&o).expect("generates");
        assert!(!a.deterministic);
        assert_ne!(a.codes, b.codes, "a CSPRNG sheet is not repeatable");
    }

    #[test]
    fn numbered_output_lists_and_summarises() {
        let text = run(&seeded("0123456789abcdef")).expect("runs");
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with(" 1. "), "got {:?}", lines[0]);
        assert!(lines[9].starts_with("10. "), "got {:?}", lines[9]);
        assert!(text.contains("10 codes · 10 characters each (a-z0-9, 36-character alphabet)"));
        assert!(text.contains("51.7 bits per code"));
        assert!(text.contains("derived from seed_hex"));
    }

    #[test]
    fn csv_output_has_a_header_and_hash_column() {
        let o = Options {
            count: 2,
            hash: "sha256".into(),
            output: "csv".into(),
            ..seeded("0123456789abcdef")
        };
        let text = run(&o).expect("runs");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "index,code,sha256");
        assert_eq!(lines.len(), 3);
        let cols: Vec<&str> = lines[1].split(',').collect();
        assert_eq!(cols[0], "1");
        assert_eq!(cols[2].len(), 64, "sha256 hex digest");
        // The digest is over the code exactly as displayed.
        let mut h = Sha256::new();
        h.update(cols[1].as_bytes());
        assert_eq!(cols[2], hex(&h.finalize()));
    }

    #[test]
    fn json_output_carries_codes_and_entropy() {
        let o = Options {
            count: 3,
            blocks: 1,
            chars_per_block: 8,
            charset: "numeric".into(),
            separator: String::new(),
            output: "json".into(),
            ..seeded("0123456789abcdef")
        };
        let text = run(&o).expect("runs");
        let v: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(v["count"], 3);
        assert_eq!(v["code_length"], 8);
        assert_eq!(v["alphabet_size"], 10);
        assert_eq!(v["bits_per_code"], 26.58);
        assert_eq!(v["codes"].as_array().unwrap().len(), 3);
        for c in v["codes"].as_array().unwrap() {
            let s = c.as_str().unwrap();
            assert_eq!(s.len(), 8);
            assert!(s.chars().all(|c| c.is_ascii_digit()), "digits only: {s}");
        }
        assert!(v.get("sha256").is_none(), "no digests when hash = none");
    }

    #[test]
    fn salted_hashes_are_salt_colon_digest_and_unique() {
        let o = Options {
            count: 4,
            hash: "sha256-salted".into(),
            ..seeded("0123456789abcdef")
        };
        let set = generate(&o).expect("generates");
        assert_eq!(set.hashes.len(), 4);
        let mut salts = Vec::new();
        for h in &set.hashes {
            let (salt, digest) = h.split_once(':').expect("salt:digest");
            assert_eq!(salt.len(), SALT_BYTES * 2);
            assert_eq!(digest.len(), 64);
            salts.push(salt.to_string());
        }
        salts.sort();
        salts.dedup();
        assert_eq!(salts.len(), 4, "a fresh salt per code");
    }

    #[test]
    fn every_charset_draws_only_its_own_characters() {
        for (name, expect_len) in [
            ("lowercase", 36),
            ("alphanumeric", 62),
            ("uppercase", 36),
            ("numeric", 10),
            ("unambiguous", 30),
            ("hex", 16),
        ] {
            let o = Options {
                count: 5,
                charset: name.into(),
                ..seeded("0123456789abcdef")
            };
            let set = generate(&o).expect(name);
            assert_eq!(set.alphabet_size, expect_len, "{name} alphabet size");
            let (alphabet, _) = alphabet_for(name).unwrap();
            for code in &set.codes {
                assert!(
                    code.chars().all(|c| c == '-' || alphabet.contains(c)),
                    "{name}: {code}"
                );
            }
        }
    }

    #[test]
    fn unambiguous_alphabet_has_no_look_alikes() {
        for c in ['0', '1', 'O', 'I', 'L', 'U'] {
            assert!(!UNAMBIGUOUS.contains(c), "{c} should be excluded");
        }
    }

    #[test]
    fn blank_separator_and_single_block_give_a_bare_code() {
        let o = Options {
            count: 1,
            blocks: 1,
            chars_per_block: 8,
            separator: String::new(),
            ..seeded("0123456789abcdef")
        };
        let set = generate(&o).expect("generates");
        assert_eq!(set.codes[0].len(), 8);
        assert!(!set.codes[0].contains('-'));
    }

    #[test]
    fn caps_are_enforced_at_the_boundary() {
        let at_cap = Options {
            count: MAX_COUNT,
            blocks: MAX_BLOCKS,
            chars_per_block: MAX_CHARS_PER_BLOCK,
            ..seeded("0123456789abcdef")
        };
        let set = generate(&at_cap).expect("the exact cap is allowed");
        assert_eq!(set.codes.len(), 50);
        assert_eq!(set.code_length, 96);

        let over = Options { count: MAX_COUNT + 1, ..at_cap.clone() };
        assert!(generate(&over).unwrap_err().contains("count must be between 1 and 50"));
        let over = Options { blocks: MAX_BLOCKS + 1, ..at_cap.clone() };
        assert!(generate(&over).unwrap_err().contains("blocks must be between 1 and 6"));
        let over = Options { chars_per_block: MAX_CHARS_PER_BLOCK + 1, ..at_cap };
        assert!(generate(&over)
            .unwrap_err()
            .contains("chars_per_block must be between 2 and 16"));
    }

    #[test]
    fn zero_count_is_rejected() {
        let o = Options { count: 0, ..Options::default() };
        assert!(generate(&o).unwrap_err().contains("count must be between 1 and 50"));
    }

    #[test]
    fn separator_colliding_with_the_alphabet_is_rejected() {
        let o = Options { separator: "x".into(), ..Options::default() };
        let err = generate(&o).unwrap_err();
        assert!(err.contains("also in the lowercase alphabet"), "got {err}");

        // '-' is fine for lowercase but a comma never is (it breaks csv).
        let o = Options { separator: ",".into(), ..Options::default() };
        assert!(generate(&o).unwrap_err().contains("not allowed"));

        let o = Options { separator: "----".into(), ..Options::default() };
        assert!(generate(&o)
            .unwrap_err()
            .contains("separator must be at most 3 characters"));
    }

    #[test]
    fn unknown_enum_values_are_rejected_by_name() {
        let o = Options { charset: "emoji".into(), ..Options::default() };
        assert!(generate(&o).unwrap_err().contains("unknown charset"));
        let o = Options { hash: "md5".into(), ..Options::default() };
        assert!(generate(&o).unwrap_err().contains("unknown hash"));
        let o = Options { output: "xml".into(), ..Options::default() };
        assert!(generate(&o).unwrap_err().contains("unknown output"));
    }

    #[test]
    fn bad_seed_hex_is_rejected() {
        let o = Options { seed_hex: "zz11223344".into(), ..Options::default() };
        assert!(generate(&o).unwrap_err().contains("not valid hex"));
        let o = Options { seed_hex: "abc".into(), ..Options::default() };
        assert!(generate(&o).unwrap_err().contains("even number of hex digits"));
        let o = Options { seed_hex: "ab".into(), ..Options::default() };
        assert!(generate(&o).unwrap_err().contains("8-128 hex digits"));
    }

    #[test]
    fn seed_whitespace_is_ignored_so_pasted_seeds_work() {
        let a = generate(&seeded("0123456789abcdef")).unwrap();
        let b = generate(&seeded(" 0123 4567 89ab cdef ")).unwrap();
        assert_eq!(a.codes, b.codes);
    }
}
