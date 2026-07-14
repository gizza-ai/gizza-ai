//! dir-checksum-report core — turn a batch of files into one checksum
//! manifest: a Markdown table (or CSV) of filename, size, and the requested
//! digests (CRC-32 and/or MD5/SHA-1/SHA-256/SHA-512), plus a "Duplicate files"
//! section (Markdown only) grouping any files whose digests are identical
//! across every requested algorithm — the same "manifest + duplicate
//! detection" shape real folder-checksum tools (FolderManifest, HashMyFiles)
//! ship. Pure compute, no wafer/wasm-bindgen deps — shared by the chat skill
//! block (this tool has no page: a multi-file report needs more than one
//! upload slot, like blocks/csv-merge and blocks/merge-pdf).
//!
//! All hashers are pure-Rust (RustCrypto), so the tool runs on every backend.
//! CRC-32 is hand-rolled (no dep), matching gizza-ai-file-hash-core.

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

/// One digest algorithm a file can be hashed with, in the order a user may
/// request them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    Crc32,
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    /// Canonical lowercase identifier — matches what `parse_algorithms` and the
    /// descriptor's `algorithms` param accept.
    pub fn id(self) -> &'static str {
        match self {
            Algorithm::Crc32 => "crc32",
            Algorithm::Md5 => "md5",
            Algorithm::Sha1 => "sha1",
            Algorithm::Sha256 => "sha256",
            Algorithm::Sha512 => "sha512",
        }
    }

    /// Friendly column label for the report.
    pub fn label(self) -> &'static str {
        match self {
            Algorithm::Crc32 => "CRC32",
            Algorithm::Md5 => "MD5",
            Algorithm::Sha1 => "SHA-1",
            Algorithm::Sha256 => "SHA-256",
            Algorithm::Sha512 => "SHA-512",
        }
    }

    fn parse_one(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "crc32" | "crc-32" | "crc" => Ok(Algorithm::Crc32),
            "md5" => Ok(Algorithm::Md5),
            "sha1" | "sha-1" => Ok(Algorithm::Sha1),
            "sha256" | "sha-256" => Ok(Algorithm::Sha256),
            "sha512" | "sha-512" => Ok(Algorithm::Sha512),
            other => Err(format!(
                "invalid algorithm '{other}': expected one of crc32, md5, sha1, sha256, sha512"
            )),
        }
    }

    fn digest(self, data: &[u8]) -> String {
        match self {
            Algorithm::Crc32 => format!("{:08x}", crc32(data)),
            Algorithm::Md5 => hex(&Md5::digest(data)),
            Algorithm::Sha1 => hex(&Sha1::digest(data)),
            Algorithm::Sha256 => hex(&Sha256::digest(data)),
            Algorithm::Sha512 => hex(&Sha512::digest(data)),
        }
    }
}

/// Parse a comma-separated algorithm list (e.g. `"crc32,sha256"`), trimming
/// whitespace and de-duplicating while preserving first-seen order. Empty or
/// all-blank input is an error (there is always at least one algorithm to
/// report).
pub fn parse_algorithms(s: &str) -> Result<Vec<Algorithm>, String> {
    let mut out: Vec<Algorithm> = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let algo = Algorithm::parse_one(part)?;
        if !out.contains(&algo) {
            out.push(algo);
        }
    }
    if out.is_empty() {
        return Err("algorithms must list at least one of crc32, md5, sha1, sha256, sha512".into());
    }
    Ok(out)
}

/// Report output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A Markdown table + an optional "Duplicate files" section.
    Markdown,
    /// Header row + one comma-separated row per file (RFC4180-ish quoting);
    /// no duplicate section — CSV stays purely tabular for machine parsing.
    Csv,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "markdown" | "md" => Ok(Format::Markdown),
            "csv" => Ok(Format::Csv),
            other => Err(format!("invalid format '{other}': expected 'markdown' or 'csv'")),
        }
    }
}

/// Row ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// Case-insensitive filename order (ties broken by the original name).
    Name,
    /// Ascending byte size (ties broken by filename).
    Size,
}

impl SortBy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "name" => Ok(SortBy::Name),
            "size" => Ok(SortBy::Size),
            other => Err(format!("invalid sort_by '{other}': expected 'name' or 'size'")),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// CRC-32 (IEEE 802.3, reflected) — the variant used by zip/gzip/PNG, and by
/// gizza-ai-file-hash-core.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// One computed row: the file's name, byte size, and its digest per requested
/// algorithm (same order as `algorithms`).
struct Row {
    name: String,
    size: u64,
    digests: Vec<String>,
}

fn escape_markdown_cell(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|")
}

fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Build the checksum report for a batch of `(filename, bytes)` files.
/// Requires at least 2 files (a single file's hash is `hash-all`/`file-hash`'s
/// job, not a batch report).
pub fn build_report(
    files: &[(String, Vec<u8>)],
    algorithms: &[Algorithm],
    format: Format,
    sort_by: SortBy,
) -> Result<String, String> {
    if files.len() < 2 {
        return Err(format!(
            "dir-checksum-report needs at least 2 files, got {}",
            files.len()
        ));
    }
    if algorithms.is_empty() {
        return Err("algorithms must list at least one of crc32, md5, sha1, sha256, sha512".into());
    }

    let mut rows: Vec<Row> = files
        .iter()
        .map(|(name, bytes)| Row {
            name: name.clone(),
            size: bytes.len() as u64,
            digests: algorithms.iter().map(|a| a.digest(bytes)).collect(),
        })
        .collect();

    match sort_by {
        SortBy::Name => rows.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        }),
        SortBy::Size => rows.sort_by(|a, b| a.size.cmp(&b.size).then_with(|| a.name.cmp(&b.name))),
    }

    match format {
        Format::Markdown => Ok(render_markdown(&rows, algorithms)),
        Format::Csv => Ok(render_csv(&rows, algorithms)),
    }
}

fn render_markdown(rows: &[Row], algorithms: &[Algorithm]) -> String {
    let mut out = String::new();
    out.push_str("| File | Size (bytes) |");
    for a in algorithms {
        out.push_str(&format!(" {} |", a.label()));
    }
    out.push('\n');
    out.push_str("| --- | ---: |");
    for _ in algorithms {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in rows {
        out.push_str(&format!("| `{}` | {} |", escape_markdown_cell(&row.name), row.size));
        for d in &row.digests {
            out.push_str(&format!(" `{d}` |"));
        }
        out.push('\n');
    }

    let dup_section = render_duplicates_markdown(rows, algorithms);
    if !dup_section.is_empty() {
        out.push('\n');
        out.push_str(&dup_section);
    }
    out
}

/// Group files whose digests match across EVERY requested algorithm (and
/// therefore, for practical purposes, share identical content) into a
/// "Duplicate files" section. Returns "" when no group has 2+ members.
fn render_duplicates_markdown(rows: &[Row], algorithms: &[Algorithm]) -> String {
    let mut groups: Vec<(&Vec<String>, Vec<&str>)> = Vec::new();
    for row in rows {
        if let Some(g) = groups.iter_mut().find(|(digests, _)| *digests == &row.digests) {
            g.1.push(&row.name);
        } else {
            groups.push((&row.digests, vec![&row.name]));
        }
    }
    let dup_groups: Vec<&(&Vec<String>, Vec<&str>)> =
        groups.iter().filter(|(_, names)| names.len() > 1).collect();
    if dup_groups.is_empty() {
        return String::new();
    }

    let algo_labels: Vec<&str> = algorithms.iter().map(|a| a.label()).collect();
    let mut out = String::new();
    out.push_str("## Duplicate files\n\n");
    out.push_str(&format!(
        "Files whose {} all match (identical content):\n\n",
        algo_labels.join(", ")
    ));
    for (_, names) in dup_groups {
        let quoted: Vec<String> = names.iter().map(|n| format!("`{}`", escape_markdown_cell(n))).collect();
        out.push_str(&format!("- {}\n", quoted.join(", ")));
    }
    out
}

fn render_csv(rows: &[Row], algorithms: &[Algorithm]) -> String {
    let mut out = String::new();
    out.push_str("file,size_bytes");
    for a in algorithms {
        out.push(',');
        out.push_str(a.id());
    }
    out.push('\n');
    for row in rows {
        out.push_str(&escape_csv_field(&row.name));
        out.push(',');
        out.push_str(&row.size.to_string());
        for d in &row.digests {
            out.push(',');
            out.push_str(d);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Vec<(String, Vec<u8>)> {
        vec![
            ("b.txt".to_string(), b"abc".to_vec()),
            ("a.txt".to_string(), b"".to_vec()),
        ]
    }

    #[test]
    fn happy_path_markdown_default_algorithms_known_vectors() {
        let algos = parse_algorithms("crc32,sha256").unwrap();
        let report = build_report(&files(), &algos, Format::Markdown, SortBy::Name).unwrap();
        // Sorted by name: a.txt (empty) then b.txt ("abc").
        assert_eq!(
            report,
            "| File | Size (bytes) | CRC32 | SHA-256 |\n\
             | --- | ---: | --- | --- |\n\
             | `a.txt` | 0 | `00000000` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |\n\
             | `b.txt` | 3 | `352441c2` | `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` |\n"
        );
    }

    #[test]
    fn error_fewer_than_two_files() {
        let algos = parse_algorithms("crc32").unwrap();
        let one = vec![("only.txt".to_string(), b"x".to_vec())];
        let err = build_report(&one, &algos, Format::Markdown, SortBy::Name).unwrap_err();
        assert!(err.contains("at least 2 files"), "unexpected error: {err}");
    }

    #[test]
    fn error_unknown_algorithm() {
        let err = parse_algorithms("crc32,rot13").unwrap_err();
        assert!(err.contains("invalid algorithm 'rot13'"), "unexpected error: {err}");
    }

    #[test]
    fn error_empty_algorithms() {
        let err = parse_algorithms(" , ,").unwrap_err();
        assert!(err.contains("at least one"), "unexpected error: {err}");
    }

    #[test]
    fn dedupes_repeated_algorithm() {
        let algos = parse_algorithms("sha256,SHA-256,sha256").unwrap();
        assert_eq!(algos, vec![Algorithm::Sha256]);
    }

    #[test]
    fn sort_by_size_ascending_then_name() {
        let algos = parse_algorithms("crc32").unwrap();
        let files = vec![
            ("big.bin".to_string(), vec![0u8; 10]),
            ("small.bin".to_string(), vec![0u8; 2]),
        ];
        let report = build_report(&files, &algos, Format::Markdown, SortBy::Size).unwrap();
        let small_pos = report.find("small.bin").unwrap();
        let big_pos = report.find("big.bin").unwrap();
        assert!(small_pos < big_pos, "expected small.bin row before big.bin row");
    }

    #[test]
    fn duplicate_files_are_flagged() {
        let algos = parse_algorithms("crc32,sha256").unwrap();
        let files = vec![
            ("copy1.txt".to_string(), b"same content".to_vec()),
            ("copy2.txt".to_string(), b"same content".to_vec()),
            ("unique.txt".to_string(), b"different".to_vec()),
        ];
        let report = build_report(&files, &algos, Format::Markdown, SortBy::Name).unwrap();
        assert!(report.contains("## Duplicate files"), "report:\n{report}");
        assert!(report.contains("`copy1.txt`, `copy2.txt`"), "report:\n{report}");
        assert!(!report.contains("unique.txt`, `"), "unique.txt must not be grouped");
    }

    #[test]
    fn no_duplicate_section_when_all_files_differ() {
        let algos = parse_algorithms("crc32").unwrap();
        let files = vec![
            ("a.txt".to_string(), b"one".to_vec()),
            ("b.txt".to_string(), b"two".to_vec()),
        ];
        let report = build_report(&files, &algos, Format::Markdown, SortBy::Name).unwrap();
        assert!(!report.contains("Duplicate files"));
    }

    #[test]
    fn csv_format_has_header_and_rows_no_duplicate_section() {
        let algos = parse_algorithms("crc32,sha256").unwrap();
        let report = build_report(&files(), &algos, Format::Csv, SortBy::Name).unwrap();
        let mut lines = report.lines();
        assert_eq!(lines.next().unwrap(), "file,size_bytes,crc32,sha256");
        assert_eq!(
            lines.next().unwrap(),
            "a.txt,0,00000000,e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            lines.next().unwrap(),
            "b.txt,3,352441c2,ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(!report.contains("Duplicate"));
    }

    #[test]
    fn csv_escapes_commas_and_quotes_in_filenames() {
        let algos = parse_algorithms("crc32").unwrap();
        let files = vec![
            ("plain.txt".to_string(), b"x".to_vec()),
            ("name, with \"quotes\".txt".to_string(), b"y".to_vec()),
        ];
        let report = build_report(&files, &algos, Format::Csv, SortBy::Name).unwrap();
        assert!(report.contains("\"name, with \"\"quotes\"\".txt\","), "report:\n{report}");
    }

    #[test]
    fn format_parse_accepts_aliases_and_rejects_unknown() {
        assert_eq!(Format::parse("").unwrap(), Format::Markdown);
        assert_eq!(Format::parse("MD").unwrap(), Format::Markdown);
        assert_eq!(Format::parse("CSV").unwrap(), Format::Csv);
        assert!(Format::parse("xml").is_err());
    }

    #[test]
    fn sort_by_parse_accepts_aliases_and_rejects_unknown() {
        assert_eq!(SortBy::parse("").unwrap(), SortBy::Name);
        assert_eq!(SortBy::parse("SIZE").unwrap(), SortBy::Size);
        assert!(SortBy::parse("date").is_err());
    }
}
