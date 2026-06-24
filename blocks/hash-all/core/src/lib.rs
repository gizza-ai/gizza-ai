//! hash-all core — compute EVERY common digest of an input at once and render
//! them in a labeled table. Pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! The input text can be interpreted as plain UTF-8 (default) or decoded first
//! from hex / base64, and each digest is emitted as lowercase/uppercase hex or
//! base64. All hashers are pure-Rust (RustCrypto + blake3) so the tool runs on
//! every backend, including the chat Service Worker.

use base64::Engine;

/// How to interpret the input string before hashing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputEncoding {
    /// Hash the UTF-8 bytes of the text as-is (default).
    Text,
    /// Decode the text from hexadecimal first, then hash the raw bytes.
    Hex,
    /// Decode the text from base64 (standard alphabet) first, then hash the bytes.
    Base64,
}

/// How to render each digest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// Lowercase/uppercase hexadecimal.
    Hex,
    /// Standard base64.
    Base64,
}

/// The labels of every digest produced, in output order. Kept in one place so
/// the core, descriptor docs, and page stay in sync.
pub const LABELS: &[&str] = &[
    "CRC-32",
    "MD5",
    "SHA-1",
    "SHA-224",
    "SHA-256",
    "SHA-384",
    "SHA-512",
    "SHA3-256",
    "SHA3-512",
    "RIPEMD-160",
    "BLAKE2b-512",
    "BLAKE2s-256",
    "BLAKE3",
    "Whirlpool",
];

fn parse_input_encoding(s: &str) -> Result<InputEncoding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" | "utf8" | "utf-8" => Ok(InputEncoding::Text),
        "hex" => Ok(InputEncoding::Hex),
        "base64" | "b64" => Ok(InputEncoding::Base64),
        other => Err(format!(
            "invalid input_encoding '{other}': expected 'text', 'hex', or 'base64'"
        )),
    }
}

fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "hex" => Ok(OutputFormat::Hex),
        "base64" | "b64" => Ok(OutputFormat::Base64),
        other => Err(format!(
            "invalid output_format '{other}': expected 'hex' or 'base64'"
        )),
    }
}

/// Decode `text` into raw bytes per `encoding`.
fn decode_input(text: &str, encoding: InputEncoding) -> Result<Vec<u8>, String> {
    match encoding {
        InputEncoding::Text => Ok(text.as_bytes().to_vec()),
        InputEncoding::Hex => {
            // Tolerate an optional 0x prefix and internal whitespace (matches
            // hash-text / keccak-hash so the hash family accepts the same input).
            let cleaned: String = text.split_whitespace().collect();
            let cleaned = cleaned
                .strip_prefix("0x")
                .or_else(|| cleaned.strip_prefix("0X"))
                .unwrap_or(&cleaned);
            hex::decode(cleaned).map_err(|e| format!("input is not valid hex: {e}"))
        }
        InputEncoding::Base64 => {
            let cleaned: String = text.split_whitespace().collect();
            base64::engine::general_purpose::STANDARD
                .decode(cleaned.as_bytes())
                .map_err(|e| format!("input is not valid base64: {e}"))
        }
    }
}

/// CRC-32 (IEEE 802.3, reflected) — the variant used by zip, gzip, PNG.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Render `digest` per `fmt`/`uppercase`.
fn render(digest: &[u8], fmt: OutputFormat, uppercase: bool) -> String {
    match fmt {
        OutputFormat::Hex => {
            let h = hex::encode(digest);
            if uppercase {
                h.to_uppercase()
            } else {
                h
            }
        }
        OutputFormat::Base64 => base64::engine::general_purpose::STANDARD.encode(digest),
    }
}

/// Compute every supported digest for `data`, rendered per `fmt`/`uppercase`.
/// Returns `(label, value)` pairs in `LABELS` order.
pub fn digest_all(data: &[u8], fmt: OutputFormat, uppercase: bool) -> Vec<(&'static str, String)> {
    use blake2::{Blake2b512, Blake2s256};
    use digest::Digest;
    use ripemd::Ripemd160;
    use sha2::{Sha224, Sha256, Sha384, Sha512};
    use sha3::{Sha3_256, Sha3_512};
    use whirlpool::Whirlpool;

    let r = |d: &[u8]| render(d, fmt, uppercase);
    // CRC-32 is rendered from its 4 big-endian bytes, like every other digest.
    let crc_bytes = crc32(data).to_be_bytes();

    vec![
        ("CRC-32", r(&crc_bytes)),
        ("MD5", r(&md5::Md5::digest(data))),
        ("SHA-1", r(&sha1::Sha1::digest(data))),
        ("SHA-224", r(&Sha224::digest(data))),
        ("SHA-256", r(&Sha256::digest(data))),
        ("SHA-384", r(&Sha384::digest(data))),
        ("SHA-512", r(&Sha512::digest(data))),
        ("SHA3-256", r(&Sha3_256::digest(data))),
        ("SHA3-512", r(&Sha3_512::digest(data))),
        ("RIPEMD-160", r(&Ripemd160::digest(data))),
        ("BLAKE2b-512", r(&Blake2b512::digest(data))),
        ("BLAKE2s-256", r(&Blake2s256::digest(data))),
        ("BLAKE3", r(blake3::hash(data).as_bytes())),
        ("Whirlpool", r(&Whirlpool::digest(data))),
    ]
}

/// Compute every digest of `text` (interpreted per `input_encoding`) and render
/// them as an aligned `label  value` table. When `output_format` is hex,
/// `uppercase` controls the hex case; it has no effect on base64.
///
/// Defaults (blank strings): input_encoding=text, output_format=hex, lowercase.
pub fn hash_all(
    text: &str,
    input_encoding: &str,
    output_format: &str,
    uppercase: bool,
) -> Result<String, String> {
    let enc = parse_input_encoding(input_encoding)?;
    let fmt = parse_output_format(output_format)?;
    let bytes = decode_input(text, enc)?;
    let rows = digest_all(&bytes, fmt, uppercase);
    let width = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    let mut out = String::new();
    for (label, value) in rows {
        out.push_str(&format!("{label:<width$}  {value}\n"));
    }
    // Trim the trailing newline so callers get a clean block.
    out.pop();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(text: &str) -> std::collections::HashMap<String, String> {
        let bytes = text.as_bytes();
        digest_all(bytes, OutputFormat::Hex, false)
            .into_iter()
            .map(|(l, v)| (l.to_string(), v))
            .collect()
    }

    #[test]
    fn abc_known_vectors() {
        let m = map("abc");
        assert_eq!(m["CRC-32"], "352441c2");
        assert_eq!(m["MD5"], "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(m["SHA-1"], "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            m["SHA-224"],
            "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7"
        );
        assert_eq!(
            m["SHA-256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            m["SHA-384"],
            "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7"
        );
        assert_eq!(
            m["SHA-512"],
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        assert_eq!(
            m["SHA3-256"],
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
        );
        assert_eq!(
            m["SHA3-512"],
            "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0"
        );
        assert_eq!(m["RIPEMD-160"], "8eb208f7e05d987a9b044a8e98c6b087f15a0bfc");
        assert_eq!(
            m["BLAKE2b-512"],
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
        );
        assert_eq!(
            m["BLAKE2s-256"],
            "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982"
        );
        assert_eq!(
            m["BLAKE3"],
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );
        // Well-known Whirlpool("abc") test vector (ISO/IEC 10118-3).
        assert_eq!(
            m["Whirlpool"],
            "4e2448a4c6f486bb16b6562c73b4020bf3043e3a731bce721ae1b303d97e6d4c7181eebdb6c57e277d0e34957114cbd6c797fc9d95d8b582d225292076d4eef5"
        );
    }

    #[test]
    fn empty_input_vectors() {
        let m = map("");
        assert_eq!(m["CRC-32"], "00000000");
        assert_eq!(m["MD5"], "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(
            m["SHA-256"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn table_has_all_labels_in_order() {
        let out = hash_all("abc", "", "", false).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), LABELS.len());
        for (line, label) in lines.iter().zip(LABELS) {
            assert!(line.starts_with(label), "line {line:?} != label {label}");
        }
    }

    #[test]
    fn uppercase_hex() {
        let out = hash_all("abc", "", "hex", true).unwrap();
        assert!(out.contains("900150983CD24FB0D6963F7D28E17F72"));
    }

    #[test]
    fn base64_output() {
        // SHA-256("abc") in base64 should appear in the table.
        let out = hash_all("abc", "", "base64", false).unwrap();
        assert!(out.contains("ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="));
    }

    #[test]
    fn hex_input_matches_text() {
        // "abc" as hex is 616263 — must produce the identical table.
        assert_eq!(
            hash_all("616263", "hex", "", false).unwrap(),
            hash_all("abc", "text", "", false).unwrap()
        );
    }

    #[test]
    fn hex_input_accepts_0x_prefix() {
        assert_eq!(
            hash_all("0x616263", "hex", "", false).unwrap(),
            hash_all("abc", "text", "", false).unwrap()
        );
    }

    #[test]
    fn base64_input_matches_text() {
        // "abc" as base64 is "YWJj".
        assert_eq!(
            hash_all("YWJj", "base64", "", false).unwrap(),
            hash_all("abc", "text", "", false).unwrap()
        );
    }

    #[test]
    fn invalid_input_encoding_errors() {
        let e = hash_all("abc", "rot13", "", false).unwrap_err();
        assert!(e.contains("invalid input_encoding"), "{e}");
    }

    #[test]
    fn invalid_output_format_errors() {
        let e = hash_all("abc", "", "octal", false).unwrap_err();
        assert!(e.contains("invalid output_format"), "{e}");
    }

    #[test]
    fn bad_hex_input_errors() {
        let e = hash_all("zz", "hex", "", false).unwrap_err();
        assert!(e.contains("not valid hex"), "{e}");
    }
}
