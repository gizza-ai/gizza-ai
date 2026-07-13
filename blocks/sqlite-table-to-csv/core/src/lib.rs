//! sqlite-table-to-csv core — reads an SQLite database file (`.db`/`.sqlite`)
//! by parsing the on-disk format directly (no SQL engine, no C `libsqlite3`),
//! and exports one table as RFC-4180 CSV text.
//!
//! Pure Rust, no wafer/wasm-bindgen deps → compiles to both `wasm32-wasip1`
//! (wafer chat block) and native (CLI + host tests). The parser walks the
//! file's b-tree pages: page 1 holds `sqlite_master` (the schema), from which
//! we resolve the requested table's rootpage + column list, then traverse that
//! table's b-tree collecting rows in rowid order.
//!
//! Supported: rowid tables (the overwhelmingly common case), `INTEGER PRIMARY
//! KEY` rowid aliases, record overflow pages, and UTF-8 / UTF-16 text encodings.
//! Not supported (errors clearly): `WITHOUT ROWID` tables (they use a different
//! index-b-tree layout) and encrypted/corrupt files.

use std::collections::HashSet;

/// The 16-byte magic header every SQLite 3 file starts with.
const MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// Output CSV field separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Comma,
    Tab,
    Semicolon,
    Pipe,
}

impl Delimiter {
    /// Parse a delimiter name (as used by the descriptor `enumv`).
    pub fn parse(s: &str) -> Result<Delimiter, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "comma" | "," => Ok(Delimiter::Comma),
            "tab" | "\\t" => Ok(Delimiter::Tab),
            "semicolon" | ";" => Ok(Delimiter::Semicolon),
            "pipe" | "|" => Ok(Delimiter::Pipe),
            other => Err(format!(
                "unknown delimiter {other:?} (expected one of: comma, tab, semicolon, pipe)"
            )),
        }
    }

    fn ch(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Tab => '\t',
            Delimiter::Semicolon => ';',
            Delimiter::Pipe => '|',
        }
    }
}

/// Options controlling the CSV export.
#[derive(Debug, Clone)]
pub struct Options {
    /// Table to export. `None` → the single user table if there is exactly one,
    /// otherwise an error listing the available tables.
    pub table: Option<String>,
    /// Field separator.
    pub delimiter: Delimiter,
    /// Emit a header row of column names.
    pub header: bool,
    /// Text substituted for SQL `NULL` cells (default empty string).
    pub null_value: String,
    /// Prepend a UTF-8 byte-order mark (helps Excel detect UTF-8).
    pub bom: bool,
    /// Cap on the number of data rows exported; `0` means all rows.
    pub limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            table: None,
            delimiter: Delimiter::Comma,
            header: true,
            null_value: String::new(),
            bom: false,
            limit: 0,
        }
    }
}

/// The result of a successful export.
#[derive(Debug, Clone)]
pub struct CsvOutput {
    /// The table that was actually exported.
    pub table: String,
    /// All user tables in the database (excludes internal `sqlite_*` tables).
    pub available_tables: Vec<String>,
    /// Column names for the exported table.
    pub columns: Vec<String>,
    /// Number of data rows written (after any `limit`).
    pub row_count: usize,
    /// Whether the row set was truncated by `limit`.
    pub truncated: bool,
    /// The CSV text (including header row if requested and BOM if requested).
    pub csv: String,
}

/// A decoded cell value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Value {
    /// The cell as an integer (also coerces a REAL that is integral). `None` for
    /// text/blob/null.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Real(f) if f.is_finite() => Some(*f as i64),
            _ => None,
        }
    }

    /// The cell as a string slice, if it is TEXT.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Whether the cell is SQL `NULL`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

/// A whole table read into memory as typed rows (rowid-alias substituted).
#[derive(Debug, Clone)]
pub struct Table {
    /// The table name as declared in the schema.
    pub name: String,
    /// Column names in declaration order.
    pub columns: Vec<String>,
    /// One `Vec<Value>` per row, each aligned to `columns` (short records are
    /// NULL-padded; an `INTEGER PRIMARY KEY` alias cell holds the rowid).
    pub rows: Vec<Vec<Value>>,
}

impl Table {
    /// Case-insensitive column-name lookup → index into each row.
    pub fn col_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.eq_ignore_ascii_case(name))
    }
}

/// Read an entire rowid table into memory as typed rows. Reuses the same on-disk
/// b-tree parser as [`to_csv`]; used by higher-level blocks (e.g.
/// `browser-history-parser`) that need typed cells rather than CSV text.
/// Errors on a missing table or a `WITHOUT ROWID` table.
pub fn read_table(bytes: &[u8], table: &str) -> Result<Table, String> {
    let hdr = parse_header(bytes)?;
    let schemas = read_schema(bytes, &hdr)?;
    let schema = schemas
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(table))
        .ok_or_else(|| format!("no table named {table:?}"))?;
    if schema.without_rowid {
        return Err(format!(
            "table {:?} is a WITHOUT ROWID table, which this parser does not support yet",
            schema.name
        ));
    }
    let ncols = schema.columns.len();
    let mut raw: Vec<(i64, Vec<Value>)> = Vec::new();
    walk_table_btree(bytes, &hdr, schema.rootpage, &mut raw, 0)?;
    let rows = raw
        .into_iter()
        .map(|(rowid, mut values)| {
            values.resize(ncols, Value::Null);
            if let Some(i) = schema.rowid_alias {
                if matches!(values[i], Value::Null) {
                    values[i] = Value::Int(rowid);
                }
            }
            values
        })
        .collect();
    Ok(Table {
        name: schema.name.clone(),
        columns: schema.columns.clone(),
        rows,
    })
}

/// A parsed schema entry for one user table.
#[derive(Debug, Clone)]
struct TableSchema {
    name: String,
    rootpage: u32,
    columns: Vec<String>,
    /// Index of the `INTEGER PRIMARY KEY` rowid-alias column, if any.
    rowid_alias: Option<usize>,
    without_rowid: bool,
}

/// Header-level facts about the database file.
struct DbHeader {
    page_size: usize,
    /// Usable bytes per page (`page_size - reserved`).
    usable: usize,
    /// 1 = UTF-8, 2 = UTF-16le, 3 = UTF-16be.
    text_encoding: u32,
}

/// List the user tables in a database file (excludes internal `sqlite_*`).
pub fn list_tables(bytes: &[u8]) -> Result<Vec<String>, String> {
    let hdr = parse_header(bytes)?;
    let schemas = read_schema(bytes, &hdr)?;
    Ok(schemas.into_iter().map(|s| s.name).collect())
}

/// One raw row of the `sqlite_master` schema catalog, as stored on page 1.
/// Higher-level tools (e.g. `sqlite-db-inspector`) interpret these to describe
/// tables, indexes, views and triggers; unlike [`list_tables`] this exposes
/// *every* object, including internal `sqlite_*` ones, so the caller decides
/// what to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterEntry {
    /// Object kind: `"table"`, `"index"`, `"view"` or `"trigger"`.
    pub kind: String,
    /// The object's own name.
    pub name: String,
    /// The table the object belongs to (equals `name` for a table itself).
    pub tbl_name: String,
    /// The b-tree root page (`None` for views, triggers and virtual tables).
    pub rootpage: Option<u32>,
    /// The `CREATE …` SQL text (empty for auto-created indexes and internal
    /// objects that store a NULL `sql`).
    pub sql: String,
}

/// Read every row of `sqlite_master` (the on-disk schema catalog rooted at page
/// 1) as raw [`MasterEntry`] records, in stored order. This is the low-level
/// primitive behind [`list_tables`]; it performs no filtering so a schema
/// inspector can see indexes, views, triggers and internal `sqlite_*` objects.
pub fn master_entries(bytes: &[u8]) -> Result<Vec<MasterEntry>, String> {
    let hdr = parse_header(bytes)?;
    let mut rows: Vec<(i64, Vec<Value>)> = Vec::new();
    walk_table_btree(bytes, &hdr, 1, &mut rows, 0)?;

    let mut out = Vec::new();
    for (_rowid, cols) in rows {
        // sqlite_master columns: type, name, tbl_name, rootpage, sql
        let kind = cols.first().and_then(as_text).unwrap_or_default();
        if kind.is_empty() {
            continue;
        }
        let name = cols.get(1).and_then(as_text).unwrap_or_default();
        let tbl_name = cols.get(2).and_then(as_text).unwrap_or_default();
        let rootpage = match cols.get(3) {
            Some(Value::Int(i)) if *i > 0 => Some(*i as u32),
            _ => None,
        };
        let sql = cols.get(4).and_then(as_text).unwrap_or_default();
        out.push(MasterEntry {
            kind,
            name,
            tbl_name,
            rootpage,
            sql,
        });
    }
    Ok(out)
}

/// Count the rows of a user table by walking its b-tree, without decoding any
/// cell values (cheap even for large tables). Returns:
///   - `Ok(Some(n))` for a normal rowid table,
///   - `Ok(None)` for a `WITHOUT ROWID` table (its index-b-tree layout isn't
///     supported here — the caller should explain that to the user),
///   - `Err(_)` if there is no such table or the file is corrupt.
pub fn count_rows(bytes: &[u8], table: &str) -> Result<Option<u64>, String> {
    let hdr = parse_header(bytes)?;
    let schemas = read_schema(bytes, &hdr)?;
    let schema = schemas
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(table))
        .ok_or_else(|| format!("no table named {table:?}"))?;
    if schema.without_rowid {
        return Ok(None);
    }
    let n = count_table_btree(bytes, &hdr, schema.rootpage)?;
    Ok(Some(n))
}

/// Walk a rowid table b-tree counting leaf cells (rows) without decoding them.
/// Shares the page/cycle guards of [`walk_table_btree`] but skips record parsing.
fn count_table_btree(bytes: &[u8], hdr: &DbHeader, root: u32) -> Result<u64, String> {
    let mut visited = HashSet::new();
    let mut stack = vec![root];
    let mut total: u64 = 0;
    while let Some(pageno) = stack.pop() {
        if pageno == 0 {
            continue;
        }
        if !visited.insert(pageno) {
            return Err("corrupt database: b-tree page cycle detected".to_string());
        }
        let page = page_slice(bytes, hdr, pageno)?;
        let hdr_off = if pageno == 1 { 100 } else { 0 };
        let page_type = page
            .get(hdr_off)
            .copied()
            .ok_or("corrupt database: truncated page header")?;
        let ncells = be_u16(page, hdr_off + 3)? as usize;
        match page_type {
            0x0d => {
                total += ncells as u64;
            }
            0x05 => {
                let right = be_u32(page, hdr_off + 8)?;
                let ptr_base = hdr_off + 12;
                for i in 0..ncells {
                    let cell_off = be_u16(page, ptr_base + i * 2)? as usize;
                    stack.push(be_u32(page, cell_off)?);
                }
                stack.push(right);
            }
            other => {
                return Err(format!(
                    "unexpected b-tree page type 0x{other:02x} on page {pageno} (not a rowid table)"
                ));
            }
        }
    }
    Ok(total)
}

/// Export a table of the SQLite database `bytes` as CSV per `opts`.
pub fn to_csv(bytes: &[u8], opts: &Options) -> Result<CsvOutput, String> {
    let hdr = parse_header(bytes)?;
    let schemas = read_schema(bytes, &hdr)?;
    let available: Vec<String> = schemas.iter().map(|s| s.name.clone()).collect();

    if schemas.is_empty() {
        return Err("the database contains no user tables".to_string());
    }

    let schema = match opts.table.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => schemas
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("no table named {name:?} (available: {available:?})"))?,
        None => {
            if schemas.len() == 1 {
                &schemas[0]
            } else {
                return Err(format!(
                    "the database has {} tables — specify which one to export via `table` (available: {available:?})",
                    schemas.len()
                ));
            }
        }
    };

    if schema.without_rowid {
        return Err(format!(
            "table {:?} is a WITHOUT ROWID table, which this tool does not support yet",
            schema.name
        ));
    }

    let ncols = schema.columns.len();
    let mut rows: Vec<(i64, Vec<Value>)> = Vec::new();
    let limit = opts.limit;
    // Fetch one extra row past the limit so we can tell whether the result was
    // actually truncated (rather than the table simply ending at the limit).
    let walk_limit = if limit == 0 { 0 } else { limit + 1 };
    walk_table_btree(bytes, &hdr, schema.rootpage, &mut rows, walk_limit)?;

    // Rows come out in rowid order from the b-tree walk; keep that stable.
    let truncated = limit != 0 && rows.len() > limit;
    if truncated {
        rows.truncate(limit);
    }
    let row_count = rows.len();

    let delim = opts.delimiter.ch();
    let mut csv = String::new();
    if opts.bom {
        csv.push('\u{feff}');
    }
    if opts.header {
        write_csv_row(&mut csv, schema.columns.iter().cloned(), delim);
    }
    for (rowid, values) in &rows {
        let fields = (0..ncols).map(|i| {
            let v = values.get(i).unwrap_or(&Value::Null);
            // An INTEGER PRIMARY KEY column stores NULL in the record; the true
            // value is the rowid.
            if schema.rowid_alias == Some(i) && matches!(v, Value::Null) {
                rowid.to_string()
            } else {
                render_value(v, &opts.null_value)
            }
        });
        write_csv_row(&mut csv, fields, delim);
    }

    Ok(CsvOutput {
        table: schema.name.clone(),
        available_tables: available,
        columns: schema.columns.clone(),
        row_count,
        truncated,
        csv,
    })
}

/// Render a decoded value for CSV output.
fn render_value(v: &Value, null_value: &str) -> String {
    match v {
        Value::Null => null_value.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Real(f) => format_real(*f),
        Value::Text(s) => s.clone(),
        // Blobs have no text form in CSV; render as lowercase hex.
        Value::Blob(b) => b.iter().map(|byte| format!("{byte:02x}")).collect(),
    }
}

/// Format an `f64` so integral values drop the trailing `.0`.
fn format_real(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

/// Append one RFC-4180 CSV row (fields joined by `delim`, `\r\n` terminated).
fn write_csv_row(out: &mut String, fields: impl Iterator<Item = String>, delim: char) {
    let mut first = true;
    for field in fields {
        if !first {
            out.push(delim);
        }
        first = false;
        out.push_str(&escape_field(&field, delim));
    }
    out.push_str("\r\n");
}

/// RFC-4180-quote a field iff it contains the delimiter, a double-quote, CR, or LF.
fn escape_field(field: &str, delim: char) -> String {
    if field.contains([delim, '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

// ---------------------------------------------------------------------------
// File-format parsing
// ---------------------------------------------------------------------------

fn parse_header(bytes: &[u8]) -> Result<DbHeader, String> {
    if bytes.len() < 100 {
        return Err("file is too small to be an SQLite database".to_string());
    }
    if &bytes[0..16] != MAGIC {
        return Err("not an SQLite 3 database (bad magic header)".to_string());
    }
    // Page size: big-endian u16 at offset 16; the value 1 encodes 65536.
    let raw = u16::from_be_bytes([bytes[16], bytes[17]]);
    let page_size = if raw == 1 { 65536 } else { raw as usize };
    if page_size < 512 || !page_size.is_power_of_two() {
        return Err(format!("invalid page size {page_size}"));
    }
    let reserved = bytes[20] as usize;
    if reserved >= page_size {
        return Err("invalid reserved-space size in header".to_string());
    }
    let usable = page_size - reserved;
    let text_encoding = u32::from_be_bytes([bytes[56], bytes[57], bytes[58], bytes[59]]);
    let text_encoding = if text_encoding == 0 { 1 } else { text_encoding };
    if !(1..=3).contains(&text_encoding) {
        return Err(format!("unknown text encoding {text_encoding}"));
    }
    Ok(DbHeader {
        page_size,
        usable,
        text_encoding,
    })
}

/// Read `sqlite_master` (rooted at page 1) into the list of user tables.
fn read_schema(bytes: &[u8], hdr: &DbHeader) -> Result<Vec<TableSchema>, String> {
    let mut rows: Vec<(i64, Vec<Value>)> = Vec::new();
    walk_table_btree(bytes, hdr, 1, &mut rows, 0)?;

    let mut out = Vec::new();
    for (_rowid, cols) in rows {
        // sqlite_master columns: type, name, tbl_name, rootpage, sql
        let typ = cols.first().and_then(as_text).unwrap_or_default();
        if typ != "table" {
            continue;
        }
        let name = cols.get(1).and_then(as_text).unwrap_or_default();
        if name.is_empty() || name.starts_with("sqlite_") {
            continue;
        }
        let rootpage = match cols.get(3) {
            Some(Value::Int(i)) if *i > 0 => *i as u32,
            _ => continue, // virtual tables etc. have a null rootpage
        };
        let sql = cols.get(4).and_then(as_text).unwrap_or_default();
        let (columns, rowid_alias) = parse_create_columns(&sql);
        let without_rowid = sql_has_without_rowid(&sql);
        out.push(TableSchema {
            name,
            rootpage,
            columns,
            rowid_alias,
            without_rowid,
        });
    }
    Ok(out)
}

fn as_text(v: &Value) -> Option<String> {
    match v {
        Value::Text(s) => Some(s.clone()),
        _ => None,
    }
}

/// Walk a table b-tree (rooted at `root`), pushing `(rowid, values)` in rowid
/// order into `rows`. `row_limit` (non-zero) stops the walk early once enough
/// rows are collected. Guards against page cycles and out-of-bounds pages.
fn walk_table_btree(
    bytes: &[u8],
    hdr: &DbHeader,
    root: u32,
    rows: &mut Vec<(i64, Vec<Value>)>,
    row_limit: usize,
) -> Result<(), String> {
    let mut visited = HashSet::new();
    // A manual stack keeps interior pages in traversal order: child pointers are
    // pushed reversed (right-most last) so they pop left-to-right (rowid order).
    let mut stack = vec![root];
    while let Some(pageno) = stack.pop() {
        if row_limit != 0 && rows.len() >= row_limit {
            return Ok(());
        }
        if pageno == 0 {
            continue;
        }
        if !visited.insert(pageno) {
            return Err("corrupt database: b-tree page cycle detected".to_string());
        }
        let page = page_slice(bytes, hdr, pageno)?;
        let hdr_off = if pageno == 1 { 100 } else { 0 };
        let page_type = page
            .get(hdr_off)
            .copied()
            .ok_or("corrupt database: truncated page header")?;
        let ncells = be_u16(page, hdr_off + 3)? as usize;
        match page_type {
            0x0d => {
                // Leaf table page: 8-byte header, then cell pointers.
                let ptr_base = hdr_off + 8;
                for i in 0..ncells {
                    if row_limit != 0 && rows.len() >= row_limit {
                        return Ok(());
                    }
                    let cell_off = be_u16(page, ptr_base + i * 2)? as usize;
                    let (rowid, values) = parse_leaf_table_cell(bytes, hdr, page, cell_off)?;
                    rows.push((rowid, values));
                }
            }
            0x05 => {
                // Interior table page: 12-byte header, then cell pointers.
                let right = be_u32(page, hdr_off + 8)?;
                let ptr_base = hdr_off + 12;
                let mut children = Vec::with_capacity(ncells + 1);
                for i in 0..ncells {
                    let cell_off = be_u16(page, ptr_base + i * 2)? as usize;
                    let child = be_u32(page, cell_off)?;
                    children.push(child);
                }
                children.push(right);
                for child in children.into_iter().rev() {
                    stack.push(child);
                }
            }
            other => {
                return Err(format!(
                    "unexpected b-tree page type 0x{other:02x} on page {pageno} (not a rowid table)"
                ));
            }
        }
    }
    Ok(())
}

/// Return the `page_size`-byte slice for a 1-based page number.
fn page_slice<'a>(bytes: &'a [u8], hdr: &DbHeader, pageno: u32) -> Result<&'a [u8], String> {
    let start = (pageno as usize - 1)
        .checked_mul(hdr.page_size)
        .ok_or("page offset overflow")?;
    let end = start
        .checked_add(hdr.page_size)
        .ok_or("page offset overflow")?;
    bytes
        .get(start..end)
        .ok_or_else(|| format!("corrupt database: page {pageno} is out of range"))
}

/// Parse a leaf-table cell at `cell_off` within `page`, returning its rowid and
/// decoded column values. Reassembles overflow pages when the payload spills.
fn parse_leaf_table_cell(
    bytes: &[u8],
    hdr: &DbHeader,
    page: &[u8],
    cell_off: usize,
) -> Result<(i64, Vec<Value>), String> {
    let mut off = cell_off;
    let (payload_len, n) = read_varint(page, off)?;
    off += n;
    let (rowid, n) = read_varint(page, off)?;
    off += n;
    let payload_len = payload_len as usize;

    // Table-leaf overflow thresholds (SQLite payload-spilling rules).
    let u = hdr.usable;
    let max_local = u - 35;
    let payload = if payload_len <= max_local {
        page.get(off..off + payload_len)
            .ok_or("corrupt database: truncated cell payload")?
            .to_vec()
    } else {
        let min_local = (u - 12) * 32 / 255 - 23;
        let k = min_local + (payload_len - min_local) % (u - 4);
        let local = if k <= max_local { k } else { min_local };
        let mut payload = page
            .get(off..off + local)
            .ok_or("corrupt database: truncated cell payload")?
            .to_vec();
        let mut next = be_u32(page, off + local)?;
        let mut guard = 0usize;
        let max_pages = bytes.len() / u.max(1) + 2;
        while next != 0 && payload.len() < payload_len {
            guard += 1;
            if guard > max_pages {
                return Err("corrupt database: overflow page chain too long".to_string());
            }
            let opage = page_slice(bytes, hdr, next)?;
            next = be_u32(opage, 0)?;
            let take = (payload_len - payload.len()).min(u - 4);
            payload.extend_from_slice(
                opage
                    .get(4..4 + take)
                    .ok_or("corrupt database: truncated overflow page")?,
            );
        }
        payload
    };

    let values = parse_record(&payload, hdr.text_encoding)?;
    Ok((rowid as i64, values))
}

/// Parse an SQLite record (a header of serial types followed by the values).
fn parse_record(payload: &[u8], text_encoding: u32) -> Result<Vec<Value>, String> {
    let (header_len, first_adv) = read_varint(payload, 0)?;
    let header_len = header_len as usize;
    if header_len > payload.len() {
        return Err("corrupt database: record header exceeds payload".to_string());
    }
    let mut serials = Vec::new();
    let mut hoff = first_adv;
    while hoff < header_len {
        let (serial, n) = read_varint(payload, hoff)?;
        serials.push(serial);
        hoff += n;
    }
    let mut body = header_len; // values start right after the header
    let mut values = Vec::with_capacity(serials.len());
    for serial in serials {
        let (val, size) = decode_serial(payload, body, serial, text_encoding)?;
        body += size;
        values.push(val);
    }
    Ok(values)
}

/// Decode one column value given its serial type, returning `(value, byte_len)`.
fn decode_serial(
    payload: &[u8],
    at: usize,
    serial: u64,
    text_encoding: u32,
) -> Result<(Value, usize), String> {
    let need = |n: usize| -> Result<&[u8], String> {
        payload
            .get(at..at + n)
            .ok_or_else(|| "corrupt database: value runs past record".to_string())
    };
    Ok(match serial {
        0 => (Value::Null, 0),
        1 => (Value::Int(need(1)?[0] as i8 as i64), 1),
        2 => (Value::Int(be_int(need(2)?)), 2),
        3 => (Value::Int(be_int(need(3)?)), 3),
        4 => (Value::Int(be_int(need(4)?)), 4),
        5 => (Value::Int(be_int(need(6)?)), 6),
        6 => (Value::Int(be_int(need(8)?)), 8),
        7 => {
            let b = need(8)?;
            let mut arr = [0u8; 8];
            arr.copy_from_slice(b);
            (Value::Real(f64::from_be_bytes(arr)), 8)
        }
        8 => (Value::Int(0), 0),
        9 => (Value::Int(1), 0),
        10 | 11 => return Err("corrupt database: reserved serial type".to_string()),
        n if n % 2 == 0 => {
            let len = ((n - 12) / 2) as usize;
            (Value::Blob(need(len)?.to_vec()), len)
        }
        n => {
            let len = ((n - 13) / 2) as usize;
            let raw = need(len)?;
            (Value::Text(decode_text(raw, text_encoding)), len)
        }
    })
}

/// Decode a big-endian two's-complement signed integer of 1..=8 bytes.
fn be_int(b: &[u8]) -> i64 {
    let mut v: i64 = if b[0] & 0x80 != 0 { -1 } else { 0 };
    for &byte in b {
        v = (v << 8) | byte as i64;
    }
    v
}

/// Decode text bytes per the database's text encoding.
fn decode_text(raw: &[u8], text_encoding: u32) -> String {
    match text_encoding {
        2 => decode_utf16(raw, false), // UTF-16 little-endian
        3 => decode_utf16(raw, true),  // UTF-16 big-endian
        _ => String::from_utf8_lossy(raw).into_owned(),
    }
}

fn decode_utf16(raw: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| {
            if big_endian {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Read a SQLite variable-length integer (big-endian base-128, ≤9 bytes).
fn read_varint(buf: &[u8], at: usize) -> Result<(u64, usize), String> {
    let mut result: u64 = 0;
    let mut i = 0;
    while i < 8 {
        let byte = *buf
            .get(at + i)
            .ok_or("corrupt database: truncated varint")?;
        result = (result << 7) | (byte & 0x7f) as u64;
        i += 1;
        if byte & 0x80 == 0 {
            return Ok((result, i));
        }
    }
    // The 9th byte contributes all 8 bits.
    let byte = *buf
        .get(at + 8)
        .ok_or("corrupt database: truncated varint")?;
    result = (result << 8) | byte as u64;
    Ok((result, 9))
}

fn be_u16(buf: &[u8], at: usize) -> Result<u16, String> {
    let b = buf
        .get(at..at + 2)
        .ok_or("corrupt database: truncated u16")?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

fn be_u32(buf: &[u8], at: usize) -> Result<u32, String> {
    let b = buf
        .get(at..at + 4)
        .ok_or("corrupt database: truncated u32")?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

// ---------------------------------------------------------------------------
// CREATE TABLE parsing (column names + rowid alias)
// ---------------------------------------------------------------------------

/// Returns `true` if a CREATE TABLE statement declares `WITHOUT ROWID`.
fn sql_has_without_rowid(sql: &str) -> bool {
    // "WITHOUT ROWID" only appears as a trailing table option, so a simple
    // case-insensitive substring test is adequate.
    sql.to_ascii_uppercase().contains("WITHOUT ROWID")
}

/// Parse column names (and the `INTEGER PRIMARY KEY` rowid-alias index) from a
/// `CREATE TABLE …` statement. Table-level constraints are skipped.
fn parse_create_columns(sql: &str) -> (Vec<String>, Option<usize>) {
    let Some(inner) = extract_paren_body(sql) else {
        return (Vec::new(), None);
    };
    let parts = split_top_level(&inner);
    let mut columns = Vec::new();
    let mut rowid_alias = None;
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, rest) = read_identifier(part);
        if name.is_empty() {
            continue;
        }
        // Skip table-level constraints (they aren't columns).
        let kw = name.to_ascii_uppercase();
        if matches!(
            kw.as_str(),
            "PRIMARY" | "UNIQUE" | "CHECK" | "FOREIGN" | "CONSTRAINT"
        ) {
            continue;
        }
        let idx = columns.len();
        // Rowid alias: exactly `INTEGER PRIMARY KEY` on this single column.
        let rest_up = rest.trim_start().to_ascii_uppercase();
        if rest_up.starts_with("INTEGER") && rest_up.contains("PRIMARY") && rest_up.contains("KEY") {
            rowid_alias = Some(idx);
        }
        columns.push(name);
    }
    (columns, rowid_alias)
}

/// Extract the substring between the first top-level `(` and its matching `)`.
fn extract_paren_body(sql: &str) -> Option<String> {
    let chars: Vec<char> = sql.chars().collect();
    let start_idx = chars.iter().position(|&c| c == '(')?;
    let mut depth = 0i32;
    let mut buf = String::new();
    let mut in_quote: Option<char> = None;
    for &c in &chars[start_idx..] {
        if let Some(q) = in_quote {
            buf.push(c);
            if c == q {
                in_quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                in_quote = Some(c);
                buf.push(c);
            }
            '[' => {
                in_quote = Some(']');
                buf.push(c);
            }
            '(' => {
                depth += 1;
                if depth > 1 {
                    buf.push(c);
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(buf);
                }
                buf.push(c);
            }
            _ => buf.push(c),
        }
    }
    None
}

/// Split a column-definition list on top-level commas (respecting nested parens
/// and quoted identifiers/strings).
fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut depth = 0i32;
    let mut in_quote: Option<char> = None;
    for c in s.chars() {
        if let Some(q) = in_quote {
            buf.push(c);
            if c == q {
                in_quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                in_quote = Some(c);
                buf.push(c);
            }
            '[' => {
                in_quote = Some(']');
                buf.push(c);
            }
            '(' => {
                depth += 1;
                buf.push(c);
            }
            ')' => {
                depth -= 1;
                buf.push(c);
            }
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut buf));
            }
            _ => buf.push(c),
        }
    }
    if !buf.trim().is_empty() {
        out.push(buf);
    }
    out
}

/// Read a leading (possibly quoted) identifier from a column definition,
/// returning `(name, remainder)`.
fn read_identifier(part: &str) -> (String, &str) {
    let trimmed = part.trim_start();
    let offset = part.len() - trimmed.len();
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return (String::new(), "");
    }
    let (is_quoted, close) = match bytes[0] {
        b'"' => (true, b'"'),
        b'`' => (true, b'`'),
        b'[' => (true, b']'),
        _ => (false, 0),
    };
    if is_quoted {
        // Quoted identifier: read until the matching close quote (a doubled
        // quote inside `"..."`/backticks escapes it; `[...]` has no doubling).
        let mut name = String::new();
        let mut i = 1;
        while i < trimmed.len() {
            let c = trimmed.as_bytes()[i];
            if c == close {
                if close != b']' && i + 1 < trimmed.len() && trimmed.as_bytes()[i + 1] == close {
                    name.push(close as char);
                    i += 2;
                    continue;
                }
                i += 1;
                break;
            }
            name.push(c as char);
            i += 1;
        }
        (name, &part[offset + i..])
    } else {
        // Bare identifier: up to whitespace, `(`, or `,`.
        let end = trimmed
            .find(|c: char| c.is_whitespace() || c == '(' || c == ',')
            .unwrap_or(trimmed.len());
        (trimmed[..end].to_string(), &part[offset + end..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real SQLite file built with Python's sqlite3 (see
    /// `core/tests/fixtures/gen_sample.py`). Contains:
    ///   - `people`  : id INTEGER PRIMARY KEY, name TEXT, score REAL, bio TEXT,
    ///                 avatar BLOB, notes TEXT (rowid alias + quoting + NULL + blob)
    ///   - `empties` : one column, zero rows
    ///   - `big`     : id INTEGER PRIMARY KEY, blob TEXT (one huge cell → overflow)
    ///   - `many`    : id INTEGER PRIMARY KEY, n INTEGER (2000 rows → interior page)
    const SAMPLE: &[u8] = include_bytes!("../tests/fixtures/sample.db");

    #[test]
    fn lists_user_tables_only() {
        let tables = list_tables(SAMPLE).unwrap();
        assert_eq!(tables, vec!["people", "empties", "big", "many"]);
    }

    #[test]
    fn exports_people_with_types_quoting_and_rowid_alias() {
        let out = to_csv(
            SAMPLE,
            &Options {
                table: Some("people".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            out.columns,
            vec!["id", "name", "score", "bio", "avatar", "notes"]
        );
        // Header + 3 rows. rowid alias fills `id`; comma/quote fields quoted;
        // REAL prints without trailing .0; NULL → empty; blob → hex.
        let expected = "id,name,score,bio,avatar,notes\r\n\
1,Ada,9.5,\"Coder, poet\",00ff10,\r\n\
2,\"Bob \"\"the builder\"\"\",7,,,plain\r\n\
3,Zoë,42,,,unicode ✓\r\n";
        assert_eq!(out.csv, expected, "got:\n{}", out.csv);
        assert_eq!(out.row_count, 3);
        assert!(!out.truncated);
    }

    #[test]
    fn default_table_errors_when_ambiguous() {
        let err = to_csv(SAMPLE, &Options::default()).unwrap_err();
        assert!(err.contains("specify which one"), "got: {err}");
    }

    #[test]
    fn unknown_table_errors_with_list() {
        let err = to_csv(
            SAMPLE,
            &Options {
                table: Some("nope".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("no table named"), "got: {err}");
    }

    #[test]
    fn empty_table_yields_header_only() {
        let out = to_csv(
            SAMPLE,
            &Options {
                table: Some("empties".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.row_count, 0);
        assert_eq!(out.csv, "label\r\n");
    }

    #[test]
    fn header_can_be_omitted() {
        let out = to_csv(
            SAMPLE,
            &Options {
                table: Some("people".to_string()),
                header: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(out.csv.starts_with("1,Ada,9.5"), "got: {}", out.csv);
    }

    #[test]
    fn delimiter_and_null_value_and_bom() {
        let out = to_csv(
            SAMPLE,
            &Options {
                table: Some("people".to_string()),
                delimiter: Delimiter::Tab,
                null_value: "NULL".to_string(),
                bom: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(out.csv.starts_with('\u{feff}'), "BOM present");
        assert!(out.csv.contains("id\tname\tscore"), "tab-delimited header");
        // Row 1 `notes` is NULL → rendered as the null_value.
        assert!(out.csv.contains("00ff10\tNULL"), "got:\n{}", out.csv);
    }

    #[test]
    fn overflow_payload_reassembled() {
        let out = to_csv(
            SAMPLE,
            &Options {
                table: Some("big".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        // The single row holds a 5000-char string that spilled to overflow pages.
        let line = out.csv.lines().nth(1).unwrap();
        assert!(line.contains(&"x".repeat(5000)), "overflow text length wrong");
    }

    #[test]
    fn interior_pages_walked_in_rowid_order() {
        let out = to_csv(
            SAMPLE,
            &Options {
                table: Some("many".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.row_count, 2000);
        // First and last data rows are in rowid order (1..=2000).
        assert_eq!(out.csv.lines().nth(1).unwrap(), "1,10");
        assert_eq!(out.csv.lines().nth(2000).unwrap(), "2000,20000");
    }

    #[test]
    fn limit_truncates_rows() {
        let out = to_csv(
            SAMPLE,
            &Options {
                table: Some("many".to_string()),
                limit: 5,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.row_count, 5);
        assert!(out.truncated);
        assert_eq!(out.csv.lines().count(), 6); // header + 5
    }

    #[test]
    fn rejects_non_sqlite_bytes() {
        let err = to_csv(b"this is not a database", &Options::default()).unwrap_err();
        assert!(err.contains("SQLite"), "got: {err}");
    }

    #[test]
    fn delimiter_parse_accepts_names_and_symbols() {
        assert_eq!(Delimiter::parse("comma").unwrap(), Delimiter::Comma);
        assert_eq!(Delimiter::parse("TAB").unwrap(), Delimiter::Tab);
        assert_eq!(Delimiter::parse(";").unwrap(), Delimiter::Semicolon);
        assert!(Delimiter::parse("colon").is_err());
    }

    #[test]
    fn read_table_returns_typed_rows_with_rowid_alias() {
        let t = read_table(SAMPLE, "people").unwrap();
        assert_eq!(t.columns, vec!["id", "name", "score", "bio", "avatar", "notes"]);
        assert_eq!(t.rows.len(), 3);
        // rowid alias filled the `id` column; typed accessors work.
        let id = t.col_index("ID").unwrap(); // case-insensitive
        assert_eq!(t.rows[0][id].as_i64(), Some(1));
        let name = t.col_index("name").unwrap();
        assert_eq!(t.rows[0][name].as_text(), Some("Ada"));
        // NULL notes on row 1.
        let notes = t.col_index("notes").unwrap();
        assert!(t.rows[0][notes].is_null());
    }

    #[test]
    fn read_table_unknown_errors() {
        assert!(read_table(SAMPLE, "nope").is_err());
    }

    #[test]
    fn master_entries_expose_all_objects() {
        let entries = master_entries(SAMPLE).unwrap();
        // Every user table appears as a "table" entry, in creation order.
        let tables: Vec<&str> = entries
            .iter()
            .filter(|e| e.kind == "table" && !e.name.starts_with("sqlite_"))
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(tables, vec!["people", "empties", "big", "many"]);
        // A table entry carries its CREATE SQL and a real rootpage.
        let people = entries.iter().find(|e| e.name == "people").unwrap();
        assert!(people.sql.to_uppercase().contains("CREATE TABLE"));
        assert!(people.rootpage.is_some());
        assert_eq!(people.tbl_name, "people");
    }

    #[test]
    fn count_rows_matches_table_sizes() {
        assert_eq!(count_rows(SAMPLE, "people").unwrap(), Some(3));
        assert_eq!(count_rows(SAMPLE, "empties").unwrap(), Some(0));
        assert_eq!(count_rows(SAMPLE, "many").unwrap(), Some(2000));
        // Case-insensitive, like the other lookups.
        assert_eq!(count_rows(SAMPLE, "MANY").unwrap(), Some(2000));
    }

    #[test]
    fn count_rows_unknown_table_errors() {
        assert!(count_rows(SAMPLE, "nope").is_err());
    }

    #[test]
    fn parse_create_columns_handles_quotes_and_alias() {
        let sql =
            "CREATE TABLE \"t\" (\"id\" INTEGER PRIMARY KEY, \"first, last\" TEXT, PRIMARY KEY(id))";
        let (cols, alias) = parse_create_columns(sql);
        assert_eq!(cols, vec!["id", "first, last"]);
        assert_eq!(alias, Some(0));
    }
}
