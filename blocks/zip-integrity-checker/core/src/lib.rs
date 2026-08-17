//! gizza-ai/zip-integrity-checker core — test a .zip archive for corruption by
//! DECOMPRESSING every entry and recomputing its CRC-32. Pure-Rust (`zip` with
//! deflate only + `crc32fast`). No wafer/wasm-bindgen deps.
//!
//! This is the opposite of `blocks/zip-inspect`, which only reads the central
//! directory and reports the CRC-32 *as recorded*. Here every entry's data is
//! expanded in memory (never written to disk), the CRC-32 of the expanded bytes
//! is recomputed, and both checksums are reported side by side — the only way a
//! silently corrupted entry is detected.
//!
//! Nothing is repaired: this detects damage, it does not fix it.

use serde::Serialize;
use std::io::{Cursor, Read};

/// Verification outcome for one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Decompressed cleanly and both the size and the CRC-32 matched.
    Ok,
    /// The data expanded, but its CRC-32 differs from the recorded one.
    CrcMismatch,
    /// The data expanded, but to a different number of bytes than recorded.
    SizeMismatch,
    /// The data could not be expanded (truncated, corrupt stream, bad header).
    DataError,
    /// The entry uses a compression method this build cannot decode.
    UnsupportedMethod,
    /// The entry's data is encrypted; no password support, so it is not tested.
    Encrypted,
}

impl Status {
    /// A real failure — the archive is damaged. `ok` is false when any entry fails.
    pub fn is_failure(self) -> bool {
        matches!(
            self,
            Status::CrcMismatch | Status::SizeMismatch | Status::DataError
        )
    }
    /// Could not be tested (rather than tested and failed).
    pub fn is_skipped(self) -> bool {
        matches!(self, Status::UnsupportedMethod | Status::Encrypted)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::CrcMismatch => "crc_mismatch",
            Status::SizeMismatch => "size_mismatch",
            Status::DataError => "data_error",
            Status::UnsupportedMethod => "unsupported_method",
            Status::Encrypted => "encrypted",
        }
    }
}

/// Which entries to include in the report's `entries` list. Counts and totals
/// always cover the whole archive either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLevel {
    /// Every entry.
    All,
    /// Only entries that failed or could not be tested.
    Problems,
}

impl ReportLevel {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "all" => Ok(ReportLevel::All),
            "problems" => Ok(ReportLevel::Problems),
            other => Err(format!(
                "unknown report level '{other}' — use 'all' (every entry) or 'problems' (only entries that failed or could not be tested)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    /// Full path of the entry inside the archive.
    pub name: String,
    /// Verification outcome.
    pub status: Status,
    /// Human-readable explanation. Omitted for entries that verified cleanly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Compression method label, e.g. "Stored", "Deflated".
    pub method: String,
    /// Uncompressed size in bytes as recorded in the central directory.
    pub size: u64,
    /// Bytes actually produced by decompressing. Omitted when the entry was not tested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_size: Option<u64>,
    /// Compressed (stored) size in bytes.
    pub compressed_size: u64,
    /// CRC-32 recorded in the archive, as 8-char lowercase hex.
    pub expected_crc32: String,
    /// CRC-32 recomputed from the decompressed bytes. Omitted when not tested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_crc32: Option<String>,
    /// True for directory entries (nothing to verify).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// True when no entry failed. Entries that could not be tested (encrypted /
    /// unsupported method) do NOT make this false — they are counted separately.
    pub ok: bool,
    /// One-line verdict, e.g. "no errors detected in 3 entries".
    pub summary: String,
    /// The entries included per the requested report level.
    pub entries: Vec<Entry>,
    /// Number of entries in `entries` (may be fewer than `count` at report=problems).
    pub reported_count: usize,
    /// Total number of entries in the archive (files + directories).
    pub count: usize,
    pub file_count: usize,
    pub dir_count: usize,
    /// Number of file entries whose data was actually decompressed and checked.
    pub checked_count: usize,
    /// Number of entries that failed (crc_mismatch / size_mismatch / data_error).
    pub failed_count: usize,
    /// Number of entries that could not be tested (encrypted / unsupported_method).
    pub skipped_count: usize,
    /// Total uncompressed size of all file entries as recorded, in bytes.
    pub total_size: u64,
    /// Total compressed size of all file entries, in bytes.
    pub total_compressed_size: u64,
    /// Bytes actually decompressed and hashed during this check.
    pub bytes_verified: u64,
    /// Name of the first entry that failed, if any (testzip semantics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_bad_entry: Option<String>,
    /// Bytes of junk/stub before the ZIP starts (non-zero for self-extracting archives).
    pub prepended_bytes: u64,
    /// The archive comment, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// Default decompression budget in MB.
pub const DEFAULT_MAX_UNCOMPRESSED_MB: u64 = 512;
pub const MIN_MAX_UNCOMPRESSED_MB: u64 = 1;
pub const MAX_MAX_UNCOMPRESSED_MB: u64 = 4096;

const CHUNK: usize = 64 * 1024;

fn budget_error(mb: u64, needed: u64) -> String {
    format!(
        "this archive expands to at least {needed} bytes, over the {mb} MB max_uncompressed_mb \
         budget — raise max_uncompressed_mb (1-{MAX_MAX_UNCOMPRESSED_MB}) if the archive really is \
         that big, or treat it as a zip bomb"
    )
}

fn supported(method: zip::CompressionMethod) -> bool {
    matches!(
        method,
        zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
    )
}

/// Verify `zip_bytes`: read the central directory, then decompress every entry
/// and compare the recomputed CRC-32 and byte count with what is recorded.
///
/// `max_uncompressed_mb` bounds the total number of bytes expanded (zip-bomb
/// guard); exceeding it is an error, not a verdict. Returns `Err` only when the
/// archive cannot be opened at all or the budget is blown — a damaged *entry* is
/// reported inside the [`Report`], not as an error.
pub fn verify(
    zip_bytes: &[u8],
    level: ReportLevel,
    max_uncompressed_mb: u64,
) -> Result<Report, String> {
    let mb = max_uncompressed_mb.clamp(MIN_MAX_UNCOMPRESSED_MB, MAX_MAX_UNCOMPRESSED_MB);
    let budget = mb.saturating_mul(1024 * 1024);

    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("not a valid zip archive: {e}"))?;

    let prepended_bytes = archive.offset();
    let comment = String::from_utf8_lossy(archive.comment())
        .trim()
        .to_string();

    // Fail fast on a declared expansion that already blows the budget, before
    // spending any work on it.
    let declared: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index_raw(i).ok().map(|f| f.size()))
        .fold(0u64, |acc, s| acc.saturating_add(s));
    if declared > budget {
        return Err(budget_error(mb, declared));
    }

    let mut entries: Vec<Entry> = Vec::with_capacity(archive.len());
    let (mut file_count, mut dir_count, mut checked, mut failed, mut skipped) = (0, 0, 0, 0, 0);
    let (mut total_size, mut total_compressed, mut bytes_verified) = (0u64, 0u64, 0u64);
    let mut first_bad: Option<String> = None;

    for i in 0..archive.len() {
        // Metadata first, without decompressing: a raw handle never fails on an
        // encrypted or unknown-method entry, so those are labelled, not lost.
        let (name, is_dir, size, compressed_size, method, expected_crc32, encrypted) = {
            let f = archive
                .by_index_raw(i)
                .map_err(|e| format!("failed to read zip entry {i}: {e}"))?;
            (
                f.name().to_string(),
                f.is_dir(),
                f.size(),
                f.compressed_size(),
                f.compression(),
                f.crc32(),
                f.encrypted(),
            )
        };

        if is_dir {
            dir_count += 1;
        } else {
            file_count += 1;
            total_size = total_size.saturating_add(size);
            total_compressed = total_compressed.saturating_add(compressed_size);
        }

        let mut entry = Entry {
            name: name.clone(),
            status: Status::Ok,
            detail: None,
            method: method.to_string(),
            size,
            actual_size: None,
            compressed_size,
            expected_crc32: format!("{expected_crc32:08x}"),
            actual_crc32: None,
            is_dir,
        };

        if is_dir {
            entry.detail = Some("directory entry — no data to verify".into());
        } else if encrypted {
            entry.status = Status::Encrypted;
            entry.detail = Some(
                "entry data is encrypted — this tool has no password support, so the entry was \
                 not tested (it is not counted as a pass)"
                    .into(),
            );
        } else if !supported(method) {
            entry.status = Status::UnsupportedMethod;
            entry.detail = Some(format!(
                "compression method {method} cannot be decoded here (only Stored and Deflated \
                 are supported), so the entry was not tested"
            ));
        } else {
            // Decompress with the crate's own CRC check DISABLED so we can read
            // the whole stream and report the actual checksum, not just "bad".
            let read = read_entry(&mut archive, i, budget, bytes_verified);
            match read {
                Err(ReadFail::Budget(needed)) => return Err(budget_error(mb, needed)),
                Err(ReadFail::Data(msg, got, crc)) => {
                    bytes_verified = bytes_verified.saturating_add(got);
                    entry.status = Status::DataError;
                    entry.actual_size = Some(got);
                    entry.actual_crc32 = Some(format!("{crc:08x}"));
                    entry.detail = Some(format!(
                        "could not decompress the entry ({msg}) — {got} of {size} bytes were \
                         recovered before the failure"
                    ));
                }
                Ok((got, crc)) => {
                    bytes_verified = bytes_verified.saturating_add(got);
                    checked += 1;
                    entry.actual_size = Some(got);
                    entry.actual_crc32 = Some(format!("{crc:08x}"));
                    if got != size {
                        entry.status = Status::SizeMismatch;
                        entry.detail = Some(format!(
                            "the archive records {size} uncompressed bytes but the entry expanded \
                             to {got}"
                        ));
                    } else if crc != expected_crc32 {
                        entry.status = Status::CrcMismatch;
                        entry.detail = Some(format!(
                            "CRC-32 mismatch: the archive records {expected_crc32:08x} but the \
                             data hashes to {crc:08x} — this entry is corrupted"
                        ));
                    }
                }
            }
        }

        if entry.status.is_failure() {
            failed += 1;
            if first_bad.is_none() {
                first_bad = Some(name.clone());
            }
        } else if entry.status.is_skipped() {
            skipped += 1;
        }

        if level == ReportLevel::All || entry.status != Status::Ok {
            entries.push(entry);
        }
    }

    let ok = failed == 0;
    Ok(Report {
        ok,
        summary: summarize(ok, archive.len(), checked, failed, skipped, &first_bad),
        reported_count: entries.len(),
        entries,
        count: archive.len(),
        file_count,
        dir_count,
        checked_count: checked,
        failed_count: failed,
        skipped_count: skipped,
        total_size,
        total_compressed_size: total_compressed,
        bytes_verified,
        first_bad_entry: first_bad,
        prepended_bytes,
        comment: (!comment.is_empty()).then_some(comment),
    })
}

enum ReadFail {
    /// The decompression budget would be exceeded; carries the bytes needed so far.
    Budget(u64),
    /// The stream broke: message, bytes recovered, CRC of what was recovered.
    Data(String, u64, u32),
}

/// Expand entry `i` in fixed-size chunks, hashing as we go, so a huge entry is
/// never materialised in one allocation and the budget is enforced mid-stream.
fn read_entry(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    i: usize,
    budget: u64,
    already: u64,
) -> Result<(u64, u32), ReadFail> {
    let mut file =
        match archive.by_index_with_options(i, zip::ZipReadOptions::new().ignore_crc32(true)) {
            Ok(f) => f,
            Err(e) => return Err(ReadFail::Data(e.to_string(), 0, 0)),
        };
    let mut hasher = crc32fast::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    let mut got = 0u64;
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                hasher.update(&buf[..n]);
                got = got.saturating_add(n as u64);
                if already.saturating_add(got) > budget {
                    return Err(ReadFail::Budget(already.saturating_add(got)));
                }
            }
            Err(e) => return Err(ReadFail::Data(e.to_string(), got, hasher.finalize())),
        }
    }
    Ok((got, hasher.finalize()))
}

fn summarize(
    ok: bool,
    count: usize,
    checked: usize,
    failed: usize,
    skipped: usize,
    first_bad: &Option<String>,
) -> String {
    let plural = |n: usize| if n == 1 { "entry" } else { "entries" };
    let tail = if skipped > 0 {
        format!("; {skipped} {} could not be tested", plural(skipped))
    } else {
        String::new()
    };
    if ok {
        format!(
            "no errors detected — {checked} of {count} {} verified{tail}",
            plural(count)
        )
    } else {
        let first = first_bad.as_deref().unwrap_or("");
        format!(
            "{failed} of {count} {} FAILED (first bad entry: {first}); {checked} verified{tail}",
            plural(count)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn deflated() -> SimpleFileOptions {
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
    }
    fn stored() -> SimpleFileOptions {
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
    }

    fn make_zip() -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            w.start_file("docs/readme.txt", deflated()).unwrap();
            w.write_all(&vec![b'a'; 2000]).unwrap();
            w.start_file("data.bin", stored()).unwrap();
            w.write_all(b"0123456789").unwrap();
            w.add_directory("docs", SimpleFileOptions::default())
                .unwrap();
            w.set_comment("built for tests").unwrap();
            w.finish().unwrap();
        }
        buf.into_inner()
    }

    /// Flip the byte at `at` — the cheapest way to make real corruption.
    fn corrupt(mut zip: Vec<u8>, at: usize) -> Vec<u8> {
        zip[at] ^= 0xff;
        zip
    }

    #[test]
    fn a_clean_archive_passes_with_both_crcs_reported() {
        let r = verify(&make_zip(), ReportLevel::All, DEFAULT_MAX_UNCOMPRESSED_MB).unwrap();
        assert!(r.ok, "{}", r.summary);
        assert_eq!(r.count, 3);
        assert_eq!(r.file_count, 2);
        assert_eq!(r.dir_count, 1);
        assert_eq!(r.checked_count, 2);
        assert_eq!(r.failed_count, 0);
        assert_eq!(r.skipped_count, 0);
        assert_eq!(r.bytes_verified, 2010);
        assert_eq!(r.total_size, 2010);
        assert_eq!(r.prepended_bytes, 0);
        assert_eq!(r.comment.as_deref(), Some("built for tests"));
        assert!(r.first_bad_entry.is_none());
        assert!(r.summary.starts_with("no errors detected"), "{}", r.summary);

        let readme = r
            .entries
            .iter()
            .find(|e| e.name == "docs/readme.txt")
            .unwrap();
        assert_eq!(readme.status, Status::Ok);
        assert_eq!(readme.method, "Deflated");
        assert_eq!(readme.actual_size, Some(2000));
        // The recorded and the recomputed checksum are both present and agree.
        assert_eq!(
            readme.actual_crc32.as_deref(),
            Some(readme.expected_crc32.as_str())
        );
        assert!(readme.compressed_size < readme.size);

        let dir = r.entries.iter().find(|e| e.is_dir).unwrap();
        assert_eq!(dir.status, Status::Ok);
        assert!(dir.actual_crc32.is_none());
    }

    #[test]
    fn a_flipped_stored_byte_is_reported_as_a_crc_mismatch() {
        let zip = make_zip();
        // "0123456789" is stored verbatim; find it and flip a byte of the data.
        let at = zip
            .windows(10)
            .position(|w| w == b"0123456789")
            .expect("stored payload present");
        let r = verify(&corrupt(zip, at + 3), ReportLevel::All, 512).unwrap();
        assert!(!r.ok);
        assert_eq!(r.failed_count, 1);
        assert_eq!(r.first_bad_entry.as_deref(), Some("data.bin"));
        let bad = r.entries.iter().find(|e| e.name == "data.bin").unwrap();
        assert_eq!(bad.status, Status::CrcMismatch);
        // Size still matches — only the checksum moved.
        assert_eq!(bad.actual_size, Some(10));
        assert_ne!(bad.actual_crc32, Some(bad.expected_crc32.clone()));
        assert!(bad.detail.as_deref().unwrap().contains("CRC-32 mismatch"));
        assert!(r.summary.contains("FAILED"), "{}", r.summary);
    }

    #[test]
    fn a_broken_deflate_stream_is_reported_as_a_data_error() {
        let zip = make_zip();
        // The deflated entry's data starts right after its local header; corrupt
        // several bytes inside the compressed stream so inflate itself breaks.
        let start = zip.windows(4).position(|w| w == b"PK\x03\x04").unwrap();
        let name_len = u16::from_le_bytes([zip[start + 26], zip[start + 27]]) as usize;
        let extra_len = u16::from_le_bytes([zip[start + 28], zip[start + 29]]) as usize;
        let data_start = start + 30 + name_len + extra_len;
        let mut broken = zip.clone();
        for off in 0..10 {
            broken[data_start + off] ^= 0xa5;
        }
        let r = verify(&broken, ReportLevel::All, 512).unwrap();
        let bad = r
            .entries
            .iter()
            .find(|e| e.name == "docs/readme.txt")
            .unwrap();
        assert!(
            matches!(
                bad.status,
                Status::DataError | Status::CrcMismatch | Status::SizeMismatch
            ),
            "{bad:?}"
        );
        assert!(!r.ok);
        assert_eq!(r.first_bad_entry.as_deref(), Some("docs/readme.txt"));
    }

    #[test]
    fn truncating_the_data_is_caught() {
        // Rebuild a zip whose central directory survives but whose entry data was
        // clipped: write a big stored entry, then shrink the stored bytes.
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            w.start_file("big.bin", stored()).unwrap();
            w.write_all(&vec![b'z'; 5000]).unwrap();
            w.finish().unwrap();
        }
        let zip = buf.into_inner();
        // Drop the last 200 bytes of the file: the end-of-central-directory goes
        // with them, so the archive itself no longer opens — the structural case.
        let err = verify(&zip[..zip.len() - 200], ReportLevel::All, 512).unwrap_err();
        assert!(err.contains("not a valid zip archive"), "{err}");
    }

    #[test]
    fn report_problems_hides_healthy_entries_but_keeps_the_counts() {
        let zip = make_zip();
        let at = zip.windows(10).position(|w| w == b"0123456789").unwrap();
        let r = verify(&corrupt(zip, at + 3), ReportLevel::Problems, 512).unwrap();
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.reported_count, 1);
        assert_eq!(r.entries[0].name, "data.bin");
        // Counts still describe the whole archive.
        assert_eq!(r.count, 3);
        assert_eq!(r.checked_count, 2);
    }

    #[test]
    fn a_clean_archive_at_report_problems_lists_nothing() {
        let r = verify(&make_zip(), ReportLevel::Problems, 512).unwrap();
        assert!(r.ok);
        assert!(r.entries.is_empty());
        assert_eq!(r.reported_count, 0);
        assert_eq!(r.count, 3);
    }

    #[test]
    fn prepended_stub_bytes_are_reported() {
        let mut sfx = b"#!/bin/sh\nself-extracting stub\n".to_vec();
        let offset = sfx.len() as u64;
        sfx.extend_from_slice(&make_zip());
        let r = verify(&sfx, ReportLevel::All, 512).unwrap();
        assert!(r.ok, "{}", r.summary);
        assert_eq!(r.prepended_bytes, offset);
    }

    #[test]
    fn the_uncompressed_budget_is_enforced() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            w.start_file("bomb.bin", deflated()).unwrap();
            // 8 MiB of zeros compresses to a few KiB — the classic ratio bomb.
            w.write_all(&vec![0u8; 8 * 1024 * 1024]).unwrap();
            w.finish().unwrap();
        }
        let zip = buf.into_inner();
        let err = verify(&zip, ReportLevel::All, 1).unwrap_err();
        assert!(err.contains("max_uncompressed_mb"), "{err}");
        // The same archive passes when the budget covers it.
        assert!(verify(&zip, ReportLevel::All, 64).unwrap().ok);
    }

    #[test]
    fn a_non_zip_input_errors() {
        assert!(verify(b"definitely not a zip", ReportLevel::All, 512).is_err());
        assert!(verify(&[], ReportLevel::All, 512).is_err());
    }

    #[test]
    fn report_level_parsing() {
        assert_eq!(ReportLevel::parse("all").unwrap(), ReportLevel::All);
        assert_eq!(
            ReportLevel::parse(" Problems ").unwrap(),
            ReportLevel::Problems
        );
        let err = ReportLevel::parse("quiet").unwrap_err();
        assert!(err.contains("unknown report level"), "{err}");
    }

    #[test]
    fn statuses_classify_failures_and_skips() {
        assert!(Status::CrcMismatch.is_failure());
        assert!(Status::SizeMismatch.is_failure());
        assert!(Status::DataError.is_failure());
        assert!(!Status::Encrypted.is_failure());
        assert!(Status::Encrypted.is_skipped());
        assert!(Status::UnsupportedMethod.is_skipped());
        assert!(!Status::Ok.is_failure() && !Status::Ok.is_skipped());
        assert_eq!(Status::CrcMismatch.as_str(), "crc_mismatch");
    }
}
