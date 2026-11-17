//! gizza-ai/xsalsa20-cipher core — XSalsa20 stream cipher.
//!
//! XSalsa20 is Salsa20 with a 24-byte nonce. The same operation encrypts and
//! decrypts by XORing the plaintext/ciphertext with the keystream.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use salsa20::cipher::{KeyIvInit, StreamCipher, StreamCipherSeek};
use salsa20::XSalsa20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" => Ok(Self::Hex),
            "base64" | "b64" => Ok(Self::Base64),
            other => Err(format!("unknown format '{other}' (use hex or base64)")),
        }
    }

    pub fn decode(self, input: &str) -> Result<Vec<u8>, String> {
        // Both encodings ignore all ASCII whitespace, so line-wrapped input
        // (e.g. `base64` / `openssl base64` output) decodes as pasted.
        let cleaned: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        match self {
            Self::Hex => hex::decode(cleaned).map_err(|e| format!("invalid hex: {e}")),
            Self::Base64 => B64.decode(cleaned).map_err(|e| format!("invalid base64: {e}")),
        }
    }

    pub fn encode(self, bytes: &[u8]) -> String {
        match self {
            Self::Hex => hex::encode(bytes),
            Self::Base64 => B64.encode(bytes),
        }
    }
}

/// How the key and nonce strings are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFormat {
    /// Raw UTF-8 bytes of the string.
    Text,
    /// Decoded with the same encoding as the ciphertext (`format`).
    Encoded,
}

impl KeyFormat {
    /// Empty input means the documented default, `encoded`. Only the two
    /// values the schema and page offer are accepted; naming an encoding here
    /// is an error rather than being silently ignored.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "encoded" => Ok(Self::Encoded),
            "text" => Ok(Self::Text),
            enc @ ("hex" | "base64" | "b64") => Err(format!(
                "key_format '{enc}' is not supported: use key_format=encoded and set format={enc} \
                 to read the key and nonce as {enc}"
            )),
            other => Err(format!("unknown key_format '{other}' (use encoded or text)")),
        }
    }
}

fn key_or_nonce_bytes(input: &str, key_format: KeyFormat, encoding: Encoding) -> Result<Vec<u8>, String> {
    match key_format {
        KeyFormat::Text => Ok(input.as_bytes().to_vec()),
        KeyFormat::Encoded => encoding.decode(input),
    }
}

/// XOR `data` in place with the XSalsa20 keystream starting at block
/// `counter`, returning it. Unlike libsodium, which wraps the block counter
/// back to 0 (reusing keystream), this refuses data that would run past the
/// last usable block (2^64 - 2).
pub fn xsalsa20_apply(key: &[u8], nonce: &[u8], counter: u64, mut data: Vec<u8>) -> Result<Vec<u8>, String> {
    if key.len() != 32 {
        return Err(format!("XSalsa20 key must be exactly 32 bytes, got {}", key.len()));
    }
    if nonce.len() != 24 {
        return Err(format!("XSalsa20 nonce must be exactly 24 bytes, got {}", nonce.len()));
    }
    let mut cipher = XSalsa20::new(key.into(), nonce.into());
    // Seek in bytes as u128: block `counter` starts at counter * 64, which
    // overflows u64 for counter >= 2^58. Never saturate — that would map many
    // distinct counters onto the same keystream position.
    let out_of_range = || {
        "counter too large: the data would run past the end of the XSalsa20 keystream \
         (the keystream is not wrapped around to block 0)"
            .to_string()
    };
    cipher.try_seek(u128::from(counter) * 64).map_err(|_| out_of_range())?;
    cipher.try_apply_keystream(&mut data).map_err(|_| out_of_range())?;
    Ok(data)
}

pub fn encrypt(
    plaintext: &str,
    key: &str,
    nonce: &str,
    key_format: KeyFormat,
    counter: u64,
    format: Encoding,
) -> Result<String, String> {
    let key = key_or_nonce_bytes(key, key_format, format)?;
    let nonce = key_or_nonce_bytes(nonce, key_format, format)?;
    let ct = xsalsa20_apply(&key, &nonce, counter, plaintext.as_bytes().to_vec())?;
    Ok(format.encode(&ct))
}

pub fn decrypt(
    ciphertext: &str,
    key: &str,
    nonce: &str,
    key_format: KeyFormat,
    counter: u64,
    format: Encoding,
) -> Result<String, String> {
    let key = key_or_nonce_bytes(key, key_format, format)?;
    let nonce = key_or_nonce_bytes(nonce, key_format, format)?;
    let ct = format.decode(ciphertext)?;
    let pt = xsalsa20_apply(&key, &nonce, counter, ct)?;
    String::from_utf8(pt).map_err(|_| "decrypted bytes are not valid UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const NONCE_HEX: &str = "000102030405060708090a0b0c0d0e0f1011121314151617";

    #[test]
    fn roundtrip_hex() {
        let msg = "hello xsalsa20";
        let ct = encrypt(msg, KEY_HEX, NONCE_HEX, KeyFormat::Encoded, 0, Encoding::Hex).unwrap();
        assert_ne!(ct, hex::encode(msg));
        let pt = decrypt(&ct, KEY_HEX, NONCE_HEX, KeyFormat::Encoded, 0, Encoding::Hex).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn roundtrip_base64() {
        let key = B64.encode([7u8; 32]);
        let nonce = B64.encode([9u8; 24]);
        let ct = encrypt("secret", &key, &nonce, KeyFormat::Encoded, 1, Encoding::Base64).unwrap();
        let pt = decrypt(&ct, &key, &nonce, KeyFormat::Encoded, 1, Encoding::Base64).unwrap();
        assert_eq!(pt, "secret");
    }

    #[test]
    fn text_key_nonce_lengths() {
        let key = "12345678901234567890123456789012";
        let nonce = "123456789012345678901234";
        let ct = encrypt("abc", key, nonce, KeyFormat::Text, 0, Encoding::Hex).unwrap();
        let pt = decrypt(&ct, key, nonce, KeyFormat::Text, 0, Encoding::Hex).unwrap();
        assert_eq!(pt, "abc");
    }

    // Reference vectors generated with libsodium's
    // crypto_stream_xsalsa20_xor_ic (key = 00..1f, nonce = 00..17).
    const KS_IC0: &str = "7cb660afdd9ec6468f57dd6d2433f93428fd82cd7386c5471a24d8ad2a525b6e\
        5eff384fc7caa210bb3c8f3e688f4a9752a546df8c253fef17a2679455c7a1e1\
        83dbf5d545b0f502b98de0997a66ab432341689ff397dc4fbc1f27bd1a6197f5\
        dc80ff190516c9ed14f281d1ca738882f6d3d2fb921e2ef899389e0a223be7ae";
    const KS_IC_2_60: &str = "95c388ce610bbf2a46332ef0ef6c89c7772380d533cad7620be704db124e2455\
        9565c46f1b8cb51a2924f009a97780c758eb54718253919bda5f7795fd6d5a03";

    fn keystream(counter: u64, len: usize) -> String {
        let key = hex::decode(KEY_HEX).unwrap();
        let nonce = hex::decode(NONCE_HEX).unwrap();
        hex::encode(xsalsa20_apply(&key, &nonce, counter, vec![0u8; len]).unwrap())
    }

    #[test]
    fn matches_libsodium_keystream() {
        assert_eq!(keystream(0, 128), KS_IC0);
        // counter 1 is the second 64-byte block of the counter-0 stream
        assert_eq!(keystream(1, 64), &KS_IC0[128..]);
        // counters past 2^58 must use the real block offset, not a saturated one
        assert_eq!(keystream(1 << 60, 64), KS_IC_2_60);
    }

    #[test]
    fn matches_libsodium_message() {
        let ct = encrypt("hello xsalsa20", KEY_HEX, NONCE_HEX, KeyFormat::Encoded, 0, Encoding::Hex).unwrap();
        assert_eq!(ct, "14d30cc3b2bebe35ee3bae0c1603");
    }

    #[test]
    fn large_counters_do_not_collide() {
        assert_ne!(keystream(1 << 58, 16), keystream(u64::MAX - 1, 16));
        assert_ne!(keystream(1 << 58, 16), keystream((1 << 58) + 1, 16));
    }

    #[test]
    fn running_past_the_keystream_end_is_an_error() {
        // the last usable block is 2^64 - 2: one block there works, a byte more fails
        assert_eq!(xsalsa20_apply(&[0u8; 32], &[0u8; 24], u64::MAX - 1, vec![0u8; 64]).unwrap().len(), 64);
        for (counter, len) in [(u64::MAX - 1, 65), (u64::MAX, 1)] {
            let err = xsalsa20_apply(&[0u8; 32], &[0u8; 24], counter, vec![0u8; len]).unwrap_err();
            assert!(err.contains("counter too large"), "{err}");
        }
    }

    #[test]
    fn key_format_defaults_to_encoded_and_rejects_encoding_names() {
        assert_eq!(KeyFormat::parse("").unwrap(), KeyFormat::Encoded);
        assert_eq!(KeyFormat::parse(" Encoded ").unwrap(), KeyFormat::Encoded);
        // empty key_format + the documented default hex key must work
        let ct = encrypt("hello xsalsa20", KEY_HEX, NONCE_HEX, KeyFormat::parse("").unwrap(), 0, Encoding::Hex).unwrap();
        assert_eq!(ct, "14d30cc3b2bebe35ee3bae0c1603");
        // naming an encoding as key_format is rejected with a pointer to `format`
        let err = KeyFormat::parse("base64").unwrap_err();
        assert!(err.contains("format=base64"), "{err}");
    }

    #[test]
    fn line_wrapped_base64_and_hex_decode() {
        // format=base64 reads the key and nonce as base64 too
        let key = B64.encode(hex::decode(KEY_HEX).unwrap());
        let nonce = B64.encode(hex::decode(NONCE_HEX).unwrap());
        let ct = encrypt("hello xsalsa20", &key, &nonce, KeyFormat::Encoded, 0, Encoding::Base64).unwrap();
        let wrapped = format!("{}\n{}\r\n", &ct[..8], &ct[8..]);
        let pt = decrypt(&wrapped, &key, &nonce, KeyFormat::Encoded, 0, Encoding::Base64).unwrap();
        assert_eq!(pt, "hello xsalsa20");
        let pt = decrypt("14d30cc3b2be\nbe35ee3bae0c1603", KEY_HEX, NONCE_HEX, KeyFormat::Encoded, 0, Encoding::Hex).unwrap();
        assert_eq!(pt, "hello xsalsa20");
    }

    #[test]
    fn validates_lengths_and_formats() {
        assert!(xsalsa20_apply(&[0u8; 31], &[0u8; 24], 0, b"x".to_vec()).is_err());
        assert!(xsalsa20_apply(&[0u8; 32], &[0u8; 23], 0, b"x".to_vec()).is_err());
        assert!(Encoding::parse("base32").is_err());
        assert!(KeyFormat::parse("pem").is_err());
    }
}
