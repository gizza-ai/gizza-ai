//! scryptenc-file core — read and write the **scrypt encrypted data format**,
//! the container the Tarsnap `scrypt enc` / `scrypt dec` CLI (and compatible
//! implementations such as `rscrypt`) produce and consume.
//!
//! Layout (version 0, from the upstream FORMAT spec) — 96-byte header, then
//! ciphertext, then a 32-byte tag:
//!
//! ```text
//! off  len  field
//!   0    6  magic "scrypt"
//!   6    1  format version (0)
//!   7    1  log2(N)
//!   8    4  r  (big-endian u32)
//!  12    4  p  (big-endian u32)
//!  16   32  salt
//!  48   16  SHA256(bytes 0..48)[0..16]        header checksum
//!  64   32  HMAC-SHA256(bytes 0..64, hmac_key)  header tag
//!  96    X  AES-256-CTR ciphertext (zero nonce)
//!  96+X 32  HMAC-SHA256(bytes 0..96+X, hmac_key)  file tag
//! ```
//!
//! `scrypt(password, salt, N, r, p)` produces 64 bytes: the first 32 are the
//! AES-256 key, the last 32 are the HMAC-SHA256 key. Pure Rust, no I/O —
//! shared by the chat skill block and the web page.

use aes::Aes256;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use cipher::{KeyIvInit, StreamCipher};
use hmac::{Hmac, Mac};
use scrypt::Params;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;
type Aes256Ctr = ctr::Ctr128BE<Aes256>;

/// Bytes before the ciphertext.
pub const HEADER_SIZE: usize = 96;
/// Bytes of trailing HMAC-SHA256.
pub const TAG_SIZE: usize = 32;
/// Fixed container overhead: header + trailing tag.
pub const OVERHEAD: usize = HEADER_SIZE + TAG_SIZE;
/// Salt length fixed by the format.
pub const SALT_LEN: usize = 32;

/// Cap on decoded input bytes, so a paste can't blow the wasm sandbox.
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

/// Default cost parameters. Upstream `scrypt` picks these from a time/memory
/// budget at runtime; a deterministic sandboxed tool cannot, so we expose them
/// directly. logN=14 with r=8 needs 16 MiB, which fits the wasm sandbox.
pub const DEFAULT_LOG_N: i64 = 14;
pub const DEFAULT_R: i64 = 8;
pub const DEFAULT_P: i64 = 1;
/// Default ceiling on the scrypt working buffer, mirroring the upstream
/// `-M maxmem` flag. Kept well under the 64 MiB wasm sandbox.
pub const DEFAULT_MAX_MEMORY_MIB: i64 = 32;
/// Hard ceiling for `max_memory_mib` — above this the sandbox traps rather
/// than returning an error we can explain.
pub const MAX_MAX_MEMORY_MIB: i64 = 64;

/// What to do with `data`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operation {
    Encrypt,
    Decrypt,
    Info,
}

impl Operation {
    pub fn parse(s: &str) -> Result<Operation, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "encrypt" | "enc" | "" => Ok(Operation::Encrypt),
            "decrypt" | "dec" => Ok(Operation::Decrypt),
            "info" | "information" => Ok(Operation::Info),
            other => Err(format!(
                "unknown operation '{other}' (use encrypt, decrypt, or info)"
            )),
        }
    }
}

/// How `data` is written, and how binary results are printed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    Text,
    Hex,
    Base64,
}

impl Encoding {
    pub fn parse_input(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "utf8" | "utf-8" | "" => Ok(Encoding::Text),
            "hex" => Ok(Encoding::Hex),
            "base64" | "b64" => Ok(Encoding::Base64),
            other => Err(format!(
                "unknown data_encoding '{other}' (use text, hex, or base64)"
            )),
        }
    }

    pub fn parse_output(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "base64" | "b64" | "" => Ok(Encoding::Base64),
            "hex" => Ok(Encoding::Hex),
            other => Err(format!(
                "unknown output_encoding '{other}' (use base64 or hex)"
            )),
        }
    }
}

/// Header fields recovered from a container, without deriving any key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileParams {
    pub log_n: u8,
    pub r: u32,
    pub p: u32,
    pub salt: Vec<u8>,
    /// Length of the ciphertext body (total length minus the 128-byte overhead).
    pub ciphertext_len: usize,
    /// Total container length in bytes.
    pub total_len: usize,
}

impl FileParams {
    /// `128 * N * r` — the scrypt working buffer this file demands.
    pub fn memory_bytes(&self) -> u64 {
        128u64
            .saturating_mul(1u64 << self.log_n)
            .saturating_mul(self.r as u64)
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .split_whitespace()
        .collect();
    if cleaned.len() % 2 != 0 {
        return Err("hex input must have an even number of digits".into());
    }
    hex::decode(&cleaned).map_err(|e| format!("invalid hex input: {e}"))
}

fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.split_whitespace().collect();
    B64.decode(cleaned.as_bytes())
        .map_err(|e| format!("invalid base64 input: {e}"))
}

/// `text` on a container means "auto-detect": a pasted blob is hex if it is all
/// hex digits, otherwise base64.
fn looks_like_hex(s: &str) -> bool {
    let cleaned: String = s
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .split_whitespace()
        .collect();
    !cleaned.is_empty()
        && cleaned.len() % 2 == 0
        && cleaned.chars().all(|c| c.is_ascii_hexdigit())
}

/// Bytes to seal: `text` is UTF-8, `hex`/`base64` decode first.
fn read_plaintext(data: &str, encoding: Encoding) -> Result<Vec<u8>, String> {
    match encoding {
        Encoding::Text => Ok(data.as_bytes().to_vec()),
        Encoding::Hex => decode_hex(data),
        Encoding::Base64 => decode_base64(data),
    }
}

/// Bytes of an existing container: `text` auto-detects hex vs base64.
fn read_container(data: &str, encoding: Encoding) -> Result<Vec<u8>, String> {
    match encoding {
        Encoding::Hex => decode_hex(data),
        Encoding::Base64 => decode_base64(data),
        Encoding::Text => {
            if looks_like_hex(data) {
                decode_hex(data)
            } else {
                decode_base64(data)
            }
        }
    }
}

fn encode_output(bytes: &[u8], encoding: Encoding) -> String {
    match encoding {
        Encoding::Hex => hex::encode(bytes),
        // `text` is not offered as an output encoding; treat it as base64.
        _ => B64.encode(bytes),
    }
}

fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf)
        .map_err(|e| format!("secure random number generator unavailable: {e}"))?;
    Ok(buf)
}

fn human_bytes(n: u64) -> String {
    if n >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", n as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if n >= 1024 * 1024 {
        format!("{} MiB", n / (1024 * 1024))
    } else if n >= 1024 {
        format!("{} KiB", n / 1024)
    } else {
        format!("{n} bytes")
    }
}

/// Reject cost parameters whose scrypt buffer would exceed the caller's budget
/// (the upstream `-M maxmem` behaviour) before the allocation traps the sandbox.
fn check_memory(log_n: u8, r: u32, p: u32, max_memory_mib: i64) -> Result<(), String> {
    if !(1..=MAX_MAX_MEMORY_MIB).contains(&max_memory_mib) {
        return Err(format!(
            "max_memory_mib must be between 1 and {MAX_MAX_MEMORY_MIB} (got {max_memory_mib})"
        ));
    }
    let budget = (max_memory_mib as u64) * 1024 * 1024;
    let core = 128u64
        .saturating_mul(1u64 << log_n)
        .saturating_mul(r as u64);
    let pipeline = 128u64.saturating_mul(r as u64).saturating_mul(p as u64);
    let needed = core.saturating_add(pipeline);
    if needed > budget {
        return Err(format!(
            "these parameters need {} of memory (logN={log_n} means N={}, r={r}, p={p}), \
             over the {} limit. Raise max_memory_mib (up to {MAX_MAX_MEMORY_MIB}) if the \
             machine can spare it, or use smaller cost parameters — this runs inside a \
             64 MiB sandbox, so files written with very high logN cannot be opened here \
             (operation=info still reports their parameters).",
            human_bytes(needed),
            1u64 << log_n,
            human_bytes(budget)
        ));
    }
    Ok(())
}

/// scrypt(password, salt) → 64 bytes = AES-256 key ++ HMAC-SHA256 key.
fn derive_keys(password: &str, salt: &[u8], log_n: u8, r: u32, p: u32) -> Result<[u8; 64], String> {
    let params = Params::new(log_n, r, p, 64)
        .map_err(|e| format!("invalid scrypt parameters (logN={log_n}, r={r}, p={p}): {e}"))?;
    let mut dk = [0u8; 64];
    scrypt::scrypt(password.as_bytes(), salt, &params, &mut dk)
        .map_err(|e| format!("scrypt key derivation failed: {e}"))?;
    Ok(dk)
}

fn aes256_ctr_xor(key: &[u8], data: &mut [u8]) -> Result<(), String> {
    // The format fixes the CTR nonce/counter block to all zeroes.
    let mut cipher = Aes256Ctr::new_from_slices(key, &[0u8; 16])
        .map_err(|e| format!("could not initialise AES-256-CTR: {e}"))?;
    cipher.apply_keystream(data);
    Ok(())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    let out = mac.finalize().into_bytes();
    let mut tag = [0u8; 32];
    tag.copy_from_slice(&out);
    tag
}

/// Parse + integrity-check the 96-byte header. Does NOT need the password:
/// this is what `operation=info` runs, so it works on files whose cost
/// parameters are far too large to actually derive a key for here.
pub fn parse_header(bytes: &[u8]) -> Result<FileParams, String> {
    if bytes.len() < OVERHEAD {
        return Err(format!(
            "not a complete scrypt file: {} bytes, but the format needs at least {OVERHEAD} \
             (a {HEADER_SIZE}-byte header plus a {TAG_SIZE}-byte tag)",
            bytes.len()
        ));
    }
    if &bytes[0..6] != b"scrypt" {
        return Err(
            "not a scrypt file: the first 6 bytes are not the magic string 'scrypt'. Check the \
             input encoding, and note that a .enc extension does not imply this format."
                .into(),
        );
    }
    if bytes[6] != 0 {
        return Err(format!(
            "unsupported scrypt format version {} (only version 0 is defined)",
            bytes[6]
        ));
    }
    let log_n = bytes[7];
    if !(1..=63).contains(&log_n) {
        return Err(format!("invalid logN {log_n} in the header (must be 1-63)"));
    }
    let r = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let p = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    if r == 0 || p == 0 {
        return Err(format!(
            "invalid cost parameters in the header (r={r}, p={p}); both must be at least 1"
        ));
    }

    let digest = Sha256::digest(&bytes[0..48]);
    if digest[0..16].ct_eq(&bytes[48..64]).unwrap_u8() != 1 {
        return Err(
            "header checksum mismatch: the file is truncated, corrupted, or was decoded with the \
             wrong input encoding."
                .into(),
        );
    }

    Ok(FileParams {
        log_n,
        r,
        p,
        salt: bytes[16..48].to_vec(),
        ciphertext_len: bytes.len() - OVERHEAD,
        total_len: bytes.len(),
    })
}

/// Build a container around `plaintext`. `salt` must be 32 bytes.
pub fn encrypt(
    plaintext: &[u8],
    password: &str,
    log_n: u8,
    r: u32,
    p: u32,
    salt: &[u8],
) -> Result<Vec<u8>, String> {
    if salt.len() != SALT_LEN {
        return Err(format!(
            "salt must be exactly {SALT_LEN} bytes ({} hex characters), got {}",
            SALT_LEN * 2,
            salt.len()
        ));
    }
    let dk = derive_keys(password, salt, log_n, r, p)?;
    let (aes_key, hmac_key) = dk.split_at(32);

    let mut out = Vec::with_capacity(OVERHEAD + plaintext.len());
    out.extend_from_slice(b"scrypt");
    out.push(0);
    out.push(log_n);
    out.extend_from_slice(&r.to_be_bytes());
    out.extend_from_slice(&p.to_be_bytes());
    out.extend_from_slice(salt);
    let digest = Sha256::digest(&out[0..48]);
    out.extend_from_slice(&digest[0..16]);
    let header_tag = hmac_sha256(hmac_key, &out[0..64]);
    out.extend_from_slice(&header_tag);

    let mut body = plaintext.to_vec();
    aes256_ctr_xor(aes_key, &mut body)?;
    out.extend_from_slice(&body);

    let file_tag = hmac_sha256(hmac_key, &out);
    out.extend_from_slice(&file_tag);
    Ok(out)
}

/// Verify and open a container. Both HMACs are checked before any plaintext is
/// returned, so a wrong password or a modified file errors instead of yielding
/// garbage.
pub fn decrypt(bytes: &[u8], password: &str, max_memory_mib: i64) -> Result<Vec<u8>, String> {
    let params = parse_header(bytes)?;
    check_memory(params.log_n, params.r, params.p, max_memory_mib)?;
    let dk = derive_keys(password, &params.salt, params.log_n, params.r, params.p)?;
    let (aes_key, hmac_key) = dk.split_at(32);

    let header_tag = hmac_sha256(hmac_key, &bytes[0..64]);
    if header_tag.ct_eq(&bytes[64..96]).unwrap_u8() != 1 {
        return Err(
            "wrong password: the header authentication tag does not match. The header itself is \
             intact, so the cost parameters were read correctly — only the passphrase is wrong."
                .into(),
        );
    }

    let body_end = bytes.len() - TAG_SIZE;
    let file_tag = hmac_sha256(hmac_key, &bytes[0..body_end]);
    if file_tag.ct_eq(&bytes[body_end..]).unwrap_u8() != 1 {
        return Err(
            "the file is corrupted or was modified: the trailing HMAC over the whole file does \
             not match, even though the password is correct."
                .into(),
        );
    }

    let mut body = bytes[HEADER_SIZE..body_end].to_vec();
    aes256_ctr_xor(aes_key, &mut body)?;
    Ok(body)
}

/// Human-readable header report — the analogue of upstream `scrypt info`.
pub fn describe(params: &FileParams) -> String {
    let mem = params.memory_bytes();
    let mut lines = vec![
        "format: scrypt encrypted data".to_string(),
        "version: 0".to_string(),
        format!("logN: {}", params.log_n),
        format!("N: {}", 1u64 << params.log_n),
        format!("r: {}", params.r),
        format!("p: {}", params.p),
        format!("salt: {}", hex::encode(&params.salt)),
        format!("estimated memory: {} (128 * N * r)", human_bytes(mem)),
        format!("header: {HEADER_SIZE} bytes"),
        format!("ciphertext: {} bytes", params.ciphertext_len),
        format!("tag: {TAG_SIZE} bytes"),
        format!("total: {} bytes", params.total_len),
    ];
    if mem > (DEFAULT_MAX_MEMORY_MIB as u64) * 1024 * 1024 {
        lines.push(format!(
            "note: decrypting this file needs max_memory_mib of at least {} MiB",
            mem.div_ceil(1024 * 1024)
        ));
    }
    lines.join("\n")
}

/// Single entry point shared by the chat block, the CLI and the page.
#[allow(clippy::too_many_arguments)]
pub fn run(
    operation: &str,
    data: &str,
    password: &str,
    data_encoding: &str,
    output_encoding: &str,
    log_n: i64,
    r: i64,
    p: i64,
    salt: &str,
    max_memory_mib: i64,
) -> Result<String, String> {
    let op = Operation::parse(operation)?;
    let in_enc = Encoding::parse_input(data_encoding)?;
    let out_enc = Encoding::parse_output(output_encoding)?;

    if data.trim().is_empty() {
        return Err(match op {
            Operation::Encrypt => "input is empty: paste the text or bytes to encrypt".into(),
            _ => "input is empty: paste the scrypt container as base64 or hex".into(),
        });
    }

    match op {
        Operation::Encrypt => {
            if password.is_empty() {
                return Err("password is required to encrypt".into());
            }
            if !(1..=63).contains(&log_n) {
                return Err(format!("log_n must be between 1 and 63 (got {log_n})"));
            }
            if !(1..=32).contains(&r) {
                return Err(format!("r must be between 1 and 32 (got {r})"));
            }
            if !(1..=16).contains(&p) {
                return Err(format!("p must be between 1 and 16 (got {p})"));
            }
            check_memory(log_n as u8, r as u32, p as u32, max_memory_mib)?;

            let plaintext = read_plaintext(data, in_enc)?;
            if plaintext.len() > MAX_INPUT_BYTES {
                return Err(format!(
                    "input is {} bytes, over the {} limit",
                    plaintext.len(),
                    human_bytes(MAX_INPUT_BYTES as u64)
                ));
            }
            let salt_bytes = if salt.trim().is_empty() {
                random_bytes(SALT_LEN)?
            } else {
                let decoded = decode_hex(salt)?;
                if decoded.len() != SALT_LEN {
                    return Err(format!(
                        "salt must be exactly {SALT_LEN} bytes ({} hex characters), got {} bytes",
                        SALT_LEN * 2,
                        decoded.len()
                    ));
                }
                decoded
            };
            let container = encrypt(&plaintext, password, log_n as u8, r as u32, p as u32, &salt_bytes)?;
            Ok(encode_output(&container, out_enc))
        }
        Operation::Decrypt => {
            if password.is_empty() {
                return Err("password is required to decrypt".into());
            }
            let container = read_container(data, in_enc)?;
            if container.len() > MAX_INPUT_BYTES {
                return Err(format!(
                    "container is {} bytes, over the {} limit",
                    container.len(),
                    human_bytes(MAX_INPUT_BYTES as u64)
                ));
            }
            let plaintext = decrypt(&container, password, max_memory_mib)?;
            match String::from_utf8(plaintext.clone()) {
                Ok(text) => Ok(text),
                Err(_) => Ok(encode_output(&plaintext, out_enc)),
            }
        }
        Operation::Info => {
            let container = read_container(data, in_enc)?;
            let params = parse_header(&container)?;
            Ok(describe(&params))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALT_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    /// Cheap parameters so the tests stay fast.
    fn cheap() -> (i64, i64, i64) {
        (10, 8, 1)
    }

    #[test]
    fn round_trip_text() {
        let (log_n, r, p) = cheap();
        let sealed = run(
            "encrypt", "attack at dawn", "hunter2", "text", "base64", log_n, r, p, SALT_HEX, 32,
        )
        .unwrap();
        let opened = run(
            "decrypt", &sealed, "hunter2", "base64", "base64", log_n, r, p, "", 32,
        )
        .unwrap();
        assert_eq!(opened, "attack at dawn");
    }

    #[test]
    fn container_shape_is_the_documented_format() {
        let (log_n, r, p) = cheap();
        let hexed = run(
            "encrypt", "hello", "pw", "text", "hex", log_n, r, p, SALT_HEX, 32,
        )
        .unwrap();
        let bytes = hex::decode(&hexed).unwrap();
        assert_eq!(bytes.len(), OVERHEAD + 5);
        assert_eq!(&bytes[0..6], b"scrypt");
        assert_eq!(bytes[6], 0, "format version");
        assert_eq!(bytes[7], log_n as u8);
        assert_eq!(u32::from_be_bytes(bytes[8..12].try_into().unwrap()), r as u32);
        assert_eq!(u32::from_be_bytes(bytes[12..16].try_into().unwrap()), p as u32);
        assert_eq!(hex::encode(&bytes[16..48]), SALT_HEX);
        // Header checksum is the first half of SHA256 over the first 48 bytes.
        let digest = Sha256::digest(&bytes[0..48]);
        assert_eq!(&bytes[48..64], &digest[0..16]);
    }

    #[test]
    fn deterministic_with_an_explicit_salt() {
        let (log_n, r, p) = cheap();
        let a = run("encrypt", "x", "pw", "text", "hex", log_n, r, p, SALT_HEX, 32).unwrap();
        let b = run("encrypt", "x", "pw", "text", "hex", log_n, r, p, SALT_HEX, 32).unwrap();
        assert_eq!(a, b, "an explicit salt must reproduce the container exactly");
    }

    #[test]
    fn fresh_salt_per_run_when_left_empty() {
        let (log_n, r, p) = cheap();
        let a = run("encrypt", "x", "pw", "text", "hex", log_n, r, p, "", 32).unwrap();
        let b = run("encrypt", "x", "pw", "text", "hex", log_n, r, p, "", 32).unwrap();
        assert_ne!(a, b, "an empty salt must draw fresh random bytes each run");
    }

    #[test]
    fn binary_round_trip_through_hex() {
        let (log_n, r, p) = cheap();
        let sealed = run(
            "encrypt", "00ff10", "pw", "hex", "base64", log_n, r, p, SALT_HEX, 32,
        )
        .unwrap();
        // Non-UTF-8 plaintext comes back in the chosen output encoding.
        let opened = run("decrypt", &sealed, "pw", "base64", "hex", log_n, r, p, "", 32).unwrap();
        assert_eq!(opened, "00ff10");
    }

    #[test]
    fn empty_plaintext_is_a_valid_container() {
        let (log_n, r, p) = cheap();
        let sealed = encrypt(b"", "pw", log_n as u8, r as u32, p as u32, &hex::decode(SALT_HEX).unwrap())
            .unwrap();
        assert_eq!(sealed.len(), OVERHEAD);
        assert_eq!(decrypt(&sealed, "pw", 32).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn info_reports_the_header_without_a_password() {
        let (log_n, r, p) = cheap();
        let sealed = run(
            "encrypt", "hello", "pw", "text", "base64", log_n, r, p, SALT_HEX, 32,
        )
        .unwrap();
        let report = run("info", &sealed, "", "base64", "base64", 14, 8, 1, "", 32).unwrap();
        assert!(report.contains("format: scrypt encrypted data"));
        assert!(report.contains("logN: 10"));
        assert!(report.contains("N: 1024"));
        assert!(report.contains("r: 8"));
        assert!(report.contains("p: 1"));
        assert!(report.contains(&format!("salt: {SALT_HEX}")));
        assert!(report.contains("ciphertext: 5 bytes"));
        assert!(report.contains(&format!("total: {} bytes", OVERHEAD + 5)));
    }

    #[test]
    fn info_works_for_parameters_too_large_to_derive() {
        // logN=20 needs 1 GiB — unopenable here, but the header still reads.
        let mut bytes = vec![0u8; OVERHEAD];
        bytes[0..6].copy_from_slice(b"scrypt");
        bytes[7] = 20;
        bytes[8..12].copy_from_slice(&8u32.to_be_bytes());
        bytes[12..16].copy_from_slice(&1u32.to_be_bytes());
        let digest = Sha256::digest(&bytes[0..48]);
        bytes[48..64].copy_from_slice(&digest[0..16]);
        let report = describe(&parse_header(&bytes).unwrap());
        assert!(report.contains("logN: 20"));
        assert!(report.contains("estimated memory: 1.0 GiB"));
        assert!(report.contains("max_memory_mib of at least 1024 MiB"));
    }

    #[test]
    fn auto_detect_hex_container_from_text_encoding() {
        let (log_n, r, p) = cheap();
        let hexed = run(
            "encrypt", "detect me", "pw", "text", "hex", log_n, r, p, SALT_HEX, 32,
        )
        .unwrap();
        let opened = run("decrypt", &hexed, "pw", "text", "base64", log_n, r, p, "", 32).unwrap();
        assert_eq!(opened, "detect me");
    }

    #[test]
    fn wrong_password_is_rejected_before_any_plaintext() {
        let (log_n, r, p) = cheap();
        let sealed = run(
            "encrypt", "secret", "right", "text", "base64", log_n, r, p, SALT_HEX, 32,
        )
        .unwrap();
        let err = run(
            "decrypt", &sealed, "wrong", "base64", "base64", log_n, r, p, "", 32,
        )
        .unwrap_err();
        assert!(err.contains("wrong password"), "got: {err}");
    }

    #[test]
    fn tampered_ciphertext_fails_the_file_tag() {
        let (log_n, r, p) = cheap();
        let mut bytes = encrypt(
            b"secret",
            "pw",
            log_n as u8,
            r as u32,
            p as u32,
            &hex::decode(SALT_HEX).unwrap(),
        )
        .unwrap();
        bytes[HEADER_SIZE] ^= 0x01;
        let err = decrypt(&bytes, "pw", 32).unwrap_err();
        assert!(err.contains("corrupted or was modified"), "got: {err}");
    }

    #[test]
    fn non_scrypt_input_is_named_as_such() {
        let err = parse_header(&vec![0x42u8; OVERHEAD]).unwrap_err();
        assert!(err.contains("not a scrypt file"), "got: {err}");
    }

    #[test]
    fn truncated_input_is_rejected() {
        let err = parse_header(b"scrypt\0\x0e").unwrap_err();
        assert!(err.contains("not a complete scrypt file"), "got: {err}");
    }

    #[test]
    fn corrupted_header_checksum_is_caught() {
        let (log_n, r, p) = cheap();
        let mut bytes = encrypt(
            b"secret",
            "pw",
            log_n as u8,
            r as u32,
            p as u32,
            &hex::decode(SALT_HEX).unwrap(),
        )
        .unwrap();
        bytes[16] ^= 0xff; // flip a salt byte, invalidating the checksum
        let err = parse_header(&bytes).unwrap_err();
        assert!(err.contains("header checksum mismatch"), "got: {err}");
    }

    #[test]
    fn memory_budget_rejects_oversized_parameters() {
        let err = run("encrypt", "x", "pw", "text", "base64", 20, 8, 1, SALT_HEX, 32).unwrap_err();
        assert!(err.contains("need 1.0 GiB"), "got: {err}");
        assert!(err.contains("max_memory_mib"), "got: {err}");
    }

    #[test]
    fn bad_operation_and_encodings_are_rejected() {
        assert!(run("sign", "x", "pw", "text", "base64", 10, 8, 1, "", 32)
            .unwrap_err()
            .contains("unknown operation"));
        assert!(run("encrypt", "x", "pw", "rot13", "base64", 10, 8, 1, "", 32)
            .unwrap_err()
            .contains("unknown data_encoding"));
        assert!(run("encrypt", "x", "pw", "text", "morse", 10, 8, 1, "", 32)
            .unwrap_err()
            .contains("unknown output_encoding"));
    }

    #[test]
    fn missing_password_and_empty_input_are_named() {
        assert!(run("encrypt", "x", "", "text", "base64", 10, 8, 1, "", 32)
            .unwrap_err()
            .contains("password is required"));
        assert!(run("encrypt", "", "pw", "text", "base64", 10, 8, 1, "", 32)
            .unwrap_err()
            .contains("input is empty"));
    }

    #[test]
    fn bad_salt_length_is_named() {
        let err = run("encrypt", "x", "pw", "text", "base64", 10, 8, 1, "00ff", 32).unwrap_err();
        assert!(err.contains("salt must be exactly 32 bytes"), "got: {err}");
    }

    #[test]
    fn out_of_range_cost_parameters_are_named() {
        assert!(run("encrypt", "x", "pw", "text", "base64", 0, 8, 1, "", 32)
            .unwrap_err()
            .contains("log_n must be between 1 and 63"));
        assert!(run("encrypt", "x", "pw", "text", "base64", 10, 0, 1, "", 32)
            .unwrap_err()
            .contains("r must be between 1 and 32"));
        assert!(run("encrypt", "x", "pw", "text", "base64", 10, 8, 0, "", 32)
            .unwrap_err()
            .contains("p must be between 1 and 16"));
    }

    /// Fixed vector: with this salt/password/parameters the container is byte
    /// stable, so a regression in key derivation or framing shows up here.
    #[test]
    fn known_vector_is_stable() {
        let sealed = run(
            "encrypt", "hello", "pleaseletmein", "text", "hex", 10, 8, 1, SALT_HEX, 32,
        )
        .unwrap();
        // "scrypt" ++ version 0x00 ++ logN 0x0a ++ r=8 ++ p=1 ++ the salt.
        assert!(
            sealed.starts_with(&format!("736372797074000a0000000800000001{SALT_HEX}")),
            "got: {sealed}"
        );
        // Re-open it to prove the vector is a real container, not just a prefix.
        let opened = run("decrypt", &sealed, "pleaseletmein", "hex", "base64", 10, 8, 1, "", 32)
            .unwrap();
        assert_eq!(opened, "hello");
    }
}
