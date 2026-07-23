//! json-to-sql-insert core — turn a JSON object or an array of row objects into
//! SQL `INSERT` statements (optionally with a `CREATE TABLE`). Pure compute, no
//! wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! The generator is dialect-aware (identifier quoting, boolean and string
//! literals, placeholder syntax), supports literal or parameterized (`?`/`$n`/
//! `@pn`) value output, multi-row or per-row inserts, three NULL policies, and
//! deterministic column ordering (first-seen or alphabetical). `serde_json`'s
//! `preserve_order` feature keeps object key order stable, so first-seen column
//! ordering is deterministic for a given input.

use serde_json::{Map, Value};

/// Table name used when the caller passes a blank `table`.
pub const DEFAULT_TABLE: &str = "my_table";

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

    /// Render a JSON boolean as this dialect's literal. Postgres and ANSI SQL
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
        match (self, t) {
            (_, ColType::Integer) => match self {
                Dialect::Mysql | Dialect::Mssql => "INT",
                _ => "INTEGER",
            },
            (_, ColType::Float) => match self {
                Dialect::Mysql => "DOUBLE",
                Dialect::Postgres | Dialect::Ansi => "DOUBLE PRECISION",
                Dialect::Sqlite => "REAL",
                Dialect::Mssql => "FLOAT",
            },
            (_, ColType::Boolean) => match self {
                Dialect::Postgres | Dialect::Ansi => "BOOLEAN",
                Dialect::Mysql => "TINYINT(1)",
                Dialect::Sqlite => "INTEGER",
                Dialect::Mssql => "BIT",
            },
            (_, ColType::Text) => match self {
                Dialect::Mysql | Dialect::Postgres | Dialect::Sqlite => "TEXT",
                Dialect::Mssql => "NVARCHAR(255)",
                Dialect::Ansi => "VARCHAR(255)",
            },
        }
    }
}

/// How JSON `null` — and a column missing from a given row — is rendered.
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

/// Inferred SQL column category (for `CREATE TABLE` type mapping).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ColType {
    Integer,
    Float,
    Boolean,
    Text,
}

/// Everything the generator needs, already parsed from the raw string params.
pub struct Options<'a> {
    pub table: &'a str,
    pub dialect: Dialect,
    pub value_mode: ValueMode,
    pub multi_row: bool,
    pub create_table: bool,
    pub drop_table: bool,
    pub primary_key: &'a str,
    pub quote_identifiers: bool,
    pub null_handling: NullHandling,
    pub sort_columns: bool,
}

/// Parse the string-typed params (the shape every surface passes) into an
/// [`Options`], then generate. Booleans accept `true`/`1`/`yes`/`on`.
#[allow(clippy::too_many_arguments)]
pub fn generate_from_str(
    json: &str,
    table: &str,
    dialect: &str,
    values: &str,
    multi_row: bool,
    create_table: bool,
    drop_table: bool,
    primary_key: &str,
    quote_identifiers: bool,
    null_handling: &str,
    sort_columns: bool,
) -> Result<String, String> {
    let opts = Options {
        table,
        dialect: Dialect::parse(dialect)?,
        value_mode: ValueMode::parse(values)?,
        multi_row,
        create_table,
        drop_table,
        primary_key,
        quote_identifiers,
        null_handling: NullHandling::parse(null_handling)?,
        sort_columns,
    };
    generate(json, &opts)
}

/// Generate `CREATE TABLE`/`INSERT` SQL for `json` under `opts`.
pub fn generate(json: &str, opts: &Options) -> Result<String, String> {
    let rows = parse_rows(json)?;
    let columns = collect_columns(&rows, opts.sort_columns);
    if columns.is_empty() {
        return Err("no columns found: every row object is empty".to_string());
    }

    let table = if opts.table.trim().is_empty() {
        DEFAULT_TABLE
    } else {
        opts.table.trim()
    };
    let table_sql = quote_qualified(table, opts.dialect, opts.quote_identifiers)?;

    // Validate the primary key names a real column before we build anything.
    let pk = opts.primary_key.trim();
    if !pk.is_empty() && !columns.iter().any(|c| c == pk) {
        return Err(format!(
            "primary_key {pk:?} is not one of the columns ({})",
            columns.join(", ")
        ));
    }

    let col_sql: Vec<String> = columns
        .iter()
        .map(|c| quote_ident(c, opts.dialect, opts.quote_identifiers))
        .collect::<Result<Vec<_>, _>>()?;

    let mut out = String::new();

    if opts.drop_table {
        out.push_str(&format!("DROP TABLE IF EXISTS {table_sql};\n"));
    }
    if opts.create_table {
        out.push_str(&create_table_sql(&table_sql, &columns, &col_sql, &rows, pk, opts));
        out.push('\n');
    }

    out.push_str(&insert_sql(&table_sql, &columns, &col_sql, &rows, opts));

    Ok(out)
}

/// Parse the input into a list of row objects. A bare object is one row; an
/// array must contain only objects.
fn parse_rows(json: &str) -> Result<Vec<Map<String, Value>>, String> {
    if json.trim().is_empty() {
        return Err("input is empty: paste a JSON object or an array of objects".to_string());
    }
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    match value {
        Value::Object(map) => Ok(vec![map]),
        Value::Array(items) => {
            if items.is_empty() {
                return Err("input is an empty array: nothing to insert".to_string());
            }
            let mut rows = Vec::with_capacity(items.len());
            for (i, item) in items.into_iter().enumerate() {
                match item {
                    Value::Object(map) => rows.push(map),
                    other => {
                        return Err(format!(
                            "array item {i} is a {}, not an object — each row must be a JSON object",
                            type_name(&other)
                        ))
                    }
                }
            }
            Ok(rows)
        }
        other => Err(format!(
            "expected a JSON object or an array of objects, got a {}",
            type_name(&other)
        )),
    }
}

/// Union of every row's keys. Default order is first-seen (deterministic thanks
/// to `preserve_order`); `sort` switches to case-sensitive alphabetical.
fn collect_columns(rows: &[Map<String, Value>], sort: bool) -> Vec<String> {
    let mut cols: Vec<String> = Vec::new();
    for row in rows {
        for key in row.keys() {
            if !cols.iter().any(|c| c == key) {
                cols.push(key.clone());
            }
        }
    }
    if sort {
        cols.sort();
    }
    cols
}

fn insert_sql(
    table_sql: &str,
    columns: &[String],
    col_sql: &[String],
    rows: &[Map<String, Value>],
    opts: &Options,
) -> String {
    let cols_joined = col_sql.join(", ");
    let mut out = String::new();

    if opts.multi_row {
        // One INSERT with every row as its own (...) tuple.
        out.push_str(&format!("INSERT INTO {table_sql} ({cols_joined}) VALUES\n"));
        let mut params: Vec<String> = Vec::new();
        let mut ph = 0usize; // running placeholder counter across the statement
        let tuples: Vec<String> = rows
            .iter()
            .map(|row| row_tuple(row, columns, opts, &mut ph, &mut params))
            .collect();
        out.push_str(&tuples.join(",\n"));
        out.push(';');
        append_params(&mut out, opts, &params);
        out.push('\n');
    } else {
        // A separate INSERT per row.
        for row in rows {
            let mut params: Vec<String> = Vec::new();
            let mut ph = 0usize;
            let tuple = row_tuple(row, columns, opts, &mut ph, &mut params);
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
    row: &Map<String, Value>,
    columns: &[String],
    opts: &Options,
    ph: &mut usize,
    params: &mut Vec<String>,
) -> String {
    let cells: Vec<String> = columns
        .iter()
        .map(|col| {
            let value = row.get(col);
            match opts.value_mode {
                ValueMode::Literal => literal(value, opts),
                ValueMode::Placeholder => {
                    *ph += 1;
                    params.push(literal(value, opts));
                    opts.dialect.placeholder(*ph)
                }
            }
        })
        .collect();
    format!("({})", cells.join(", "))
}

/// The SQL literal for a single cell. `None` (key missing from this row) is
/// treated identically to an explicit JSON `null`.
fn literal(value: Option<&Value>, opts: &Options) -> String {
    match value {
        None | Some(Value::Null) => match opts.null_handling {
            NullHandling::Null => "NULL".to_string(),
            NullHandling::Default => "DEFAULT".to_string(),
            NullHandling::EmptyString => "''".to_string(),
        },
        Some(Value::Bool(b)) => opts.dialect.bool_literal(*b).to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) => opts.dialect.string_literal(s),
        // Nested arrays/objects are serialized to compact JSON and stored as a
        // string literal so nothing is silently dropped.
        Some(other @ (Value::Array(_) | Value::Object(_))) => {
            opts.dialect.string_literal(&other.to_string())
        }
    }
}

/// Append the `-- params: ...` comment for placeholder mode.
fn append_params(out: &mut String, opts: &Options, params: &[String]) {
    if opts.value_mode == ValueMode::Placeholder && !params.is_empty() {
        out.push_str(&format!("  -- params: {}", params.join(", ")));
    }
}

fn create_table_sql(
    table_sql: &str,
    columns: &[String],
    col_sql: &[String],
    rows: &[Map<String, Value>],
    pk: &str,
    opts: &Options,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for (col, col_q) in columns.iter().zip(col_sql) {
        let ty = opts.dialect.column_type(infer_type(rows, col));
        lines.push(format!("  {col_q} {ty}"));
    }
    if !pk.is_empty() {
        let pk_q = quote_ident(pk, opts.dialect, opts.quote_identifiers)
            .unwrap_or_else(|_| pk.to_string());
        lines.push(format!("  PRIMARY KEY ({pk_q})"));
    }
    format!("CREATE TABLE {table_sql} (\n{}\n);", lines.join(",\n"))
}

/// Infer a column's SQL type from every non-null value seen for it.
fn infer_type(rows: &[Map<String, Value>], col: &str) -> ColType {
    let mut saw_value = false;
    let mut all_int = true;
    let mut all_number = true;
    let mut all_bool = true;
    for row in rows {
        match row.get(col) {
            None | Some(Value::Null) => {}
            Some(v) => {
                saw_value = true;
                match v {
                    Value::Number(n) => {
                        all_bool = false;
                        if !n.is_i64() && !n.is_u64() {
                            all_int = false;
                        }
                    }
                    Value::Bool(_) => {
                        all_int = false;
                        all_number = false;
                    }
                    _ => {
                        all_int = false;
                        all_number = false;
                        all_bool = false;
                    }
                }
            }
        }
    }
    if !saw_value {
        return ColType::Text;
    }
    if all_bool {
        ColType::Boolean
    } else if all_int {
        ColType::Integer
    } else if all_number {
        ColType::Float
    } else {
        ColType::Text
    }
}

/// Quote a possibly schema-qualified table name (`schema.table`), quoting each
/// dot-separated part. When `quote` is false, every part is validated as a bare
/// SQL identifier instead.
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

/// Quote (or, with `quote=false`, validate) a single identifier.
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

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(table: &'a str) -> Options<'a> {
        Options {
            table,
            dialect: Dialect::Mysql,
            value_mode: ValueMode::Literal,
            multi_row: true,
            create_table: false,
            drop_table: false,
            primary_key: "",
            quote_identifiers: true,
            null_handling: NullHandling::Null,
            sort_columns: false,
        }
    }

    #[test]
    fn happy_single_object_multi_row_mysql() {
        let sql = generate(r#"{"id": 1, "name": "Alice"}"#, &opts("users")).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `users` (`id`, `name`) VALUES\n(1, 'Alice');\n"
        );
    }

    #[test]
    fn array_of_rows_batches_and_fills_missing_with_null() {
        // Second row omits `email`; column order is first-seen union.
        let sql = generate(
            r#"[{"id":1,"email":"a@x.com"},{"id":2}]"#,
            &opts("t"),
        )
        .unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `t` (`id`, `email`) VALUES\n(1, 'a@x.com'),\n(2, NULL);\n"
        );
    }

    #[test]
    fn per_row_inserts_when_multi_row_off() {
        let mut o = opts("t");
        o.multi_row = false;
        let sql = generate(r#"[{"a":1},{"a":2}]"#, &o).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `t` (`a`) VALUES (1);\nINSERT INTO `t` (`a`) VALUES (2);\n"
        );
    }

    #[test]
    fn placeholder_mode_postgres_numbers_and_lists_params() {
        let mut o = opts("t");
        o.dialect = Dialect::Postgres;
        o.value_mode = ValueMode::Placeholder;
        let sql = generate(r#"{"a": 1, "b": "x"}"#, &o).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO \"t\" (\"a\", \"b\") VALUES\n($1, $2);  -- params: 1, 'x'\n"
        );
    }

    #[test]
    fn dialect_quoting_and_bools() {
        // Postgres: double quotes + TRUE/FALSE.
        let mut o = opts("t");
        o.dialect = Dialect::Postgres;
        let sql = generate(r#"{"ok": true}"#, &o).unwrap();
        assert_eq!(sql, "INSERT INTO \"t\" (\"ok\") VALUES\n(TRUE);\n");
        // MySQL: backticks + 1/0.
        let sql = generate(r#"{"ok": false}"#, &opts("t")).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`ok`) VALUES\n(0);\n");
        // SQL Server: brackets + 1/0.
        let mut o = opts("t");
        o.dialect = Dialect::Mssql;
        let sql = generate(r#"{"ok": true}"#, &o).unwrap();
        assert_eq!(sql, "INSERT INTO [t] ([ok]) VALUES\n(1);\n");
    }

    #[test]
    fn mysql_escapes_backslash_and_quote() {
        let sql = generate(r#"{"p": "a'b\\c"}"#, &opts("t")).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`p`) VALUES\n('a''b\\\\c');\n");
        // Postgres does NOT double the backslash (only the quote).
        let mut o = opts("t");
        o.dialect = Dialect::Postgres;
        let sql = generate(r#"{"p": "a'b\\c"}"#, &o).unwrap();
        assert_eq!(sql, "INSERT INTO \"t\" (\"p\") VALUES\n('a''b\\c');\n");
    }

    #[test]
    fn null_handling_variants() {
        let mut o = opts("t");
        o.null_handling = NullHandling::Default;
        let sql = generate(r#"{"a": null}"#, &o).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`a`) VALUES\n(DEFAULT);\n");
        o.null_handling = NullHandling::EmptyString;
        let sql = generate(r#"{"a": null}"#, &o).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`a`) VALUES\n('');\n");
    }

    #[test]
    fn sort_columns_alphabetical() {
        let mut o = opts("t");
        o.sort_columns = true;
        let sql = generate(r#"{"b": 2, "a": 1}"#, &o).unwrap();
        assert_eq!(sql, "INSERT INTO `t` (`a`, `b`) VALUES\n(1, 2);\n");
    }

    #[test]
    fn nested_value_serialized_as_json_string() {
        let sql = generate(r#"{"meta": {"k": [1,2]}}"#, &opts("t")).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO `t` (`meta`) VALUES\n('{\"k\":[1,2]}');\n"
        );
    }

    #[test]
    fn create_table_infers_types_and_pk() {
        let mut o = opts("users");
        o.create_table = true;
        o.drop_table = true;
        o.primary_key = "id";
        let sql = generate(
            r#"[{"id":1,"name":"Al","score":1.5,"active":true},{"id":2,"name":"Bo","score":3.0,"active":false}]"#,
            &o,
        )
        .unwrap();
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
    fn unquoted_identifiers_when_safe() {
        let mut o = opts("public.users");
        o.quote_identifiers = false;
        let sql = generate(r#"{"id": 1}"#, &o).unwrap();
        assert_eq!(sql, "INSERT INTO public.users (id) VALUES\n(1);\n");
    }

    #[test]
    fn blank_table_uses_default() {
        let sql = generate(r#"{"a": 1}"#, &opts("")).unwrap();
        assert!(sql.starts_with("INSERT INTO `my_table`"), "got: {sql}");
    }

    // ---- error paths ----

    #[test]
    fn err_on_invalid_json() {
        let err = generate("{not json}", &opts("t")).unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
    }

    #[test]
    fn err_on_array_of_scalars() {
        let err = generate("[1, 2, 3]", &opts("t")).unwrap_err();
        assert!(err.contains("not an object"), "got: {err}");
    }

    #[test]
    fn err_on_empty_array() {
        let err = generate("[]", &opts("t")).unwrap_err();
        assert!(err.contains("empty array"), "got: {err}");
    }

    #[test]
    fn err_on_top_level_scalar() {
        let err = generate("42", &opts("t")).unwrap_err();
        assert!(err.contains("expected a JSON object"), "got: {err}");
    }

    #[test]
    fn err_on_unsafe_unquoted_identifier() {
        let mut o = opts("t");
        o.quote_identifiers = false;
        let err = generate(r#"{"weird col": 1}"#, &o).unwrap_err();
        assert!(err.contains("safe bare identifier"), "got: {err}");
    }

    #[test]
    fn err_on_bad_primary_key() {
        let mut o = opts("t");
        o.create_table = true;
        o.primary_key = "nope";
        let err = generate(r#"{"a": 1}"#, &o).unwrap_err();
        assert!(err.contains("not one of the columns"), "got: {err}");
    }

    #[test]
    fn err_on_bad_dialect() {
        let err = generate_from_str(
            r#"{"a":1}"#, "t", "oracle", "literal", true, false, false, "", true, "null", false,
        )
        .unwrap_err();
        assert!(err.contains("invalid dialect"), "got: {err}");
    }

    #[test]
    fn generate_from_str_happy() {
        let sql = generate_from_str(
            r#"{"a":1}"#, "t", "sqlite", "literal", true, false, false, "", true, "null", false,
        )
        .unwrap();
        assert_eq!(sql, "INSERT INTO \"t\" (\"a\") VALUES\n(1);\n");
    }
}
