//! registry-hive-parser core — open a raw Windows registry hive (`regf`) and
//! report its header, browse a key path, or sweep the well-known autostart
//! ("Run key") locations.
//!
//! A registry hive is a self-describing binary container. The first 4096 bytes
//! are the **base block**: the ASCII signature `regf`, a pair of sequence
//! numbers (equal on a cleanly-unmounted hive, unequal on a dirty one that
//! still needs its `.LOG` replayed), the format version, the offset of the root
//! key cell, the size of the hive-bin area, the path the hive was loaded from,
//! and a XOR checksum over the first 508 bytes. Everything after that is a run
//! of 4096-byte **hive bins** holding variable-sized **cells**: `nk` key nodes,
//! `vk` value records, `sk` security descriptors and the subkey lists that tie
//! them together.
//!
//! Structured traversal is delegated to the `regf` crate. Two things are done
//! here that a plain `regf` load cannot do:
//!
//!   * The base block is parsed **independently**, so a hive whose header
//!     checksum is wrong, whose version is unsupported, or whose cell tree is
//!     damaged still produces a full header/integrity report instead of one
//!     opaque error.
//!   * When structured traversal is unavailable, key names are **carved**
//!     directly out of the raw bytes by scanning for `nk` records. That locates
//!     Run/RunOnce keys in a truncated or carved hive, with the honest caveat
//!     that a carved key node cannot be tied back to its values.
//!
//! Pure compute, no wafer/wasm-bindgen deps — shared by the chat skill block and
//! the web page. Runs on every backend including the chat Service Worker.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use regf::hive::{RegistryHive, RegistryKey, RegistryValueEntry};
use regf::structures::{DataType, RegistryValue};

/// The base block is a fixed 4096 bytes; the hive bins start right after it.
const BASE_BLOCK_SIZE: usize = 4096;
/// `max_entries` when the caller leaves it at 0.
const DEFAULT_MAX_ENTRIES: usize = 50;
/// Upper bound on `max_entries` — a listing longer than this is a data dump,
/// not a report, and would blow past the chat surface's token budget.
const MAX_MAX_ENTRIES: usize = 1000;
/// Bytes of a REG_BINARY value shown before eliding.
const BINARY_PREVIEW_BYTES: usize = 32;
/// Characters of a string value shown before eliding.
const STRING_PREVIEW_CHARS: usize = 200;
/// Key names to show when a path lookup fails partway down.
const SIBLING_HINTS: usize = 12;
/// Label column width in the header report.
const LABEL_W: usize = 22;

/// Well-known autostart locations, as paths **relative to the hive root**, with
/// the hive each one lives in. Windows autostart is spread across the user hive
/// (NTUSER.DAT), the machine hive (SOFTWARE) and the boot hive (SYSTEM), and a
/// hive root has no "HKLM\"/"HKCU\" prefix, so all three families are probed and
/// the ones that match tell you which hive you loaded.
const RUN_KEY_CANDIDATES: &[(&str, &str)] = &[
    // ---- NTUSER.DAT (per-user, HKCU) ----
    ("Software\\Microsoft\\Windows\\CurrentVersion\\Run", "NTUSER.DAT — per-user autostart"),
    ("Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce", "NTUSER.DAT — runs once, then deleted"),
    ("Software\\Microsoft\\Windows\\CurrentVersion\\RunOnceEx", "NTUSER.DAT — RunOnceEx loader"),
    ("Software\\Microsoft\\Windows\\CurrentVersion\\RunServices", "NTUSER.DAT — legacy service autostart"),
    ("Software\\Microsoft\\Windows\\CurrentVersion\\RunServicesOnce", "NTUSER.DAT — legacy run-once services"),
    ("Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer\\Run", "NTUSER.DAT — policy-injected autostart"),
    ("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders", "NTUSER.DAT — Startup folder redirection"),
    ("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders", "NTUSER.DAT — resolved Startup folder"),
    ("Software\\Microsoft\\Windows NT\\CurrentVersion\\Windows", "NTUSER.DAT — Load / Run values"),
    ("Software\\Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\Run", "NTUSER.DAT — 32-bit view"),
    ("Software\\Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\RunOnce", "NTUSER.DAT — 32-bit view, run once"),
    // ---- SOFTWARE (machine-wide, HKLM\SOFTWARE) ----
    ("Microsoft\\Windows\\CurrentVersion\\Run", "SOFTWARE — machine-wide autostart"),
    ("Microsoft\\Windows\\CurrentVersion\\RunOnce", "SOFTWARE — runs once, then deleted"),
    ("Microsoft\\Windows\\CurrentVersion\\RunOnceEx", "SOFTWARE — RunOnceEx loader"),
    ("Microsoft\\Windows\\CurrentVersion\\RunServices", "SOFTWARE — legacy service autostart"),
    ("Microsoft\\Windows\\CurrentVersion\\RunServicesOnce", "SOFTWARE — legacy run-once services"),
    ("Microsoft\\Windows\\CurrentVersion\\Policies\\Explorer\\Run", "SOFTWARE — policy-injected autostart"),
    ("Microsoft\\Windows NT\\CurrentVersion\\Winlogon", "SOFTWARE — Userinit / Shell hijacks"),
    ("Microsoft\\Windows NT\\CurrentVersion\\Windows", "SOFTWARE — AppInit_DLLs"),
    ("Microsoft\\Windows NT\\CurrentVersion\\Image File Execution Options", "SOFTWARE — debugger hijacks"),
    ("Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\Run", "SOFTWARE — 32-bit view"),
    ("Wow6432Node\\Microsoft\\Windows\\CurrentVersion\\RunOnce", "SOFTWARE — 32-bit view, run once"),
    // ---- SYSTEM (boot, HKLM\SYSTEM) ----
    ("ControlSet001\\Control\\Session Manager", "SYSTEM — BootExecute"),
    ("ControlSet002\\Control\\Session Manager", "SYSTEM — BootExecute (second control set)"),
    ("CurrentControlSet\\Control\\Session Manager", "SYSTEM — BootExecute (mounted view)"),
];

/// Bare key names treated as autostart hits when carving a damaged hive.
const RUN_KEY_LEAF_NAMES: &[&str] = &[
    "Run",
    "RunOnce",
    "RunOnceEx",
    "RunServices",
    "RunServicesOnce",
    "Winlogon",
    "Session Manager",
    "User Shell Folders",
    "Image File Execution Options",
];

/// How to interpret the supplied hive bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InFmt {
    /// A hex string (e.g. `72656766 ...` or `0x72656766...`), case-insensitive — the default.
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
            InFmt::Hex => parse_hex(s),
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

/// What to report about the hive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Header/integrity report plus the root key's subkeys and values.
    Summary,
    /// Navigate a backslash-separated key path and list what is under it.
    Path,
    /// Probe the well-known autostart locations.
    RunKeys,
}

impl Mode {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "summary" => Ok(Mode::Summary),
            "path" => Ok(Mode::Path),
            "runkeys" | "run_keys" | "run" => Ok(Mode::RunKeys),
            other => Err(format!(
                "invalid mode {other:?}: expected \"summary\", \"path\", or \"runkeys\""
            )),
        }
    }
}

/// Parse a hex string into bytes, ignoring ASCII whitespace and an optional
/// `0x` prefix. Rejects odd length or non-hex digits.
fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex input has an odd number of digits ({}); each byte needs two",
            cleaned.len()
        ));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte {:?}", &cleaned[i..i + 2]))
        })
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le_u64(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}

/// The regf header checksum: XOR of the first 508 bytes taken as little-endian
/// u32s, with 0 and 0xFFFFFFFF folded to 1 and 0xFFFFFFFE respectively.
fn header_checksum(b: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    for chunk in b[..508].chunks_exact(4) {
        sum ^= u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    match sum {
        0xFFFF_FFFF => 0xFFFF_FFFE,
        0 => 1,
        other => other,
    }
}

/// Decode a UTF-16LE buffer, stopping at the first NUL and dropping unpaired
/// surrogates rather than failing — hive strings are frequently untidy.
fn utf16le_lossy(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Days → (year, month, day), Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Render a Windows FILETIME (100-nanosecond ticks since 1601-01-01 UTC) as
/// `YYYY-MM-DD HH:MM:SS UTC`, matching how `regf` displays key timestamps.
fn format_filetime(ft: u64) -> String {
    if ft == 0 {
        return "(not set)".to_string();
    }
    // 1601-01-01 → 1970-01-01 is 11,644,473,600 seconds.
    let unix = (ft / 10_000_000) as i64 - 11_644_473_600;
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// Replace control characters and strip trailing NULs so a registry string is
/// safe to drop into a single-line report.
fn sanitize(s: &str) -> String {
    s.trim_end_matches('\0')
        .chars()
        .map(|c| if c.is_control() { '.' } else { c })
        .collect()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max).collect();
        format!("{head}… ({} chars)", s.chars().count())
    }
}

/// Split a backslash- (or forward-slash-) separated registry path into its
/// components, tolerating leading/trailing/duplicated separators.
fn split_path(path: &str) -> Vec<&str> {
    path.split(['\\', '/'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Base block
// ---------------------------------------------------------------------------

/// The parsed 4096-byte base block, read independently of `regf` so that a hive
/// with a bad checksum or an unsupported version still yields a report.
struct Header {
    primary_sequence: u32,
    secondary_sequence: u32,
    last_written: u64,
    major_version: u32,
    minor_version: u32,
    file_type: u32,
    file_format: u32,
    root_cell_offset: u32,
    hive_bins_data_size: u32,
    clustering_factor: u32,
    file_name: String,
    flags: u32,
    stored_checksum: u32,
    computed_checksum: u32,
    total_len: usize,
}

impl Header {
    fn parse(b: &[u8]) -> Result<Self, String> {
        if b.is_empty() {
            return Err(
                "no input bytes: paste a registry hive as hex or Base64 (a hive begins with the \
                 ASCII signature \"regf\")"
                    .to_string(),
            );
        }
        if b.len() < 4 || &b[..4] != b"regf" {
            let n = b.len().min(4);
            let seen: String = b[..n]
                .iter()
                .map(|&c| if (0x20..0x7f).contains(&c) { c as char } else { '.' })
                .collect();
            return Err(format!(
                "not a registry hive: the first bytes are {} (\"{seen}\"), but a hive must start \
                 with the ASCII signature \"regf\" (72 65 67 66). If this is a .reg text export or \
                 a transaction log (.LOG1/.LOG2), it is not a primary hive.",
                to_hex(&b[..n])
            ));
        }
        if b.len() < BASE_BLOCK_SIZE {
            return Err(format!(
                "truncated hive: the base block is a fixed {BASE_BLOCK_SIZE} bytes but only {} \
                 byte(s) were supplied. Paste the whole file, not just its opening bytes.",
                b.len()
            ));
        }
        Ok(Header {
            primary_sequence: le_u32(b, 4),
            secondary_sequence: le_u32(b, 8),
            last_written: le_u64(b, 12),
            major_version: le_u32(b, 20),
            minor_version: le_u32(b, 24),
            file_type: le_u32(b, 28),
            file_format: le_u32(b, 32),
            root_cell_offset: le_u32(b, 36),
            hive_bins_data_size: le_u32(b, 40),
            clustering_factor: le_u32(b, 44),
            file_name: utf16le_lossy(&b[48..112]),
            flags: le_u32(b, 144),
            stored_checksum: le_u32(b, 508),
            computed_checksum: header_checksum(b),
            total_len: b.len(),
        })
    }

    fn checksum_ok(&self) -> bool {
        self.stored_checksum == self.computed_checksum
    }

    /// A hive is "dirty" when the two sequence numbers disagree: a write was
    /// interrupted and the matching `.LOG1`/`.LOG2` still has to be replayed.
    fn is_dirty(&self) -> bool {
        self.primary_sequence != self.secondary_sequence
    }

    fn file_type_name(&self) -> &'static str {
        match self.file_type {
            0 => "primary hive",
            1 => "transaction log (Windows XP+)",
            2 => "transaction log (Windows NT/2000)",
            6 => "transaction log (Windows 8.1+)",
            _ => "unrecognized",
        }
    }

    /// Guess which of the standard hives this is from the path Windows recorded
    /// in the header. Purely a convenience label — it is never used for parsing.
    fn likely_hive(&self) -> Option<&'static str> {
        let n = self.file_name.to_ascii_lowercase();
        let n = n.rsplit(['\\', '/']).next().unwrap_or(&n);
        Some(match n {
            x if x.contains("ntuser") => "NTUSER.DAT — a single user's HKCU",
            x if x.contains("usrclass") => "UsrClass.dat — HKCU\\Software\\Classes",
            x if x.contains("software") => "SOFTWARE — HKLM\\SOFTWARE",
            x if x.contains("system") => "SYSTEM — HKLM\\SYSTEM (services, control sets)",
            x if x.contains("security") => "SECURITY — HKLM\\SECURITY",
            x if x.contains("sam") => "SAM — local account database",
            x if x.contains("default") => "DEFAULT — HKU\\.DEFAULT",
            x if x.contains("components") => "COMPONENTS — servicing stack",
            x if x.contains("bcd") => "BCD — boot configuration data",
            _ => return None,
        })
    }

    fn render(&self) -> String {
        let mut out = String::new();
        let mut row = |label: &str, value: String| {
            out.push_str(&format!("  {label:<LABEL_W$}{value}\n"));
        };
        row("Signature", "regf (valid)".to_string());
        row(
            "Format version",
            format!("{}.{}", self.major_version, self.minor_version),
        );
        row(
            "File type",
            format!("{} ({})", self.file_type, self.file_type_name()),
        );
        row(
            "File format",
            format!(
                "{} ({})",
                self.file_format,
                if self.file_format == 1 { "direct memory load" } else { "unrecognized" }
            ),
        );
        row(
            "Embedded file name",
            if self.file_name.is_empty() {
                "(empty)".to_string()
            } else {
                sanitize(&self.file_name)
            },
        );
        if let Some(kind) = self.likely_hive() {
            row("Looks like", kind.to_string());
        }
        row("Primary sequence", self.primary_sequence.to_string());
        row("Secondary sequence", self.secondary_sequence.to_string());
        row(
            "State",
            if self.is_dirty() {
                "DIRTY — sequence numbers differ; a write was interrupted and the .LOG1/.LOG2 \
                 transaction log has not been replayed"
                    .to_string()
            } else {
                "clean (sequence numbers match)".to_string()
            },
        );
        row("Last written", format_filetime(self.last_written));
        row("Header flags", format!("0x{:08x}", self.flags));
        row("Root cell offset", format!("0x{:08x}", self.root_cell_offset));
        row(
            "Hive bins data size",
            format!("{} bytes", self.hive_bins_data_size),
        );
        row("Clustering factor", self.clustering_factor.to_string());
        row(
            "Header checksum",
            if self.checksum_ok() {
                format!("0x{:08x} (valid)", self.stored_checksum)
            } else {
                format!(
                    "0x{:08x} INVALID — recomputes to 0x{:08x}",
                    self.stored_checksum, self.computed_checksum
                )
            },
        );
        let expected = BASE_BLOCK_SIZE as u64 + self.hive_bins_data_size as u64;
        let size_note = match (self.total_len as u64).cmp(&expected) {
            std::cmp::Ordering::Equal => "matches the header".to_string(),
            std::cmp::Ordering::Less => format!(
                "TRUNCATED — the header declares {expected} bytes (4096 base block + {} bins)",
                self.hive_bins_data_size
            ),
            std::cmp::Ordering::Greater => format!(
                "{} byte(s) of slack past the declared {expected}",
                self.total_len as u64 - expected
            ),
        };
        row("Supplied size", format!("{} bytes — {size_note}", self.total_len));
        out
    }
}

// ---------------------------------------------------------------------------
// Raw carving (fallback when structured traversal is unavailable)
// ---------------------------------------------------------------------------

/// Carve `nk` (key node) records straight out of the raw bytes.
///
/// Each key node begins with the ASCII signature `nk`, and its name length sits
/// at a fixed offset 72 with the name itself at offset 76 — ASCII when the
/// `KEY_COMP_NAME` flag (0x0020) is set, UTF-16LE otherwise. Scanning for that
/// shape finds keys in a hive whose cell tree is too damaged to walk. It cannot
/// reconstruct the parent path or the attached values, which is why callers
/// label these results as carved.
fn carve_key_names(bytes: &[u8]) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    if bytes.len() <= BASE_BLOCK_SIZE {
        return found;
    }
    let mut i = BASE_BLOCK_SIZE;
    while i + 76 < bytes.len() {
        if &bytes[i..i + 2] != b"nk" {
            i += 1;
            continue;
        }
        let flags = le_u16(bytes, i + 2);
        let name_len = le_u16(bytes, i + 72) as usize;
        let start = i + 76;
        // A real key name is short and non-empty; anything else is a false hit
        // on the two ASCII bytes "nk" inside unrelated data.
        if name_len == 0 || name_len > 512 || start + name_len > bytes.len() {
            i += 1;
            continue;
        }
        let name = if flags & 0x0020 != 0 {
            String::from_utf8_lossy(&bytes[start..start + name_len]).to_string()
        } else {
            utf16le_lossy(&bytes[start..start + name_len])
        };
        if name.is_empty() || name.chars().any(|c| c.is_control()) {
            i += 1;
            continue;
        }
        found.push((i, name));
        i += 76 + name_len;
    }
    found
}

/// Count occurrences of a two-byte cell signature in the hive-bin area.
fn count_signature(bytes: &[u8], sig: &[u8; 2]) -> usize {
    if bytes.len() <= BASE_BLOCK_SIZE {
        return 0;
    }
    bytes[BASE_BLOCK_SIZE..]
        .windows(2)
        .filter(|w| w == sig)
        .count()
}

// ---------------------------------------------------------------------------
// Structured rendering
// ---------------------------------------------------------------------------

fn type_label(dt: &DataType) -> String {
    match dt {
        DataType::Unknown(n) => format!("REG_UNKNOWN(0x{n:08x})"),
        other => other.name().to_string(),
    }
}

fn render_registry_value(v: &RegistryValue) -> String {
    match v {
        RegistryValue::None => "(no data)".to_string(),
        RegistryValue::String(s) => {
            format!("\"{}\"", truncate_chars(&sanitize(s), STRING_PREVIEW_CHARS))
        }
        RegistryValue::MultiString(items) => {
            if items.is_empty() {
                "(empty multi-string)".to_string()
            } else {
                let joined = items
                    .iter()
                    .map(|s| format!("\"{}\"", sanitize(s)))
                    .collect::<Vec<_>>()
                    .join(", ");
                truncate_chars(&joined, STRING_PREVIEW_CHARS)
            }
        }
        RegistryValue::Binary(d) => {
            if d.len() <= BINARY_PREVIEW_BYTES {
                format!("{} ({} bytes)", to_hex(d), d.len())
            } else {
                format!(
                    "{}… ({} bytes)",
                    to_hex(&d[..BINARY_PREVIEW_BYTES]),
                    d.len()
                )
            }
        }
        RegistryValue::Dword(n) => format!("0x{n:08x} ({n})"),
        RegistryValue::DwordBigEndian(n) => format!("0x{n:08x} ({n}, big-endian)"),
        RegistryValue::Qword(n) => format!("0x{n:016x} ({n})"),
    }
}

fn render_value_entry(v: &RegistryValueEntry, indent: &str) -> String {
    let name = if v.is_default() {
        "(Default)".to_string()
    } else {
        sanitize(&v.name())
    };
    let ty = type_label(&v.data_type());
    let data = match v.data() {
        Ok(d) => render_registry_value(&d),
        Err(e) => format!("(unreadable: {e})"),
    };
    format!("{indent}{name}  [{ty}]  {data}\n")
}

/// List a key's values, capped at `max_entries`.
fn render_values(key: &RegistryKey, max_entries: usize, indent: &str) -> String {
    let mut out = String::new();
    let values = match key.values() {
        Ok(v) => v,
        Err(e) => return format!("{indent}(values unreadable: {e})\n"),
    };
    if values.is_empty() {
        out.push_str(&format!("{indent}(none)\n"));
        return out;
    }
    for v in values.iter().take(max_entries) {
        out.push_str(&render_value_entry(v, indent));
    }
    if values.len() > max_entries {
        out.push_str(&format!(
            "{indent}… {} more value(s) not shown — raise max_entries to see them\n",
            values.len() - max_entries
        ));
    }
    out
}

/// List a key's subkeys with their own child counts, capped at `max_entries`.
fn render_subkeys(key: &RegistryKey, max_entries: usize, indent: &str) -> String {
    let mut out = String::new();
    let subkeys = match key.subkeys() {
        Ok(v) => v,
        Err(e) => return format!("{indent}(subkeys unreadable: {e})\n"),
    };
    if subkeys.is_empty() {
        out.push_str(&format!("{indent}(none)\n"));
        return out;
    }
    for k in subkeys.iter().take(max_entries) {
        out.push_str(&format!(
            "{indent}{}  ({} subkeys, {} values)\n",
            sanitize(&k.name()),
            k.subkey_count(),
            k.value_count()
        ));
    }
    if subkeys.len() > max_entries {
        out.push_str(&format!(
            "{indent}… {} more subkey(s) not shown — raise max_entries to see them\n",
            subkeys.len() - max_entries
        ));
    }
    out
}

fn key_timestamp(key: &RegistryKey) -> String {
    match key.last_written() {
        Some(ts) => ts.to_string(),
        None => "(not set)".to_string(),
    }
}

/// Walk a backslash path one component at a time so a miss can name the exact
/// component that failed and show what was actually there.
fn open_path<'h>(hive: &'h RegistryHive, path: &str) -> Result<RegistryKey<'h>, String> {
    let mut key = hive
        .root_key()
        .map_err(|e| format!("cannot read the root key: {e}"))?;
    let mut walked: Vec<String> = Vec::new();
    for part in split_path(path) {
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
                    "key not found: {part:?} is not a subkey of {where_}.{available} Paths are \
                     relative to the hive root, so drop any HKLM\\/HKCU\\ prefix — in NTUSER.DAT \
                     the Run key is \"Software\\Microsoft\\Windows\\CurrentVersion\\Run\"."
                ));
            }
        }
    }
    Ok(key)
}

/// The `Key: ...` block shared by `path` mode and the root section of `summary`.
fn render_key_block(key: &RegistryKey, display_path: &str, max_entries: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!("Key: {display_path}\n"));
    out.push_str(&format!("  {:<LABEL_W$}{}\n", "Name", {
        let n = key.name();
        if n.is_empty() {
            "(root)".to_string()
        } else {
            sanitize(&n)
        }
    }));
    out.push_str(&format!(
        "  {:<LABEL_W$}{}\n",
        "Last written",
        key_timestamp(key)
    ));
    out.push_str(&format!(
        "  {:<LABEL_W$}{}\n",
        "Subkeys",
        key.subkey_count()
    ));
    out.push_str(&format!(
        "  {:<LABEL_W$}{}\n",
        "Values",
        key.value_count()
    ));
    if let Ok(Some(class)) = key.class_name() {
        out.push_str(&format!(
            "  {:<LABEL_W$}{}\n",
            "Class name",
            sanitize(&class)
        ));
    }
    out.push('\n');
    out.push_str("Values\n");
    out.push_str(&render_values(key, max_entries, "  "));
    out.push('\n');
    out.push_str("Subkeys\n");
    out.push_str(&render_subkeys(key, max_entries, "  "));
    out
}

// ---------------------------------------------------------------------------
// Modes
// ---------------------------------------------------------------------------

fn mode_summary(
    header: &Header,
    hive: Result<&RegistryHive, &str>,
    bytes: &[u8],
    max_entries: usize,
) -> String {
    let mut out = String::from("Windows registry hive — header and root key\n\n");
    out.push_str("Base block\n");
    out.push_str(&header.render());
    out.push('\n');

    match hive {
        Ok(hive) => match hive.root_key() {
            Ok(root) => {
                out.push_str(&render_key_block(&root, "(hive root)", max_entries));
            }
            Err(e) => {
                out.push_str(&degraded_note(&e.to_string()));
                out.push_str(&carve_report(bytes, max_entries));
            }
        },
        Err(e) => {
            out.push_str(&degraded_note(e));
            out.push_str(&carve_report(bytes, max_entries));
        }
    }
    out
}

fn mode_path(
    hive: Result<&RegistryHive, &str>,
    bytes: &[u8],
    path: &str,
    max_entries: usize,
) -> Result<String, String> {
    let components = split_path(path);
    let display = if components.is_empty() {
        "(hive root)".to_string()
    } else {
        components.join("\\")
    };
    match hive {
        Ok(hive) => match open_path(hive, path) {
            Ok(key) => Ok(render_key_block(&key, &display, max_entries)),
            Err(e) if e.contains("cannot read the root key") || e.contains("Invalid cell offset") => {
                let mut out = String::from("Windows registry hive — key lookup (degraded)\n\n");
                out.push_str(&degraded_note(&e));
                let leaf = components.last().copied().unwrap_or("");
                if leaf.is_empty() {
                    out.push_str(
                        "No key path was supplied, and the cell tree cannot be walked, so there is \
                         nothing to look up. Use mode=\"summary\" for the header report.\n",
                    );
                } else {
                    let carved = carve_key_names(bytes);
                    let hits: Vec<&(usize, String)> = carved
                        .iter()
                        .filter(|(_, n)| n.eq_ignore_ascii_case(leaf))
                        .collect();
                    if hits.is_empty() {
                        out.push_str(&format!(
                            "Carved key-node scan: no key named {leaf:?} is present anywhere in the \
                             raw bytes, so the path \"{display}\" is not in this hive (or its key node \
                             was overwritten). {} key node(s) were carved in total.\n",
                            carved.len()
                        ));
                    } else {
                        out.push_str(&format!(
                            "Carved key-node scan: {} key node(s) named {leaf:?} found in the raw \
                             bytes. A carved node cannot be tied back to its parent path or its \
                             values, so this confirms the key name exists but not that it sits at \
                             \"{display}\".\n",
                            hits.len()
                        ));
                        for (off, name) in hits.iter().take(max_entries) {
                            out.push_str(&format!("  0x{off:08x}  {name}\n"));
                        }
                        if hits.len() > max_entries {
                            out.push_str(&format!(
                                "  … {} more not shown\n",
                                hits.len() - max_entries
                            ));
                        }
                    }
                }
                Ok(out)
            }
            Err(e) => Err(e),
        },
        Err(e) => {
            let mut out = String::from("Windows registry hive — key lookup (degraded)\n\n");
            out.push_str(&degraded_note(e));
            let leaf = components.last().copied().unwrap_or("");
            if leaf.is_empty() {
                out.push_str(
                    "No key path was supplied, and the cell tree cannot be walked, so there is \
                     nothing to look up. Use mode=\"summary\" for the header report.\n",
                );
            } else {
                let carved = carve_key_names(bytes);
                let hits: Vec<&(usize, String)> = carved
                    .iter()
                    .filter(|(_, n)| n.eq_ignore_ascii_case(leaf))
                    .collect();
                if hits.is_empty() {
                    out.push_str(&format!(
                        "Carved key-node scan: no key named {leaf:?} is present anywhere in the \
                         raw bytes, so the path \"{display}\" is not in this hive (or its key node \
                         was overwritten). {} key node(s) were carved in total.\n",
                        carved.len()
                    ));
                } else {
                    out.push_str(&format!(
                        "Carved key-node scan: {} key node(s) named {leaf:?} found in the raw \
                         bytes. A carved node cannot be tied back to its parent path or its \
                         values, so this confirms the key name exists but not that it sits at \
                         \"{display}\".\n",
                        hits.len()
                    ));
                    for (off, name) in hits.iter().take(max_entries) {
                        out.push_str(&format!("  0x{off:08x}  {name}\n"));
                    }
                    if hits.len() > max_entries {
                        out.push_str(&format!(
                            "  … {} more not shown\n",
                            hits.len() - max_entries
                        ));
                    }
                }
            }
            Ok(out)
        }
    }
}

fn mode_runkeys(
    hive: Result<&RegistryHive, &str>,
    bytes: &[u8],
    max_entries: usize,
) -> Result<String, String> {
    let hive = match hive {
        Ok(h) => h,
        Err(e) => {
            let mut out = String::from("Windows registry hive — autostart sweep (degraded)\n\n");
            out.push_str(&degraded_note(e));
            let carved = carve_key_names(bytes);
            let hits: Vec<&(usize, String)> = carved
                .iter()
                .filter(|(_, n)| RUN_KEY_LEAF_NAMES.iter().any(|k| n.eq_ignore_ascii_case(k)))
                .collect();
            if hits.is_empty() {
                out.push_str(&format!(
                    "Carved key-node scan: none of the {} autostart key names ({}) appear among \
                     the {} key node(s) carved from the raw bytes. No autostart key is present in \
                     what was supplied.\n",
                    RUN_KEY_LEAF_NAMES.len(),
                    RUN_KEY_LEAF_NAMES.join(", "),
                    carved.len()
                ));
            } else {
                out.push_str(&format!(
                    "Carved key-node scan: {} autostart key node(s) found among {} carved key \
                     nodes. A carved node cannot be tied back to its parent path or its values, so \
                     treat these as leads, not as a value listing.\n\n",
                    hits.len(),
                    carved.len()
                ));
                for (off, name) in hits.iter().take(max_entries) {
                    out.push_str(&format!("  0x{off:08x}  {name}\n"));
                }
                if hits.len() > max_entries {
                    out.push_str(&format!("  … {} more not shown\n", hits.len() - max_entries));
                }
            }
            return Ok(out);
        }
    };

    if let Err(e) = hive.root_key() {
        let mut out = String::from("Windows registry hive — autostart sweep (degraded)\n\n");
        out.push_str(&degraded_note(&e.to_string()));
        let carved = carve_key_names(bytes);
        let hits: Vec<&(usize, String)> = carved
            .iter()
            .filter(|(_, n)| RUN_KEY_LEAF_NAMES.iter().any(|k| n.eq_ignore_ascii_case(k)))
            .collect();
        if hits.is_empty() {
            out.push_str(&format!(
                "Carved key-node scan: none of the {} autostart key names ({}) appear among \
                 the {} key node(s) carved from the raw bytes. No autostart key is present in \
                 what was supplied.\n",
                RUN_KEY_LEAF_NAMES.len(),
                RUN_KEY_LEAF_NAMES.join(", "),
                carved.len()
            ));
        } else {
            out.push_str(&format!(
                "Carved key-node scan: {} autostart key node(s) found among {} carved key \
                 nodes. A carved node cannot be tied back to its parent path or its values, so \
                 treat these as leads, not as a value listing.\n\n",
                hits.len(),
                carved.len()
            ));
            for (off, name) in hits.iter().take(max_entries) {
                out.push_str(&format!("  0x{off:08x}  {name}\n"));
            }
            if hits.len() > max_entries {
                out.push_str(&format!("  … {} more not shown\n", hits.len() - max_entries));
            }
        }
        return Ok(out);
    }

    let mut present: Vec<(&str, &str, RegistryKey)> = Vec::new();
    let mut absent: Vec<&str> = Vec::new();
    for (path, note) in RUN_KEY_CANDIDATES {
        match open_path(hive, path) {
            Ok(key) => present.push((path, note, key)),
            Err(_) => absent.push(path),
        }
    }

    let mut out = format!(
        "Windows registry hive — autostart sweep ({} of {} well-known locations present)\n\n",
        present.len(),
        RUN_KEY_CANDIDATES.len()
    );

    if present.is_empty() {
        out.push_str(
            "None of the well-known autostart locations exist in this hive.\n\n\
             The probed paths are relative to the hive root — a hive has no HKLM\\ or HKCU\\ \
             prefix — so this normally means the file is not NTUSER.DAT, SOFTWARE or SYSTEM \
             (SAM, SECURITY, UsrClass.dat and BCD hold no Run keys), or the autostart keys were \
             genuinely never created. The root subkeys below show what this hive actually \
             contains.\n\n",
        );
        match hive.root_key() {
            Ok(root) => {
                out.push_str("Root subkeys\n");
                out.push_str(&render_subkeys(&root, max_entries, "  "));
            }
            Err(e) => out.push_str(&format!("Root subkeys could not be listed: {e}\n")),
        }
        out.push('\n');
    } else {
        for (path, note, key) in &present {
            out.push_str(&format!("{path}\n"));
            out.push_str(&format!("  ({note})\n"));
            out.push_str(&format!(
                "  last written {} — {} value(s), {} subkey(s)\n",
                key_timestamp(key),
                key.value_count(),
                key.subkey_count()
            ));
            out.push_str(&render_values(key, max_entries, "    "));
            out.push('\n');
        }
    }

    out.push_str(&format!("Not present in this hive ({})\n", absent.len()));
    if absent.is_empty() {
        out.push_str("  (none — every probed location exists)\n");
    } else {
        for path in absent.iter().take(max_entries) {
            out.push_str(&format!("  {path}\n"));
        }
        if absent.len() > max_entries {
            out.push_str(&format!(
                "  … {} more not shown — raise max_entries to see them\n",
                absent.len() - max_entries
            ));
        }
    }
    out.push_str(
        "\nOnly the fixed list of well-known autostart locations above is probed; this is not an \
         exhaustive persistence hunt. Use mode=\"path\" to inspect any other key.\n",
    );
    Ok(out)
}

fn degraded_note(reason: &str) -> String {
    format!(
        "NOTE: structured traversal is unavailable — {reason}.\n\
         The 4096-byte base block above was parsed independently, so the header report is still \
         accurate. Below is a raw scan of the hive-bin area instead of a real key tree: key names \
         are carved by matching the `nk` cell signature, which recovers names but cannot \
         reconstruct parent paths or attach values.\n\n"
    )
}

fn carve_report(bytes: &[u8], max_entries: usize) -> String {
    let carved = carve_key_names(bytes);
    let mut out = String::from("Raw cell scan\n");
    out.push_str(&format!(
        "  {:<LABEL_W$}{}\n",
        "Key nodes (nk) carved",
        carved.len()
    ));
    out.push_str(&format!(
        "  {:<LABEL_W$}{}\n",
        "Value records (vk)",
        count_signature(bytes, b"vk")
    ));
    out.push_str(&format!(
        "  {:<LABEL_W$}{}\n",
        "Hive bins (hbin)",
        count_signature(bytes, b"hb")
    ));
    out.push('\n');
    out.push_str("Carved key names\n");
    if carved.is_empty() {
        out.push_str("  (none — no intact `nk` records were found in the hive-bin area)\n");
    } else {
        for (off, name) in carved.iter().take(max_entries) {
            out.push_str(&format!("  0x{off:08x}  {name}\n"));
        }
        if carved.len() > max_entries {
            out.push_str(&format!(
                "  … {} more not shown — raise max_entries to see them\n",
                carved.len() - max_entries
            ));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Parse a Windows registry hive and report on it.
///
/// * `data` — the hive file bytes, encoded per `input_encoding`.
/// * `input_encoding` — `"hex"` (default) or `"base64"`.
/// * `mode` — `"summary"` (default), `"path"`, or `"runkeys"`.
/// * `path` — for `mode="path"`, a backslash-separated key path relative to the
///   hive root (no `HKLM\`/`HKCU\` prefix). Ignored by the other modes.
/// * `max_entries` — cap on the entries listed per section; 0 means the default
///   of 50, and anything above 1000 is clamped.
pub fn run(
    data: &str,
    input_encoding: &str,
    mode: &str,
    path: &str,
    max_entries: usize,
) -> Result<String, String> {
    let mode = Mode::parse(mode)?;
    let fmt = InFmt::parse(input_encoding)?;
    let max_entries = match max_entries {
        0 => DEFAULT_MAX_ENTRIES,
        n => n.min(MAX_MAX_ENTRIES),
    };
    let bytes = fmt.to_bytes(data)?;
    let header = Header::parse(&bytes)?;

    // Structured traversal is best-effort: `regf` rejects a hive whose header
    // checksum is wrong or whose version predates 1.3, and those are exactly the
    // hives a forensic user most wants a report on.
    let parsed = RegistryHive::from_bytes(bytes.clone());
    let hive_ref: Result<&RegistryHive, String> = match &parsed {
        Ok(h) => Ok(h),
        Err(e) => Err(e.to_string()),
    };
    let hive_ref: Result<&RegistryHive, &str> = hive_ref.as_ref().map(|h| *h).map_err(|s| s.as_str());

    match mode {
        Mode::Summary => Ok(mode_summary(&header, hive_ref, &bytes, max_entries)),
        Mode::Path => mode_path(hive_ref, &bytes, path, max_entries),
        Mode::RunKeys => mode_runkeys(hive_ref, &bytes, max_entries),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regf::structures::DataType;
    use regf::writer::HiveBuilder;

    fn utf16z(s: &str) -> Vec<u8> {
        let mut v: Vec<u8> = s.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        v.extend_from_slice(&[0, 0]);
        v
    }

    /// A synthetic NTUSER.DAT-shaped hive: a Run key with two entries, a
    /// RunOnce key, and a DWORD elsewhere. Built with `regf`'s own writer so the
    /// bytes are a genuine well-formed hive, checksum included.
    fn fixture_hive() -> Vec<u8> {
        let mut b = HiveBuilder::new();
        let root = b.root_offset();
        let software = b.add_key(root, "Software").unwrap();
        let ms = b.add_key(software, "Microsoft").unwrap();
        let windows = b.add_key(ms, "Windows").unwrap();
        let cv = b.add_key(windows, "CurrentVersion").unwrap();

        let run = b.add_key(cv, "Run").unwrap();
        b.add_value(
            run,
            "OneDrive",
            DataType::String,
            &utf16z("C:\\Users\\alice\\OneDrive.exe /background"),
        )
        .unwrap();
        b.add_value(
            run,
            "Updater",
            DataType::ExpandString,
            &utf16z("%APPDATA%\\upd.exe"),
        )
        .unwrap();

        let run_once = b.add_key(cv, "RunOnce").unwrap();
        b.add_value(run_once, "Cleanup", DataType::String, &utf16z("cleanup.exe"))
            .unwrap();

        let explorer = b.add_key(cv, "Explorer").unwrap();
        b.add_value(explorer, "Serial", DataType::Dword, &0xdeadu32.to_le_bytes())
            .unwrap();

        b.to_bytes().unwrap()
    }

    fn fixture_hex() -> String {
        to_hex(&fixture_hive())
    }

    // ---- happy paths -----------------------------------------------------

    #[test]
    fn summary_reports_header_and_root() {
        let out = run(&fixture_hex(), "hex", "summary", "", 50).unwrap();
        assert!(out.contains("Signature             regf (valid)"), "{out}");
        assert!(out.contains("Header checksum"), "{out}");
        assert!(out.contains("(valid)"), "{out}");
        assert!(out.contains("clean (sequence numbers match)"), "{out}");
        assert!(out.contains("Key: (hive root)"), "{out}");
        assert!(out.contains("Software"), "{out}");
        // The header report must never claim a traversal failure on a good hive.
        assert!(!out.contains("NOTE: structured traversal"), "{out}");
    }

    #[test]
    fn summary_accepts_base64_input() {
        let b64 = B64.encode(fixture_hive());
        let out = run(&b64, "base64", "summary", "", 50).unwrap();
        assert!(out.contains("regf (valid)"), "{out}");
        assert!(out.contains("Key: (hive root)"), "{out}");
    }

    #[test]
    fn path_lists_values_and_types() {
        let out = run(
            &fixture_hex(),
            "hex",
            "path",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            50,
        )
        .unwrap();
        assert!(out.contains("Key: Software\\Microsoft\\Windows\\CurrentVersion\\Run"), "{out}");
        assert!(out.contains("OneDrive  [REG_SZ]"), "{out}");
        assert!(out.contains("OneDrive.exe /background"), "{out}");
        assert!(out.contains("Updater  [REG_EXPAND_SZ]"), "{out}");
        assert!(out.contains("%APPDATA%\\upd.exe"), "{out}");
    }

    #[test]
    fn path_tolerates_slashes_and_stray_separators() {
        let a = run(&fixture_hex(), "hex", "path", "Software\\Microsoft", 50).unwrap();
        let b = run(&fixture_hex(), "hex", "path", "\\Software/Microsoft\\\\", 50).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn path_renders_dword_values() {
        let out = run(
            &fixture_hex(),
            "hex",
            "path",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer",
            50,
        )
        .unwrap();
        assert!(out.contains("Serial  [REG_DWORD]  0x0000dead (57005)"), "{out}");
    }

    #[test]
    fn path_empty_shows_the_root() {
        let out = run(&fixture_hex(), "hex", "path", "", 50).unwrap();
        assert!(out.contains("Key: (hive root)"), "{out}");
        assert!(out.contains("Software"), "{out}");
    }

    #[test]
    fn runkeys_finds_the_user_run_key() {
        let out = run(&fixture_hex(), "hex", "runkeys", "", 50).unwrap();
        assert!(out.contains("Software\\Microsoft\\Windows\\CurrentVersion\\Run\n"), "{out}");
        assert!(out.contains("NTUSER.DAT — per-user autostart"), "{out}");
        assert!(out.contains("OneDrive"), "{out}");
        assert!(out.contains("Cleanup"), "{out}");
        // Locations that genuinely are not in the hive are reported as absent.
        assert!(out.contains("Not present in this hive"), "{out}");
        assert!(out.contains("Microsoft\\Windows NT\\CurrentVersion\\Winlogon"), "{out}");
    }

    #[test]
    fn runkeys_says_so_when_no_autostart_path_exists() {
        let mut b = HiveBuilder::new();
        let root = b.root_offset();
        b.add_key(root, "SAM").unwrap();
        let out = run(&to_hex(&b.to_bytes().unwrap()), "hex", "runkeys", "", 50).unwrap();
        assert!(out.contains("0 of 25 well-known locations present"), "{out}");
        assert!(
            out.contains("None of the well-known autostart locations exist in this hive"),
            "{out}"
        );
        assert!(out.contains("Root subkeys"), "{out}");
        assert!(out.contains("SAM"), "{out}");
    }

    #[test]
    fn max_entries_caps_each_listing_and_defaults_when_zero() {
        let capped = run(&fixture_hex(), "hex", "runkeys", "", 1).unwrap();
        assert!(capped.contains("more value(s) not shown"), "{capped}");
        assert!(capped.contains("more not shown"), "{capped}");
        // 0 means "use the default", not "show nothing".
        let defaulted = run(&fixture_hex(), "hex", "summary", "", 0).unwrap();
        assert!(defaulted.contains("Software"), "{defaulted}");
        // Above the ceiling is clamped rather than rejected.
        assert!(run(&fixture_hex(), "hex", "summary", "", usize::MAX).is_ok());
    }

    // ---- degraded / carved paths ----------------------------------------

    /// Corrupt the root cell offset so the base block still parses but `regf`
    /// cannot walk the cell tree.
    fn hive_with_broken_tree() -> Vec<u8> {
        let mut bytes = fixture_hive();
        bytes[36..40].copy_from_slice(&0x0080_0000u32.to_le_bytes());
        let sum = header_checksum(&bytes);
        bytes[508..512].copy_from_slice(&sum.to_le_bytes());
        bytes
    }

    #[test]
    fn summary_still_reports_header_when_traversal_fails() {
        let out = run(&to_hex(&hive_with_broken_tree()), "hex", "summary", "", 50).unwrap();
        assert!(out.contains("regf (valid)"), "{out}");
        assert!(out.contains("Root cell offset      0x00800000"), "{out}");
        assert!(out.contains("NOTE: structured traversal is unavailable"), "{out}");
        assert!(out.contains("Key nodes (nk) carved"), "{out}");
        assert!(out.contains("CurrentVersion"), "{out}");
    }

    #[test]
    fn summary_survives_a_bad_header_checksum() {
        let mut bytes = fixture_hive();
        bytes[508..512].copy_from_slice(&0xdead_beefu32.to_le_bytes());
        let out = run(&to_hex(&bytes), "hex", "summary", "", 50).unwrap();
        assert!(out.contains("0xdeadbeef INVALID — recomputes to 0x"), "{out}");
        assert!(out.contains("NOTE: structured traversal is unavailable"), "{out}");
    }

    #[test]
    fn dirty_hive_is_flagged() {
        let mut bytes = fixture_hive();
        bytes[8..12].copy_from_slice(&99u32.to_le_bytes()); // secondary sequence
        let sum = header_checksum(&bytes);
        bytes[508..512].copy_from_slice(&sum.to_le_bytes());
        let out = run(&to_hex(&bytes), "hex", "summary", "", 50).unwrap();
        assert!(out.contains("DIRTY — sequence numbers differ"), "{out}");
    }

    #[test]
    fn truncated_hive_body_is_reported_as_truncated() {
        let mut bytes = fixture_hive();
        bytes.truncate(BASE_BLOCK_SIZE + 64);
        let sum = header_checksum(&bytes);
        bytes[508..512].copy_from_slice(&sum.to_le_bytes());
        let out = run(&to_hex(&bytes), "hex", "summary", "", 50).unwrap();
        assert!(out.contains("TRUNCATED — the header declares"), "{out}");
    }

    #[test]
    fn runkeys_falls_back_to_carving() {
        let out = run(&to_hex(&hive_with_broken_tree()), "hex", "runkeys", "", 50).unwrap();
        assert!(out.contains("autostart sweep (degraded)"), "{out}");
        assert!(out.contains("Carved key-node scan"), "{out}");
        assert!(out.contains("Run"), "{out}");
        assert!(out.contains("cannot be tied back to its parent path"), "{out}");
    }

    #[test]
    fn path_falls_back_to_carving_and_is_honest_about_a_miss() {
        let hive = to_hex(&hive_with_broken_tree());
        let hit = run(&hive, "hex", "path", "Software\\Microsoft", 50).unwrap();
        assert!(hit.contains("key lookup (degraded)"), "{hit}");
        assert!(hit.contains("key node(s) named \"Microsoft\" found"), "{hit}");

        let miss = run(&hive, "hex", "path", "Software\\NoSuchKeyHere", 50).unwrap();
        assert!(
            miss.contains("no key named \"NoSuchKeyHere\" is present anywhere in the raw bytes"),
            "{miss}"
        );
    }

    #[test]
    fn carving_recovers_key_names_from_a_good_hive() {
        let names: Vec<String> = carve_key_names(&fixture_hive())
            .into_iter()
            .map(|(_, n)| n)
            .collect();
        assert!(names.iter().any(|n| n == "Run"), "{names:?}");
        assert!(names.iter().any(|n| n == "CurrentVersion"), "{names:?}");
    }

    // ---- error paths -----------------------------------------------------

    #[test]
    fn rejects_a_non_regf_file() {
        let err = run("504b0304140000000800", "hex", "summary", "", 50).unwrap_err();
        assert!(err.contains("not a registry hive"), "{err}");
        assert!(err.contains("\"regf\""), "{err}");
    }

    #[test]
    fn rejects_a_hive_shorter_than_the_base_block() {
        let err = run("72656766 01000000", "hex", "summary", "", 50).unwrap_err();
        assert!(err.contains("truncated hive"), "{err}");
        assert!(err.contains("4096"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = run("", "hex", "summary", "", 50).unwrap_err();
        assert!(err.contains("no input bytes"), "{err}");
    }

    #[test]
    fn rejects_bad_hex_and_base64() {
        assert!(run("726", "hex", "summary", "", 50)
            .unwrap_err()
            .contains("odd number of digits"));
        assert!(run("7z65", "hex", "summary", "", 50)
            .unwrap_err()
            .contains("invalid hex byte"));
        assert!(run("!!!!", "base64", "summary", "", 50)
            .unwrap_err()
            .contains("invalid Base64 input"));
    }

    #[test]
    fn rejects_unknown_mode_and_encoding() {
        let err = run(&fixture_hex(), "hex", "tree", "", 50).unwrap_err();
        assert!(err.contains("invalid mode \"tree\""), "{err}");
        let err = run(&fixture_hex(), "utf8", "summary", "", 50).unwrap_err();
        assert!(err.contains("invalid input_encoding \"utf8\""), "{err}");
    }

    #[test]
    fn missing_path_names_the_failing_component() {
        let err = run(&fixture_hex(), "hex", "path", "Software\\Nope\\Deeper", 50).unwrap_err();
        assert!(err.contains("key not found: \"Nope\""), "{err}");
        assert!(err.contains("is not a subkey of \"Software\""), "{err}");
        assert!(err.contains("Subkeys present there: Microsoft"), "{err}");
        assert!(err.contains("drop any HKLM\\/HKCU\\ prefix"), "{err}");
    }

    // ---- helpers ---------------------------------------------------------

    #[test]
    fn filetime_formats_as_utc() {
        // 1601-01-01T00:00:00Z + 0 ticks is the epoch itself.
        assert_eq!(format_filetime(0), "(not set)");
        assert_eq!(format_filetime(1), "1601-01-01 00:00:00 UTC");
        // 2024-03-11T08:42:19Z = 1710146539 unix.
        let ft = (1_710_146_539u64 + 11_644_473_600) * 10_000_000;
        assert_eq!(format_filetime(ft), "2024-03-11 08:42:19 UTC");
    }

    #[test]
    fn checksum_matches_the_regf_rule() {
        let bytes = fixture_hive();
        assert_eq!(header_checksum(&bytes), le_u32(&bytes, 508));
        // The two documented special cases never return the reserved words.
        let zeros = vec![0u8; BASE_BLOCK_SIZE];
        assert_eq!(header_checksum(&zeros), 1);
        let mut ones = vec![0u8; BASE_BLOCK_SIZE];
        ones[..4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert_eq!(header_checksum(&ones), 0xFFFF_FFFE);
    }

    #[test]
    fn binary_and_multi_string_values_are_elided_not_dumped() {
        let big = RegistryValue::Binary(vec![0xabu8; 4096]);
        let rendered = render_registry_value(&big);
        assert!(rendered.ends_with("… (4096 bytes)"), "{rendered}");
        assert!(rendered.len() < 200, "{rendered}");

        let multi = RegistryValue::MultiString(vec!["a".into(), "b".into()]);
        assert_eq!(render_registry_value(&multi), "\"a\", \"b\"");
    }

    #[test]
    fn control_characters_are_neutralized() {
        assert_eq!(sanitize("a\u{0}b\nc\0\0"), "a.b.c");
    }
}
