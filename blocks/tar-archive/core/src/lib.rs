//! gizza-ai/tar-archive core — pure-Rust tar bundling shared by the chat skill
//! block + CLI. No wafer/wasm-bindgen deps. Takes named file byte buffers and
//! writes a single USTAR `.tar` archive, then optionally compresses the whole
//! archive into `.tar.gz` (gzip), `.tar.bz2` (bzip2) or `.tar.xz` (xz) — exactly
//! the four formats produced by `tar -c[z|j|J]`. Duplicate entry names are made
//! unique (`name (2).ext`). Compression reuses the same wasm-safe encoders as
//! the standalone gzip/bzip2/lzma tools, so output is byte-compatible.

use std::collections::HashSet;
use std::io::Write;

use flate2::write::GzEncoder;
use flate2::Compression as GzLevel;

/// Output archive compression. `None` => plain `.tar`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    None,
    Gzip,
    Bzip2,
    Xz,
}

impl Compression {
    /// Parse from the `compression` param value (case-insensitive). Accepts a
    /// few common aliases (`tgz`, `bz2`, `lzma`).
    pub fn parse(s: &str) -> Result<Compression, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" | "tar" | "" => Ok(Compression::None),
            "gzip" | "gz" | "tgz" => Ok(Compression::Gzip),
            "bzip2" | "bz2" | "tbz2" => Ok(Compression::Bzip2),
            "xz" | "lzma" | "txz" => Ok(Compression::Xz),
            other => Err(format!(
                "unknown compression '{other}' (use none, gzip, bzip2 or xz)"
            )),
        }
    }

    /// File extension for the produced archive.
    pub fn ext(self) -> &'static str {
        match self {
            Compression::None => "tar",
            Compression::Gzip => "tar.gz",
            Compression::Bzip2 => "tar.bz2",
            Compression::Xz => "tar.xz",
        }
    }

    /// MIME type for the produced archive.
    pub fn mime(self) -> &'static str {
        match self {
            Compression::None => "application/x-tar",
            Compression::Gzip => "application/gzip",
            Compression::Bzip2 => "application/x-bzip2",
            Compression::Xz => "application/x-xz",
        }
    }
}

/// Make `name` unique against `seen` by inserting `(n)` before the extension.
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

/// Build an uncompressed tar archive from `(filename, bytes)` entries.
fn build_tar(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut seen = HashSet::new();
    for (name, bytes) in files {
        let entry = unique_name(name, &mut seen);
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        // Regular file, sensible default perms; deterministic mtime (0) so the
        // same inputs always produce byte-identical archives.
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();
        builder
            .append_data(&mut header, &entry, bytes.as_slice())
            .map_err(|e| format!("tar append {entry}: {e}"))?;
    }
    builder.into_inner().map_err(|e| format!("tar finish: {e}"))
}

fn gzip(tar: &[u8]) -> Result<Vec<u8>, String> {
    let mut enc = GzEncoder::new(Vec::new(), GzLevel::default());
    enc.write_all(tar).map_err(|e| format!("gzip write: {e}"))?;
    enc.finish().map_err(|e| format!("gzip finish: {e}"))
}

/// Bundle `(filename, bytes)` entries into a single tar archive, then compress
/// the archive with the chosen scheme. Errors if empty.
pub fn create_tar(files: &[(String, Vec<u8>)], compression: Compression) -> Result<Vec<u8>, String> {
    if files.is_empty() {
        return Err("need at least one file".into());
    }
    let tar = build_tar(files)?;
    match compression {
        Compression::None => Ok(tar),
        Compression::Gzip => gzip(&tar),
        // bzip2 block-size 9 / xz preset 6 = the standard `tar`/`bzip2`/`xz` defaults.
        Compression::Bzip2 => gizza_ai_bzip2_compress_core::bzip2(&tar, 9),
        Compression::Xz => gizza_ai_lzma_compress_core::xz_compress(&tar, 6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn entries(tar_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
        let mut out = Vec::new();
        for e in archive.entries().expect("entries") {
            let mut e = e.unwrap();
            let name = e.path().unwrap().to_string_lossy().into_owned();
            let mut data = Vec::new();
            e.read_to_end(&mut data).unwrap();
            out.push((name, data));
        }
        out
    }

    fn gunzip(gz: &[u8]) -> Vec<u8> {
        let mut dec = flate2::read::GzDecoder::new(gz);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn parse_aliases() {
        assert_eq!(Compression::parse("none").unwrap(), Compression::None);
        assert_eq!(Compression::parse("TAR").unwrap(), Compression::None);
        assert_eq!(Compression::parse("gz").unwrap(), Compression::Gzip);
        assert_eq!(Compression::parse("BZ2").unwrap(), Compression::Bzip2);
        assert_eq!(Compression::parse("lzma").unwrap(), Compression::Xz);
        assert!(Compression::parse("rar").is_err());
    }

    #[test]
    fn bundles_files_roundtrip() {
        let files = vec![
            ("a.txt".to_string(), b"hello".to_vec()),
            ("b.bin".to_string(), vec![0u8, 1, 2, 3]),
        ];
        let tar = create_tar(&files, Compression::None).unwrap();
        let got = entries(&tar);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], ("a.txt".to_string(), b"hello".to_vec()));
        assert_eq!(got[1].1, vec![0u8, 1, 2, 3]);
    }

    #[test]
    fn gzip_roundtrip() {
        let files = vec![("doc.txt".to_string(), b"compress me".to_vec())];
        let gz = create_tar(&files, Compression::Gzip).unwrap();
        assert_eq!(&gz[..2], &[0x1f, 0x8b]); // gzip magic
        let got = entries(&gunzip(&gz));
        assert_eq!(got, vec![("doc.txt".to_string(), b"compress me".to_vec())]);
    }

    #[test]
    fn bzip2_roundtrip() {
        let files = vec![("doc.txt".to_string(), b"bzip me ".repeat(50).to_vec())];
        let bz = create_tar(&files, Compression::Bzip2).unwrap();
        assert_eq!(&bz[0..3], b"BZh"); // bzip2 magic
        // round-trip via the same decoder the bzip2-compress tests use
        let mut dec = bzip2_rs::DecoderReader::new(&bz[..]);
        let mut tar = Vec::new();
        dec.read_to_end(&mut tar).unwrap();
        let got = entries(&tar);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "doc.txt");
    }

    #[test]
    fn xz_roundtrip() {
        let files = vec![("doc.txt".to_string(), b"xz me ".repeat(50).to_vec())];
        let xz = create_tar(&files, Compression::Xz).unwrap();
        // .xz stream header magic
        assert_eq!(&xz[0..6], &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]);
        let mut dec = lzma_rust2::XzReader::new(std::io::Cursor::new(&xz), false);
        let mut tar = Vec::new();
        dec.read_to_end(&mut tar).unwrap();
        let got = entries(&tar);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, "doc.txt");
    }

    #[test]
    fn deterministic_output() {
        let files = vec![("x".to_string(), b"y".to_vec())];
        assert_eq!(
            create_tar(&files, Compression::None).unwrap(),
            create_tar(&files, Compression::None).unwrap()
        );
    }

    #[test]
    fn duplicate_names_made_unique() {
        let files = vec![
            ("dup.txt".to_string(), b"1".to_vec()),
            ("dup.txt".to_string(), b"2".to_vec()),
            ("dup.txt".to_string(), b"3".to_vec()),
        ];
        let got = entries(&create_tar(&files, Compression::None).unwrap());
        let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["dup.txt", "dup (2).txt", "dup (3).txt"]);
    }

    #[test]
    fn empty_errors() {
        assert!(create_tar(&[], Compression::None).is_err());
    }

    #[test]
    fn blank_name_falls_back() {
        let got = entries(&create_tar(&[("".to_string(), b"x".to_vec())], Compression::None).unwrap());
        assert_eq!(got[0].0, "file");
    }
}
