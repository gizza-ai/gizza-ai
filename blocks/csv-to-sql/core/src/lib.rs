//! csv-to-sql core — turn a CSV (or JSON) table into SQL `CREATE TABLE` +
//! `INSERT` statements, with a column type inferred for every column. Pure
//! compute, no wafer/wasm-bindgen deps; shared by the chat skill block and the
//! web page.
//!
//! The CSV parser (RFC-4180-ish, delimiter + quote aware) and the per-cell type
//! recognizers (int / float / bool / date / datetime / text, with zero-padded
//! codes kept as text) mirror the proven `csv-type-inferrer` core. The SQL
//! emitter is dialect-aware (identifier quoting, boolean/string literals,
//! placeholder syntax, and per-dialect column types) like `json-to-sql-insert`,
//! but here types are inferred from *string* cells, so date/datetime columns map
//! to real SQL `DATE`/`TIMESTAMP` types rather than being stored as text.

use serde_json::Value;

/// Table name used when the caller passes a blank `table`.
pub const DEFAULT_TABLE: &str = "my_table";

/// Candidate delimiters tried during auto-detection, in preference order.
const CANDIDATE_DELIMS: [char; 4] = [',', ';', '\t', '|'];

/// SQL dialect: drives identifier quoting, boolean/string literal escaping,
/// placeholder syntax, and inferred column types for `CREATE TABLE`.
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
            "" | "mysql" | "mariadb" => Ok(Dialect::Mysql),
            "postgres" | "postgresql" | "pg" => Ok(Dialect::Postgres),
            "sqlite" => Ok(Dialect::Sqlite),
            "mssql" | "sqlserver" | "sql-server" | "tsql" => Ok(Dialect::Mssql),
            "ansi" | "standard" | "generic" => Ok(Dialect::Ansi),
            other => Err(format!(
                "invalid dialect {other:?}: expected \"mysql\", \"postgres\", \"sqlite\", \"mssql\", or \"ansi\""
            )),
        }
    }

    /// Quote a single identifier PART (no dots) for this dialect, escaping the
    /// closing quote by doubling it.
    fn quote_part(self, ident: &str) -> String {
        match self {
            Dialect::Mysql => format!("`{}`", ident.replace('`', "``")),
            Dialect::Mssql => format!("[{}]", ident.replace(']', "]]")),
            // Postgres / SQLite / ANSI all use double-quoted identifiers.
            _ => format!("\"{}\"", ident.replace('"', "\"\"")),
        }
    }

    /// Render a boolean cell as this dialect's literal. Postgres and ANSI SQL
    /// have a real boolean type (`TRUE`/`FALSE`); MySQL, SQLite, and SQL Server
    /// store booleans numerically, so `1`/`0` are the portable choice there.
    fn bool_literal(self, b: bool) -> &'static str {
        match self {
            Dialect::Postgres | Dialect::Ansi => {
                if b {
                    "TRUE"
                } else {
                    "FALSE"
                }
            }
            _ => {
                if b {
                    "1"
                } else {
                    "0"
                }
            }
        }
    }

    /// Wrap `s` as a single-quoted string literal, escaping per dialect. Every
    /// dialect doubles a single quote (`'` → `''`); MySQL additionally treats a
    /// backslash as an escape character by default, so it is doubled too.
    fn string_literal(self, s: &str) -> String {
        let escaped = match self {
            Dialect::Mysql => s.replace('\\', "\\\\").replace('\'', "''"),
            _ => s.replace('\'', "''"),
        };
        format!("'{escaped}'")
    }

    /// One positional placeholder, 1-based. Postgres uses `$n`, SQL Server uses
    /// `@pn`, and everyone else uses a bare `?`.
    fn placeholder(self, n: usize) -> String {
        match self {
            Dialect::Postgres => format!("${n}"),
            Dialect::Mssql => format!("@p{n}"),
            _ => "?".to_string(),
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
            // SQLite has no dedicated date type; it stores dates as TEXT.
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

/// How a blank / null cell is rendered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NullHandling {
    /// Emit the SQL keyword `NULL` (default).
    Null,
    /// Emit the SQL keyword `DEFAULT` so the column's own default applies.
    Default,
    /// Emit an empty string literal `''`.
    EmptyString,
}

impl NullHandling {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "null" => Ok(NullHandling::Null),
            "default" => Ok(NullHandling::Default),
            "empty-string" | "empty" | "empty_string" => Ok(NullHandling::EmptyString),
            other => Err(format!(
                "invalid null_handling {other:?}: expected \"null\", \"default\", or \"empty-string\""
            )),
        }
    }
}

/// Value output mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueMode {
    /// Inline SQL literals (default).
    Literal,
    /// Positional placeholders for a prepared statement; the bound values are
    /// listed in a trailing `-- params:` comment.
    Placeholder,
}

impl ValueMode {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "literal" | "literals" | "inline" => Ok(ValueMode::Literal),
            "placeholder" | "placeholders" | "prepared" | "parameterized" => {
                Ok(ValueMode::Placeholder)
            }
            other => Err(format!(
                "invalid values {other:?}: expected \"literal\" or \"placeholder\""
            )),
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

/// Inferred SQL column category (for `CREATE TABLE` type mapping).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ColType {
    Integer,
    Float,
    Boolean,
    Date,
    Datetime,
    Text,
}

/// Everything the generator needs, already parsed from the raw string params.
pub struct Options<'a> {
    pub format: Format,
    pub delimiter: &'a str,
    pub has_header: bool,
    pub table: &'a str,
    pub dialect: Dialect,
    pub value_mode: ValueMode,
    pub multi_row: bool,
    pub create_table: bool,
    pub drop_table: bool,
    pub primary_key: &'a str,
    pub quote_identifiers: bool,
    pub null_handling: NullHandling,
    pub infer_types: bool,
    pub detect_dates: bool,
}

/// A parsed table: column names plus each row's already-trimmed string cells
/// (a `None` cell is a blank/null; short rows are padded with `None`).
struct Table {
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
}

/// Parse the string-typed params (the shape every surface passes) into an
/// [`Options`], then generate. Booleans accept `true`/`1`/`yes`/`on`.
#[allow(clippy::too_many_arguments)]
pub fn generate_from_str(
    input: &str,
    format: &str,
    delimiter: &str,
    has_header: bool,
    table: &str,
    dialect: &str,
    values: &str,
    multi_row: bool,
    create_table: bool,
    drop_table: bool,
    primary_key: &str,
    quote_identifiers: bool,
    null_handling: &str,
    infer_types: bool,
    detect_dates: bool,
) -> Result<String, String> {
    let opts = Options {
        format: Format::parse(format)?,
        delimiter,
        has_header,
        table,
        dialect: Dialect::parse(dialect)?,
        value_mode: ValueMode::parse(values)?,
        multi_row,
        create_table,
        drop_table,
        primary_key,
        quote_identifiers,
        null_handling: NullHandling::parse(null_handling)?,
        infer_types,
        detect_dates,
    };
    generate(input, &opts)
}

/// Generate `CREATE TABLE`/`INSERT` SQL for `input` under `opts`.
pub fn generate(input: &str, opts: &Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty: paste a CSV or JSON table".to_string());
    }

    let table = build_table(input, opts)?;
    if table.columns.is_empty() {
        return Err("no columns found in the input".to_string());
    }

    let table_name = if opts.table.trim().is_empty() {
        DEFAULT_TABLE
    } else {
        opts.table.trim()
    };
    let table_sql = quote_qualified(table_name, opts.dialect, opts.quote_identifiers)?;

    // Validate the primary key names a real column before we build anything.
    let pk = opts.primary_key.trim();
    if !pk.is_empty() && !table.columns.iter().any(|c| c == pk) {
        return Err(format!(
            "primary_key {pk:?} is not one of the columns ({})",
            table.columns.join(", ")
        ));
    }

    let col_sql: Vec<String> = table
        .columns
        .iter()
        .map(|c| quote_ident(c, opts.dialect, opts.quote_identifiers))
        .collect::<Result<Vec<_>, _>>()?;

    // Infer a type per column (used for CREATE TABLE and for numeric/boolean
    // literal rendering). When type inference is off, every column is text.
    let col_types: Vec<ColType> = (0..table.columns.len())
        .map(|i| infer_column(&table.rows, i, opts))
        .collect();

    let mut out = String::new();

    if opts.drop_table {
        out.push_str(&format!("DROP TABLE IF EXISTS {table_sql};\n"));
    }
    if opts.create_table {
        out.push_str(&create_table_sql(&table_sql, &col_sql, &col_types, pk, opts));
        out.push('\n');
    }

    out.push_str(&insert_sql(&table_sql, &table, &col_sql, &col_types, opts));

    Ok(out)
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
        // Auto resolved to Csv, or Csv requested explicitly.
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

/// Build unique, non-empty column names from a CSV header row. A blank header
/// cell becomes `column_N`; a duplicate name gets a `_2`, `_3`, … suffix.
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
/// [`Table`]. Values are stringified for uniform type inference; a JSON `null`
/// or a missing key becomes a `None` cell.
fn table_from_json(input: &str) -> Result<Table, String> {
    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
    let objects = match value {
        Value::Object(map) => vec![map],
        Value::Array(items) => {
            if items.is_empty() {
                return Err("input is an empty array: nothing to insert".to_string());
            }
            let mut rows = Vec::with_capacity(items.len());
            for (i, item) in items.into_iter().enumerate() {
                match item {
                    Value::Object(map) => rows.push(map),
                    _ => {
                        return Err(format!(
                            "JSON array item {i} is not an object — each row must be a JSON object"
                        ))
                    }
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

    // Column order = union of every object's keys, in first-seen order
    // (serde_json's preserve_order keeps per-object key order stable).
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

/// Parse `text` into rows of fields using `delim` and `quote`. Handles doubled
/// quotes (`""` → `"`) and quoted fields that span newlines. Fully-empty rows
/// (blank lines) are skipped. Surrounding whitespace of each field is trimmed.
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

/// Score how consistently `text` splits into columns under `delim`. Returns
/// `(modal_column_count, fraction_of_rows_at_modal)`.
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

/// Detect the field-quoting character (`"` or `'`) by counting quotes that open
/// a field. Defaults to `"`.
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

fn is_true(s: &str) -> bool {
    matches!(s.to_ascii_lowercase().as_str(), "true" | "yes" | "t" | "1")
}
fn is_bool(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "t" | "f"
    )
}

/// Parse a base-10 integer, but reject zero-padded codes (`007`, `01`) so ZIP
/// codes / IDs stay strings rather than losing their leading zero.
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

/// Supported date formats: `(separator, order)`.
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
    // `YYYY-MM-DDTHH:MM:SS[Z]` or `YYYY-MM-DD HH:MM:SS`.
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

// ---------------------------------------------------------------------------
// Column inference + literal rendering
// ---------------------------------------------------------------------------

/// Infer a column's SQL type from its non-null string cells. With type
/// inference off, everything is text. Bool → Int → Float → Date → Datetime →
/// Text, checked most-specific first; an empty column defaults to text.
fn infer_column(rows: &[Vec<Option<String>>], idx: usize, opts: &Options) -> ColType {
    if !opts.infer_types {
        return ColType::Text;
    }
    let cells: Vec<&str> = rows
        .iter()
        .filter_map(|r| r.get(idx).and_then(|c| c.as_deref()))
        .collect();
    if cells.is_empty() {
        return ColType::Text;
    }
    if cells.iter().all(|c| is_bool(c)) {
        return ColType::Boolean;
    }
    if cells.iter().all(|c| parse_int(c).is_some()) {
        return ColType::Integer;
    }
    if cells
        .iter()
        .all(|c| parse_int(c).is_some() || parse_float(c).is_some())
    {
        return ColType::Float;
    }
    if opts.detect_dates {
        if cells.iter().all(|c| is_date(c)) {
            return ColType::Date;
        }
        if cells.iter().all(|c| is_datetime(c)) {
            return ColType::Datetime;
        }
    }
    ColType::Text
}

/// The SQL literal for a single cell given its column's inferred type.
fn literal(cell: Option<&String>, ty: ColType, opts: &Options) -> String {
    let c = match cell {
        None => {
            return match opts.null_handling {
                NullHandling::Null => "NULL".to_string(),
                NullHandling::Default => "DEFAULT".to_string(),
                NullHandling::EmptyString => "''".to_string(),
            }
        }
        Some(c) => c,
    };
    match ty {
        // Integer/Float columns emit the numeric text verbatim when it parses;
        // any stray non-numeric cell falls back to a quoted string so output
        // never becomes invalid SQL.
        ColType::Integer => match parse_int(c) {
            Some(_) => c.clone(),
            None => opts.dialect.string_literal(c),
        },
        ColType::Float => {
            if parse_int(c).is_some() || parse_float(c).is_some() {
                c.clone()
            } else {
                opts.dialect.string_literal(c)
            }
        }
        ColType::Boolean => opts.dialect.bool_literal(is_true(c)).to_string(),
        // Dates/datetimes/text are all quoted string literals.
        _ => opts.dialect.string_literal(c),
    }
}

// ---------------------------------------------------------------------------
// SQL emission
// ---------------------------------------------------------------------------

fn create_table_sql(
    table_sql: &str,
    col_sql: &[String],
    col_types: &[ColType],
    pk: &str,
    opts: &Options,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for (col_q, ty) in col_sql.iter().zip(col_types) {
        lines.push(format!("  {col_q} {}", opts.dialect.column_type(*ty)));
    }
    if !pk.is_empty() {
        let pk_q =
            quote_ident(pk, opts.dialect, opts.quote_identifiers).unwrap_or_else(|_| pk.to_string());
        lines.push(format!("  PRIMARY KEY ({pk_q})"));
    }
    format!("CREATE TABLE {table_sql} (\n{}\n);", lines.join(",\n"))
}

fn insert_sql(
    table_sql: &str,
    table: &Table,
    col_sql: &[String],
    col_types: &[ColType],
    opts: &Options,
) -> String {
    let cols_joined = col_sql.join(", ");
    let mut out = String::new();

    if table.rows.is_empty() {
        // Header/columns only, no data rows: nothing to insert.
        return out;
    }

    if opts.multi_row {
        out.push_str(&format!("INSERT INTO {table_sql} ({cols_joined}) VALUES\n"));
        let mut params: Vec<String> = Vec::new();
        let mut ph = 0usize;
        let tuples: Vec<String> = table
            .rows
            .iter()
            .map(|row| row_tuple(row, col_types, opts, &mut ph, &mut params))
            .collect();
        out.push_str(&tuples.join(",\n"));
        out.push(';');
        append_params(&mut out, opts, &params);
        out.push('\n');
    } else {
        for row in &table.rows {
            let mut params: Vec<String> = Vec::new();
            let mut ph = 0usize;
            let tuple = row_tuple(row, col_types, opts, &mut ph, &mut params);
            out.push_str(&format!(
                "INSERT INTO {table_sql} ({cols_joined}) VALUES {tuple};"
            ));
            append_params(&mut out, opts, &params);
            out.push('\n');
        }
    }
    out
}

/// Render one `(v1, v2, ...)` tuple. In placeholder mode this emits placeholders
/// and pushes the literal renderings onto `params` (and advances `ph`).
fn row_tuple(
    row: &[Option<String>],
    col_types: &[ColType],
    opts: &Options,
    ph: &mut usize,
    params: &mut Vec<String>,
) -> String {
    let cells: Vec<String> = col_types
        .iter()
        .enumerate()
        .map(|(i, &ty)| {
            let cell = row.get(i).and_then(|c| c.as_ref());
            match opts.value_mode {
                ValueMode::Literal => literal(cell, ty, opts),
                ValueMode::Placeholder => {
                    *ph += 1;
                    params.push(literal(cell, ty, opts));
                    opts.dialect.placeholder(*ph)
                }
            }
        })
        .collect();
    format!("({})", cells.join(", "))
}

fn append_params(out: &mut String, opts: &Options, params: &[String]) {
    if opts.value_mode == ValueMode::Placeholder && !params.is_empty() {
        out.push_str(&format!("  -- params: {}", params.join(", ")));
    }
}

// ---------------------------------------------------------------------------
// Identifier quoting
// ---------------------------------------------------------------------------

/// Quote a possibly schema-qualified table name (`schema.table`), quoting each
/// dot-separated part. When `quote` is false, every part is validated instead.
fn quote_qualified(name: &str, dialect: Dialect, quote: bool) -> Result<String, String> {
    let parts: Vec<&str> = name.split('.').collect();
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        if part.is_empty() {
            return Err(format!("invalid table name {name:?}: empty identifier part"));
        }
        out.push(quote_ident(part, dialect, quote)?);
    }
    Ok(out.join("."))
}

fn quote_ident(ident: &str, dialect: Dialect, quote: bool) -> Result<String, String> {
    if ident.is_empty() {
        return Err("empty identifier".to_string());
    }
    if quote {
        Ok(dialect.quote_part(ident))
    } else if is_safe_bare_identifier(ident) {
        Ok(ident.to_string())
    } else {
        Err(format!(
            "identifier {ident:?} is not a safe bare identifier (letters, digits, and underscores; not starting with a digit) — enable identifier quoting to use it"
        ))
    }
}

/// A conservative "safe unquoted identifier": ASCII letter or `_` first, then
/// ASCII letters, digits, or `_`.
fn is_safe_bare_identifier(ident: &str) -> bool {
    let mut chars = ident.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(table: &'a str) -> Options<'a> {
        Options {
            format: Format::Auto,
            delimiter: "auto",
            has_header: true,
            table,
            dialect: Dialect::Mysql,
            value_mode: ValueMode::Literal,
            multi_row: true,
            create_table: false,
            drop_table: false,
            primary_key: "",
            quote_identifiers: true,
            null_handling: NullHandling::Null,
            infer_types: true,
            detect_dates: true,
        }
    }

    #[test]
    fn happy_csv_multi_row_mysql() {
        let sql = generate("id,name\n1,Alice\n2,Bob", &opts("users")).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `users` (`id`, `name`) VALUES\n(1, 'Alice'),\n(2, 'Bob');\n"
        );
    }

    #[test]
    fn create_table_infers_types_and_pk() {
        let mut o = opts("users");
        o.create_table = true;
        o.drop_table = true;
        o.primary_key = "id";
        let sql = generate("id,name,score,active\n1,Al,1.5,true\n2,Bo,3.0,false", &o).unwrap();
        assert_eq!(
            sql,
            "DROP TABLE IF EXISTS `users`;\n\
             CREATE TABLE `users` (\n  \
             `id` INT,\n  `name` TEXT,\n  `score` DOUBLE,\n  `active` TINYINT(1),\n  \
             PRIMARY KEY (`id`)\n);\n\
             INSERT INTO `users` (`id`, `name`, `score`, `active`) VALUES\n\
             (1, 'Al', 1.5, 1),\n(2, 'Bo', 3.0, 0);\n"
        );
    }

    #[test]
    fn detects_date_and_datetime_types_postgres() {
        let mut o = opts("events");
        o.dialect = Dialect::Postgres;
        o.create_table = true;
        let sql = generate(
            "d,ts\n2024-01-02,2024-01-02 03:04:05\n2024-06-07,2024-06-07 08:09:10",
            &o,
        )
        .unwrap();
        assert!(
            sql.contains("\"d\" DATE") && sql.contains("\"ts\" TIMESTAMP"),
            "got: {sql}"
        );
        assert!(
            sql.contains("('2024-01-02', '2024-01-02 03:04:05')"),
            "got: {sql}"
        );
    }

    #[test]
    fn json_input_auto_detected() {
        let sql =
            generate(r#"[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]"#, &opts("t")).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `t` (`id`, `name`) VALUES\n(1, 'Alice'),\n(2, 'Bob');\n"
        );
    }

    #[test]
    fn json_single_object_one_row() {
        let sql = generate(r#"{"a":1,"b":"x"}"#, &opts("t")).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`a`, `b`) VALUES\n(1, 'x');\n");
    }

    #[test]
    fn no_header_generates_column_names() {
        let mut o = opts("t");
        o.has_header = false;
        let sql = generate("1,Alice\n2,Bob", &o).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `t` (`column_1`, `column_2`) VALUES\n(1, 'Alice'),\n(2, 'Bob');\n"
        );
    }

    #[test]
    fn semicolon_delimiter_sniffed() {
        let sql = generate("a;b\n1;2\n3;4", &opts("t")).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`a`, `b`) VALUES\n(1, 2),\n(3, 4);\n");
    }

    #[test]
    fn explicit_tab_delimiter() {
        let mut o = opts("t");
        o.delimiter = "tab";
        let sql = generate("a\tb\n1\t2", &o).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`a`, `b`) VALUES\n(1, 2);\n");
    }

    #[test]
    fn quoted_field_with_comma_and_doubled_quote() {
        let sql = generate("name\n\"Smith, John\"\n\"say \"\"hi\"\"\"", &opts("t")).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `t` (`name`) VALUES\n('Smith, John'),\n('say \"hi\"');\n"
        );
    }

    #[test]
    fn placeholder_mode_postgres_lists_params() {
        let mut o = opts("t");
        o.dialect = Dialect::Postgres;
        o.value_mode = ValueMode::Placeholder;
        let sql = generate("a,b\n1,x", &o).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO \"t\" (\"a\", \"b\") VALUES\n($1, $2);  -- params: 1, 'x'\n"
        );
    }

    #[test]
    fn per_row_inserts_when_multi_row_off() {
        let mut o = opts("t");
        o.multi_row = false;
        let sql = generate("a\n1\n2", &o).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `t` (`a`) VALUES (1);\nINSERT INTO `t` (`a`) VALUES (2);\n"
        );
    }

    #[test]
    fn blank_cell_uses_null_handling() {
        let mut o = opts("t");
        o.null_handling = NullHandling::Default;
        let sql = generate("a,b\n1,\n2,y", &o).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `t` (`a`, `b`) VALUES\n(1, DEFAULT),\n(2, 'y');\n"
        );
    }

    #[test]
    fn zero_padded_code_stays_text() {
        let mut o = opts("t");
        o.create_table = true;
        let sql = generate("zip\n01234\n00567", &o).unwrap();
        assert!(sql.contains("`zip` TEXT"), "got: {sql}");
        assert!(sql.contains("('01234')"), "got: {sql}");
    }

    #[test]
    fn mysql_escapes_backslash_and_quote() {
        let sql = generate("p\na'b\\c", &opts("t")).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`p`) VALUES\n('a''b\\\\c');\n");
    }

    #[test]
    fn infer_types_off_everything_text() {
        let mut o = opts("t");
        o.infer_types = false;
        o.create_table = true;
        let sql = generate("id,active\n1,true", &o).unwrap();
        assert!(
            sql.contains("`id` TEXT") && sql.contains("`active` TEXT"),
            "got: {sql}"
        );
        assert!(sql.contains("('1', 'true')"), "got: {sql}");
    }

    #[test]
    fn duplicate_headers_deduped() {
        let sql = generate("a,a\n1,2", &opts("t")).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`a`, `a_2`) VALUES\n(1, 2);\n");
    }

    #[test]
    fn blank_table_uses_default() {
        let sql = generate("a\n1", &opts("")).unwrap();
        assert!(sql.starts_with("INSERT INTO `my_table`"), "got: {sql}");
    }

    #[test]
    fn unquoted_identifiers_when_safe() {
        let mut o = opts("public.users");
        o.quote_identifiers = false;
        let sql = generate("id\n1", &o).unwrap();
        assert_eq!(sql, "INSERT INTO public.users (id) VALUES\n(1);\n");
    }

    // ---- error paths ----

    #[test]
    fn err_on_empty_input() {
        let err = generate("   ", &opts("t")).unwrap_err();
        assert!(err.contains("input is empty"), "got: {err}");
    }

    #[test]
    fn err_on_invalid_json() {
        let mut o = opts("t");
        o.format = Format::Json;
        let err = generate("{not json}", &o).unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
    }

    #[test]
    fn err_on_json_array_of_scalars() {
        let err = generate("[1, 2, 3]", &opts("t")).unwrap_err();
        assert!(err.contains("not an object"), "got: {err}");
    }

    #[test]
    fn err_on_bad_primary_key() {
        let mut o = opts("t");
        o.create_table = true;
        o.primary_key = "nope";
        let err = generate("a\n1", &o).unwrap_err();
        assert!(err.contains("not one of the columns"), "got: {err}");
    }

    #[test]
    fn err_on_unsafe_unquoted_identifier() {
        let mut o = opts("t");
        o.quote_identifiers = false;
        let err = generate("weird col\n1", &o).unwrap_err();
        assert!(err.contains("safe bare identifier"), "got: {err}");
    }

    #[test]
    fn err_on_bad_dialect() {
        let err = generate_from_str(
            "a\n1", "csv", "auto", true, "t", "oracle", "literal", true, false, false, "", true,
            "null", true, true,
        )
        .unwrap_err();
        assert!(err.contains("invalid dialect"), "got: {err}");
    }

    #[test]
    fn generate_from_str_happy() {
        let sql = generate_from_str(
            "a,b\n1,x", "auto", "auto", true, "t", "sqlite", "literal", true, false, false, "",
            true, "null", true, true,
        )
        .unwrap();
        assert_eq!(sql, "INSERT INTO \"t\" (\"a\", \"b\") VALUES\n(1, 'x');\n");
    }
}
