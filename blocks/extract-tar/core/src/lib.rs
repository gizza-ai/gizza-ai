//! gizza-ai/extract-tar core — pure-Rust listing + extraction of tar / tar.gz /
//! tgz archives, re-bundled into a ZIP. No wafer/wasm-bindgen deps.
//!
//! Auto-detects gzip (magic `1F 8B`) and inflates it first, then parses the tar.
//! Regular files are repacked into a single ZIP (deflate); directories and other
//! special entries are listed but not packed. Guards against decompression bombs
//! with entry-count and total-byte caps, and against path traversal by storing
//! normalized, relative paths only.

use std::io::{Cursor, Read, Write};

use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// One archive member, for the caller's listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

/// Result of extracting an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    /// Every entry encountered (files + dirs), in archive order.
    pub entries: Vec<EntryInfo>,
    /// Number of regular files packed into the ZIP.
    pub file_count: usize,
}

/// Safety caps to avoid decompression bombs.
const MAX_ENTRIES: usize = 10_000;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB uncompressed

/// Normalize a tar member path to a safe, relative ZIP entry name: strip a
/// leading `/`, drop `.`/`..` components and Windows drive prefixes. Returns
/// `None` if nothing usable remains.
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

/// Gunzip `bytes` if they carry the gzip magic; otherwise return them unchanged.
fn maybe_gunzip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() >= 2 && bytes[0] == 0x1F && bytes[1] == 0x8B {
        use flate2::read::GzDecoder;
        let mut out = Vec::new();
        let mut dec = GzDecoder::new(bytes);
        // Cap the inflated size to guard against gzip bombs.
        let mut limited = (&mut dec).take(MAX_TOTAL_BYTES + 1);
        limited
            .read_to_end(&mut out)
            .map_err(|e| format!("gzip decode failed: {e}"))?;
        if out.len() as u64 > MAX_TOTAL_BYTES {
            return Err("archive is too large when decompressed".into());
        }
        Ok(out)
    } else {
        Ok(bytes.to_vec())
    }
}

/// Parse a (possibly gzipped) tar archive, repack its regular files into a ZIP,
/// and return the ZIP bytes plus a listing. Errors on empty input, a malformed
/// archive, or an archive with no regular files.
pub fn extract_to_zip(archive: &[u8]) -> Result<(Vec<u8>, Extracted), String> {
    if archive.is_empty() {
        return Err("input archive is empty".into());
    }
    let tar_bytes = maybe_gunzip(archive)?;

    let mut ar = tar::Archive::new(Cursor::new(&tar_bytes));
    let mut entries: Vec<EntryInfo> = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;

    let iter = ar
        .entries()
        .map_err(|e| format!("not a valid tar archive: {e}"))?;
    for entry in iter {
        let mut entry = entry.map_err(|e| format!("corrupt tar entry: {e}"))?;
        if entries.len() >= MAX_ENTRIES {
            return Err(format!("archive has too many entries (> {MAX_ENTRIES})"));
        }
        let header = entry.header();
        let size = header.size().unwrap_or(0);
        let is_dir = header.entry_type().is_dir();
        let is_file = header.entry_type().is_file();
        let raw_path = entry
            .path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        entries.push(EntryInfo {
            path: raw_path.clone(),
            size,
            is_dir,
        });

        if is_file {
            total = total.saturating_add(size);
            if total > MAX_TOTAL_BYTES {
                return Err("archive contents are too large".into());
            }
            if let Some(name) = safe_path(&raw_path) {
                let mut buf = Vec::with_capacity(size as usize);
                entry
                    .read_to_end(&mut buf)
                    .map_err(|e| format!("failed reading '{raw_path}': {e}"))?;
                files.push((name, buf));
            }
        }
    }

    if files.is_empty() {
        return Err("the archive contains no extractable files".into());
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

    Ok((buf, Extracted { entries, file_count }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        out
    }

    /// Build a tar with two files and one directory entry.
    fn sample_tar() -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        // directory entry
        let mut dh = tar::Header::new_gnu();
        dh.set_path("docs/").unwrap();
        dh.set_entry_type(tar::EntryType::Directory);
        dh.set_size(0);
        dh.set_cksum();
        b.append(&dh, std::io::empty()).unwrap();
        // files
        let mut h1 = tar::Header::new_gnu();
        h1.set_size(5);
        h1.set_cksum();
        b.append_data(&mut h1, "hello.txt", &b"hello"[..]).unwrap();
        let mut h2 = tar::Header::new_gnu();
        h2.set_size(6);
        h2.set_cksum();
        b.append_data(&mut h2, "docs/a.txt", &b"world!"[..]).unwrap();
        b.into_inner().unwrap()
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(bytes).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn extracts_plain_tar_files() {
        let tar = sample_tar();
        let (zip, ex) = extract_to_zip(&tar).unwrap();
        assert_eq!(ex.file_count, 2);
        // directory + 2 files listed
        assert_eq!(ex.entries.len(), 3);
        let mut got = zip_entries(&zip);
        got.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(got[0].0, "docs/a.txt");
        assert_eq!(got[0].1, b"world!");
        assert_eq!(got[1].0, "hello.txt");
        assert_eq!(got[1].1, b"hello");
    }

    #[test]
    fn handles_gzipped_tar() {
        let tgz = gzip(&sample_tar());
        let (zip, ex) = extract_to_zip(&tgz).unwrap();
        assert_eq!(ex.file_count, 2);
        assert_eq!(zip_entries(&zip).len(), 2);
    }

    #[test]
    fn empty_and_garbage_error() {
        assert!(extract_to_zip(&[]).is_err());
        assert!(extract_to_zip(b"not a tar at all, just some bytes").is_err());
    }

    #[test]
    fn safe_path_strips_traversal_and_absolute() {
        assert_eq!(safe_path("/etc/passwd").as_deref(), Some("etc/passwd"));
        assert_eq!(safe_path("../../secret").as_deref(), Some("secret"));
        assert_eq!(safe_path("a/./b/../c").as_deref(), Some("a/c"));
        assert_eq!(safe_path("/").as_deref(), None);
        assert_eq!(safe_path("dir\\file.txt").as_deref(), Some("dir/file.txt"));
    }
}
