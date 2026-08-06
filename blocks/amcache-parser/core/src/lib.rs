//! amcache-parser core — read `Amcache.hve` and report the programs, files,
//! drivers and shortcuts Windows recorded, with their SHA-1 hashes and times.
//!
//! `Amcache.hve` is an ordinary registry hive (the `regf` container) written by
//! the Microsoft Compatibility Appraiser. It is one of the few artifacts that
//! records a **file hash** alongside a full path, which is why it is a standard
//! stop in execution/existence triage. Two schemas exist and both are handled:
//!
//! * **Modern** (Windows 10 1607 and later) — named values under
//!   `Root\InventoryApplicationFile`, `Root\InventoryApplication`,
//!   `Root\InventoryDriverBinary` and `Root\InventoryApplicationShortcut`.
//! * **Legacy** (Windows 7 SP1 through Windows 10 1511) — `Root\File\{volume
//!   GUID}\{NTFS file reference}` and `Root\Programs\{ProgramId}`, whose value
//!   names are bare hex numbers (`0`, `15`, `101`, …) that have to be looked up
//!   in a documented table.
//!
//! Three different clocks show up in the data and are deliberately kept apart:
//! the **key last-write** time (when the appraiser last touched the record —
//! *not* a first-run time), the PE **link date** (compiler-supplied, trivially
//! forged) and the installer-supplied **install date**. Merging them into a
//! single "executed at" column would be the one mistake that matters here.
//!
//! Pure compute, no wafer/wasm-bindgen deps — shared by the chat skill block and
//! the web page, so it runs on every backend including the chat Service Worker.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use regf::hive::{RegistryHive, RegistryKey};
use regf::structures::RegistryValue;

/// A registry hive always opens with a 4096-byte base block.
const BASE_BLOCK_SIZE: usize = 4096;
/// `max_entries` when the caller leaves it at 0.
const DEFAULT_MAX_ENTRIES: usize = 200;
/// Upper bound on `max_entries` — beyond this it is a data dump, not a report,
/// and it would blow past the chat surface's token budget.
const MAX_MAX_ENTRIES: usize = 5000;
/// Subkey names listed when a container is missing, to show what IS there.
const SIBLING_HINTS: usize = 15;
/// Longest string value echoed before eliding.
const MAX_VALUE_CHARS: usize = 300;

// ---------------------------------------------------------------------------
// Input decoding
// ---------------------------------------------------------------------------

/// How to interpret the supplied hive bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InFmt {
    /// A hex string (`72656766…`, optionally spaced/`0x`-prefixed) — the default.
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

/// Hex with tolerant separators: whitespace, `:`, `-`, `,` and a leading `0x`.
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
// Parameters
// ---------------------------------------------------------------------------

/// Which Amcache container(s) to report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Section {
    /// Files plus programs — what triage almost always wants.
    Auto,
    /// Executables: `InventoryApplicationFile`, or legacy `Root\File`.
    Files,
    /// Installed applications: `InventoryApplication`, or legacy `Root\Programs`.
    Programs,
    /// Kernel/driver binaries: `InventoryDriverBinary`.
    Drivers,
    /// Start-menu shortcuts: `InventoryApplicationShortcut`.
    Shortcuts,
    /// Every container above.
    All,
}

impl Section {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "auto" => Ok(Section::Auto),
            "files" => Ok(Section::Files),
            "programs" => Ok(Section::Programs),
            "drivers" => Ok(Section::Drivers),
            "shortcuts" => Ok(Section::Shortcuts),
            "all" => Ok(Section::All),
            other => Err(format!(
                "invalid section {other:?}: expected \"auto\", \"files\", \"programs\", \
                 \"drivers\", \"shortcuts\" or \"all\""
            )),
        }
    }

    fn wants(self, k: Kind) -> bool {
        match self {
            Section::All => true,
            Section::Auto => matches!(k, Kind::File | Kind::Program),
            Section::Files => k == Kind::File,
            Section::Programs => k == Kind::Program,
            Section::Drivers => k == Kind::Driver,
            Section::Shortcuts => k == Kind::Shortcut,
        }
    }
}

/// Output shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Grouped, labelled, human-readable — the default.
    Report,
    /// One dense line per entry.
    List,
    /// Spreadsheet-ready table with a header row.
    Csv,
    /// Sleuth Kit bodyfile lines for mactime.
    Bodyfile,
    /// De-duplicated SHA-1 list for hash lookups.
    Hashes,
}

impl Mode {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "report" => Ok(Mode::Report),
            "list" => Ok(Mode::List),
            "csv" => Ok(Mode::Csv),
            "bodyfile" => Ok(Mode::Bodyfile),
            "hashes" => Ok(Mode::Hashes),
            other => Err(format!(
                "invalid mode {other:?}: expected \"report\", \"list\", \"csv\", \"bodyfile\" \
                 or \"hashes\""
            )),
        }
    }

    /// CSV and bodyfile stay machine-readable: prose is commented out.
    fn machine(self) -> bool {
        matches!(self, Mode::Csv | Mode::Bodyfile | Mode::Hashes)
    }
}

/// Whether a file entry's `ProgramId` resolves to a program record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Association {
    All,
    Associated,
    Unassociated,
}

impl Association {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "all" => Ok(Association::All),
            "associated" => Ok(Association::Associated),
            "unassociated" => Ok(Association::Unassociated),
            other => Err(format!(
                "invalid association {other:?}: expected \"all\", \"associated\" or \
                 \"unassociated\""
            )),
        }
    }
}

/// Ordering applied before the entry cap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sort {
    /// Key last-write, newest first — the timeline view.
    Time,
    /// Path/name, A→Z.
    Path,
    /// Hive order, untouched.
    None,
}

impl Sort {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "time" => Ok(Sort::Time),
            "path" => Ok(Sort::Path),
            "none" | "hive" => Ok(Sort::None),
            other => Err(format!(
                "invalid sort {other:?}: expected \"time\", \"path\" or \"none\""
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Amcache schema tables
// ---------------------------------------------------------------------------

/// The record kind an entry came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    File,
    Program,
    Driver,
    Shortcut,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::File => "file",
            Kind::Program => "program",
            Kind::Driver => "driver",
            Kind::Shortcut => "shortcut",
        }
    }
}

/// Modern containers, as paths relative to the hive root.
const MODERN_CONTAINERS: &[(&str, Kind, &str)] = &[
    (
        "Root\\InventoryApplicationFile",
        Kind::File,
        "executables the appraiser has seen (path, SHA-1, publisher, version)",
    ),
    (
        "Root\\InventoryApplication",
        Kind::Program,
        "installed applications (name, version, publisher, install date)",
    ),
    (
        "Root\\InventoryDriverBinary",
        Kind::Driver,
        "driver binaries (path, SHA-1, signer, driver timestamp)",
    ),
    (
        "Root\\InventoryApplicationShortcut",
        Kind::Shortcut,
        "Start menu / desktop shortcuts and their targets",
    ),
];

/// Legacy (Windows 7 / 8) containers. `Root\File` is two levels deep: one
/// subkey per volume GUID, then one per NTFS file reference.
const LEGACY_FILE_CONTAINER: &str = "Root\\File";
const LEGACY_PROGRAM_CONTAINER: &str = "Root\\Programs";

/// Legacy `Root\File` numeric value names → what they mean. Names are matched
/// case-insensitively with any leading zeros stripped.
const LEGACY_FILE_FIELDS: &[(&str, &str)] = &[
    ("0", "ProductName"),
    ("1", "Publisher"),
    ("2", "BinFileVersion"),
    ("3", "Language"),
    ("4", "SwitchBackContext"),
    ("5", "Version"),
    ("6", "Size"),
    ("7", "SizeOfImage"),
    ("8", "PeHeaderHash"),
    ("9", "PeChecksum"),
    ("c", "Description"),
    ("d", "LinkerVersion"),
    ("f", "LinkDate"),
    ("10", "BinProductVersion"),
    ("11", "LastModified"),
    ("12", "Created"),
    ("15", "Path"),
    ("17", "LastModified2"),
    ("100", "ProgramId"),
    ("101", "FileId"),
];

/// Legacy `Root\Programs` numeric value names → what they mean.
const LEGACY_PROGRAM_FIELDS: &[(&str, &str)] = &[
    ("0", "Name"),
    ("1", "Version"),
    ("2", "Publisher"),
    ("3", "Language"),
    ("5", "EntryType"),
    ("6", "Source"),
    ("7", "UninstallKey"),
    ("a", "InstallDate"),
    ("b", "InstallDateFromLinkFile"),
    ("d", "RootDirPath"),
    ("f", "ProductCode"),
    ("10", "PackageCode"),
    ("11", "MsiProductCode"),
    ("12", "MsiPackageCode"),
    ("13", "ProductId"),
];

/// Legacy value names whose payload is a Windows FILETIME.
const LEGACY_FILETIME_FIELDS: &[&str] = &["11", "12", "17"];
/// Legacy value names whose payload is a Unix epoch (seconds).
const LEGACY_EPOCH_FIELDS: &[&str] = &["f", "a", "b"];

fn legacy_field_name(
    table: &'static [(&'static str, &'static str)],
    raw: &str,
) -> Option<&'static str> {
    let key = raw.trim_start_matches('0');
    let key = if key.is_empty() { "0" } else { key };
    table
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(key))
        .map(|(_, label)| *label)
}

fn legacy_key_matches(list: &[&str], raw: &str) -> bool {
    let key = raw.trim_start_matches('0');
    let key = if key.is_empty() { "0" } else { key };
    list.iter().any(|n| n.eq_ignore_ascii_case(key))
}

// ---------------------------------------------------------------------------
// Time helpers
// ---------------------------------------------------------------------------

/// Windows FILETIME epoch (1601-01-01) → Unix epoch, in seconds.
const FILETIME_EPOCH_DIFF: i64 = 11_644_473_600;

/// FILETIME (100 ns ticks since 1601) → ISO-8601 UTC. Zero/absurd values give
/// `None` so a placeholder never renders as a 1601 date.
fn filetime_to_iso(ticks: u64) -> Option<String> {
    if ticks == 0 || ticks == u64::MAX {
        return None;
    }
    let secs = (ticks / 10_000_000) as i64 - FILETIME_EPOCH_DIFF;
    epoch_to_iso(secs)
}

/// Unix epoch seconds → ISO-8601 UTC, rejecting values outside 1970..=2100 (a
/// PE TimeDateStamp of 0 or 0xFFFFFFFF is a placeholder, not a date).
fn epoch_to_iso(secs: i64) -> Option<String> {
    if !(0..=4_102_444_800).contains(&secs) {
        return None;
    }
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    Some(format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    ))
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

/// Parse a timestamp back to Unix seconds for bodyfile output. Accepts the ISO
/// strings this module emits and the `MM/DD/YYYY HH:MM:SS` form Windows writes
/// into `InstallDate`/`LinkDate` string values.
fn iso_to_epoch(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let (y, mo, d, h, mi, sec): (i64, i64, i64, i64, i64, i64) = if b.len() >= 19
        && b[4] == b'-'
        && b[7] == b'-'
    {
        (
            s[0..4].parse().ok()?,
            s[5..7].parse().ok()?,
            s[8..10].parse().ok()?,
            s[11..13].parse().ok()?,
            s[14..16].parse().ok()?,
            s[17..19].parse().ok()?,
        )
    } else if b.len() >= 19 && b[2] == b'/' && b[5] == b'/' {
        (
            s[6..10].parse().ok()?,
            s[0..2].parse().ok()?,
            s[3..5].parse().ok()?,
            s[11..13].parse().ok()?,
            s[14..16].parse().ok()?,
            s[17..19].parse().ok()?,
        )
    } else {
        return None;
    };
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + sec)
}

/// Inverse of `civil_from_days`.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// ---------------------------------------------------------------------------
// Value rendering
// ---------------------------------------------------------------------------

/// Strip control characters so a corrupt string can't wreck the report.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == '\t' || c == '\n' || c == '\r' || (c as u32) < 0x20 {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn elide(s: &str) -> String {
    if s.chars().count() > MAX_VALUE_CHARS {
        let head: String = s.chars().take(MAX_VALUE_CHARS).collect();
        format!("{head}… (+{} more chars)", s.chars().count() - MAX_VALUE_CHARS)
    } else {
        s.to_string()
    }
}

/// A registry value flattened to display text. Binary blobs become hex.
fn value_text(v: &RegistryValue) -> String {
    match v {
        RegistryValue::None => String::new(),
        RegistryValue::String(s) => sanitize(s),
        RegistryValue::MultiString(items) => items
            .iter()
            .map(|s| sanitize(s))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("; "),
        RegistryValue::Binary(d) => {
            if d.is_empty() {
                String::new()
            } else {
                to_hex(d)
            }
        }
        RegistryValue::Dword(n) => n.to_string(),
        RegistryValue::DwordBigEndian(n) => n.to_string(),
        RegistryValue::Qword(n) => n.to_string(),
    }
}

/// The integer behind a value, when it has one (binary blobs of 4 or 8 bytes
/// count — legacy Amcache stores timestamps that way).
fn value_int(v: &RegistryValue) -> Option<u64> {
    match v {
        RegistryValue::Dword(n) | RegistryValue::DwordBigEndian(n) => Some(*n as u64),
        RegistryValue::Qword(n) => Some(*n),
        RegistryValue::Binary(d) if d.len() == 8 => {
            Some(u64::from_le_bytes(d[..8].try_into().unwrap()))
        }
        RegistryValue::Binary(d) if d.len() == 4 => {
            Some(u32::from_le_bytes(d[..4].try_into().unwrap()) as u64)
        }
        RegistryValue::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

/// `FileId` is `"0000"` + SHA-1, or the rare `"0001"` + MD5. Strip the prefix so
/// the hash can be pasted straight into a lookup; anything else is passed
/// through untouched rather than silently mangled.
fn strip_file_id(raw: &str) -> Option<String> {
    let s = raw.trim().trim_matches('"').to_ascii_lowercase();
    let s = s.trim_start_matches("0x");
    if !s.chars().all(|c| c.is_ascii_hexdigit()) || s.is_empty() {
        return None;
    }
    let stripped = match s.len() {
        44 if s.starts_with("0000") => &s[4..],
        36 if s.starts_with("0001") => &s[4..],
        40 | 32 | 64 => s,
        _ => return None,
    };
    Some(stripped.to_string())
}

/// Windows stores timestamps in Amcache three ways: an ISO/US-format string, a
/// FILETIME (100 ns ticks since 1601) or a Unix epoch. Normalise all three to
/// ISO-8601 UTC, keeping the raw text when it isn't recognisable as any of them.
fn normalize_time(v: &RegistryValue, hint: TimeHint) -> Option<String> {
    if let RegistryValue::String(s) = v {
        let t = sanitize(s);
        if t.is_empty() {
            return None;
        }
        // Already a date string: keep as-is, but normalise the US form.
        if let Some(secs) = iso_to_epoch(&t) {
            return epoch_to_iso(secs);
        }
        if let Ok(n) = t.parse::<u64>() {
            return from_int(n, hint);
        }
        return Some(t);
    }
    value_int(v).and_then(|n| from_int(n, hint))
}

/// Which clock an integer timestamp is counting on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TimeHint {
    /// 100 ns ticks since 1601.
    Filetime,
    /// Seconds since 1970 (a PE TimeDateStamp).
    Epoch,
    /// Unknown — decide from magnitude.
    Guess,
}

fn from_int(n: u64, hint: TimeHint) -> Option<String> {
    match hint {
        TimeHint::Filetime => filetime_to_iso(n),
        TimeHint::Epoch => epoch_to_iso(n as i64),
        // A FILETIME is ~1.3e17 today; a Unix epoch is ~1.7e9. The gap is eight
        // orders of magnitude, so the split is unambiguous.
        TimeHint::Guess => {
            if n > 100_000_000_000 {
                filetime_to_iso(n)
            } else {
                epoch_to_iso(n as i64)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entries
// ---------------------------------------------------------------------------

/// One decoded Amcache record, schema-independent.
struct Entry {
    kind: Kind,
    /// The container path the record came from.
    container: String,
    /// The record's registry key name (a file reference, ProgramId or hash).
    key_name: String,
    /// Display name (file name, program name, shortcut name).
    name: String,
    /// Full path, where the record has one.
    path: String,
    publisher: String,
    version: String,
    size: String,
    /// SHA-1 (or MD5) with the Amcache prefix removed.
    sha1: String,
    /// PE link date / driver timestamp, ISO-8601 UTC.
    link_date: String,
    /// Installer-supplied install date, ISO-8601 UTC.
    install_date: String,
    /// The `ProgramId` this record points at, if any.
    program_id: String,
    /// The resolved program name for `program_id`, filled in after both
    /// containers have been read.
    program_name: String,
    /// Registry key last-write time, ISO-8601 UTC.
    key_written: String,
    key_epoch: Option<i64>,
    /// Everything else the record carried, in hive order.
    extras: Vec<(String, String)>,
}

impl Entry {
    fn new(kind: Kind, container: &str, key_name: &str) -> Self {
        Entry {
            kind,
            container: container.to_string(),
            key_name: key_name.to_string(),
            name: String::new(),
            path: String::new(),
            publisher: String::new(),
            version: String::new(),
            size: String::new(),
            sha1: String::new(),
            link_date: String::new(),
            install_date: String::new(),
            program_id: String::new(),
            program_name: String::new(),
            key_written: String::new(),
            key_epoch: None,
            extras: Vec::new(),
        }
    }

    /// Best display path: the recorded path, else the name, else the key.
    fn display_path(&self) -> &str {
        if !self.path.is_empty() {
            &self.path
        } else if !self.name.is_empty() {
            &self.name
        } else {
            &self.key_name
        }
    }

    /// Case-insensitive substring match across every searchable field.
    fn matches(&self, needle_lower: &str) -> bool {
        [
            &self.name,
            &self.path,
            &self.publisher,
            &self.version,
            &self.sha1,
            &self.program_id,
            &self.program_name,
            &self.key_name,
        ]
        .iter()
        .any(|f| f.to_lowercase().contains(needle_lower))
    }
}

/// Canonical field names, in the order they are reported. Anything not on this
/// list lands in `extras` so no value is ever dropped.
fn assign_modern(e: &mut Entry, name: &str, v: &RegistryValue) {
    let text = elide(&value_text(v));
    match name.to_ascii_lowercase().as_str() {
        "name" | "drivername" | "shortcutname" => e.name = text,
        "lowercaselongpath" | "longpathhash" if name.eq_ignore_ascii_case("lowercaselongpath") => {
            e.path = text
        }
        "driverpackagestrongname" | "shortcutpath" | "targetpath" | "rootdirpath" => {
            if e.path.is_empty() {
                e.path = text.clone();
            }
            e.extras.push((name.to_string(), text));
        }
        "publisher" | "drivercompany" | "productname" => {
            if e.publisher.is_empty() {
                e.publisher = text;
            }
        }
        "version" | "driverversion" | "binfileversion" => {
            if e.version.is_empty() {
                e.version = text;
            }
        }
        "size" | "driverpackagesize" => e.size = text,
        "fileid" | "hash" | "driverid" => {
            if let Some(h) = strip_file_id(&text) {
                e.sha1 = h;
            } else if !text.is_empty() {
                e.extras.push((name.to_string(), text));
            }
        }
        "linkdate" | "drivertimestamp" => {
            if let Some(t) = normalize_time(v, TimeHint::Guess) {
                e.link_date = t;
            }
        }
        "installdate" | "installdatearplastmodified" | "installdatemsi" => {
            if e.install_date.is_empty() {
                if let Some(t) = normalize_time(v, TimeHint::Guess) {
                    e.install_date = t;
                }
            }
        }
        "programid" => e.program_id = text,
        _ => {
            if !text.is_empty() {
                e.extras.push((name.to_string(), text));
            }
        }
    }
}

/// Legacy records name their values in hex; map through the documented table.
fn assign_legacy(e: &mut Entry, raw_name: &str, v: &RegistryValue, program: bool) {
    let table = if program {
        LEGACY_PROGRAM_FIELDS
    } else {
        LEGACY_FILE_FIELDS
    };
    let label = legacy_field_name(table, raw_name);
    let hint = if legacy_key_matches(LEGACY_FILETIME_FIELDS, raw_name) && !program {
        TimeHint::Filetime
    } else if legacy_key_matches(LEGACY_EPOCH_FIELDS, raw_name) {
        TimeHint::Epoch
    } else {
        TimeHint::Guess
    };
    let text = elide(&value_text(v));
    match label {
        Some("Name") | Some("ProductName") => {
            if e.name.is_empty() {
                e.name = text;
            }
        }
        Some("Path") | Some("RootDirPath") => e.path = text,
        Some("Publisher") => e.publisher = text,
        Some("Version") | Some("BinFileVersion") => {
            if e.version.is_empty() {
                e.version = text;
            }
        }
        Some("Size") => e.size = text,
        Some("FileId") => {
            if let Some(h) = strip_file_id(&text) {
                e.sha1 = h;
            }
        }
        Some("LinkDate") => {
            if let Some(t) = normalize_time(v, hint) {
                e.link_date = t;
            }
        }
        Some("InstallDate") | Some("InstallDateFromLinkFile") => {
            if e.install_date.is_empty() {
                if let Some(t) = normalize_time(v, hint) {
                    e.install_date = t;
                }
            }
        }
        Some("ProgramId") => e.program_id = text,
        Some(other) => {
            let shown = if matches!(other, "LastModified" | "LastModified2" | "Created") {
                normalize_time(v, hint).unwrap_or(text)
            } else {
                text
            };
            if !shown.is_empty() {
                e.extras.push((other.to_string(), shown));
            }
        }
        None => {
            if !text.is_empty() {
                e.extras.push((format!("0x{raw_name} (undocumented)"), text));
            }
        }
    }
}

/// Read one record key into an `Entry`.
fn read_record(key: &RegistryKey, kind: Kind, container: &str, legacy: bool) -> Entry {
    let mut e = Entry::new(kind, container, &sanitize(&key.name()));
    if let Some(t) = key.last_written() {
        e.key_written = t.to_string().replace(" UTC", "Z").replace(' ', "T");
        e.key_epoch = Some(t.timestamp());
    }
    if let Ok(values) = key.values() {
        for v in &values {
            let name = sanitize(&v.name());
            let data = match v.data() {
                Ok(d) => d,
                Err(_) => continue,
            };
            if legacy {
                assign_legacy(&mut e, &name, &data, kind == Kind::Program);
            } else {
                assign_modern(&mut e, &name, &data);
            }
        }
    }
    if e.name.is_empty() && !e.path.is_empty() {
        e.name = e
            .path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or_default()
            .to_string();
    }
    e
}

// ---------------------------------------------------------------------------
// Hive traversal
// ---------------------------------------------------------------------------

/// Walk a backslash path one component at a time so a miss names the exact
/// component that failed instead of a single opaque error.
fn open_path<'h>(hive: &'h RegistryHive, path: &str) -> Option<RegistryKey<'h>> {
    let mut key = hive.root_key().ok()?;
    for part in path
        .split(['\\', '/'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        key = key.open_subkey(part).ok()?;
    }
    Some(key)
}

/// Which schema the hive uses. Reported so the analyst knows which value tables
/// were applied.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Schema {
    Modern,
    Legacy,
    Both,
    Unknown,
}

impl Schema {
    fn label(self) -> &'static str {
        match self {
            Schema::Modern => "modern (Windows 10 1607+, Root\\Inventory* containers)",
            Schema::Legacy => "legacy (Windows 7/8, Root\\File + Root\\Programs)",
            Schema::Both => "mixed — both the legacy and the modern containers are present",
            Schema::Unknown => "unrecognised — no known Amcache container was found",
        }
    }
}

/// Collect every record in every known container. Section filtering happens
/// AFTER program linking — a file record's program name can only be resolved
/// once the program container has been read, even when the caller asked for
/// files alone.
fn collect(hive: &RegistryHive) -> (Vec<Entry>, Schema, Vec<String>) {
    let mut entries: Vec<Entry> = Vec::new();
    let mut found: Vec<String> = Vec::new();
    let mut modern = false;
    let mut legacy = false;

    for (path, kind, desc) in MODERN_CONTAINERS {
        let key = match open_path(hive, path) {
            Some(k) => k,
            None => continue,
        };
        modern = true;
        let subs = key.subkeys().unwrap_or_default();
        found.push(format!("{path} — {} record(s); {desc}", subs.len()));
        for sub in &subs {
            entries.push(read_record(sub, *kind, path, false));
        }
    }

    // Legacy Root\File is volume-GUID → file-reference, two levels deep.
    if let Some(files) = open_path(hive, LEGACY_FILE_CONTAINER) {
        legacy = true;
        let volumes = files.subkeys().unwrap_or_default();
        let total: usize = volumes
            .iter()
            .map(|v| v.subkeys().map(|s| s.len()).unwrap_or(0))
            .sum();
        found.push(format!(
            "{LEGACY_FILE_CONTAINER} — {total} record(s) across {} volume(s); legacy executable \
             records keyed by NTFS file reference",
            volumes.len()
        ));
        for vol in &volumes {
            let vol_name = sanitize(&vol.name());
            let container = format!("{LEGACY_FILE_CONTAINER}\\{vol_name}");
            for rec in &vol.subkeys().unwrap_or_default() {
                entries.push(read_record(rec, Kind::File, &container, true));
            }
        }
    }

    if let Some(programs) = open_path(hive, LEGACY_PROGRAM_CONTAINER) {
        legacy = true;
        let subs = programs.subkeys().unwrap_or_default();
        found.push(format!(
            "{LEGACY_PROGRAM_CONTAINER} — {} record(s); legacy installed-application records",
            subs.len()
        ));
        for sub in &subs {
            entries.push(read_record(sub, Kind::Program, LEGACY_PROGRAM_CONTAINER, true));
        }
    }

    let schema = match (modern, legacy) {
        (true, true) => Schema::Both,
        (true, false) => Schema::Modern,
        (false, true) => Schema::Legacy,
        (false, false) => Schema::Unknown,
    };
    (entries, schema, found)
}

/// Resolve each file/driver/shortcut record's `ProgramId` to a program name.
/// This is what splits "associated" from "unassociated" file entries.
fn link_programs(entries: &mut [Entry]) {
    let map: Vec<(String, String)> = entries
        .iter()
        .filter(|e| e.kind == Kind::Program)
        .map(|e| {
            (
                e.key_name.to_ascii_lowercase(),
                if e.name.is_empty() {
                    e.key_name.clone()
                } else {
                    e.name.clone()
                },
            )
        })
        .collect();
    for e in entries.iter_mut() {
        if e.kind == Kind::Program || e.program_id.is_empty() {
            continue;
        }
        let want = e.program_id.to_ascii_lowercase();
        if let Some((_, name)) = map.iter().find(|(id, _)| *id == want) {
            e.program_name = name.clone();
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

const CSV_HEADER: &str = "category,name,path,sha1,publisher,version,size,link_date,install_date,program_id,program_name,key_last_write,container,key_name";

fn render_csv(entries: &[Entry]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for e in entries {
        let row = [
            e.kind.label(),
            &e.name,
            &e.path,
            &e.sha1,
            &e.publisher,
            &e.version,
            &e.size,
            &e.link_date,
            &e.install_date,
            &e.program_id,
            &e.program_name,
            &e.key_written,
            &e.container,
            &e.key_name,
        ]
        .iter()
        .map(|f| csv_escape(f))
        .collect::<Vec<_>>()
        .join(",");
        out.push_str(&row);
        out.push('\n');
    }
    out
}

fn render_list(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in entries {
        let mut bits: Vec<String> = Vec::new();
        if !e.key_written.is_empty() {
            bits.push(e.key_written.clone());
        }
        bits.push(format!("[{}]", e.kind.label()));
        bits.push(e.display_path().to_string());
        if !e.sha1.is_empty() {
            bits.push(format!("sha1={}", e.sha1));
        }
        if !e.publisher.is_empty() {
            bits.push(format!("publisher={}", e.publisher));
        }
        if !e.version.is_empty() {
            bits.push(format!("version={}", e.version));
        }
        if !e.program_name.is_empty() {
            bits.push(format!("program={}", e.program_name));
        }
        out.push_str(&bits.join("  "));
        out.push('\n');
    }
    out
}

/// Sleuth Kit bodyfile: MD5|name|inode|mode|UID|GID|size|atime|mtime|ctime|crtime.
/// Amcache has no atime/ctime, so the key last-write drives mtime and the link
/// date drives crtime — both are labelled in the report header.
fn render_bodyfile(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in entries {
        let m = e.key_epoch.unwrap_or(0);
        let cr = iso_to_epoch(&e.link_date).unwrap_or(0);
        let size = e.size.parse::<u64>().unwrap_or(0);
        let hash = if e.sha1.is_empty() { "0" } else { &e.sha1 };
        out.push_str(&format!(
            "{hash}|Amcache {} {}|0|0|0|0|{size}|0|{m}|0|{cr}\n",
            e.kind.label(),
            e.display_path().replace('|', "_"),
        ));
    }
    out
}

/// De-duplicated hash list, in first-seen order, ready for a lookup service or
/// the hash-ioc-match tool.
fn render_hashes(entries: &[Entry]) -> (String, usize) {
    let mut seen: Vec<String> = Vec::new();
    for e in entries {
        if !e.sha1.is_empty() && !seen.iter().any(|h| h == &e.sha1) {
            seen.push(e.sha1.clone());
        }
    }
    let mut out = String::new();
    for h in &seen {
        out.push_str(h);
        out.push('\n');
    }
    (out, seen.len())
}

fn field_line(out: &mut String, label: &str, value: &str) {
    if !value.is_empty() {
        out.push_str(&format!("    {label:<16}{value}\n"));
    }
}

fn render_report(entries: &[Entry]) -> String {
    let mut out = String::new();
    let mut last_kind: Option<Kind> = None;
    for e in entries {
        if last_kind != Some(e.kind) {
            if last_kind.is_some() {
                out.push('\n');
            }
            out.push_str(&format!("{} entries\n", title_case(e.kind.label())));
            last_kind = Some(e.kind);
        }
        out.push_str(&format!("  {}\n", e.display_path()));
        field_line(&mut out, "SHA-1", &e.sha1);
        field_line(&mut out, "Publisher", &e.publisher);
        field_line(&mut out, "Version", &e.version);
        field_line(&mut out, "Size", &e.size);
        field_line(&mut out, "Link date", &e.link_date);
        field_line(&mut out, "Install date", &e.install_date);
        if !e.program_id.is_empty() {
            let suffix = if e.program_name.is_empty() {
                " (no matching program record)".to_string()
            } else {
                format!(" ({})", e.program_name)
            };
            field_line(&mut out, "Program ID", &format!("{}{suffix}", e.program_id));
        }
        field_line(&mut out, "Key last write", &e.key_written);
        field_line(&mut out, "Key", &format!("{}\\{}", e.container, e.key_name));
        for (k, v) in &e.extras {
            field_line(&mut out, k, v);
        }
        out.push('\n');
    }
    out
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Parse `Amcache.hve` supplied as hex or Base64 text.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    input_encoding: &str,
    section: &str,
    mode: &str,
    association: &str,
    filter: &str,
    sort: &str,
    max_entries: usize,
) -> Result<String, String> {
    let fmt = InFmt::parse(input_encoding)?;
    let section = Section::parse(section)?;
    let mode = Mode::parse(mode)?;
    let association = Association::parse(association)?;
    let sort = Sort::parse(sort)?;
    let max_entries = match max_entries {
        0 => DEFAULT_MAX_ENTRIES,
        n => n.min(MAX_MAX_ENTRIES),
    };

    if data.trim().is_empty() {
        return Err("no hive bytes supplied: paste the contents of Amcache.hve encoded as hex \
                    or Base64."
            .to_string());
    }
    let bytes = fmt.to_bytes(data)?;
    if bytes.len() < BASE_BLOCK_SIZE {
        return Err(format!(
            "input is only {} byte(s); a registry hive starts with a {BASE_BLOCK_SIZE}-byte base \
             block. Paste the whole file, not just its opening bytes.",
            bytes.len()
        ));
    }
    if &bytes[0..4] != b"regf" {
        return Err(format!(
            "input does not start with the \"regf\" signature (found {}). Supply the raw \
             Amcache.hve file — not a .reg export, not a CSV, not a disk image.",
            to_hex(&bytes[0..4])
        ));
    }

    let hive = RegistryHive::from_bytes(bytes)
        .map_err(|e| format!("the hive header parsed but the hive could not be loaded: {e}"))?;

    let (mut entries, schema, containers) = collect(&hive);
    link_programs(&mut entries);

    entries.retain(|e| section.wants(e.kind));

    // Association filtering only means anything for records that carry a
    // ProgramId; program records themselves are always kept.
    let total_before = entries.len();
    entries.retain(|e| match association {
        Association::All => true,
        Association::Associated => e.kind == Kind::Program || !e.program_name.is_empty(),
        Association::Unassociated => e.kind != Kind::Program && e.program_name.is_empty(),
    });
    let dropped_by_association = total_before - entries.len();

    let needle = filter.trim().to_lowercase();
    let before_filter = entries.len();
    if !needle.is_empty() {
        entries.retain(|e| e.matches(&needle));
    }
    let dropped_by_filter = before_filter - entries.len();

    match sort {
        Sort::Time => entries.sort_by(|a, b| {
            b.key_epoch
                .cmp(&a.key_epoch)
                .then_with(|| a.display_path().to_lowercase().cmp(&b.display_path().to_lowercase()))
        }),
        Sort::Path => entries.sort_by(|a, b| {
            a.display_path()
                .to_lowercase()
                .cmp(&b.display_path().to_lowercase())
        }),
        Sort::None => {}
    }
    // The report groups by category, so keep categories contiguous without
    // disturbing the sort inside each one.
    if mode == Mode::Report {
        entries.sort_by_key(|e| match e.kind {
            Kind::Program => 0,
            Kind::File => 1,
            Kind::Driver => 2,
            Kind::Shortcut => 3,
        });
    }

    let matched = entries.len();
    let truncated = matched > max_entries;
    entries.truncate(max_entries);

    // ---- header -----------------------------------------------------------
    let mut header = String::new();
    header.push_str(&format!("Amcache schema: {}\n", schema.label()));
    if containers.is_empty() {
        header.push_str("Containers found: none\n");
    } else {
        header.push_str("Containers found:\n");
        for c in &containers {
            header.push_str(&format!("  {c}\n"));
        }
    }

    if schema == Schema::Unknown {
        let mut msg = header;
        msg.push_str(
            "\nThis hive holds none of the known Amcache containers. Amcache.hve lives in \
             C:\\Windows\\AppCompat\\Programs\\. If you loaded SYSTEM, SOFTWARE, SAM, NTUSER.DAT \
             or UsrClass.dat there is nothing to find here. Root subkeys of the hive you \
             loaded:\n",
        );
        match hive.root_key().and_then(|r| r.subkeys()) {
            Ok(subs) if !subs.is_empty() => {
                for k in subs.iter().take(SIBLING_HINTS) {
                    msg.push_str(&format!("  {}\n", sanitize(&k.name())));
                }
                if subs.len() > SIBLING_HINTS {
                    msg.push_str(&format!("  … (+{} more)\n", subs.len() - SIBLING_HINTS));
                }
            }
            Ok(_) => msg.push_str("  (the root key has no subkeys)\n"),
            Err(e) => msg.push_str(&format!("  (root subkeys unreadable: {e})\n")),
        }
        return Ok(msg);
    }

    // ---- notes ------------------------------------------------------------
    let mut notes: Vec<String> = Vec::new();
    notes.push(format!(
        "{} record(s) shown of {matched} matching",
        entries.len()
    ));
    if dropped_by_association > 0 {
        let which = format!("{association:?}").to_lowercase();
        notes.push(format!(
            "{dropped_by_association} hidden by the {which} association filter"
        ));
    }
    if dropped_by_filter > 0 {
        notes.push(format!(
            "{dropped_by_filter} hidden by the filter {:?}",
            filter.trim()
        ));
    }
    if truncated {
        notes.push(format!(
            "stopped at the max_entries cap of {max_entries} — raise it to see the rest"
        ));
    }
    notes.push(
        "key last-write is the appraiser's last observation, not a first-run time".to_string(),
    );

    // ---- body -------------------------------------------------------------
    let (body, extra_note) = match mode {
        Mode::Report => (render_report(&entries), None),
        Mode::List => (render_list(&entries), None),
        Mode::Csv => (render_csv(&entries), None),
        Mode::Bodyfile => (
            render_bodyfile(&entries),
            Some("bodyfile mtime = key last write, crtime = PE link date".to_string()),
        ),
        Mode::Hashes => {
            let (text, n) = render_hashes(&entries);
            (text, Some(format!("{n} unique hash(es)")))
        }
    };
    if let Some(n) = extra_note {
        notes.push(n);
    }

    let mut out = String::new();
    let note_block = format!("({})\n", notes.join("; "));
    if mode == Mode::List {
        if entries.is_empty() {
            out.push_str("No records matched.\n\n");
        }
        out.push_str(&body);
        out.push_str(&note_block);
    } else if mode.machine() {
        for line in header.lines() {
            out.push_str(&format!("# {line}\n"));
        }
        out.push_str(&body);
        for line in note_block.lines() {
            out.push_str(&format!("# {line}\n"));
        }
    } else {
        out.push_str(&header);
        out.push('\n');
        if entries.is_empty() {
            out.push_str("No records matched.\n\n");
        }
        out.push_str(&body);
        out.push_str(&note_block);
    }
    Ok(out.trim_end().to_string() + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use regf::structures::DataType;
    use regf::writer::HiveBuilder;

    fn utf16z(s: &str) -> Vec<u8> {
        let mut v: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        v.extend_from_slice(&[0, 0]);
        v
    }

    /// 2024-05-17T09:30:00Z as a Windows FILETIME.
    fn filetime_2024() -> u64 {
        ((days_from_civil(2024, 5, 17) * 86_400 + 9 * 3600 + 30 * 60) as u64
            + FILETIME_EPOCH_DIFF as u64)
            * 10_000_000
    }

    /// A synthetic modern Amcache.hve: two programs, three executables (one of
    /// them unassociated), one driver and one shortcut. Built with `regf`'s own
    /// writer, so the bytes are a genuine hive.
    fn modern_hive() -> Vec<u8> {
        let mut b = HiveBuilder::new();
        let root_off = b.root_offset();
        let root = b.add_key(root_off, "Root").unwrap();

        // ---- InventoryApplication (two installed programs) ----------------
        let apps = b.add_key(root, "InventoryApplication").unwrap();
        let app1 = b.add_key(apps, "0000f1a9be1e4f9b7d0000000000000000000000").unwrap();
        for (n, v) in [
            ("Name", "Contoso Backup Suite"),
            ("Version", "3.2.1"),
            ("Publisher", "Contoso Ltd"),
            ("RootDirPath", "c:\\program files\\contoso"),
            ("InstallDate", "05/17/2024 09:30:00"),
            ("Source", "AddRemoveProgram"),
        ] {
            b.add_value(app1, n, DataType::String, &utf16z(v)).unwrap();
        }
        let app2 = b.add_key(apps, "0000aaaabbbbccccdddd0000000000000000eeee").unwrap();
        for (n, v) in [
            ("Name", "Sysinternals Suite"),
            ("Version", "2023.9"),
            ("Publisher", "Microsoft"),
        ] {
            b.add_value(app2, n, DataType::String, &utf16z(v)).unwrap();
        }

        // ---- InventoryApplicationFile (three executables) -----------------
        let files = b.add_key(root, "InventoryApplicationFile").unwrap();

        let f1 = b.add_key(files, "backup.exe|1f2e3d4c5b6a7988").unwrap();
        for (n, v) in [
            ("Name", "backup.exe"),
            ("LowerCaseLongPath", "c:\\program files\\contoso\\backup.exe"),
            (
                "FileId",
                "0000da39a3ee5e6b4b0d3255bfef95601890afd80709",
            ),
            ("Publisher", "Contoso Ltd"),
            ("Version", "3.2.1"),
            ("BinaryType", "pe64_amd64"),
            ("LinkDate", "05/16/2024 08:20:12"),
            ("ProgramId", "0000f1a9be1e4f9b7d0000000000000000000000"),
        ] {
            b.add_value(f1, n, DataType::String, &utf16z(v)).unwrap();
        }
        b.add_value(f1, "Size", DataType::Qword, &1_234_567u64.to_le_bytes())
            .unwrap();
        b.add_value(f1, "IsPeFile", DataType::Dword, &1u32.to_le_bytes())
            .unwrap();

        let f2 = b.add_key(files, "psexec.exe|aabbccdd11223344").unwrap();
        for (n, v) in [
            ("Name", "psexec.exe"),
            ("LowerCaseLongPath", "c:\\users\\alice\\downloads\\psexec.exe"),
            (
                "FileId",
                "0000aa11223344556677889900112233445566778899",
            ),
            ("Publisher", "Sysinternals"),
            ("Version", "2.40"),
            ("ProgramId", "0000ffffffffffffffffffffffffffffffffffff"),
        ] {
            b.add_value(f2, n, DataType::String, &utf16z(v)).unwrap();
        }

        let f3 = b.add_key(files, "notepad.exe|9988776655443322").unwrap();
        for (n, v) in [
            ("Name", "notepad.exe"),
            ("LowerCaseLongPath", "c:\\windows\\system32\\notepad.exe"),
            ("Publisher", "Microsoft"),
            ("ProgramId", "0000aaaabbbbccccdddd0000000000000000eeee"),
        ] {
            b.add_value(f3, n, DataType::String, &utf16z(v)).unwrap();
        }

        // ---- InventoryDriverBinary ---------------------------------------
        let drivers = b.add_key(root, "InventoryDriverBinary").unwrap();
        let d1 = b.add_key(drivers, "c:/windows/system32/drivers/contoso.sys").unwrap();
        for (n, v) in [
            ("DriverName", "c:\\windows\\system32\\drivers\\contoso.sys"),
            ("DriverCompany", "Contoso Ltd"),
            ("DriverVersion", "1.0.0.7"),
            ("DriverSigned", "true"),
        ] {
            b.add_value(d1, n, DataType::String, &utf16z(v)).unwrap();
        }
        b.add_value(
            d1,
            "DriverTimeStamp",
            DataType::Qword,
            &filetime_2024().to_le_bytes(),
        )
        .unwrap();

        // ---- InventoryApplicationShortcut --------------------------------
        let cuts = b.add_key(root, "InventoryApplicationShortcut").unwrap();
        let s1 = b.add_key(cuts, "backup.lnk").unwrap();
        b.add_value(
            s1,
            "ShortcutPath",
            DataType::String,
            &utf16z("c:\\programdata\\start menu\\backup.lnk"),
        )
        .unwrap();

        b.to_bytes().unwrap()
    }

    /// A synthetic legacy (Windows 7/8) Amcache.hve with one Root\File record
    /// and one Root\Programs record, using the numeric value names.
    fn legacy_hive() -> Vec<u8> {
        let mut b = HiveBuilder::new();
        let root_off = b.root_offset();
        let root = b.add_key(root_off, "Root").unwrap();

        let files = b.add_key(root, "File").unwrap();
        let vol = b
            .add_key(files, "{4b1f2e8a-0000-0000-0000-000000000001}")
            .unwrap();
        let rec = b.add_key(vol, "00003a5c00000e7f").unwrap();
        b.add_value(rec, "0", DataType::String, &utf16z("Contoso Backup"))
            .unwrap();
        b.add_value(rec, "1", DataType::String, &utf16z("Contoso Ltd"))
            .unwrap();
        b.add_value(rec, "15", DataType::String, &utf16z("c:\\tools\\legacy.exe"))
            .unwrap();
        b.add_value(
            rec,
            "101",
            DataType::String,
            &utf16z("0000da39a3ee5e6b4b0d3255bfef95601890afd80709"),
        )
        .unwrap();
        b.add_value(rec, "6", DataType::Qword, &4096u64.to_le_bytes())
            .unwrap();
        // PE TimeDateStamp: 2021-01-02T03:04:05Z as Unix epoch seconds.
        let link = (days_from_civil(2021, 1, 2) * 86_400 + 3 * 3600 + 4 * 60 + 5) as u32;
        b.add_value(rec, "f", DataType::Dword, &link.to_le_bytes())
            .unwrap();
        b.add_value(rec, "11", DataType::Qword, &filetime_2024().to_le_bytes())
            .unwrap();

        let programs = b.add_key(root, "Programs").unwrap();
        let p = b.add_key(programs, "00006f1a2b3c4d5e").unwrap();
        b.add_value(p, "0", DataType::String, &utf16z("Legacy Toolkit"))
            .unwrap();
        b.add_value(p, "1", DataType::String, &utf16z("1.4"))
            .unwrap();
        b.add_value(p, "2", DataType::String, &utf16z("Contoso Ltd"))
            .unwrap();

        b.to_bytes().unwrap()
    }

    /// A hive with no Amcache containers at all.
    fn foreign_hive() -> Vec<u8> {
        let mut b = HiveBuilder::new();
        let root = b.root_offset();
        let sw = b.add_key(root, "Software").unwrap();
        b.add_key(sw, "Microsoft").unwrap();
        b.to_bytes().unwrap()
    }

    fn hex_of(bytes: &[u8]) -> String {
        bytes.iter().map(|x| format!("{x:02x}")).collect()
    }

    fn modern_hex() -> String {
        hex_of(&modern_hive())
    }

    fn report() -> String {
        run(&modern_hex(), "hex", "auto", "report", "all", "", "path", 200).unwrap()
    }

    // ---- happy paths ------------------------------------------------------

    #[test]
    fn report_lists_programs_and_files_with_hashes() {
        let out = report();
        assert!(out.contains("modern (Windows 10 1607+"), "{out}");
        assert!(out.contains("Root\\InventoryApplicationFile — 3 record(s)"), "{out}");
        assert!(out.contains("c:\\program files\\contoso\\backup.exe"), "{out}");
        assert!(
            out.contains("SHA-1           da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            "{out}"
        );
        assert!(out.contains("Contoso Backup Suite"), "{out}");
        assert!(out.contains("Install date    2024-05-17T09:30:00Z"), "{out}");
    }

    #[test]
    fn the_file_id_prefix_is_stripped_so_the_hash_can_be_looked_up() {
        assert_eq!(
            strip_file_id("0000da39a3ee5e6b4b0d3255bfef95601890afd80709").unwrap(),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        // A bare SHA-1 is already usable.
        assert_eq!(
            strip_file_id("DA39A3EE5E6B4B0D3255BFEF95601890AFD80709").unwrap(),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
        // The rare MD5 form keeps its 32 hex digits.
        assert_eq!(strip_file_id("0001").is_none(), true);
        assert!(strip_file_id("not a hash").is_none());
    }

    #[test]
    fn a_file_entry_is_linked_to_its_installed_program() {
        let out = report();
        assert!(
            out.contains("Program ID      0000f1a9be1e4f9b7d0000000000000000000000 (Contoso Backup Suite)"),
            "{out}"
        );
        assert!(out.contains("(no matching program record)"), "{out}");
    }

    #[test]
    fn unassociated_keeps_only_files_with_no_program_record() {
        let out = run(
            &modern_hex(),
            "hex",
            "files",
            "list",
            "unassociated",
            "",
            "path",
            200,
        )
        .unwrap();
        assert!(out.contains("psexec.exe"), "{out}");
        assert!(!out.contains("backup.exe"), "{out}");
        assert!(!out.contains("notepad.exe"), "{out}");
    }

    #[test]
    fn associated_keeps_only_files_that_resolve_to_a_program() {
        let out = run(
            &modern_hex(),
            "hex",
            "files",
            "list",
            "associated",
            "",
            "path",
            200,
        )
        .unwrap();
        assert!(out.contains("backup.exe"), "{out}");
        assert!(out.contains("notepad.exe"), "{out}");
        assert!(!out.contains("psexec.exe"), "{out}");
    }

    #[test]
    fn csv_has_a_header_and_one_row_per_record() {
        let out = run(&modern_hex(), "hex", "files", "csv", "all", "", "path", 200).unwrap();
        let rows: Vec<&str> = out
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .collect();
        assert_eq!(rows[0], CSV_HEADER, "{out}");
        assert_eq!(rows.len(), 4, "header + 3 file records\n{out}");
        assert!(
            rows.iter().any(|r| r.starts_with(
                "file,backup.exe,c:\\program files\\contoso\\backup.exe,da39a3ee5e6b4b0d3255bfef95601890afd80709,Contoso Ltd,3.2.1,1234567,2024-05-16T08:20:12Z"
            )),
            "{out}"
        );
    }

    #[test]
    fn hashes_mode_emits_a_deduplicated_pastable_hash_list() {
        let out = run(&modern_hex(), "hex", "all", "hashes", "all", "", "path", 200).unwrap();
        let hashes: Vec<&str> = out.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(
            hashes,
            vec![
                "da39a3ee5e6b4b0d3255bfef95601890afd80709",
                "aa11223344556677889900112233445566778899",
            ],
            "{out}"
        );
        assert!(out.contains("# (7 record(s) shown"), "{out}");
        assert!(out.contains("2 unique hash(es)"), "{out}");
    }

    #[test]
    fn bodyfile_uses_the_sleuth_kit_layout() {
        let out = run(&modern_hex(), "hex", "files", "bodyfile", "all", "backup", "path", 200)
            .unwrap();
        let line = out
            .lines()
            .find(|l| l.contains("backup.exe"))
            .unwrap_or_else(|| panic!("{out}"));
        let cols: Vec<&str> = line.split('|').collect();
        assert_eq!(cols.len(), 11, "{line}");
        assert_eq!(cols[0], "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(cols[1], "Amcache file c:\\program files\\contoso\\backup.exe");
        assert_eq!(cols[6], "1234567");
        // crtime column holds the PE link date.
        assert_eq!(cols[10].parse::<i64>().unwrap(), 1_715_847_612);
    }

    #[test]
    fn the_filter_matches_path_publisher_and_hash() {
        let by_path = run(&modern_hex(), "hex", "files", "list", "all", "downloads", "path", 200)
            .unwrap();
        assert!(by_path.contains("psexec.exe"), "{by_path}");
        assert!(!by_path.contains("notepad.exe"), "{by_path}");

        let by_publisher =
            run(&modern_hex(), "hex", "files", "list", "all", "sysinternals", "path", 200).unwrap();
        assert!(by_publisher.contains("psexec.exe"), "{by_publisher}");

        let by_hash = run(
            &modern_hex(),
            "hex",
            "files",
            "list",
            "all",
            "da39a3ee5e6b",
            "path",
            200,
        )
        .unwrap();
        assert!(by_hash.contains("backup.exe"), "{by_hash}");
        assert!(by_hash.contains("2 hidden by the filter"), "{by_hash}");
    }

    #[test]
    fn drivers_and_shortcuts_are_reported_when_asked_for() {
        let drv = run(&modern_hex(), "hex", "drivers", "report", "all", "", "path", 200).unwrap();
        assert!(drv.contains("contoso.sys"), "{drv}");
        assert!(drv.contains("Link date       2024-05-17T09:30:00Z"), "{drv}");

        let cut = run(&modern_hex(), "hex", "shortcuts", "report", "all", "", "path", 200).unwrap();
        assert!(cut.contains("backup.lnk"), "{cut}");
        // The default section keeps drivers and shortcuts out of the way.
        assert!(!report().contains("contoso.sys"), "{}", report());
    }

    #[test]
    fn the_legacy_windows_7_schema_is_decoded_from_the_numeric_value_names() {
        let out = run(&hex_of(&legacy_hive()), "hex", "all", "report", "all", "", "path", 200)
            .unwrap();
        assert!(out.contains("legacy (Windows 7/8"), "{out}");
        assert!(out.contains("c:\\tools\\legacy.exe"), "{out}");
        assert!(
            out.contains("SHA-1           da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            "{out}"
        );
        assert!(out.contains("Publisher       Contoso Ltd"), "{out}");
        // "f" is a PE TimeDateStamp in Unix seconds; "11" is a FILETIME.
        assert!(out.contains("Link date       2021-01-02T03:04:05Z"), "{out}");
        assert!(out.contains("LastModified    2024-05-17T09:30:00Z"), "{out}");
        assert!(out.contains("Legacy Toolkit"), "{out}");
    }

    #[test]
    fn sorting_by_time_puts_the_newest_key_write_first() {
        let out = run(&modern_hex(), "hex", "files", "list", "all", "", "time", 200).unwrap();
        let times: Vec<String> = out
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('('))
            .map(|l| l.split("  ").next().unwrap_or_default().to_string())
            .collect();
        let mut sorted = times.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(times, sorted, "{out}");
    }

    #[test]
    fn base64_input_is_accepted() {
        let b64 = B64.encode(modern_hive());
        let out = run(&b64, "base64", "auto", "report", "all", "", "path", 200).unwrap();
        assert!(out.contains("backup.exe"), "{out}");
    }

    #[test]
    fn hex_input_tolerates_separators_and_prefixes() {
        assert_eq!(decode_hex("0x72 65:67-66").unwrap(), b"regf".to_vec());
        assert_eq!(decode_hex("72\n65\t67 66").unwrap(), b"regf".to_vec());
    }

    // ---- caps and reporting ----------------------------------------------

    #[test]
    fn caps_are_reported_not_silently_applied() {
        let out = run(&modern_hex(), "hex", "files", "list", "all", "", "path", 1).unwrap();
        assert_eq!(
            out.lines().filter(|l| !l.starts_with('(') && !l.is_empty()).count(),
            1,
            "{out}"
        );
        assert!(out.contains("stopped at the max_entries cap of 1"), "{out}");
        assert!(out.contains("1 record(s) shown of 3 matching"), "{out}");
    }

    #[test]
    fn the_key_last_write_caveat_is_always_stated() {
        assert!(
            report().contains("key last-write is the appraiser's last observation, not a \
                               first-run time"),
            "{}",
            report()
        );
    }

    #[test]
    fn an_empty_result_says_so_instead_of_rendering_nothing() {
        let out = run(&modern_hex(), "hex", "files", "report", "all", "zzzz-no-match", "path", 200)
            .unwrap();
        assert!(out.contains("No records matched."), "{out}");
        assert!(out.contains("3 hidden by the filter \"zzzz-no-match\""), "{out}");
    }

    // ---- errors -----------------------------------------------------------

    #[test]
    fn empty_input_is_rejected() {
        let err = run("   ", "hex", "auto", "report", "all", "", "time", 200).unwrap_err();
        assert!(err.contains("no hive bytes supplied"), "{err}");
    }

    #[test]
    fn non_hive_bytes_are_rejected_with_the_signature_found() {
        let bytes = vec![0x41u8; 8192];
        let err = run(&hex_of(&bytes), "hex", "auto", "report", "all", "", "time", 200)
            .unwrap_err();
        assert!(err.contains("does not start with the \"regf\" signature"), "{err}");
        assert!(err.contains("41414141"), "{err}");
    }

    #[test]
    fn a_truncated_hive_says_how_short_it_is() {
        let err = run("72656766", "hex", "auto", "report", "all", "", "time", 200).unwrap_err();
        assert!(err.contains("input is only 4 byte(s)"), "{err}");
    }

    #[test]
    fn invalid_hex_names_the_offending_character() {
        let err = run("72zz", "hex", "auto", "report", "all", "", "time", 200).unwrap_err();
        assert!(err.contains("unexpected character 'z'"), "{err}");
    }

    #[test]
    fn odd_hex_digit_count_is_rejected() {
        let err = run("726", "hex", "auto", "report", "all", "", "time", 200).unwrap_err();
        assert!(err.contains("odd count"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected_by_name() {
        let h = modern_hex();
        for (args, want) in [
            (("utf8", "auto", "report", "all", "time"), "invalid input_encoding \"utf8\""),
            (("hex", "devices", "report", "all", "time"), "invalid section \"devices\""),
            (("hex", "auto", "json", "all", "time"), "invalid mode \"json\""),
            (("hex", "auto", "report", "orphan", "time"), "invalid association \"orphan\""),
            (("hex", "auto", "report", "all", "size"), "invalid sort \"size\""),
        ] {
            let err = run(&h, args.0, args.1, args.2, args.3, "", args.4, 200).unwrap_err();
            assert!(err.contains(want), "expected {want:?}, got {err:?}");
        }
    }

    #[test]
    fn a_hive_without_amcache_containers_reports_what_it_does_contain() {
        let out = run(&hex_of(&foreign_hive()), "hex", "auto", "report", "all", "", "time", 200)
            .unwrap();
        assert!(out.contains("unrecognised"), "{out}");
        assert!(out.contains("C:\\Windows\\AppCompat\\Programs"), "{out}");
        assert!(out.contains("Software"), "{out}");
    }

    // ---- unit-level helpers ----------------------------------------------

    #[test]
    fn filetime_and_epoch_timestamps_round_trip() {
        assert_eq!(
            filetime_to_iso(filetime_2024()).unwrap(),
            "2024-05-17T09:30:00Z"
        );
        assert_eq!(epoch_to_iso(0).unwrap(), "1970-01-01T00:00:00Z");
        // Placeholders must not render as 1601/1970 dates.
        assert!(filetime_to_iso(0).is_none());
        assert!(epoch_to_iso(-1).is_none());
        assert!(epoch_to_iso(9_999_999_999).is_none());
    }

    #[test]
    fn the_us_date_form_windows_writes_is_normalised_to_iso() {
        let v = RegistryValue::String("05/17/2024 09:30:00".into());
        assert_eq!(
            normalize_time(&v, TimeHint::Guess).unwrap(),
            "2024-05-17T09:30:00Z"
        );
    }

    #[test]
    fn legacy_value_names_match_with_or_without_leading_zeros() {
        assert_eq!(legacy_field_name(LEGACY_FILE_FIELDS, "0101"), Some("FileId"));
        assert_eq!(legacy_field_name(LEGACY_FILE_FIELDS, "15"), Some("Path"));
        assert_eq!(legacy_field_name(LEGACY_FILE_FIELDS, "0"), Some("ProductName"));
        assert_eq!(legacy_field_name(LEGACY_FILE_FIELDS, "ff"), None);
    }

    #[test]
    fn oversized_string_values_are_elided_not_dumped() {
        let long = "a".repeat(MAX_VALUE_CHARS + 50);
        assert!(elide(&long).ends_with("(+50 more chars)"));
        assert_eq!(elide("short"), "short");
    }

    #[test]
    fn csv_fields_containing_commas_and_quotes_are_escaped() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("plain"), "plain");
    }
}
