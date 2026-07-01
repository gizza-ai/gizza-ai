//! gizza-ai/xor-cipher core — repeating-key bytewise XOR.
//!
//! XOR is symmetric: the same operation encrypts and decrypts. This is useful
//! for CTFs, interoperability, obfuscation, and learning, but it is not secure
//! encryption for real secrets.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    Text,
    Hex,
    Base64,
}

impl DataFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "utf8" | "utf-8" => Ok(Self::Text),
            "hex" | "base16" => Ok(Self::Hex),
            "base64" | "b64" => Ok(Self::Base64),
            other => Err(format!("unknown format '{other}' (use text, hex, or base64)")),
        }
    }

    pub fn decode(self, input: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Text => Ok(input.as_bytes().to_vec()),
            Self::Hex => {
                let trimmed = input.trim();
                let without_prefix = trimmed
                    .strip_prefix("0x")
                    .or_else(|| trimmed.strip_prefix("0X"))
                    .unwrap_or(trimmed);
                let cleaned: String = without_prefix
                    .chars()
                    .filter(|c| !c.is_ascii_whitespace())
                    .collect();
                hex::decode(cleaned).map_err(|e| format!("invalid hex: {e}"))
            }
            Self::Base64 => B64
                .decode(input.trim())
                .map_err(|e| format!("invalid base64: {e}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Hex,
    Base64,
    Utf8,
}

impl OutFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" | "base16" => Ok(Self::Hex),
            "base64" | "b64" => Ok(Self::Base64),
            "utf8" | "utf-8" | "text" => Ok(Self::Utf8),
            other => Err(format!("unknown output '{other}' (use hex, base64, or utf8)")),
        }
    }

    pub fn encode(self, bytes: &[u8]) -> Result<String, String> {
        match self {
            Self::Hex => Ok(hex::encode(bytes)),
            Self::Base64 => Ok(B64.encode(bytes)),
            Self::Utf8 => String::from_utf8(bytes.to_vec())
                .map_err(|_| "output is not valid UTF-8 text (use hex or base64)".to_string()),
        }
    }
}

pub fn xor_bytes(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    if key.is_empty() {
        return Err("key must not be empty".to_string());
    }
    Ok(data
        .iter()
        .enumerate()
        .map(|(idx, byte)| byte ^ key[idx % key.len()])
        .collect())
}

pub fn xor_cipher(
    data: &str,
    input: DataFormat,
    key: &str,
    key_format: DataFormat,
    output: OutFormat,
) -> Result<String, String> {
    let data_bytes = input.decode(data)?;
    let key_bytes = key_format.decode(key)?;
    let result = xor_bytes(&data_bytes, &key_bytes)?;
    output.encode(&result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_single_byte_vector() {
        let out = xor_cipher("Hello", DataFormat::Text, "K", DataFormat::Text, OutFormat::Hex)
            .unwrap();
        assert_eq!(out, "032e272724");
    }

    #[test]
    fn cryptopals_repeating_key_vector() {
        let pt = "Burning 'em, if you ain't quick and nimble\nI go crazy when I hear a cymbal";
        let out = xor_cipher(pt, DataFormat::Text, "ICE", DataFormat::Text, OutFormat::Hex)
            .unwrap();
        assert_eq!(out, "0b3637272a2b2e63622c2e69692a23693a2a3c6324202d623d63343c2a26226324272765272a282b2f20430a652e2c652a3124333a653e2b2027630c692b20283165286326302e27282f");
    }

    #[test]
    fn roundtrips_hex_to_utf8() {
        let msg = "The quick brown fox jumps over the lazy dog";
        let ct = xor_cipher(msg, DataFormat::Text, "s3cr3t", DataFormat::Text, OutFormat::Hex)
            .unwrap();
        let pt = xor_cipher(&ct, DataFormat::Hex, "s3cr3t", DataFormat::Text, OutFormat::Utf8)
            .unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn roundtrips_base64() {
        let msg = "symmetric XOR";
        let ct = xor_cipher(msg, DataFormat::Text, "0badf00d", DataFormat::Hex, OutFormat::Base64)
            .unwrap();
        let pt = xor_cipher(&ct, DataFormat::Base64, "0badf00d", DataFormat::Hex, OutFormat::Utf8)
            .unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn accepts_hex_whitespace_and_prefix() {
        let out = xor_cipher("0x48 65 6c 6c 6f", DataFormat::Hex, "00", DataFormat::Hex, OutFormat::Utf8)
            .unwrap();
        assert_eq!(out, "Hello");
    }

    #[test]
    fn reports_errors() {
        assert!(xor_bytes(b"x", b"").is_err());
        assert!(DataFormat::parse("octal").is_err());
        assert!(OutFormat::parse("rot13").is_err());
        assert!(xor_cipher("zz", DataFormat::Hex, "k", DataFormat::Text, OutFormat::Hex).is_err());
        assert!(xor_cipher("/w==", DataFormat::Base64, "00", DataFormat::Hex, OutFormat::Utf8).is_err());
    }
}
