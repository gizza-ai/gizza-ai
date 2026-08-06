//! shellbags-parser core — reconstruct the folders a Windows user browsed from
//! the `BagMRU` shellbag tree inside an offline registry hive.
//!
//! ## What a shellbag is
//!
//! Explorer remembers per-folder view preferences (window size, sort column,
//! icon size). To do that it has to remember *which folder* the preference
//! belongs to, so it writes a tree of keys under `…\Shell\BagMRU`. Every node in
//! that tree carries:
//!
//!   * numbered values `0`, `1`, `2`, … — each one a raw **shell item**
//!     (`SHITEMID`) blob naming one child folder;
//!   * a subkey per numbered value, holding that child's own children;
//!   * `MRUListEx` — the order the children were last interacted with, most
//!     recent first;
//!   * `NodeSlot` — the bag number whose `Bags\<slot>\Shell` key holds the
//!     actual view preferences.
//!
//! Walking that tree and decoding each shell item rebuilds absolute paths for
//! folders the user opened — **including folders that no longer exist**, folders
//! on removable media that is long gone, and network shares. Reconstructed paths
//! keep the shell-namespace root they were reached through (`This PC\C:\Users\…`),
//! matching the convention of the established shellbag parsers: the same
//! directory reached via a library, a mapped drive and the volume itself really
//! are three different bags.
//!
//! ## Shell item decoding
//!
//! A shell item is `size: u16`, `class: u8`, then class-specific data
//! (libyal's `libfwsi` format documentation is the reference used here):
//!
//! | class        | meaning                              |
//! |--------------|--------------------------------------|
//! | `0x1F`       | root/GUID folder (This PC, Network…) |
//! | `0x20`–`0x2F`| volume (`C:\`)                       |
//! | `0x30`–`0x3F`| file entry (bit 0 set = directory)   |
//! | `0x40`–`0x4F`| network location (`\\server\share`)  |
//! | `0x61`       | URI                                  |
//! | `0x71`       | control-panel item                   |
//! | `0x74`       | delegate folder (`CFSF`-signed)      |
//!
//! File entries put the file size at `+4`, a DOS/FAT modification date-time at
//! `+8`, attribute flags at `+12` and the primary (often 8.3) name at `+14`. The
//! nicer long name and the creation/access times live in an optional extension
//! block signed `0xBEEF0004`, which also carries the NTFS file reference (MFT
//! entry + sequence) from version 7 onwards. The block is located by scanning
//! for its signature rather than by trusting a computed offset, because the
//! primary name's alignment padding varies in the wild.
//!
//! Pure compute, no wafer/wasm-bindgen deps — shared by the chat skill block and
//! the web page. Runs on every backend including the chat Service Worker.

use base64::engine::general_purpose::STANDARD as B64;
use base64::engine::general_purpose::STANDARD_NO_PAD as B64_NO_PAD;
use base64::Engine;
use regf::hive::{RegistryHive, RegistryKey};

/// `max_entries` when the caller leaves it at 0.
const DEFAULT_MAX_ENTRIES: usize = 200;
/// Upper bound on `max_entries` — beyond this it is a data dump, not a report.
const MAX_MAX_ENTRIES: usize = 5000;
/// `max_depth` when the caller leaves it at 0.
const DEFAULT_MAX_DEPTH: usize = 32;
/// Upper bound on `max_depth`. Real shellbag trees rarely pass ~20 levels.
const MAX_MAX_DEPTH: usize = 64;
/// Bytes of a shell item shown in `raw` mode before eliding.
const RAW_PREVIEW_BYTES: usize = 96;
/// Sibling key names listed when a shellbag root is missing.
const SIBLING_HINTS: usize = 12;

/// Where `BagMRU` lives, as a path **relative to the hive root**, paired with the
/// hive that root belongs to. A raw `UsrClass.dat` is rooted at the user's
/// `Classes` key, so its shellbags sit directly under `Local Settings\…`; an
/// `NTUSER.DAT` has them under `Software\…`, and Windows XP additionally used a
/// separate `ShellNoRoam` tree for non-roaming folders.
const BAG_ROOTS: &[(&str, &str, RootFamily)] = &[
    (
        "Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU",
        "UsrClass.dat — Shell (Windows Vista and later)",
        RootFamily::UsrClass,
    ),
    (
        "Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU",
        "UsrClass.dat content merged into a NTUSER.DAT export",
        RootFamily::UsrClass,
    ),
    (
        "Software\\Microsoft\\Windows\\Shell\\BagMRU",
        "NTUSER.DAT — Shell",
        RootFamily::NtUser,
    ),
    (
        "Software\\Microsoft\\Windows\\ShellNoRoam\\BagMRU",
        "NTUSER.DAT — ShellNoRoam (Windows XP)",
        RootFamily::ShellNoRoam,
    ),
];

/// Which family of shellbag roots a `bag_root` selection covers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RootFamily {
    UsrClass,
    NtUser,
    ShellNoRoam,
}

/// Shell-namespace GUIDs that are stable across Windows releases and appear at
/// the top of virtually every shellbag tree. Anything not listed here is printed
/// as a raw `{guid}` rather than guessed at — a wrong friendly name in a
/// forensic report is worse than an honest GUID.
const KNOWN_GUIDS: &[(&str, &str)] = &[
    ("20d04fe0-3aea-1069-a2d8-08002b30309d", "This PC"),
    ("21ec2020-3aea-1069-a2dd-08002b30309d", "Control Panel"),
    ("645ff040-5081-101b-9f08-00aa002f954e", "Recycle Bin"),
    ("450d8fba-ad25-11d0-98a8-0800361b1103", "My Documents"),
    ("208d2c60-3aea-1069-a2d7-08002b30309d", "My Network Places"),
    ("871c5380-42a0-1069-a2ea-08002b30309d", "Internet Explorer"),
    ("031e4825-7b94-4dc3-b131-e946b44c8dd5", "Libraries"),
    ("59031a47-3f72-44a7-89c5-5595fe6b30ee", "Users"),
    ("b4bfcc3a-db2c-424c-b029-7fe99a87c641", "Desktop"),
    ("374de290-123f-4565-9164-39c4925e467b", "Downloads"),
    ("33e28130-4e1e-4676-835a-98395c3bc3bb", "Pictures"),
    ("4bd8d571-6d19-48d3-be97-422220080e43", "Music"),
    ("18989b1d-99b5-455b-841c-ab7c74e4ddfc", "Videos"),
    ("d3162b92-9365-467a-956b-92703aca08af", "Documents"),
    ("679f85cb-0220-4080-b29b-5540cc05aab6", "Quick access"),
    ("1f3427c8-5c10-4210-aa03-2ee45287d668", "User Pinned"),
    ("d20ea4e1-3957-11d2-a40b-0c5020524153", "Administrative Tools"),
];

/// How to interpret the supplied hive bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InFmt {
    /// Hex, with or without separators and an optional leading `0x` — the default.
    Hex,
    /// Standard Base64 (RFC 4648), padding optional, whitespace ignored.
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
                // Accept both padded and unpadded Base64 — a hive pasted out of
                // a report is routinely missing its trailing "=".
                let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
                B64.decode(&cleaned)
                    .or_else(|_| B64_NO_PAD.decode(cleaned.trim_end_matches('=')))
                    .map_err(|e| format!("input is not valid Base64: {e}"))
            }
        }
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(s.len() / 2);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() || c == ',' || c == ':' || c == '-' {
            continue;
        }
        if c == '0' && matches!(chars.peek(), Some('x') | Some('X')) {
            chars.next();
            continue;
        }
        match c.to_digit(16) {
            Some(d) => nibbles.push(d as u8),
            None => {
                return Err(format!(
                    "input is not valid hex: unexpected character {c:?}. Supply contiguous or \
                     whitespace-separated hex bytes (72 65 67 66 …), or switch input_encoding \
                     to \"base64\"."
                ))
            }
        }
    }
    if nibbles.len() % 2 != 0 {
        return Err(format!(
            "input is not valid hex: {} hex digits is an odd count, so the last byte is \
             incomplete.",
            nibbles.len()
        ));
    }
    Ok(nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect())
}

/// What to render.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Indented reconstruction of the browsed folder hierarchy.
    Tree,
    /// One absolute path per line with the forensic detail columns.
    List,
    /// Comma-separated rows with a header, for a spreadsheet or timeline.
    Csv,
    /// The Sleuth Kit bodyfile format, for `mactime`.
    Bodyfile,
    /// Per-entry shell item diagnostics: class byte, decoded fields, hex preview.
    Raw,
}

impl Mode {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "tree" => Ok(Mode::Tree),
            "list" => Ok(Mode::List),
            "csv" => Ok(Mode::Csv),
            "bodyfile" => Ok(Mode::Bodyfile),
            "raw" => Ok(Mode::Raw),
            other => Err(format!(
                "invalid mode {other:?}: expected \"tree\", \"list\", \"csv\", \"bodyfile\" or \
                 \"raw\""
            )),
        }
    }
}

fn parse_bag_root(s: &str) -> Result<Option<RootFamily>, String> {
    match s.trim() {
        "" | "auto" => Ok(None),
        "usrclass" => Ok(Some(RootFamily::UsrClass)),
        "ntuser" => Ok(Some(RootFamily::NtUser)),
        "shellnoroam" => Ok(Some(RootFamily::ShellNoRoam)),
        other => Err(format!(
            "invalid bag_root {other:?}: expected \"auto\", \"usrclass\", \"ntuser\" or \
             \"shellnoroam\""
        )),
    }
}

// ---------------------------------------------------------------------------
// DOS/FAT date-time
// ---------------------------------------------------------------------------

/// A decoded DOS/FAT date-time. Stored in a shell item as one little-endian
/// `u32`: the low half is the date, the high half is the time.
#[derive(Clone, Copy)]
struct DosTime {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    min: i64,
    sec: i64,
}

impl DosTime {
    fn parse(v: u32) -> Option<Self> {
        if v == 0 {
            return None;
        }
        let date = (v & 0xFFFF) as u32;
        let time = (v >> 16) as u32;
        let t = DosTime {
            year: 1980 + ((date >> 9) & 0x7F) as i64,
            month: ((date >> 5) & 0x0F) as i64,
            day: (date & 0x1F) as i64,
            hour: ((time >> 11) & 0x1F) as i64,
            min: ((time >> 5) & 0x3F) as i64,
            sec: ((time & 0x1F) as i64) * 2,
        };
        // A zeroed or nonsensical field is "not recorded", not "1980-00-00".
        if t.month == 0 || t.month > 12 || t.day == 0 || t.day > 31 || t.hour > 23 || t.min > 59 {
            return None;
        }
        Some(t)
    }

    fn text(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.min, self.sec
        )
    }

    /// Seconds since the Unix epoch. FAT date-times in shell items are recorded
    /// in local time on the machine that wrote them; there is no offset stored,
    /// so this treats them as UTC and the page says so.
    fn epoch(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * 86_400
            + self.hour * 3600
            + self.min * 60
            + self.sec
    }
}

/// Howard Hinnant's civil-date → days-since-1970 algorithm; no date crate needed.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// ---------------------------------------------------------------------------
// Shell item decoding
// ---------------------------------------------------------------------------

/// One decoded `SHITEMID`.
struct ShellItem {
    class: u8,
    kind: String,
    /// Best available display name (long name beats the primary/8.3 name).
    name: String,
    /// The primary name, when a nicer long name replaced it.
    primary_name: Option<String>,
    guid: Option<String>,
    file_size: Option<u32>,
    attributes: Option<u16>,
    modified: Option<DosTime>,
    created: Option<DosTime>,
    accessed: Option<DosTime>,
    /// NTFS file reference from a version ≥ 7 `0xBEEF0004` block.
    mft: Option<(u64, u16)>,
    /// Why a field is missing / what could not be decoded.
    note: Option<String>,
}

fn read_u16(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// NUL-terminated ASCII/ANSI string; non-ASCII bytes become `.` so a report can
/// never emit raw control characters.
fn read_ascii(b: &[u8], o: usize) -> (String, usize) {
    let mut s = String::new();
    let mut i = o;
    while i < b.len() && b[i] != 0 {
        let c = b[i];
        s.push(if (0x20..0x7F).contains(&c) {
            c as char
        } else {
            '.'
        });
        i += 1;
    }
    (s, i + 1 - o)
}

/// NUL-terminated UTF-16LE string.
fn read_utf16(b: &[u8], o: usize) -> (String, usize) {
    let mut units: Vec<u16> = Vec::new();
    let mut i = o;
    while i + 1 < b.len() {
        let u = u16::from_le_bytes([b[i], b[i + 1]]);
        i += 2;
        if u == 0 {
            break;
        }
        units.push(u);
    }
    (sanitize(&String::from_utf16_lossy(&units)), i - o)
}

fn format_guid(b: &[u8]) -> Option<String> {
    let g = b.get(0..16)?;
    Some(format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        u32::from_le_bytes([g[0], g[1], g[2], g[3]]),
        u16::from_le_bytes([g[4], g[5]]),
        u16::from_le_bytes([g[6], g[7]]),
        g[8],
        g[9],
        g[10],
        g[11],
        g[12],
        g[13],
        g[14],
        g[15],
    ))
}

fn guid_name(guid: &str) -> Option<&'static str> {
    KNOWN_GUIDS
        .iter()
        .find(|(g, _)| *g == guid)
        .map(|(_, n)| *n)
}

/// Locate the optional `0xBEEF0004` extension block by scanning for its
/// signature. Real items pad the primary name unpredictably, so a computed
/// offset misses blocks that a signature scan finds every time.
fn find_beef0004(b: &[u8]) -> Option<usize> {
    const SIG: [u8; 4] = [0x04, 0x00, 0xEF, 0xBE];
    b.windows(4)
        .position(|w| w == SIG)
        .and_then(|i| i.checked_sub(4))
}

/// Pull creation/access times, the NTFS file reference and the long name out of
/// a `0xBEEF0004` block. Applied to every class, because delegate and
/// vendor-specific items carry the same block.
fn parse_extension_block(b: &[u8], item: &mut ShellItem) {
    let Some(start) = find_beef0004(b) else {
        return;
    };
    let Some(version) = read_u16(b, start + 2) else {
        return;
    };
    if let Some(v) = read_u32(b, start + 8) {
        item.created = DosTime::parse(v);
    }
    if let Some(v) = read_u32(b, start + 12) {
        item.accessed = DosTime::parse(v);
    }

    // +16 is a 2-byte identifier/unknown shared by every version.
    let mut p = start + 18;
    if version >= 7 {
        p += 2; // unknown
        if let Some(raw) = b.get(p..p + 8) {
            let entry = u64::from_le_bytes([
                raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], 0, 0,
            ]);
            let seq = u16::from_le_bytes([raw[6], raw[7]]);
            if entry != 0 {
                item.mft = Some((entry, seq));
            }
        }
        p += 8; // file reference
        p += 8; // unknown, empty
    }
    // Size of the trailing localized name, then version-gated padding.
    let long_string_size = read_u16(b, p).unwrap_or(0);
    p += 2;
    if version >= 9 {
        p += 4;
    }
    if version >= 8 {
        p += 4;
    }
    let _ = long_string_size;
    if p < b.len() {
        let (long, _) = read_utf16(b, p);
        if !long.is_empty() {
            if long != item.name {
                item.primary_name = Some(item.name.clone());
            }
            item.name = long;
        }
    }
}

/// Decode a shell item blob into a display name plus whatever metadata it holds.
/// Never fails: an undecodable item still reports its class byte and a note, so
/// a damaged hive produces evidence rather than a hole.
fn parse_shell_item(raw: &[u8], resolve_guids: bool) -> ShellItem {
    let mut item = ShellItem {
        class: 0,
        kind: "Unknown".to_string(),
        name: String::new(),
        primary_name: None,
        guid: None,
        file_size: None,
        attributes: None,
        modified: None,
        created: None,
        accessed: None,
        mft: None,
        note: None,
    };
    if raw.len() < 3 {
        item.note = Some(format!(
            "shell item is only {} byte(s); at least 3 are needed for a size and class",
            raw.len()
        ));
        item.name = "(unreadable shell item)".to_string();
        return item;
    }
    // The item's own size field bounds it; trust it only when it is sane.
    let declared = read_u16(raw, 0).unwrap_or(0) as usize;
    let b: &[u8] = if declared >= 3 && declared <= raw.len() {
        &raw[..declared]
    } else {
        raw
    };
    let class = b[2];
    item.class = class;

    match class {
        0x1F => {
            item.kind = "Root folder".to_string();
            match format_guid(&b[4.min(b.len())..]) {
                Some(g) => {
                    item.name = match (resolve_guids, guid_name(&g)) {
                        (true, Some(n)) => n.to_string(),
                        _ => format!("{{{g}}}"),
                    };
                    item.guid = Some(g);
                }
                None => {
                    item.name = "(root folder, GUID truncated)".to_string();
                    item.note = Some("root folder item is too short to hold a 16-byte GUID".into());
                }
            }
        }
        0x20..=0x2F => {
            item.kind = "Volume".to_string();
            let (ascii, _) = read_ascii(b, 3);
            if !ascii.is_empty() && ascii.contains(':') {
                item.name = ascii;
            } else if let Some(g) = format_guid(&b[4.min(b.len())..]) {
                item.name = match (resolve_guids, guid_name(&g)) {
                    (true, Some(n)) => n.to_string(),
                    _ => format!("{{{g}}}"),
                };
                item.guid = Some(g);
            } else if !ascii.is_empty() {
                item.name = ascii;
            } else {
                item.name = "(volume, name not recorded)".to_string();
                item.note = Some("volume item carried neither a drive string nor a GUID".into());
            }
        }
        0x30..=0x3F => {
            item.kind = if class & 0x01 != 0 {
                "Directory".to_string()
            } else {
                "File".to_string()
            };
            item.file_size = read_u32(b, 4);
            item.modified = read_u32(b, 8).and_then(DosTime::parse);
            item.attributes = read_u16(b, 12);
            // Bit 2 of the class byte marks UTF-16 names; some writers set it
            // inconsistently, so an ASCII read that stalls on an embedded NUL is
            // retried as UTF-16.
            let (ascii, _) = read_ascii(b, 14);
            let unicode_flag = class & 0x04 != 0;
            let looks_utf16 = b.len() > 16 && b[15] == 0 && b[14] != 0;
            item.name = if unicode_flag || (ascii.len() <= 1 && looks_utf16) {
                read_utf16(b, 14).0
            } else {
                ascii
            };
            if item.name.is_empty() {
                item.name = "(file entry, name not recorded)".to_string();
                item.note = Some("file entry item has an empty primary name".into());
            }
        }
        0x40..=0x4F => {
            item.kind = "Network location".to_string();
            let (loc, _) = read_ascii(b, 4);
            let loc = if loc.is_empty() { read_utf16(b, 4).0 } else { loc };
            item.name = if loc.is_empty() {
                item.note = Some("network item carried no location string".into());
                "(network location, name not recorded)".to_string()
            } else if loc.starts_with("\\\\") {
                loc
            } else {
                format!("\\\\{loc}")
            };
        }
        0x61 => {
            item.kind = "URI".to_string();
            let (u16s, _) = read_utf16(b, 10.min(b.len()));
            let (ascii, _) = read_ascii(b, 10.min(b.len()));
            item.name = if u16s.len() >= ascii.len() { u16s } else { ascii };
            if item.name.is_empty() {
                item.name = "(URI, not decodable)".to_string();
            }
        }
        0x71 => {
            item.kind = "Control Panel item".to_string();
            match format_guid(&b[14.min(b.len())..]) {
                Some(g) => {
                    item.name = match (resolve_guids, guid_name(&g)) {
                        (true, Some(n)) => n.to_string(),
                        _ => format!("{{{g}}}"),
                    };
                    item.guid = Some(g);
                }
                None => item.name = "(control panel item)".to_string(),
            }
        }
        0x74 => {
            item.kind = "Delegate folder".to_string();
            // Anchored on the "CFSF" signature: everything after it is laid out
            // like a file entry, but the bytes before it vary by writer.
            match b.windows(4).position(|w| w == b"CFSF") {
                Some(sig) => {
                    item.file_size = read_u32(b, sig + 4);
                    item.modified = read_u32(b, sig + 8).and_then(DosTime::parse);
                    item.attributes = read_u16(b, sig + 12);
                    let (ascii, _) = read_ascii(b, sig + 14);
                    item.name = ascii;
                }
                None => {
                    item.note = Some("delegate item has no CFSF signature".into());
                }
            }
            if item.name.is_empty() {
                item.name = "(delegate folder)".to_string();
            }
        }
        _ => {
            item.kind = format!("Unrecognised (class 0x{class:02x})");
            item.name = "(undecoded shell item)".to_string();
            item.note = Some(format!(
                "class 0x{class:02x} is not one of the documented shell item classes; only the \
                 extension block was read"
            ));
        }
    }

    parse_extension_block(b, &mut item);
    item.name = sanitize(&item.name);
    item
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() {
                '.'
            } else {
                c
            }
        })
        .collect()
}

fn to_hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// BagMRU walk
// ---------------------------------------------------------------------------

/// One reconstructed shellbag entry.
struct Entry {
    /// Registry path of the node relative to the `BagMRU` root, e.g. `0\3\1`.
    bag_path: String,
    depth: usize,
    /// Reconstructed absolute path of the browsed folder.
    path: String,
    name: String,
    /// Position in `MRUListEx`, 0 = most recently interacted with.
    mru_pos: Option<usize>,
    /// `NodeSlot` — the `Bags\<slot>\Shell` key holding this folder's view prefs.
    slot: Option<u32>,
    /// Last-write time of the child registry key, the best proxy for "when the
    /// user last interacted with this folder".
    key_written: Option<String>,
    key_epoch: Option<i64>,
    item: ShellItem,
    raw: Vec<u8>,
}

struct WalkCtx {
    max_entries: usize,
    max_depth: usize,
    resolve_guids: bool,
    truncated: bool,
    depth_capped: bool,
}

/// `MRUListEx` is a run of little-endian `u32` indices terminated by
/// `0xFFFFFFFF`, most recently used first.
fn parse_mru_list(raw: &[u8]) -> Vec<u32> {
    let mut out = Vec::new();
    for chunk in raw.chunks_exact(4) {
        let v = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if v == 0xFFFF_FFFF {
            break;
        }
        out.push(v);
    }
    out
}

fn join_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else if parent.ends_with('\\') {
        format!("{parent}{name}")
    } else {
        format!("{parent}\\{name}")
    }
}

fn walk(
    key: &RegistryKey,
    bag_path: &str,
    parent_path: &str,
    depth: usize,
    ctx: &mut WalkCtx,
    out: &mut Vec<Entry>,
) {
    if depth >= ctx.max_depth {
        ctx.depth_capped = true;
        return;
    }
    let values = match key.values() {
        Ok(v) => v,
        Err(_) => return,
    };

    // Numbered values are the child shell items; MRUListEx gives their order.
    let mut numbered: Vec<u32> = values
        .iter()
        .filter_map(|v| v.name().parse::<u32>().ok())
        .collect();
    numbered.sort_unstable();

    let mru: Vec<u32> = values
        .iter()
        .find(|v| v.name().eq_ignore_ascii_case("MRUListEx"))
        .and_then(|v| v.raw_data().ok())
        .map(|d| parse_mru_list(&d))
        .unwrap_or_default();

    // MRU order first, then anything the MRU list forgot (a truncated or
    // partially-overwritten MRUListEx must not hide entries).
    let mut order: Vec<(u32, Option<usize>)> = Vec::new();
    for (pos, idx) in mru.iter().enumerate() {
        if numbered.contains(idx) {
            order.push((*idx, Some(pos)));
        }
    }
    for idx in &numbered {
        if !order.iter().any(|(i, _)| i == idx) {
            order.push((*idx, None));
        }
    }

    for (idx, mru_pos) in order {
        if out.len() >= ctx.max_entries {
            ctx.truncated = true;
            return;
        }
        let vname = idx.to_string();
        let raw = match key.value(&vname).and_then(|v| v.raw_data()) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let item = parse_shell_item(&raw, ctx.resolve_guids);
        let path = join_path(parent_path, &item.name);
        let child = key.open_subkey(&vname).ok();
        let slot = child
            .as_ref()
            .and_then(|c| c.value("NodeSlot").ok())
            .and_then(|v| v.dword_data().ok());
        let written = child
            .as_ref()
            .and_then(|c| c.last_written())
            .or_else(|| key.last_written());
        let child_bag = if bag_path.is_empty() {
            vname.clone()
        } else {
            format!("{bag_path}\\{vname}")
        };

        out.push(Entry {
            bag_path: child_bag.clone(),
            depth,
            path: path.clone(),
            name: item.name.clone(),
            mru_pos,
            slot,
            key_written: written.map(|t| t.to_string().replace(" UTC", "Z")),
            key_epoch: written.map(|t| t.timestamp()),
            item,
            raw,
        });

        if let Some(c) = child {
            walk(&c, &child_bag, &path, depth + 1, ctx, out);
        }
    }
}

/// Walk a backslash path one component at a time so a miss names the exact
/// component that failed and shows what was actually there.
fn open_path<'h>(hive: &'h RegistryHive, path: &str) -> Result<RegistryKey<'h>, String> {
    let mut key = hive
        .root_key()
        .map_err(|e| format!("cannot read the root key: {e}"))?;
    let mut walked: Vec<String> = Vec::new();
    for part in path
        .split(['\\', '/'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        match key.open_subkey(part) {
            Ok(next) => {
                walked.push(part.to_string());
                key = next;
            }
            Err(_) => {
                let where_ = if walked.is_empty() {
                    "the hive root".to_string()
                } else {
                    format!("\"{}\"", walked.join("\\"))
                };
                let available = key
                    .subkeys()
                    .map(|v| {
                        let names: Vec<String> =
                            v.iter().take(SIBLING_HINTS).map(|k| k.name()).collect();
                        if names.is_empty() {
                            " It has no subkeys at all.".to_string()
                        } else if v.len() > names.len() {
                            format!(
                                " Subkeys present there: {} … (+{} more).",
                                names.join(", "),
                                v.len() - names.len()
                            )
                        } else {
                            format!(" Subkeys present there: {}.", names.join(", "))
                        }
                    })
                    .unwrap_or_default();
                return Err(format!(
                    "{part:?} is not a subkey of {where_}.{available}"
                ));
            }
        }
    }
    Ok(key)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn detail_suffix(e: &Entry) -> String {
    let mut bits: Vec<String> = Vec::new();
    if let Some(s) = e.slot {
        bits.push(format!("slot {s}"));
    }
    if let Some(p) = e.mru_pos {
        bits.push(format!("mru {p}"));
    }
    if let Some(t) = &e.item.modified {
        bits.push(format!("modified {}", t.text()));
    }
    if let Some(t) = &e.item.created {
        bits.push(format!("created {}", t.text()));
    }
    if let Some(t) = &e.item.accessed {
        bits.push(format!("accessed {}", t.text()));
    }
    if let Some(t) = &e.key_written {
        bits.push(format!("key written {t}"));
    }
    if let Some((entry, seq)) = e.item.mft {
        bits.push(format!("mft {entry}-{seq}"));
    }
    if bits.is_empty() {
        String::new()
    } else {
        format!("  [{}]", bits.join(", "))
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_tree(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in entries {
        out.push_str(&format!(
            "{}{}  ({}){}\n",
            "  ".repeat(e.depth),
            e.name,
            e.item.kind,
            detail_suffix(e)
        ));
    }
    out
}

fn render_list(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in entries {
        out.push_str(&format!("{}{}\n", e.path, detail_suffix(e)));
    }
    out
}

fn render_csv(entries: &[Entry]) -> String {
    let mut out = String::from(
        "BagPath,Slot,MRUPosition,Depth,Path,Name,ShellType,ClassByte,Modified,Created,Accessed,\
         KeyLastWrite,MftEntry,FileSize\n",
    );
    for e in entries {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},0x{:02x},{},{},{},{},{},{}\n",
            csv_escape(&e.bag_path),
            e.slot.map(|s| s.to_string()).unwrap_or_default(),
            e.mru_pos.map(|s| s.to_string()).unwrap_or_default(),
            e.depth,
            csv_escape(&e.path),
            csv_escape(&e.name),
            csv_escape(&e.item.kind),
            e.item.class,
            e.item.modified.map(|t| t.text()).unwrap_or_default(),
            e.item.created.map(|t| t.text()).unwrap_or_default(),
            e.item.accessed.map(|t| t.text()).unwrap_or_default(),
            e.key_written.clone().unwrap_or_default(),
            e.item
                .mft
                .map(|(n, s)| format!("{n}-{s}"))
                .unwrap_or_default(),
            e.item.file_size.map(|s| s.to_string()).unwrap_or_default(),
        ));
    }
    out
}

/// The Sleuth Kit 3.x bodyfile layout:
/// `MD5|name|inode|mode|UID|GID|size|atime|mtime|ctime|crtime`.
fn render_bodyfile(entries: &[Entry]) -> String {
    let mut out = String::new();
    for (i, e) in entries.iter().enumerate() {
        out.push_str(&format!(
            "0|{} (Shellbag)|{}|0|0|0|{}|{}|{}|{}|{}\n",
            e.path,
            i + 1,
            e.item.file_size.unwrap_or(0),
            e.item.accessed.map(|t| t.epoch()).unwrap_or(0),
            e.item.modified.map(|t| t.epoch()).unwrap_or(0),
            e.key_epoch.unwrap_or(0),
            e.item.created.map(|t| t.epoch()).unwrap_or(0),
        ));
    }
    out
}

fn render_raw(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in entries {
        out.push_str(&format!("BagMRU\\{}\n", e.bag_path));
        out.push_str(&format!("  Path            {}\n", e.path));
        out.push_str(&format!(
            "  Class           0x{:02x} ({})\n",
            e.item.class, e.item.kind
        ));
        out.push_str(&format!("  Name            {}\n", e.name));
        if let Some(p) = &e.item.primary_name {
            out.push_str(&format!("  Primary name    {p}\n"));
        }
        if let Some(g) = &e.item.guid {
            out.push_str(&format!("  GUID            {{{g}}}\n"));
        }
        if let Some(s) = e.slot {
            out.push_str(&format!("  NodeSlot        {s}\n"));
        }
        if let Some(p) = e.mru_pos {
            out.push_str(&format!("  MRU position    {p}\n"));
        }
        if let Some(s) = e.item.file_size {
            out.push_str(&format!("  File size       {s}\n"));
        }
        if let Some(a) = e.item.attributes {
            out.push_str(&format!("  Attributes      0x{a:04x}\n"));
        }
        for (label, t) in [
            ("Modified", &e.item.modified),
            ("Created", &e.item.created),
            ("Accessed", &e.item.accessed),
        ] {
            if let Some(t) = t {
                out.push_str(&format!("  {label:<15} {} (DOS/FAT)\n", t.text()));
            }
        }
        if let Some(t) = &e.key_written {
            out.push_str(&format!("  Key last write  {t}\n"));
        }
        if let Some((entry, seq)) = e.item.mft {
            out.push_str(&format!("  NTFS reference  MFT entry {entry}, sequence {seq}\n"));
        }
        if let Some(n) = &e.item.note {
            out.push_str(&format!("  Note            {n}\n"));
        }
        let shown = e.raw.len().min(RAW_PREVIEW_BYTES);
        out.push_str(&format!(
            "  Shell item      {} byte(s): {}{}\n",
            e.raw.len(),
            to_hex(&e.raw[..shown]),
            if shown < e.raw.len() { " …" } else { "" }
        ));
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Parse shellbags out of a registry hive supplied as hex or Base64 text.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    input_encoding: &str,
    mode: &str,
    bag_root: &str,
    custom_path: &str,
    max_entries: usize,
    max_depth: usize,
    resolve_guids: bool,
) -> Result<String, String> {
    let fmt = InFmt::parse(input_encoding)?;
    let mode = Mode::parse(mode)?;
    let family = parse_bag_root(bag_root)?;
    let max_entries = match max_entries {
        0 => DEFAULT_MAX_ENTRIES,
        n => n.min(MAX_MAX_ENTRIES),
    };
    let max_depth = match max_depth {
        0 => DEFAULT_MAX_DEPTH,
        n => n.min(MAX_MAX_DEPTH),
    };

    if data.trim().is_empty() {
        return Err("no hive bytes supplied: paste the contents of UsrClass.dat or NTUSER.DAT \
                    encoded as hex or Base64."
            .to_string());
    }
    let bytes = fmt.to_bytes(data)?;
    if bytes.len() < 4096 {
        return Err(format!(
            "input is only {} byte(s); a registry hive starts with a 4096-byte base block. Paste \
             the whole file, not just its opening bytes.",
            bytes.len()
        ));
    }
    if &bytes[0..4] != b"regf" {
        return Err(format!(
            "input does not start with the \"regf\" signature (found {}). Supply a raw registry \
             hive such as UsrClass.dat or NTUSER.DAT — not a .reg export, not a disk image.",
            to_hex(&bytes[0..4])
        ));
    }

    let hive = RegistryHive::from_bytes(bytes)
        .map_err(|e| format!("the hive header parsed but the hive could not be loaded: {e}"))?;

    // Which BagMRU roots to try.
    let candidates: Vec<(String, String)> = if !custom_path.trim().is_empty() {
        vec![(
            custom_path.trim().to_string(),
            "custom_path (supplied by you)".to_string(),
        )]
    } else {
        BAG_ROOTS
            .iter()
            .filter(|(_, _, f)| family.is_none_or(|want| want == *f))
            .map(|(p, d, _)| (p.to_string(), d.to_string()))
            .collect()
    };

    let mut out = String::new();
    let mut found_any = false;
    let mut misses: Vec<String> = Vec::new();

    for (path, desc) in &candidates {
        let key = match open_path(&hive, path) {
            Ok(k) => k,
            Err(e) => {
                misses.push(format!("  {path}\n    not present — {e}"));
                continue;
            }
        };
        let mut ctx = WalkCtx {
            max_entries,
            max_depth,
            resolve_guids,
            truncated: false,
            depth_capped: false,
        };
        let mut entries: Vec<Entry> = Vec::new();
        walk(&key, "", "", 0, &mut ctx, &mut entries);
        found_any = true;

        if !matches!(mode, Mode::Bodyfile | Mode::Csv) || candidates.len() > 1 {
            let header = format!("Shellbag root: {path}\n  {desc}\n");
            // CSV/bodyfile stay machine-readable: comment the header out.
            if matches!(mode, Mode::Bodyfile | Mode::Csv) {
                for line in header.lines() {
                    out.push_str(&format!("# {line}\n"));
                }
            } else {
                out.push_str(&header);
            }
        }

        if entries.is_empty() {
            out.push_str(
                "  BagMRU exists but holds no numbered shell item values — this user browsed no \
                 folders, or the tree was cleared.\n\n",
            );
            continue;
        }

        let body = match mode {
            Mode::Tree => render_tree(&entries),
            Mode::List => render_list(&entries),
            Mode::Csv => render_csv(&entries),
            Mode::Bodyfile => render_bodyfile(&entries),
            Mode::Raw => render_raw(&entries),
        };
        out.push_str(&body);

        let mut notes: Vec<String> = Vec::new();
        notes.push(format!("{} entr(ies) reconstructed", entries.len()));
        if ctx.truncated {
            notes.push(format!(
                "stopped at the max_entries cap of {max_entries} — raise it to see the rest"
            ));
        }
        if ctx.depth_capped {
            notes.push(format!(
                "stopped descending at the max_depth cap of {max_depth}"
            ));
        }
        let note_line = format!("({})\n\n", notes.join("; "));
        if matches!(mode, Mode::Bodyfile | Mode::Csv) {
            out.push_str(&format!("# {note_line}"));
        } else {
            out.push_str(&note_line);
        }
    }

    if !found_any {
        let mut msg = String::from("No shellbag (BagMRU) key was found in this hive.\n\n");
        msg.push_str("Locations checked:\n");
        for m in &misses {
            msg.push_str(m);
            msg.push('\n');
        }
        msg.push_str(
            "\nShellbags live in UsrClass.dat on Windows Vista and later, and in NTUSER.DAT on \
             Windows XP. If you loaded SYSTEM, SOFTWARE, SAM or Amcache.hve there is nothing to \
             find here. Root subkeys of the hive you loaded:\n",
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

    Ok(out.trim_end().to_string() + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use regf::structures::DataType;
    use regf::writer::HiveBuilder;

    // ---- shell item builders --------------------------------------------

    /// A `0x31` directory file-entry shell item with an optional
    /// `0xBEEF0004` version-3 extension block carrying a long name.
    fn file_entry(name: &str, long: Option<&str>, modified: u32, created: u32) -> Vec<u8> {
        let mut body: Vec<u8> = Vec::new();
        body.push(0x31); // class: directory
        body.push(0x00); // reserved
        body.extend_from_slice(&0u32.to_le_bytes()); // file size
        body.extend_from_slice(&modified.to_le_bytes()); // DOS modification
        body.extend_from_slice(&0x0010u16.to_le_bytes()); // FILE_ATTRIBUTE_DIRECTORY
        body.extend_from_slice(name.as_bytes());
        body.push(0); // NUL
        if body.len() % 2 == 1 {
            body.push(0); // 16-bit alignment before the extension block
        }
        if let Some(long) = long {
            let mut ext: Vec<u8> = Vec::new();
            ext.extend_from_slice(&0x0003u16.to_le_bytes()); // version 3
            ext.extend_from_slice(&0xBEEF_0004u32.to_le_bytes()); // signature
            ext.extend_from_slice(&created.to_le_bytes()); // creation
            ext.extend_from_slice(&created.to_le_bytes()); // last access
            ext.extend_from_slice(&0x0014u16.to_le_bytes()); // identifier
            ext.extend_from_slice(&0u16.to_le_bytes()); // long string size
            for u in long.encode_utf16() {
                ext.extend_from_slice(&u.to_le_bytes());
            }
            ext.extend_from_slice(&[0, 0]); // NUL
            ext.extend_from_slice(&0u16.to_le_bytes()); // first-block offset
            let size = (ext.len() + 2) as u16;
            body.extend_from_slice(&size.to_le_bytes());
            body.extend_from_slice(&ext);
        }
        let mut item = ((body.len() + 2) as u16).to_le_bytes().to_vec();
        item.extend_from_slice(&body);
        item
    }

    /// A `0x1F` root-folder item for the given GUID.
    fn root_folder(guid: [u8; 16]) -> Vec<u8> {
        let mut body = vec![0x1Fu8, 0x00];
        body.extend_from_slice(&guid);
        let mut item = ((body.len() + 2) as u16).to_le_bytes().to_vec();
        item.extend_from_slice(&body);
        item
    }

    /// A `0x2F` volume item, e.g. `C:\`.
    fn volume(drive: &str) -> Vec<u8> {
        let mut body = vec![0x2Fu8];
        body.extend_from_slice(drive.as_bytes());
        body.push(0);
        let mut item = ((body.len() + 2) as u16).to_le_bytes().to_vec();
        item.extend_from_slice(&body);
        item
    }

    /// A `0x42` network-location item.
    fn network(loc: &str) -> Vec<u8> {
        let mut body = vec![0x42u8, 0x81];
        body.extend_from_slice(loc.as_bytes());
        body.push(0);
        let mut item = ((body.len() + 2) as u16).to_le_bytes().to_vec();
        item.extend_from_slice(&body);
        item
    }

    /// `MRUListEx` bytes for the given order.
    fn mru(order: &[u32]) -> Vec<u8> {
        let mut v: Vec<u8> = order.iter().flat_map(|i| i.to_le_bytes()).collect();
        v.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        v
    }

    /// DOS/FAT date-time for 2024-05-17 09:30:00.
    fn dos_2024_05_17() -> u32 {
        let date: u32 = (((2024 - 1980) as u32) << 9) | (5 << 5) | 17;
        let time: u32 = (9 << 11) | (30 << 5);
        (time << 16) | date
    }

    /// DOS/FAT date-time for 2021-01-02 03:04:00.
    fn dos_2021_01_02() -> u32 {
        let date: u32 = (((2021 - 1980) as u32) << 9) | (1 << 5) | 2;
        let time: u32 = (3 << 11) | (4 << 5);
        (time << 16) | date
    }

    const THIS_PC_GUID: [u8; 16] = [
        0xE0, 0x4F, 0xD0, 0x20, 0xEA, 0x3A, 0x69, 0x10, 0xA2, 0xD8, 0x08, 0x00, 0x2B, 0x30, 0x30,
        0x9D,
    ];

    /// A synthetic UsrClass.dat-shaped hive whose shellbag tree is
    /// `This PC → C:\ → Users → alice → Secret Plans` plus a network share.
    /// Built with `regf`'s own writer, so the bytes are a genuine hive.
    fn fixture_hive() -> Vec<u8> {
        let mut b = HiveBuilder::new();
        let root = b.root_offset();
        let mut cur = root;
        for part in [
            "Local Settings",
            "Software",
            "Microsoft",
            "Windows",
            "Shell",
        ] {
            cur = b.add_key(cur, part).unwrap();
        }
        let bagmru = b.add_key(cur, "BagMRU").unwrap();

        // Level 0: two children, network share most recently used.
        b.add_value(bagmru, "0", DataType::Binary, &root_folder(THIS_PC_GUID))
            .unwrap();
        b.add_value(bagmru, "1", DataType::Binary, &network("fileserver\\share"))
            .unwrap();
        b.add_value(bagmru, "MRUListEx", DataType::Binary, &mru(&[1, 0]))
            .unwrap();
        b.add_value(bagmru, "NodeSlot", DataType::Dword, &0u32.to_le_bytes())
            .unwrap();

        // 0 → C:\
        let k0 = b.add_key(bagmru, "0").unwrap();
        b.add_value(k0, "0", DataType::Binary, &volume("C:\\")).unwrap();
        b.add_value(k0, "MRUListEx", DataType::Binary, &mru(&[0]))
            .unwrap();
        b.add_value(k0, "NodeSlot", DataType::Dword, &1u32.to_le_bytes())
            .unwrap();

        // 1 → the network share leaf.
        let k1 = b.add_key(bagmru, "1").unwrap();
        b.add_value(k1, "NodeSlot", DataType::Dword, &9u32.to_le_bytes())
            .unwrap();

        // 0\0 → Users
        let k00 = b.add_key(k0, "0").unwrap();
        b.add_value(
            k00,
            "0",
            DataType::Binary,
            &file_entry("Users", None, dos_2021_01_02(), 0),
        )
        .unwrap();
        b.add_value(k00, "MRUListEx", DataType::Binary, &mru(&[0]))
            .unwrap();
        b.add_value(k00, "NodeSlot", DataType::Dword, &2u32.to_le_bytes())
            .unwrap();

        // 0\0\0 → alice
        let k000 = b.add_key(k00, "0").unwrap();
        b.add_value(
            k000,
            "0",
            DataType::Binary,
            &file_entry("alice", None, dos_2021_01_02(), 0),
        )
        .unwrap();
        b.add_value(k000, "MRUListEx", DataType::Binary, &mru(&[0]))
            .unwrap();
        b.add_value(k000, "NodeSlot", DataType::Dword, &3u32.to_le_bytes())
            .unwrap();

        // 0\0\0\0 → SECRET~1 with the long name "Secret Plans"
        let k0000 = b.add_key(k000, "0").unwrap();
        b.add_value(
            k0000,
            "0",
            DataType::Binary,
            &file_entry(
                "SECRET~1",
                Some("Secret Plans"),
                dos_2024_05_17(),
                dos_2021_01_02(),
            ),
        )
        .unwrap();
        b.add_value(k0000, "MRUListEx", DataType::Binary, &mru(&[0]))
            .unwrap();
        b.add_value(k0000, "NodeSlot", DataType::Dword, &4u32.to_le_bytes())
            .unwrap();
        let leaf = b.add_key(k0000, "0").unwrap();
        b.add_value(leaf, "NodeSlot", DataType::Dword, &5u32.to_le_bytes())
            .unwrap();

        b.to_bytes().unwrap()
    }

    fn fixture_hex() -> String {
        fixture_hive()
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>()
    }

    fn tree() -> String {
        run(&fixture_hex(), "hex", "tree", "auto", "", 200, 32, true).unwrap()
    }

    // ---- happy paths -----------------------------------------------------

    #[test]
    fn tree_reconstructs_the_browsed_hierarchy() {
        let out = tree();
        assert!(out.contains("Shellbag root: Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU"), "{out}");
        assert!(out.contains("This PC  (Root folder)"), "{out}");
        assert!(out.contains("C:\\  (Volume)"), "{out}");
        assert!(out.contains("Users  (Directory)"), "{out}");
        assert!(out.contains("alice  (Directory)"), "{out}");
        // The long name from the 0xBEEF0004 block wins over the 8.3 primary name.
        assert!(out.contains("Secret Plans  (Directory)"), "{out}");
        assert!(!out.contains("SECRET~1  (Directory)"), "{out}");
        // Indentation encodes depth: two spaces per level.
        assert!(out.contains("\nThis PC  (Root folder)"), "{out}");
        assert!(out.contains("\n  C:\\  (Volume)"), "{out}");
        assert!(out.contains("\n    Users  (Directory)"), "{out}");
        assert!(out.contains("6 entr(ies) reconstructed"), "{out}");
    }

    #[test]
    fn list_emits_absolute_paths_with_detail_columns() {
        let out = run(&fixture_hex(), "hex", "list", "auto", "", 200, 32, true).unwrap();
        assert!(out.contains("C:\\Users\\alice\\Secret Plans"), "{out}");
        assert!(out.contains("\\\\fileserver\\share"), "{out}");
        assert!(out.contains("slot 4"), "{out}");
        assert!(out.contains("modified 2024-05-17 09:30:00"), "{out}");
        assert!(out.contains("created 2021-01-02 03:04:00"), "{out}");
    }

    #[test]
    fn mru_order_puts_the_most_recent_child_first() {
        let out = run(&fixture_hex(), "hex", "list", "auto", "", 200, 32, true).unwrap();
        let share = out.find("\\\\fileserver\\share").unwrap();
        let this_pc = out.find("This PC").unwrap();
        // MRUListEx is [1, 0], so the share (index 1) is listed before This PC.
        assert!(share < this_pc, "{out}");
        assert!(out.contains("mru 0"), "{out}");
        assert!(out.contains("mru 1"), "{out}");
    }

    #[test]
    fn csv_has_a_header_and_one_row_per_entry() {
        let out = run(&fixture_hex(), "hex", "csv", "auto", "", 200, 32, true).unwrap();
        assert!(out.starts_with("# Shellbag root:"), "{out}");
        assert!(
            out.contains("BagPath,Slot,MRUPosition,Depth,Path,Name,ShellType,ClassByte"),
            "{out}"
        );
        assert!(out.contains("0x31"), "{out}");
        assert!(out.contains("\"C:\\Users\\alice\\Secret Plans\"") || out.contains("C:\\Users\\alice\\Secret Plans"), "{out}");
        assert_eq!(out.lines().filter(|l| l.starts_with("0\\")).count(), 4, "{out}");
    }

    #[test]
    fn bodyfile_uses_the_sleuth_kit_layout() {
        let out = run(&fixture_hex(), "hex", "bodyfile", "auto", "", 200, 32, true).unwrap();
        let line = out
            .lines()
            .find(|l| l.contains("Secret Plans"))
            .expect(&out);
        let cols: Vec<&str> = line.split('|').collect();
        assert_eq!(cols.len(), 11, "{line}");
        assert_eq!(cols[0], "0");
        // Paths keep the shell-namespace root they were reached through, the
        // same convention the established shellbag parsers use.
        assert_eq!(cols[1], "This PC\\C:\\Users\\alice\\Secret Plans (Shellbag)");
        // mtime column = the DOS modification time of 2024-05-17 09:30:00 UTC.
        assert_eq!(cols[8], "1715938200", "{line}");
    }

    #[test]
    fn raw_mode_reports_class_bytes_and_hex() {
        let out = run(&fixture_hex(), "hex", "raw", "auto", "", 200, 32, true).unwrap();
        assert!(out.contains("Class           0x1f (Root folder)"), "{out}");
        assert!(out.contains("Class           0x31 (Directory)"), "{out}");
        assert!(out.contains("Primary name    SECRET~1"), "{out}");
        assert!(out.contains("GUID            {20d04fe0-3aea-1069-a2d8-08002b30309d}"), "{out}");
        assert!(out.contains("Attributes      0x0010"), "{out}");
        assert!(out.contains("Shell item      "), "{out}");
    }

    #[test]
    fn base64_input_is_accepted() {
        let b64 = B64.encode(fixture_hive());
        let out = run(&b64, "base64", "tree", "auto", "", 200, 32, true).unwrap();
        assert!(out.contains("Secret Plans"), "{out}");
    }

    #[test]
    fn resolve_guids_off_prints_the_raw_guid() {
        let on = run(&fixture_hex(), "hex", "tree", "auto", "", 200, 32, true).unwrap();
        let off = run(&fixture_hex(), "hex", "tree", "auto", "", 200, 32, false).unwrap();
        assert!(on.contains("This PC  (Root folder)"), "{on}");
        assert!(!off.contains("This PC"), "{off}");
        assert!(
            off.contains("{20d04fe0-3aea-1069-a2d8-08002b30309d}  (Root folder)"),
            "{off}"
        );
    }

    #[test]
    fn custom_path_overrides_the_auto_detected_roots() {
        let out = run(
            &fixture_hex(),
            "hex",
            "tree",
            "auto",
            "Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU\\0",
            200,
            32,
            true,
        )
        .unwrap();
        assert!(out.contains("custom_path (supplied by you)"), "{out}");
        // Rooted one level down, so This PC is gone and C:\ is the top entry.
        assert!(out.contains("C:\\  (Volume)"), "{out}");
        assert!(!out.contains("This PC"), "{out}");
    }

    #[test]
    fn bag_root_selection_narrows_the_search() {
        let out = run(&fixture_hex(), "hex", "tree", "ntuser", "", 200, 32, true).unwrap();
        assert!(out.contains("No shellbag (BagMRU) key was found"), "{out}");
        assert!(out.contains("Software\\Microsoft\\Windows\\Shell\\BagMRU"), "{out}");
        // The honest fallback shows what the hive actually contains.
        assert!(out.contains("Local Settings"), "{out}");
    }

    #[test]
    fn caps_are_reported_not_silently_applied() {
        let capped = run(&fixture_hex(), "hex", "tree", "auto", "", 2, 32, true).unwrap();
        assert!(capped.contains("stopped at the max_entries cap of 2"), "{capped}");
        let shallow = run(&fixture_hex(), "hex", "tree", "auto", "", 200, 2, true).unwrap();
        assert!(shallow.contains("stopped descending at the max_depth cap of 2"), "{shallow}");
        assert!(!shallow.contains("alice"), "{shallow}");
        // 0 means "use the default", not "show nothing".
        let defaulted = run(&fixture_hex(), "hex", "tree", "auto", "", 0, 0, true).unwrap();
        assert!(defaulted.contains("Secret Plans"), "{defaulted}");
        // Above the ceiling is clamped, not rejected.
        assert!(run(&fixture_hex(), "hex", "tree", "auto", "", usize::MAX, usize::MAX, true).is_ok());
    }

    // ---- error paths -----------------------------------------------------

    #[test]
    fn empty_input_is_rejected() {
        let err = run("   ", "hex", "tree", "auto", "", 200, 32, true).unwrap_err();
        assert!(err.contains("no hive bytes supplied"), "{err}");
    }

    #[test]
    fn non_hive_bytes_are_rejected_with_the_signature_found() {
        let bytes = vec![0x50u8, 0x4B, 0x03, 0x04]
            .into_iter()
            .chain(std::iter::repeat(0).take(5000))
            .collect::<Vec<u8>>();
        let hex: String = bytes.iter().map(|x| format!("{x:02x}")).collect();
        let err = run(&hex, "hex", "tree", "auto", "", 200, 32, true).unwrap_err();
        assert!(err.contains("does not start with the \"regf\" signature"), "{err}");
        assert!(err.contains("50 4b 03 04"), "{err}");
    }

    #[test]
    fn a_truncated_hive_says_so() {
        let err = run("72656766", "hex", "tree", "auto", "", 200, 32, true).unwrap_err();
        assert!(err.contains("only 4 byte(s)"), "{err}");
        assert!(err.contains("4096-byte base block"), "{err}");
    }

    #[test]
    fn invalid_hex_names_the_offending_character() {
        let err = run("zz", "hex", "tree", "auto", "", 200, 32, true).unwrap_err();
        assert!(err.contains("not valid hex"), "{err}");
        assert!(err.contains("'z'"), "{err}");
    }

    #[test]
    fn odd_hex_digit_count_is_rejected() {
        let err = run("abc", "hex", "tree", "auto", "", 200, 32, true).unwrap_err();
        assert!(err.contains("odd count"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected_by_name() {
        assert!(run("00", "rot13", "tree", "auto", "", 200, 32, true)
            .unwrap_err()
            .contains("invalid input_encoding"));
        assert!(run("00", "hex", "timeline", "auto", "", 200, 32, true)
            .unwrap_err()
            .contains("invalid mode"));
        assert!(run("00", "hex", "tree", "sam", "", 200, 32, true)
            .unwrap_err()
            .contains("invalid bag_root"));
    }

    #[test]
    fn a_hive_without_shellbags_reports_what_it_does_contain() {
        let mut b = HiveBuilder::new();
        let root = b.root_offset();
        b.add_key(root, "ControlSet001").unwrap();
        let hex: String = b
            .to_bytes()
            .unwrap()
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect();
        let out = run(&hex, "hex", "tree", "auto", "", 200, 32, true).unwrap();
        assert!(out.contains("No shellbag (BagMRU) key was found"), "{out}");
        assert!(out.contains("Locations checked:"), "{out}");
        assert!(out.contains("ControlSet001"), "{out}");
    }

    #[test]
    fn a_bagmru_key_with_no_items_is_reported_as_empty() {
        let mut b = HiveBuilder::new();
        let root = b.root_offset();
        let mut cur = root;
        for part in ["Software", "Microsoft", "Windows", "Shell"] {
            cur = b.add_key(cur, part).unwrap();
        }
        b.add_key(cur, "BagMRU").unwrap();
        let hex: String = b
            .to_bytes()
            .unwrap()
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect();
        let out = run(&hex, "hex", "tree", "ntuser", "", 200, 32, true).unwrap();
        assert!(out.contains("holds no numbered shell item values"), "{out}");
    }

    // ---- shell item unit coverage ---------------------------------------

    #[test]
    fn an_unknown_class_byte_is_reported_not_guessed() {
        let item = parse_shell_item(&[0x06, 0x00, 0xAB, 0x01, 0x02, 0x03], true);
        assert_eq!(item.class, 0xAB);
        assert!(item.kind.contains("0xab"), "{}", item.kind);
        assert!(item.note.as_deref().unwrap().contains("not one of the documented"));
    }

    #[test]
    fn a_two_byte_blob_does_not_panic() {
        let item = parse_shell_item(&[0x02, 0x00], true);
        assert!(item.name.contains("unreadable"), "{}", item.name);
    }

    #[test]
    fn dos_timestamps_round_trip_through_the_epoch() {
        let t = DosTime::parse(dos_2024_05_17()).unwrap();
        assert_eq!(t.text(), "2024-05-17 09:30:00");
        assert_eq!(t.epoch(), 1_715_938_200);
        assert!(DosTime::parse(0).is_none());
        // Month 0 is "not recorded", not January.
        assert!(DosTime::parse(0x0000_0020).is_none());
    }

    #[test]
    fn mru_list_stops_at_the_terminator() {
        assert_eq!(parse_mru_list(&mru(&[2, 0, 1])), vec![2, 0, 1]);
        assert_eq!(parse_mru_list(&[]), Vec::<u32>::new());
    }

    #[test]
    fn hex_input_tolerates_separators_and_prefixes() {
        assert_eq!(decode_hex("0x72 65:67-66").unwrap(), b"regf".to_vec());
        assert_eq!(decode_hex("72\n65\t67 66").unwrap(), b"regf".to_vec());
    }
}
