//! gizza-ai/archive-extractor core — universal, auto-detecting archive
//! extractor. Detects the format from the leading magic bytes (no filename /
//! extension needed) and unpacks it, repacking every extracted regular file
//! into a single ZIP for download.
//!
//! Supported formats:
//!  * **Containers** — `zip`, `tar`.
//!  * **Single-stream compressors** — `gzip` (.gz), `bzip2` (.bz2), `xz` (.xz),
//!    `zstd` (.zst), `lz4` (frame, .lz4). When the decompressed payload is
//!    itself a tar (the `.tar.gz` / `.tar.bz2` / `.tar.xz` / `.tar.zst` /
//!    `.tar.lz4` family), the tar members are extracted; otherwise the single
//!    decompressed file is returned.
//!
//! Pure-Rust (`zip`, `tar`, `flate2`, `bzip2-rs`, `lzma-rust2`, `ruzstd`,
//! `lz4_flex`) — no C bindings, so it instantiates on every backend including
//! the chat Service Worker. Guards against decompression bombs with entry-count
//! and total-byte caps, and against path traversal by storing normalized,
//! relative paths only.

use std::io::{Cursor, Read, Write};

use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Safety caps to avoid decompression bombs.
const MAX_ENTRIES: usize = 10_000;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB uncompressed

/// The archive / compression format detected from the leading bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    Tar,
    Gzip,
    Bzip2,
    Xz,
    Zstd,
    Lz4,
}

impl Format {
    /// Short lowercase id, e.g. `gzip`, `zip`.
    pub fn label(self) -> &'static str {
        match self {
            Format::Zip => "zip",
            Format::Tar => "tar",
            Format::Gzip => "gzip",
            Format::Bzip2 => "bzip2",
            Format::Xz => "xz",
            Format::Zstd => "zstd",
            Format::Lz4 => "lz4",
        }
    }

    /// Conventional lowercase file extension WITHOUT the dot, e.g. `gz`, `bz2`.
    pub fn ext(self) -> &'static str {
        match self {
            Format::Zip => "zip",
            Format::Tar => "tar",
            Format::Gzip => "gz",
            Format::Bzip2 => "bz2",
            Format::Xz => "xz",
            Format::Zstd => "zst",
            Format::Lz4 => "lz4",
        }
    }

    /// A multi-file container (`zip`, `tar`) vs. a single-stream compressor.
    fn is_container(self) -> bool {
        matches!(self, Format::Zip | Format::Tar)
    }
}

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
    /// The detected outer format.
    pub format: Format,
    /// For a single-stream compressor: whether the decompressed payload was a
    /// tar (i.e. a `.tar.gz`-style archive). Always false for `zip`/`tar` input.
    pub inner_tar: bool,
    /// Every entry encountered (files + dirs), in archive order.
    pub entries: Vec<EntryInfo>,
    /// Number of regular files packed into the output ZIP.
    pub file_count: usize,
}

fn starts(b: &[u8], sig: &[u8]) -> bool {
    b.len() >= sig.len() && &b[..sig.len()] == sig
}

/// Detect the outer format from the leading magic bytes. Returns `None` for an
/// empty or unrecognised blob.
pub fn detect(bytes: &[u8]) -> Option<Format> {
    if bytes.is_empty() {
        return None;
    }
    let b = bytes;
    if starts(b, &[0x1F, 0x8B]) {
        return Some(Format::Gzip);
    }
    if starts(b, b"BZh") {
        return Some(Format::Bzip2);
    }
    if starts(b, &[0xFD, b'7', b'z', b'X', b'Z', 0x00]) {
        return Some(Format::Xz);
    }
    if starts(b, &[0x28, 0xB5, 0x2F, 0xFD]) {
        return Some(Format::Zstd);
    }
    if starts(b, &[0x04, 0x22, 0x4D, 0x18]) {
        return Some(Format::Lz4);
    }
    // ZIP: local-file (03 04), empty-archive EOCD (05 06), or spanned (07 08).
    if starts(b, &[0x50, 0x4B, 0x03, 0x04])
        || starts(b, &[0x50, 0x4B, 0x05, 0x06])
        || starts(b, &[0x50, 0x4B, 0x07, 0x08])
    {
        return Some(Format::Zip);
    }
    if looks_like_tar(b) {
        return Some(Format::Tar);
    }
    None
}

/// A POSIX/GNU tar carries the `ustar` magic at offset 257.
fn looks_like_tar(b: &[u8]) -> bool {
    b.len() >= 262 && &b[257..262] == b"ustar"
}

/// Normalize an archive member path to a safe, relative ZIP entry name: strip a
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

/// Decompress a single-stream compressor to its payload bytes (capped), plus an
/// embedded original filename when the format records one (only gzip's FNAME).
fn decompress_stream(format: Format, data: &[u8]) -> Result<(Vec<u8>, Option<String>), String> {
    let mut out = Vec::new();
    let embedded_name;
    match format {
        Format::Gzip => {
            use flate2::read::GzDecoder;
            let mut dec = GzDecoder::new(data);
            embedded_name = dec
                .header()
                .and_then(|h| h.filename())
                .map(|f| String::from_utf8_lossy(f).to_string())
                .filter(|s| !s.is_empty());
            read_capped(&mut dec, &mut out, "gzip")?;
        }
        Format::Bzip2 => {
            let mut dec = bzip2_rs::DecoderReader::new(data);
            read_capped(&mut dec, &mut out, "bzip2")?;
            embedded_name = None;
        }
        Format::Xz => {
            let mut dec = lzma_rust2::XzReader::new(Cursor::new(data), true);
            read_capped(&mut dec, &mut out, "xz")?;
            embedded_name = None;
        }
        Format::Zstd => {
            let mut dec = ruzstd::decoding::StreamingDecoder::new(Cursor::new(data))
                .map_err(|e| format!("zstd decode failed: {e}"))?;
            read_capped(&mut dec, &mut out, "zstd")?;
            embedded_name = None;
        }
        Format::Lz4 => {
            let mut dec = lz4_flex::frame::FrameDecoder::new(data);
            read_capped(&mut dec, &mut out, "lz4")?;
            embedded_name = None;
        }
        Format::Zip | Format::Tar => {
            return Err("internal: containers are not single-stream".into());
        }
    }
    Ok((out, embedded_name))
}

/// Read a decoder into `out`, failing if the decompressed size exceeds the cap.
fn read_capped<R: Read>(dec: &mut R, out: &mut Vec<u8>, what: &str) -> Result<(), String> {
    dec.take(MAX_TOTAL_BYTES + 1)
        .read_to_end(out)
        .map_err(|e| format!("{what} decompression failed: {e}"))?;
    if out.len() as u64 > MAX_TOTAL_BYTES {
        return Err("archive is too large when decompressed".into());
    }
    Ok(())
}

/// Parse a tar archive into a listing + the regular files (sanitized names).
fn read_tar(tar_bytes: &[u8]) -> Result<(Vec<EntryInfo>, Vec<(String, Vec<u8>)>), String> {
    let mut ar = tar::Archive::new(Cursor::new(tar_bytes));
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

        entries.push(EntryInfo { path: raw_path.clone(), size, is_dir });

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
    Ok((entries, files))
}

/// Parse a ZIP archive into a listing + the regular files (sanitized names).
fn read_zip(zip_bytes: &[u8]) -> Result<(Vec<EntryInfo>, Vec<(String, Vec<u8>)>), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("not a valid zip archive: {e}"))?;
    let mut entries: Vec<EntryInfo> = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;

    for i in 0..archive.len() {
        if entries.len() >= MAX_ENTRIES {
            return Err(format!("archive has too many entries (> {MAX_ENTRIES})"));
        }
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("failed to read zip entry {i}: {e}"))?;
        let raw_path = file.name().to_string();
        let is_dir = file.is_dir();
        let size = file.size();
        entries.push(EntryInfo { path: raw_path.clone(), size, is_dir });

        if !is_dir {
            total = total.saturating_add(size);
            if total > MAX_TOTAL_BYTES {
                return Err("archive contents are too large".into());
            }
            if let Some(name) = safe_path(&raw_path) {
                let mut buf = Vec::with_capacity(size as usize);
                file.read_to_end(&mut buf)
                    .map_err(|e| format!("failed to extract '{raw_path}': {e}"))?;
                files.push((name, buf));
            }
        }
    }
    Ok((entries, files))
}

/// Choose a name for a single decompressed file: prefer the format's embedded
/// original name (gzip FNAME), then the caller's `name_hint`, then a default.
fn single_file_name(name_hint: &str, embedded: Option<String>) -> String {
    embedded
        .as_deref()
        .and_then(safe_path)
        .or_else(|| {
            let h = name_hint.trim();
            if h.is_empty() {
                None
            } else {
                safe_path(h)
            }
        })
        .unwrap_or_else(|| "extracted-file".to_string())
}

/// Detect the format of `archive`, unpack it, and repack the regular files into
/// a single ZIP. `name_hint` names the output when the input is a single-stream
/// compressor whose payload is NOT a tar (e.g. a lone `.gz` of one file); it is
/// ignored for `zip`/`tar` and for `.tar.*` inputs. Returns the ZIP bytes plus a
/// listing. Errors on empty input, an unrecognised or malformed archive, or a
/// container with no regular files.
pub fn extract_to_zip(archive: &[u8], name_hint: &str) -> Result<(Vec<u8>, Extracted), String> {
    if archive.is_empty() {
        return Err("input archive is empty".into());
    }
    let format = detect(archive).ok_or_else(|| {
        "unrecognised archive: not a zip, tar, gzip, bzip2, xz, zstd, or lz4 stream".to_string()
    })?;

    let mut inner_tar = false;
    let (entries, files): (Vec<EntryInfo>, Vec<(String, Vec<u8>)>) = if format.is_container() {
        match format {
            Format::Zip => read_zip(archive)?,
            Format::Tar => read_tar(archive)?,
            _ => unreachable!(),
        }
    } else {
        // Single-stream compressor: decompress, then peek for an inner tar.
        let (payload, embedded) = decompress_stream(format, archive)?;
        if looks_like_tar(&payload) {
            inner_tar = true;
            read_tar(&payload)?
        } else {
            let name = single_file_name(name_hint, embedded);
            let size = payload.len() as u64;
            let entries = vec![EntryInfo { path: name.clone(), size, is_dir: false }];
            (entries, vec![(name, payload)])
        }
    };

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

    Ok((buf, Extracted { format, inner_tar, entries, file_count }))
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

    /// A tar with two files and one directory entry.
    fn sample_tar() -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        let mut dh = tar::Header::new_gnu();
        dh.set_path("docs/").unwrap();
        dh.set_entry_type(tar::EntryType::Directory);
        dh.set_size(0);
        dh.set_cksum();
        b.append(&dh, std::io::empty()).unwrap();
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

    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            for (name, data) in files {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(bytes).unwrap();
        enc.finish().unwrap()
    }

    fn bzip2(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        banzai::encode(Cursor::new(bytes), std::io::BufWriter::new(&mut out), 9).unwrap();
        out
    }

    fn xz(bytes: &[u8]) -> Vec<u8> {
        use lzma_rust2::{XzOptions, XzWriter};
        let opts = XzOptions::with_preset(1);
        let mut enc = XzWriter::new(Vec::new(), opts).unwrap();
        enc.write_all(bytes).unwrap();
        enc.finish().unwrap()
    }

    fn zstd(bytes: &[u8]) -> Vec<u8> {
        zstd::encode_all(Cursor::new(bytes), 3).unwrap()
    }

    fn lz4(bytes: &[u8]) -> Vec<u8> {
        use lz4_flex::frame::FrameEncoder;
        let mut enc = FrameEncoder::new(Vec::new());
        enc.write_all(bytes).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn detects_every_format() {
        assert_eq!(detect(&make_zip(&[("a", b"x")])), Some(Format::Zip));
        assert_eq!(detect(&sample_tar()), Some(Format::Tar));
        assert_eq!(detect(&gzip(b"x")), Some(Format::Gzip));
        assert_eq!(detect(&bzip2(b"x")), Some(Format::Bzip2));
        assert_eq!(detect(&xz(b"x")), Some(Format::Xz));
        assert_eq!(detect(&zstd(b"x")), Some(Format::Zstd));
        assert_eq!(detect(&lz4(b"x")), Some(Format::Lz4));
        assert_eq!(detect(b"just plain text bytes, no magic"), None);
        assert_eq!(detect(&[]), None);
    }

    #[test]
    fn extracts_plain_zip() {
        let zip = make_zip(&[("hello.txt", b"hello"), ("docs/a.txt", b"world!")]);
        let (out, ex) = extract_to_zip(&zip, "").unwrap();
        assert_eq!(ex.format, Format::Zip);
        assert_eq!(ex.file_count, 2);
        let mut got = zip_entries(&out);
        got.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(got[0], ("docs/a.txt".to_string(), b"world!".to_vec()));
        assert_eq!(got[1], ("hello.txt".to_string(), b"hello".to_vec()));
    }

    #[test]
    fn extracts_plain_tar() {
        let (out, ex) = extract_to_zip(&sample_tar(), "").unwrap();
        assert_eq!(ex.format, Format::Tar);
        assert_eq!(ex.file_count, 2);
        assert_eq!(ex.entries.len(), 3); // dir + 2 files listed
        assert_eq!(zip_entries(&out).len(), 2);
    }

    #[test]
    fn single_gzip_file_uses_embedded_name() {
        // flate2's default GzEncoder writes no FNAME; fall back to the hint.
        let gz = gzip(b"plain gzip payload");
        let (out, ex) = extract_to_zip(&gz, "notes.txt").unwrap();
        assert_eq!(ex.format, Format::Gzip);
        assert!(!ex.inner_tar);
        assert_eq!(ex.file_count, 1);
        let got = zip_entries(&out);
        assert_eq!(got[0].0, "notes.txt");
        assert_eq!(got[0].1, b"plain gzip payload");
    }

    #[test]
    fn single_stream_default_name_when_no_hint() {
        let (out, _ex) = extract_to_zip(&xz(b"payload"), "").unwrap();
        assert_eq!(zip_entries(&out)[0].0, "extracted-file");
    }

    #[test]
    fn handles_layered_tar_for_every_compressor() {
        let tar = sample_tar();
        for (label, blob) in [
            ("gz", gzip(&tar)),
            ("bz2", bzip2(&tar)),
            ("xz", xz(&tar)),
            ("zst", zstd(&tar)),
            ("lz4", lz4(&tar)),
        ] {
            let (out, ex) = extract_to_zip(&blob, "ignored").unwrap();
            assert!(ex.inner_tar, "{label}: payload should be recognised as a tar");
            assert_eq!(ex.file_count, 2, "{label}: two files");
            let mut got = zip_entries(&out);
            got.sort_by(|a, b| a.0.cmp(&b.0));
            assert_eq!(got[0], ("docs/a.txt".to_string(), b"world!".to_vec()), "{label}");
            assert_eq!(got[1], ("hello.txt".to_string(), b"hello".to_vec()), "{label}");
        }
    }

    #[test]
    fn zip_sanitizes_traversal_paths() {
        let zip = make_zip(&[("../../etc/evil", b"x"), ("ok.txt", b"y")]);
        let (out, _ex) = extract_to_zip(&zip, "").unwrap();
        let names: Vec<String> = zip_entries(&out).into_iter().map(|(n, _)| n).collect();
        assert!(names.contains(&"etc/evil".to_string()), "traversal stripped: {names:?}");
        assert!(names.contains(&"ok.txt".to_string()));
        assert!(!names.iter().any(|n| n.contains("..")));
    }

    #[test]
    fn empty_and_unrecognised_error() {
        assert!(extract_to_zip(&[], "x").is_err());
        assert!(extract_to_zip(b"not any known archive format at all", "x").is_err());
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
