//! gizza-ai/one-time-pad core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! A one-time pad is only a one-time pad if the key material is truly random,
//! at least as long as the message, and used exactly once. This core enforces
//! the length rule (a short pad is a hard error — the pad is never repeated the
//! way a Vigenère keyword or `blocks/xor-cipher`'s key is) and generates pad
//! material from the platform CSPRNG via `getrandom` (WASI `random_get` on
//! wasm32-wasip1 in the wafer runtime; getrandom's `js` backend — WebCrypto —
//! on the page's wasm32-unknown-unknown build). Letter/digit pad symbols are
//! drawn with rejection sampling so there is no modulo bias.
//!
//! Encryption and decryption are fully deterministic given a supplied pad; only
//! pad generation is random.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::json;

/// Cap on pad symbols (letters, digits or bytes) handled in one call.
const MAX_LEN: usize = 16384;

// ---------------------------------------------------------------------------
// Parameter enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Encrypt,
    Decrypt,
    GeneratePad,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "encrypt" | "enc" => Ok(Self::Encrypt),
            "decrypt" | "dec" => Ok(Self::Decrypt),
            "generate-pad" | "generate_pad" | "generate" | "pad" => Ok(Self::GeneratePad),
            other => Err(format!(
                "unknown mode '{other}' (use encrypt, decrypt, or generate-pad)"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Encrypt => "encrypt",
            Self::Decrypt => "decrypt",
            Self::GeneratePad => "generate-pad",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cipher {
    /// `C = (P + K) mod 26` over A-Z, case and non-letters preserved.
    Letters,
    /// `C = (P + K) mod 10` over 0-9, non-digits preserved.
    Digits,
    /// Bytewise XOR over the UTF-8 bytes of the message.
    Xor,
}

impl Cipher {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "letters" | "letter" | "alpha" | "vigenere" => Ok(Self::Letters),
            "digits" | "digit" | "numeric" | "numbers" => Ok(Self::Digits),
            "xor" | "bytes" | "binary" | "vernam" => Ok(Self::Xor),
            other => Err(format!(
                "unknown cipher '{other}' (use letters, digits, or xor)"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Letters => "letters",
            Self::Digits => "digits",
            Self::Xor => "xor",
        }
    }

    /// Plural noun for one unit of pad material, used in messages.
    fn unit(self) -> &'static str {
        match self {
            Self::Letters => "letters",
            Self::Digits => "digits",
            Self::Xor => "bytes",
        }
    }

    /// What the cipher actually transforms, used in messages.
    fn covers(self) -> &'static str {
        match self {
            Self::Letters => "A-Z letters",
            Self::Digits => "0-9 digits",
            Self::Xor => "bytes",
        }
    }

    /// Alphabet size for the modular ciphers.
    fn modulus(self) -> u16 {
        match self {
            Self::Letters => 26,
            Self::Digits => 10,
            Self::Xor => 256,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" | "base16" => Ok(Self::Hex),
            "base64" | "b64" => Ok(Self::Base64),
            other => Err(format!("unknown encoding '{other}' (use hex or base64)")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Hex => "hex",
            Self::Base64 => "base64",
        }
    }

    fn encode(self, bytes: &[u8]) -> String {
        match self {
            Self::Hex => hex::encode(bytes),
            Self::Base64 => B64.encode(bytes),
        }
    }

    fn decode(self, what: &str, s: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Hex => {
                let trimmed = s.trim();
                let body = trimmed
                    .strip_prefix("0x")
                    .or_else(|| trimmed.strip_prefix("0X"))
                    .unwrap_or(trimmed);
                let cleaned: String = body.chars().filter(|c| !c.is_whitespace()).collect();
                hex::decode(cleaned).map_err(|e| format!("invalid hex {what}: {e}"))
            }
            Self::Base64 => {
                let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
                B64.decode(cleaned)
                    .map_err(|e| format!("invalid base64 {what}: {e}"))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Randomness
// ---------------------------------------------------------------------------

/// Uniform random value in `0..n` via rejection sampling (no modulo bias).
fn rand_below(n: u16) -> Result<u16, String> {
    let nn = u64::from(n);
    let zone = (u64::from(u32::MAX) + 1) / nn * nn; // largest multiple of n <= 2^32
    loop {
        let mut b = [0u8; 4];
        getrandom::getrandom(&mut b).map_err(|e| format!("RNG error: {e}"))?;
        let v = u64::from(u32::from_le_bytes(b));
        if v < zone {
            return Ok((v % nn) as u16);
        }
    }
}

/// `len` fresh pad symbols in `0..cipher.modulus()`.
fn random_symbols(len: usize, cipher: Cipher) -> Result<Vec<u16>, String> {
    if cipher == Cipher::Xor {
        let mut bytes = vec![0u8; len];
        getrandom::getrandom(&mut bytes).map_err(|e| format!("RNG error: {e}"))?;
        return Ok(bytes.into_iter().map(u16::from).collect());
    }
    let m = cipher.modulus();
    (0..len).map(|_| rand_below(m)).collect()
}

// ---------------------------------------------------------------------------
// Pad parsing / rendering
// ---------------------------------------------------------------------------

/// Parse a supplied pad into symbol values. Whitespace (including the spaces
/// this tool emits when `group` is set) is ignored; anything else outside the
/// alphabet is rejected rather than silently dropped, because a mis-read pad
/// character silently shifts every symbol after it.
fn parse_pad(pad: &str, cipher: Cipher, encoding: Encoding) -> Result<Vec<u16>, String> {
    if cipher == Cipher::Xor {
        return Ok(encoding
            .decode("pad", pad)?
            .into_iter()
            .map(u16::from)
            .collect());
    }
    let mut out = Vec::new();
    for ch in pad.chars() {
        if ch.is_whitespace() {
            continue;
        }
        let v = match cipher {
            Cipher::Letters if ch.is_ascii_alphabetic() => {
                u16::from(ch.to_ascii_uppercase() as u8 - b'A')
            }
            Cipher::Digits if ch.is_ascii_digit() => u16::from(ch as u8 - b'0'),
            _ => {
                return Err(format!(
                    "pad contains '{ch}', which is not one of the {} the {} cipher uses (spaces are ignored)",
                    cipher.covers(),
                    cipher.name()
                ))
            }
        };
        out.push(v);
    }
    Ok(out)
}

/// Render pad symbols back into the cipher's own alphabet.
fn render_pad(symbols: &[u16], cipher: Cipher, encoding: Encoding) -> String {
    match cipher {
        Cipher::Letters => symbols
            .iter()
            .map(|v| (b'A' + *v as u8) as char)
            .collect(),
        Cipher::Digits => symbols.iter().map(|v| (b'0' + *v as u8) as char).collect(),
        Cipher::Xor => {
            let bytes: Vec<u8> = symbols.iter().map(|v| *v as u8).collect();
            encoding.encode(&bytes)
        }
    }
}

/// Split `s` into fixed-size groups separated by a single space. Existing
/// whitespace is dropped first so re-grouping an already-grouped string works.
/// `n == 0` keeps the original layout.
fn group_text(s: &str, n: usize) -> String {
    if n == 0 {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + s.len() / n + 1);
    let mut count = 0usize;
    for ch in s.chars().filter(|c| !c.is_whitespace()) {
        if count > 0 && count % n == 0 {
            out.push(' ');
        }
        out.push(ch);
        count += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Modular ciphers
// ---------------------------------------------------------------------------

/// How many pad symbols `message` consumes under `cipher`. Characters outside
/// the alphabet (spaces, punctuation, and — for `digits` — letters) are passed
/// through untouched and consume nothing.
fn pad_demand(message: &str, cipher: Cipher) -> usize {
    match cipher {
        Cipher::Letters => message.chars().filter(char::is_ascii_alphabetic).count(),
        Cipher::Digits => message.chars().filter(char::is_ascii_digit).count(),
        Cipher::Xor => message.len(),
    }
}

/// Apply the modular cipher, consuming one pad symbol per in-alphabet
/// character. `add` selects encryption (`P + K`) or decryption (`P - K`).
fn modular(message: &str, pad: &[u16], cipher: Cipher, add: bool) -> String {
    let m = cipher.modulus();
    let mut out = String::with_capacity(message.len());
    let mut i = 0usize;
    for ch in message.chars() {
        let base = match cipher {
            Cipher::Letters if ch.is_ascii_uppercase() => b'A',
            Cipher::Letters if ch.is_ascii_lowercase() => b'a',
            Cipher::Digits if ch.is_ascii_digit() => b'0',
            _ => {
                out.push(ch);
                continue;
            }
        };
        let p = u16::from(ch as u8 - base);
        let k = pad[i];
        i += 1;
        let v = if add { (p + k) % m } else { (p + m - k) % m };
        out.push((base + v as u8) as char);
    }
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the tool. `message` is the plaintext when encrypting and the ciphertext
/// when decrypting; `pad` is the key material (empty in `encrypt` means "make
/// me a fresh one"). `length` only applies to `generate-pad` (0 = size it from
/// `message`). `group` splits the emitted ciphertext/pad into blocks of that
/// many characters (0 = leave the layout alone). Returns a pretty JSON object.
pub fn run(
    mode: &str,
    cipher: &str,
    message: &str,
    pad: &str,
    encoding: &str,
    length: usize,
    group: usize,
) -> Result<String, String> {
    let mode = Mode::parse(mode)?;
    let cipher = Cipher::parse(cipher)?;
    let encoding = Encoding::parse(encoding)?;
    if group > 20 {
        return Err(format!("group must be between 0 and 20 (got {group})"));
    }
    if length > MAX_LEN {
        return Err(format!("length must be at most {MAX_LEN} (got {length})"));
    }

    let mut warnings: Vec<String> = Vec::new();
    let out = match mode {
        Mode::GeneratePad => generate_pad(cipher, encoding, message, length, group)?,
        Mode::Encrypt | Mode::Decrypt => {
            transform(mode, cipher, encoding, message, pad, group, &mut warnings)?
        }
    };
    let mut obj = out;
    obj["warnings"] = json!(warnings);
    serde_json::to_string_pretty(&obj).map_err(|e| format!("could not serialize result: {e}"))
}

fn generate_pad(
    cipher: Cipher,
    encoding: Encoding,
    message: &str,
    length: usize,
    group: usize,
) -> Result<serde_json::Value, String> {
    let len = if length > 0 {
        length
    } else {
        let derived = pad_demand(message, cipher);
        if derived == 0 {
            return Err(format!(
                "generate-pad needs a length: set length, or supply a message containing {} to size the pad from",
                cipher.covers()
            ));
        }
        derived
    };
    if len > MAX_LEN {
        return Err(format!(
            "pad length {len} is over the {MAX_LEN}-{} limit",
            if cipher == Cipher::Xor { "byte" } else { "symbol" }
        ));
    }
    let symbols = random_symbols(len, cipher)?;
    let mut obj = json!({
        "mode": Mode::GeneratePad.name(),
        "cipher": cipher.name(),
        "pad": group_text(&render_pad(&symbols, cipher, encoding), group),
        "pad_length": len,
    });
    if cipher == Cipher::Xor {
        obj["encoding"] = json!(encoding.name());
    }
    Ok(obj)
}

fn transform(
    mode: Mode,
    cipher: Cipher,
    encoding: Encoding,
    message: &str,
    pad: &str,
    group: usize,
    warnings: &mut Vec<String>,
) -> Result<serde_json::Value, String> {
    if message.is_empty() {
        return Err(match mode {
            Mode::Decrypt => "message is required: paste the ciphertext to decrypt".into(),
            _ => "message is required: type the text to encrypt".to_string(),
        });
    }

    // XOR works on bytes, so the ciphertext is encoded rather than plain text.
    let working: String = if cipher == Cipher::Xor && mode == Mode::Decrypt {
        let bytes = encoding.decode("message", message)?;
        // Latin-1-style passthrough: each byte becomes one char so the shared
        // modular/XOR path below is byte-for-byte reversible.
        bytes.iter().map(|b| *b as char).collect()
    } else {
        message.to_string()
    };

    let demand = if cipher == Cipher::Xor {
        if mode == Mode::Decrypt {
            working.chars().count()
        } else {
            working.len()
        }
    } else {
        pad_demand(&working, cipher)
    };
    if demand == 0 {
        return Err(format!(
            "message contains no {} — the {} cipher only transforms {}",
            cipher.unit(),
            cipher.name(),
            cipher.covers()
        ));
    }
    if demand > MAX_LEN {
        return Err(format!(
            "message needs {demand} pad {}, over the {MAX_LEN} limit",
            cipher.unit()
        ));
    }

    // An empty pad while encrypting means "generate one for this message".
    let pad_generated = pad.trim().is_empty() && mode == Mode::Encrypt;
    let symbols = if pad_generated {
        random_symbols(demand, cipher)?
    } else if pad.trim().is_empty() {
        return Err("pad is required to decrypt: paste the pad used to encrypt".into());
    } else {
        parse_pad(pad, cipher, encoding)?
    };

    if symbols.len() < demand {
        return Err(format!(
            "pad too short: message needs {demand} pad {}, pad has {}",
            cipher.unit(),
            symbols.len()
        ));
    }
    if symbols.len() > demand {
        warnings.push(format!(
            "pad is longer than the message: only the first {demand} {} were used, the remaining {} were ignored. Never reuse leftover pad material for another message.",
            cipher.unit(),
            symbols.len() - demand
        ));
    }
    let used = &symbols[..demand];

    if pad_generated {
        warnings.push(
            "A fresh pad was generated for this message. Store it securely, share it out of band, and never use it again — reusing a pad breaks the cipher completely."
                .to_string(),
        );
    }
    if cipher != Cipher::Xor && working.chars().any(|c| c.is_alphanumeric() && !c.is_ascii()) {
        warnings.push(format!(
            "non-ASCII characters were passed through unencrypted — the {} cipher only transforms {}.",
            cipher.name(),
            cipher.covers()
        ));
    }

    let mut obj = json!({
        "mode": mode.name(),
        "cipher": cipher.name(),
        "pad_length": demand,
    });
    if cipher == Cipher::Xor {
        obj["encoding"] = json!(encoding.name());
    }

    match (cipher, mode) {
        (Cipher::Xor, Mode::Encrypt) => {
            let bytes: Vec<u8> = working
                .as_bytes()
                .iter()
                .zip(used)
                .map(|(b, k)| b ^ *k as u8)
                .collect();
            obj["ciphertext"] = json!(group_text(&encoding.encode(&bytes), group));
        }
        (Cipher::Xor, Mode::Decrypt) => {
            let bytes: Vec<u8> = working
                .chars()
                .zip(used)
                .map(|(c, k)| c as u8 ^ *k as u8)
                .collect();
            obj["plaintext"] = json!(String::from_utf8(bytes).map_err(|_| {
                "decrypted bytes are not valid UTF-8 text — check that the pad and the encoding match the ones used to encrypt".to_string()
            })?);
        }
        (_, Mode::Encrypt) => {
            obj["ciphertext"] = json!(group_text(&modular(&working, used, cipher, true), group));
        }
        (_, _) => {
            obj["plaintext"] = json!(modular(&working, used, cipher, false));
        }
    }

    if mode == Mode::Encrypt {
        obj["pad"] = json!(group_text(&render_pad(used, cipher, encoding), group));
        obj["pad_generated"] = json!(pad_generated);
    }
    Ok(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(mode: &str, cipher: &str, msg: &str, pad: &str) -> serde_json::Value {
        let s = run(mode, cipher, msg, pad, "hex", 0, 0).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    fn field(v: &serde_json::Value, k: &str) -> String {
        v[k].as_str().unwrap_or_default().to_string()
    }

    #[test]
    fn canonical_letters_vector() {
        let v = call("encrypt", "letters", "HELLO", "XMCKA");
        assert_eq!(field(&v, "ciphertext"), "EQNVO");
        assert_eq!(field(&v, "pad"), "XMCKA");
        assert_eq!(v["pad_generated"], json!(false));
        assert_eq!(v["warnings"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn letters_decrypt_is_the_inverse() {
        let v = call("decrypt", "letters", "EQNVO", "XMCKA");
        assert_eq!(field(&v, "plaintext"), "HELLO");
    }

    #[test]
    fn letters_preserve_case_and_non_letters_consume_no_pad() {
        let v = call("encrypt", "letters", "Hi, Bob!", "XMCKAZ");
        // 5 letters -> 5 pad symbols; ", " "!" pass through, case preserved.
        assert_eq!(v["pad_length"], json!(5));
        let ct = field(&v, "ciphertext");
        assert_eq!(ct.len(), "Hi, Bob!".len());
        assert!(ct.starts_with("Eu, "), "{ct}");
        assert!(ct.ends_with('!'), "{ct}");
        assert!(ct.chars().next().unwrap().is_ascii_uppercase());
        // The extra pad letter is reported, not used.
        assert_eq!(field(&v, "pad"), "XMCKA");
        assert!(v["warnings"][0]
            .as_str()
            .unwrap()
            .contains("the remaining 1 were ignored"));
        let back = call("decrypt", "letters", &ct, "XMCKA");
        assert_eq!(field(&back, "plaintext"), "Hi, Bob!");
    }

    #[test]
    fn digits_are_mod_10_and_skip_non_digits() {
        let v = call("encrypt", "digits", "12-34", "9876");
        assert_eq!(v["pad_length"], json!(4));
        // 1+9=0, 2+8=0, 3+7=0, 4+6=0 (mod 10), dash preserved.
        assert_eq!(field(&v, "ciphertext"), "00-00");
        let back = call("decrypt", "digits", "00-00", "9876");
        assert_eq!(field(&back, "plaintext"), "12-34");
    }

    #[test]
    fn xor_hex_roundtrip() {
        let ct = call("encrypt", "xor", "Attack at dawn", "");
        let hex_ct = field(&ct, "ciphertext");
        let pad = field(&ct, "pad");
        assert_eq!(ct["encoding"], json!("hex"));
        assert_eq!(ct["pad_generated"], json!(true));
        assert_eq!(hex_ct.len(), "Attack at dawn".len() * 2);
        assert_eq!(pad.len(), "Attack at dawn".len() * 2);
        let back = call("decrypt", "xor", &hex_ct, &pad);
        assert_eq!(field(&back, "plaintext"), "Attack at dawn");
    }

    #[test]
    fn xor_base64_roundtrip_and_known_vector() {
        let s = run("encrypt", "xor", "Hi", "0000", "hex", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(field(&v, "ciphertext"), "4869"); // pad of zero bytes is a no-op
        let s = run("encrypt", "xor", "Hi", "AAA=", "base64", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["encoding"], json!("base64"));
        let ct = field(&v, "ciphertext");
        let s = run("decrypt", "xor", &ct, "AAA=", "base64", 0, 0).unwrap();
        let back: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(field(&back, "plaintext"), "Hi");
    }

    #[test]
    fn xor_covers_multibyte_utf8() {
        let msg = "héllo ✓";
        let ct = call("encrypt", "xor", msg, "");
        assert_eq!(ct["pad_length"], json!(msg.len()));
        let back = call("decrypt", "xor", &field(&ct, "ciphertext"), &field(&ct, "pad"));
        assert_eq!(field(&back, "plaintext"), msg);
    }

    #[test]
    fn auto_generated_pads_are_message_sized_and_roundtrip() {
        for (cipher, msg) in [("letters", "MEET ME AT NOON"), ("digits", "5551234")] {
            let v = call("encrypt", cipher, msg, "");
            let pad = field(&v, "pad");
            let ct = field(&v, "ciphertext");
            assert_eq!(v["pad_generated"], json!(true));
            assert_eq!(pad.chars().count(), pad_demand(msg, Cipher::parse(cipher).unwrap()));
            assert!(v["warnings"][0].as_str().unwrap().contains("never use it again"));
            let back = call("decrypt", cipher, &ct, &pad);
            assert_eq!(field(&back, "plaintext"), msg);
        }
    }

    #[test]
    fn generated_pads_differ_between_runs() {
        let a = run("generate-pad", "letters", "", "", "hex", 64, 0).unwrap();
        let b = run("generate-pad", "letters", "", "", "hex", 64, 0).unwrap();
        assert_ne!(a, b, "a pad must never repeat");
    }

    #[test]
    fn generate_pad_shapes() {
        // Explicit length.
        let s = run("generate-pad", "letters", "", "", "hex", 10, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["pad_length"], json!(10));
        assert_eq!(field(&v, "pad").len(), 10);
        assert!(field(&v, "pad").chars().all(|c| c.is_ascii_uppercase()));
        assert!(v.get("encoding").is_none(), "letters pads are not encoded");

        // Length derived from the message.
        let v = call("generate-pad", "digits", "phone 555-1234", "");
        assert_eq!(v["pad_length"], json!(7));
        assert!(field(&v, "pad").chars().all(|c| c.is_ascii_digit()));

        // XOR pads are encoded and report the encoding.
        let s = run("generate-pad", "xor", "", "", "base64", 12, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["encoding"], json!("base64"));
        assert_eq!(v["pad_length"], json!(12));
        assert_eq!(B64.decode(field(&v, "pad")).unwrap().len(), 12);
    }

    #[test]
    fn grouping_applies_to_ciphertext_and_pad() {
        let s = run("encrypt", "letters", "HELLOWORLD", "XMCKAXMCKA", "hex", 0, 5).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(field(&v, "ciphertext"), "EQNVO TATVD");
        assert_eq!(field(&v, "pad"), "XMCKA XMCKA");
        // A grouped ciphertext feeds straight back in: spaces are non-letters
        // on the way in and simply reappear in the plaintext.
        let s = run("decrypt", "letters", "EQNVO TATVD", "XMCKA XMCKA", "hex", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(field(&v, "plaintext"), "HELLO WORLD");
        // group = 0 leaves the layout alone.
        let s = run("encrypt", "letters", "HELLOWORLD", "XMCKAXMCKA", "hex", 0, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(field(&v, "ciphertext"), "EQNVOTATVD");
    }

    #[test]
    fn short_pad_is_a_hard_error_never_repeated() {
        let err = run("encrypt", "letters", "ATTACK AT DAWN", "XMCKA", "hex", 0, 0).unwrap_err();
        assert_eq!(
            err,
            "pad too short: message needs 12 pad letters, pad has 5"
        );
        assert!(run("encrypt", "digits", "123456", "99", "hex", 0, 0)
            .unwrap_err()
            .contains("pad too short"));
        assert!(run("encrypt", "xor", "hello", "00ff", "hex", 0, 0)
            .unwrap_err()
            .contains("pad too short: message needs 5 pad bytes, pad has 2"));
    }

    #[test]
    fn non_ascii_letters_are_flagged() {
        let v = call("encrypt", "letters", "café", "XMCKA");
        assert!(
            v["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w.as_str().unwrap().contains("non-ASCII")),
            "{v}"
        );
    }

    #[test]
    fn parse_helpers_accept_aliases() {
        assert_eq!(Mode::parse("").unwrap(), Mode::Encrypt);
        assert_eq!(Mode::parse("GENERATE-PAD").unwrap(), Mode::GeneratePad);
        assert_eq!(Cipher::parse("").unwrap(), Cipher::Letters);
        assert_eq!(Cipher::parse("Bytes").unwrap(), Cipher::Xor);
        assert_eq!(Encoding::parse("").unwrap(), Encoding::Hex);
        assert_eq!(Encoding::parse("b64").unwrap(), Encoding::Base64);
    }

    #[test]
    fn errors() {
        // Unknown enum values.
        assert!(run("shred", "letters", "a", "b", "hex", 0, 0).is_err());
        assert!(run("encrypt", "rot13", "a", "b", "hex", 0, 0).is_err());
        assert!(run("encrypt", "xor", "a", "00", "octal", 0, 0).is_err());
        // Bounds.
        assert!(run("encrypt", "letters", "A", "B", "hex", 0, 21)
            .unwrap_err()
            .contains("group must be between 0 and 20"));
        assert!(run("generate-pad", "letters", "", "", "hex", 99999, 0).is_err());
        // Missing inputs.
        assert!(run("encrypt", "letters", "", "", "hex", 0, 0)
            .unwrap_err()
            .contains("message is required"));
        assert!(run("decrypt", "letters", "EQNVO", "", "hex", 0, 0)
            .unwrap_err()
            .contains("pad is required"));
        assert!(run("generate-pad", "letters", "", "", "hex", 0, 0)
            .unwrap_err()
            .contains("generate-pad needs a length"));
        // Nothing in the alphabet to work on.
        assert!(run("encrypt", "digits", "no digits here", "1234", "hex", 0, 0)
            .unwrap_err()
            .contains("contains no digits"));
        // Malformed pads.
        assert!(run("encrypt", "letters", "HELLO", "XM9KA", "hex", 0, 0)
            .unwrap_err()
            .contains("pad contains '9'"));
        assert!(run("encrypt", "digits", "12345", "1234X", "hex", 0, 0)
            .unwrap_err()
            .contains("pad contains 'X'"));
        assert!(run("encrypt", "xor", "hi", "zz", "hex", 0, 0)
            .unwrap_err()
            .contains("invalid hex pad"));
        assert!(run("decrypt", "xor", "zz", "0000", "hex", 0, 0)
            .unwrap_err()
            .contains("invalid hex message"));
        // XOR decrypt with the wrong pad usually lands outside UTF-8.
        assert!(run("decrypt", "xor", "ffffffff", "00000000", "hex", 0, 0)
            .unwrap_err()
            .contains("not valid UTF-8"));
    }
}
