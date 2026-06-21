//! gizza-ai/strings core — extract printable string sequences from binary data,
//! like the Unix `strings` command. Pure-Rust, dependency-free.
//!
//! Finds runs of printable characters at least `min_len` long. Supports ASCII,
//! UTF-16LE and UTF-16BE (basic-latin) scanning.

/// Which encodings to scan for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Ascii,
    Utf16,
    All,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "ascii" | "" => Ok(Encoding::Ascii),
            "utf16" | "unicode" => Ok(Encoding::Utf16),
            "all" | "both" => Ok(Encoding::All),
            other => Err(format!("unknown encoding '{other}' (use 'ascii', 'utf16', or 'all')")),
        }
    }
}

/// Cap on the number of strings returned (keeps output bounded for huge inputs).
pub const MAX_STRINGS: usize = 100_000;

/// Extracted strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub strings: Vec<String>,
    pub count: usize,
    /// True if the result was capped at `MAX_STRINGS`.
    pub truncated: bool,
}

fn is_printable(b: u8) -> bool {
    b == b'\t' || (0x20..=0x7e).contains(&b)
}

fn scan_ascii(data: &[u8], min_len: usize, out: &mut Vec<String>) {
    let mut run: Vec<u8> = Vec::new();
    for &b in data {
        if is_printable(b) {
            run.push(b);
        } else {
            if run.len() >= min_len {
                out.push(String::from_utf8_lossy(&run).into_owned());
            }
            run.clear();
        }
        if out.len() >= MAX_STRINGS {
            return;
        }
    }
    if run.len() >= min_len {
        out.push(String::from_utf8_lossy(&run).into_owned());
    }
}

/// Scan UTF-16 where each char is a printable low byte plus a zero high byte.
/// `lo_first = true` for little-endian (byte, 0x00), false for big-endian.
fn scan_utf16(data: &[u8], min_len: usize, lo_first: bool, out: &mut Vec<String>) {
    let mut run: Vec<u8> = Vec::new();
    let mut i = 0;
    while i + 1 < data.len() {
        let (lo, hi) = if lo_first {
            (data[i], data[i + 1])
        } else {
            (data[i + 1], data[i])
        };
        if hi == 0 && is_printable(lo) {
            run.push(lo);
        } else {
            if run.len() >= min_len {
                out.push(String::from_utf8_lossy(&run).into_owned());
            }
            run.clear();
        }
        if out.len() >= MAX_STRINGS {
            return;
        }
        i += 2;
    }
    if run.len() >= min_len {
        out.push(String::from_utf8_lossy(&run).into_owned());
    }
}

/// Extract printable strings from `data`.
pub fn extract(data: &[u8], min_len: usize, encoding: Encoding) -> Found {
    let min_len = min_len.max(1);
    let mut strings: Vec<String> = Vec::new();

    match encoding {
        Encoding::Ascii => scan_ascii(data, min_len, &mut strings),
        Encoding::Utf16 => {
            scan_utf16(data, min_len, true, &mut strings);
            scan_utf16(data, min_len, false, &mut strings);
        }
        Encoding::All => {
            scan_ascii(data, min_len, &mut strings);
            scan_utf16(data, min_len, true, &mut strings);
            scan_utf16(data, min_len, false, &mut strings);
        }
    }

    let truncated = strings.len() >= MAX_STRINGS;
    strings.truncate(MAX_STRINGS);
    let count = strings.len();
    Found { strings, count, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_basic() {
        let data = b"\x00\x01Hello\x00\x02World!\xff";
        let f = extract(data, 4, Encoding::Ascii);
        assert_eq!(f.strings, vec!["Hello".to_string(), "World!".to_string()]);
        assert_eq!(f.count, 2);
        assert!(!f.truncated);
    }

    #[test]
    fn min_len_filters() {
        let data = b"ab\x00abcd\x00xyzzy";
        // min_len 4 drops "ab" (2 chars), keeps "abcd" and "xyzzy".
        let f = extract(data, 4, Encoding::Ascii);
        assert_eq!(f.strings, vec!["abcd".to_string(), "xyzzy".to_string()]);
        // min_len 6 keeps neither.
        assert!(extract(data, 6, Encoding::Ascii).strings.is_empty());
    }

    #[test]
    fn utf16le_and_be() {
        // "Hi" in UTF-16LE: 48 00 69 00
        let le = [0x48, 0x00, 0x69, 0x00];
        let f = extract(&le, 2, Encoding::Utf16);
        assert!(f.strings.contains(&"Hi".to_string()));
        // "Hi" in UTF-16BE: 00 48 00 69
        let be = [0x00, 0x48, 0x00, 0x69];
        let f2 = extract(&be, 2, Encoding::Utf16);
        assert!(f2.strings.contains(&"Hi".to_string()));
    }

    #[test]
    fn all_includes_ascii_and_utf16() {
        // ASCII "test" then a UTF-16LE "ok".
        let mut data = b"test\x00".to_vec();
        data.extend_from_slice(&[0x6f, 0x00, 0x6b, 0x00]); // "ok" LE
        let f = extract(&data, 2, Encoding::All);
        assert!(f.strings.contains(&"test".to_string()));
        assert!(f.strings.contains(&"ok".to_string()));
    }

    #[test]
    fn tab_is_printable() {
        let f = extract(b"a\tbc\x00", 4, Encoding::Ascii);
        assert_eq!(f.strings, vec!["a\tbc".to_string()]);
    }

    #[test]
    fn empty_input() {
        assert!(extract(b"", 4, Encoding::All).strings.is_empty());
    }
}
