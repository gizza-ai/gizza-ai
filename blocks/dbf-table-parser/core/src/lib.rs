//! dbf-table-parser core — parse a dBase / `.dbf` table file into its column
//! definitions and rows, then render them as CSV or JSON. No wafer/wasm-bindgen
//! deps; pure logic shared by the chat skill block (and host-testable).
//!
//! The DBF layout is parsed by hand (no reader crate): a 32-byte file header, a
//! run of 32-byte field descriptors terminated by `0x0D`, then fixed-width
//! records each prefixed by a 1-byte deletion flag (`0x20` active, `0x2A`
//! deleted). Field types handled: `C` character, `N`/`F` numeric, `D` date
//! (`YYYYMMDD` → `YYYY-MM-DD`), `L` logical (`T/Y` → true, `F/N` → false), `I`
//! FoxPro 4-byte little-endian integer. `M` memo cells emit empty (the sidecar
//! `.dbt`/`.fpt` file is not available to a single-file tool); any other type is
//! decoded as text.

use serde_json::{json, Map, Value};

/// Output rendering target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
}

/// Character-field text decoding. DBF stores text in a code page indicated by the
/// header's language-driver byte; we expose a small, predictable set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 if the bytes are valid UTF-8, otherwise Latin-1 (never fails).
    Auto,
    /// Interpret bytes as UTF-8 (lossy: invalid sequences become `�`).
    Utf8,
    /// ISO-8859-1: each byte maps directly to the same Unicode code point.
    Latin1,
    /// Windows-1252: Latin-1 plus the printable overrides in `0x80..=0x9F`.
    Cp1252,
}

/// Rendering options (parsed from the block/CLI args upstream).
#[derive(Debug, Clone)]
pub struct Options {
    pub format: Format,
    /// CSV field separator (ignored for JSON).
    pub delimiter: char,
    /// Emit a leading row of column names (CSV only).
    pub header: bool,
    /// Comma-separated columns to keep/reorder, by name or 0-based index. Empty = all.
    pub columns: String,
    /// Include records flagged as deleted.
    pub include_deleted: bool,
    /// Trim trailing padding from character fields.
    pub trim: bool,
    pub encoding: Encoding,
    /// Max data rows to emit; 0 = all.
    pub limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Csv,
            delimiter: ',',
            header: true,
            columns: String::new(),
            include_deleted: false,
            trim: true,
            encoding: Encoding::Auto,
            limit: 0,
        }
    }
}

/// A parsed field/column definition.
#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    /// The 1-character DBF field type (`C`, `N`, `F`, `D`, `L`, `I`, `M`, ...).
    pub dtype: char,
    pub length: usize,
    pub decimal: usize,
}

/// A single decoded cell: both the CSV text form and the JSON value form, so the
/// two renderers stay consistent.
struct Cell {
    text: String,
    json: Value,
}

const HEADER_LEN: usize = 32;
const FIELD_DESC_LEN: usize = 32;
const FIELD_TERMINATOR: u8 = 0x0D;

/// Parse `bytes` as a DBF table and render it per `opts`. Returns `Err` with an
/// actionable message on truncated / non-DBF input or an unknown column selector.
pub fn parse_dbf(bytes: &[u8], opts: &Options) -> Result<String, String> {
    let (columns, records) = read_table(bytes, opts)?;
    let selected = select_columns(&columns, &opts.columns)?;

    match opts.format {
        Format::Csv => Ok(render_csv(&columns, &records, &selected, opts)),
        Format::Json => Ok(render_json(&columns, &records, &selected, opts)),
    }
}

/// Read the header + field descriptors + records into columns and per-row cells.
fn read_table(bytes: &[u8], opts: &Options) -> Result<(Vec<Column>, Vec<Row>), String> {
    if bytes.is_empty() {
        return Err("empty .dbf input".to_string());
    }
    if bytes.len() < HEADER_LEN {
        return Err(format!(
            "not a valid .dbf file: header is {} bytes, need at least {HEADER_LEN}",
            bytes.len()
        ));
    }

    let record_count = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let header_size = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let record_size = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;

    if header_size < HEADER_LEN + 1 || header_size > bytes.len() {
        return Err(format!(
            "not a valid .dbf file: declared header size {header_size} is out of range (file is {} bytes)",
            bytes.len()
        ));
    }
    if record_size == 0 {
        return Err("not a valid .dbf file: record length is zero".to_string());
    }

    // Field descriptors run from byte 32 up to the 0x0D terminator (bounded by the
    // header size).
    let mut columns = Vec::new();
    let mut off = HEADER_LEN;
    loop {
        if off >= header_size || off >= bytes.len() {
            return Err(
                "not a valid .dbf file: field descriptors are not 0x0D-terminated".to_string(),
            );
        }
        if bytes[off] == FIELD_TERMINATOR {
            break;
        }
        if off + FIELD_DESC_LEN > bytes.len() {
            return Err("not a valid .dbf file: truncated field descriptor".to_string());
        }
        let desc = &bytes[off..off + FIELD_DESC_LEN];
        let name = ascii_name(&desc[0..11]);
        let dtype = desc[11] as char;
        let length = desc[16] as usize;
        let decimal = desc[17] as usize;
        columns.push(Column {
            name,
            dtype,
            length,
            decimal,
        });
        off += FIELD_DESC_LEN;
    }

    if columns.is_empty() {
        return Err("not a valid .dbf file: no field definitions".to_string());
    }

    let fields_len: usize = columns.iter().map(|c| c.length).sum();
    // The record has a 1-byte deletion flag plus each field's bytes. Trust the
    // header's record_size for stepping, but require it to at least cover the
    // fields we're about to read.
    if record_size < fields_len + 1 {
        return Err(format!(
            "not a valid .dbf file: record length {record_size} is smaller than the fields ({} + 1 flag byte)",
            fields_len
        ));
    }

    // Records begin at header_size. Bound the count by the bytes actually present
    // (a truncated file shouldn't over-read or panic).
    let data = &bytes[header_size..];
    let available = data.len() / record_size;
    let n = record_count.min(available);

    let mut rows = Vec::new();
    for i in 0..n {
        let rec = &data[i * record_size..i * record_size + record_size];
        let deleted = rec.first() == Some(&0x2A); // '*'
        if deleted && !opts.include_deleted {
            continue;
        }
        let mut cells = Vec::with_capacity(columns.len());
        let mut fo = 1; // skip the deletion flag
        for col in &columns {
            let raw = &rec[fo..fo + col.length];
            cells.push(decode_cell(col, raw, opts));
            fo += col.length;
        }
        rows.push(Row { deleted, cells });
        if opts.limit != 0 && rows.len() >= opts.limit {
            break;
        }
    }

    Ok((columns, rows))
}

struct Row {
    deleted: bool,
    cells: Vec<Cell>,
}

/// Decode one fixed-width field into a `Cell` (CSV text + JSON value) by type.
fn decode_cell(col: &Column, raw: &[u8], opts: &Options) -> Cell {
    match col.dtype {
        'C' => {
            let mut s = decode_text(raw, opts.encoding);
            if opts.trim {
                let trimmed = s.trim_end();
                s = trimmed.to_string();
            }
            Cell {
                json: Value::String(s.clone()),
                text: s,
            }
        }
        'N' | 'F' => {
            let s = decode_text(raw, opts.encoding);
            let t = s.trim();
            if t.is_empty() {
                return Cell {
                    text: String::new(),
                    json: Value::Null,
                };
            }
            let json = if col.decimal == 0 {
                match t.parse::<i64>() {
                    Ok(i) => json!(i),
                    Err(_) => t.parse::<f64>().map(number).unwrap_or(Value::Null),
                }
            } else {
                t.parse::<f64>().map(number).unwrap_or(Value::Null)
            };
            Cell {
                text: t.to_string(),
                json,
            }
        }
        'I' => {
            // FoxPro 4-byte little-endian signed integer.
            if raw.len() == 4 {
                let v = i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
                Cell {
                    text: v.to_string(),
                    json: json!(v),
                }
            } else {
                Cell {
                    text: String::new(),
                    json: Value::Null,
                }
            }
        }
        'L' => {
            let c = raw.first().copied().unwrap_or(b' ');
            match c {
                b'T' | b't' | b'Y' | b'y' => Cell {
                    text: "true".to_string(),
                    json: Value::Bool(true),
                },
                b'F' | b'f' | b'N' | b'n' => Cell {
                    text: "false".to_string(),
                    json: Value::Bool(false),
                },
                _ => Cell {
                    text: String::new(),
                    json: Value::Null,
                },
            }
        }
        'D' => {
            let s = decode_text(raw, opts.encoding);
            let t = s.trim();
            if t.len() == 8 && t.bytes().all(|b| b.is_ascii_digit()) {
                let iso = format!("{}-{}-{}", &t[0..4], &t[4..6], &t[6..8]);
                Cell {
                    text: iso.clone(),
                    json: Value::String(iso),
                }
            } else {
                Cell {
                    text: String::new(),
                    json: Value::Null,
                }
            }
        }
        // Memo: the referenced .dbt/.fpt block isn't available to a single-file
        // tool, so emit empty rather than a raw block number.
        'M' => Cell {
            text: String::new(),
            json: Value::Null,
        },
        // Anything else: best-effort text.
        _ => {
            let mut s = decode_text(raw, opts.encoding);
            if opts.trim {
                s = s.trim().to_string();
            }
            Cell {
                json: Value::String(s.clone()),
                text: s,
            }
        }
    }
}

/// Convert a finite `f64` into a JSON number; non-finite → Null.
fn number(f: f64) -> Value {
    serde_json::Number::from_f64(f)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// Field name = bytes up to the first NUL (or the full 11 bytes), ASCII.
fn ascii_name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

/// Decode raw field bytes to text under the requested encoding.
fn decode_text(raw: &[u8], enc: Encoding) -> String {
    match enc {
        Encoding::Utf8 => String::from_utf8_lossy(raw).into_owned(),
        Encoding::Latin1 => raw.iter().map(|&b| b as char).collect(),
        Encoding::Cp1252 => raw.iter().map(|&b| cp1252_char(b)).collect(),
        Encoding::Auto => match std::str::from_utf8(raw) {
            Ok(s) => s.to_string(),
            Err(_) => raw.iter().map(|&b| b as char).collect(),
        },
    }
}

/// Windows-1252: identical to Latin-1 except for the 32 printable overrides in
/// `0x80..=0x9F` (five of which are undefined → `�`).
fn cp1252_char(b: u8) -> char {
    if (0x80..=0x9F).contains(&b) {
        const HI: [char; 32] = [
            '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
            '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}',
            '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
            '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
            '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
        ];
        HI[(b - 0x80) as usize]
    } else {
        b as char
    }
}

/// Resolve the `columns` selector into an ordered list of column indices. Empty
/// selector = every column, in file order.
fn select_columns(columns: &[Column], spec: &str) -> Result<Vec<usize>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok((0..columns.len()).collect());
    }
    let mut out = Vec::new();
    for tok in spec.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        // A bare non-negative integer selects by 0-based index; otherwise by name
        // (case-insensitive). An exact name match wins first so a column literally
        // named "0" is still reachable.
        if let Some(i) = columns.iter().position(|c| c.name.eq_ignore_ascii_case(tok)) {
            out.push(i);
        } else if let Ok(idx) = tok.parse::<usize>() {
            if idx >= columns.len() {
                return Err(format!(
                    "column index {idx} out of range (table has {} columns)",
                    columns.len()
                ));
            }
            out.push(idx);
        } else {
            let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
            return Err(format!("no column named {tok:?} (available: {names:?})"));
        }
    }
    if out.is_empty() {
        return Ok((0..columns.len()).collect());
    }
    Ok(out)
}

/// RFC-4180-style quote a CSV field for the given `delimiter`.
fn escape_field(field: &str, delimiter: char) -> String {
    if field.contains([delimiter, '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn render_csv(columns: &[Column], rows: &[Row], selected: &[usize], opts: &Options) -> String {
    let mut out = String::new();
    let d = opts.delimiter;
    if opts.header {
        let hdr: Vec<String> = selected
            .iter()
            .map(|&i| escape_field(&columns[i].name, d))
            .collect();
        out.push_str(&hdr.join(&d.to_string()));
        out.push_str("\r\n");
    }
    for row in rows {
        let line: Vec<String> = selected
            .iter()
            .map(|&i| escape_field(&row.cells[i].text, d))
            .collect();
        out.push_str(&line.join(&d.to_string()));
        out.push_str("\r\n");
    }
    out
}

fn render_json(columns: &[Column], rows: &[Row], selected: &[usize], opts: &Options) -> String {
    let cols: Vec<Value> = selected
        .iter()
        .map(|&i| {
            let c = &columns[i];
            json!({
                "name": c.name,
                "type": c.dtype.to_string(),
                "length": c.length,
                "decimal": c.decimal,
            })
        })
        .collect();

    let rows_json: Vec<Value> = rows
        .iter()
        .map(|row| {
            let mut obj = Map::new();
            for &i in selected {
                obj.insert(columns[i].name.clone(), row.cells[i].json.clone());
            }
            Value::Object(obj)
        })
        .collect();

    let root = json!({
        "columns": cols,
        "row_count": rows_json.len(),
        "rows": rows_json,
    });
    let _ = opts; // encoding/trim already applied per-cell
    serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Left-justify `s` into `width` bytes, padding on the right with spaces
    /// (how DBF stores C fields).
    fn padr(s: &[u8], width: usize) -> Vec<u8> {
        let mut v = s.to_vec();
        v.resize(width, b' ');
        v
    }
    /// Right-justify `s` into `width` bytes, padding on the left with spaces
    /// (how DBF stores N fields).
    fn padl(s: &[u8], width: usize) -> Vec<u8> {
        let mut v = vec![b' '; width.saturating_sub(s.len())];
        v.extend_from_slice(s);
        v.truncate(width);
        v
    }

    /// Build a tiny in-memory dBase III file: fields NAME C(10), AGE N(3);
    /// 3 records — Alice/30 (active), Bob/25 (deleted), Cara/"" (active).
    fn sample_dbf() -> Vec<u8> {
        let fields: [(&[u8], u8, u8, u8); 2] = [(b"NAME", b'C', 10, 0), (b"AGE", b'N', 3, 0)];
        let fields_len: usize = fields.iter().map(|f| f.2 as usize).sum();
        let record_size = 1 + fields_len; // 14
        let header_size = HEADER_LEN + fields.len() * FIELD_DESC_LEN + 1; // 97

        let mut h = vec![0u8; HEADER_LEN];
        h[0] = 0x03; // dBase III, no memo
        h[4..8].copy_from_slice(&(3u32).to_le_bytes());
        h[8..10].copy_from_slice(&(header_size as u16).to_le_bytes());
        h[10..12].copy_from_slice(&(record_size as u16).to_le_bytes());
        h[29] = 0x03; // cp1252 language driver

        let mut out = h;
        for (name, t, len, dec) in fields {
            let mut d = vec![0u8; FIELD_DESC_LEN];
            d[..name.len()].copy_from_slice(name);
            d[11] = t;
            d[16] = len;
            d[17] = dec;
            out.extend(d);
        }
        out.push(FIELD_TERMINATOR);

        // Records.
        out.push(0x20); // active
        out.extend(padr(b"Alice", 10));
        out.extend(padl(b"30", 3));

        out.push(0x2A); // deleted '*'
        out.extend(padr(b"Bob", 10));
        out.extend(padl(b"25", 3));

        out.push(0x20); // active, empty age
        out.extend(padr(b"Cara", 10));
        out.extend(padl(b"", 3));

        out.push(0x1A); // EOF marker
        out
    }

    #[test]
    fn csv_default_excludes_deleted_and_trims() {
        let csv = parse_dbf(&sample_dbf(), &Options::default()).unwrap();
        assert_eq!(csv, "NAME,AGE\r\nAlice,30\r\nCara,\r\n", "got: {csv:?}");
    }

    #[test]
    fn csv_include_deleted_adds_bob() {
        let opts = Options {
            include_deleted: true,
            ..Options::default()
        };
        let csv = parse_dbf(&sample_dbf(), &opts).unwrap();
        assert_eq!(
            csv, "NAME,AGE\r\nAlice,30\r\nBob,25\r\nCara,\r\n",
            "got: {csv:?}"
        );
    }

    #[test]
    fn csv_no_header_and_tab_delimiter() {
        let opts = Options {
            header: false,
            delimiter: '\t',
            ..Options::default()
        };
        let csv = parse_dbf(&sample_dbf(), &opts).unwrap();
        assert_eq!(csv, "Alice\t30\r\nCara\t\r\n", "got: {csv:?}");
    }

    #[test]
    fn json_has_columns_and_typed_rows() {
        let opts = Options {
            format: Format::Json,
            ..Options::default()
        };
        let out = parse_dbf(&sample_dbf(), &opts).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["columns"][0]["name"], "NAME");
        assert_eq!(v["columns"][0]["type"], "C");
        assert_eq!(v["columns"][1]["type"], "N");
        assert_eq!(v["row_count"], 2);
        assert_eq!(v["rows"][0]["NAME"], "Alice");
        assert_eq!(v["rows"][0]["AGE"], 30); // integer, not "30"
        assert!(v["rows"][1]["AGE"].is_null()); // Cara's empty age
    }

    #[test]
    fn column_selection_and_reorder() {
        let opts = Options {
            columns: "AGE,NAME".to_string(),
            ..Options::default()
        };
        let csv = parse_dbf(&sample_dbf(), &opts).unwrap();
        assert_eq!(csv, "AGE,NAME\r\n30,Alice\r\n,Cara\r\n", "got: {csv:?}");
    }

    #[test]
    fn column_selection_by_index() {
        let opts = Options {
            columns: "1".to_string(),
            ..Options::default()
        };
        let csv = parse_dbf(&sample_dbf(), &opts).unwrap();
        assert_eq!(csv, "AGE\r\n30\r\n\r\n", "got: {csv:?}");
    }

    #[test]
    fn limit_caps_rows() {
        let opts = Options {
            limit: 1,
            ..Options::default()
        };
        let csv = parse_dbf(&sample_dbf(), &opts).unwrap();
        assert_eq!(csv, "NAME,AGE\r\nAlice,30\r\n", "got: {csv:?}");
    }

    #[test]
    fn logical_and_date_fields() {
        // One record: ACTIVE L, HIRED D(8).
        let fields: [(&[u8], u8, u8, u8); 2] = [(b"ACTIVE", b'L', 1, 0), (b"HIRED", b'D', 8, 0)];
        let record_size = 1 + 1 + 8;
        let header_size = HEADER_LEN + 2 * FIELD_DESC_LEN + 1;
        let mut h = vec![0u8; HEADER_LEN];
        h[0] = 0x03;
        h[4..8].copy_from_slice(&(1u32).to_le_bytes());
        h[8..10].copy_from_slice(&(header_size as u16).to_le_bytes());
        h[10..12].copy_from_slice(&(record_size as u16).to_le_bytes());
        let mut out = h;
        for (name, t, len, dec) in fields {
            let mut d = vec![0u8; FIELD_DESC_LEN];
            d[..name.len()].copy_from_slice(name);
            d[11] = t;
            d[16] = len;
            d[17] = dec;
            out.extend(d);
        }
        out.push(FIELD_TERMINATOR);
        out.push(0x20);
        out.push(b'T');
        out.extend_from_slice(b"20240115");
        out.push(0x1A);

        let opts = Options {
            format: Format::Json,
            ..Options::default()
        };
        let v: Value = serde_json::from_str(&parse_dbf(&out, &opts).unwrap()).unwrap();
        assert_eq!(v["rows"][0]["ACTIVE"], true);
        assert_eq!(v["rows"][0]["HIRED"], "2024-01-15");
    }

    #[test]
    fn encoding_cp1252_vs_latin1() {
        // Single C(1) field holding byte 0x80 (€ in cp1252, U+0080 in latin1).
        let header_size = HEADER_LEN + FIELD_DESC_LEN + 1;
        let record_size = 1 + 1;
        let mut h = vec![0u8; HEADER_LEN];
        h[0] = 0x03;
        h[4..8].copy_from_slice(&(1u32).to_le_bytes());
        h[8..10].copy_from_slice(&(header_size as u16).to_le_bytes());
        h[10..12].copy_from_slice(&(record_size as u16).to_le_bytes());
        let mut out = h;
        let mut d = vec![0u8; FIELD_DESC_LEN];
        d[..4].copy_from_slice(b"SYM ");
        d[3] = 0; // NUL-terminate "SYM"
        d[11] = b'C';
        d[16] = 1;
        out.extend(d);
        out.push(FIELD_TERMINATOR);
        out.push(0x20);
        out.push(0x80);

        let cp = parse_dbf(
            &out,
            &Options {
                format: Format::Json,
                encoding: Encoding::Cp1252,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(cp.contains('\u{20AC}'), "cp1252 should decode 0x80 as €");

        let l1 = parse_dbf(
            &out,
            &Options {
                format: Format::Json,
                encoding: Encoding::Latin1,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(
            l1.contains('\u{0080}'),
            "latin1 should decode 0x80 as U+0080"
        );
    }

    #[test]
    fn empty_input_errors() {
        let err = parse_dbf(&[], &Options::default()).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn garbage_input_errors() {
        let err = parse_dbf(b"not a dbf file at all!!", &Options::default()).unwrap_err();
        assert!(!err.is_empty(), "should reject non-DBF bytes: {err}");
    }

    #[test]
    fn unknown_column_errors() {
        let opts = Options {
            columns: "NOPE".to_string(),
            ..Options::default()
        };
        let err = parse_dbf(&sample_dbf(), &opts).unwrap_err();
        assert!(err.contains("no column named"), "got: {err}");
    }
}
