//! gizza-ai/csv-pii-redactor core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Column-scoped PII redaction over a CSV: for the columns you name (or address by
//! 1-based index), replace every data cell using one of three deterministic modes:
//!
//! - **mask**  — replace each character with `mask_char` (length preserved), optionally
//!   leaving the last `keep_last` characters visible (e.g. a card → `************1111`).
//! - **hash**  — salted SHA-256: `hex(SHA256(salt || value))` truncated to `hash_length`
//!   hex chars. Deterministic, so equal inputs map to equal codes (values stay joinable),
//!   while the salt defeats naive rainbow-table reversal.
//! - **redact** — replace the whole cell with the fixed `label` string.
//!
//! Non-selected columns and the header row pass through unchanged. Distinct from
//! `redact-pii` / `pii-tokenize`, which operate on free TEXT rather than chosen columns.

use sha2::{Digest, Sha256};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Redaction mode applied to every selected column's data cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Replace each character with `mask_char`, keeping the last `keep_last` visible.
    Mask,
    /// Replace with a salted SHA-256 hex code, truncated to `hash_length`.
    Hash,
    /// Replace the whole cell with a fixed `label`.
    Redact,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mask" | "" => Ok(Mode::Mask),
            "hash" => Ok(Mode::Hash),
            "redact" => Ok(Mode::Redact),
            other => Err(format!(
                "unknown mode '{other}' (use 'mask', 'hash', or 'redact')"
            )),
        }
    }
}

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Resolve the comma-separated `columns` spec into a per-column selected flag. When a
/// `header` record is present each entry is matched against a header name first, then
/// treated as a 1-based index if no name matches; without a header, only 1-based
/// indices are accepted.
fn resolve_columns(
    spec: &str,
    header: Option<&csv::StringRecord>,
    width: usize,
) -> Result<Vec<bool>, String> {
    let mut selected = vec![false; width];
    let mut any = false;
    for raw in spec.split(',') {
        let key = raw.trim();
        if key.is_empty() {
            continue;
        }
        any = true;
        let mut hit = None;
        if let Some(hdr) = header {
            for (i, name) in hdr.iter().enumerate() {
                if name.trim() == key {
                    hit = Some(i);
                    break;
                }
            }
        }
        let idx = match hit {
            Some(i) => i,
            None => {
                let n: usize = key.parse().map_err(|_| {
                    if header.is_some() {
                        format!("no column named '{key}' and it is not a valid index")
                    } else {
                        format!("column must be a 1-based index, got '{key}'")
                    }
                })?;
                if n == 0 || n > width {
                    return Err(format!(
                        "column index {n} out of range (file has {width} column(s))"
                    ));
                }
                n - 1
            }
        };
        selected[idx] = true;
    }
    if !any {
        return Err("no columns specified".into());
    }
    Ok(selected)
}

fn mask_value(value: &str, mask_char: char, keep_last: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    let n = chars.len();
    let keep = keep_last.min(n);
    let hidden = n - keep;
    let mut out = String::with_capacity(value.len());
    for (i, c) in chars.into_iter().enumerate() {
        if i < hidden {
            out.push(mask_char);
        } else {
            out.push(c);
        }
    }
    out
}

fn hash_value(value: &str, salt: &str, hash_length: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest.iter() {
        hex.push(char::from(HEX[(b >> 4) as usize]));
        hex.push(char::from(HEX[(b & 0x0f) as usize]));
    }
    hex.truncate(hash_length.clamp(4, 64));
    hex
}

/// Options for a redaction pass (mode-specific fields are ignored for other modes).
#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    /// Character used in `mask` mode (single char).
    pub mask_char: char,
    /// Trailing characters left visible in `mask` mode.
    pub keep_last: usize,
    /// Salt prepended before hashing in `hash` mode.
    pub salt: String,
    /// Hex length kept in `hash` mode (clamped to 4..=64).
    pub hash_length: usize,
    /// Replacement string in `redact` mode.
    pub label: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Mask,
            mask_char: '*',
            keep_last: 0,
            salt: String::new(),
            hash_length: 8,
            label: "[REDACTED]".to_string(),
        }
    }
}

/// Redact the named/indexed `columns` of `data` using `opts`. Non-selected columns and
/// the header row (when `header` is true) are passed through unchanged. Returns the
/// rewritten CSV text (same delimiter in and out).
pub fn redact_csv(
    data: &str,
    columns: &str,
    header: bool,
    delimiter: &str,
    opts: &Options,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("no rows found".into());
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);

    let header_rec = if header { records.first() } else { None };
    let selected = resolve_columns(columns, header_rec, width)?;

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    for (i, rec) in records.iter().enumerate() {
        let is_header = header && i == 0;
        let fields: Vec<String> = rec
            .iter()
            .enumerate()
            .map(|(col, cell)| {
                if is_header || !selected.get(col).copied().unwrap_or(false) {
                    cell.to_string()
                } else {
                    match opts.mode {
                        Mode::Mask => mask_value(cell, opts.mask_char, opts.keep_last),
                        Mode::Hash => hash_value(cell, &opts.salt, opts.hash_length),
                        Mode::Redact => opts.label.clone(),
                    }
                }
            })
            .collect();
        wtr.write_record(&fields)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: Mode) -> Options {
        Options {
            mode,
            ..Options::default()
        }
    }

    #[test]
    fn masks_selected_column_by_name() {
        let d = "name,email\nAda,ada@example.com\nBob,bob@corp.io";
        let out = redact_csv(d, "email", true, ",", &opts(Mode::Mask)).unwrap();
        assert_eq!(out, "name,email\nAda,***************\nBob,***********\n");
    }

    #[test]
    fn mask_keep_last_shows_tail() {
        let d = "id,card\n1,4111111111111111";
        let o = Options {
            mode: Mode::Mask,
            keep_last: 4,
            ..Options::default()
        };
        let out = redact_csv(d, "card", true, ",", &o).unwrap();
        assert_eq!(out, "id,card\n1,************1111\n");
    }

    #[test]
    fn hash_is_deterministic_and_salted() {
        let d = "user\nada\nada\nbob";
        let o = Options {
            mode: Mode::Hash,
            salt: "s3cret".into(),
            hash_length: 8,
            ..Options::default()
        };
        let out = redact_csv(d, "1", false, ",", &o).unwrap();
        let lines: Vec<&str> = out.trim().split('\n').collect();
        // header=false, so all 4 rows are data: [user, ada, ada, bob].
        // equal inputs -> equal codes (linkability); different input -> different code.
        assert_eq!(lines[1], lines[2]);
        assert_ne!(lines[2], lines[3]);
        // 8 hex chars.
        assert_eq!(lines[1].len(), 8);
        assert!(lines[1].chars().all(|c| c.is_ascii_hexdigit()));
        // a different salt changes the code.
        let o2 = Options {
            salt: "other".into(),
            ..o.clone()
        };
        let out2 = redact_csv(d, "1", false, ",", &o2).unwrap();
        assert_ne!(out, out2);
    }

    #[test]
    fn redact_mode_uses_fixed_label() {
        let d = "name,ssn\nAda,123-45-6789";
        let o = Options {
            mode: Mode::Redact,
            label: "[PII]".into(),
            ..Options::default()
        };
        let out = redact_csv(d, "ssn", true, ",", &o).unwrap();
        assert_eq!(out, "name,ssn\nAda,[PII]\n");
    }

    #[test]
    fn multiple_columns_and_indices_no_header() {
        let d = "Ada,ada@x.com,London\nBob,bob@y.com,Paris";
        let out = redact_csv(d, "1,2", false, ",", &opts(Mode::Redact)).unwrap();
        // cols 1 and 2 redacted, col 3 (city) kept; no header, so row 1 is data too.
        assert_eq!(
            out,
            "[REDACTED],[REDACTED],London\n[REDACTED],[REDACTED],Paris\n"
        );
    }

    #[test]
    fn header_row_passes_through_unchanged() {
        let d = "email\nada@example.com";
        let out = redact_csv(d, "email", true, ",", &opts(Mode::Hash)).unwrap();
        assert!(out.starts_with("email\n"));
        assert!(!out.contains("ada@example.com"));
    }

    #[test]
    fn tab_delimiter_roundtrips() {
        let d = "name\temail\nAda\tada@x.com";
        let out = redact_csv(d, "email", true, "tab", &opts(Mode::Redact)).unwrap();
        assert_eq!(out, "name\temail\nAda\t[REDACTED]\n");
    }

    #[test]
    fn unknown_column_name_errors() {
        let d = "name,email\nAda,ada@x.com";
        let err = redact_csv(d, "phone", true, ",", &opts(Mode::Mask)).unwrap_err();
        assert!(err.contains("no column named 'phone'"), "got: {err}");
    }

    #[test]
    fn index_out_of_range_errors() {
        let d = "a,b\n1,2";
        let err = redact_csv(d, "9", false, ",", &opts(Mode::Mask)).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(redact_csv("   ", "1", true, ",", &opts(Mode::Mask)).is_err());
    }

    #[test]
    fn no_columns_specified_errors() {
        let d = "a,b\n1,2";
        assert!(redact_csv(d, "  ", true, ",", &opts(Mode::Mask)).is_err());
    }

    #[test]
    fn mode_parse() {
        assert_eq!(Mode::parse("MASK").unwrap(), Mode::Mask);
        assert_eq!(Mode::parse("hash").unwrap(), Mode::Hash);
        assert_eq!(Mode::parse("redact").unwrap(), Mode::Redact);
        assert!(Mode::parse("shred").is_err());
    }

    #[test]
    fn hash_length_clamped() {
        let d = "u\nada";
        let o = Options {
            mode: Mode::Hash,
            hash_length: 200, // clamps to 64
            ..Options::default()
        };
        let out = redact_csv(d, "1", false, ",", &o).unwrap();
        let code = out.trim().lines().nth(1).unwrap();
        assert_eq!(code.len(), 64);
    }
}
