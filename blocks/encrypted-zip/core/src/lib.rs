//! gizza-ai/encrypted-zip core — password-protected ZIP pack + extract, shared
//! by the chat skill block and CLI. No wafer/wasm-bindgen deps.
//!
//! Pack: named file byte buffers → one deflate ZIP whose entries are encrypted
//! with WinZip AES (AE-2, AES-256 default or AES-128) — the format 7-Zip,
//! WinZip, WinRAR, and The Unarchiver open. Duplicate names are made unique
//! (`name (2).ext`, same policy as create-zip).
//!
//! Extract: decrypt + extract a password-protected ZIP. The encryption method
//! is auto-detected per entry — AES-256/192/128 (AE-1/AE-2) and legacy
//! ZipCrypto both work; unencrypted entries in a mixed archive extract too.
//! Entry content is returned inline (text when UTF-8, else base64) up to a
//! total byte budget; beyond it entries are listed without content. Reads are
//! `.take()`-guarded against decompression bombs (declared-size + budget caps,
//! per the 2026-07-17 hardening sweep).

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Serialize;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::{AesMode, CompressionMethod};

/// Default cap on total inlined content bytes on extract (same as unzip).
pub const DEFAULT_CONTENT_BUDGET: usize = 8 * 1024 * 1024;

/// AES key strength for pack (WinZip AE-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encryption {
    Aes256,
    Aes128,
}

impl Encryption {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "aes256" => Ok(Self::Aes256),
            "aes128" => Ok(Self::Aes128),
            other => Err(format!("invalid encryption '{other}' (expected aes256 or aes128)")),
        }
    }
}

/// Make `name` unique against `seen` by inserting `(n)` before the extension
/// (same policy as create-zip).
fn unique_name(name: &str, seen: &mut HashSet<String>) -> String {
    let base = if name.trim().is_empty() { "file".to_string() } else { name.trim().to_string() };
    if seen.insert(base.clone()) {
        return base;
    }
    let (stem, ext) = match base.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (base.clone(), String::new()),
    };
    for n in 2.. {
        let cand = format!("{stem} ({n}){ext}");
        if seen.insert(cand.clone()) {
            return cand;
        }
    }
    unreachable!()
}

/// Pack `(filename, bytes)` entries into a password-protected, AES-encrypted,
/// deflate-compressed ZIP. `level` is the deflate level 1–9.
pub fn pack(
    files: &[(String, Vec<u8>)],
    password: &str,
    encryption: Encryption,
    level: i64,
) -> Result<Vec<u8>, String> {
    if files.is_empty() {
        return Err("need at least one file".into());
    }
    if password.is_empty() {
        return Err("password must not be empty".into());
    }
    if !(1..=9).contains(&level) {
        return Err(format!("compression level must be 1-9, got {level}"));
    }
    let aes_mode = match encryption {
        Encryption::Aes256 => AesMode::Aes256,
        Encryption::Aes128 => AesMode::Aes128,
    };
    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let mut seen = HashSet::new();
        for (name, bytes) in files {
            let opts = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(level))
                .with_aes_encryption(aes_mode, password);
            let entry = unique_name(name, &mut seen);
            zw.start_file(entry, opts).map_err(|e| format!("zip start_file: {e}"))?;
            zw.write_all(bytes).map_err(|e| format!("zip write: {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }
    Ok(buf)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    /// Whether this entry was stored encrypted (mixed archives are possible).
    pub encrypted: bool,
    /// UTF-8 text content, when the file decodes as UTF-8 and fit the budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Base64 content, when the file is binary and fit the budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Set when the content was omitted because the budget was exhausted.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub content_omitted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Extracted {
    pub entries: Vec<Entry>,
    /// Number of regular files (excludes directories).
    pub count: usize,
    /// How many of those were stored encrypted.
    pub encrypted_count: usize,
}

const WRONG_PASSWORD: &str =
    "wrong password: the archive's password verifier rejected the supplied password";

/// Decrypt + extract `zip_bytes`. Handles AES-256/192/128 (AE-1/AE-2) and
/// legacy ZipCrypto entries; unencrypted entries extract normally (the
/// password is ignored for them). `content_budget` caps total inlined bytes.
pub fn extract(zip_bytes: &[u8], password: &str, content_budget: usize) -> Result<Extracted, String> {
    if password.is_empty() {
        return Err("password must not be empty".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("not a valid zip archive: {e}"))?;

    let mut entries = Vec::new();
    let mut used = 0usize;
    let mut encrypted_count = 0usize;
    for i in 0..archive.len() {
        let mut file = archive.by_index_decrypt(i, password.as_bytes()).map_err(|e| match e {
            zip::result::ZipError::InvalidPassword => WRONG_PASSWORD.to_string(),
            e => format!("failed to open zip entry {i}: {e}"),
        })?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        let size = file.size();
        let encrypted = file.encrypted();
        if encrypted {
            encrypted_count += 1;
        }

        // Read content if it fits the remaining budget. `.take(size + 1)`
        // guards against entries whose real decompressed size exceeds the
        // declared one (decompression bomb with a lying header).
        let mut text = None;
        let mut data = None;
        let mut content_omitted = false;
        if (size as usize).saturating_add(used) <= content_budget {
            let mut buf = Vec::with_capacity(size as usize);
            let read_err = |e: std::io::Error| {
                if encrypted {
                    format!("failed to extract '{name}': {e} (wrong password or corrupted archive?)")
                } else {
                    format!("failed to extract '{name}': {e}")
                }
            };
            (&mut file).take(size + 1).read_to_end(&mut buf).map_err(read_err)?;
            if buf.len() as u64 > size {
                return Err(format!(
                    "entry '{name}' decompresses past its declared size of {size} bytes — refusing (zip bomb?)"
                ));
            }
            used += buf.len();
            match String::from_utf8(buf) {
                Ok(s) => text = Some(s),
                Err(e) => data = Some(B64.encode(e.as_bytes())),
            }
        } else {
            content_omitted = true;
        }

        entries.push(Entry { name, size, encrypted, text, data, content_omitted });
    }

    let count = entries.len();
    Ok(Extracted { entries, count, encrypted_count })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Info-ZIP (`zip -P letmein`) fixture: legacy ZipCrypto, one entry
    /// `secret.txt` containing "top secret\n".
    const ZIPCRYPTO_FIXTURE: &[u8] = include_bytes!("../fixtures/zipcrypto.zip");

    #[test]
    fn pack_extract_roundtrip_aes256() {
        let files = vec![
            ("a.txt".to_string(), b"hello".to_vec()),
            ("b.bin".to_string(), vec![0u8, 159, 146, 150]), // not valid UTF-8
        ];
        let zip = pack(&files, "s3cret!", Encryption::Aes256, 6).unwrap();
        assert_eq!(&zip[..2], b"PK");
        let got = extract(&zip, "s3cret!", DEFAULT_CONTENT_BUDGET).unwrap();
        assert_eq!(got.count, 2);
        assert_eq!(got.encrypted_count, 2);
        assert_eq!(got.entries[0].name, "a.txt");
        assert!(got.entries[0].encrypted);
        assert_eq!(got.entries[0].text.as_deref(), Some("hello"));
        assert_eq!(got.entries[1].data.as_deref(), Some(B64.encode([0u8, 159, 146, 150]).as_str()));
    }

    #[test]
    fn pack_extract_roundtrip_aes128() {
        let files = vec![("x.txt".to_string(), b"aes128 works".to_vec())];
        let zip = pack(&files, "pw", Encryption::Aes128, 1).unwrap();
        let got = extract(&zip, "pw", DEFAULT_CONTENT_BUDGET).unwrap();
        assert_eq!(got.entries[0].text.as_deref(), Some("aes128 works"));
        assert_eq!(got.encrypted_count, 1);
    }

    #[test]
    fn wrong_password_is_rejected() {
        let zip = pack(&[("a.txt".to_string(), b"data".to_vec())], "right", Encryption::Aes256, 6)
            .unwrap();
        let err = extract(&zip, "wrong", DEFAULT_CONTENT_BUDGET).unwrap_err();
        assert!(err.contains("wrong password"), "got: {err}");
    }

    #[test]
    fn extracts_legacy_zipcrypto_archive() {
        let got = extract(ZIPCRYPTO_FIXTURE, "letmein", DEFAULT_CONTENT_BUDGET).unwrap();
        assert_eq!(got.count, 1);
        assert_eq!(got.encrypted_count, 1);
        assert_eq!(got.entries[0].name, "secret.txt");
        assert!(got.entries[0].encrypted);
        assert_eq!(got.entries[0].text.as_deref(), Some("top secret\n"));
    }

    #[test]
    fn zipcrypto_wrong_password_is_rejected() {
        let err = extract(ZIPCRYPTO_FIXTURE, "not-letmein", DEFAULT_CONTENT_BUDGET).unwrap_err();
        assert!(err.contains("wrong password") || err.contains("corrupted"), "got: {err}");
    }

    #[test]
    fn unencrypted_entries_extract_with_any_password() {
        // Mixed/plain archives: the password is ignored for entries that are
        // not encrypted (create-zip output is plain deflate).
        let mut buf = Vec::new();
        {
            let mut zw = ZipWriter::new(Cursor::new(&mut buf));
            let opts =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zw.start_file("plain.txt", opts).unwrap();
            zw.write_all(b"not encrypted").unwrap();
            zw.finish().unwrap();
        }
        let got = extract(&buf, "anything", DEFAULT_CONTENT_BUDGET).unwrap();
        assert_eq!(got.count, 1);
        assert_eq!(got.encrypted_count, 0);
        assert!(!got.entries[0].encrypted);
        assert_eq!(got.entries[0].text.as_deref(), Some("not encrypted"));
    }

    #[test]
    fn budget_omits_content_but_lists_entry() {
        let files = vec![
            ("small.txt".to_string(), b"ok".to_vec()),
            ("big.bin".to_string(), vec![7u8; 4096]),
        ];
        let zip = pack(&files, "pw", Encryption::Aes256, 6).unwrap();
        let got = extract(&zip, "pw", 16).unwrap();
        assert_eq!(got.entries[0].text.as_deref(), Some("ok"));
        assert!(got.entries[1].content_omitted);
        assert!(got.entries[1].text.is_none() && got.entries[1].data.is_none());
        assert_eq!(got.entries[1].size, 4096);
    }

    #[test]
    fn duplicate_names_made_unique() {
        let files = vec![
            ("dup.txt".to_string(), b"1".to_vec()),
            ("dup.txt".to_string(), b"2".to_vec()),
        ];
        let zip = pack(&files, "pw", Encryption::Aes256, 6).unwrap();
        let got = extract(&zip, "pw", DEFAULT_CONTENT_BUDGET).unwrap();
        let names: Vec<&str> = got.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["dup.txt", "dup (2).txt"]);
    }

    #[test]
    fn pack_input_validation_errors() {
        assert!(pack(&[], "pw", Encryption::Aes256, 6).is_err());
        let one = [("a".to_string(), b"x".to_vec())];
        assert!(pack(&one, "", Encryption::Aes256, 6).unwrap_err().contains("password"));
        assert!(pack(&one, "pw", Encryption::Aes256, 0).unwrap_err().contains("level"));
        assert!(pack(&one, "pw", Encryption::Aes256, 10).unwrap_err().contains("level"));
    }

    #[test]
    fn extract_input_validation_errors() {
        assert!(extract(b"not a zip", "pw", DEFAULT_CONTENT_BUDGET)
            .unwrap_err()
            .contains("not a valid zip"));
        assert!(extract(ZIPCRYPTO_FIXTURE, "", DEFAULT_CONTENT_BUDGET)
            .unwrap_err()
            .contains("password"));
    }

    #[test]
    fn encryption_parse() {
        assert_eq!(Encryption::parse("aes256").unwrap(), Encryption::Aes256);
        assert_eq!(Encryption::parse("aes128").unwrap(), Encryption::Aes128);
        assert!(Encryption::parse("zipcrypto").is_err());
    }
}
