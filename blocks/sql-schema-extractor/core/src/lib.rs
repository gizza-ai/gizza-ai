//! sql-schema-extractor core — parse the DDL out of a SQL dump (`CREATE TABLE`,
//! `ALTER TABLE`, `CREATE INDEX`) into a clean table / column / constraint model
//! and render it as JSON or Markdown. Pure compute, no wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.
//!
//! This is a *lenient DDL parser*, not a full SQL engine: DML
//! (`INSERT`/`UPDATE`/…), comments (`--`, `#`, `/* */`) and statements it does
//! not understand are skipped rather than erroring, so it survives a real dump.
//! It never executes SQL and never touches INSERT rows (that is
//! `blocks/sql-dump-to-csv`'s job).

use serde_json::{json, Map, Value};

/// Table / entity name used when a statement omits one (should be rare).
const DEFAULT_TABLE: &str = "table";

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Column {
    name: String,
    /// Data type as written, normalized to upper-case keyword + `(args)` — e.g.
    /// `VARCHAR(255)`, `DECIMAL(10,2)`, `TIMESTAMP WITH TIME ZONE`.
    data_type: String,
    /// `None` = unspecified (defaults to nullable in SQL), `Some(false)` =
    /// `NOT NULL`, `Some(true)` = explicit `NULL`.
    nullable: Option<bool>,
    default: Option<String>,
    primary_key: bool,
    unique: bool,
    auto_increment: bool,
    comment: Option<String>,
}

#[derive(Clone, Debug)]
struct ForeignKey {
    name: Option<String>,
    columns: Vec<String>,
    ref_table: String,
    ref_columns: Vec<String>,
    on_delete: Option<String>,
    on_update: Option<String>,
}

#[derive(Clone, Debug)]
struct Unique {
    name: Option<String>,
    columns: Vec<String>,
}

#[derive(Clone, Debug)]
struct Check {
    name: Option<String>,
    expression: String,
}

#[derive(Clone, Debug)]
struct Index {
    name: Option<String>,
    columns: Vec<String>,
    unique: bool,
}

#[derive(Clone, Debug, Default)]
struct Table {
    name: String,
    columns: Vec<Column>,
    primary_key: Vec<String>,
    foreign_keys: Vec<ForeignKey>,
    uniques: Vec<Unique>,
    checks: Vec<Check>,
    indexes: Vec<Index>,
}

impl Table {
    fn column_mut(&mut self, name: &str) -> Option<&mut Column> {
        self.columns
            .iter_mut()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

/// A parsed schema: tables in first-seen order.
#[derive(Default)]
struct Schema {
    tables: Vec<Table>,
}

impl Schema {
    /// Find a table by (case-insensitive) name, creating a stub if it is
    /// referenced by an `ALTER`/`CREATE INDEX` before its `CREATE TABLE`.
    fn table_mut(&mut self, name: &str) -> &mut Table {
        if let Some(idx) = self
            .tables
            .iter()
            .position(|t| t.name.eq_ignore_ascii_case(name))
        {
            return &mut self.tables[idx];
        }
        self.tables.push(Table {
            name: name.to_string(),
            ..Default::default()
        });
        self.tables.last_mut().unwrap()
    }
}

/// SQL dialect hint — mostly cosmetic (identifiers are normalized for every
/// dialect), but MySQL/auto also treat `#` as a line-comment marker.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Auto,
    Mysql,
    Postgres,
    Sqlite,
    Mssql,
    Generic,
}

impl Dialect {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Dialect::Auto),
            "mysql" | "mariadb" => Ok(Dialect::Mysql),
            "postgres" | "postgresql" | "pg" => Ok(Dialect::Postgres),
            "sqlite" | "sqlite3" => Ok(Dialect::Sqlite),
            "mssql" | "sqlserver" | "tsql" => Ok(Dialect::Mssql),
            "generic" | "ansi" | "standard" => Ok(Dialect::Generic),
            other => Err(format!(
                "invalid dialect {other:?}: expected auto, mysql, postgres, sqlite, mssql, or generic"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Dialect::Auto => "auto",
            Dialect::Mysql => "mysql",
            Dialect::Postgres => "postgres",
            Dialect::Sqlite => "sqlite",
            Dialect::Mssql => "mssql",
            Dialect::Generic => "generic",
        }
    }
    /// Whether `#` starts a line comment (MySQL-family dumps use it).
    fn hash_comments(self) -> bool {
        matches!(self, Dialect::Auto | Dialect::Mysql)
    }
}

enum OutputFormat {
    Json,
    Markdown,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(OutputFormat::Json),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            other => Err(format!(
                "invalid output {other:?}: expected \"json\" or \"markdown\""
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse `sql` and render the extracted schema.
///
/// * `output` — `json` | `markdown`.
/// * `dialect` — `auto` | `mysql` | `postgres` | `sqlite` | `mssql` | `generic`.
/// * `apply_alter` — fold `ALTER TABLE` statements onto their target table.
/// * `include_indexes` — keep index definitions (`INDEX`/`KEY`/`CREATE INDEX`).
pub fn extract(
    sql: &str,
    output: &str,
    dialect: &str,
    apply_alter: bool,
    include_indexes: bool,
) -> Result<String, String> {
    let out = OutputFormat::parse(output)?;
    let dia = Dialect::parse(dialect)?;

    if sql.trim().is_empty() {
        return Err("no SQL provided: paste a dump containing CREATE TABLE / ALTER TABLE".into());
    }

    let cleaned = strip_comments(sql, dia.hash_comments());
    let statements = split_statements(&cleaned);

    let mut schema = Schema::default();
    for stmt in &statements {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        let head = leading_keywords(trimmed, 3);
        if starts_with_kw(&head, &["CREATE", "TABLE"])
            || starts_with_kw(&head, &["CREATE", "TEMPORARY", "TABLE"])
            || starts_with_kw(&head, &["CREATE", "TEMP", "TABLE"])
        {
            parse_create_table(trimmed, &mut schema, include_indexes);
        } else if apply_alter && starts_with_kw(&head, &["ALTER", "TABLE"]) {
            parse_alter_table(trimmed, &mut schema, include_indexes);
        } else if include_indexes
            && (starts_with_kw(&head, &["CREATE", "INDEX"])
                || starts_with_kw(&head, &["CREATE", "UNIQUE", "INDEX"]))
        {
            parse_create_index(trimmed, &mut schema);
        }
        // Everything else (INSERT, DROP, SET, comments, unknown) is skipped.
    }

    if schema.tables.is_empty() {
        return Err(
            "no CREATE TABLE / ALTER TABLE statements found: this tool extracts a schema from DDL, \
             not from INSERT rows or query results"
                .into(),
        );
    }

    match out {
        OutputFormat::Json => Ok(render_json(&schema, dia, include_indexes)),
        OutputFormat::Markdown => Ok(render_markdown(&schema, dia, include_indexes)),
    }
}

// ---------------------------------------------------------------------------
// Lexing helpers
// ---------------------------------------------------------------------------

/// Remove `--`/`#` line comments and `/* */` block comments, preserving string
/// and quoted-identifier literals verbatim.
fn strip_comments(sql: &str, hash_comments: bool) -> String {
    let b = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            '\'' | '"' | '`' => {
                let (lit, ni) = read_quoted(b, i, c);
                out.push_str(lit);
                i = ni;
            }
            '[' => {
                // MSSQL bracket identifier — no escapes inside.
                let start = i;
                i += 1;
                while i < b.len() && b[i] as char != ']' {
                    i += 1;
                }
                if i < b.len() {
                    i += 1; // consume ']'
                }
                out.push_str(&sql[start..i]);
            }
            '-' if i + 1 < b.len() && b[i + 1] as char == '-' => {
                while i < b.len() && b[i] as char != '\n' {
                    i += 1;
                }
            }
            '#' if hash_comments => {
                while i < b.len() && b[i] as char != '\n' {
                    i += 1;
                }
            }
            '/' if i + 1 < b.len() && b[i + 1] as char == '*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] as char == '*' && b[i + 1] as char == '/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
                out.push(' ');
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Read a quoted literal starting at `start` (whose byte is `quote`), honoring
/// doubled-quote escapes (`''`, `""`, ` `` `) and backslash escapes. Returns the
/// slice (including the surrounding quotes) and the index just past it.
fn read_quoted(b: &[u8], start: usize, quote: char) -> (&str, usize) {
    let mut i = start + 1;
    while i < b.len() {
        let c = b[i] as char;
        if c == '\\' && i + 1 < b.len() {
            i += 2;
            continue;
        }
        if c == quote {
            if i + 1 < b.len() && b[i + 1] as char == quote {
                i += 2; // doubled → escaped quote
                continue;
            }
            i += 1; // closing quote
            break;
        }
        i += 1;
    }
    // Slicing note: quote/backslash scanning only ever matches ASCII bytes; any
    // multibyte literal body byte is stepped over as a whole, so `start..i`
    // stays on a char boundary.
    (std::str::from_utf8(&b[start..i]).unwrap_or(""), i)
}

/// Split comment-stripped SQL into statements at top-level `;`, respecting
/// string/identifier literals and parentheses.
fn split_statements(sql: &str) -> Vec<String> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut depth = 0i32;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            '\'' | '"' | '`' => {
                let (_, ni) = read_quoted(b, i, c);
                i = ni;
            }
            '[' => {
                i += 1;
                while i < b.len() && b[i] as char != ']' {
                    i += 1;
                }
                if i < b.len() {
                    i += 1;
                }
            }
            '(' => {
                depth += 1;
                i += 1;
            }
            ')' => {
                depth = (depth - 1).max(0);
                i += 1;
            }
            ';' if depth == 0 => {
                out.push(sql[start..i].to_string());
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < sql.len() {
        out.push(sql[start..].to_string());
    }
    out
}

/// Tokenize a fragment: quoted identifiers/strings and balanced `(...)` groups
/// each become a single token; bare runs (identifiers, keywords, numbers,
/// operators like `>=`) become word tokens; a bare `,` is its own token.
fn tokenize(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                let (lit, ni) = read_quoted(b, i, c);
                toks.push(lit.to_string());
                i = ni;
            }
            '[' => {
                let start = i;
                i += 1;
                while i < b.len() && b[i] as char != ']' {
                    i += 1;
                }
                if i < b.len() {
                    i += 1;
                }
                toks.push(s[start..i].to_string());
            }
            '(' => {
                let (grp, ni) = read_group(b, i);
                toks.push(grp.to_string());
                i = ni;
            }
            ',' => {
                toks.push(",".to_string());
                i += 1;
            }
            _ => {
                let start = i;
                while i < b.len() {
                    let d = b[i] as char;
                    if d.is_whitespace() || matches!(d, '\'' | '"' | '`' | '[' | '(' | ',') {
                        break;
                    }
                    i += 1;
                }
                toks.push(s[start..i].to_string());
            }
        }
    }
    toks
}

/// Read a balanced parenthesized group starting at `(`; returns the slice
/// including both parens and the index just past it.
fn read_group(b: &[u8], start: usize) -> (&str, usize) {
    let mut i = start;
    let mut depth = 0i32;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            '\'' | '"' | '`' => {
                let (_, ni) = read_quoted(b, i, c);
                i = ni;
            }
            '(' => {
                depth += 1;
                i += 1;
            }
            ')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    break;
                }
            }
            _ => i += 1,
        }
    }
    (std::str::from_utf8(&b[start..i]).unwrap_or(""), i)
}

/// Split a comma-separated list at top-level commas (respecting parens/quotes).
fn split_top_commas(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut depth = 0i32;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            '\'' | '"' | '`' => {
                let (_, ni) = read_quoted(b, i, c);
                i = ni;
            }
            '[' => {
                i += 1;
                while i < b.len() && b[i] as char != ']' {
                    i += 1;
                }
                if i < b.len() {
                    i += 1;
                }
            }
            '(' => {
                depth += 1;
                i += 1;
            }
            ')' => {
                depth = (depth - 1).max(0);
                i += 1;
            }
            ',' if depth == 0 => {
                parts.push(s[start..i].to_string());
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < s.len() {
        parts.push(s[start..].to_string());
    }
    parts
}

/// First `n` whitespace-separated keywords, upper-cased.
fn leading_keywords(s: &str, n: usize) -> Vec<String> {
    s.split_whitespace()
        .take(n)
        .map(|w| w.to_ascii_uppercase())
        .collect()
}

fn starts_with_kw(head: &[String], kws: &[&str]) -> bool {
    head.len() >= kws.len() && head.iter().zip(kws).all(|(h, k)| h.eq_ignore_ascii_case(k))
}

/// Strip identifier quoting (`` `x` `` / `"x"` / `[x]`) and return the bare name.
/// A schema-qualified `a.b` keeps only the last part (the object's own name).
fn unquote_ident(raw: &str) -> String {
    let last = split_qualified(raw).pop().unwrap_or_default();
    strip_quotes(&last)
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let (a, z) = (bytes[0] as char, bytes[bytes.len() - 1] as char);
        if (a == '`' && z == '`') || (a == '"' && z == '"') {
            return s[1..s.len() - 1].replace(&format!("{a}{a}"), &a.to_string());
        }
        if a == '[' && z == ']' {
            return s[1..s.len() - 1].to_string();
        }
        if a == '\'' && z == '\'' {
            return s[1..s.len() - 1].replace("''", "'");
        }
    }
    s.to_string()
}

/// Split a possibly-qualified name (`schema.table`, `` `db`.`t` ``) into its
/// dot-separated parts, respecting quotes.
fn split_qualified(raw: &str) -> Vec<String> {
    let b = raw.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            '`' | '"' => {
                let (_, ni) = read_quoted(b, i, c);
                i = ni;
            }
            '[' => {
                i += 1;
                while i < b.len() && b[i] as char != ']' {
                    i += 1;
                }
                if i < b.len() {
                    i += 1;
                }
            }
            '.' => {
                parts.push(raw[start..i].to_string());
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    parts.push(raw[start..].to_string());
    parts
}

/// Parse the comma-separated column list inside a `(...)` group token.
fn parse_column_group(group: &str) -> Vec<String> {
    let inner = strip_outer_parens(group);
    split_top_commas(&inner)
        .iter()
        // A column ref may carry a length/order suffix — `name(10)`, `id ASC` —
        // keep only the leading identifier token.
        .filter_map(|c| tokenize(c.trim()).into_iter().next())
        .map(|c| unquote_ident(&c))
        .filter(|c| !c.is_empty())
        .collect()
}

fn is_group(t: &str) -> bool {
    t.starts_with('(')
}

/// Read a possibly schema-qualified name starting at `i` and return the bare
/// object name plus the next index. The tokenizer splits `` `db`.`t` `` into
/// three tokens (quote, `.`, quote); this stitches them back together.
fn read_name(toks: &[String], i: usize) -> (String, usize) {
    if i >= toks.len() {
        return (String::new(), i);
    }
    let mut raw = toks[i].clone();
    let mut j = i + 1;
    loop {
        if j < toks.len() && toks[j] == "." {
            raw.push('.');
            if let Some(next) = toks.get(j + 1) {
                raw.push_str(next);
                j += 2;
            } else {
                j += 1;
                break;
            }
        } else if j < toks.len() && toks[j].starts_with('.') && !is_group(&toks[j]) {
            raw.push_str(&toks[j]);
            j += 1;
        } else {
            break;
        }
    }
    (unquote_ident(&raw), j)
}

// ---------------------------------------------------------------------------
// CREATE TABLE
// ---------------------------------------------------------------------------

fn parse_create_table(stmt: &str, schema: &mut Schema, include_indexes: bool) {
    // Locate the body group (first top-level `(...)`) and the name before it.
    let open = match find_top_paren(stmt) {
        Some(p) => p,
        None => return,
    };
    let header = &stmt[..open];
    let body_group = read_group(stmt.as_bytes(), open).0; // starts at '('
    let name = table_name_from_header(header);
    let name = if name.is_empty() {
        DEFAULT_TABLE.to_string()
    } else {
        name
    };

    let inner = strip_outer_parens(body_group);

    let mut table = Table {
        name: name.clone(),
        ..Default::default()
    };
    for part in split_top_commas(&inner) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        parse_table_item(part, &mut table, include_indexes);
    }
    // Merge into any pre-existing stub (e.g. created by an earlier ALTER).
    merge_table(schema, table);
}

/// Extract the table name from a `CREATE … TABLE … <name>` header.
fn table_name_from_header(header: &str) -> String {
    let toks = tokenize(header);
    let mut i = 0;
    while i < toks.len() && !toks[i].eq_ignore_ascii_case("TABLE") {
        i += 1;
    }
    i += 1; // past TABLE
    // skip IF NOT EXISTS
    if i + 2 < toks.len()
        && toks[i].eq_ignore_ascii_case("IF")
        && toks[i + 1].eq_ignore_ascii_case("NOT")
        && toks[i + 2].eq_ignore_ascii_case("EXISTS")
    {
        i += 3;
    }
    read_name(&toks, i).0
}

/// Byte offset of the first top-level `(` in a statement (respecting quotes).
fn find_top_paren(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        match c {
            '\'' | '"' | '`' => {
                let (_, ni) = read_quoted(b, i, c);
                i = ni;
            }
            '[' => {
                i += 1;
                while i < b.len() && b[i] as char != ']' {
                    i += 1;
                }
                if i < b.len() {
                    i += 1;
                }
            }
            '(' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// One item inside a `CREATE TABLE (...)` body — either a table constraint or a
/// column definition.
fn parse_table_item(part: &str, table: &mut Table, include_indexes: bool) {
    let toks = tokenize(part);
    if toks.is_empty() {
        return;
    }
    // A leading `CONSTRAINT <name>` may prefix a table constraint.
    let (constraint_name, rest) = if toks[0].eq_ignore_ascii_case("CONSTRAINT") {
        let cname = toks.get(1).map(|t| unquote_ident(t));
        (cname, &toks[2.min(toks.len())..])
    } else {
        (None, &toks[..])
    };
    let first = rest
        .first()
        .map(|t| t.to_ascii_uppercase())
        .unwrap_or_default();

    match first.as_str() {
        "PRIMARY" => apply_primary_key(rest, table),
        "FOREIGN" => {
            if let Some(fk) = parse_foreign_key(rest, constraint_name) {
                table.foreign_keys.push(fk);
            }
        }
        "UNIQUE" => {
            if let Some(u) = parse_unique(rest, constraint_name) {
                table.uniques.push(u);
            }
        }
        "CHECK" => {
            if let Some(expr) = first_group(rest) {
                table.checks.push(Check {
                    name: constraint_name,
                    expression: strip_outer_parens(&expr),
                });
            }
        }
        "KEY" | "INDEX" | "FULLTEXT" | "SPATIAL" => {
            if include_indexes {
                if let Some(idx) = parse_inline_index(rest) {
                    table.indexes.push(idx);
                }
            }
        }
        _ => {
            // Column definition (a leading CONSTRAINT before a bare word is not
            // valid, so only treat as a column when there was no constraint).
            if constraint_name.is_none() {
                if let Some(col) = parse_column(&toks, table) {
                    table.columns.push(col);
                }
            }
        }
    }
}

/// Parse a single column definition from its tokens.
fn parse_column(toks: &[String], table: &mut Table) -> Option<Column> {
    let mut col = Column {
        name: unquote_ident(&toks[0]),
        ..Default::default()
    };
    if col.name.is_empty() {
        return None;
    }
    let mut i = 1;
    // Data type: a keyword, optional `(args)` group, plus known continuation
    // words (DOUBLE PRECISION, CHARACTER VARYING, TIMESTAMP WITH TIME ZONE, INT
    // UNSIGNED, …).
    if i < toks.len() && !is_column_option(&toks[i]) {
        let mut ty = toks[i].to_ascii_uppercase();
        i += 1;
        if i < toks.len() && is_group(&toks[i]) {
            ty.push_str(&normalize_type_args(&toks[i]));
            i += 1;
        }
        while i < toks.len() && is_type_continuation(&toks[i]) {
            ty.push(' ');
            ty.push_str(&toks[i].to_ascii_uppercase());
            i += 1;
            if i < toks.len() && is_group(&toks[i]) {
                ty.push_str(&normalize_type_args(&toks[i]));
                i += 1;
            }
        }
        col.data_type = ty;
    }

    // Column options.
    while i < toks.len() {
        let up = toks[i].to_ascii_uppercase();
        match up.as_str() {
            "NOT" => {
                if toks.get(i + 1).map(|t| t.eq_ignore_ascii_case("NULL")) == Some(true) {
                    col.nullable = Some(false);
                    i += 2;
                    continue;
                }
                i += 1;
            }
            "NULL" => {
                col.nullable = Some(true);
                i += 1;
            }
            "DEFAULT" => {
                let (val, ni) = read_default(toks, i + 1);
                col.default = Some(val);
                i = ni;
            }
            "PRIMARY" => {
                col.primary_key = true;
                col.nullable = Some(false);
                if !table
                    .primary_key
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(&col.name))
                {
                    table.primary_key.push(col.name.clone());
                }
                if toks.get(i + 1).map(|t| t.eq_ignore_ascii_case("KEY")) == Some(true) {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "UNIQUE" => {
                col.unique = true;
                if toks.get(i + 1).map(|t| t.eq_ignore_ascii_case("KEY")) == Some(true) {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "AUTO_INCREMENT" | "AUTOINCREMENT" | "IDENTITY" | "SERIAL" => {
                col.auto_increment = true;
                // IDENTITY may carry (seed, incr) — skip a following group.
                if toks.get(i + 1).map(|t| is_group(t)) == Some(true) {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "REFERENCES" => {
                if let Some(fk) = parse_inline_references(&col.name, toks, i) {
                    table.foreign_keys.push(fk);
                }
                i = skip_references(toks, i);
            }
            "CHECK" => {
                if let Some(g) = toks.get(i + 1).filter(|t| is_group(t)) {
                    table.checks.push(Check {
                        name: None,
                        expression: strip_outer_parens(g),
                    });
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "COLLATE" => {
                i += 2; // COLLATE <name>
            }
            "COMMENT" => {
                if let Some(c) = toks.get(i + 1) {
                    col.comment = Some(strip_quotes(c));
                }
                i += 2;
            }
            "GENERATED" => {
                // GENERATED ALWAYS AS (...) [STORED|VIRTUAL] — skip to the group.
                i += 1;
                while i < toks.len() && !is_group(&toks[i]) && !is_column_option(&toks[i]) {
                    i += 1;
                }
                if i < toks.len() && is_group(&toks[i]) {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    Some(col)
}

/// Read a DEFAULT value starting at index `i`. A function-call default
/// (`nextval('x')`, `now()`) keeps its argument group.
fn read_default(toks: &[String], i: usize) -> (String, usize) {
    if i >= toks.len() {
        return (String::new(), i);
    }
    let mut val = toks[i].clone();
    let mut ni = i + 1;
    if ni < toks.len() && is_group(&toks[ni]) {
        val.push_str(&toks[ni]);
        ni += 1;
    }
    (val, ni)
}

fn skip_references(toks: &[String], start: usize) -> usize {
    // start points at REFERENCES; advance past table, optional (cols), and any
    // ON DELETE/UPDATE <action> clauses.
    let (_, mut i) = read_name(toks, start + 1);
    if i < toks.len() && is_group(&toks[i]) {
        i += 1;
    }
    while i < toks.len() && toks[i].eq_ignore_ascii_case("ON") {
        let (_, ni) = read_action(toks, i + 2);
        i = ni;
    }
    i
}

fn parse_inline_references(col: &str, toks: &[String], refs_idx: usize) -> Option<ForeignKey> {
    let (ref_table, mut i) = read_name(toks, refs_idx + 1);
    if ref_table.is_empty() {
        return None;
    }
    let mut ref_columns = Vec::new();
    if let Some(g) = toks.get(i).filter(|t| is_group(t)) {
        ref_columns = parse_column_group(g);
        i += 1;
    }
    let (on_delete, on_update) = parse_ref_actions(toks, i);
    Some(ForeignKey {
        name: None,
        columns: vec![col.to_string()],
        ref_table,
        ref_columns,
        on_delete,
        on_update,
    })
}

fn parse_ref_actions(toks: &[String], start: usize) -> (Option<String>, Option<String>) {
    let mut on_delete = None;
    let mut on_update = None;
    let mut i = start;
    while i < toks.len() && toks[i].eq_ignore_ascii_case("ON") {
        let which = toks.get(i + 1).map(|t| t.to_ascii_uppercase());
        let (action, ni) = read_action(toks, i + 2);
        match which.as_deref() {
            Some("DELETE") => on_delete = Some(action),
            Some("UPDATE") => on_update = Some(action),
            _ => {}
        }
        i = ni;
    }
    (on_delete, on_update)
}

fn read_action(toks: &[String], i: usize) -> (String, usize) {
    let a = toks
        .get(i)
        .map(|t| t.to_ascii_uppercase())
        .unwrap_or_default();
    match a.as_str() {
        "NO" => ("NO ACTION".to_string(), i + 2), // NO ACTION
        "SET" => {
            let b = toks
                .get(i + 1)
                .map(|t| t.to_ascii_uppercase())
                .unwrap_or_default();
            (format!("SET {b}"), i + 2) // SET NULL / SET DEFAULT
        }
        other => (other.to_string(), i + 1), // CASCADE / RESTRICT
    }
}

fn apply_primary_key(rest: &[String], table: &mut Table) {
    // PRIMARY KEY (col, ...)
    if let Some(g) = first_group(rest) {
        for c in parse_column_group(&g) {
            if !table.primary_key.iter().any(|p| p.eq_ignore_ascii_case(&c)) {
                table.primary_key.push(c.clone());
            }
            if let Some(col) = table.column_mut(&c) {
                col.primary_key = true;
                if col.nullable.is_none() {
                    col.nullable = Some(false);
                }
            }
        }
    }
}

fn parse_foreign_key(rest: &[String], name: Option<String>) -> Option<ForeignKey> {
    // FOREIGN KEY [index_name] (cols) REFERENCES table [(refcols)] [ON ...]
    let mut i = 1;
    if rest.get(i).map(|t| t.eq_ignore_ascii_case("KEY")) == Some(true) {
        i += 1;
    }
    let mut columns = Vec::new();
    while i < rest.len() {
        if is_group(&rest[i]) {
            columns = parse_column_group(&rest[i]);
            i += 1;
            break;
        }
        i += 1;
    }
    while i < rest.len() && !rest[i].eq_ignore_ascii_case("REFERENCES") {
        i += 1;
    }
    if i >= rest.len() {
        return None;
    }
    i += 1; // past REFERENCES
    let (ref_table, ni) = read_name(rest, i);
    if ref_table.is_empty() {
        return None;
    }
    i = ni;
    let mut ref_columns = Vec::new();
    if let Some(g) = rest.get(i).filter(|t| is_group(t)) {
        ref_columns = parse_column_group(g);
        i += 1;
    }
    let (on_delete, on_update) = parse_ref_actions(rest, i);
    Some(ForeignKey {
        name,
        columns,
        ref_table,
        ref_columns,
        on_delete,
        on_update,
    })
}

fn parse_unique(rest: &[String], name: Option<String>) -> Option<Unique> {
    // UNIQUE [KEY|INDEX] [name] (cols)
    let g = first_group(rest)?;
    Some(Unique {
        name,
        columns: parse_column_group(&g),
    })
}

fn parse_inline_index(rest: &[String]) -> Option<Index> {
    // [KEY|INDEX|FULLTEXT|SPATIAL] [KEY|INDEX] [name] (cols)
    let mut i = 1;
    if rest.get(i).map(|t| {
        let u = t.to_ascii_uppercase();
        u == "KEY" || u == "INDEX"
    }) == Some(true)
    {
        i += 1;
    }
    let mut name = None;
    if let Some(t) = rest.get(i) {
        if !is_group(t) {
            name = Some(unquote_ident(t));
            i += 1;
        }
    }
    let cols = rest
        .get(i)
        .filter(|t| is_group(t))
        .map(|g| parse_column_group(g))?;
    Some(Index {
        name,
        columns: cols,
        unique: false,
    })
}

fn first_group(toks: &[String]) -> Option<String> {
    toks.iter().find(|t| is_group(t)).cloned()
}

fn strip_outer_parens(s: &str) -> String {
    let s = s.trim();
    s.strip_prefix('(')
        .and_then(|x| x.strip_suffix(')'))
        .map(|x| x.trim().to_string())
        .unwrap_or_else(|| s.to_string())
}

/// Normalize a type-arg group like `( 255 )` / `(10, 2)` → `(255)` / `(10,2)`.
fn normalize_type_args(g: &str) -> String {
    let inner = strip_outer_parens(g);
    let joined = split_top_commas(&inner)
        .iter()
        .map(|p| p.trim())
        .collect::<Vec<_>>()
        .join(",");
    format!("({joined})")
}

fn is_column_option(t: &str) -> bool {
    matches!(
        t.to_ascii_uppercase().as_str(),
        "NOT" | "NULL"
            | "DEFAULT"
            | "PRIMARY"
            | "UNIQUE"
            | "REFERENCES"
            | "CHECK"
            | "COLLATE"
            | "COMMENT"
            | "GENERATED"
            | "AUTO_INCREMENT"
            | "AUTOINCREMENT"
            | "IDENTITY"
            | "CONSTRAINT"
    )
}

fn is_type_continuation(t: &str) -> bool {
    matches!(
        t.to_ascii_uppercase().as_str(),
        "PRECISION"
            | "VARYING"
            | "UNSIGNED"
            | "SIGNED"
            | "ZEROFILL"
            | "WITH"
            | "WITHOUT"
            | "TIME"
            | "ZONE"
            | "LOCAL"
            | "SET"
            | "CHARACTER"
            | "NATIONAL"
    )
}

// ---------------------------------------------------------------------------
// ALTER TABLE
// ---------------------------------------------------------------------------

fn parse_alter_table(stmt: &str, schema: &mut Schema, include_indexes: bool) {
    let toks = tokenize(stmt);
    // ALTER TABLE [IF EXISTS] [ONLY] <name> <actions...>
    let mut i = 2; // past ALTER TABLE
    if toks.get(i).map(|t| t.eq_ignore_ascii_case("IF")) == Some(true) {
        i += 2; // IF EXISTS
    }
    if toks.get(i).map(|t| t.eq_ignore_ascii_case("ONLY")) == Some(true) {
        i += 1;
    }
    let (name, ni) = read_name(&toks, i);
    if name.is_empty() {
        return;
    }
    i = ni;
    let rest_str = toks[i..].join(" ");
    let table = schema.table_mut(&name);
    for action in split_top_commas(&rest_str) {
        let atoks = tokenize(action.trim());
        apply_alter_action(&atoks, table, include_indexes);
    }
}

fn apply_alter_action(toks: &[String], table: &mut Table, include_indexes: bool) {
    if toks.is_empty() {
        return;
    }
    if !toks[0].eq_ignore_ascii_case("ADD") {
        return; // only ADD is folded into the model; DROP/MODIFY skipped
    }
    let mut i = 1;
    // optional COLUMN keyword
    if toks.get(i).map(|t| t.eq_ignore_ascii_case("COLUMN")) == Some(true) {
        i += 1;
    }
    // optional CONSTRAINT <name>
    let mut cname = None;
    if toks.get(i).map(|t| t.eq_ignore_ascii_case("CONSTRAINT")) == Some(true) {
        cname = toks.get(i + 1).map(|t| unquote_ident(t));
        i += 2;
    }
    let rest = &toks[i.min(toks.len())..];
    let first = rest
        .first()
        .map(|t| t.to_ascii_uppercase())
        .unwrap_or_default();
    match first.as_str() {
        "PRIMARY" => apply_primary_key(rest, table),
        "FOREIGN" => {
            if let Some(fk) = parse_foreign_key(rest, cname) {
                table.foreign_keys.push(fk);
            }
        }
        "UNIQUE" => {
            if let Some(u) = parse_unique(rest, cname) {
                table.uniques.push(u);
            }
        }
        "CHECK" => {
            if let Some(g) = first_group(rest) {
                table.checks.push(Check {
                    name: cname,
                    expression: strip_outer_parens(&g),
                });
            }
        }
        "KEY" | "INDEX" | "FULLTEXT" | "SPATIAL" => {
            if include_indexes {
                if let Some(idx) = parse_inline_index(rest) {
                    table.indexes.push(idx);
                }
            }
        }
        _ if cname.is_none() => {
            // ADD [COLUMN] <coldef>
            if let Some(col) = parse_column(rest, table) {
                if !col.name.is_empty() && table.column_mut(&col.name).is_none() {
                    table.columns.push(col);
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// CREATE INDEX
// ---------------------------------------------------------------------------

fn parse_create_index(stmt: &str, schema: &mut Schema) {
    let toks = tokenize(stmt);
    // CREATE [UNIQUE] INDEX [IF NOT EXISTS] name ON table (cols)
    let mut i = 1; // past CREATE
    let mut unique = false;
    if toks.get(i).map(|t| t.eq_ignore_ascii_case("UNIQUE")) == Some(true) {
        unique = true;
        i += 1;
    }
    if toks.get(i).map(|t| t.eq_ignore_ascii_case("INDEX")) != Some(true) {
        return;
    }
    i += 1;
    if toks.get(i).map(|t| t.eq_ignore_ascii_case("IF")) == Some(true) {
        i += 3; // IF NOT EXISTS
    }
    let idx_name = toks.get(i).map(|t| unquote_ident(t));
    i += 1;
    if toks.get(i).map(|t| t.eq_ignore_ascii_case("ON")) != Some(true) {
        return;
    }
    i += 1;
    let (table_name, ni) = read_name(&toks, i);
    if table_name.is_empty() {
        return;
    }
    i = ni;
    let cols = match toks.get(i).filter(|t| is_group(t)) {
        Some(g) => parse_column_group(g),
        None => return,
    };
    let table = schema.table_mut(&table_name);
    table.indexes.push(Index {
        name: idx_name,
        columns: cols,
        unique,
    });
}

// ---------------------------------------------------------------------------
// Merge (ALTER-created stub ← CREATE TABLE)
// ---------------------------------------------------------------------------

fn merge_table(schema: &mut Schema, new: Table) {
    if let Some(existing) = schema
        .tables
        .iter_mut()
        .find(|t| t.name.eq_ignore_ascii_case(&new.name))
    {
        // A CREATE arriving after an ALTER stub: columns/PK from CREATE fill the
        // stub; the stub's already-gathered constraints are kept (prepended).
        if existing.columns.is_empty() {
            existing.columns = new.columns;
        }
        if existing.primary_key.is_empty() {
            existing.primary_key = new.primary_key;
        }
        existing.foreign_keys.splice(0..0, new.foreign_keys);
        existing.uniques.splice(0..0, new.uniques);
        existing.checks.splice(0..0, new.checks);
        existing.indexes.splice(0..0, new.indexes);
        // Re-mark PK flags now that both columns and PK list are present.
        let pk = existing.primary_key.clone();
        for c in pk {
            if let Some(col) = existing.column_mut(&c) {
                col.primary_key = true;
                if col.nullable.is_none() {
                    col.nullable = Some(false);
                }
            }
        }
    } else {
        schema.tables.push(new);
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_json(schema: &Schema, dia: Dialect, include_indexes: bool) -> String {
    let mut total_cols = 0usize;
    let tables: Vec<Value> = schema
        .tables
        .iter()
        .map(|t| {
            total_cols += t.columns.len();
            let columns: Vec<Value> = t
                .columns
                .iter()
                .map(|c| {
                    let mut m = Map::new();
                    m.insert("name".into(), json!(c.name));
                    m.insert("type".into(), json!(c.data_type));
                    m.insert("nullable".into(), json!(c.nullable.unwrap_or(true)));
                    if let Some(d) = &c.default {
                        m.insert("default".into(), json!(d));
                    }
                    if c.primary_key {
                        m.insert("primary_key".into(), json!(true));
                    }
                    if c.unique {
                        m.insert("unique".into(), json!(true));
                    }
                    if c.auto_increment {
                        m.insert("auto_increment".into(), json!(true));
                    }
                    if let Some(cm) = &c.comment {
                        m.insert("comment".into(), json!(cm));
                    }
                    Value::Object(m)
                })
                .collect();

            let fks: Vec<Value> = t
                .foreign_keys
                .iter()
                .map(|fk| {
                    let mut m = Map::new();
                    if let Some(n) = &fk.name {
                        m.insert("name".into(), json!(n));
                    }
                    m.insert("columns".into(), json!(fk.columns));
                    m.insert(
                        "references".into(),
                        json!({ "table": fk.ref_table, "columns": fk.ref_columns }),
                    );
                    if let Some(a) = &fk.on_delete {
                        m.insert("on_delete".into(), json!(a));
                    }
                    if let Some(a) = &fk.on_update {
                        m.insert("on_update".into(), json!(a));
                    }
                    Value::Object(m)
                })
                .collect();

            let uniques: Vec<Value> = t
                .uniques
                .iter()
                .map(|u| {
                    let mut m = Map::new();
                    if let Some(n) = &u.name {
                        m.insert("name".into(), json!(n));
                    }
                    m.insert("columns".into(), json!(u.columns));
                    Value::Object(m)
                })
                .collect();

            let checks: Vec<Value> = t
                .checks
                .iter()
                .map(|c| {
                    let mut m = Map::new();
                    if let Some(n) = &c.name {
                        m.insert("name".into(), json!(n));
                    }
                    m.insert("expression".into(), json!(c.expression));
                    Value::Object(m)
                })
                .collect();

            let mut tm = Map::new();
            tm.insert("name".into(), json!(t.name));
            tm.insert("columns".into(), Value::Array(columns));
            tm.insert("primary_key".into(), json!(t.primary_key));
            tm.insert("foreign_keys".into(), Value::Array(fks));
            tm.insert("unique_constraints".into(), Value::Array(uniques));
            tm.insert("checks".into(), Value::Array(checks));
            if include_indexes {
                let indexes: Vec<Value> = t
                    .indexes
                    .iter()
                    .map(|idx| {
                        let mut m = Map::new();
                        if let Some(n) = &idx.name {
                            m.insert("name".into(), json!(n));
                        }
                        m.insert("columns".into(), json!(idx.columns));
                        m.insert("unique".into(), json!(idx.unique));
                        Value::Object(m)
                    })
                    .collect();
                tm.insert("indexes".into(), Value::Array(indexes));
            }
            Value::Object(tm)
        })
        .collect();

    let out = json!({
        "dialect": dia.label(),
        "table_count": schema.tables.len(),
        "column_count": total_cols,
        "tables": tables,
    });
    serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".into())
}

fn render_markdown(schema: &Schema, dia: Dialect, include_indexes: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Schema ({} table{}, dialect: {})\n",
        schema.tables.len(),
        if schema.tables.len() == 1 { "" } else { "s" },
        dia.label()
    ));

    for t in &schema.tables {
        out.push_str(&format!("\n## {}\n\n", t.name));
        out.push_str("| Column | Type | Nullable | Key | Default | Extra |\n");
        out.push_str("|--------|------|----------|-----|---------|-------|\n");
        for c in &t.columns {
            let key = if c.primary_key {
                "PK"
            } else if c.unique {
                "UNIQUE"
            } else {
                ""
            };
            let nullable = if c.nullable.unwrap_or(true) { "YES" } else { "NO" };
            let default = c.default.clone().unwrap_or_default();
            let mut extra = Vec::new();
            if c.auto_increment {
                extra.push("AUTO_INCREMENT".to_string());
            }
            if let Some(cm) = &c.comment {
                extra.push(format!("COMMENT {cm}"));
            }
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                md_cell(&c.name),
                md_cell(&c.data_type),
                nullable,
                key,
                md_cell(&default),
                md_cell(&extra.join(", ")),
            ));
        }

        if !t.primary_key.is_empty() {
            out.push_str(&format!(
                "\n- **Primary key:** {}\n",
                t.primary_key.join(", ")
            ));
        }
        for fk in &t.foreign_keys {
            let name = fk
                .name
                .as_ref()
                .map(|n| format!("`{n}` "))
                .unwrap_or_default();
            let refc = if fk.ref_columns.is_empty() {
                String::new()
            } else {
                format!(" ({})", fk.ref_columns.join(", "))
            };
            let mut acts = Vec::new();
            if let Some(a) = &fk.on_delete {
                acts.push(format!("ON DELETE {a}"));
            }
            if let Some(a) = &fk.on_update {
                acts.push(format!("ON UPDATE {a}"));
            }
            let act = if acts.is_empty() {
                String::new()
            } else {
                format!(" {}", acts.join(" "))
            };
            out.push_str(&format!(
                "- **Foreign key:** {}({}) → {}{}{}\n",
                name,
                fk.columns.join(", "),
                fk.ref_table,
                refc,
                act
            ));
        }
        for u in &t.uniques {
            let name = u
                .name
                .as_ref()
                .map(|n| format!("`{n}` "))
                .unwrap_or_default();
            out.push_str(&format!("- **Unique:** {}({})\n", name, u.columns.join(", ")));
        }
        for c in &t.checks {
            let name = c
                .name
                .as_ref()
                .map(|n| format!("`{n}` "))
                .unwrap_or_default();
            out.push_str(&format!("- **Check:** {}{}\n", name, c.expression));
        }
        if include_indexes {
            for idx in &t.indexes {
                let name = idx
                    .name
                    .as_ref()
                    .map(|n| format!("`{n}` "))
                    .unwrap_or_default();
                let kind = if idx.unique { "Unique index" } else { "Index" };
                out.push_str(&format!(
                    "- **{}:** {}({})\n",
                    kind,
                    name,
                    idx.columns.join(", ")
                ));
            }
        }
    }
    out
}

/// Escape `|` and newlines so a value stays inside a Markdown table cell.
fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn json_val(sql: &str) -> Value {
        let s = extract(sql, "json", "auto", true, true).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn happy_simple_create_table() {
        let sql = "CREATE TABLE users (\n  id INT PRIMARY KEY AUTO_INCREMENT,\n  \
                   email VARCHAR(255) NOT NULL UNIQUE,\n  age INT DEFAULT 0 CHECK (age >= 0)\n);";
        let v = json_val(sql);
        assert_eq!(v["table_count"], 1);
        assert_eq!(v["column_count"], 3);
        let t = &v["tables"][0];
        assert_eq!(t["name"], "users");
        assert_eq!(t["primary_key"][0], "id");
        let cols = t["columns"].as_array().unwrap();
        assert_eq!(cols[0]["name"], "id");
        assert_eq!(cols[0]["type"], "INT");
        assert_eq!(cols[0]["nullable"], false);
        assert_eq!(cols[0]["primary_key"], true);
        assert_eq!(cols[0]["auto_increment"], true);
        assert_eq!(cols[1]["type"], "VARCHAR(255)");
        assert_eq!(cols[1]["unique"], true);
        assert_eq!(cols[2]["default"], "0");
        assert_eq!(t["checks"][0]["expression"], "age >= 0");
    }

    #[test]
    fn alter_add_foreign_key_folds_into_table() {
        let sql = "CREATE TABLE orders (id INT PRIMARY KEY, user_id INT NOT NULL);\n\
                   ALTER TABLE orders ADD CONSTRAINT fk_ou FOREIGN KEY (user_id) \
                   REFERENCES users(id) ON DELETE CASCADE;";
        let v = json_val(sql);
        let t = &v["tables"][0];
        assert_eq!(t["name"], "orders");
        let fk = &t["foreign_keys"][0];
        assert_eq!(fk["name"], "fk_ou");
        assert_eq!(fk["columns"][0], "user_id");
        assert_eq!(fk["references"]["table"], "users");
        assert_eq!(fk["references"]["columns"][0], "id");
        assert_eq!(fk["on_delete"], "CASCADE");
    }

    #[test]
    fn composite_pk_and_decimal() {
        let sql = "CREATE TABLE line_items (\n  order_id INT,\n  sku VARCHAR(32),\n  \
                   price DECIMAL(10, 2) NOT NULL,\n  PRIMARY KEY (order_id, sku)\n);";
        let v = json_val(sql);
        let t = &v["tables"][0];
        assert_eq!(t["primary_key"][0], "order_id");
        assert_eq!(t["primary_key"][1], "sku");
        let cols = t["columns"].as_array().unwrap();
        assert_eq!(cols[2]["type"], "DECIMAL(10,2)");
        assert_eq!(cols[2]["nullable"], false);
        assert_eq!(cols[0]["primary_key"], true);
        assert_eq!(cols[1]["primary_key"], true);
    }

    #[test]
    fn backtick_idents_and_dml_ignored() {
        let sql = "INSERT INTO `t` VALUES (1);\n\
                   CREATE TABLE `db`.`widgets` (`id` BIGINT UNSIGNED NOT NULL, `name` TEXT);\n\
                   SELECT 1;";
        let v = json_val(sql);
        assert_eq!(v["table_count"], 1);
        let t = &v["tables"][0];
        assert_eq!(t["name"], "widgets");
        assert_eq!(t["columns"][0]["name"], "id");
        assert_eq!(t["columns"][0]["type"], "BIGINT UNSIGNED");
    }

    #[test]
    fn create_index_and_dialect_hash_comment() {
        let sql = "# a mysql comment\nCREATE TABLE p (id INT, name TEXT);\n\
                   CREATE UNIQUE INDEX idx_name ON p (name);";
        let s = extract(sql, "json", "mysql", true, true).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        let t = &v["tables"][0];
        assert_eq!(t["indexes"][0]["name"], "idx_name");
        assert_eq!(t["indexes"][0]["unique"], true);
        assert_eq!(t["indexes"][0]["columns"][0], "name");
    }

    #[test]
    fn include_indexes_false_drops_indexes() {
        let sql = "CREATE TABLE p (id INT);\nCREATE INDEX i ON p (id);";
        let s = extract(sql, "json", "auto", true, false).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v["tables"][0].get("indexes").is_none());
    }

    #[test]
    fn apply_alter_false_ignores_alter() {
        let sql = "CREATE TABLE p (id INT);\nALTER TABLE p ADD COLUMN extra TEXT;";
        let s = extract(sql, "json", "auto", false, true).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["tables"][0]["columns"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn markdown_output_has_table_and_pk() {
        let sql = "CREATE TABLE t (id INT PRIMARY KEY, name VARCHAR(10) NOT NULL);";
        let md = extract(sql, "markdown", "auto", true, true).unwrap();
        assert!(md.contains("## t"));
        assert!(md.contains("| id | INT | NO | PK |"));
        assert!(md.contains("**Primary key:** id"));
    }

    #[test]
    fn error_no_ddl() {
        let err = extract("INSERT INTO t VALUES (1);", "json", "auto", true, true).unwrap_err();
        assert!(err.contains("no CREATE TABLE"), "got: {err}");
    }

    #[test]
    fn error_empty_input() {
        let err = extract("   ", "json", "auto", true, true).unwrap_err();
        assert!(err.contains("no SQL provided"));
    }

    #[test]
    fn error_bad_output() {
        assert!(extract("CREATE TABLE t (id INT);", "xml", "auto", true, true).is_err());
    }

    #[test]
    fn error_bad_dialect() {
        assert!(extract("CREATE TABLE t (id INT);", "json", "oracle", true, true).is_err());
    }

    #[test]
    fn quoted_semicolon_in_default_not_a_split() {
        let sql = "CREATE TABLE t (id INT, note VARCHAR(20) DEFAULT 'a;b');";
        let v = json_val(sql);
        assert_eq!(v["table_count"], 1);
        assert_eq!(v["tables"][0]["columns"][1]["default"], "'a;b'");
    }

    #[test]
    fn alter_before_create_creates_stub_then_merges() {
        let sql = "ALTER TABLE t ADD CONSTRAINT u UNIQUE (email);\n\
                   CREATE TABLE t (id INT, email VARCHAR(50));";
        let v = json_val(sql);
        assert_eq!(v["table_count"], 1);
        let t = &v["tables"][0];
        assert_eq!(t["columns"].as_array().unwrap().len(), 2);
        assert_eq!(t["unique_constraints"][0]["columns"][0], "email");
    }

    #[test]
    fn postgres_timestamptz_and_double_precision() {
        let sql = "CREATE TABLE m (\n  ts TIMESTAMP WITH TIME ZONE NOT NULL,\n  \
                   amount DOUBLE PRECISION\n);";
        let v = json_val(sql);
        let cols = v["tables"][0]["columns"].as_array().unwrap();
        assert_eq!(cols[0]["type"], "TIMESTAMP WITH TIME ZONE");
        assert_eq!(cols[1]["type"], "DOUBLE PRECISION");
    }

    #[test]
    fn inline_references_captured_as_fk() {
        let sql = "CREATE TABLE t (id INT, uid INT REFERENCES users(id) ON UPDATE RESTRICT);";
        let v = json_val(sql);
        let fk = &v["tables"][0]["foreign_keys"][0];
        assert_eq!(fk["columns"][0], "uid");
        assert_eq!(fk["references"]["table"], "users");
        assert_eq!(fk["on_update"], "RESTRICT");
    }
}
