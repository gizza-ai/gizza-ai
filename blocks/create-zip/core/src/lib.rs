//! gizza-ai/create-zip core — pure-Rust ZIP bundling shared by the chat skill
//! block + CLI. No wafer/wasm-bindgen deps. Takes named file byte buffers and
//! writes a single deflate-compressed ZIP archive. Duplicate names are made
//! unique (`name (2).ext`).

use std::io::{Cursor, Write};

use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Make `name` unique against `seen` by inserting `(n)` before the extension.
fn unique_name(name: &str, seen: &mut std::collections::HashSet<String>) -> String {
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

/// Bundle `(filename, bytes)` entries into a deflate ZIP. Errors if empty.
pub fn create_zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    if files.is_empty() {
        return Err("need at least one file".into());
    }
    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let mut seen = std::collections::HashSet::new();
        for (name, bytes) in files {
            let entry = unique_name(name, &mut seen);
            zw.start_file(entry, opts).map_err(|e| format!("zip start_file: {e}"))?;
            zw.write_all(bytes).map_err(|e| format!("zip write: {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn entries(zip_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).expect("valid zip");
        let mut out = Vec::new();
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).unwrap();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            out.push((f.name().to_string(), data));
        }
        out
    }

    #[test]
    fn bundles_files_roundtrip() {
        let files = vec![
            ("a.txt".to_string(), b"hello".to_vec()),
            ("b.bin".to_string(), vec![0u8, 1, 2, 3]),
        ];
        let zip = create_zip(&files).unwrap();
        assert_eq!(&zip[..2], b"PK"); // zip magic
        let got = entries(&zip);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], ("a.txt".to_string(), b"hello".to_vec()));
        assert_eq!(got[1].1, vec![0u8, 1, 2, 3]);
    }

    #[test]
    fn duplicate_names_made_unique() {
        let files = vec![
            ("dup.txt".to_string(), b"1".to_vec()),
            ("dup.txt".to_string(), b"2".to_vec()),
            ("dup.txt".to_string(), b"3".to_vec()),
        ];
        let got = entries(&create_zip(&files).unwrap());
        let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["dup.txt", "dup (2).txt", "dup (3).txt"]);
    }

    #[test]
    fn empty_errors() {
        assert!(create_zip(&[]).is_err());
    }

    #[test]
    fn blank_name_falls_back() {
        let got = entries(&create_zip(&[("".to_string(), b"x".to_vec())]).unwrap());
        assert_eq!(got[0].0, "file");
    }
}
