//! gizza-ai/random-bytes core — draw N cryptographically random BYTES and
//! render those exact bytes in a chosen encoding (hex, Base64, Base64URL,
//! binary, decimal, a C array literal or a Python bytes literal). No
//! wafer/wasm-bindgen deps.
//!
//! Randomness comes from `getrandom` (WASI `random_get` on wasm32-wasip1 for the
//! chat/CLI block, the JS crypto backend on the page's wasm32-unknown-unknown
//! build). Supplying `seed_hex` switches to a deterministic SHA-256 counter
//! stream so a run is reproducible (tests, page deep-links, documented
//! fixtures) — that output is only as secret as the seed.
//!
//! Byte-count semantics are the point of this block: `bytes = 32` always means
//! 256 bits of entropy no matter which encoding is selected, so `hex` yields 64
//! characters and `base64` 44. The sibling `random-token-generator` block does
//! the opposite — it samples a requested number of CHARACTERS from an alphabet,
//! where 32 hex characters carry only 128 bits.

use sha2::{Digest, Sha256};

/// Caps. `bytes` covers every realistic key size (a 4096-bit RSA modulus is 512
/// bytes); `count` matches the batch sizes competing generators offer; the
/// product is capped separately so a big-times-big request cannot produce a
/// multi-megabyte string.
pub const MAX_BYTES: usize = 4096;
pub const MAX_COUNT: usize = 100;
pub const MAX_TOTAL_BYTES: usize = 8192;

const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";
const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
const B64_STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Everything the generator needs. Strings are the raw surface values (chat,
/// CLI and page all hand over text) and are parsed here.
#[derive(Debug, Clone)]
pub struct Options {
    /// Random bytes per value.
    pub bytes: usize,
    /// How many independent values to draw.
    pub count: usize,
    /// Encoding name, see [`Encoding::parse`].
    pub encoding: String,
    /// Separator name, see [`Separator::parse`].
    pub separator: String,
    /// Uppercase the hex digits of `hex` / `c-array`.
    pub uppercase: bool,
    /// `text` or `json`.
    pub output: String,
    /// Optional hex seed; blank means "use the platform CSPRNG".
    pub seed_hex: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            bytes: 32,
            count: 1,
            encoding: "hex".into(),
            separator: "auto".into(),
            uppercase: false,
            output: "text".into(),
            seed_hex: String::new(),
        }
    }
}

/// How the drawn bytes are written out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
    Base64Url,
    Binary,
    Decimal,
    CArray,
    PythonBytes,
}

impl Encoding {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" | "hexadecimal" => Encoding::Hex,
            "base64" | "b64" => Encoding::Base64,
            "base64url" | "base64-url" | "b64url" => Encoding::Base64Url,
            "binary" | "bits" => Encoding::Binary,
            "decimal" | "dec" | "byte-array" => Encoding::Decimal,
            "c-array" | "c_array" | "carray" => Encoding::CArray,
            "python-bytes" | "python_bytes" | "python" => Encoding::PythonBytes,
            other => {
                return Err(format!(
                    "unknown encoding {other:?} — use hex, base64, base64url, binary, decimal, c-array, or python-bytes"
                ))
            }
        })
    }

    /// Label used in the text summary and the json payload.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Hex => "hex",
            Encoding::Base64 => "base64",
            Encoding::Base64Url => "base64url",
            Encoding::Binary => "binary",
            Encoding::Decimal => "decimal",
            Encoding::CArray => "c-array",
            Encoding::PythonBytes => "python-bytes",
        }
    }

    /// True when the encoding writes one visible unit per byte, i.e. when a
    /// byte separator is meaningful. Base64 packs 3 bytes into 4 characters and
    /// the two literal encodings carry their own punctuation, so neither can
    /// take one.
    fn takes_separator(self) -> bool {
        matches!(self, Encoding::Hex | Encoding::Binary | Encoding::Decimal)
    }

    /// The `openssl rand` flag that produces the same thing, when one exists.
    fn openssl_flag(self) -> Option<&'static str> {
        match self {
            Encoding::Hex => Some("-hex"),
            Encoding::Base64 => Some("-base64"),
            _ => None,
        }
    }
}

/// Byte separator for the one-unit-per-byte encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Separator {
    /// Each encoding's conventional default.
    Auto,
    None,
    Space,
    Colon,
    Dash,
    Comma,
}

impl Separator {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Separator::Auto,
            "none" => Separator::None,
            "space" => Separator::Space,
            "colon" => Separator::Colon,
            "dash" | "hyphen" => Separator::Dash,
            "comma" => Separator::Comma,
            other => {
                return Err(format!(
                    "unknown separator {other:?} — use auto, none, space, colon, dash, or comma"
                ))
            }
        })
    }

    /// Resolve to the literal text placed between bytes. `Auto` follows each
    /// encoding's convention: hex runs together (as `openssl rand -hex` prints
    /// it), bits are grouped per byte, decimal reads as a comma-separated list.
    fn text(self, enc: Encoding) -> &'static str {
        // base64/base64url pack 3 bytes into 4 characters and the two literal
        // encodings carry their own punctuation, so there is no byte boundary
        // to split: the separator is ignored rather than corrupting the value
        // (and must not be advertised in the summary either).
        if !enc.takes_separator() {
            return "";
        }
        match self {
            Separator::Auto => match enc {
                Encoding::Binary => " ",
                Encoding::Decimal => ", ",
                _ => "",
            },
            Separator::None => "",
            Separator::Space => " ",
            Separator::Colon => ":",
            Separator::Dash => "-",
            Separator::Comma => ", ",
        }
    }

    /// Label for the text summary, or `None` when nothing is inserted.
    fn label(self, enc: Encoding) -> Option<&'static str> {
        match self.text(enc) {
            "" => None,
            " " => Some("space-separated"),
            ":" => Some("colon-separated"),
            "-" => Some("dash-separated"),
            ", " => Some("comma-separated"),
            _ => None,
        }
    }
}

/// How the result is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputKind {
    Text,
    Json,
}

impl OutputKind {
    fn parse(s: &str) -> Result<Self, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "plain" => OutputKind::Text,
            "json" => OutputKind::Json,
            other => return Err(format!("unknown output {other:?} — use text or json")),
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
                h.update(b"gizza random-bytes v1");
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
}

/// Parse the optional seed. Whitespace is ignored so a seed pasted in groups
/// still works.
fn parse_seed(s: &str) -> Result<Vec<u8>, String> {
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

/// Base64 (RFC 4648). `std` = §4 alphabet with `=` padding, `url` = §5 URL-safe
/// alphabet without padding, which is what tokens and JWT segments use.
fn base64(bytes: &[u8], url_safe: bool) -> String {
    let table = if url_safe { B64_URL } else { B64_STD };
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(table[(n >> 18) as usize & 63] as char);
        out.push(table[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(table[(n >> 6) as usize & 63] as char);
        } else if !url_safe {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(table[n as usize & 63] as char);
        } else if !url_safe {
            out.push('=');
        }
    }
    out
}

fn hex_str(bytes: &[u8], upper: bool, sep: &str) -> String {
    let table = if upper { HEX_UPPER } else { HEX_LOWER };
    let mut out = String::with_capacity(bytes.len() * (2 + sep.len()));
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        out.push(table[(b >> 4) as usize] as char);
        out.push(table[(b & 0x0f) as usize] as char);
    }
    out
}

/// Render one value's bytes in the chosen encoding.
fn encode(bytes: &[u8], enc: Encoding, sep: &str, upper: bool) -> String {
    match enc {
        Encoding::Hex => hex_str(bytes, upper, sep),
        Encoding::Base64 => base64(bytes, false),
        Encoding::Base64Url => base64(bytes, true),
        Encoding::Binary => bytes
            .iter()
            .map(|b| format!("{b:08b}"))
            .collect::<Vec<_>>()
            .join(sep),
        Encoding::Decimal => bytes
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(sep),
        // A C initializer list, ready to paste into `static const uint8_t
        // key[N] = ...;`.
        Encoding::CArray => format!(
            "{{ {} }}",
            bytes
                .iter()
                .map(|b| format!("0x{}", hex_str(&[*b], upper, "")))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        // A Python bytes literal. Every byte is escaped as \xNN (including the
        // printable ones) so the literal never depends on quoting rules.
        Encoding::PythonBytes => format!(
            "b'{}'",
            bytes
                .iter()
                .map(|b| format!("\\x{}", hex_str(&[*b], upper, "")))
                .collect::<Vec<_>>()
                .join("")
        ),
    }
}

/// A generated batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteSet {
    /// The encoded values, exactly as displayed.
    pub values: Vec<String>,
    /// Random bytes behind each value.
    pub bytes_each: usize,
    /// Encoding used.
    pub encoding: Encoding,
    /// True when the batch came from `seed_hex` instead of the system CSPRNG.
    pub deterministic: bool,
}

impl ByteSet {
    /// Entropy of one value, in bits. Every byte contributes a full 8 bits
    /// regardless of encoding — the encoding changes the character count, never
    /// the entropy.
    pub fn bits_each(&self) -> usize {
        self.bytes_each * 8
    }
}

/// Validate the options and draw the bytes.
pub fn generate(o: &Options) -> Result<ByteSet, String> {
    if o.bytes == 0 || o.bytes > MAX_BYTES {
        return Err(format!(
            "bytes must be between 1 and {MAX_BYTES}, got {}",
            o.bytes
        ));
    }
    if o.count == 0 || o.count > MAX_COUNT {
        return Err(format!(
            "count must be between 1 and {MAX_COUNT}, got {}",
            o.count
        ));
    }
    if o.bytes * o.count > MAX_TOTAL_BYTES {
        return Err(format!(
            "bytes x count must be at most {MAX_TOTAL_BYTES} random bytes per run, got {} x {} = {}",
            o.bytes,
            o.count,
            o.bytes * o.count
        ));
    }
    let enc = Encoding::parse(&o.encoding)?;
    let sep = Separator::parse(&o.separator)?;
    let _ = OutputKind::parse(&o.output)?;

    let deterministic = !o.seed_hex.trim().is_empty();
    let mut rng = if deterministic {
        Rng::seeded(parse_seed(&o.seed_hex)?)
    } else {
        Rng::os()
    };

    let sep_text = sep.text(enc);
    let mut values = Vec::with_capacity(o.count);
    let mut raw = vec![0u8; o.bytes];
    for _ in 0..o.count {
        rng.fill(&mut raw)?;
        values.push(encode(&raw, enc, sep_text, o.uppercase));
    }

    Ok(ByteSet { values, bytes_each: o.bytes, encoding: enc, deterministic })
}

/// One-line summary printed under the values in `text` output.
fn summary(set: &ByteSet, o: &Options, sep: Separator) -> String {
    let n = set.values.len();
    let mut parts = vec![
        format!("{n} value{}", if n == 1 { "" } else { "s" }),
        format!(
            "{} byte{} ({} bits) each",
            set.bytes_each,
            if set.bytes_each == 1 { "" } else { "s" },
            set.bits_each()
        ),
        set.encoding.name().to_string(),
    ];
    if let Some(l) = sep.label(set.encoding) {
        parts.push(l.to_string());
    }
    if o.uppercase && matches!(set.encoding, Encoding::Hex | Encoding::CArray) {
        parts.push("uppercase".to_string());
    }
    if set.deterministic {
        parts.push("derived from seed_hex (reproducible, NOT secret unless the seed is)".to_string());
    }
    if let Some(flag) = set.encoding.openssl_flag() {
        parts.push(format!("equivalent: openssl rand {flag} {}", set.bytes_each));
    }
    parts.join(" · ")
}

/// Generate and render. This is what every surface calls.
pub fn run(o: &Options) -> Result<String, String> {
    let kind = OutputKind::parse(&o.output)?;
    let sep = Separator::parse(&o.separator)?;
    let set = generate(o)?;
    Ok(match kind {
        OutputKind::Text => {
            format!("{}\n\n{}", set.values.join("\n"), summary(&set, o, sep))
        }
        OutputKind::Json => {
            let v = serde_json::json!({
                "count": set.values.len(),
                "bytes": set.bytes_each,
                "bits": set.bits_each(),
                "encoding": set.encoding.name(),
                "uppercase": o.uppercase,
                "deterministic": set.deterministic,
                "values": set.values,
            });
            serde_json::to_string_pretty(&v).map_err(|e| format!("json error: {e}"))?
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded(bytes: usize, count: usize, encoding: &str) -> Options {
        Options {
            bytes,
            count,
            encoding: encoding.into(),
            seed_hex: "00112233445566778899aabbccddeeff".into(),
            ..Options::default()
        }
    }

    #[test]
    fn hex_default_is_64_lowercase_characters_for_32_bytes() {
        let out = run(&Options::default()).unwrap();
        let first = out.lines().next().unwrap();
        assert_eq!(first.len(), 64, "32 bytes = 64 hex characters: {first}");
        assert!(first.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert!(out.contains("1 value · 32 bytes (256 bits) each · hex"));
        assert!(out.contains("equivalent: openssl rand -hex 32"));
    }

    #[test]
    fn seed_is_reproducible_and_differs_from_another_seed() {
        let a = run(&seeded(16, 2, "hex")).unwrap();
        let b = run(&seeded(16, 2, "hex")).unwrap();
        assert_eq!(a, b, "same seed must reproduce the same batch");
        let mut other = seeded(16, 2, "hex");
        other.seed_hex = "ffeeddccbbaa99887766554433221100".into();
        assert_ne!(run(&other).unwrap(), a);
        assert!(a.contains("derived from seed_hex"));
    }

    #[test]
    fn unseeded_runs_differ() {
        let a = run(&Options::default()).unwrap();
        let b = run(&Options::default()).unwrap();
        assert_ne!(a, b, "two CSPRNG draws of 32 bytes must not collide");
    }

    #[test]
    fn every_encoding_round_trips_the_same_seeded_bytes() {
        // The same seed must yield the same underlying bytes in every encoding,
        // so the renderings agree byte for byte.
        let hex = run(&seeded(3, 1, "hex")).unwrap().lines().next().unwrap().to_string();
        let raw: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(raw.len(), 3);

        let dec = run(&seeded(3, 1, "decimal")).unwrap().lines().next().unwrap().to_string();
        assert_eq!(
            dec,
            raw.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", ")
        );

        let bin = run(&seeded(3, 1, "binary")).unwrap().lines().next().unwrap().to_string();
        assert_eq!(bin, raw.iter().map(|b| format!("{b:08b}")).collect::<Vec<_>>().join(" "));

        let b64 = run(&seeded(3, 1, "base64")).unwrap().lines().next().unwrap().to_string();
        assert_eq!(b64, base64(&raw, false));
        assert_eq!(b64.len(), 4, "3 bytes fit one base64 quantum");

        let c = run(&seeded(3, 1, "c-array")).unwrap().lines().next().unwrap().to_string();
        assert_eq!(c, format!("{{ 0x{:02x}, 0x{:02x}, 0x{:02x} }}", raw[0], raw[1], raw[2]));

        let py = run(&seeded(3, 1, "python-bytes")).unwrap().lines().next().unwrap().to_string();
        assert_eq!(py, format!("b'\\x{:02x}\\x{:02x}\\x{:02x}'", raw[0], raw[1], raw[2]));
    }

    #[test]
    fn base64_padding_and_url_safe_lengths() {
        // Standard base64 pads to a multiple of 4; base64url drops the padding.
        for n in [1usize, 2, 3, 16, 32] {
            let std = run(&seeded(n, 1, "base64")).unwrap().lines().next().unwrap().to_string();
            assert_eq!(std.len(), (n + 2) / 3 * 4, "padded length for {n} bytes");
            let url = run(&seeded(n, 1, "base64url")).unwrap().lines().next().unwrap().to_string();
            assert_eq!(url.len(), (n * 8 + 5) / 6, "unpadded length for {n} bytes");
            assert!(!url.contains('=') && !url.contains('+') && !url.contains('/'));
        }
        // Known vector: the RFC 4648 §10 test vector for "Ma".
        assert_eq!(base64(b"Ma", false), "TWE=");
        assert_eq!(base64(b"Ma", true), "TWE");
        assert_eq!(base64(&[0xfb, 0xff], false), "+/8=");
        assert_eq!(base64(&[0xfb, 0xff], true), "-_8");
    }

    #[test]
    fn separators_and_uppercase_apply_only_where_meaningful() {
        let mut o = seeded(4, 1, "hex");
        o.separator = "colon".into();
        let colon = run(&o).unwrap().lines().next().unwrap().to_string();
        assert_eq!(colon.matches(':').count(), 3, "3 gaps between 4 bytes: {colon}");
        assert!(run(&o).unwrap().contains("colon-separated"));

        o.uppercase = true;
        let upper = run(&o).unwrap().lines().next().unwrap().to_string();
        assert_eq!(upper, colon.to_ascii_uppercase());

        // base64 has no per-byte boundary, so a separator is ignored rather
        // than corrupting the encoding.
        let mut b = seeded(4, 1, "base64");
        b.separator = "dash".into();
        b.uppercase = true;
        let plain = run(&seeded(4, 1, "base64")).unwrap();
        assert_eq!(run(&b).unwrap(), plain);

        // binary defaults to one space per byte boundary; none removes them.
        let mut bin = seeded(2, 1, "binary");
        assert_eq!(run(&bin).unwrap().lines().next().unwrap().len(), 17);
        bin.separator = "none".into();
        assert_eq!(run(&bin).unwrap().lines().next().unwrap().len(), 16);
    }

    #[test]
    fn json_output_reports_the_batch() {
        let mut o = seeded(8, 3, "base64url");
        o.output = "json".into();
        let v: serde_json::Value = serde_json::from_str(&run(&o).unwrap()).unwrap();
        assert_eq!(v["count"], 3);
        assert_eq!(v["bytes"], 8);
        assert_eq!(v["bits"], 64);
        assert_eq!(v["encoding"], "base64url");
        assert_eq!(v["deterministic"], true);
        assert_eq!(v["values"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn count_draws_independent_values() {
        let out = run(&seeded(16, 5, "hex")).unwrap();
        let values: Vec<&str> = out.lines().take(5).collect();
        assert_eq!(values.len(), 5);
        for i in 0..5 {
            for j in (i + 1)..5 {
                assert_ne!(values[i], values[j], "values must be independent draws");
            }
        }
        assert!(out.contains("5 values · 16 bytes (128 bits) each"));
    }

    #[test]
    fn boundaries_are_accepted_and_one_over_is_rejected() {
        assert!(run(&seeded(MAX_BYTES, 1, "hex")).is_ok());
        assert!(run(&seeded(1, MAX_COUNT, "hex")).is_ok());
        assert!(run(&seeded(MAX_TOTAL_BYTES / MAX_COUNT, MAX_COUNT, "hex")).is_ok());

        let e = run(&seeded(MAX_BYTES + 1, 1, "hex")).unwrap_err();
        assert!(e.contains("bytes must be between 1 and 4096"), "{e}");
        let e = run(&seeded(1, MAX_COUNT + 1, "hex")).unwrap_err();
        assert!(e.contains("count must be between 1 and 100"), "{e}");
        let e = run(&seeded(0, 1, "hex")).unwrap_err();
        assert!(e.contains("bytes must be between 1 and 4096"), "{e}");
    }

    #[test]
    fn total_byte_cap_rejects_big_times_big() {
        let e = run(&seeded(4096, 100, "hex")).unwrap_err();
        assert!(
            e.contains("at most 8192 random bytes per run") && e.contains("409600"),
            "{e}"
        );
    }

    #[test]
    fn unknown_names_and_bad_seeds_are_rejected_by_name() {
        let e = run(&seeded(4, 1, "base58")).unwrap_err();
        assert!(e.contains("unknown encoding \"base58\""), "{e}");

        let mut o = seeded(4, 1, "hex");
        o.separator = "pipe".into();
        assert!(run(&o).unwrap_err().contains("unknown separator \"pipe\""));

        let mut o = seeded(4, 1, "hex");
        o.output = "yaml".into();
        assert!(run(&o).unwrap_err().contains("unknown output \"yaml\""));

        let mut o = seeded(4, 1, "hex");
        o.seed_hex = "abc".into();
        assert!(run(&o).unwrap_err().contains("even number of hex digits"));
        o.seed_hex = "ab".into();
        assert!(run(&o).unwrap_err().contains("8-128 hex digits"));
        o.seed_hex = "zzzzzzzz".into();
        assert!(run(&o).unwrap_err().contains("not valid hex"));
        // Whitespace inside a pasted seed is ignored.
        o.seed_hex = "0011 2233 4455 6677".into();
        assert!(run(&o).is_ok());
    }

    #[test]
    fn aliases_are_accepted_on_every_enum() {
        for (name, want) in [
            ("HEX", Encoding::Hex),
            ("b64", Encoding::Base64),
            ("base64-url", Encoding::Base64Url),
            ("bits", Encoding::Binary),
            ("byte-array", Encoding::Decimal),
            ("c_array", Encoding::CArray),
            ("python", Encoding::PythonBytes),
        ] {
            assert_eq!(Encoding::parse(name).unwrap(), want, "{name}");
        }
        assert_eq!(Separator::parse("HYPHEN").unwrap(), Separator::Dash);
        assert_eq!(Separator::parse("").unwrap(), Separator::Auto);
    }
}
