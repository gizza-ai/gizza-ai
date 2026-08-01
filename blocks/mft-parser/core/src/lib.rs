//! mft-parser core — pure-Rust NTFS Master File Table (`$MFT`) reader.
//!
//! Wraps the `mft` crate (binary `$MFT` record parser) to turn a raw `$MFT`
//! buffer into LLM-friendly structured JSON for filesystem-timeline / DFIR work:
//! a flat list of entries, each with its MFT entry number, sequence number,
//! in-use vs deleted state, file-vs-directory flag, reconstructed full path,
//! logical size, and BOTH timestamp sets NTFS keeps per record —
//! `$STANDARD_INFORMATION` (SI) and `$FILE_NAME` (FN), each with created,
//! modified, MFT-modified, and accessed instants (the "MACB" times).
//!
//! On top of the parser it adds the filtering + triage an analyst wants: filter
//! by path/name substring, files-only or dirs-only, active-only or deleted-only,
//! or only records whose SI creation time predates the FN creation time — the
//! classic timestomping ("anti-forensics") tell. `summary=true` returns aggregate
//! counts + the file-system time span instead of the records.
//!
//! No wafer/wasm-bindgen deps — shared verbatim by the chat skill block. The
//! min/max span is computed with `jiff` (the same crate `mft` stamps each
//! record's FILETIME with) so ordering is exact instant comparison.

use jiff::Timestamp;
use mft::attribute::MftAttributeContent;
use mft::MftParser;
use serde::Serialize;

/// Which record kinds to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Include {
    #[default]
    All,
    Files,
    Dirs,
}

/// Which allocation state to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    All,
    Active,
    Deleted,
}

/// Parsed request options. Empty / default fields mean "no filter".
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Case-insensitive substring the entry's name OR full path must contain
    /// (None = no filter).
    pub path_contains: Option<String>,
    /// Keep only files, only directories, or all (default).
    pub include: Include,
    /// Keep only in-use, only deleted (unallocated), or all (default).
    pub status: Status,
    /// Keep only records flagged as timestomp suspects (SI created < FN created).
    pub only_timestomp: bool,
    /// Max records to return in file order (0 = all). Ignored in summary mode.
    pub max_records: usize,
    /// Return aggregate counts + time span instead of the records.
    pub summary: bool,
}

/// The four NTFS timestamps carried by an attribute (RFC-3339 strings).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Times {
    pub created: String,
    pub modified: String,
    pub mft_modified: String,
    pub accessed: String,
}

/// One MFT record in the flat output list.
#[derive(Debug, Clone, Serialize)]
pub struct EntryOut {
    /// MFT entry (record) number.
    pub entry: u64,
    /// Sequence number (incremented each time the record is freed/reused).
    pub sequence: u16,
    /// True if the record is allocated (in use); false = deleted/unallocated.
    pub in_use: bool,
    /// True if the record is a directory (has an index).
    pub is_directory: bool,
    /// Parent directory's MFT entry number (from the `$FILE_NAME` attribute).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_entry: Option<u64>,
    /// Primary (best) file name.
    pub name: String,
    /// Reconstructed full path (best-effort; falls back to `name` if the parent
    /// chain can't be resolved).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Logical file size in bytes (from the `$FILE_NAME` attribute).
    pub size: u64,
    /// True when SI creation time is earlier than FN creation time — a common
    /// timestomping indicator.
    pub timestomp_suspect: bool,
    /// `$STANDARD_INFORMATION` timestamps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standard_info: Option<Times>,
    /// `$FILE_NAME` timestamps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<Times>,
}

/// Aggregate view over the matched records (summary mode).
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub files: usize,
    pub directories: usize,
    pub active: usize,
    pub deleted: usize,
    pub timestomp_suspects: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
}

/// The full parse result.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// FILE records the parser decoded (before any filter).
    pub total_records: usize,
    /// Records passing the filters (before the `max_records` cap).
    pub matched_records: usize,
    /// Records actually included in `entries` (after the cap).
    pub returned_records: usize,
    /// Records the parser could not decode (bad signature / out-of-range
    /// timestamps), skipped.
    pub parse_errors: usize,
    /// True when `matched_records > returned_records` (hit the cap).
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<EntryOut>,
}

/// Extracted-but-owned view of one record (borrows released before path lookup).
struct Extracted {
    name: String,
    parent_entry: Option<u64>,
    size: u64,
    /// (rendered SI times, SI created instant)
    si: Option<(Times, Timestamp)>,
    /// (rendered FN times, FN created instant)
    fname: Option<(Times, Timestamp)>,
}

fn si_times(a: &mft::attribute::x10::StandardInfoAttr) -> (Times, Timestamp) {
    (
        Times {
            created: a.created.to_string(),
            modified: a.modified.to_string(),
            mft_modified: a.mft_modified.to_string(),
            accessed: a.accessed.to_string(),
        },
        a.created,
    )
}

fn fn_times(a: &mft::attribute::x30::FileNameAttr) -> (Times, Timestamp) {
    (
        Times {
            created: a.created.to_string(),
            modified: a.modified.to_string(),
            mft_modified: a.mft_modified.to_string(),
            accessed: a.accessed.to_string(),
        },
        a.created,
    )
}

/// Pull the owned fields we need out of an entry (SI + best FN), releasing all
/// borrows so the parser can be re-borrowed for path resolution.
fn extract(entry: &mft::MftEntry) -> Extracted {
    let mut si: Option<(Times, Timestamp)> = None;
    for attr in entry.iter_attributes().flatten() {
        if let MftAttributeContent::AttrX10(ref s) = attr.data {
            si = Some(si_times(s));
            break;
        }
    }

    let best = entry.find_best_name_attribute();
    let (name, parent_entry, size, fname) = match best {
        Some(ref f) => (
            f.name.clone(),
            Some(f.parent.entry),
            f.logical_size,
            Some(fn_times(f)),
        ),
        None => (String::new(), None, 0, None),
    };

    Extracted {
        name,
        parent_entry,
        size,
        si,
        fname,
    }
}

/// Parse an `$MFT` buffer and apply the options. Errors only when the bytes are
/// not a usable `$MFT` (no decodable FILE records at all); individual corrupt
/// records are counted in `parse_errors` and skipped.
pub fn parse(bytes: &[u8], opts: &Options) -> Result<Report, String> {
    let mut parser = MftParser::from_buffer(bytes.to_vec())
        .map_err(|e| format!("not a valid $MFT buffer: {e}"))?;

    let want = opts.path_contains.as_ref().map(|s| s.to_lowercase());
    let count = parser.get_entry_count();

    let mut total = 0usize;
    let mut matched = 0usize;
    let mut parse_errors = 0usize;
    let mut entries: Vec<EntryOut> = Vec::new();

    // summary aggregates
    let mut files = 0usize;
    let mut directories = 0usize;
    let mut active = 0usize;
    let mut deleted = 0usize;
    let mut timestomp_suspects = 0usize;
    let mut earliest: Option<Timestamp> = None;
    let mut latest: Option<Timestamp> = None;

    for i in 0..count {
        let entry = match parser.get_entry(i) {
            Ok(e) => e,
            Err(_) => {
                parse_errors += 1;
                continue;
            }
        };
        // Skip empty/unused slots (no "FILE" signature) — not an error.
        if entry.header.signature != *b"FILE" {
            continue;
        }
        total += 1;

        let in_use = entry.is_allocated();
        let is_directory = entry.is_dir();
        let ex = extract(&entry);

        let timestomp_suspect = match (&ex.si, &ex.fname) {
            (Some((_, si_c)), Some((_, fn_c))) => si_c < fn_c,
            _ => false,
        };

        // --- filters ---
        match opts.include {
            Include::Files if is_directory => continue,
            Include::Dirs if !is_directory => continue,
            _ => {}
        }
        match opts.status {
            Status::Active if !in_use => continue,
            Status::Deleted if in_use => continue,
            _ => {}
        }
        if opts.only_timestomp && !timestomp_suspect {
            continue;
        }

        // path resolution (re-borrows parser mutably; `ex` already owns its data)
        let path = parser
            .get_full_path_for_entry(&entry)
            .ok()
            .flatten()
            .map(|p| p.to_string_lossy().replace('\\', "/"));

        if let Some(ref needle) = want {
            let name_hit = ex.name.to_lowercase().contains(needle);
            let path_hit = path
                .as_deref()
                .map(|p| p.to_lowercase().contains(needle))
                .unwrap_or(false);
            if !name_hit && !path_hit {
                continue;
            }
        }

        // --- matched ---
        matched += 1;
        if is_directory {
            directories += 1;
        } else {
            files += 1;
        }
        if in_use {
            active += 1;
        } else {
            deleted += 1;
        }
        if timestomp_suspect {
            timestomp_suspects += 1;
        }
        for t in [ex.si.as_ref().map(|s| s.1), ex.fname.as_ref().map(|s| s.1)]
            .into_iter()
            .flatten()
        {
            earliest = Some(earliest.map_or(t, |e| if e <= t { e } else { t }));
            latest = Some(latest.map_or(t, |l| if l >= t { l } else { t }));
        }

        if opts.summary {
            continue;
        }
        if opts.max_records != 0 && entries.len() >= opts.max_records {
            continue;
        }
        entries.push(EntryOut {
            entry: entry.header.record_number,
            sequence: entry.header.sequence,
            in_use,
            is_directory,
            parent_entry: ex.parent_entry,
            name: ex.name,
            path,
            size: ex.size,
            timestomp_suspect,
            standard_info: ex.si.map(|s| s.0),
            file_name: ex.fname.map(|s| s.0),
        });
    }

    if total == 0 {
        return Err(
            "not a valid $MFT buffer: no decodable FILE records found (expected 1024-byte MFT entries)"
                .to_string(),
        );
    }

    let returned = entries.len();
    let summary = if opts.summary {
        Some(Summary {
            files,
            directories,
            active,
            deleted,
            timestomp_suspects,
            earliest: earliest.map(|t| t.to_string()),
            latest: latest.map(|t| t.to_string()),
        })
    } else {
        None
    };

    Ok(Report {
        total_records: total,
        matched_records: matched,
        returned_records: returned,
        parse_errors,
        truncated: matched > returned && !opts.summary,
        summary,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGLE: &[u8] = include_bytes!("../tests/fixtures/entry_single_file");
    const DIR: &[u8] = include_bytes!("../tests/fixtures/dir_entry");

    #[test]
    fn parses_single_file_entry() {
        let r = parse(SINGLE, &Options::default()).unwrap();
        assert_eq!(r.total_records, 1);
        assert_eq!(r.matched_records, 1);
        assert_eq!(r.returned_records, 1);
        assert!(!r.truncated);
        let e = &r.entries[0];
        assert!(!e.name.is_empty(), "entry should have a name");
        assert!(e.standard_info.is_some(), "should carry SI timestamps");
        assert!(e.file_name.is_some(), "should carry FN timestamps");
        // SI created is a real RFC-3339 instant.
        assert!(e.standard_info.as_ref().unwrap().created.contains('T'));
    }

    #[test]
    fn dir_entry_is_directory() {
        let r = parse(DIR, &Options::default()).unwrap();
        assert_eq!(r.total_records, 1);
        assert!(r.entries[0].is_directory, "index-root entry is a directory");
    }

    #[test]
    fn include_files_only_drops_directory() {
        let mut o = Options::default();
        o.include = Include::Files;
        let r = parse(DIR, &o).unwrap();
        assert_eq!(r.total_records, 1);
        assert_eq!(r.matched_records, 0, "the only record is a directory");
        assert!(r.entries.is_empty());
    }

    #[test]
    fn max_records_caps_and_marks_truncated() {
        // Both fixtures concatenated = 2 records; cap at 1.
        let mut buf = SINGLE.to_vec();
        buf.extend_from_slice(DIR);
        let mut o = Options::default();
        o.max_records = 1;
        let r = parse(&buf, &o).unwrap();
        assert_eq!(r.total_records, 2);
        assert_eq!(r.returned_records, 1);
        assert_eq!(r.matched_records, 2);
        assert!(r.truncated);
    }

    #[test]
    fn summary_mode_counts_without_records() {
        let mut buf = SINGLE.to_vec();
        buf.extend_from_slice(DIR);
        let mut o = Options::default();
        o.summary = true;
        let r = parse(&buf, &o).unwrap();
        assert!(r.entries.is_empty());
        let s = r.summary.unwrap();
        assert_eq!(s.files + s.directories, 2);
        assert_eq!(s.active + s.deleted, 2);
        assert!(s.earliest.is_some() && s.latest.is_some());
    }

    #[test]
    fn path_contains_filters() {
        let name = {
            let r = parse(SINGLE, &Options::default()).unwrap();
            r.entries[0].name.clone()
        };
        let mut o = Options::default();
        o.path_contains = Some(name.to_uppercase()); // case-insensitive
        let r = parse(SINGLE, &o).unwrap();
        assert_eq!(r.matched_records, 1);

        let mut o2 = Options::default();
        o2.path_contains = Some("definitely-not-present-xyzzy".into());
        let r2 = parse(SINGLE, &o2).unwrap();
        assert_eq!(r2.matched_records, 0);
    }

    #[test]
    fn rejects_non_mft_bytes() {
        let err = parse(b"not a master file table at all", &Options::default()).unwrap_err();
        assert!(err.contains("not a valid $MFT"), "got: {err}");
    }
}
