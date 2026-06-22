//! gizza-ai/unzip core — extract the entries of a .zip archive, returning each
//! file individually. Pure-Rust (`zip`, deflate only). No wafer/wasm-bindgen deps.
//!
//! Each regular file is returned with its path and size; its content is included
//! inline as `text` when it decodes as UTF-8, otherwise as base64 `data`. Content
//! is included up to a total byte budget so the result stays bounded; beyond that,
//! entries are still listed (name + size) without content.

use std::io::{Cursor, Read};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Serialize;

/// Default cap on total inlined content bytes.
pub const DEFAULT_CONTENT_BUDGET: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub name: String,
    pub size: u64,
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
}

/// Extract `zip_bytes`. `content_budget` caps total inlined content bytes.
pub fn extract(zip_bytes: &[u8], content_budget: usize) -> Result<Extracted, String> {
    let reader = Cursor::new(zip_bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("not a valid zip archive: {e}"))?;

    let mut entries = Vec::new();
    let mut used = 0usize;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("failed to read zip entry {i}: {e}"))?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        let size = file.size();

        // Read content if it fits the remaining budget.
        let mut text = None;
        let mut data = None;
        let mut content_omitted = false;
        if (size as usize).saturating_add(used) <= content_budget {
            let mut buf = Vec::with_capacity(size as usize);
            file.read_to_end(&mut buf)
                .map_err(|e| format!("failed to extract '{name}': {e}"))?;
            used += buf.len();
            match String::from_utf8(buf) {
                Ok(s) => text = Some(s),
                Err(e) => data = Some(B64.encode(e.as_bytes())),
            }
        } else {
            content_omitted = true;
        }

        entries.push(Entry { name, size, text, data, content_omitted });
    }

    let count = entries.len();
    Ok(Extracted { entries, count })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn make_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, data) in files {
                w.start_file(*name, opts).unwrap();
                w.write_all(data).unwrap();
            }
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn extracts_text_and_binary() {
        let zip = make_zip(&[("hello.txt", b"hello world"), ("blob.bin", &[0u8, 159, 146, 150])]);
        let ex = extract(&zip, DEFAULT_CONTENT_BUDGET).unwrap();
        assert_eq!(ex.count, 2);
        let txt = ex.entries.iter().find(|e| e.name == "hello.txt").unwrap();
        assert_eq!(txt.text.as_deref(), Some("hello world"));
        assert!(txt.data.is_none());
        let bin = ex.entries.iter().find(|e| e.name == "blob.bin").unwrap();
        assert!(bin.text.is_none());
        assert_eq!(bin.data.as_deref(), Some(&*B64.encode([0u8, 159, 146, 150])));
    }

    #[test]
    fn budget_omits_large_content_but_lists_it() {
        let big = vec![b'a'; 1000];
        let zip = make_zip(&[("big.txt", &big)]);
        // Budget smaller than the file -> content omitted, still listed.
        let ex = extract(&zip, 100).unwrap();
        assert_eq!(ex.count, 1);
        let e = &ex.entries[0];
        assert!(e.content_omitted);
        assert!(e.text.is_none() && e.data.is_none());
        assert_eq!(e.size, 1000);
    }

    #[test]
    fn errors_on_non_zip() {
        assert!(extract(b"not a zip", DEFAULT_CONTENT_BUDGET).is_err());
    }
}
