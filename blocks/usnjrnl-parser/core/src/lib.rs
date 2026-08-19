//! usnjrnl-parser core — read an NTFS `$UsnJrnl:$J` change journal into a
//! file-activity timeline.
//!
//! The USN (Update Sequence Number) change journal is the `$J` alternate data
//! stream of `\$Extend\$UsnJrnl`. NTFS appends one fixed-layout `USN_RECORD`
//! per metadata change, so the stream is a near-complete log of file creation,
//! renaming, writing and deletion — including for files that no longer exist.
//! That makes it one of the highest-value artifacts in a filesystem timeline.
//!
//! Three record layouts exist and all three are recognised here:
//!
//! * **V2** — the classic layout: 64-bit file reference numbers, a UTF-16LE
//!   name at the end of the record. Still what most volumes emit.
//! * **V3** — identical semantics with 128-bit `FILE_ID_128` references (ReFS
//!   and newer NTFS features). On NTFS the low 64 bits are the ordinary MFT
//!   reference, so entry/sequence decoding is shared.
//! * **V4** — range-tracking records. They carry extents, *no timestamp and no
//!   file name*, so they are counted and reported but never rendered as if they
//!   were a file event.
//!
//! `$J` is a **sparse** file: the deallocated head reads back as zeroes, and a
//! carved or partially-overwritten copy can contain arbitrary garbage between
//! valid records. The scanner therefore never assumes it starts on a record
//! boundary — it validates every candidate header and resynchronises on the
//! 8-byte alignment NTFS guarantees, reporting how many bytes it skipped as
//! sparse versus unparseable.
//!
//! One deliberate omission: a `$J` record stores the **parent reference
//! number**, never the parent's name, so a full path cannot be reconstructed
//! from this artifact alone. Rather than invent paths, every row carries the
//! parent MFT entry + sequence so it can be joined against a `$MFT` listing.
//!
//! Pure compute, no wafer/wasm-bindgen deps — shared verbatim by the chat skill
//! block and the web page, so it runs on every backend including the chat
//! Service Worker.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Serialize;

/// Largest decoded journal accepted. Beyond this the paste surfaces (chat and
/// the page textarea) are the wrong tool anyway.
const MAX_INPUT_BYTES: usize = 48 * 1024 * 1024;
/// `max_entries` when the caller leaves it at 0.
const DEFAULT_MAX_ENTRIES: usize = 200;
/// Upper bound on `max_entries` — past this it is a data dump, not a timeline,
/// and it would blow through the chat surface's token budget.
const MAX_MAX_ENTRIES: usize = 5000;
/// Shortest byte length a real `USN_RECORD` can claim (V2 header, empty name,
/// rounded up to the 8-byte alignment NTFS writes).
const MIN_RECORD_LEN: usize = 60;
/// Longest byte length a real `USN_RECORD` can claim. A V3 header plus the
/// 255-character maximum name is 586 bytes; the ceiling is deliberately loose
/// so that alternate-stream names still parse, but tight enough that random
/// garbage almost never passes validation.
const MAX_RECORD_LEN: usize = 4096;
/// V2 fixed-header size (`FileNameOffset` may not point below this).
const V2_HEADER: usize = 60;
/// V3 fixed-header size.
const V3_HEADER: usize = 76;
/// V4 fixed-header size (record length / version / two 128-bit refs / USN /
/// reason / source info / remaining extents / extent count / extent size).
const V4_HEADER: usize = 80;
/// How far ahead of a `RENAME_OLD_NAME` record to look for its
/// `RENAME_NEW_NAME` partner. NTFS writes them back to back; the window only
/// has to tolerate interleaving from other threads.
const RENAME_WINDOW: usize = 64;
/// Names listed in the summary's activity ranking.
const TOP_NAMES: usize = 10;

// ---------------------------------------------------------------------------
// Input decoding
// ---------------------------------------------------------------------------

/// How to interpret the supplied journal bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InFmt {
    /// A hex string (`50000000 0200 0000…`, optionally spaced/`0x`-prefixed).
    Hex,
    /// Standard Base64 (RFC 4648), padding optional on decode.
    Base64,
}

impl InFmt {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "hex" => Ok(InFmt::Hex),
            "base64" | "b64" => Ok(InFmt::Base64),
            other => Err(format!(
                "invalid input_encoding {other:?}: expected \"hex\" or \"base64\""
            )),
        }
    }

    fn to_bytes(self, s: &str) -> Result<Vec<u8>, String> {
        match self {
            InFmt::Hex => decode_hex(s),
            InFmt::Base64 => {
                let cleaned: String = s.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                B64.decode(cleaned.as_bytes())
                    .or_else(|_| {
                        base64::engine::general_purpose::STANDARD_NO_PAD
                            .decode(cleaned.trim_end_matches('=').as_bytes())
                    })
                    .map_err(|e| format!("invalid Base64 input: {e}"))
            }
        }
    }
}

/// Hex with tolerant separators: whitespace, `:`, `-`, `,`, `_`, `.` and a
/// leading `0x` — so `xxd`, `xxd -p`, PowerShell and hex-editor output all work.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(s.len() / 2);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '0' if matches!(chars.peek(), Some('x') | Some('X')) => {
                chars.next();
            }
            c if c.is_ascii_hexdigit() => nibbles.push(c.to_digit(16).unwrap() as u8),
            c if c.is_ascii_whitespace() || matches!(c, ':' | '-' | ',' | '_' | '.') => {}
            other => {
                return Err(format!(
                    "invalid hex input: unexpected character {other:?}. Hex bytes may be \
                     separated by spaces, newlines, colons, dashes or commas."
                ))
            }
        }
    }
    if nibbles.len() % 2 != 0 {
        return Err(format!(
            "invalid hex input: {} hex digits is an odd count, so the last byte is incomplete.",
            nibbles.len()
        ));
    }
    Ok(nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

// ---------------------------------------------------------------------------
// Flag tables
// ---------------------------------------------------------------------------

/// `USN_REASON_*` bits, in the order Microsoft documents them.
const REASON_FLAGS: &[(u32, &str)] = &[
    (0x0000_0001, "DATA_OVERWRITE"),
    (0x0000_0002, "DATA_EXTEND"),
    (0x0000_0004, "DATA_TRUNCATION"),
    (0x0000_0010, "NAMED_DATA_OVERWRITE"),
    (0x0000_0020, "NAMED_DATA_EXTEND"),
    (0x0000_0040, "NAMED_DATA_TRUNCATION"),
    (0x0000_0100, "FILE_CREATE"),
    (0x0000_0200, "FILE_DELETE"),
    (0x0000_0400, "EA_CHANGE"),
    (0x0000_0800, "SECURITY_CHANGE"),
    (0x0000_1000, "RENAME_OLD_NAME"),
    (0x0000_2000, "RENAME_NEW_NAME"),
    (0x0000_4000, "INDEXABLE_CHANGE"),
    (0x0000_8000, "BASIC_INFO_CHANGE"),
    (0x0001_0000, "HARD_LINK_CHANGE"),
    (0x0002_0000, "COMPRESSION_CHANGE"),
    (0x0004_0000, "ENCRYPTION_CHANGE"),
    (0x0008_0000, "OBJECT_ID_CHANGE"),
    (0x0010_0000, "REPARSE_POINT_CHANGE"),
    (0x0020_0000, "STREAM_CHANGE"),
    (0x0040_0000, "TRANSACTED_CHANGE"),
    (0x0080_0000, "INTEGRITY_CHANGE"),
    (0x0100_0000, "DESIRED_STORAGE_CLASS_CHANGE"),
    (0x8000_0000, "CLOSE"),
];

/// `USN_SOURCE_*` bits — set when the change came from the system rather than a
/// user action, which is exactly what an analyst wants to discount.
const SOURCE_FLAGS: &[(u32, &str)] = &[
    (0x0000_0001, "DATA_MANAGEMENT"),
    (0x0000_0002, "AUXILIARY_DATA"),
    (0x0000_0004, "REPLICATION_MANAGEMENT"),
    (0x0000_0008, "CLIENT_REPLICATION_MANAGEMENT"),
];

/// `FILE_ATTRIBUTE_*` bits.
const ATTR_FLAGS: &[(u32, &str)] = &[
    (0x0000_0001, "READONLY"),
    (0x0000_0002, "HIDDEN"),
    (0x0000_0004, "SYSTEM"),
    (0x0000_0010, "DIRECTORY"),
    (0x0000_0020, "ARCHIVE"),
    (0x0000_0040, "DEVICE"),
    (0x0000_0080, "NORMAL"),
    (0x0000_0100, "TEMPORARY"),
    (0x0000_0200, "SPARSE_FILE"),
    (0x0000_0400, "REPARSE_POINT"),
    (0x0000_0800, "COMPRESSED"),
    (0x0000_1000, "OFFLINE"),
    (0x0000_2000, "NOT_CONTENT_INDEXED"),
    (0x0000_4000, "ENCRYPTED"),
    (0x0000_8000, "INTEGRITY_STREAM"),
    (0x0001_0000, "VIRTUAL"),
    (0x0002_0000, "NO_SCRUB_DATA"),
    (0x0004_0000, "RECALL_ON_OPEN"),
    (0x0040_0000, "RECALL_ON_DATA_ACCESS"),
];

const R_DATA_OVERWRITE: u32 = 0x0000_0001;
const R_FILE_CREATE: u32 = 0x0000_0100;
const R_FILE_DELETE: u32 = 0x0000_0200;
const R_RENAME_OLD: u32 = 0x0000_1000;
const R_RENAME_NEW: u32 = 0x0000_2000;
const R_CLOSE: u32 = 0x8000_0000;
/// Every data/stream write bit: the three unnamed-stream and the three
/// named-stream (alternate data stream) write reasons.
const WRITE_MASK: u32 =
    R_DATA_OVERWRITE | 0x0000_0002 | 0x0000_0004 | 0x0000_0010 | 0x0000_0020 | 0x0000_0040;
/// Every rename bit.
const RENAME_MASK: u32 = R_RENAME_OLD | R_RENAME_NEW;
/// Every "the file's metadata changed but its data did not" bit.
const METADATA_MASK: u32 = 0x0000_0400
    | 0x0000_0800
    | 0x0000_4000
    | 0x0000_8000
    | 0x0001_0000
    | 0x0002_0000
    | 0x0004_0000
    | 0x0008_0000
    | 0x0010_0000
    | 0x0020_0000
    | 0x0040_0000
    | 0x0080_0000
    | 0x0100_0000;
/// `FILE_ATTRIBUTE_DIRECTORY`.
const ATTR_DIRECTORY: u32 = 0x0000_0010;

/// Expand a bitmask into its documented flag names, appending `UNKNOWN_0x…` for
/// bits the table does not cover (never silently dropped).
fn flag_names(value: u32, table: &[(u32, &str)]) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = 0u32;
    for (bit, name) in table {
        if value & bit != 0 {
            out.push((*name).to_string());
            seen |= bit;
        }
    }
    let leftover = value & !seen;
    if leftover != 0 {
        out.push(format!("UNKNOWN_0x{leftover:08X}"));
    }
    out
}

/// Flag names joined for display, or a placeholder when the mask is empty.
fn flags_or(value: u32, table: &[(u32, &str)], empty: &str) -> String {
    let names = flag_names(value, table);
    if names.is_empty() {
        empty.to_string()
    } else {
        names.join(" | ")
    }
}

// ---------------------------------------------------------------------------
// Time helpers
// ---------------------------------------------------------------------------

/// Windows FILETIME epoch (1601-01-01) → Unix epoch, in seconds.
const FILETIME_EPOCH_DIFF: i64 = 11_644_473_600;

/// FILETIME (100 ns ticks since 1601 UTC) → Unix seconds, rejecting the
/// placeholder values and anything outside 1970..=2100.
fn filetime_to_epoch(ticks: u64) -> Option<i64> {
    if ticks == 0 || ticks == u64::MAX {
        return None;
    }
    let secs = (ticks / 10_000_000) as i64 - FILETIME_EPOCH_DIFF;
    if (0..=4_102_444_800).contains(&secs) {
        Some(secs)
    } else {
        None
    }
}

/// FILETIME → ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`). Output is always UTC: a
/// browser tool cannot know the acquisition host's timezone, and guessing it
/// would silently mis-time a timeline.
fn filetime_to_iso(ticks: u64) -> Option<String> {
    filetime_to_epoch(ticks).map(|secs| {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        )
    })
}

/// Howard Hinnant's `civil_from_days` — days since 1970-01-01 → (y, m, d).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// Which change class to keep.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Event {
    All,
    Create,
    Delete,
    Rename,
    Write,
    Metadata,
    Close,
}

impl Event {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "all" => Ok(Event::All),
            "create" => Ok(Event::Create),
            "delete" => Ok(Event::Delete),
            "rename" => Ok(Event::Rename),
            "write" => Ok(Event::Write),
            "metadata" => Ok(Event::Metadata),
            "close" => Ok(Event::Close),
            other => Err(format!(
                "invalid event {other:?}: expected \"all\", \"create\", \"delete\", \
                 \"rename\", \"write\", \"metadata\" or \"close\""
            )),
        }
    }

    fn mask(self) -> u32 {
        match self {
            Event::All => u32::MAX,
            Event::Create => R_FILE_CREATE,
            Event::Delete => R_FILE_DELETE,
            Event::Rename => RENAME_MASK,
            Event::Write => WRITE_MASK,
            Event::Metadata => METADATA_MASK,
            Event::Close => R_CLOSE,
        }
    }
}

/// Files-only / directories-only filter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Include {
    All,
    Files,
    Dirs,
}

impl Include {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "all" => Ok(Include::All),
            "files" => Ok(Include::Files),
            "dirs" | "directories" => Ok(Include::Dirs),
            other => Err(format!(
                "invalid include {other:?}: expected \"all\", \"files\" or \"dirs\""
            )),
        }
    }
}

/// Output rendering.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Summary,
    Report,
    List,
    Csv,
    Bodyfile,
    Tln,
    Json,
}

impl Mode {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "summary" => Ok(Mode::Summary),
            "" | "report" => Ok(Mode::Report),
            "list" => Ok(Mode::List),
            "csv" => Ok(Mode::Csv),
            "bodyfile" => Ok(Mode::Bodyfile),
            "tln" => Ok(Mode::Tln),
            "json" => Ok(Mode::Json),
            other => Err(format!(
                "invalid mode {other:?}: expected \"summary\", \"report\", \"list\", \
                 \"csv\", \"bodyfile\", \"tln\" or \"json\""
            )),
        }
    }
}

/// Ordering applied before the entry cap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sort {
    /// Journal order — ascending USN, which is chronological by construction.
    Usn,
    /// Newest first.
    Time,
    /// File name A→Z, case-insensitive.
    Name,
}

impl Sort {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "usn" => Ok(Sort::Usn),
            "time" => Ok(Sort::Time),
            "name" => Ok(Sort::Name),
            other => Err(format!(
                "invalid sort {other:?}: expected \"usn\", \"time\" or \"name\""
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Record model
// ---------------------------------------------------------------------------

/// One decoded `USN_RECORD`, after optional rename pairing.
#[derive(Clone, Debug)]
struct Record {
    /// Byte offset of the record inside the supplied stream.
    offset: usize,
    /// `RecordLength` as claimed by the record header.
    length: usize,
    major: u16,
    minor: u16,
    usn: i64,
    /// MFT entry number of the changed file (low 48 bits of the reference).
    file_entry: u64,
    /// Sequence number of the changed file (high 16 bits of the reference).
    file_seq: u16,
    /// MFT entry number of the containing directory.
    parent_entry: u64,
    parent_seq: u16,
    /// Raw FILETIME ticks (0 when the layout carries no timestamp).
    ticks: u64,
    reason: u32,
    source_info: u32,
    security_id: u32,
    attributes: u32,
    name: String,
    /// New name, when a `RENAME_OLD_NAME` record was paired with its
    /// `RENAME_NEW_NAME` partner.
    rename_to: Option<String>,
}

impl Record {
    fn is_directory(&self) -> bool {
        self.attributes & ATTR_DIRECTORY != 0
    }

    /// The single change class shown in the `change` column, chosen by
    /// forensic significance rather than bit order.
    fn change(&self) -> &'static str {
        let r = self.reason;
        if self.rename_to.is_some() {
            "Rename"
        } else if r & R_FILE_DELETE != 0 {
            "File delete"
        } else if r & R_FILE_CREATE != 0 {
            "File create"
        } else if r & R_RENAME_OLD != 0 {
            "Rename (old name)"
        } else if r & R_RENAME_NEW != 0 {
            "Rename (new name)"
        } else if r & WRITE_MASK != 0 {
            "Data write"
        } else if r & METADATA_MASK != 0 {
            "Metadata change"
        } else if r & R_CLOSE != 0 {
            "Close"
        } else {
            "Other"
        }
    }

    /// Display name, showing both sides of a paired rename.
    fn display_name(&self) -> String {
        match &self.rename_to {
            Some(to) => format!("{} -> {}", self.name, to),
            None => self.name.clone(),
        }
    }

    fn iso_time(&self) -> Option<String> {
        filetime_to_iso(self.ticks)
    }

    fn time_or(&self, empty: &str) -> String {
        self.iso_time().unwrap_or_else(|| empty.to_string())
    }
}

/// The JSON view emitted by `mode=json` — every decoded field, nothing derived
/// away.
#[derive(Serialize)]
struct RecordJson {
    offset: usize,
    record_length: usize,
    version: String,
    usn: i64,
    timestamp: Option<String>,
    timestamp_filetime: u64,
    change: String,
    reason: String,
    reason_flags: Vec<String>,
    reason_value: u32,
    file_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    renamed_to: Option<String>,
    is_directory: bool,
    file_attributes: String,
    file_attribute_flags: Vec<String>,
    file_attributes_value: u32,
    file_entry: u64,
    file_sequence: u16,
    parent_entry: u64,
    parent_sequence: u16,
    source_info: String,
    source_info_flags: Vec<String>,
    source_info_value: u32,
    security_id: u32,
}

impl RecordJson {
    fn from(r: &Record) -> Self {
        RecordJson {
            offset: r.offset,
            record_length: r.length,
            version: format!("{}.{}", r.major, r.minor),
            usn: r.usn,
            timestamp: r.iso_time(),
            timestamp_filetime: r.ticks,
            change: r.change().to_string(),
            reason: flags_or(r.reason, REASON_FLAGS, ""),
            reason_flags: flag_names(r.reason, REASON_FLAGS),
            reason_value: r.reason,
            file_name: r.name.clone(),
            renamed_to: r.rename_to.clone(),
            is_directory: r.is_directory(),
            file_attributes: flags_or(r.attributes, ATTR_FLAGS, ""),
            file_attribute_flags: flag_names(r.attributes, ATTR_FLAGS),
            file_attributes_value: r.attributes,
            file_entry: r.file_entry,
            file_sequence: r.file_seq,
            parent_entry: r.parent_entry,
            parent_sequence: r.parent_seq,
            source_info: flags_or(r.source_info, SOURCE_FLAGS, ""),
            source_info_flags: flag_names(r.source_info, SOURCE_FLAGS),
            source_info_value: r.source_info,
            security_id: r.security_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

/// What a full pass over the stream found, including the bytes it could not use.
struct Scan {
    records: Vec<Record>,
    /// Bytes inside the sparse (all-zero) regions of `$J`.
    zero_bytes: usize,
    /// Bytes skipped while resynchronising on a non-record.
    resync_bytes: usize,
    /// V4 range-tracking records: counted, never rendered as file events.
    v4_records: usize,
    v2_records: usize,
    v3_records: usize,
    /// Offset of the first non-zero byte, for the "this is not a `$J`" error.
    first_nonzero: Option<usize>,
}

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn u64le(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}

/// Split an NTFS file reference number into (MFT entry, sequence).
fn split_ref(reference: u64) -> (u64, u16) {
    (reference & 0x0000_FFFF_FFFF_FFFF, (reference >> 48) as u16)
}

/// Decode the UTF-16LE name a record points at, validating that it lies inside
/// the record. Returns `None` when the header is not a plausible record.
fn read_name(rec: &[u8], name_off: usize, name_len: usize, header: usize) -> Option<String> {
    if name_len == 0 || name_len % 2 != 0 || name_off < header {
        return None;
    }
    let end = name_off.checked_add(name_len)?;
    if end > rec.len() {
        return None;
    }
    let units: Vec<u16> = rec[name_off..end]
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    Some(String::from_utf16_lossy(&units))
}

/// Try to decode a V2 or V3 record at `buf[pos..pos+len]`.
fn parse_record(buf: &[u8], pos: usize, len: usize) -> Option<Record> {
    let rec = &buf[pos..pos + len];
    let major = u16le(rec, 4);
    let minor = u16le(rec, 6);
    let (header, file_ref, parent_ref, usn, ticks, reason, source_info, security_id, attrs, nl, no) =
        match major {
            2 => {
                if len < V2_HEADER {
                    return None;
                }
                (
                    V2_HEADER,
                    u64le(rec, 8),
                    u64le(rec, 16),
                    u64le(rec, 24) as i64,
                    u64le(rec, 32),
                    u32le(rec, 40),
                    u32le(rec, 44),
                    u32le(rec, 48),
                    u32le(rec, 52),
                    u16le(rec, 56) as usize,
                    u16le(rec, 58) as usize,
                )
            }
            3 => {
                if len < V3_HEADER {
                    return None;
                }
                // FILE_ID_128: on NTFS the low 64 bits are the ordinary
                // reference number, the high 64 are zero.
                (
                    V3_HEADER,
                    u64le(rec, 8),
                    u64le(rec, 24),
                    u64le(rec, 40) as i64,
                    u64le(rec, 48),
                    u32le(rec, 56),
                    u32le(rec, 60),
                    u32le(rec, 64),
                    u32le(rec, 68),
                    u16le(rec, 72) as usize,
                    u16le(rec, 74) as usize,
                )
            }
            _ => return None,
        };
    if minor != 0 {
        return None;
    }
    let name = read_name(rec, no, nl, header)?;
    let (file_entry, file_seq) = split_ref(file_ref);
    let (parent_entry, parent_seq) = split_ref(parent_ref);
    Some(Record {
        offset: pos,
        length: len,
        major,
        minor,
        usn,
        file_entry,
        file_seq,
        parent_entry,
        parent_seq,
        ticks,
        reason,
        source_info,
        security_id,
        attributes: attrs,
        name,
        rename_to: None,
    })
}

/// Walk the whole stream, tolerating sparse regions and garbage.
fn scan(buf: &[u8]) -> Scan {
    let mut out = Scan {
        records: Vec::new(),
        zero_bytes: 0,
        resync_bytes: 0,
        v4_records: 0,
        v2_records: 0,
        v3_records: 0,
        first_nonzero: buf.iter().position(|b| *b != 0),
    };
    let mut pos = 0usize;
    while pos + 4 <= buf.len() {
        let step = 8usize.min(buf.len() - pos);
        let len = u32le(buf, pos) as usize;
        if len == 0 {
            out.zero_bytes += step;
            pos += step;
            continue;
        }
        if len < MIN_RECORD_LEN || len > MAX_RECORD_LEN || len % 8 != 0 || pos + len > buf.len() {
            out.resync_bytes += step;
            pos += step;
            continue;
        }
        let major = u16le(buf, pos + 4);
        if major == 4 {
            if len >= V4_HEADER {
                out.v4_records += 1;
                pos += len;
                continue;
            }
            out.resync_bytes += step;
            pos += step;
            continue;
        }
        match parse_record(buf, pos, len) {
            Some(rec) => {
                if rec.major == 2 {
                    out.v2_records += 1;
                } else {
                    out.v3_records += 1;
                }
                out.records.push(rec);
                pos += len;
            }
            None => {
                out.resync_bytes += step;
                pos += step;
            }
        }
    }
    out
}

/// Merge each `RENAME_OLD_NAME` record with the following `RENAME_NEW_NAME`
/// record for the same file, so a rename reads as one timeline row instead of
/// two half-rows. The merged row keeps the old name, gains `rename_to`, and
/// adopts the completing record's USN, timestamp and attributes.
fn pair_rename_records(records: Vec<Record>) -> Vec<Record> {
    let n = records.len();
    let mut used = vec![false; n];
    let mut out: Vec<Record> = Vec::with_capacity(n);
    for i in 0..n {
        if used[i] {
            continue;
        }
        let r = &records[i];
        if r.reason & R_RENAME_OLD != 0 && r.reason & R_RENAME_NEW == 0 {
            let limit = (i + RENAME_WINDOW).min(n - 1);
            let partner = ((i + 1)..=limit).find(|j| {
                !used[*j]
                    && records[*j].file_entry == r.file_entry
                    && records[*j].file_seq == r.file_seq
                    && records[*j].reason & R_RENAME_NEW != 0
            });
            if let Some(j) = partner {
                used[j] = true;
                let mut merged = r.clone();
                merged.rename_to = Some(records[j].name.clone());
                merged.reason |= records[j].reason;
                merged.usn = records[j].usn;
                merged.ticks = records[j].ticks;
                merged.attributes = records[j].attributes;
                merged.parent_entry = records[j].parent_entry;
                merged.parent_seq = records[j].parent_seq;
                out.push(merged);
                continue;
            }
        }
        out.push(r.clone());
    }
    out
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// RFC 4180 quoting: quote when the field holds a comma, quote, CR or LF.
fn csv_field(s: &str) -> String {
    if s.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Bodyfile and TLN are pipe-delimited with no escaping rule, so a literal pipe
/// in a file name would corrupt the row — replace it rather than emit a broken
/// line.
fn pipe_safe(s: &str) -> String {
    s.replace('|', "/")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Parse an NTFS `$UsnJrnl:$J` change journal and render it.
///
/// * `data` — the journal bytes as hex (default) or Base64.
/// * `input_encoding` — `hex` | `base64`.
/// * `event` — change class filter: `all`|`create`|`delete`|`rename`|`write`|`metadata`|`close`.
/// * `include` — `all` | `files` | `dirs`.
/// * `filter` — case-insensitive substring matched against the file name(s).
/// * `pair_renames` — merge `RENAME_OLD_NAME`/`RENAME_NEW_NAME` pairs.
/// * `mode` — `summary`|`report`|`list`|`csv`|`bodyfile`|`tln`|`json`.
/// * `host` — host/system name for the TLN host column.
/// * `sort` — `usn` | `time` | `name`.
/// * `max_entries` — cap on emitted records (0 = the 200 default).
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    input_encoding: &str,
    event: &str,
    include: &str,
    filter: &str,
    pair_renames: bool,
    mode: &str,
    host: &str,
    sort: &str,
    max_entries: usize,
) -> Result<String, String> {
    let fmt = InFmt::parse(input_encoding)?;
    let event = Event::parse(event)?;
    let include = Include::parse(include)?;
    let mode = Mode::parse(mode)?;
    let sort = Sort::parse(sort)?;
    let cap = match max_entries {
        0 => DEFAULT_MAX_ENTRIES,
        n => n.min(MAX_MAX_ENTRIES),
    };

    if data.trim().is_empty() {
        return Err("input is empty: paste the NTFS $UsnJrnl:$J stream bytes as hex \
                    (the default) or Base64."
            .to_string());
    }
    let bytes = fmt.to_bytes(data)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {} byte limit. Split the $J stream (for example with \
             `dd`) and parse it in chunks — the scanner resynchronises, so a chunk need not \
             start on a record boundary.",
            bytes.len(),
            MAX_INPUT_BYTES
        ));
    }
    if bytes.len() < MIN_RECORD_LEN {
        return Err(format!(
            "input is only {}; the shortest possible USN_RECORD is {MIN_RECORD_LEN} bytes. \
             Export the whole $Extend\\$UsnJrnl:$J alternate data stream.",
            plural(bytes.len(), "byte", "bytes")
        ));
    }

    let mut scanned = scan(&bytes);
    if scanned.records.is_empty() && scanned.v4_records == 0 {
        let detail = match scanned.first_nonzero {
            None => "The input is entirely zeroes. $J is a sparse file, so its deallocated head \
                     reads back as zeroes — export the whole stream, or seek past the sparse \
                     region before copying."
                .to_string(),
            Some(off) => {
                let len = if off + 4 <= bytes.len() {
                    u32le(&bytes, off) as u64
                } else {
                    0
                };
                format!(
                    "The first non-zero byte is at offset {off}, where the 4-byte record length \
                     reads {len}. A valid length is a multiple of 8 between {MIN_RECORD_LEN} and \
                     {MAX_RECORD_LEN}, followed by major version 2, 3 or 4."
                )
            }
        };
        return Err(format!(
            "no USN records found in {}. Expected an NTFS $UsnJrnl:$J change journal: \
             8-byte-aligned USN_RECORD structures, each starting with its own 4-byte length. \
             {detail}",
            plural(bytes.len(), "byte", "bytes")
        ));
    }

    let raw = std::mem::take(&mut scanned.records);
    let parsed_total = raw.len();
    let records = if pair_renames {
        pair_rename_records(raw)
    } else {
        raw
    };
    let paired = parsed_total - records.len();

    let needle = filter.trim().to_lowercase();
    let mask = event.mask();
    let mut matched: Vec<Record> = records
        .into_iter()
        .filter(|r| r.reason & mask != 0)
        .filter(|r| match include {
            Include::All => true,
            Include::Files => !r.is_directory(),
            Include::Dirs => r.is_directory(),
        })
        .filter(|r| {
            if needle.is_empty() {
                return true;
            }
            r.name.to_lowercase().contains(&needle)
                || r
                    .rename_to
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains(&needle))
        })
        .collect();

    match sort {
        Sort::Usn => matched.sort_by_key(|r| (r.usn, r.offset)),
        Sort::Time => matched.sort_by(|a, b| b.ticks.cmp(&a.ticks).then(b.usn.cmp(&a.usn))),
        Sort::Name => matched.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.usn.cmp(&b.usn))
        }),
    }

    let matched_total = matched.len();
    let truncated = matched_total > cap;
    let shown: Vec<Record> = matched.iter().take(cap).cloned().collect();

    let ctx = RenderCtx {
        scanned: &scanned,
        parsed_total,
        paired,
        matched_total,
        shown: &shown,
        truncated,
        cap,
        input_len: bytes.len(),
        host: host.trim(),
    };

    Ok(match mode {
        Mode::Summary => render_summary(&ctx, &matched),
        Mode::Report => render_report(&ctx),
        Mode::List => render_list(&ctx),
        Mode::Csv => render_csv(&ctx),
        Mode::Bodyfile => render_bodyfile(&ctx),
        Mode::Tln => render_tln(&ctx),
        Mode::Json => render_json(&ctx),
    })
}

struct RenderCtx<'a> {
    scanned: &'a Scan,
    parsed_total: usize,
    paired: usize,
    matched_total: usize,
    shown: &'a [Record],
    truncated: bool,
    cap: usize,
    input_len: usize,
    host: &'a str,
}

impl RenderCtx<'_> {
    /// The one-line provenance banner shared by the human-readable modes.
    fn banner(&self) -> String {
        let mut s = format!(
            "$UsnJrnl:$J — {} parsed, {} matched, {} shown",
            plural(self.parsed_total, "record", "records"),
            self.matched_total,
            self.shown.len()
        );
        if self.truncated {
            s.push_str(&format!(" (capped at max_entries={})", self.cap));
        }
        s
    }

    fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.paired == 1 {
            notes.push("1 rename pair merged into a single Rename row (pair_renames=true).".into());
        } else if self.paired > 1 {
            notes.push(format!(
                "{} rename pairs merged into a single Rename row each (pair_renames=true).",
                self.paired
            ));
        }
        if self.scanned.v4_records > 0 {
            notes.push(format!(
                "{} skipped: V4 range-tracking records carry extents, not a name or timestamp.",
                plural(self.scanned.v4_records, "record", "records")
            ));
        }
        if self.scanned.zero_bytes > 0 {
            notes.push(format!(
                "{} of sparse (zeroed) journal skipped.",
                plural(self.scanned.zero_bytes, "byte", "bytes")
            ));
        }
        if self.scanned.resync_bytes > 0 {
            notes.push(format!(
                "{} unparseable — resynchronised on the 8-byte record alignment.",
                plural(self.scanned.resync_bytes, "byte was", "bytes were")
            ));
        }
        notes
    }
}

fn render_report(ctx: &RenderCtx) -> String {
    let mut out = String::new();
    out.push_str(&ctx.banner());
    out.push('\n');
    for note in ctx.notes() {
        out.push_str(&format!("  note: {note}\n"));
    }
    if ctx.shown.is_empty() {
        out.push_str("\nNo records matched the current filters.\n");
        return out;
    }
    for (i, r) in ctx.shown.iter().enumerate() {
        out.push_str(&format!(
            "\n[{}] {}  USN {} (0x{:X})\n",
            i + 1,
            r.time_or("(no timestamp)"),
            r.usn,
            r.usn
        ));
        out.push_str(&format!("    File         {}\n", r.name));
        if let Some(to) = &r.rename_to {
            out.push_str(&format!("    Renamed to   {to}\n"));
        }
        out.push_str(&format!("    Change       {}\n", r.change()));
        out.push_str(&format!(
            "    Reasons      {}\n",
            flags_or(r.reason, REASON_FLAGS, "(none)")
        ));
        out.push_str(&format!(
            "    Attributes   {}\n",
            flags_or(r.attributes, ATTR_FLAGS, "(none)")
        ));
        out.push_str(&format!(
            "    File ref     MFT entry {}, sequence {}\n",
            r.file_entry, r.file_seq
        ));
        out.push_str(&format!(
            "    Parent ref   MFT entry {}, sequence {}\n",
            r.parent_entry, r.parent_seq
        ));
        out.push_str(&format!(
            "    Source       {}\n",
            flags_or(r.source_info, SOURCE_FLAGS, "(user action)")
        ));
        out.push_str(&format!("    Security id  {}\n", r.security_id));
        out.push_str(&format!(
            "    Record       v{}.{}, {} bytes at offset {}\n",
            r.major, r.minor, r.length, r.offset
        ));
    }
    out
}

fn render_list(ctx: &RenderCtx) -> String {
    let mut out = String::new();
    out.push_str(&ctx.banner());
    out.push('\n');
    for note in ctx.notes() {
        out.push_str(&format!("  note: {note}\n"));
    }
    if ctx.shown.is_empty() {
        out.push_str("No records matched the current filters.\n");
        return out;
    }
    for r in ctx.shown {
        out.push_str(&format!(
            "{}  usn={}  {}  {}  [{}]\n",
            r.time_or("(no timestamp)     "),
            r.usn,
            r.change(),
            r.display_name(),
            flags_or(r.reason, REASON_FLAGS, "none")
        ));
    }
    out
}

fn render_csv(ctx: &RenderCtx) -> String {
    let mut out = String::from(
        "timestamp,usn,file_name,renamed_to,change,reasons,file_attributes,is_directory,\
         file_entry,file_sequence,parent_entry,parent_sequence,source_info,security_id,\
         version,offset\n",
    );
    for r in ctx.shown {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_field(&r.time_or("")),
            r.usn,
            csv_field(&r.name),
            csv_field(r.rename_to.as_deref().unwrap_or("")),
            csv_field(r.change()),
            csv_field(&flags_or(r.reason, REASON_FLAGS, "")),
            csv_field(&flags_or(r.attributes, ATTR_FLAGS, "")),
            r.is_directory(),
            r.file_entry,
            r.file_seq,
            r.parent_entry,
            r.parent_seq,
            csv_field(&flags_or(r.source_info, SOURCE_FLAGS, "")),
            r.security_id,
            format_args!("{}.{}", r.major, r.minor),
            r.offset
        ));
    }
    out
}

/// Sleuth Kit 3.x bodyfile:
/// `MD5|name|inode|mode|UID|GID|size|atime|mtime|ctime|crtime`. A USN record
/// carries a single instant, so it is written into all four time columns —
/// mactime then shows the change once per column, which is the convention these
/// journal parsers already follow.
fn render_bodyfile(ctx: &RenderCtx) -> String {
    let mut out = String::new();
    for r in ctx.shown {
        let t = filetime_to_epoch(r.ticks).unwrap_or(0);
        let name = format!(
            "{} ({})",
            pipe_safe(&r.display_name()),
            pipe_safe(&flags_or(r.reason, REASON_FLAGS, "none"))
        );
        out.push_str(&format!(
            "0|{}|{}|0|0|0|0|{t}|{t}|{t}|{t}\n",
            name, r.file_entry
        ));
    }
    out
}

/// TLN timeline: `epoch|source|host|user|description`.
fn render_tln(ctx: &RenderCtx) -> String {
    let host = if ctx.host.is_empty() { "-" } else { ctx.host };
    let mut out = String::new();
    for r in ctx.shown {
        let t = filetime_to_epoch(r.ticks).unwrap_or(0);
        out.push_str(&format!(
            "{t}|USN|{}|-|{}: {} [{}]\n",
            pipe_safe(host),
            r.change(),
            pipe_safe(&r.display_name()),
            pipe_safe(&flags_or(r.reason, REASON_FLAGS, "none"))
        ));
    }
    out
}

fn render_json(ctx: &RenderCtx) -> String {
    #[derive(Serialize)]
    struct Out {
        input_bytes: usize,
        records_parsed: usize,
        records_matched: usize,
        records_shown: usize,
        truncated: bool,
        rename_pairs_merged: usize,
        v2_records: usize,
        v3_records: usize,
        v4_range_records: usize,
        sparse_bytes_skipped: usize,
        unparseable_bytes_skipped: usize,
        records: Vec<RecordJson>,
    }
    let out = Out {
        input_bytes: ctx.input_len,
        records_parsed: ctx.parsed_total,
        records_matched: ctx.matched_total,
        records_shown: ctx.shown.len(),
        truncated: ctx.truncated,
        rename_pairs_merged: ctx.paired,
        v2_records: ctx.scanned.v2_records,
        v3_records: ctx.scanned.v3_records,
        v4_range_records: ctx.scanned.v4_records,
        sparse_bytes_skipped: ctx.scanned.zero_bytes,
        unparseable_bytes_skipped: ctx.scanned.resync_bytes,
        records: ctx.shown.iter().map(RecordJson::from).collect(),
    };
    serde_json::to_string_pretty(&out).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Triage view: totals, spans and rankings over everything that matched — not
/// just the capped page — so it stays useful on a multi-million-record journal.
fn render_summary(ctx: &RenderCtx, matched: &[Record]) -> String {
    let mut out = String::from("$UsnJrnl:$J summary\n");
    out.push_str(&format!("  Input                  {} bytes\n", ctx.input_len));
    out.push_str(&format!("  Records parsed         {}\n", ctx.parsed_total));
    out.push_str(&format!("  Records matched        {}\n", ctx.matched_total));
    out.push_str(&format!(
        "  Record versions        v2: {}, v3: {}, v4 range: {}\n",
        ctx.scanned.v2_records, ctx.scanned.v3_records, ctx.scanned.v4_records
    ));
    out.push_str(&format!(
        "  Rename pairs merged    {}\n",
        ctx.paired
    ));
    out.push_str(&format!(
        "  Sparse bytes skipped   {}\n",
        ctx.scanned.zero_bytes
    ));
    out.push_str(&format!(
        "  Unparseable bytes      {}\n",
        ctx.scanned.resync_bytes
    ));

    if matched.is_empty() {
        out.push_str("\nNo records matched the current filters.\n");
        return out;
    }

    let min_usn = matched.iter().map(|r| r.usn).min().unwrap_or(0);
    let max_usn = matched.iter().map(|r| r.usn).max().unwrap_or(0);
    out.push_str(&format!(
        "  USN range              {min_usn} (0x{min_usn:X}) .. {max_usn} (0x{max_usn:X})\n"
    ));
    let times: Vec<u64> = matched.iter().map(|r| r.ticks).filter(|t| *t != 0).collect();
    match (times.iter().min(), times.iter().max()) {
        (Some(lo), Some(hi)) => out.push_str(&format!(
            "  Time range (UTC)       {} .. {}\n",
            filetime_to_iso(*lo).unwrap_or_else(|| "(unreadable)".into()),
            filetime_to_iso(*hi).unwrap_or_else(|| "(unreadable)".into())
        )),
        _ => out.push_str("  Time range (UTC)       (no readable timestamps)\n"),
    }

    let mut entries: Vec<u64> = matched.iter().map(|r| r.file_entry).collect();
    entries.sort_unstable();
    entries.dedup();
    out.push_str(&format!("  Distinct MFT entries   {}\n", entries.len()));
    let dirs = matched.iter().filter(|r| r.is_directory()).count();
    out.push_str(&format!(
        "  Directories / files    {} / {}\n",
        dirs,
        matched.len() - dirs
    ));

    out.push_str("\nChange classes\n");
    let mut classes: Vec<(&str, usize)> = Vec::new();
    for r in matched {
        let c = r.change();
        match classes.iter_mut().find(|(k, _)| *k == c) {
            Some((_, n)) => *n += 1,
            None => classes.push((c, 1)),
        }
    }
    classes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (class, n) in &classes {
        out.push_str(&format!("  {class:<20} {n}\n"));
    }

    out.push_str("\nMost active names\n");
    let mut names: Vec<(String, usize)> = Vec::new();
    for r in matched {
        match names.iter_mut().find(|(k, _)| *k == r.name) {
            Some((_, n)) => *n += 1,
            None => names.push((r.name.clone(), 1)),
        }
    }
    names.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (i, (name, n)) in names.iter().take(TOP_NAMES).enumerate() {
        out.push_str(&format!("  {:>2}. {name}  ({})\n", i + 1, plural(*n, "record", "records")));
    }
    if names.len() > TOP_NAMES {
        out.push_str(&format!("  … and {} more name(s)\n", names.len() - TOP_NAMES));
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a V2 `USN_RECORD` the way NTFS lays one out.
    #[allow(clippy::too_many_arguments)]
    fn v2(
        file_ref: u64,
        parent_ref: u64,
        usn: i64,
        ticks: u64,
        reason: u32,
        attrs: u32,
        name: &str,
    ) -> Vec<u8> {
        let name16: Vec<u8> = name
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect::<Vec<u8>>();
        let mut len = V2_HEADER + name16.len();
        if len % 8 != 0 {
            len += 8 - (len % 8);
        }
        let mut rec = vec![0u8; len];
        rec[0..4].copy_from_slice(&(len as u32).to_le_bytes());
        rec[4..6].copy_from_slice(&2u16.to_le_bytes());
        rec[6..8].copy_from_slice(&0u16.to_le_bytes());
        rec[8..16].copy_from_slice(&file_ref.to_le_bytes());
        rec[16..24].copy_from_slice(&parent_ref.to_le_bytes());
        rec[24..32].copy_from_slice(&usn.to_le_bytes());
        rec[32..40].copy_from_slice(&ticks.to_le_bytes());
        rec[40..44].copy_from_slice(&reason.to_le_bytes());
        rec[44..48].copy_from_slice(&0u32.to_le_bytes());
        rec[48..52].copy_from_slice(&256u32.to_le_bytes());
        rec[52..56].copy_from_slice(&attrs.to_le_bytes());
        rec[56..58].copy_from_slice(&(name16.len() as u16).to_le_bytes());
        rec[58..60].copy_from_slice(&(V2_HEADER as u16).to_le_bytes());
        rec[V2_HEADER..V2_HEADER + name16.len()].copy_from_slice(&name16);
        rec
    }

    /// Build a V3 record (128-bit references).
    fn v3(file_ref: u64, parent_ref: u64, usn: i64, ticks: u64, reason: u32, name: &str) -> Vec<u8> {
        let name16: Vec<u8> = name
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect::<Vec<u8>>();
        let mut len = V3_HEADER + name16.len();
        if len % 8 != 0 {
            len += 8 - (len % 8);
        }
        let mut rec = vec![0u8; len];
        rec[0..4].copy_from_slice(&(len as u32).to_le_bytes());
        rec[4..6].copy_from_slice(&3u16.to_le_bytes());
        rec[8..16].copy_from_slice(&file_ref.to_le_bytes());
        rec[24..32].copy_from_slice(&parent_ref.to_le_bytes());
        rec[40..48].copy_from_slice(&usn.to_le_bytes());
        rec[48..56].copy_from_slice(&ticks.to_le_bytes());
        rec[56..60].copy_from_slice(&reason.to_le_bytes());
        rec[68..72].copy_from_slice(&0x20u32.to_le_bytes());
        rec[72..74].copy_from_slice(&(name16.len() as u16).to_le_bytes());
        rec[74..76].copy_from_slice(&(V3_HEADER as u16).to_le_bytes());
        rec[V3_HEADER..V3_HEADER + name16.len()].copy_from_slice(&name16);
        rec
    }

    /// 2024-05-01T12:00:00Z as FILETIME ticks.
    fn ticks_2024() -> u64 {
        ((1_714_564_800i64 + FILETIME_EPOCH_DIFF) as u64) * 10_000_000
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn make_ref(entry: u64, seq: u16) -> u64 {
        entry | ((seq as u64) << 48)
    }

    /// A small journal: create + write/close for `notes.txt`, a rename pair for
    /// `draft.txt` → `final.txt`, a delete of `secret.tmp`, and a directory
    /// create.
    fn sample_journal() -> Vec<u8> {
        let t = ticks_2024();
        let file = make_ref(42, 1);
        let dir = make_ref(5, 1);
        let mut b = Vec::new();
        b.extend(v2(file, dir, 4096, t, R_FILE_CREATE, 0x20, "notes.txt"));
        b.extend(v2(
            file,
            dir,
            4184,
            t + 10_000_000,
            R_DATA_OVERWRITE | R_CLOSE,
            0x20,
            "notes.txt",
        ));
        let d = make_ref(77, 2);
        b.extend(v2(d, dir, 4272, t + 20_000_000, R_RENAME_OLD, 0x20, "draft.txt"));
        b.extend(v2(
            d,
            dir,
            4360,
            t + 20_000_000,
            R_RENAME_NEW | R_CLOSE,
            0x20,
            "final.txt",
        ));
        b.extend(v2(
            make_ref(99, 3),
            dir,
            4448,
            t + 30_000_000,
            R_FILE_DELETE | R_CLOSE,
            0x20,
            "secret.tmp",
        ));
        b.extend(v2(
            make_ref(120, 1),
            dir,
            4536,
            t + 40_000_000,
            R_FILE_CREATE | R_CLOSE,
            ATTR_DIRECTORY,
            "Reports",
        ));
        b
    }

    fn run_hex(bytes: &[u8], mode: &str) -> String {
        run(&hex(bytes), "hex", "all", "all", "", true, mode, "", "usn", 0).unwrap()
    }

    #[test]
    fn parses_a_v2_create_record_end_to_end() {
        let out = run_hex(&sample_journal(), "report");
        assert!(out.starts_with("$UsnJrnl:$J — 6 records parsed, 5 matched, 5 shown\n"));
        assert!(out.contains("[1] 2024-05-01T12:00:00Z  USN 4096 (0x1000)"));
        assert!(out.contains("    File         notes.txt"));
        assert!(out.contains("    Change       File create"));
        assert!(out.contains("    Reasons      FILE_CREATE"));
        assert!(out.contains("    Attributes   ARCHIVE"));
        assert!(out.contains("    File ref     MFT entry 42, sequence 1"));
        assert!(out.contains("    Parent ref   MFT entry 5, sequence 1"));
        assert!(out.contains("    Source       (user action)"));
        assert!(out.contains("    Record       v2.0, 80 bytes at offset 0"));
    }

    #[test]
    fn pairs_rename_old_and_new_into_one_row() {
        let out = run_hex(&sample_journal(), "list");
        assert!(out.contains("Rename  draft.txt -> final.txt"), "{out}");
        // 6 parsed, one pair merged → 5 rows.
        assert!(out.starts_with("$UsnJrnl:$J — 6 records parsed, 5 matched, 5 shown\n"));
        assert!(out.contains("note: 1 rename pair merged into a single Rename row"));
    }

    #[test]
    fn pair_renames_off_keeps_both_halves() {
        let j = sample_journal();
        let out = run(&hex(&j), "hex", "all", "all", "", false, "list", "", "usn", 0).unwrap();
        assert!(out.contains("Rename (old name)  draft.txt"), "{out}");
        assert!(out.contains("Rename (new name)  final.txt"), "{out}");
        assert!(out.starts_with("$UsnJrnl:$J — 6 records parsed, 6 matched, 6 shown\n"));
    }

    #[test]
    fn event_filter_selects_a_change_class() {
        let j = hex(&sample_journal());
        let del = run(&j, "hex", "delete", "all", "", true, "list", "", "usn", 0).unwrap();
        assert!(del.contains("secret.tmp"));
        assert!(!del.contains("notes.txt"));
        let create = run(&j, "hex", "create", "all", "", true, "list", "", "usn", 0).unwrap();
        assert!(create.contains("notes.txt"));
        assert!(create.contains("Reports"));
        assert!(!create.contains("secret.tmp"));
        let ren = run(&j, "hex", "rename", "all", "", true, "list", "", "usn", 0).unwrap();
        assert!(ren.contains("draft.txt -> final.txt"));
        assert!(!ren.contains("notes.txt"));
    }

    #[test]
    fn include_splits_files_from_directories() {
        let j = hex(&sample_journal());
        let dirs = run(&j, "hex", "all", "dirs", "", true, "list", "", "usn", 0).unwrap();
        assert!(dirs.contains("Reports"));
        assert!(!dirs.contains("notes.txt"));
        let files = run(&j, "hex", "all", "files", "", true, "list", "", "usn", 0).unwrap();
        assert!(files.contains("notes.txt"));
        assert!(!files.contains("Reports"));
    }

    #[test]
    fn filter_matches_both_sides_of_a_rename() {
        let j = hex(&sample_journal());
        let out = run(&j, "hex", "all", "all", "final", true, "list", "", "usn", 0).unwrap();
        assert!(out.contains("draft.txt -> final.txt"));
        assert!(!out.contains("notes.txt"));
    }

    #[test]
    fn v3_records_decode_with_128_bit_references() {
        let t = ticks_2024();
        let bytes = v3(make_ref(1000, 4), make_ref(5, 1), 9000, t, R_FILE_CREATE, "modern.log");
        let out = run_hex(&bytes, "report");
        assert!(out.contains("    File         modern.log"), "{out}");
        assert!(out.contains("    File ref     MFT entry 1000, sequence 4"), "{out}");
        assert!(out.contains("    Record       v3.0,"), "{out}");
    }

    #[test]
    fn sparse_head_and_garbage_are_skipped_and_reported() {
        let mut b = vec![0u8; 64]; // sparse head
        b.extend([0xAAu8; 8]); // garbage that fails length validation
        b.extend(sample_journal());
        let out = run_hex(&b, "report");
        assert!(out.contains("note: 64 bytes of sparse (zeroed) journal skipped."), "{out}");
        assert!(
            out.contains("note: 8 bytes were unparseable — resynchronised"),
            "{out}"
        );
        assert!(out.contains("notes.txt"));
    }

    #[test]
    fn csv_has_a_stable_header_and_quotes_commas() {
        let t = ticks_2024();
        let bytes = v2(make_ref(7, 1), make_ref(5, 1), 8, t, R_FILE_CREATE, 0x20, "a,b.txt");
        let out = run_hex(&bytes, "csv");
        let mut lines = out.lines();
        assert_eq!(
            lines.next().unwrap(),
            "timestamp,usn,file_name,renamed_to,change,reasons,file_attributes,is_directory,\
             file_entry,file_sequence,parent_entry,parent_sequence,source_info,security_id,\
             version,offset"
        );
        assert_eq!(
            lines.next().unwrap(),
            "2024-05-01T12:00:00Z,8,\"a,b.txt\",,File create,FILE_CREATE,ARCHIVE,false,7,1,5,1,,256,2.0,0"
        );
    }

    #[test]
    fn bodyfile_and_tln_render_epoch_rows() {
        let j = sample_journal();
        let body = run_hex(&j, "bodyfile");
        assert!(
            body.starts_with("0|notes.txt (FILE_CREATE)|42|0|0|0|0|1714564800|1714564800|1714564800|1714564800\n"),
            "{body}"
        );
        let tln = run(&hex(&j), "hex", "create", "all", "", true, "tln", "WS01", "usn", 0).unwrap();
        assert!(
            tln.starts_with("1714564800|USN|WS01|-|File create: notes.txt [FILE_CREATE]\n"),
            "{tln}"
        );
    }

    #[test]
    fn json_mode_reports_scan_accounting() {
        let out = run_hex(&sample_journal(), "json");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["records_parsed"], 6);
        assert_eq!(v["records_matched"], 5);
        assert_eq!(v["rename_pairs_merged"], 1);
        assert_eq!(v["v2_records"], 6);
        assert_eq!(v["records"][0]["file_name"], "notes.txt");
        assert_eq!(v["records"][0]["reason_flags"][0], "FILE_CREATE");
        assert_eq!(v["records"][2]["renamed_to"], "final.txt");
    }

    #[test]
    fn summary_reports_spans_and_rankings() {
        let out = run_hex(&sample_journal(), "summary");
        assert!(out.contains("  Records parsed         6"), "{out}");
        assert!(out.contains("  Records matched        5"), "{out}");
        assert!(out.contains("  Record versions        v2: 6, v3: 0, v4 range: 0"), "{out}");
        assert!(out.contains("  USN range              4096 (0x1000) .. 4536 (0x11B8)"), "{out}");
        assert!(
            out.contains("  Time range (UTC)       2024-05-01T12:00:00Z .. 2024-05-01T12:00:04Z"),
            "{out}"
        );
        assert!(out.contains("  Directories / files    1 / 4"), "{out}");
        assert!(out.contains("   1. notes.txt  (2 records)"), "{out}");
    }

    #[test]
    fn sort_orders_by_time_and_name() {
        let j = hex(&sample_journal());
        let by_name = run(&j, "hex", "all", "all", "", true, "list", "", "name", 0).unwrap();
        let first = by_name.lines().nth(2).unwrap();
        assert!(first.contains("draft.txt"), "{by_name}");
        let by_time = run(&j, "hex", "all", "all", "", true, "list", "", "time", 0).unwrap();
        let newest = by_time.lines().nth(2).unwrap();
        assert!(newest.contains("Reports"), "{by_time}");
    }

    #[test]
    fn max_entries_caps_and_says_so() {
        let j = hex(&sample_journal());
        let out = run(&j, "hex", "all", "all", "", true, "list", "", "usn", 2).unwrap();
        assert!(
            out.starts_with("$UsnJrnl:$J — 6 records parsed, 5 matched, 2 shown (capped at max_entries=2)\n"),
            "{out}"
        );
        // The cap is clamped, never rejected.
        let big = run(&j, "hex", "all", "all", "", true, "list", "", "usn", 999_999).unwrap();
        assert!(big.contains("5 matched, 5 shown"), "{big}");
    }

    #[test]
    fn base64_input_matches_hex_input() {
        let j = sample_journal();
        let via_hex = run_hex(&j, "list");
        let via_b64 = run(&B64.encode(&j), "base64", "all", "all", "", true, "list", "", "usn", 0)
            .unwrap();
        assert_eq!(via_hex, via_b64);
    }

    #[test]
    fn v4_range_records_are_counted_not_rendered() {
        let mut rec = vec![0u8; V4_HEADER];
        rec[0..4].copy_from_slice(&(V4_HEADER as u32).to_le_bytes());
        rec[4..6].copy_from_slice(&4u16.to_le_bytes());
        let mut b = sample_journal();
        b.extend(rec);
        let out = run_hex(&b, "report");
        assert!(
            out.contains("note: 1 record skipped: V4 range-tracking records"),
            "{out}"
        );
    }

    #[test]
    fn empty_input_is_rejected_with_guidance() {
        let err = run("   ", "hex", "all", "all", "", true, "report", "", "usn", 0).unwrap_err();
        assert!(err.starts_with("input is empty:"), "{err}");
    }

    #[test]
    fn all_zero_input_explains_the_sparse_head() {
        let err = run(&hex(&[0u8; 128]), "hex", "all", "all", "", true, "report", "", "usn", 0)
            .unwrap_err();
        assert!(err.starts_with("no USN records found in 128 bytes."), "{err}");
        assert!(err.contains("entirely zeroes"), "{err}");
    }

    #[test]
    fn non_journal_input_names_the_offending_length() {
        let err = run(&hex(&[0xFFu8; 128]), "hex", "all", "all", "", true, "report", "", "usn", 0)
            .unwrap_err();
        assert!(
            err.contains("The first non-zero byte is at offset 0, where the 4-byte record length reads 4294967295"),
            "{err}"
        );
    }

    #[test]
    fn short_input_is_rejected() {
        let err = run("00112233", "hex", "all", "all", "", true, "report", "", "usn", 0)
            .unwrap_err();
        assert!(err.starts_with("input is only 4 bytes;"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_rejected_by_name() {
        let j = hex(&sample_journal());
        assert!(run(&j, "utf8", "all", "all", "", true, "report", "", "usn", 0)
            .unwrap_err()
            .starts_with("invalid input_encoding \"utf8\""));
        assert!(run(&j, "hex", "moved", "all", "", true, "report", "", "usn", 0)
            .unwrap_err()
            .starts_with("invalid event \"moved\""));
        assert!(run(&j, "hex", "all", "links", "", true, "report", "", "usn", 0)
            .unwrap_err()
            .starts_with("invalid include \"links\""));
        assert!(run(&j, "hex", "all", "all", "", true, "xml", "", "usn", 0)
            .unwrap_err()
            .starts_with("invalid mode \"xml\""));
        assert!(run(&j, "hex", "all", "all", "", true, "report", "", "size", 0)
            .unwrap_err()
            .starts_with("invalid sort \"size\""));
    }

    #[test]
    fn odd_hex_and_stray_characters_are_rejected() {
        let err = run("012", "hex", "all", "all", "", true, "report", "", "usn", 0).unwrap_err();
        assert!(err.contains("odd count"), "{err}");
        let err = run("00 zz", "hex", "all", "all", "", true, "report", "", "usn", 0).unwrap_err();
        assert!(err.contains("unexpected character 'z'"), "{err}");
    }

    #[test]
    fn unknown_reason_bits_are_surfaced_not_dropped() {
        assert_eq!(
            flag_names(0x0000_0100 | 0x0200_0000, REASON_FLAGS),
            vec!["FILE_CREATE".to_string(), "UNKNOWN_0x02000000".to_string()]
        );
    }

    #[test]
    fn filetime_conversion_matches_known_instants() {
        assert_eq!(
            filetime_to_iso(ticks_2024()).as_deref(),
            Some("2024-05-01T12:00:00Z")
        );
        assert_eq!(filetime_to_iso(0), None);
        assert_eq!(filetime_to_iso(u64::MAX), None);
    }
}
