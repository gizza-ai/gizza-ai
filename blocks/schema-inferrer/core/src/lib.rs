//! schema-inferrer core — from a sample dataset (CSV/delimited text or JSON
//! records), infer a per-column type + nullability and emit BOTH a standard
//! JSON Schema (Draft 2020-12 / Draft-07) and a SQL `CREATE TABLE` statement.
//! Pure compute, no wafer/wasm-bindgen deps; shared by the chat skill block and
//! the web page.
//!
//! This tool is schema/DDL-only: it never emits INSERT rows (that is
//! `blocks/csv-to-sql`'s job). The CSV parser (RFC-4180-ish, delimiter + quote
//! aware) and the per-cell type recognizers (int / float / bool / date /
//! datetime / text, zero-padded codes kept as text) mirror the proven
//! `csv-to-sql` / `csv-type-inferrer` cores; the JSON-Schema string-`format`
//! detectors (email / uri / uuid / ipv4) mirror `json-to-json-schema`.

use serde_json::{Map, Value};

/// Table / schema title used when the caller passes a blank `table`.
pub const DEFAULT_TABLE: &str = "my_table";

/// Candidate delimiters tried during auto-detection, in preference order.
const CANDIDATE_DELIMS: [char; 4] = [',', ';', '\t', '|'];

/// Which artifact(s) to emit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// JSON Schema then the CREATE TABLE (default).
    Both,
    /// The inferred JSON Schema only.
    JsonSchema,
    /// The inferred SQL CREATE TABLE only.
    Sql,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" | "all" => Ok(Output::Both),
            "json-schema" | "json_schema" | "jsonschema" | "schema" | "json" => {
                Ok(Output::JsonSchema)
            }
            "sql" | "create-table" | "create_table" | "ddl" => Ok(Output::Sql),
            other => Err(format!(
                "invalid output {other:?}: expected \"both\", \"json-schema\", or \"sql\""
            )),
        }
    }
}

/// JSON Schema dialect to emit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Draft {
    Draft2020,
    Draft07,
}

impl Draft {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "2020-12" | "2020" | "draft-2020-12" | "draft2020" => Ok(Draft::Draft2020),
            "draft-07" | "draft07" | "07" | "7" | "draft-7" => Ok(Draft::Draft07),
            other => Err(format!(
                "invalid draft {other:?}: expected \"2020-12\" or \"draft-07\""
            )),
        }
    }

    fn uri(self) -> &'static str {
        match self {
            Draft::Draft2020 => "https://json-schema.org/draft/2020-12/schema",
            Draft::Draft07 => "http://json-schema.org/draft-07/schema#",
        }
    }

    /// Draft-07 has no `uuid` string format; 2020-12 does.
    fn supports_uuid(self) -> bool {
        self == Draft::Draft2020
    }
}

/// SQL dialect: drives identifier quoting and inferred column types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialect {
    Mysql,
    Postgres,
    Sqlite,
    Mssql,
    Ansi,
}

impl Dialect {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mysql" | "mariadb" => Ok(Dialect::Mysql),
            "" | "postgres" | "postgresql" | "pg" => Ok(Dialect::Postgres),
            "sqlite" => Ok(Dialect::Sqlite),
            "mssql" | "sqlserver" | "sql-server" | "tsql" => Ok(Dialect::Mssql),
            "ansi" | "standard" | "generic" => Ok(Dialect::Ansi),
            other => Err(format!(
                "invalid dialect {other:?}: expected \"mysql\", \"postgres\", \"sqlite\", \"mssql\", or \"ansi\""
            )),
        }
    }

    /// Quote a single identifier PART (no dots), escaping the closing quote.
    fn quote_part(self, ident: &str) -> String {
        match self {
            Dialect::Mysql => format!("`{}`", ident.replace('`', "``")),
            Dialect::Mssql => format!("[{}]", ident.replace(']', "]]")),
            _ => format!("\"{}\"", ident.replace('"', "\"\"")),
        }
    }

    /// Column type keyword for an inferred [`ColType`], used by `CREATE TABLE`.
    fn column_type(self, t: ColType) -> &'static str {
        match t {
            ColType::Integer => match self {
                Dialect::Mysql | Dialect::Mssql => "INT",
                _ => "INTEGER",
            },
            ColType::Float => match self {
                Dialect::Mysql => "DOUBLE",
                Dialect::Postgres | Dialect::Ansi => "DOUBLE PRECISION",
                Dialect::Sqlite => "REAL",
                Dialect::Mssql => "FLOAT",
            },
            ColType::Boolean => match self {
                Dialect::Postgres | Dialect::Ansi => "BOOLEAN",
                Dialect::Mysql => "TINYINT(1)",
                Dialect::Sqlite => "INTEGER",
                Dialect::Mssql => "BIT",
            },
            ColType::Date => match self {
                Dialect::Sqlite => "TEXT",
                _ => "DATE",
            },
            ColType::Datetime => match self {
                Dialect::Mysql => "DATETIME",
                Dialect::Postgres | Dialect::Ansi => "TIMESTAMP",
                Dialect::Sqlite => "TEXT",
                Dialect::Mssql => "DATETIME2",
            },
            ColType::Text => match self {
                Dialect::Mysql | Dialect::Postgres | Dialect::Sqlite => "TEXT",
                Dialect::Mssql => "NVARCHAR(255)",
                Dialect::Ansi => "VARCHAR(255)",
            },
        }
    }
}

/// Which input the text is parsed as.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Csv,
    Json,
    Auto,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Format::Auto),
            "csv" => Ok(Format::Csv),
            "json" => Ok(Format::Json),
            other => Err(format!(
                "invalid format {other:?}: expected \"auto\", \"csv\", or \"json\""
            )),
        }
    }
}

fn resolve_delim(delimiter: &str, text: &str) -> Result<char, String> {
    match delimiter.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(sniff_delimiter(text)),
        "comma" | "," => Ok(','),
        "tab" | "\\t" | "\t" => Ok('\t'),
        "semicolon" | ";" => Ok(';'),
        "pipe" | "|" => Ok('|'),
        other => Err(format!(
            "invalid delimiter {other:?}: expected \"auto\", \"comma\", \"tab\", \"semicolon\", or \"pipe\""
        )),
    }
}

/// Inferred column category.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ColType {
    Integer,
    Float,
    Boolean,
    Date,
    Datetime,
    Text,
}

/// A detected JSON-Schema string `format` for a Text column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum StrFormat {
    Email,
    Uri,
    Uuid,
    Ipv4,
}

impl StrFormat {
    fn as_str(self) -> &'static str {
        match self {
            StrFormat::Email => "email",
            StrFormat::Uri => "uri",
            StrFormat::Uuid => "uuid",
            StrFormat::Ipv4 => "ipv4",
        }
    }
}

/// Everything the generator needs, already parsed from the raw string params.
pub struct Options<'a> {
    pub format: Format,
    pub delimiter: &'a str,
    pub has_header: bool,
    pub output: Output,
    pub table: &'a str,
    pub dialect: Dialect,
    pub draft: Draft,
    pub primary_key: &'a str,
    pub not_null: bool,
    pub detect_formats: bool,
}

/// A parsed table: column names plus each row's already-trimmed string cells
/// (a `None` cell is a blank/null; short rows are padded with `None`).
struct Table {
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
}

/// One column's inferred schema.
struct ColumnInfo {
    name: String,
    ty: ColType,
    format: Option<StrFormat>,
    nullable: bool,
}

/// Parse the string-typed params (the shape every surface passes) and infer.
#[allow(clippy::too_many_arguments)]
pub fn infer_from_str(
    data: &str,
    format: &str,
    delimiter: &str,
    has_header: bool,
    output: &str,
    table: &str,
    dialect: &str,
    draft: &str,
    primary_key: &str,
    not_null: bool,
    detect_formats: bool,
) -> Result<String, String> {
    let opts = Options {
        format: Format::parse(format)?,
        delimiter,
        has_header,
        output: Output::parse(output)?,
        table,
        dialect: Dialect::parse(dialect)?,
        draft: Draft::parse(draft)?,
        primary_key,
        not_null,
        detect_formats,
    };
    infer(data, &opts)
}

/// Infer a schema for `data` under `opts` and render the requested artifact(s).
pub fn infer(data: &str, opts: &Options) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty: paste a CSV or JSON sample dataset".to_string());
    }

    let table = build_table(data, opts)?;
    if table.columns.is_empty() {
        return Err("no columns found in the input".to_string());
    }

    let pk = opts.primary_key.trim();
    if !pk.is_empty() && !table.columns.iter().any(|c| c == pk) {
        return Err(format!(
            "primary_key {pk:?} is not one of the columns ({})",
            table.columns.join(", ")
        ));
    }

    let cols: Vec<ColumnInfo> = (0..table.columns.len())
        .map(|i| infer_column(&table, i, opts))
        .collect();

    let table_name = if opts.table.trim().is_empty() {
        DEFAULT_TABLE
    } else {
        opts.table.trim()
    };

    match opts.output {
        Output::JsonSchema => Ok(json_schema(&cols, table_name, pk, opts)),
        Output::Sql => Ok(create_table_sql(&cols, table_name, pk, opts)?),
        Output::Both => {
            let schema = json_schema(&cols, table_name, pk, opts);
            let sql = create_table_sql(&cols, table_name, pk, opts)?;
            Ok(format!("{schema}\n\n{sql}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Input parsing (CSV or JSON) → Table
// ---------------------------------------------------------------------------

fn build_table(input: &str, opts: &Options) -> Result<Table, String> {
    let format = match opts.format {
        Format::Auto => sniff_format(input),
        other => other,
    };
    match format {
        Format::Json => table_from_json(input),
        _ => table_from_csv(input, opts),
    }
}

/// Auto-detect: a leading `{` or `[` is treated as JSON, otherwise CSV.
fn sniff_format(input: &str) -> Format {
    match input.trim_start().chars().next() {
        Some('{') | Some('[') => Format::Json,
        _ => Format::Csv,
    }
}

fn table_from_csv(input: &str, opts: &Options) -> Result<Table, String> {
    let delim = resolve_delim(opts.delimiter, input)?;
    let quote = sniff_quote(input, delim);
    let grid = parse_csv(input, delim, quote);
    if grid.is_empty() {
        return Err("no rows found in the CSV input".to_string());
    }

    let (columns, data): (Vec<String>, &[Vec<String>]) = if opts.has_header {
        let cols = dedupe_headers(&grid[0]);
        (cols, &grid[1..])
    } else {
        let ncol = grid.iter().map(|r| r.len()).max().unwrap_or(0);
        let cols = (1..=ncol).map(|i| format!("column_{i}")).collect();
        (cols, &grid[..])
    };

    let ncol = columns.len();
    let rows: Vec<Vec<Option<String>>> = data
        .iter()
        .map(|r| {
            (0..ncol)
                .map(|i| match r.get(i) {
                    Some(s) if s.is_empty() => None,
                    Some(s) => Some(s.clone()),
                    None => None,
                })
                .collect()
        })
        .collect();

    Ok(Table { columns, rows })
}

/// Build unique, non-empty column names from a CSV header row.
fn dedupe_headers(header: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(header.len());
    for (i, raw) in header.iter().enumerate() {
        let base = if raw.trim().is_empty() {
            format!("column_{}", i + 1)
        } else {
            raw.trim().to_string()
        };
        let mut name = base.clone();
        let mut n = 2;
        while out.iter().any(|c| c == &name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        out.push(name);
    }
    out
}

/// Parse a JSON object (one row) or array of objects (one row each) into a
/// [`Table`]. A JSON `null` or a missing key becomes a `None` cell.
fn table_from_json(input: &str) -> Result<Table, String> {
    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
    let objects = match value {
        Value::Object(map) => vec![map],
        Value::Array(items) => {
            if items.is_empty() {
                return Err("input is an empty array: nothing to infer a schema from".to_string());
            }
            let mut rows = Vec::with_capacity(items.len());
            for (i, item) in items.into_iter().enumerate() {
                match item {
                    Value::Object(map) => rows.push(map),
                    _ => return Err(format!(
                        "JSON array item {i} is not an object — each record must be a JSON object"
                    )),
                }
            }
            rows
        }
        _ => {
            return Err(
                "expected a JSON object or an array of objects (or paste CSV instead)".to_string(),
            )
        }
    };

    // Column order = union of every object's keys, in first-seen order.
    let mut columns: Vec<String> = Vec::new();
    for obj in &objects {
        for key in obj.keys() {
            if !columns.iter().any(|c| c == key) {
                columns.push(key.clone());
            }
        }
    }

    let rows: Vec<Vec<Option<String>>> = objects
        .iter()
        .map(|obj| {
            columns
                .iter()
                .map(|col| match obj.get(col) {
                    None | Some(Value::Null) => None,
                    Some(v) => Some(json_cell_to_string(v)),
                })
                .collect()
        })
        .collect();

    Ok(Table { columns, rows })
}

/// Stringify a JSON cell so it can flow through the same string-based type
/// inference the CSV path uses. Nested arrays/objects become compact JSON.
fn json_cell_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// CSV parsing (RFC-4180-ish, delimiter + quote aware)
// ---------------------------------------------------------------------------

/// Parse `text` into rows of fields using `delim` and `quote`.
pub fn parse_csv(text: &str, delim: char, quote: char) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut field_started = false;
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;

    let push_field = |field: &mut String, row: &mut Vec<String>| {
        row.push(std::mem::take(field).trim().to_string());
    };

    while i < n {
        let c = chars[i];
        if in_quotes {
            if c == quote {
                if i + 1 < n && chars[i + 1] == quote {
                    field.push(quote);
                    i += 2;
                    continue;
                }
                in_quotes = false;
                i += 1;
                continue;
            }
            field.push(c);
            i += 1;
            continue;
        }
        if c == quote && field.is_empty() {
            in_quotes = true;
            field_started = true;
            i += 1;
        } else if c == delim {
            push_field(&mut field, &mut row);
            field_started = true;
            i += 1;
        } else if c == '\n' || c == '\r' {
            if field_started || !field.is_empty() || !row.is_empty() {
                push_field(&mut field, &mut row);
                rows.push(std::mem::take(&mut row));
            }
            field_started = false;
            if c == '\r' && i + 1 < n && chars[i + 1] == '\n' {
                i += 2;
            } else {
                i += 1;
            }
        } else {
            field.push(c);
            field_started = true;
            i += 1;
        }
    }
    if field_started || !field.is_empty() || !row.is_empty() {
        push_field(&mut field, &mut row);
        rows.push(row);
    }

    rows.retain(|r| !r.iter().all(|f| f.is_empty()));
    rows
}

fn delim_score(text: &str, delim: char) -> (usize, f64) {
    let rows = parse_csv(text, delim, '"');
    if rows.is_empty() {
        return (0, 0.0);
    }
    let mut freq: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for r in &rows {
        *freq.entry(r.len()).or_insert(0) += 1;
    }
    let (&modal, &hits) = freq.iter().max_by_key(|(_, &c)| c).unwrap();
    (modal, hits as f64 / rows.len() as f64)
}

/// Pick the delimiter that yields the most consistent, widest table.
pub fn sniff_delimiter(text: &str) -> char {
    let mut best: Option<(char, usize, f64)> = None;
    for d in CANDIDATE_DELIMS {
        let (modal, consistency) = delim_score(text, d);
        if modal < 2 {
            continue;
        }
        let better = match best {
            None => true,
            Some((_, bm, bc)) => (consistency, modal) > (bc, bm),
        };
        if better {
            best = Some((d, modal, consistency));
        }
    }
    best.map(|(d, _, _)| d).unwrap_or(',')
}

/// Detect the field-quoting character (`"` or `'`). Defaults to `"`.
pub fn sniff_quote(text: &str, delim: char) -> char {
    let mut dq = 0i64;
    let mut sq = 0i64;
    let mut at_field_start = true;
    for c in text.chars() {
        if at_field_start {
            if c == '"' {
                dq += 1;
            } else if c == '\'' {
                sq += 1;
            }
        }
        at_field_start = c == delim || c == '\n' || c == '\r';
    }
    if sq > dq {
        '\''
    } else {
        '"'
    }
}

// ---------------------------------------------------------------------------
// Scalar recognizers
// ---------------------------------------------------------------------------

fn is_bool(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "t" | "f"
    )
}

/// Parse a base-10 integer, rejecting zero-padded codes (`007`) so IDs / ZIP
/// codes stay text.
fn parse_int(s: &str) -> Option<i64> {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if body.len() > 1 && body.starts_with('0') {
        return None;
    }
    s.parse::<i64>().ok()
}

/// Parse a finite float. Rejects textual `nan`/`inf` and zero-padded codes.
fn parse_float(s: &str) -> Option<f64> {
    match s.to_ascii_lowercase().as_str() {
        "nan" | "inf" | "+inf" | "-inf" | "infinity" | "+infinity" | "-infinity" => return None,
        _ => {}
    }
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if body.len() > 1 && body.starts_with('0') && body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match s.parse::<f64>() {
        Ok(v) if v.is_finite() => Some(v),
        _ => None,
    }
}

// ---- date / datetime recognizers ----

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}
fn valid_ymd(y: i64, m: i64, d: i64) -> bool {
    (1..=9999).contains(&y) && (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}
fn split3(s: &str, sep: char) -> Option<(&str, &str, &str)> {
    let mut it = s.split(sep);
    let a = it.next()?;
    let b = it.next()?;
    let c = it.next()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}
fn digits(s: &str, len: usize) -> Option<i64> {
    if s.len() == len && s.bytes().all(|b| b.is_ascii_digit()) {
        s.parse().ok()
    } else {
        None
    }
}
fn date_from(a: &str, b: &str, c: &str, order: &str) -> bool {
    match order {
        "ymd" => matches!(
            (digits(a, 4), digits(b, 2), digits(c, 2)),
            (Some(y), Some(m), Some(d)) if valid_ymd(y, m, d)
        ),
        "mdy" => matches!(
            (digits(a, 2), digits(b, 2), digits(c, 4)),
            (Some(m), Some(d), Some(y)) if valid_ymd(y, m, d)
        ),
        "dmy" => matches!(
            (digits(a, 2), digits(b, 2), digits(c, 4)),
            (Some(d), Some(m), Some(y)) if valid_ymd(y, m, d)
        ),
        _ => false,
    }
}

const DATE_FORMATS: [(char, &str); 6] = [
    ('-', "ymd"),
    ('/', "ymd"),
    ('/', "mdy"),
    ('/', "dmy"),
    ('-', "mdy"),
    ('-', "dmy"),
];

fn match_date(s: &str, sep: char, order: &str) -> bool {
    match split3(s, sep) {
        Some((a, b, c)) => date_from(a, b, c, order),
        None => false,
    }
}
fn is_date(s: &str) -> bool {
    DATE_FORMATS
        .iter()
        .any(|(sep, order)| match_date(s, *sep, order))
}

fn valid_time(tp: &str) -> bool {
    let (main, frac) = match tp.split_once('.') {
        Some((m, f)) => (m, Some(f)),
        None => (tp, None),
    };
    if let Some(f) = frac {
        if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    if let Some((h, mi, se)) = split3(main, ':') {
        if let (Some(h), Some(mi), Some(se)) = (digits(h, 2), digits(mi, 2), digits(se, 2)) {
            return (0..=23).contains(&h) && (0..=59).contains(&mi) && (0..=60).contains(&se);
        }
    }
    false
}

fn is_datetime(s: &str) -> bool {
    let core = s.strip_suffix('Z').unwrap_or(s);
    let sep = if core.contains('T') {
        'T'
    } else if core.contains(' ') {
        ' '
    } else {
        return false;
    };
    match core.split_once(sep) {
        Some((date, time)) => match_date(date, '-', "ymd") && valid_time(time),
        None => false,
    }
}

// ---- string format recognizers (for JSON Schema `format`) ----

fn is_email(s: &str) -> bool {
    let (local, domain) = match s.split_once('@') {
        Some(p) => p,
        None => return false,
    };
    if local.is_empty() || domain.is_empty() || domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    let dot = match domain.rfind('.') {
        Some(i) => i,
        None => return false,
    };
    // A TLD of at least two letters after the last dot.
    domain[dot + 1..].len() >= 2
        && !s.chars().any(|c| c.is_whitespace())
        && domain[dot + 1..].bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_uri(s: &str) -> bool {
    let (scheme, rest) = match s.split_once("://") {
        Some(p) => p,
        None => return false,
    };
    !scheme.is_empty()
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
        && scheme
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic())
        && !rest.is_empty()
        && !s.chars().any(|c| c.is_whitespace())
}

fn is_uuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (i, b) in bytes.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            if *b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| {
        !p.is_empty()
            && p.len() <= 3
            && p.bytes().all(|b| b.is_ascii_digit())
            && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
            // reject leading zeros like "01" so we don't mislabel codes
            && !(p.len() > 1 && p.starts_with('0'))
    })
}

// ---------------------------------------------------------------------------
// Column inference
// ---------------------------------------------------------------------------

/// Infer a column's type, string-format, and nullability from its cells.
fn infer_column(table: &Table, idx: usize, opts: &Options) -> ColumnInfo {
    let name = table.columns[idx].clone();
    // A column is nullable if any row is missing a value for it, OR there are
    // zero data rows (we can't assert NOT NULL from no evidence).
    let nullable = table.rows.is_empty()
        || table
            .rows
            .iter()
            .any(|r| r.get(idx).map(|c| c.is_none()).unwrap_or(true));

    let cells: Vec<&str> = table
        .rows
        .iter()
        .filter_map(|r| r.get(idx).and_then(|c| c.as_deref()))
        .collect();

    if cells.is_empty() {
        return ColumnInfo {
            name,
            ty: ColType::Text,
            format: None,
            nullable: true,
        };
    }

    let ty = if cells.iter().all(|c| is_bool(c)) {
        ColType::Boolean
    } else if cells.iter().all(|c| parse_int(c).is_some()) {
        ColType::Integer
    } else if cells
        .iter()
        .all(|c| parse_int(c).is_some() || parse_float(c).is_some())
    {
        ColType::Float
    } else if cells.iter().all(|c| is_date(c)) {
        ColType::Date
    } else if cells.iter().all(|c| is_datetime(c)) {
        ColType::Datetime
    } else {
        ColType::Text
    };

    // String-format detection only applies to Text columns.
    let format = if opts.detect_formats && ty == ColType::Text {
        detect_format(&cells, opts.draft)
    } else {
        None
    };

    ColumnInfo {
        name,
        ty,
        format,
        nullable,
    }
}

/// The first string-format matched by ALL cells, or `None`.
fn detect_format(cells: &[&str], draft: Draft) -> Option<StrFormat> {
    let candidates: &[(StrFormat, fn(&str) -> bool)] = &[
        (StrFormat::Email, is_email),
        (StrFormat::Uri, is_uri),
        (StrFormat::Uuid, is_uuid),
        (StrFormat::Ipv4, is_ipv4),
    ];
    for &(fmt, test) in candidates {
        if fmt == StrFormat::Uuid && !draft.supports_uuid() {
            continue;
        }
        if cells.iter().all(|c| test(c)) {
            return Some(fmt);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// JSON Schema emission
// ---------------------------------------------------------------------------

/// The JSON Schema `type` keyword for an inferred [`ColType`].
fn json_type(t: ColType) -> &'static str {
    match t {
        ColType::Integer => "integer",
        ColType::Float => "number",
        ColType::Boolean => "boolean",
        // Dates/datetimes/text are all JSON strings (dates via `format`).
        ColType::Date | ColType::Datetime | ColType::Text => "string",
    }
}

fn json_schema(cols: &[ColumnInfo], table_name: &str, pk: &str, opts: &Options) -> String {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();

    for col in cols {
        let base = json_type(col.ty);
        let mut prop = Map::new();

        // A PRIMARY KEY column is implicitly NOT NULL, so treat it as required.
        let is_required = !col.nullable || col.name == pk;

        if is_required {
            prop.insert("type".to_string(), Value::String(base.to_string()));
        } else {
            // A nullable column allows JSON null: type becomes ["<base>", "null"].
            prop.insert(
                "type".to_string(),
                Value::Array(vec![
                    Value::String(base.to_string()),
                    Value::String("null".to_string()),
                ]),
            );
        }

        // `format` from the inferred type or the detected string format.
        let fmt = match col.ty {
            ColType::Date => Some("date"),
            ColType::Datetime => Some("date-time"),
            _ => col.format.map(|f| f.as_str()),
        };
        if let Some(f) = fmt {
            prop.insert("format".to_string(), Value::String(f.to_string()));
        }

        properties.insert(col.name.clone(), Value::Object(prop));
        if is_required {
            required.push(Value::String(col.name.clone()));
        }
    }

    let mut items = Map::new();
    items.insert("type".to_string(), Value::String("object".to_string()));
    items.insert("properties".to_string(), Value::Object(properties));
    items.insert("required".to_string(), Value::Array(required));
    items.insert("additionalProperties".to_string(), Value::Bool(false));

    let mut root = Map::new();
    root.insert(
        "$schema".to_string(),
        Value::String(opts.draft.uri().to_string()),
    );
    root.insert("title".to_string(), Value::String(table_name.to_string()));
    root.insert("type".to_string(), Value::String("array".to_string()));
    root.insert("items".to_string(), Value::Object(items));

    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// SQL CREATE TABLE emission
// ---------------------------------------------------------------------------

fn create_table_sql(
    cols: &[ColumnInfo],
    table_name: &str,
    pk: &str,
    opts: &Options,
) -> Result<String, String> {
    let table_sql = quote_qualified(table_name, opts.dialect)?;
    let mut lines: Vec<String> = Vec::new();
    for col in cols {
        let col_q = opts.dialect.quote_part(&col.name);
        let ty = opts.dialect.column_type(col.ty);
        // A PRIMARY KEY column is implicitly NOT NULL. Otherwise emit NOT NULL
        // only when `not_null` is on AND the column had a value in every row.
        let not_null = col.name == pk || (opts.not_null && !col.nullable);
        if not_null {
            lines.push(format!("  {col_q} {ty} NOT NULL"));
        } else {
            lines.push(format!("  {col_q} {ty}"));
        }
    }
    if !pk.is_empty() {
        let pk_q = opts.dialect.quote_part(pk);
        lines.push(format!("  PRIMARY KEY ({pk_q})"));
    }
    Ok(format!(
        "CREATE TABLE {table_sql} (\n{}\n);",
        lines.join(",\n")
    ))
}

/// Quote a possibly schema-qualified table name (`schema.table`).
fn quote_qualified(name: &str, dialect: Dialect) -> Result<String, String> {
    let parts: Vec<&str> = name.split('.').collect();
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        if part.is_empty() {
            return Err(format!(
                "invalid table name {name:?}: empty identifier part"
            ));
        }
        out.push(dialect.quote_part(part));
    }
    Ok(out.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(output: Output, table: &'a str) -> Options<'a> {
        Options {
            format: Format::Auto,
            delimiter: "auto",
            has_header: true,
            output,
            table,
            dialect: Dialect::Postgres,
            draft: Draft::Draft2020,
            primary_key: "",
            not_null: true,
            detect_formats: true,
        }
    }

    #[test]
    fn json_schema_from_csv_infers_types_and_required() {
        let o = opts(Output::JsonSchema, "users");
        let out = infer("id,name,score,active\n1,Alice,9.5,true\n2,Bob,7,false", &o).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["type"], "array");
        assert_eq!(v["title"], "users");
        assert_eq!(v["$schema"], "https://json-schema.org/draft/2020-12/schema");
        let items = &v["items"];
        assert_eq!(items["properties"]["id"]["type"], "integer");
        assert_eq!(items["properties"]["score"]["type"], "number");
        assert_eq!(items["properties"]["active"]["type"], "boolean");
        assert_eq!(items["properties"]["name"]["type"], "string");
        assert_eq!(items["additionalProperties"], Value::Bool(false));
        // Every column had a value in every row → all required.
        let req = items["required"].as_array().unwrap();
        assert_eq!(req.len(), 4);
    }

    #[test]
    fn sql_create_table_postgres_with_pk_and_not_null() {
        let mut o = opts(Output::Sql, "users");
        o.primary_key = "id";
        let sql = infer("id,name,score,joined\n1,Al,1.5,2024-01-02", &o).unwrap();
        assert_eq!(
            sql,
            "CREATE TABLE \"users\" (\n  \
             \"id\" INTEGER NOT NULL,\n  \
             \"name\" TEXT NOT NULL,\n  \
             \"score\" DOUBLE PRECISION NOT NULL,\n  \
             \"joined\" DATE NOT NULL,\n  \
             PRIMARY KEY (\"id\")\n);"
        );
    }

    #[test]
    fn nullable_column_from_blank_cell() {
        let o = opts(Output::Both, "t");
        let out = infer("a,b\n1,x\n2,", &o).unwrap();
        // JSON schema: b is nullable (["string","null"]) and not required.
        let schema_part = out.split("\n\nCREATE TABLE").next().unwrap();
        let v: Value = serde_json::from_str(schema_part).unwrap();
        assert_eq!(v["items"]["properties"]["b"]["type"][0], "string");
        assert_eq!(v["items"]["properties"]["b"]["type"][1], "null");
        let req = v["items"]["required"].as_array().unwrap();
        assert_eq!(req.len(), 1);
        assert_eq!(req[0], "a");
        // SQL: a is NOT NULL, b is nullable (no NOT NULL).
        assert!(out.contains("\"a\" INTEGER NOT NULL"), "got: {out}");
        assert!(
            out.contains("\"b\" TEXT,") || out.contains("\"b\" TEXT\n"),
            "got: {out}"
        );
    }

    #[test]
    fn json_input_auto_detected_and_missing_key_nullable() {
        let o = opts(Output::JsonSchema, "t");
        let out = infer(r#"[{"id":1,"name":"Alice"},{"id":2}]"#, &o).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["items"]["properties"]["id"]["type"], "integer");
        // name missing in the second object → nullable.
        assert_eq!(v["items"]["properties"]["name"]["type"][1], "null");
    }

    #[test]
    fn detects_string_formats() {
        let o = opts(Output::JsonSchema, "t");
        let out = infer(
            "email,site,uid,ip\na@b.com,https://x.io,550e8400-e29b-41d4-a716-446655440000,10.0.0.1\nc@d.org,http://y.net,550e8400-e29b-41d4-a716-446655440001,192.168.1.1",
            &o,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let p = &v["items"]["properties"];
        assert_eq!(p["email"]["format"], "email");
        assert_eq!(p["site"]["format"], "uri");
        assert_eq!(p["uid"]["format"], "uuid");
        assert_eq!(p["ip"]["format"], "ipv4");
    }

    #[test]
    fn draft07_uri_and_no_uuid_format() {
        let mut o = opts(Output::JsonSchema, "t");
        o.draft = Draft::Draft07;
        let out = infer(
            "uid\n550e8400-e29b-41d4-a716-446655440000\n550e8400-e29b-41d4-a716-446655440001",
            &o,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["$schema"], "http://json-schema.org/draft-07/schema#");
        // Draft-07 has no uuid format → the property carries no `format`.
        assert!(v["items"]["properties"]["uid"].get("format").is_none());
    }

    #[test]
    fn zero_padded_code_stays_text() {
        let o = opts(Output::Sql, "t");
        let sql = infer("zip\n01234\n00567", &o).unwrap();
        assert!(sql.contains("\"zip\" TEXT"), "got: {sql}");
    }

    #[test]
    fn datetime_maps_to_timestamp() {
        let o = opts(Output::Sql, "e");
        let sql = infer("ts\n2024-01-02 03:04:05\n2024-06-07 08:09:10", &o).unwrap();
        assert!(sql.contains("\"ts\" TIMESTAMP NOT NULL"), "got: {sql}");
    }

    #[test]
    fn mysql_dialect_types_and_backticks() {
        let mut o = opts(Output::Sql, "users");
        o.dialect = Dialect::Mysql;
        let sql = infer("id,active\n1,true", &o).unwrap();
        assert!(sql.contains("`id` INT NOT NULL"), "got: {sql}");
        assert!(sql.contains("`active` TINYINT(1) NOT NULL"), "got: {sql}");
    }

    #[test]
    fn not_null_off_omits_constraint() {
        let mut o = opts(Output::Sql, "t");
        o.not_null = false;
        let sql = infer("id\n1\n2", &o).unwrap();
        assert!(sql.contains("\"id\" INTEGER\n"), "got: {sql}");
        assert!(!sql.contains("NOT NULL"), "got: {sql}");
    }

    #[test]
    fn both_output_has_schema_then_sql() {
        let o = opts(Output::Both, "t");
        let out = infer("a\n1", &o).unwrap();
        assert!(out.starts_with('{'), "got: {out}");
        assert!(out.contains("\n\nCREATE TABLE \"t\" ("), "got: {out}");
    }

    #[test]
    fn schema_qualified_table_name() {
        let o = opts(Output::Sql, "public.users");
        let sql = infer("id\n1", &o).unwrap();
        assert!(
            sql.starts_with("CREATE TABLE \"public\".\"users\" ("),
            "got: {sql}"
        );
    }

    // ---- error paths ----

    #[test]
    fn err_on_empty_input() {
        let err = infer("   ", &opts(Output::Both, "t")).unwrap_err();
        assert!(err.contains("input is empty"), "got: {err}");
    }

    #[test]
    fn err_on_invalid_json() {
        let mut o = opts(Output::Both, "t");
        o.format = Format::Json;
        let err = infer("{not json}", &o).unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
    }

    #[test]
    fn err_on_json_array_of_scalars() {
        let err = infer("[1, 2, 3]", &opts(Output::Both, "t")).unwrap_err();
        assert!(err.contains("not an object"), "got: {err}");
    }

    #[test]
    fn err_on_bad_primary_key() {
        let mut o = opts(Output::Sql, "t");
        o.primary_key = "nope";
        let err = infer("a\n1", &o).unwrap_err();
        assert!(err.contains("not one of the columns"), "got: {err}");
    }

    #[test]
    fn err_on_bad_output() {
        let err = infer_from_str(
            "a\n1", "csv", "auto", true, "yaml", "t", "postgres", "2020-12", "", true, true,
        )
        .unwrap_err();
        assert!(err.contains("invalid output"), "got: {err}");
    }

    #[test]
    fn err_on_bad_dialect() {
        let err = infer_from_str(
            "a\n1", "csv", "auto", true, "sql", "t", "oracle", "2020-12", "", true, true,
        )
        .unwrap_err();
        assert!(err.contains("invalid dialect"), "got: {err}");
    }

    #[test]
    fn infer_from_str_happy() {
        let out = infer_from_str(
            "a,b\n1,x", "auto", "auto", true, "sql", "t", "sqlite", "2020-12", "", true, true,
        )
        .unwrap();
        assert_eq!(
            out,
            "CREATE TABLE \"t\" (\n  \"a\" INTEGER NOT NULL,\n  \"b\" TEXT NOT NULL\n);"
        );
    }
}
