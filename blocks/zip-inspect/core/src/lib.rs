//! gizza-ai/zip-inspect core — list the contents of a .zip archive WITHOUT
//! extracting anything. Pure-Rust (`zip`, deflate only). No wafer/wasm-bindgen deps.
//!
//! Inspection reads only the ZIP central directory (entry names, uncompressed and
//! compressed sizes, compression method, CRC-32, modification time, directory flag,
//! and the encrypted flag). It NEVER decompresses entry data, so it can report the
//! method label even for archives whose entries use a method we cannot decode.

use serde::Serialize;
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    /// Full path of the entry inside the archive.
    pub name: String,
    /// Uncompressed size in bytes.
    pub size: u64,
    /// Compressed (stored) size in bytes.
    pub compressed_size: u64,
    /// Compression method label, e.g. "Stored", "Deflated", "Bzip2".
    pub method: String,
    /// CRC-32 checksum of the uncompressed data, as 8-char lowercase hex.
    pub crc32: String,
    /// Compression ratio as a percentage saved (0..=100, rounded to 1 decimal):
    /// `(size - compressed_size) / size * 100`; 0.0 when `size` is 0.
    pub ratio_pct: f64,
    /// True for directory entries (their name ends with '/').
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_dir: bool,
    /// True when the entry's data is encrypted.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub encrypted: bool,
    /// Last-modified timestamp, when the archive records one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Listing {
    pub entries: Vec<Entry>,
    /// Number of entries (files + directories).
    pub count: usize,
    /// Number of regular files (excludes directory entries).
    pub file_count: usize,
    /// Number of directory entries.
    pub dir_count: usize,
    /// Total uncompressed size of all files, in bytes.
    pub total_size: u64,
    /// Total compressed size of all files, in bytes.
    pub total_compressed_size: u64,
    /// Overall compression ratio (percent saved across all files), rounded.
    pub total_ratio_pct: f64,
}

fn ratio_pct(size: u64, compressed: u64) -> f64 {
    if size == 0 {
        return 0.0;
    }
    let saved = size.saturating_sub(compressed) as f64 / size as f64 * 100.0;
    (saved * 10.0).round() / 10.0
}

/// Inspect `zip_bytes` and return a listing of every entry, without extracting
/// or decompressing any data.
pub fn inspect(zip_bytes: &[u8]) -> Result<Listing, String> {
    let reader = Cursor::new(zip_bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("not a valid zip archive: {e}"))?;

    let mut entries = Vec::with_capacity(archive.len());
    let mut total_size = 0u64;
    let mut total_compressed = 0u64;
    let mut file_count = 0usize;
    let mut dir_count = 0usize;

    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| format!("failed to read zip entry {i}: {e}"))?;
        let name = file.name().to_string();
        let is_dir = file.is_dir();
        let size = file.size();
        let compressed_size = file.compressed_size();
        let method = file.compression().to_string();
        let crc32 = format!("{:08x}", file.crc32());
        let encrypted = file.encrypted();
        let modified = file.last_modified().map(|d| d.to_string());

        if is_dir {
            dir_count += 1;
        } else {
            file_count += 1;
            total_size += size;
            total_compressed += compressed_size;
        }

        entries.push(Entry {
            name,
            size,
            compressed_size,
            method,
            crc32,
            ratio_pct: ratio_pct(size, compressed_size),
            is_dir,
            encrypted,
            modified,
        });
    }

    Ok(Listing {
        count: entries.len(),
        file_count,
        dir_count,
        total_ratio_pct: ratio_pct(total_size, total_compressed),
        total_size,
        total_compressed_size: total_compressed,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn make_zip() -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            // A deflated, compressible file.
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            w.start_file("docs/readme.txt", opts).unwrap();
            w.write_all(&vec![b'a'; 2000]).unwrap();
            // A stored (uncompressed) file.
            let stored =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            w.start_file("data.bin", stored).unwrap();
            w.write_all(b"0123456789").unwrap();
            // An explicit directory entry.
            w.add_directory("docs", SimpleFileOptions::default()).unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn lists_entries_with_metadata() {
        let zip = make_zip();
        let l = inspect(&zip).unwrap();
        assert_eq!(l.count, 3);
        assert_eq!(l.file_count, 2);
        assert_eq!(l.dir_count, 1);

        let readme = l.entries.iter().find(|e| e.name == "docs/readme.txt").unwrap();
        assert_eq!(readme.size, 2000);
        assert_eq!(readme.method, "Deflated");
        assert!(!readme.is_dir);
        // 2000 highly-compressible bytes -> stored size well below original.
        assert!(readme.compressed_size < readme.size);
        assert!(readme.ratio_pct > 0.0);
        // CRC is 8 lowercase hex chars.
        assert_eq!(readme.crc32.len(), 8);
        assert!(readme.crc32.chars().all(|c| c.is_ascii_hexdigit()));

        let bin = l.entries.iter().find(|e| e.name == "data.bin").unwrap();
        assert_eq!(bin.method, "Stored");
        assert_eq!(bin.size, 10);
        assert_eq!(bin.compressed_size, 10);
        assert_eq!(bin.ratio_pct, 0.0);

        let dir = l.entries.iter().find(|e| e.is_dir).unwrap();
        assert!(dir.name.ends_with('/'));
        // Totals exclude the directory entry.
        assert_eq!(l.total_size, 2010);
    }

    #[test]
    fn errors_on_non_zip() {
        assert!(inspect(b"definitely not a zip").is_err());
    }

    #[test]
    fn empty_size_ratio_is_zero() {
        assert_eq!(ratio_pct(0, 0), 0.0);
        assert_eq!(ratio_pct(100, 100), 0.0);
        assert_eq!(ratio_pct(100, 50), 50.0);
    }
}
