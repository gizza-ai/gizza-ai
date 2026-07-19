//! spreadsheet-to-sql core — read a spreadsheet (`.xlsx`/`.xlsm`/`.xls`/`.ods`)
//! and emit `CREATE TABLE` + `INSERT` statements, one table per worksheet. No
//! wafer/wasm-bindgen deps; pure logic shared by the chat skill block (and
//! host-testable). Reads via `calamine` (pure Rust, no C deps → compiles to
//! wasm32-unknown-unknown).

use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader};

/// Target SQL dialect. Governs identifier quoting, boolean/value literals, and
/// the column types chosen during type inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    MySql,
    Postgres,
    Sqlite,
    MsSql,
}

impl Dialect {
    /// Parse the `dialect` param. Accepts the canonical enum values plus a few
    /// common aliases; anything else is an error.
    pub fn parse(s: &str) -> Result<Dialect, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mysql" | "mariadb" => Ok(Dialect::MySql),
            "postgres" | "postgresql" | "psql" => Ok(Dialect::Postgres),
            "sqlite" | "sqlite3" => Ok(Dialect::Sqlite),
            "mssql" | "sqlserver" | "tsql" => Ok(Dialect::MsSql),
            other => Err(format!(
                "unknown dialect {other:?} (expected one of: mysql, postgres, sqlite, mssql)"
            )),
        }
    }

    /// Quote a SQL identifier (table/column name) for this dialect, escaping the
    /// closing delimiter so the name can't break out of the quotes.
    fn quote_ident(self, ident: &str) -> String {
        match self {
            Dialect::MySql => format!("`{}`", ident.replace('`', "``")),
            Dialect::Postgres | Dialect::Sqlite => format!("\"{}\"", ident.replace('"', "\"\"")),
            Dialect::MsSql => format!("[{}]", ident.replace(']', "]]")),
        }
    }

    /// A boolean literal for this dialect.
    fn bool_lit(self, b: bool) -> &'static str {
        match self {
            Dialect::Postgres => {
                if b {
                    "TRUE"
                } else {
                    "FALSE"
                }
            }
            // MySQL/SQLite/SQL Server have no dedicated boolean literal here.
            _ => {
                if b {
                    "1"
                } else {
                    "0"
                }
            }
        }
    }

    /// Quote a string value, escaping embedded quotes. MySQL also treats the
    /// backslash as an escape character by default, so it is doubled too.
    fn string_lit(self, s: &str) -> String {
        let escaped = match self {
            Dialect::MySql => s.replace('\\', "\\\\").replace('\'', "''"),
            _ => s.replace('\'', "''"),
        };
        format!("'{escaped}'")
    }

    /// The column type for an all-text column (or every column when type
    /// inference is off).
    fn text_type(self, max_len: usize) -> String {
        let n = max_len.clamp(1, 65535);
        match self {
            Dialect::MySql => format!("VARCHAR({n})"),
            Dialect::MsSql => format!("NVARCHAR({n})"),
            Dialect::Postgres | Dialect::Sqlite => "TEXT".to_string(),
        }
    }

    fn int_type(self) -> &'static str {
        match self {
            Dialect::MySql | Dialect::MsSql => "INT",
            Dialect::Postgres | Dialect::Sqlite => "INTEGER",
        }
    }

    fn float_type(self) -> &'static str {
        match self {
            Dialect::MySql => "DOUBLE",
            Dialect::Postgres => "DOUBLE PRECISION",
            Dialect::Sqlite => "REAL",
            Dialect::MsSql => "FLOAT",
        }
    }

    fn bool_type(self) -> &'static str {
        match self {
            Dialect::MySql => "TINYINT(1)",
            Dialect::Postgres => "BOOLEAN",
            Dialect::Sqlite => "INTEGER",
            Dialect::MsSql => "BIT",
        }
    }
}

/// Conversion options (mirrors the block's descriptor params).
#[derive(Debug, Clone)]
pub struct Options {
    pub dialect: Dialect,
    /// Worksheet selector: a sheet name, a 0-based index as a string, or
    /// `None`/empty for every sheet (one table each).
    pub sheet: Option<String>,
    /// Base table-name override. When emitting more than one sheet this is used
    /// as a prefix (`{table}_{sheet}`); for a single sheet it is used verbatim.
    pub table: Option<String>,
    /// Emit `CREATE TABLE` before the inserts (else inserts only).
    pub create_table: bool,
    /// Treat the first row of each sheet as column names (else `col1..colN` and
    /// the first row becomes data).
    pub header_row: bool,
    /// Infer column SQL types from the data (else every column is text).
    pub infer_types: bool,
    /// Emit one multi-row `INSERT ... VALUES (...),(...)` per sheet (else one
    /// `INSERT` statement per row).
    pub batch_insert: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            dialect: Dialect::MySql,
            sheet: None,
            table: None,
            create_table: true,
            header_row: true,
            infer_types: true,
            batch_insert: true,
        }
    }
}

/// The inferred kind of a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColKind {
    Int,
    Float,
    Bool,
    Text,
}

/// Convert the in-memory spreadsheet `bytes` to SQL text per `opts`.
///
/// Returns `Err` on unreadable/empty/corrupt bytes or an unknown sheet.
pub fn to_sql(bytes: &[u8], opts: &Options) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("empty spreadsheet bytes".to_string());
    }

    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("not a readable spreadsheet: {e}"))?;

    let names = workbook.sheet_names();
    if names.is_empty() {
        return Err("spreadsheet has no worksheets".to_string());
    }

    let targets = resolve_targets(&names, opts.sheet.as_deref())?;
    let multi = targets.len() > 1;

    let mut out = String::new();
    for (i, sheet_name) in targets.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let range = workbook
            .worksheet_range(sheet_name)
            .map_err(|e| format!("failed to read sheet {sheet_name:?}: {e}"))?;
        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|r| r.iter().map(cell_to_string).collect())
            .collect();

        let table = table_name(opts.table.as_deref(), sheet_name, multi);
        emit_sheet(&mut out, &table, &rows, opts);
    }
    Ok(out)
}

/// Resolve which sheet(s) to emit. `None`/empty → all sheets in order; a name
/// wins over an index; a non-negative integer selects by 0-based index.
fn resolve_targets(names: &[String], sheet: Option<&str>) -> Result<Vec<String>, String> {
    let Some(sel) = sheet.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(names.to_vec());
    };
    if let Some(found) = names.iter().find(|n| n.as_str() == sel) {
        return Ok(vec![found.clone()]);
    }
    if let Ok(idx) = sel.parse::<usize>() {
        return names
            .get(idx)
            .cloned()
            .map(|n| vec![n])
            .ok_or_else(|| {
                format!(
                    "sheet index {idx} out of range (workbook has {} sheet(s): {names:?})",
                    names.len()
                )
            });
    }
    Err(format!("no sheet named {sel:?} (available: {names:?})"))
}

/// Pick a table name for a sheet: the override verbatim for a single sheet,
/// `{override}_{sheet}` when several sheets are emitted, else the sheet name.
/// The result is always sanitized to a safe identifier.
fn table_name(override_: Option<&str>, sheet: &str, multi: bool) -> String {
    let base = match override_.map(str::trim).filter(|s| !s.is_empty()) {
        Some(o) if multi => format!("{o}_{sheet}"),
        Some(o) => o.to_string(),
        None => sheet.to_string(),
    };
    sanitize_ident(&base, "table")
}

/// Turn arbitrary text into a safe SQL identifier: keep ASCII alphanumerics and
/// underscores, map every other run to a single `_`, prefix `t_` if it starts
/// with a digit, and fall back to `fallback` if nothing usable remains.
fn sanitize_ident(raw: &str, fallback: &str) -> String {
    let mut s = String::new();
    let mut prev_us = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            s.push(ch);
            prev_us = false;
        } else if !prev_us {
            s.push('_');
            prev_us = true;
        }
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        return fallback.to_string();
    }
    if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("t_{s}")
    } else {
        s
    }
}

/// Emit CREATE TABLE + INSERTs for one sheet into `out`.
fn emit_sheet(out: &mut String, table: &str, rows: &[Vec<String>], opts: &Options) {
    let dialect = opts.dialect;
    let qtable = dialect.quote_ident(table);

    // Column count = widest row. Empty sheet → a comment, nothing else.
    let ncols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if ncols == 0 {
        out.push_str(&format!("-- sheet {table:?} is empty; no statements emitted\n"));
        return;
    }

    // Split header vs data rows.
    let (headers, data): (Vec<String>, &[Vec<String>]) = if opts.header_row && !rows.is_empty() {
        (rows[0].clone(), &rows[1..])
    } else {
        (Vec::new(), rows)
    };
    let cols = column_names(&headers, ncols);
    let qcols: Vec<String> = cols.iter().map(|c| dialect.quote_ident(c)).collect();

    if opts.create_table {
        let kinds: Vec<ColKind> = (0..ncols)
            .map(|c| {
                if opts.infer_types {
                    infer_column(data, c)
                } else {
                    ColKind::Text
                }
            })
            .collect();
        out.push_str(&format!("CREATE TABLE {qtable} (\n"));
        for c in 0..ncols {
            let ty = match kinds[c] {
                ColKind::Int => dialect.int_type().to_string(),
                ColKind::Float => dialect.float_type().to_string(),
                ColKind::Bool => dialect.bool_type().to_string(),
                ColKind::Text => dialect.text_type(max_col_len(data, c)),
            };
            let comma = if c + 1 < ncols { "," } else { "" };
            out.push_str(&format!("  {} {}{}\n", qcols[c], ty, comma));
        }
        out.push_str(");\n");
    }

    if data.is_empty() {
        return;
    }

    let col_list = qcols.join(", ");
    let render_row = |row: &[String]| -> String {
        let vals: Vec<String> = (0..ncols)
            .map(|c| value_lit(row.get(c).map(String::as_str).unwrap_or(""), dialect))
            .collect();
        format!("({})", vals.join(", "))
    };

    if opts.batch_insert {
        out.push_str(&format!("INSERT INTO {qtable} ({col_list}) VALUES\n"));
        for (i, row) in data.iter().enumerate() {
            let sep = if i + 1 < data.len() { "," } else { ";" };
            out.push_str(&format!("  {}{}\n", render_row(row), sep));
        }
    } else {
        for row in data {
            out.push_str(&format!(
                "INSERT INTO {qtable} ({col_list}) VALUES {};\n",
                render_row(row)
            ));
        }
    }
}

/// Column names from a header row (deduped, sanitized), or generated
/// `col1..colN` when there is no header. Empty header cells become `colN`.
fn column_names(headers: &[String], ncols: usize) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(ncols);
    for c in 0..ncols {
        let raw = headers.get(c).map(String::as_str).unwrap_or("").trim();
        let base = if raw.is_empty() {
            format!("col{}", c + 1)
        } else {
            sanitize_ident(raw, &format!("col{}", c + 1))
        };
        // Case-insensitive dedupe so `Name`/`name` don't collide in engines that
        // fold identifier case.
        let key = base.to_ascii_lowercase();
        let name = match seen.get_mut(&key) {
            Some(n) => {
                *n += 1;
                format!("{base}_{n}")
            }
            None => {
                seen.insert(key, 1);
                base
            }
        };
        out.push(name);
    }
    out
}

/// Infer a column's kind from its data cells. Empty cells are skipped (they map
/// to NULL and don't constrain the type). An all-empty column is text.
fn infer_column(data: &[Vec<String>], col: usize) -> ColKind {
    let mut saw = false;
    let mut all_int = true;
    let mut all_num = true;
    let mut all_bool = true;
    for row in data {
        let cell = row.get(col).map(String::as_str).unwrap_or("").trim();
        if cell.is_empty() {
            continue;
        }
        saw = true;
        if !is_bool_lit(cell) {
            all_bool = false;
        }
        if cell.parse::<i64>().is_err() {
            all_int = false;
        }
        if !is_number(cell) {
            all_num = false;
        }
    }
    if !saw {
        return ColKind::Text;
    }
    if all_bool {
        ColKind::Bool
    } else if all_int {
        ColKind::Int
    } else if all_num {
        ColKind::Float
    } else {
        ColKind::Text
    }
}

fn is_bool_lit(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false"
    )
}

/// A finite decimal number (no leading `+`, no infinities/NaN, no hex).
fn is_number(s: &str) -> bool {
    match s.parse::<f64>() {
        Ok(f) => f.is_finite() && !s.eq_ignore_ascii_case("inf") && !s.eq_ignore_ascii_case("nan"),
        Err(_) => false,
    }
}

/// Longest cell string in a column (for VARCHAR sizing).
fn max_col_len(data: &[Vec<String>], col: usize) -> usize {
    data.iter()
        .filter_map(|r| r.get(col))
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0)
}

/// Render one cell as a SQL value literal: empty → NULL, numbers/booleans as
/// bare literals, everything else as a quoted string.
fn value_lit(cell: &str, dialect: Dialect) -> String {
    let t = cell.trim();
    if t.is_empty() {
        return "NULL".to_string();
    }
    if is_bool_lit(t) {
        return dialect.bool_lit(t.eq_ignore_ascii_case("true")).to_string();
    }
    if is_number(t) {
        return t.to_string();
    }
    dialect.string_lit(cell)
}

/// Render one calamine cell as a plain string. Whole floats print without a
/// trailing `.0`; other floats use default formatting.
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => format_float(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => {
            if *b {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Data::DateTime(dt) => format_float(dt.as_f64()),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{e:?}"),
    }
}

/// Format an `f64` so integral values drop the `.0` (Excel-style display).
fn format_float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_xlsxwriter::Workbook;

    /// A tiny workbook: sheet "People" with a header + typed data (int, text,
    /// bool, a string needing quote-escaping, and an empty cell); plus a second
    /// sheet to exercise multi-sheet emission and sheet selection.
    fn sample_xlsx() -> Vec<u8> {
        let mut wb = Workbook::new();
        let s1 = wb.add_worksheet().set_name("People").unwrap();
        for (c, h) in ["id", "name", "active", "note"].iter().enumerate() {
            s1.write_string(0, c as u16, *h).unwrap();
        }
        // row 1
        s1.write_number(1, 0, 1.0).unwrap();
        s1.write_string(1, 1, "O'Brien").unwrap(); // needs '' escaping
        s1.write_boolean(1, 2, true).unwrap();
        s1.write_string(1, 3, "hi").unwrap();
        // row 2 (note cell left empty → NULL)
        s1.write_number(2, 0, 2.0).unwrap();
        s1.write_string(2, 1, "Alice").unwrap();
        s1.write_boolean(2, 2, false).unwrap();

        let s2 = wb.add_worksheet().set_name("Cities").unwrap();
        s2.write_string(0, 0, "city").unwrap();
        s2.write_string(1, 0, "Paris").unwrap();

        wb.save_to_buffer().unwrap()
    }

    #[test]
    fn happy_mysql_all_sheets_infer_and_batch() {
        let bytes = sample_xlsx();
        let sql = to_sql(&bytes, &Options::default()).unwrap();
        // First table, MySQL backtick quoting + inferred types.
        assert!(sql.contains("CREATE TABLE `People` ("), "got:\n{sql}");
        assert!(sql.contains("`id` INT"), "got:\n{sql}");
        assert!(sql.contains("`active` TINYINT(1)"), "got:\n{sql}");
        assert!(sql.contains("`name` VARCHAR("), "got:\n{sql}");
        // Batch insert, quote-escaped string, bool→1/0, empty→NULL.
        assert!(sql.contains("INSERT INTO `People` (`id`, `name`, `active`, `note`) VALUES"));
        assert!(sql.contains("(1, 'O''Brien', 1, 'hi'),"), "got:\n{sql}");
        assert!(sql.contains("(2, 'Alice', 0, NULL);"), "got:\n{sql}");
        // Second sheet also emitted.
        assert!(sql.contains("CREATE TABLE `Cities` ("), "got:\n{sql}");
        assert!(sql.contains("('Paris');"), "got:\n{sql}");
    }

    #[test]
    fn postgres_bool_and_identifier_quoting() {
        let bytes = sample_xlsx();
        let opts = Options {
            dialect: Dialect::Postgres,
            sheet: Some("People".to_string()),
            ..Default::default()
        };
        let sql = to_sql(&bytes, &opts).unwrap();
        assert!(sql.contains("CREATE TABLE \"People\" ("), "got:\n{sql}");
        assert!(sql.contains("\"active\" BOOLEAN"), "got:\n{sql}");
        assert!(sql.contains("(1, 'O''Brien', TRUE, 'hi'),"), "got:\n{sql}");
        // only the selected sheet
        assert!(!sql.contains("Cities"), "got:\n{sql}");
    }

    #[test]
    fn inserts_only_and_per_row_and_no_header() {
        let bytes = sample_xlsx();
        let opts = Options {
            sheet: Some("0".to_string()),
            table: Some("staff".to_string()),
            create_table: false,
            header_row: false,
            batch_insert: false,
            ..Default::default()
        };
        let sql = to_sql(&bytes, &opts).unwrap();
        // No CREATE TABLE, generated column names, table override, one row per stmt.
        assert!(!sql.contains("CREATE TABLE"), "got:\n{sql}");
        assert!(sql.contains("INSERT INTO `staff` (`col1`, `col2`, `col3`, `col4`) VALUES ('id', 'name', 'active', 'note');"), "got:\n{sql}");
        // header row is now data, so 3 INSERTs total.
        assert_eq!(sql.matches("INSERT INTO").count(), 3, "got:\n{sql}");
    }

    #[test]
    fn mssql_bracket_quoting_and_no_infer() {
        let bytes = sample_xlsx();
        let opts = Options {
            dialect: Dialect::MsSql,
            sheet: Some("People".to_string()),
            infer_types: false,
            ..Default::default()
        };
        let sql = to_sql(&bytes, &opts).unwrap();
        assert!(sql.contains("CREATE TABLE [People] ("), "got:\n{sql}");
        // no inference → every column is text (NVARCHAR for mssql)
        assert!(sql.contains("[id] NVARCHAR("), "got:\n{sql}");
        assert!(sql.contains("[active] NVARCHAR("), "got:\n{sql}");
        // mssql bool literal is 1/0
        assert!(sql.contains("(1, 'O''Brien', 1, 'hi'),"), "got:\n{sql}");
    }

    #[test]
    fn duplicate_and_dirty_headers_are_sanitized_and_deduped() {
        let mut wb = Workbook::new();
        let s = wb.add_worksheet().set_name("S").unwrap();
        s.write_string(0, 0, "First Name").unwrap();
        s.write_string(0, 1, "First Name").unwrap(); // duplicate
        s.write_string(0, 2, "123col").unwrap(); // leading digit
        s.write_string(1, 0, "a").unwrap();
        s.write_string(1, 1, "b").unwrap();
        s.write_string(1, 2, "c").unwrap();
        let bytes = wb.save_to_buffer().unwrap();
        let sql = to_sql(&bytes, &Options::default()).unwrap();
        assert!(sql.contains("`First_Name`"), "got:\n{sql}");
        assert!(sql.contains("`First_Name_2`"), "got:\n{sql}");
        assert!(sql.contains("`t_123col`"), "got:\n{sql}");
    }

    #[test]
    fn empty_bytes_error() {
        let err = to_sql(&[], &Options::default()).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn garbage_bytes_error() {
        let err = to_sql(b"this is not a spreadsheet", &Options::default()).unwrap_err();
        assert!(err.contains("not a readable spreadsheet"), "got: {err}");
    }

    #[test]
    fn unknown_sheet_error() {
        let bytes = sample_xlsx();
        let opts = Options {
            sheet: Some("Nope".to_string()),
            ..Default::default()
        };
        let err = to_sql(&bytes, &opts).unwrap_err();
        assert!(err.contains("no sheet named"), "got: {err}");
    }

    #[test]
    fn sheet_index_out_of_range_error() {
        let bytes = sample_xlsx();
        let opts = Options {
            sheet: Some("9".to_string()),
            ..Default::default()
        };
        let err = to_sql(&bytes, &opts).unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn dialect_parse_accepts_aliases_and_rejects_unknown() {
        assert_eq!(Dialect::parse("PostgreSQL").unwrap(), Dialect::Postgres);
        assert_eq!(Dialect::parse(" mariadb ").unwrap(), Dialect::MySql);
        assert!(Dialect::parse("oracle").is_err());
    }

    #[test]
    fn float_column_inferred() {
        let mut wb = Workbook::new();
        let s = wb.add_worksheet().set_name("N").unwrap();
        s.write_string(0, 0, "amount").unwrap();
        s.write_number(1, 0, 1.5).unwrap();
        s.write_number(2, 0, 3.0).unwrap();
        let bytes = wb.save_to_buffer().unwrap();
        let sql = to_sql(&bytes, &Options::default()).unwrap();
        assert!(sql.contains("`amount` DOUBLE"), "got:\n{sql}");
        assert!(sql.contains("(1.5),"), "got:\n{sql}");
    }
}
