//! gizza-ai/7z-extract core — extract a `.7z` archive (including AES-256
//! encrypted ones) entirely in memory and repack every regular file into a
//! single ZIP for download.
//!
//! Reads via [`sevenz_rust2::ArchiveReader::new`] over an in-memory
//! `Cursor<Vec<u8>>` — never the crate's std::fs `open`/`decompress` helpers —
//! so it instantiates on every backend, including the chat Service Worker.
//! Supports the LZMA and LZMA2 codecs (the 7z defaults, always built in) plus
//! AES-256 decryption (the `aes256` feature). Guards against decompression
//! bombs with entry-count and total-byte caps, and against path traversal by
//! storing normalized, relative paths only.
//!
//! Not supported (rare in real `.7z` files, which default to LZMA2): the
//! BZip2, Zstandard, PPMd, Brotli and Deflate 7z codecs — a member compressed
//! with one of those yields a clear "unsupported 7z compression method" error.

use std::io::{Cursor, Read, Write};

use sevenz_rust2::{ArchiveReader, Error as SzError, Password};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Safety caps to avoid decompression bombs.
const MAX_ENTRIES: usize = 10_000;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB uncompressed

/// The 7z file signature: `7z\xBC\xAF\x27\x1C`.
const SEVENZ_MAGIC: [u8; 6] = [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];

/// One archive member, for the caller's listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

/// Result of extracting a `.7z` archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    /// Every entry encountered (files + directories), in archive order.
    pub entries: Vec<EntryInfo>,
    /// Number of regular files packed into the output ZIP.
    pub file_count: usize,
    /// Whether a non-empty password was used to open the archive.
    pub encrypted: bool,
}

/// Whether `bytes` begins with the 7z signature.
pub fn is_7z(bytes: &[u8]) -> bool {
    bytes.len() >= 6 && bytes[..6] == SEVENZ_MAGIC
}

/// Normalize an archive member path to a safe, relative ZIP entry name: convert
/// `\` to `/`, strip a leading `/`, drop `.`/`..` components and Windows drive
/// prefixes. Returns `None` if nothing usable remains.
fn safe_path(raw: &str) -> Option<String> {
    let raw = raw.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for comp in raw.split('/') {
        match comp {
            "" | "." => continue,
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

/// Map a failure from [`ArchiveReader::new`] (opening / reading the 7z header)
/// to a user-facing message. When `-mhe=on` header encryption is used, the
/// header itself is encrypted, so a missing/wrong password fails here.
fn map_open_error(e: SzError, has_password: bool) -> String {
    match e {
        SzError::PasswordRequired => {
            "this .7z has an encrypted header — provide the archive password via the `password` parameter".into()
        }
        SzError::MaybeBadPassword(_) => {
            "incorrect password for this encrypted .7z archive".into()
        }
        SzError::BadSignature(_) => {
            "not a valid .7z archive (bad signature)".into()
        }
        SzError::ChecksumVerificationFailed | SzError::NextHeaderCrcMismatch if has_password => {
            "could not open the .7z — the password may be incorrect, or the archive is corrupt".into()
        }
        other => format!("could not open the .7z archive: {other}"),
    }
}

/// Map a failure encountered while streaming an entry's content.
fn map_read_error(e: SzError, has_password: bool) -> String {
    match e {
        SzError::PasswordRequired => {
            "this .7z archive is encrypted — provide the `password` parameter".into()
        }
        SzError::MaybeBadPassword(_) => "incorrect password for this encrypted .7z archive".into(),
        SzError::UnsupportedCompressionMethod(m) => format!(
            "unsupported 7z compression method: {m}. This tool supports LZMA and LZMA2 (the 7z \
             defaults) plus AES-256 encryption; BZip2/Zstd/PPMd/Brotli/Deflate-compressed 7z \
             entries are not supported"
        ),
        SzError::ChecksumVerificationFailed if has_password => {
            "could not decrypt the .7z — the password is likely incorrect".into()
        }
        other => format!("failed to extract the .7z archive: {other}"),
    }
}

/// Extract every regular file from a `.7z` archive and repack them into a single
/// ZIP. Pass an empty `password` for unencrypted archives; pass the password for
/// AES-256-encrypted ones. Returns the ZIP bytes plus a listing. Errors on empty
/// input, a non-7z or malformed archive, a missing/incorrect password, an
/// unsupported codec, or an archive with no regular files.
pub fn extract_to_zip(archive: &[u8], password: &str) -> Result<(Vec<u8>, Extracted), String> {
    if archive.is_empty() {
        return Err("input archive is empty".into());
    }
    if !is_7z(archive) {
        return Err(
            "not a .7z archive: the file does not start with the 7z signature (37 7A BC AF 27 1C)"
                .into(),
        );
    }
    let has_password = !password.is_empty();
    let pw = if has_password {
        Password::new(password)
    } else {
        Password::empty()
    };

    let mut reader = ArchiveReader::new(Cursor::new(archive.to_vec()), pw)
        .map_err(|e| map_open_error(e, has_password))?;

    let mut entries: Vec<EntryInfo> = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;
    let mut cap_err: Option<String> = None;
    let mut read_err: Option<String> = None;

    let each = reader.for_each_entries(|entry, rd| {
        if entries.len() >= MAX_ENTRIES {
            cap_err = Some(format!("archive has too many entries (> {MAX_ENTRIES})"));
            return Ok(false);
        }
        let name = entry.name().to_string();
        let is_dir = entry.is_directory();
        entries.push(EntryInfo {
            path: name.clone(),
            size: entry.size(),
            is_dir,
        });
        if is_dir {
            return Ok(true);
        }
        // Read the (already-decompressed/decrypted) entry content, capped so the
        // total extracted size can't exceed MAX_TOTAL_BYTES (decompression bomb).
        let remaining = MAX_TOTAL_BYTES.saturating_sub(total).saturating_add(1);
        let mut buf = Vec::new();
        if let Err(e) = rd.take(remaining).read_to_end(&mut buf) {
            read_err = Some(format!("failed reading '{name}': {e}"));
            return Ok(false);
        }
        total = total.saturating_add(buf.len() as u64);
        if total > MAX_TOTAL_BYTES {
            cap_err = Some("archive contents are too large when extracted".into());
            return Ok(false);
        }
        if let Some(sname) = safe_path(&name) {
            files.push((sname, buf));
        }
        Ok(true)
    });

    if let Some(msg) = cap_err {
        return Err(msg);
    }
    if let Some(msg) = read_err {
        return Err(msg);
    }
    each.map_err(|e| map_read_error(e, has_password))?;

    if files.is_empty() {
        return Err("the .7z archive contains no extractable files".into());
    }

    let file_count = files.len();
    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in &files {
            zw.start_file(name, opts)
                .map_err(|e| format!("zip start_file: {e}"))?;
            zw.write_all(bytes).map_err(|e| format!("zip write: {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }

    Ok((
        buf,
        Extracted {
            entries,
            file_count,
            encrypted: has_password,
        },
    ))
}

/// Derive the output ZIP filename: strip a trailing `.7z` (case-insensitive) and
/// append `.zip`; fall back to `archive.zip`.
pub fn output_zip_name(in_filename: &str) -> String {
    let lower = in_filename.to_lowercase();
    if let Some(stem) = lower.strip_suffix(".7z") {
        if !stem.is_empty() {
            return format!("{}.zip", &in_filename[..stem.len()]);
        }
    }
    match in_filename.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => format!("{stem}.zip"),
        _ if in_filename.is_empty() => "archive.zip".to_string(),
        _ => format!("{in_filename}.zip"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN_7Z: &[u8] = include_bytes!("../tests/data/plain.7z");
    const ENCRYPTED_7Z: &[u8] = include_bytes!("../tests/data/encrypted.7z");

    /// Read a produced ZIP back into (name, bytes) pairs, sorted by name.
    fn zip_entries(zip_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).expect("valid zip");
        let mut out = Vec::new();
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes).unwrap();
            out.push((name, bytes));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    #[test]
    fn extracts_plain_lzma2_archive() {
        let (zip, ex) = extract_to_zip(PLAIN_7Z, "").expect("plain .7z extracts");
        assert_eq!(ex.file_count, 3);
        assert!(!ex.encrypted);
        let files = zip_entries(&zip);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["data.bin", "docs/readme.md", "hello.txt"]);
        let hello = &files.iter().find(|(n, _)| n == "hello.txt").unwrap().1;
        assert_eq!(hello, b"hello from 7z\n");
        let readme = &files.iter().find(|(n, _)| n == "docs/readme.md").unwrap().1;
        assert_eq!(readme, b"nested readme\n");
    }

    #[test]
    fn extracts_aes256_encrypted_archive_with_password() {
        let (zip, ex) =
            extract_to_zip(ENCRYPTED_7Z, "gizza123").expect("encrypted .7z extracts with password");
        assert_eq!(ex.file_count, 3);
        assert!(ex.encrypted);
        let files = zip_entries(&zip);
        let hello = &files.iter().find(|(n, _)| n == "hello.txt").unwrap().1;
        assert_eq!(hello, b"hello from 7z\n");
    }

    #[test]
    fn encrypted_archive_without_password_asks_for_one() {
        // -mhe=on encrypts the header, so opening without a password fails with
        // a clear "provide the password" message (not a panic or a raw error).
        let err = extract_to_zip(ENCRYPTED_7Z, "").expect_err("missing password must error");
        assert!(
            err.contains("password"),
            "expected a password hint, got: {err}"
        );
    }

    #[test]
    fn encrypted_archive_with_wrong_password_errors() {
        let err = extract_to_zip(ENCRYPTED_7Z, "wrongpass").expect_err("wrong password must error");
        assert!(
            err.to_lowercase().contains("password") || err.to_lowercase().contains("corrupt"),
            "expected a password/corrupt hint, got: {err}"
        );
    }

    #[test]
    fn rejects_non_7z_input() {
        let err = extract_to_zip(b"PK\x03\x04 this is a zip, not 7z", "")
            .expect_err("non-7z must error");
        assert!(err.contains("not a .7z archive"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = extract_to_zip(&[], "").expect_err("empty must error");
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn is_7z_detects_signature() {
        assert!(is_7z(PLAIN_7Z));
        assert!(is_7z(ENCRYPTED_7Z));
        assert!(!is_7z(b"PK\x03\x04"));
        assert!(!is_7z(&[0x37, 0x7A]));
        assert!(!is_7z(&[]));
    }

    #[test]
    fn output_zip_name_maps_extension() {
        assert_eq!(output_zip_name("project.7z"), "project.zip");
        assert_eq!(output_zip_name("Backup.7Z"), "Backup.zip");
        assert_eq!(output_zip_name("data.tar.7z"), "data.tar.zip");
        assert_eq!(output_zip_name("noext"), "noext.zip");
        assert_eq!(output_zip_name(""), "archive.zip");
    }

    #[test]
    fn safe_path_strips_traversal() {
        assert_eq!(safe_path("../../etc/passwd").as_deref(), Some("etc/passwd"));
        assert_eq!(safe_path("/abs/path.txt").as_deref(), Some("abs/path.txt"));
        assert_eq!(safe_path("a\\b\\c.txt").as_deref(), Some("a/b/c.txt"));
        assert_eq!(safe_path(".."), None);
        assert_eq!(safe_path(""), None);
    }
}
