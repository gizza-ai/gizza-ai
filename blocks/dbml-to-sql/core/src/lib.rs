//! dbml-to-sql core — compile a DBML (Database Markup Language) document into
//! `CREATE TABLE` DDL for a chosen SQL dialect. Pure compute, no wafer /
//! wasm-bindgen deps; shared by the chat skill block and the standalone page.
//!
//! The parser is a lenient structural scanner (comment stripper → top-level
//! block splitter → per-construct parsers) rather than a full DBML grammar: it
//! reads `Project`, `Table`, `TablePartial`, `Enum` and `Ref` constructs,
//! skips what it does not model (`TableGroup`, `Note`, `records`, sticky
//! notes), and never executes anything.
//!
//! Emission order is deliberately dependency-free: drops → enum types →
//! tables → indexes → foreign keys → comments. Foreign keys are trailing
//! `ALTER TABLE` statements, so circular references and table order never
//! matter.

/// Largest DBML document accepted, in bytes. Anything bigger is rejected
/// instead of being silently truncated.
pub const MAX_INPUT_BYTES: usize = 200_000;

// ---------------------------------------------------------------------------
// Dialect
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dialect {
    Postgres,
    Mysql,
    Sqlite,
    SqlServer,
    Oracle,
}

impl Dialect {
    /// Parse the `dialect` param. `auto` (or empty) means "read the `Project`
    /// block's `database_type`", which the caller resolves.
    fn parse(s: &str) -> Result<Option<Self>, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(None),
            "postgresql" | "postgres" | "pg" | "postgres_sql" => Ok(Some(Dialect::Postgres)),
            "mysql" | "mariadb" => Ok(Some(Dialect::Mysql)),
            "sqlite" | "sqlite3" => Ok(Some(Dialect::Sqlite)),
            "sqlserver" | "mssql" | "tsql" | "sql server" => Ok(Some(Dialect::SqlServer)),
            "oracle" | "plsql" => Ok(Some(Dialect::Oracle)),
            other => Err(format!(
                "invalid dialect {other:?}: expected auto, postgresql, mysql, sqlite, sqlserver, or oracle"
            )),
        }
    }

    /// Map a `Project { database_type: '…' }` value onto a dialect.
    fn from_project(s: &str) -> Option<Self> {
        Dialect::parse(s).ok().flatten()
    }

    fn quote(self, ident: &str) -> String {
        match self {
            Dialect::Mysql => format!("`{}`", ident.replace('`', "``")),
            Dialect::SqlServer => format!("[{}]", ident.replace(']', "]]")),
            _ => format!("\"{}\"", ident.replace('"', "\"\"")),
        }
    }

    /// `CREATE INDEX IF NOT EXISTS` is only accepted by these two.
    fn index_supports_if_not_exists(self) -> bool {
        matches!(self, Dialect::Postgres | Dialect::Sqlite)
    }

    /// Dialects with a real `COMMENT ON …` statement.
    fn has_comment_on(self) -> bool {
        matches!(self, Dialect::Postgres | Dialect::Oracle)
    }
}

// ---------------------------------------------------------------------------
// Parsed model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum DefaultValue {
    /// Backtick-quoted: a raw SQL expression, emitted wrapped in parens.
    Expr(String),
    /// Quoted string literal.
    Str(String),
    /// Bare token: number, `true`, `false`, `null`, or an unquoted word.
    Bare(String),
}

#[derive(Clone, Debug)]
struct Column {
    name: String,
    ty: String,
    pk: bool,
    increment: bool,
    not_null: bool,
    unique: bool,
    default: Option<DefaultValue>,
    check: Option<String>,
    note: Option<String>,
}

#[derive(Clone, Debug)]
enum IndexPart {
    Col(String),
    Expr(String),
}

#[derive(Clone, Debug)]
struct IndexDef {
    parts: Vec<IndexPart>,
    unique: bool,
    pk: bool,
    name: Option<String>,
    method: Option<String>,
}

#[derive(Clone, Debug)]
struct Table {
    schema: Option<String>,
    name: String,
    alias: Option<String>,
    note: Option<String>,
    columns: Vec<Column>,
    indexes: Vec<IndexDef>,
    checks: Vec<(String, Option<String>)>,
}

impl Table {
    fn key(&self) -> String {
        table_key(self.schema.as_deref(), &self.name)
    }
}

fn table_key(schema: Option<&str>, name: &str) -> String {
    format!("{}\u{1}{}", schema.unwrap_or(""), name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefOp {
    /// `<` — one row on the left, many on the right.
    OneToMany,
    /// `>` — many rows on the left, one on the right.
    ManyToOne,
    /// `-` — one to one.
    OneToOne,
    /// `<>` — many to many; compiled into a join table.
    ManyToMany,
}

#[derive(Clone, Debug)]
struct Endpoint {
    schema: Option<String>,
    table: String,
    cols: Vec<String>,
}

#[derive(Clone, Debug)]
struct RefDef {
    name: Option<String>,
    left: Endpoint,
    right: Endpoint,
    op: RefOp,
    on_delete: Option<String>,
    on_update: Option<String>,
}

#[derive(Clone, Debug)]
struct EnumDef {
    schema: Option<String>,
    name: String,
    values: Vec<String>,
}

#[derive(Default)]
struct Document {
    database_type: Option<String>,
    tables: Vec<Table>,
    partials: Vec<(String, Table)>,
    enums: Vec<EnumDef>,
    refs: Vec<RefDef>,
}

// ---------------------------------------------------------------------------
// Lexical helpers
// ---------------------------------------------------------------------------

/// Line number (1-based) of a byte offset, for error messages.
fn line_at(src: &str, offset: usize) -> usize {
    src[..offset.min(src.len())].matches('\n').count() + 1
}

/// If `s[i]` opens a string (`'''`, `'`, `"` or a backtick), return the index
/// just past its closing delimiter. Otherwise `None`.
fn string_end(s: &[u8], i: usize) -> Option<usize> {
    let n = s.len();
    if i >= n {
        return None;
    }
    match s[i] {
        b'\'' if s[i..].starts_with(b"'''") => {
            let mut j = i + 3;
            while j + 2 < n {
                if &s[j..j + 3] == b"'''" {
                    return Some(j + 3);
                }
                j += 1;
            }
            Some(n)
        }
        q @ (b'\'' | b'"' | b'`') => {
            let mut j = i + 1;
            while j < n {
                if s[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if s[j] == q {
                    return Some(j + 1);
                }
                j += 1;
            }
            Some(n)
        }
        _ => None,
    }
}

/// Strip `//` line and `/* … */` block comments, preserving every newline so
/// reported line numbers stay accurate and string literals stay intact.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < n {
        if let Some(end) = string_end(b, i) {
            out.push_str(&src[i..end]);
            i = end;
            continue;
        }
        if b[i] == b'/' && i + 1 < n && b[i + 1] == b'/' {
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && i + 1 < n && b[i + 1] == b'*' {
            let mut j = i + 2;
            while j + 1 < n && !(b[j] == b'*' && b[j + 1] == b'/') {
                if b[j] == b'\n' {
                    out.push('\n');
                }
                j += 1;
            }
            i = (j + 2).min(n);
            continue;
        }
        let ch_len = utf8_len(b[i]);
        out.push_str(&src[i..(i + ch_len).min(n)]);
        i += ch_len;
    }
    out
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && (b[*i] as char).is_whitespace() {
        *i += 1;
    }
}

fn skip_inline_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && (b[*i] == b' ' || b[*i] == b'\t' || b[*i] == b'\r') {
        *i += 1;
    }
}

fn read_ident(src: &str, i: &mut usize) -> String {
    let b = src.as_bytes();
    let start = *i;
    while *i < b.len() && is_ident_byte(b[*i]) {
        *i += 1;
    }
    src[start..*i].to_string()
}

/// Read a name: a quoted string or a bare identifier. Returns `None` when the
/// cursor is not on either.
fn read_name(src: &str, i: &mut usize) -> Option<String> {
    let b = src.as_bytes();
    if *i >= b.len() {
        return None;
    }
    if matches!(b[*i], b'"' | b'\'' | b'`') {
        let end = string_end(b, *i)?;
        let raw = &src[*i..end];
        *i = end;
        return Some(unquote(raw));
    }
    let ident = read_ident(src, i);
    if ident.is_empty() {
        None
    } else {
        Some(ident)
    }
}

/// Strip the surrounding quote delimiters (and unescape `\'`) from a literal.
fn unquote(raw: &str) -> String {
    let t = raw.trim();
    if let Some(inner) = t.strip_prefix("'''").and_then(|r| r.strip_suffix("'''")) {
        return dedent(inner);
    }
    for q in ['\'', '"', '`'] {
        if t.len() >= 2 && t.starts_with(q) && t.ends_with(q) {
            let inner = &t[1..t.len() - 1];
            return inner.replace(&format!("\\{q}"), &q.to_string());
        }
    }
    t.to_string()
}

/// Trim the common leading indentation of a `'''…'''` block and its blank
/// first/last lines, so multi-line notes render sensibly.
fn dedent(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let body: Vec<&str> = lines
        .iter()
        .skip_while(|l| l.trim().is_empty())
        .copied()
        .collect();
    let indent = body
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut out: Vec<String> = body
        .iter()
        .map(|l| {
            if l.len() >= indent {
                l[indent..].to_string()
            } else {
                l.trim_start().to_string()
            }
        })
        .collect();
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Split on `sep` at the top level — ignoring separators inside strings,
/// parens, brackets and braces.
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let b = s.as_bytes();
    let n = b.len();
    let (mut out, mut start, mut depth, mut i) = (Vec::new(), 0usize, 0i32, 0usize);
    while i < n {
        if let Some(end) = string_end(b, i) {
            i = end;
            continue;
        }
        match b[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            c if depth == 0 && c == sep as u8 => {
                out.push(s[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(s[start..].to_string());
    out
}

/// From an opening `open` byte at `*i`, return the body between the delimiters
/// and leave `*i` just past the closer.
fn read_balanced(src: &str, i: &mut usize, open: u8, close: u8) -> Result<String, String> {
    let b = src.as_bytes();
    if *i >= b.len() || b[*i] != open {
        return Err(format!(
            "expected `{}` at line {}, found {}",
            open as char,
            line_at(src, *i),
            b.get(*i)
                .map(|c| format!("`{}`", *c as char))
                .unwrap_or_else(|| "end of input".into())
        ));
    }
    let start = *i + 1;
    let mut depth = 0i32;
    let mut j = *i;
    while j < b.len() {
        if let Some(end) = string_end(b, j) {
            j = end;
            continue;
        }
        if b[j] == open {
            depth += 1;
        } else if b[j] == close {
            depth -= 1;
            if depth == 0 {
                *i = j + 1;
                return Ok(src[start..j].to_string());
            }
        }
        j += 1;
    }
    Err(format!(
        "unclosed `{}` opened at line {}",
        open as char,
        line_at(src, *i)
    ))
}

/// Advance to the next `\n` (exclusive) and return the text skipped.
fn read_line(src: &str, i: &mut usize) -> String {
    let b = src.as_bytes();
    let start = *i;
    let mut j = *i;
    while j < b.len() {
        if let Some(end) = string_end(b, j) {
            j = end;
            continue;
        }
        if b[j] == b'\n' {
            break;
        }
        j += 1;
    }
    *i = j;
    src[start..j].to_string()
}

/// Find the first top-level occurrence of `needle`, skipping strings and any
/// bracketed/parenthesised span.
fn find_top_level(s: &str, needle: u8) -> Option<usize> {
    let b = s.as_bytes();
    let (mut depth, mut i) = (0i32, 0usize);
    while i < b.len() {
        if let Some(end) = string_end(b, i) {
            i = end;
            continue;
        }
        // The needle is tested FIRST so an opening bracket can itself be the
        // needle (`find_top_level(s, b'[')` must return the `[`, not descend
        // into it).
        if depth == 0 && b[i] == needle {
            return Some(i);
        }
        match b[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Settings (`[ … ]`) parsing
// ---------------------------------------------------------------------------

/// One `[ … ]` entry: either a bare flag or a `key: value` pair.
struct Setting {
    key: String,
    value: Option<String>,
}

fn parse_settings(raw: &str) -> Vec<Setting> {
    split_top_level(raw, ',')
        .into_iter()
        .filter_map(|item| {
            let item = item.trim();
            if item.is_empty() {
                return None;
            }
            match find_top_level(item, b':') {
                Some(pos) => Some(Setting {
                    key: normalize_key(&item[..pos]),
                    value: Some(item[pos + 1..].trim().to_string()),
                }),
                None => Some(Setting {
                    key: normalize_key(item),
                    value: None,
                }),
            }
        })
        .collect()
}

/// Lowercase and collapse internal whitespace, so `not  null` == `not null`.
fn normalize_key(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn parse_default(raw: &str) -> DefaultValue {
    let t = raw.trim();
    if t.starts_with('`') && t.ends_with('`') && t.len() >= 2 {
        return DefaultValue::Expr(unquote(t));
    }
    if t.starts_with('\'') || t.starts_with('"') {
        return DefaultValue::Str(unquote(t));
    }
    DefaultValue::Bare(t.to_string())
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

fn parse_document(src: &str) -> Result<Document, String> {
    let mut doc = Document::default();
    let b = src.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        skip_ws(b, &mut i);
        if i >= b.len() {
            break;
        }
        let word_start = i;
        let word = read_ident(src, &mut i);
        if word.is_empty() {
            // Not a keyword — skip the rest of the line rather than failing on
            // stray punctuation.
            read_line(src, &mut i);
            continue;
        }
        match word.to_ascii_lowercase().as_str() {
            "project" => {
                let header_end = seek_brace(src, &mut i, word_start, "Project")?;
                let _ = header_end;
                let body = read_balanced(src, &mut i, b'{', b'}')?;
                if let Some(v) = project_database_type(&body) {
                    doc.database_type = Some(v);
                }
            }
            "table" | "tablepartial" => {
                let is_partial = word.eq_ignore_ascii_case("tablepartial");
                let header_start = i;
                seek_brace(src, &mut i, word_start, if is_partial { "TablePartial" } else { "Table" })?;
                let header = src[header_start..i].to_string();
                let body = read_balanced(src, &mut i, b'{', b'}')?;
                let (mut table, partial_refs) = parse_table(&header, &body, src, header_start)?;
                if is_partial {
                    let name = table.name.clone();
                    table.schema = None;
                    doc.partials.push((name, table));
                } else {
                    doc.tables.push(table);
                    // Record which partials this table pulls in; expansion
                    // happens after the whole document is parsed so a partial
                    // may be declared after its user.
                    PENDING_PARTIALS.with(|p| {
                        p.borrow_mut()
                            .push((doc.tables.len() - 1, partial_refs.clone()))
                    });
                }
            }
            "enum" => {
                let header_start = i;
                seek_brace(src, &mut i, word_start, "Enum")?;
                let header = src[header_start..i].to_string();
                let body = read_balanced(src, &mut i, b'{', b'}')?;
                doc.enums.push(parse_enum(&header, &body)?);
            }
            "ref" => {
                parse_ref_construct(src, &mut i, &mut doc.refs)?;
            }
            // Constructs DBML defines but SQL DDL has no place for.
            "tablegroup" | "note" | "records" | "stickynote" | "import" | "sticky" => {
                skip_construct(src, &mut i);
            }
            _ => {
                skip_construct(src, &mut i);
            }
        }
    }
    Ok(doc)
}

// A table's `~partial` references, collected during parsing and applied once
// every `TablePartial` has been seen.
thread_local! {
    static PENDING_PARTIALS: std::cell::RefCell<Vec<(usize, Vec<String>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Advance `*i` to the next top-level `{`, erroring with the construct name if
/// there isn't one.
fn seek_brace(src: &str, i: &mut usize, start: usize, what: &str) -> Result<usize, String> {
    let b = src.as_bytes();
    let mut j = *i;
    while j < b.len() {
        if let Some(end) = string_end(b, j) {
            j = end;
            continue;
        }
        if b[j] == b'{' {
            *i = j;
            return Ok(j);
        }
        if b[j] == b'\n' && src[*i..j].trim().is_empty() && j > *i + 200 {
            break;
        }
        j += 1;
    }
    Err(format!(
        "{what} block at line {} has no opening `{{`",
        line_at(src, start)
    ))
}

/// Skip an unmodelled construct: its header plus a `{ … }` body, a quoted
/// value, or just the rest of the line.
fn skip_construct(src: &str, i: &mut usize) {
    let b = src.as_bytes();
    let mut j = *i;
    while j < b.len() {
        if let Some(end) = string_end(b, j) {
            j = end;
            continue;
        }
        match b[j] {
            b'{' => {
                *i = j;
                let _ = read_balanced(src, i, b'{', b'}');
                return;
            }
            b'\n' => break,
            _ => j += 1,
        }
    }
    *i = j;
    read_line(src, i);
}

fn project_database_type(body: &str) -> Option<String> {
    for line in body.lines() {
        let line = line.trim();
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("database_type") {
            if let Some(stripped) = rest.trim_start().strip_prefix(':') {
                let raw = &line[line.len() - stripped.len()..];
                return Some(unquote(raw.trim()));
            }
        }
    }
    None
}

/// `Table [schema.]name [as alias] [ [settings] ]`
fn parse_table(
    header: &str,
    body: &str,
    src: &str,
    header_start: usize,
) -> Result<(Table, Vec<String>), String> {
    let mut i = 0usize;
    skip_ws(header.as_bytes(), &mut i);
    let first = read_name(header, &mut i).ok_or_else(|| {
        format!(
            "Table block at line {} has no name",
            line_at(src, header_start)
        )
    })?;
    // Optional `schema.name`.
    let (schema, name) = if header.as_bytes().get(i) == Some(&b'.') {
        i += 1;
        let second = read_name(header, &mut i).ok_or_else(|| {
            format!(
                "Table `{first}.` at line {} is missing the table name after the schema",
                line_at(src, header_start)
            )
        })?;
        (Some(first), second)
    } else {
        (None, first)
    };

    let mut alias = None;
    let mut settings_raw = String::new();
    loop {
        skip_ws(header.as_bytes(), &mut i);
        if i >= header.len() {
            break;
        }
        if header.as_bytes()[i] == b'[' {
            settings_raw = read_balanced(header, &mut i, b'[', b']')?;
            continue;
        }
        let save = i;
        let word = read_name(header, &mut i);
        match word {
            Some(w) if w.eq_ignore_ascii_case("as") => {
                skip_ws(header.as_bytes(), &mut i);
                alias = read_name(header, &mut i);
            }
            Some(_) => {}
            None => {
                i = save + 1;
            }
        }
    }

    let mut table = Table {
        schema,
        name,
        alias,
        note: None,
        columns: Vec::new(),
        indexes: Vec::new(),
        checks: Vec::new(),
    };
    for s in parse_settings(&settings_raw) {
        if s.key == "note" {
            table.note = s.value.as_deref().map(unquote);
        }
    }
    let partial_refs = parse_table_body(body, &mut table, src, header_start)?;
    Ok((table, partial_refs))
}

/// Parse a table body: columns, an `indexes { }` block, a `checks { }` block,
/// a `Note:` line, and `~partial` expansions. Inline `[ref: …]` settings are
/// pushed onto the document's ref list by the caller via `INLINE_REFS`.
fn parse_table_body(
    body: &str,
    table: &mut Table,
    src: &str,
    header_start: usize,
) -> Result<Vec<String>, String> {
    let b = body.as_bytes();
    let mut i = 0usize;
    let mut partials = Vec::new();
    while i < b.len() {
        skip_ws(b, &mut i);
        if i >= b.len() {
            break;
        }
        if b[i] == b'~' {
            i += 1;
            skip_ws(b, &mut i);
            if let Some(p) = read_name(body, &mut i) {
                partials.push(p);
            }
            continue;
        }
        let save = i;
        let word = read_ident(body, &mut i).to_ascii_lowercase();
        let mut peek = i;
        skip_ws(b, &mut peek);
        let next = b.get(peek).copied();

        if word == "indexes" && next == Some(b'{') {
            i = peek;
            let block = read_balanced(body, &mut i, b'{', b'}')?;
            parse_indexes(&block, table)?;
            continue;
        }
        if word == "checks" && next == Some(b'{') {
            i = peek;
            let block = read_balanced(body, &mut i, b'{', b'}')?;
            parse_checks(&block, table);
            continue;
        }
        if word == "note" && next == Some(b':') {
            i = peek + 1;
            skip_inline_ws(b, &mut i);
            table.note = Some(read_value_token(body, &mut i));
            continue;
        }
        if word == "note" && next == Some(b'{') {
            i = peek;
            let block = read_balanced(body, &mut i, b'{', b'}')?;
            table.note = Some(unquote(block.trim()));
            continue;
        }

        i = save;
        parse_column_line(body, &mut i, table, src, header_start)?;
    }
    Ok(partials)
}

/// Read a value token: a quoted literal, or a bare run up to the end of line.
fn read_value_token(src: &str, i: &mut usize) -> String {
    let b = src.as_bytes();
    if let Some(end) = string_end(b, *i) {
        let raw = &src[*i..end];
        *i = end;
        return unquote(raw);
    }
    read_line(src, i).trim().to_string()
}

const TYPE_CONTINUATIONS: [&str; 7] = [
    "precision",
    "varying",
    "zone",
    "with",
    "without",
    "time",
    "character",
];

fn parse_column_line(
    body: &str,
    i: &mut usize,
    table: &mut Table,
    src: &str,
    header_start: usize,
) -> Result<(), String> {
    let b = body.as_bytes();
    let start = *i;
    let name = read_name(body, i).ok_or_else(|| {
        format!(
            "could not read a column name in table `{}` (block starting at line {})",
            table.name,
            line_at(src, header_start)
        )
    })?;
    skip_inline_ws(b, i);
    let mut ty = read_type_token(body, i).ok_or_else(|| {
        format!(
            "column `{}` in table `{}` has no type — expected e.g. `{} varchar(255)` (block starting at line {})",
            name,
            table.name,
            name,
            line_at(src, header_start)
        )
    })?;
    // Multi-word SQL types (`double precision`, `timestamp with time zone`).
    loop {
        let save = *i;
        skip_inline_ws(b, i);
        let peek = *i;
        let word = read_ident(body, i);
        if !word.is_empty() && TYPE_CONTINUATIONS.contains(&word.to_ascii_lowercase().as_str()) {
            ty.push(' ');
            ty.push_str(&word);
            // A trailing `(n)` may follow the final word.
            if b.get(*i) == Some(&b'(') {
                let args = read_balanced(body, i, b'(', b')')?;
                ty.push('(');
                ty.push_str(args.trim());
                ty.push(')');
            }
            continue;
        }
        *i = save;
        let _ = peek;
        break;
    }

    let mut col = Column {
        name,
        ty,
        pk: false,
        increment: false,
        not_null: false,
        unique: false,
        default: None,
        check: None,
        note: None,
    };

    skip_inline_ws(b, i);
    if b.get(*i) == Some(&b'[') {
        let raw = read_balanced(body, i, b'[', b']')?;
        for s in parse_settings(&raw) {
            match s.key.as_str() {
                "pk" | "primary key" => col.pk = true,
                "increment" => col.increment = true,
                "not null" => col.not_null = true,
                "null" => col.not_null = false,
                "unique" => col.unique = true,
                "default" => col.default = s.value.as_deref().map(parse_default),
                "check" => col.check = s.value.as_deref().map(unquote),
                "note" => col.note = s.value.as_deref().map(unquote),
                "ref" => {
                    if let Some(v) = s.value.as_deref() {
                        let owner = Endpoint {
                            schema: table.schema.clone(),
                            table: table.name.clone(),
                            cols: vec![col.name.clone()],
                        };
                        let r = parse_inline_ref(owner, v).map_err(|e| {
                            format!(
                                "inline ref on column `{}` of table `{}`: {e}",
                                col.name, table.name
                            )
                        })?;
                        INLINE_REFS.with(|c| c.borrow_mut().push(r));
                    }
                }
                _ => {}
            }
        }
    }
    let _ = start;
    table.columns.push(col);
    Ok(())
}

thread_local! {
    static INLINE_REFS: std::cell::RefCell<Vec<RefDef>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Read a type token: `varchar(255)`, `decimal(10,2)`, `int[]`, `"my type"`,
/// or a schema-qualified `public.mood`.
fn read_type_token(src: &str, i: &mut usize) -> Option<String> {
    let b = src.as_bytes();
    if *i >= b.len() {
        return None;
    }
    let mut out = String::new();
    if matches!(b[*i], b'"' | b'\'' | b'`') {
        let end = string_end(b, *i)?;
        out.push_str(&unquote(&src[*i..end]));
        *i = end;
    } else {
        let start = *i;
        while *i < b.len() && (is_ident_byte(b[*i]) || b[*i] == b'.') {
            *i += 1;
        }
        if *i == start {
            return None;
        }
        out.push_str(&src[start..*i]);
    }
    if b.get(*i) == Some(&b'(') {
        let mut j = *i;
        if let Ok(args) = read_balanced(src, &mut j, b'(', b')') {
            out.push('(');
            out.push_str(&args.split_whitespace().collect::<Vec<_>>().join(""));
            out.push(')');
            *i = j;
        }
    }
    while b.get(*i) == Some(&b'[') && b.get(*i + 1) == Some(&b']') {
        out.push_str("[]");
        *i += 2;
    }
    Some(out)
}

fn parse_indexes(block: &str, table: &mut Table) -> Result<(), String> {
    let b = block.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        skip_ws(b, &mut i);
        if i >= b.len() {
            break;
        }
        let mut parts = Vec::new();
        if b[i] == b'(' {
            let inner = read_balanced(block, &mut i, b'(', b')')?;
            for p in split_top_level(&inner, ',') {
                let p = p.trim();
                if p.is_empty() {
                    continue;
                }
                parts.push(index_part(p));
            }
        } else if b[i] == b'`' {
            let end = string_end(b, i).unwrap_or(b.len());
            parts.push(IndexPart::Expr(unquote(&block[i..end])));
            i = end;
        } else {
            match read_name(block, &mut i) {
                Some(n) => parts.push(IndexPart::Col(n)),
                None => {
                    read_line(block, &mut i);
                    continue;
                }
            }
        }
        let mut idx = IndexDef {
            parts,
            unique: false,
            pk: false,
            name: None,
            method: None,
        };
        skip_inline_ws(b, &mut i);
        if b.get(i) == Some(&b'[') {
            let raw = read_balanced(block, &mut i, b'[', b']')?;
            for s in parse_settings(&raw) {
                match s.key.as_str() {
                    "unique" => idx.unique = true,
                    "pk" | "primary key" => idx.pk = true,
                    "name" => idx.name = s.value.as_deref().map(unquote),
                    "type" => idx.method = s.value.as_deref().map(|v| unquote(v).to_lowercase()),
                    _ => {}
                }
            }
        }
        if !idx.parts.is_empty() {
            table.indexes.push(idx);
        }
    }
    Ok(())
}

fn index_part(p: &str) -> IndexPart {
    if p.starts_with('`') {
        IndexPart::Expr(unquote(p))
    } else {
        IndexPart::Col(unquote(p))
    }
}

fn parse_checks(block: &str, table: &mut Table) {
    let b = block.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        skip_ws(b, &mut i);
        if i >= b.len() {
            break;
        }
        let expr = if b[i] == b'`' {
            let end = string_end(b, i).unwrap_or(b.len());
            let e = unquote(&block[i..end]);
            i = end;
            e
        } else {
            let line = read_line(block, &mut i);
            line.trim().to_string()
        };
        let mut name = None;
        skip_inline_ws(b, &mut i);
        if b.get(i) == Some(&b'[') {
            if let Ok(raw) = read_balanced(block, &mut i, b'[', b']') {
                for s in parse_settings(&raw) {
                    if s.key == "name" {
                        name = s.value.as_deref().map(unquote);
                    }
                }
            }
        }
        if !expr.is_empty() {
            table.checks.push((expr, name));
        }
    }
}

fn parse_enum(header: &str, body: &str) -> Result<EnumDef, String> {
    let mut i = 0usize;
    skip_ws(header.as_bytes(), &mut i);
    let first = read_name(header, &mut i)
        .ok_or_else(|| "Enum block has no name — expected `Enum status { … }`".to_string())?;
    let (schema, name) = if header.as_bytes().get(i) == Some(&b'.') {
        i += 1;
        let second = read_name(header, &mut i)
            .ok_or_else(|| format!("Enum `{first}.` is missing the enum name after the schema"))?;
        (Some(first), second)
    } else {
        (None, first)
    };
    let b = body.as_bytes();
    let mut j = 0usize;
    let mut values = Vec::new();
    while j < b.len() {
        skip_ws(b, &mut j);
        if j >= b.len() {
            break;
        }
        match read_name(body, &mut j) {
            Some(v) => {
                skip_inline_ws(b, &mut j);
                if b.get(j) == Some(&b'[') {
                    let _ = read_balanced(body, &mut j, b'[', b']');
                }
                values.push(v);
            }
            None => {
                read_line(body, &mut j);
            }
        }
    }
    if values.is_empty() {
        return Err(format!("enum `{name}` has no values"));
    }
    Ok(EnumDef {
        schema,
        name,
        values,
    })
}

/// `Ref [name] : a.b > c.d [settings]` or `Ref [name] { … }`.
fn parse_ref_construct(src: &str, i: &mut usize, out: &mut Vec<RefDef>) -> Result<(), String> {
    let b = src.as_bytes();
    let start = *i;
    // Everything up to the first top-level `:` or `{` is the (optional) name.
    let mut j = *i;
    let mut mode = None;
    while j < b.len() {
        if let Some(end) = string_end(b, j) {
            j = end;
            continue;
        }
        match b[j] {
            b':' => {
                mode = Some(b':');
                break;
            }
            b'{' => {
                mode = Some(b'{');
                break;
            }
            b'\n' => break,
            _ => j += 1,
        }
    }
    let name_raw = src[*i..j].trim().to_string();
    let name = if name_raw.is_empty() {
        None
    } else {
        Some(unquote(&name_raw))
    };
    match mode {
        Some(b':') => {
            *i = j + 1;
            let line = read_line(src, i);
            let r = parse_ref_body(&line, name, src, start)?;
            out.push(r);
        }
        Some(b'{') => {
            *i = j;
            let block = read_balanced(src, i, b'{', b'}')?;
            for line in block.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                // A long-form block may still carry a leading `name:`.
                let body = match find_top_level(line, b':') {
                    Some(p) if !line[..p].contains(['<', '>', '-']) => &line[p + 1..],
                    _ => line,
                };
                out.push(parse_ref_body(body, name.clone(), src, start)?);
            }
        }
        // No `:` and no `{` before the end of the line — nothing to parse.
        _ => {
            read_line(src, i);
        }
    }
    Ok(())
}

/// Parse `users.id < posts.user_id [delete: cascade]`.
fn parse_ref_body(
    body: &str,
    name: Option<String>,
    src: &str,
    at: usize,
) -> Result<RefDef, String> {
    let mut rest = body.trim().to_string();
    let (mut on_delete, mut on_update) = (None, None);
    if let Some(open) = find_top_level(&rest, b'[') {
        let mut k = open;
        let settings = read_balanced(&rest.clone(), &mut k, b'[', b']')?;
        for s in parse_settings(&settings) {
            match s.key.as_str() {
                "delete" => on_delete = s.value.as_deref().map(referential_action),
                "update" => on_update = s.value.as_deref().map(referential_action),
                _ => {}
            }
        }
        rest = rest[..open].trim().to_string();
    }
    let (op, lo, hi) = find_ref_op(&rest).ok_or_else(|| {
        format!(
            "Ref at line {} has no relationship operator — expected one of `<` `>` `-` `<>`, as in `Ref: posts.user_id > users.id`",
            line_at(src, at)
        )
    })?;
    let left = parse_endpoint(rest[..lo].trim(), src, at)?;
    let right = parse_endpoint(rest[hi..].trim(), src, at)?;
    Ok(RefDef {
        name,
        left,
        right,
        op,
        on_delete,
        on_update,
    })
}

/// Locate the relationship operator outside strings/parens; returns the op and
/// its byte span. A `?` cardinality marker (`>?`, `?>`) is absorbed.
fn find_ref_op(s: &str) -> Option<(RefOp, usize, usize)> {
    let b = s.as_bytes();
    let (mut depth, mut i) = (0i32, 0usize);
    while i < b.len() {
        if let Some(end) = string_end(b, i) {
            i = end;
            continue;
        }
        match b[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            _ if depth != 0 => {}
            b'<' if b.get(i + 1) == Some(&b'>') => {
                return Some((RefOp::ManyToMany, trim_marker(s, i), i + 2))
            }
            b'<' => return Some((RefOp::OneToMany, trim_marker(s, i), skip_marker(b, i + 1))),
            b'>' => return Some((RefOp::ManyToOne, trim_marker(s, i), skip_marker(b, i + 1))),
            b'-' => return Some((RefOp::OneToOne, trim_marker(s, i), skip_marker(b, i + 1))),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Drop a `?` that sits immediately before the operator (`users.id ?< …`).
fn trim_marker(s: &str, op_at: usize) -> usize {
    let b = s.as_bytes();
    let mut k = op_at;
    while k > 0 && (b[k - 1] == b'?' || b[k - 1] == b' ') {
        k -= 1;
    }
    if s[k..op_at].trim().is_empty() {
        k
    } else {
        op_at
    }
}

fn skip_marker(b: &[u8], mut i: usize) -> usize {
    while b.get(i) == Some(&b'?') {
        i += 1;
    }
    i
}

fn referential_action(v: &str) -> String {
    let n = normalize_key(&unquote(v));
    match n.as_str() {
        "cascade" => "CASCADE".into(),
        "restrict" => "RESTRICT".into(),
        "set null" => "SET NULL".into(),
        "set default" => "SET DEFAULT".into(),
        "no action" => "NO ACTION".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// `[schema.]table.col` or `[schema.]table.(col1, col2)`.
fn parse_endpoint(s: &str, src: &str, at: usize) -> Result<Endpoint, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err(format!(
            "Ref at line {} is missing one side — expected `table.column` on both sides of the operator",
            line_at(src, at)
        ));
    }
    let mut segments: Vec<String> = Vec::new();
    let b = s.as_bytes();
    let mut i = 0usize;
    let mut cur_start = 0usize;
    let mut depth = 0i32;
    while i < b.len() {
        if let Some(end) = string_end(b, i) {
            i = end;
            continue;
        }
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'.' if depth == 0 => {
                segments.push(s[cur_start..i].to_string());
                cur_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    segments.push(s[cur_start..].to_string());
    if segments.len() < 2 {
        return Err(format!(
            "Ref endpoint `{s}` at line {} is not `table.column` — qualify the column with its table",
            line_at(src, at)
        ));
    }
    let col_part = segments.pop().expect("len >= 2");
    let cols = parse_col_list(&col_part);
    if cols.is_empty() {
        return Err(format!(
            "Ref endpoint `{s}` at line {} names no column",
            line_at(src, at)
        ));
    }
    let table = unquote(&segments.pop().expect("len >= 2"));
    let schema = segments.pop().map(|s| unquote(&s));
    Ok(Endpoint {
        schema,
        table,
        cols,
    })
}

fn parse_col_list(part: &str) -> Vec<String> {
    let t = part.trim();
    if let Some(inner) = t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        split_top_level(inner, ',')
            .into_iter()
            .map(|c| unquote(c.trim()))
            .filter(|c| !c.is_empty())
            .collect()
    } else if t.is_empty() {
        Vec::new()
    } else {
        vec![unquote(t)]
    }
}

fn parse_inline_ref(owner: Endpoint, value: &str) -> Result<RefDef, String> {
    let v = value.trim();
    let (op, _, hi) = find_ref_op(v).ok_or_else(|| {
        "expected a relationship operator — e.g. `[ref: > users.id]`".to_string()
    })?;
    let target = parse_endpoint(v[hi..].trim(), v, 0)?;
    Ok(RefDef {
        name: None,
        left: owner,
        right: target,
        op,
        on_delete: None,
        on_update: None,
    })
}

// ---------------------------------------------------------------------------
// Type mapping
// ---------------------------------------------------------------------------

/// Split `varchar(255)` into `("varchar", Some("255"))`.
fn split_type(ty: &str) -> (String, Option<String>, bool) {
    let array = ty.trim_end().ends_with("[]");
    let base = ty.trim_end().trim_end_matches("[]").trim();
    match base.find('(') {
        Some(p) if base.ends_with(')') => (
            base[..p].trim().to_ascii_lowercase(),
            Some(base[p + 1..base.len() - 1].trim().to_string()),
            array,
        ),
        _ => (base.to_ascii_lowercase(), None, array),
    }
}

/// Map a DBML column type onto the dialect's own spelling. Unknown types pass
/// through verbatim (with their arguments) so native types keep working.
fn map_type(ty: &str, d: Dialect) -> String {
    let (base, args, array) = split_type(ty);
    let a = args.as_deref();
    let with = |t: &str| -> String {
        match a {
            Some(x) => format!("{t}({x})"),
            None => t.to_string(),
        }
    };
    let mapped: String = match (base.as_str(), d) {
        // Integers
        ("int" | "integer" | "int4", Dialect::Postgres) => "INTEGER".into(),
        ("int" | "integer" | "int4", Dialect::Mysql | Dialect::SqlServer) => "INT".into(),
        ("int" | "integer" | "int4", Dialect::Sqlite) => "INTEGER".into(),
        ("int" | "integer" | "int4", Dialect::Oracle) => "NUMBER(10)".into(),
        ("smallint" | "int2", Dialect::Sqlite) => "INTEGER".into(),
        ("smallint" | "int2", Dialect::Oracle) => "NUMBER(5)".into(),
        ("smallint" | "int2", _) => "SMALLINT".into(),
        ("bigint" | "int8", Dialect::Oracle) => "NUMBER(19)".into(),
        ("bigint" | "int8", _) => "BIGINT".into(),
        ("tinyint", Dialect::Postgres | Dialect::Sqlite) => "SMALLINT".into(),
        ("tinyint", Dialect::Oracle) => "NUMBER(3)".into(),
        ("tinyint", _) => with("TINYINT"),
        // Serial types imply auto-increment; the caller also sets `increment`.
        ("serial" | "bigserial" | "smallserial", Dialect::Postgres) => base.to_uppercase(),
        ("serial" | "smallserial", Dialect::Mysql | Dialect::SqlServer) => "INT".into(),
        ("serial" | "smallserial", Dialect::Sqlite) => "INTEGER".into(),
        ("serial" | "smallserial", Dialect::Oracle) => "NUMBER(10)".into(),
        ("bigserial", Dialect::Oracle) => "NUMBER(19)".into(),
        ("bigserial", _) => "BIGINT".into(),
        // Strings
        ("varchar" | "character varying", Dialect::Sqlite) => "TEXT".into(),
        ("varchar" | "character varying", Dialect::SqlServer) => match a {
            Some(x) => format!("NVARCHAR({x})"),
            None => "NVARCHAR(255)".into(),
        },
        ("varchar" | "character varying", Dialect::Oracle) => match a {
            Some(x) => format!("VARCHAR2({x})"),
            None => "VARCHAR2(255)".into(),
        },
        ("varchar" | "character varying", Dialect::Mysql) => match a {
            Some(x) => format!("VARCHAR({x})"),
            None => "VARCHAR(255)".into(),
        },
        ("varchar" | "character varying", Dialect::Postgres) => with("VARCHAR"),
        ("char" | "character", Dialect::Sqlite) => "TEXT".into(),
        ("char" | "character", Dialect::SqlServer) => with("NCHAR"),
        ("char" | "character", _) => with("CHAR"),
        ("text" | "longtext" | "mediumtext" | "clob", Dialect::SqlServer) => "NVARCHAR(MAX)".into(),
        ("text" | "longtext" | "mediumtext" | "clob", Dialect::Oracle) => "CLOB".into(),
        ("text" | "longtext" | "mediumtext" | "clob", _) => "TEXT".into(),
        // Booleans
        ("bool" | "boolean", Dialect::Mysql) => "TINYINT(1)".into(),
        ("bool" | "boolean", Dialect::SqlServer) => "BIT".into(),
        ("bool" | "boolean", Dialect::Oracle) => "NUMBER(1)".into(),
        ("bool" | "boolean", _) => "BOOLEAN".into(),
        // Floats
        ("real" | "float4", Dialect::Mysql) => "FLOAT".into(),
        ("real" | "float4", Dialect::Oracle) => "BINARY_FLOAT".into(),
        ("real" | "float4", _) => "REAL".into(),
        ("float" | "double" | "double precision" | "float8", Dialect::Postgres) => {
            "DOUBLE PRECISION".into()
        }
        ("float" | "double" | "double precision" | "float8", Dialect::Mysql) => "DOUBLE".into(),
        ("float" | "double" | "double precision" | "float8", Dialect::Sqlite) => "REAL".into(),
        ("float" | "double" | "double precision" | "float8", Dialect::SqlServer) => {
            "FLOAT(53)".into()
        }
        ("float" | "double" | "double precision" | "float8", Dialect::Oracle) => {
            "BINARY_DOUBLE".into()
        }
        ("decimal" | "numeric" | "money", Dialect::Oracle) => match a {
            Some(x) => format!("NUMBER({x})"),
            None => "NUMBER".into(),
        },
        ("decimal" | "numeric" | "money", Dialect::Postgres | Dialect::Sqlite) => with("NUMERIC"),
        ("decimal" | "numeric" | "money", _) => with("DECIMAL"),
        // Dates and times
        ("date", _) => "DATE".into(),
        ("time", Dialect::Sqlite) => "TEXT".into(),
        ("time", Dialect::Oracle) => "TIMESTAMP".into(),
        ("time", _) => with("TIME"),
        ("timestamp" | "datetime", Dialect::Postgres) => with("TIMESTAMP"),
        ("timestamp" | "datetime", Dialect::Mysql) => with("DATETIME"),
        ("timestamp" | "datetime", Dialect::Sqlite) => "DATETIME".into(),
        ("timestamp" | "datetime", Dialect::SqlServer) => "DATETIME2".into(),
        ("timestamp" | "datetime", Dialect::Oracle) => "TIMESTAMP".into(),
        ("timestamptz" | "timestamp with time zone" | "datetimeoffset", Dialect::Postgres) => {
            "TIMESTAMPTZ".into()
        }
        ("timestamptz" | "timestamp with time zone" | "datetimeoffset", Dialect::Mysql) => {
            "DATETIME".into()
        }
        ("timestamptz" | "timestamp with time zone" | "datetimeoffset", Dialect::Sqlite) => {
            "DATETIME".into()
        }
        ("timestamptz" | "timestamp with time zone" | "datetimeoffset", Dialect::SqlServer) => {
            "DATETIMEOFFSET".into()
        }
        ("timestamptz" | "timestamp with time zone" | "datetimeoffset", Dialect::Oracle) => {
            "TIMESTAMP WITH TIME ZONE".into()
        }
        // Semi-structured / binary
        ("json", Dialect::Postgres) => "JSON".into(),
        ("jsonb", Dialect::Postgres) => "JSONB".into(),
        ("json" | "jsonb", Dialect::Mysql) => "JSON".into(),
        ("json" | "jsonb", Dialect::Sqlite) => "TEXT".into(),
        ("json" | "jsonb", Dialect::SqlServer) => "NVARCHAR(MAX)".into(),
        ("json" | "jsonb", Dialect::Oracle) => "CLOB".into(),
        ("uuid", Dialect::Postgres) => "UUID".into(),
        ("uuid", Dialect::Mysql) => "CHAR(36)".into(),
        ("uuid", Dialect::Sqlite) => "TEXT".into(),
        ("uuid", Dialect::SqlServer) => "UNIQUEIDENTIFIER".into(),
        ("uuid", Dialect::Oracle) => "RAW(16)".into(),
        ("blob" | "bytea" | "binary" | "varbinary" | "longblob", Dialect::Postgres) => {
            "BYTEA".into()
        }
        ("blob" | "bytea" | "binary" | "varbinary" | "longblob", Dialect::Mysql) => {
            "LONGBLOB".into()
        }
        ("blob" | "bytea" | "binary" | "varbinary" | "longblob", Dialect::Sqlite) => "BLOB".into(),
        ("blob" | "bytea" | "binary" | "varbinary" | "longblob", Dialect::SqlServer) => {
            "VARBINARY(MAX)".into()
        }
        ("blob" | "bytea" | "binary" | "varbinary" | "longblob", Dialect::Oracle) => "BLOB".into(),
        // Anything else: pass through exactly as written.
        _ => ty.trim_end().trim_end_matches("[]").trim().to_string(),
    };
    // Only PostgreSQL has real array columns; elsewhere the element type stands
    // in (documented on the page).
    if array && d == Dialect::Postgres {
        format!("{mapped}[]")
    } else {
        mapped
    }
}

/// `serial`-family types carry auto-increment in the type name itself.
fn type_is_serial(ty: &str) -> bool {
    matches!(
        split_type(ty).0.as_str(),
        "serial" | "bigserial" | "smallserial"
    )
}

fn is_bigint(ty: &str) -> bool {
    matches!(split_type(ty).0.as_str(), "bigint" | "int8" | "bigserial")
}

fn is_smallint(ty: &str) -> bool {
    matches!(
        split_type(ty).0.as_str(),
        "smallint" | "int2" | "smallserial"
    )
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

struct Emitter {
    d: Dialect,
    quote: bool,
    foreign_keys: bool,
    indexes: bool,
    comments: bool,
    if_not_exists: bool,
}

impl Emitter {
    fn id(&self, name: &str) -> String {
        if self.quote {
            self.d.quote(name)
        } else {
            name.to_string()
        }
    }

    fn qualified(&self, schema: Option<&str>, name: &str) -> String {
        match schema {
            Some(s) if !s.is_empty() => format!("{}.{}", self.id(s), self.id(name)),
            _ => self.id(name),
        }
    }

    fn table_ref(&self, t: &Table) -> String {
        self.qualified(t.schema.as_deref(), &t.name)
    }
}

fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// `NUMBER(10)` → whether the enum values fit an inline `ENUM(...)` list.
fn enum_values_sql(values: &[String]) -> String {
    values
        .iter()
        .map(|v| sql_string(v))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Compile a DBML document into SQL DDL.
///
/// - `dialect`: `auto` | `postgresql` | `mysql` | `sqlite` | `sqlserver` | `oracle`.
/// - `foreign_keys` / `indexes` / `comments`: emit those statement groups.
/// - `if_not_exists` / `drop_if_exists`: make the script re-runnable / rebuildable.
/// - `quote_identifiers`: wrap identifiers in the dialect's delimiter.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    dialect: &str,
    foreign_keys: bool,
    indexes: bool,
    comments: bool,
    if_not_exists: bool,
    drop_if_exists: bool,
    quote_identifiers: bool,
) -> Result<String, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, which is over the {} byte limit — split the schema or remove unused tables",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    if input.trim().is_empty() {
        return Err("no DBML given — paste a schema with at least one `Table` block".into());
    }
    let requested = Dialect::parse(dialect)?;

    INLINE_REFS.with(|c| c.borrow_mut().clear());
    PENDING_PARTIALS.with(|c| c.borrow_mut().clear());
    let cleaned = strip_comments(input);
    let parse_result = parse_document(&cleaned);
    let inline = INLINE_REFS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    let pending = PENDING_PARTIALS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    let mut doc = parse_result?;
    doc.refs.extend(inline);
    expand_partials(&mut doc, &pending);

    if doc.tables.is_empty() {
        return Err(
            "no `Table` blocks found — a DBML schema needs at least one, e.g. `Table users { id int [pk] }`"
                .into(),
        );
    }

    let d = requested
        .or_else(|| doc.database_type.as_deref().and_then(Dialect::from_project))
        .unwrap_or(Dialect::Postgres);

    let e = Emitter {
        d,
        quote: quote_identifiers,
        foreign_keys,
        indexes,
        comments,
        if_not_exists,
    };

    let mut out: Vec<String> = Vec::new();

    if drop_if_exists {
        for t in doc.tables.iter().rev() {
            out.push(drop_table_stmt(&e, &e.table_ref(t)));
        }
        for j in join_tables(&e, &doc)? {
            out.push(drop_table_stmt(&e, &j.name_sql));
        }
        if d == Dialect::Postgres {
            for en in &doc.enums {
                out.push(format!(
                    "DROP TYPE IF EXISTS {};",
                    e.qualified(en.schema.as_deref(), &en.name)
                ));
            }
        }
    }

    if d == Dialect::Postgres {
        for en in &doc.enums {
            out.push(format!(
                "CREATE TYPE {} AS ENUM (\n  {}\n);",
                e.qualified(en.schema.as_deref(), &en.name),
                enum_values_sql(&en.values)
            ));
        }
    }

    let mut comment_stmts: Vec<String> = Vec::new();
    let mut index_stmts: Vec<String> = Vec::new();

    for t in &doc.tables {
        if t.columns.is_empty() {
            return Err(format!(
                "table `{}` has no columns — add at least one, e.g. `id int [pk]`",
                t.name
            ));
        }
        out.push(emit_table(&e, t, &doc.enums)?);
        if e.indexes {
            index_stmts.extend(emit_indexes(&e, t)?);
        }
        if e.comments && d.has_comment_on() {
            if let Some(n) = &t.note {
                comment_stmts.push(format!(
                    "COMMENT ON TABLE {} IS {};",
                    e.table_ref(t),
                    sql_string(n)
                ));
            }
            for c in &t.columns {
                if let Some(n) = &c.note {
                    comment_stmts.push(format!(
                        "COMMENT ON COLUMN {}.{} IS {};",
                        e.table_ref(t),
                        e.id(&c.name),
                        sql_string(n)
                    ));
                }
            }
        }
    }

    let joins = join_tables(&e, &doc)?;
    for j in &joins {
        out.push(j.create_sql.clone());
    }

    out.extend(index_stmts);

    if e.foreign_keys {
        out.extend(emit_foreign_keys(&e, &doc)?);
        for j in &joins {
            out.extend(j.fk_sql.clone());
        }
    }

    out.extend(comment_stmts);

    Ok(out.join("\n\n"))
}

fn drop_table_stmt(e: &Emitter, name_sql: &str) -> String {
    match e.d {
        Dialect::Oracle => format!("DROP TABLE IF EXISTS {name_sql} CASCADE CONSTRAINTS;"),
        _ => format!("DROP TABLE IF EXISTS {name_sql};"),
    }
}

/// Pull `~partial` blocks into the tables that reference them.
fn expand_partials(doc: &mut Document, pending: &[(usize, Vec<String>)]) {
    for (idx, names) in pending {
        let Some(table) = doc.tables.get(*idx) else {
            continue;
        };
        let mut cols = Vec::new();
        let mut idxs = Vec::new();
        for n in names {
            if let Some((_, p)) = doc
                .partials
                .iter()
                .find(|(pn, _)| pn.eq_ignore_ascii_case(n))
            {
                for c in &p.columns {
                    if !table.columns.iter().any(|e| e.name == c.name) {
                        cols.push(c.clone());
                    }
                }
                idxs.extend(p.indexes.clone());
            }
        }
        if cols.is_empty() && idxs.is_empty() {
            continue;
        }
        let t = &mut doc.tables[*idx];
        // Partial columns lead, matching how DBML composes them.
        cols.extend(std::mem::take(&mut t.columns));
        t.columns = cols;
        t.indexes.extend(idxs);
    }
}

/// Find the `Enum` a column's type names, ignoring any schema qualifier and
/// letter case (`public.mood` and `Mood` both match `Enum mood`).
fn lookup_enum<'a>(enums: &'a [EnumDef], ty: &str) -> Option<&'a EnumDef> {
    let base = split_type(ty).0;
    let bare = base.rsplit('.').next().unwrap_or(&base);
    enums.iter().find(|en| en.name.eq_ignore_ascii_case(bare))
}

fn emit_table(e: &Emitter, t: &Table, enums: &[EnumDef]) -> Result<String, String> {
    let mut pk_cols: Vec<String> = t
        .columns
        .iter()
        .filter(|c| c.pk)
        .map(|c| c.name.clone())
        .collect();
    for idx in &t.indexes {
        if idx.pk {
            for p in &idx.parts {
                if let IndexPart::Col(c) = p {
                    if !pk_cols.contains(c) {
                        pk_cols.push(c.clone());
                    }
                }
            }
        }
    }
    let single_pk = pk_cols.len() == 1;

    let mut lines: Vec<String> = Vec::new();
    let mut table_checks: Vec<String> = Vec::new();

    for c in &t.columns {
        let mut parts: Vec<String> = vec![e.id(&c.name)];
        let increment = c.increment || type_is_serial(&c.ty);
        let is_sole_pk = single_pk && pk_cols[0] == c.name;
        let enum_def = lookup_enum(enums, &c.ty);

        let ty_sql = match (enum_def, e.d) {
            (Some(en), Dialect::Postgres) => e.qualified(en.schema.as_deref(), &en.name),
            (Some(en), Dialect::Mysql) => format!("ENUM({})", enum_values_sql(&en.values)),
            (Some(_), Dialect::SqlServer) => "NVARCHAR(255)".into(),
            (Some(_), Dialect::Oracle) => "VARCHAR2(255)".into(),
            (Some(_), Dialect::Sqlite) => "TEXT".into(),
            (None, _) => increment_type(&c.ty, e.d, increment, is_sole_pk),
        };
        parts.push(ty_sql);

        // Auto-increment marker, where it is a column suffix rather than a type.
        if increment {
            match e.d {
                Dialect::Mysql => parts.push("AUTO_INCREMENT".into()),
                Dialect::SqlServer => parts.push("IDENTITY(1,1)".into()),
                Dialect::Oracle => parts.push("GENERATED BY DEFAULT AS IDENTITY".into()),
                Dialect::Sqlite if is_sole_pk => parts.push("PRIMARY KEY AUTOINCREMENT".into()),
                _ => {}
            }
        }

        let sqlite_pk_done = e.d == Dialect::Sqlite && increment && is_sole_pk;
        if is_sole_pk && !sqlite_pk_done {
            parts.push("PRIMARY KEY".into());
        }
        // SERIAL / IDENTITY already imply NOT NULL.
        if (c.not_null || (c.pk && !is_sole_pk)) && !(increment && e.d == Dialect::Postgres) {
            parts.push("NOT NULL".into());
        }
        if let Some(dv) = &c.default {
            parts.push(format!("DEFAULT {}", default_sql(dv, e.d)));
        }
        if c.unique && !is_sole_pk {
            parts.push("UNIQUE".into());
        }
        if let Some(chk) = &c.check {
            parts.push(format!("CHECK ({chk})"));
        }
        // Enum values become a CHECK where the dialect has no enum type.
        if let Some(en) = enum_def {
            if matches!(e.d, Dialect::Sqlite | Dialect::SqlServer | Dialect::Oracle) {
                parts.push(format!(
                    "CHECK ({} IN ({}))",
                    e.id(&c.name),
                    enum_values_sql(&en.values)
                ));
            }
        }
        if e.comments && e.d == Dialect::Mysql {
            if let Some(n) = &c.note {
                parts.push(format!("COMMENT {}", sql_string(n)));
            }
        }

        let mut line = format!("  {}", parts.join(" "));
        // Dialects without COMMENT support get the note as an SQL line comment.
        if e.comments && !e.d.has_comment_on() && e.d != Dialect::Mysql {
            if let Some(n) = &c.note {
                line.push_str(&format!(" -- {}", n.replace('\n', " ")));
            }
        }
        lines.push(line);
    }

    if !single_pk && !pk_cols.is_empty() {
        let cols = pk_cols
            .iter()
            .map(|c| e.id(c))
            .collect::<Vec<_>>()
            .join(", ");
        table_checks.push(format!("  PRIMARY KEY ({cols})"));
    }
    for (expr, name) in &t.checks {
        match name {
            Some(n) => table_checks.push(format!("  CONSTRAINT {} CHECK ({expr})", e.id(n))),
            None => table_checks.push(format!("  CHECK ({expr})")),
        }
    }
    // Composite UNIQUE from an `indexes { (a, b) [unique] }` entry stays a
    // separate CREATE UNIQUE INDEX, so nothing else is added here.

    let body = lines
        .into_iter()
        .chain(table_checks)
        .collect::<Vec<_>>()
        .join(",\n");

    let name_sql = e.table_ref(t);
    let head = create_table_head(e, &name_sql, t);
    let mut stmt = format!("{head} (\n{body}\n)");
    if e.comments && e.d == Dialect::Mysql {
        if let Some(n) = &t.note {
            stmt.push_str(&format!(" COMMENT={}", sql_string(n)));
        }
    }
    stmt.push(';');
    if e.comments && !e.d.has_comment_on() && e.d != Dialect::Mysql {
        if let Some(n) = &t.note {
            stmt = format!("-- {}\n{stmt}", n.replace('\n', " "));
        }
    }
    Ok(stmt)
}

fn create_table_head(e: &Emitter, name_sql: &str, _t: &Table) -> String {
    if !e.if_not_exists {
        return format!("CREATE TABLE {name_sql}");
    }
    match e.d {
        Dialect::SqlServer => format!(
            "IF OBJECT_ID(N'{}', N'U') IS NULL\nCREATE TABLE {name_sql}",
            name_sql.replace('\'', "''")
        ),
        _ => format!("CREATE TABLE IF NOT EXISTS {name_sql}"),
    }
}

/// Apply the dialect's auto-increment spelling to the column type.
fn increment_type(ty: &str, d: Dialect, increment: bool, is_sole_pk: bool) -> String {
    if !increment {
        return map_type(ty, d);
    }
    match d {
        Dialect::Postgres => {
            if is_bigint(ty) {
                "BIGSERIAL".into()
            } else if is_smallint(ty) {
                "SMALLSERIAL".into()
            } else {
                "SERIAL".into()
            }
        }
        Dialect::Sqlite if is_sole_pk => "INTEGER".into(),
        _ => map_type(ty, d),
    }
}

fn default_sql(v: &DefaultValue, d: Dialect) -> String {
    match v {
        DefaultValue::Expr(e) => format!("({e})"),
        DefaultValue::Str(s) => sql_string(s),
        DefaultValue::Bare(b) => {
            let l = b.to_ascii_lowercase();
            match l.as_str() {
                "true" | "false" => match d {
                    Dialect::Mysql => (if l == "true" { "1" } else { "0" }).to_string(),
                    Dialect::SqlServer | Dialect::Oracle => {
                        (if l == "true" { "1" } else { "0" }).to_string()
                    }
                    _ => l.to_uppercase(),
                },
                "null" => "NULL".into(),
                _ => b.clone(),
            }
        }
    }
}

fn emit_indexes(e: &Emitter, t: &Table) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for idx in &t.indexes {
        if idx.pk {
            continue; // already a table-level PRIMARY KEY
        }
        let cols = idx
            .parts
            .iter()
            .map(|p| match p {
                IndexPart::Col(c) => e.id(c),
                IndexPart::Expr(x) => format!("({x})"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let default_name = {
            let joined = idx
                .parts
                .iter()
                .map(|p| match p {
                    IndexPart::Col(c) => c.clone(),
                    IndexPart::Expr(_) => "expr".into(),
                })
                .collect::<Vec<_>>()
                .join("_");
            format!(
                "{}_{}_{}",
                t.name,
                joined,
                if idx.unique { "key" } else { "idx" }
            )
        };
        let name = idx.name.clone().unwrap_or(default_name);
        let unique = if idx.unique { "UNIQUE " } else { "" };
        let guard = if e.if_not_exists && e.d.index_supports_if_not_exists() {
            "IF NOT EXISTS "
        } else {
            ""
        };
        let using = match idx.method.as_deref() {
            Some(m) if matches!(e.d, Dialect::Postgres | Dialect::Mysql) => {
                format!(" USING {}", m.to_uppercase())
            }
            _ => String::new(),
        };
        // PostgreSQL puts USING before the column list; MySQL after it.
        let stmt = if e.d == Dialect::Postgres && !using.is_empty() {
            format!(
                "CREATE {unique}INDEX {guard}{} ON {}{using} ({cols});",
                e.id(&name),
                e.table_ref(t)
            )
        } else {
            format!(
                "CREATE {unique}INDEX {guard}{} ON {} ({cols}){using};",
                e.id(&name),
                e.table_ref(t)
            )
        };
        out.push(stmt);
    }
    Ok(out)
}

struct JoinTable {
    name_sql: String,
    create_sql: String,
    fk_sql: Vec<String>,
}

/// Resolve a ref endpoint to a table, honouring `as` aliases and bare names in
/// a schema-qualified document.
fn resolve<'a>(doc: &'a Document, ep: &Endpoint) -> Option<&'a Table> {
    let key = table_key(ep.schema.as_deref(), &ep.table);
    if let Some(t) = doc.tables.iter().find(|t| t.key() == key) {
        return Some(t);
    }
    if ep.schema.is_none() {
        if let Some(t) = doc
            .tables
            .iter()
            .find(|t| t.alias.as_deref() == Some(ep.table.as_str()))
        {
            return Some(t);
        }
        if let Some(t) = doc.tables.iter().find(|t| t.name == ep.table) {
            return Some(t);
        }
    }
    doc.tables
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(&ep.table))
}

fn missing_table(ep: &Endpoint) -> String {
    format!(
        "Ref target table `{}` is not defined in this schema — add a `Table {}` block or fix the reference",
        ep.table, ep.table
    )
}

fn column_is_unique(t: &Table, cols: &[String]) -> bool {
    if cols.len() == 1 {
        if let Some(c) = t.columns.iter().find(|c| c.name == cols[0]) {
            if c.pk || c.unique {
                return true;
            }
        }
    }
    t.indexes.iter().any(|i| {
        (i.pk || i.unique)
            && i.parts.len() == cols.len()
            && i.parts.iter().zip(cols).all(|(p, c)| match p {
                IndexPart::Col(pc) => pc == c,
                IndexPart::Expr(_) => false,
            })
    })
}

/// Decide which side of a ref carries the foreign key.
/// `>` → left; `<` → right; `-` → the side whose columns are NOT the unique
/// key, falling back to the left.
fn fk_sides<'a>(
    doc: &'a Document,
    r: &'a RefDef,
) -> Result<(&'a Table, &'a Endpoint, &'a Table, &'a Endpoint), String> {
    let lt = resolve(doc, &r.left).ok_or_else(|| missing_table(&r.left))?;
    let rt = resolve(doc, &r.right).ok_or_else(|| missing_table(&r.right))?;
    let child_is_left = match r.op {
        RefOp::ManyToOne => true,
        RefOp::OneToMany => false,
        RefOp::OneToOne => {
            !(column_is_unique(lt, &r.left.cols) && !column_is_unique(rt, &r.right.cols))
        }
        RefOp::ManyToMany => true,
    };
    Ok(if child_is_left {
        (lt, &r.left, rt, &r.right)
    } else {
        (rt, &r.right, lt, &r.left)
    })
}

fn fk_name(child: &Table, cols: &[String], explicit: Option<&str>) -> String {
    match explicit {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => format!("fk_{}_{}", child.name, cols.join("_")),
    }
}

fn emit_foreign_keys(e: &Emitter, doc: &Document) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for r in &doc.refs {
        if r.op == RefOp::ManyToMany {
            continue; // handled by the join table
        }
        let (child, child_ep, parent, parent_ep) = fk_sides(doc, r)?;
        if child_ep.cols.len() != parent_ep.cols.len() {
            return Err(format!(
                "Ref between `{}` and `{}` lists {} column(s) on one side and {} on the other — composite refs need the same count on both",
                child.name,
                parent.name,
                child_ep.cols.len(),
                parent_ep.cols.len()
            ));
        }
        for c in &child_ep.cols {
            if !child.columns.iter().any(|x| &x.name == c) {
                return Err(format!(
                    "Ref uses column `{}.{}`, which table `{}` does not define",
                    child.name, c, child.name
                ));
            }
        }
        let cols = child_ep
            .cols
            .iter()
            .map(|c| e.id(c))
            .collect::<Vec<_>>()
            .join(", ");
        let refs = parent_ep
            .cols
            .iter()
            .map(|c| e.id(c))
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = format!(
            "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({cols}) REFERENCES {} ({refs})",
            e.table_ref(child),
            e.id(&fk_name(child, &child_ep.cols, r.name.as_deref())),
            e.table_ref(parent)
        );
        if let Some(a) = &r.on_delete {
            stmt.push_str(&format!(" ON DELETE {a}"));
        }
        if let Some(a) = &r.on_update {
            stmt.push_str(&format!(" ON UPDATE {a}"));
        }
        stmt.push(';');
        out.push(stmt);
    }
    Ok(out)
}

/// `<>` many-to-many refs compile into a real join table.
fn join_tables(e: &Emitter, doc: &Document) -> Result<Vec<JoinTable>, String> {
    let mut out = Vec::new();
    for r in doc.refs.iter().filter(|r| r.op == RefOp::ManyToMany) {
        let lt = resolve(doc, &r.left).ok_or_else(|| missing_table(&r.left))?;
        let rt = resolve(doc, &r.right).ok_or_else(|| missing_table(&r.right))?;
        let jname = r
            .name
            .clone()
            .unwrap_or_else(|| format!("{}_{}", lt.name, rt.name));
        let name_sql = e.qualified(lt.schema.as_deref(), &jname);
        let lcol = format!("{}_{}", lt.name, r.left.cols.join("_"));
        let rcol = format!("{}_{}", rt.name, r.right.cols.join("_"));
        let lty = ref_col_type(lt, &r.left.cols, e.d);
        let rty = ref_col_type(rt, &r.right.cols, e.d);
        let head = create_table_head(e, &name_sql, lt);
        let create_sql = format!(
            "{head} (\n  {} {lty} NOT NULL,\n  {} {rty} NOT NULL,\n  PRIMARY KEY ({}, {})\n);",
            e.id(&lcol),
            e.id(&rcol),
            e.id(&lcol),
            e.id(&rcol)
        );
        let fk_sql = vec![
            format!(
                "ALTER TABLE {name_sql} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE CASCADE;",
                e.id(&format!("fk_{jname}_{lcol}")),
                e.id(&lcol),
                e.table_ref(lt),
                r.left.cols.iter().map(|c| e.id(c)).collect::<Vec<_>>().join(", ")
            ),
            format!(
                "ALTER TABLE {name_sql} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE CASCADE;",
                e.id(&format!("fk_{jname}_{rcol}")),
                e.id(&rcol),
                e.table_ref(rt),
                r.right.cols.iter().map(|c| e.id(c)).collect::<Vec<_>>().join(", ")
            ),
        ];
        out.push(JoinTable {
            name_sql,
            create_sql,
            fk_sql,
        });
    }
    Ok(out)
}

/// The SQL type a join-table column needs to match the referenced key. A
/// `serial` parent becomes a plain integer on the child side.
fn ref_col_type(t: &Table, cols: &[String], d: Dialect) -> String {
    let ty = cols
        .first()
        .and_then(|c| t.columns.iter().find(|x| &x.name == c))
        .map(|c| c.ty.clone())
        .unwrap_or_else(|| "int".into());
    let (base, _, _) = split_type(&ty);
    let plain = match base.as_str() {
        "serial" | "smallserial" => "int".to_string(),
        "bigserial" => "bigint".to_string(),
        _ => ty,
    };
    map_type(&plain, d)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dbml: &str, dialect: &str) -> String {
        convert(dbml, dialect, true, true, true, false, false, true).expect("convert should succeed")
    }

    const BLOG: &str = r#"
Project blog {
  database_type: 'PostgreSQL'
}

Table users {
  id integer [pk, increment]
  email varchar(255) [not null, unique]
  role role [not null, default: 'member']
  created_at timestamp [default: `now()`]
}

Table posts {
  id integer [pk, increment]
  title varchar(200) [not null]
  author_id integer [not null, ref: > users.id]

  indexes {
    author_id [name: 'posts_author_idx']
  }
}

Enum role {
  member
  admin
}
"#;

    #[test]
    fn happy_path_postgres() {
        let sql = run(BLOG, "auto");
        assert!(
            sql.contains("CREATE TYPE \"role\" AS ENUM (\n  'member', 'admin'\n);"),
            "{sql}"
        );
        assert!(sql.contains("\"id\" SERIAL PRIMARY KEY"), "{sql}");
        assert!(
            sql.contains("\"email\" VARCHAR(255) NOT NULL UNIQUE"),
            "{sql}"
        );
        assert!(
            sql.contains("\"role\" \"role\" NOT NULL DEFAULT 'member'"),
            "{sql}"
        );
        assert!(
            sql.contains("\"created_at\" TIMESTAMP DEFAULT (now())"),
            "{sql}"
        );
        assert!(
            sql.contains("CREATE INDEX \"posts_author_idx\" ON \"posts\" (\"author_id\");"),
            "{sql}"
        );
        assert!(
            sql.contains("ALTER TABLE \"posts\" ADD CONSTRAINT \"fk_posts_author_id\" FOREIGN KEY (\"author_id\") REFERENCES \"users\" (\"id\");"),
            "{sql}"
        );
    }

    #[test]
    fn error_on_unknown_ref_target() {
        let err = convert(
            "Table posts {\n  id int [pk]\n  user_id int [ref: > users.id]\n}",
            "postgresql",
            true,
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap_err();
        assert!(err.contains("users"), "{err}");
        assert!(err.contains("not defined in this schema"), "{err}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = convert("   \n  ", "auto", true, true, true, false, false, true).unwrap_err();
        assert!(err.contains("no DBML given"), "{err}");
    }

    #[test]
    fn error_on_no_tables() {
        let err = convert(
            "Project x {\n  database_type: 'MySQL'\n}",
            "auto",
            true,
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap_err();
        assert!(err.contains("no `Table` blocks found"), "{err}");
    }

    #[test]
    fn error_on_column_without_type() {
        let err = convert(
            "Table t {\n  id\n}",
            "postgresql",
            true,
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap_err();
        assert!(err.contains("has no type"), "{err}");
    }

    #[test]
    fn error_on_bad_dialect() {
        let err = convert(
            "Table t { id int [pk] }",
            "duckdb",
            true,
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap_err();
        assert!(err.contains("expected auto, postgresql"), "{err}");
    }

    #[test]
    fn error_on_oversized_input() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = convert(&big, "auto", true, true, true, false, false, true).unwrap_err();
        assert!(err.contains("over the 200000 byte limit"), "{err}");
    }

    #[test]
    fn exact_cap_boundary_is_accepted() {
        let table = "Table t {\n  id int [pk]\n}\n";
        let pad = MAX_INPUT_BYTES - table.len();
        let doc = format!("{}{}", "/".repeat(0) , format!("{table}{}", " ".repeat(pad)));
        assert_eq!(doc.len(), MAX_INPUT_BYTES);
        let sql = convert(&doc, "postgresql", true, true, true, false, false, true).unwrap();
        assert!(sql.starts_with("CREATE TABLE \"t\""), "{sql}");
    }

    #[test]
    fn mysql_dialect_types_and_autoincrement() {
        let sql = run(BLOG, "mysql");
        assert!(sql.contains("`id` INT AUTO_INCREMENT PRIMARY KEY"), "{sql}");
        assert!(
            sql.contains("`role` ENUM('member', 'admin') NOT NULL DEFAULT 'member'"),
            "{sql}"
        );
        assert!(sql.contains("`created_at` DATETIME DEFAULT (now())"), "{sql}");
        assert!(!sql.contains("CREATE TYPE"), "mysql has no enum type: {sql}");
    }

    #[test]
    fn sqlite_dialect_uses_integer_primary_key_autoincrement() {
        let sql = run(BLOG, "sqlite");
        assert!(
            sql.contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"),
            "{sql}"
        );
        assert!(
            sql.contains("CHECK (\"role\" IN ('member', 'admin'))"),
            "{sql}"
        );
    }

    #[test]
    fn sqlserver_dialect_uses_identity_and_nvarchar() {
        let sql = run(BLOG, "sqlserver");
        assert!(sql.contains("[id] INT IDENTITY(1,1) PRIMARY KEY"), "{sql}");
        assert!(sql.contains("[email] NVARCHAR(255) NOT NULL UNIQUE"), "{sql}");
        assert!(sql.contains("[created_at] DATETIME2"), "{sql}");
    }

    #[test]
    fn oracle_dialect_uses_identity_and_varchar2() {
        let sql = run(BLOG, "oracle");
        assert!(
            sql.contains("\"id\" NUMBER(10) GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY"),
            "{sql}"
        );
        assert!(sql.contains("\"email\" VARCHAR2(255)"), "{sql}");
    }

    #[test]
    fn auto_dialect_reads_project_database_type() {
        let dbml = "Project p {\n  database_type: 'MySQL'\n}\nTable t {\n  id int [pk]\n}";
        let sql = run(dbml, "auto");
        assert!(sql.contains("CREATE TABLE `t`"), "{sql}");
    }

    #[test]
    fn auto_dialect_defaults_to_postgres_without_project() {
        let sql = run("Table t {\n  id int [pk]\n}", "auto");
        assert_eq!(sql, "CREATE TABLE \"t\" (\n  \"id\" INTEGER PRIMARY KEY\n);");
    }

    #[test]
    fn long_and_short_ref_forms_both_work() {
        let dbml = r#"
Table users { id int [pk] }
Table posts { id int [pk] user_id int }
Table tags { id int [pk] post_id int }
Ref: posts.user_id > users.id [delete: cascade, update: no action]
Ref tag_owner {
  tags.post_id > posts.id
}
"#;
        let sql = run(dbml, "postgresql");
        assert!(
            sql.contains("FOREIGN KEY (\"user_id\") REFERENCES \"users\" (\"id\") ON DELETE CASCADE ON UPDATE NO ACTION;"),
            "{sql}"
        );
        assert!(sql.contains("ADD CONSTRAINT \"tag_owner\" FOREIGN KEY"), "{sql}");
    }

    #[test]
    fn one_to_many_operator_puts_fk_on_the_many_side() {
        let dbml = "Table users { id int [pk] }\nTable posts { id int [pk] user_id int }\nRef: users.id < posts.user_id";
        let sql = run(dbml, "postgresql");
        assert!(
            sql.contains("ALTER TABLE \"posts\" ADD CONSTRAINT \"fk_posts_user_id\" FOREIGN KEY (\"user_id\") REFERENCES \"users\" (\"id\");"),
            "{sql}"
        );
    }

    #[test]
    fn one_to_one_puts_fk_on_the_non_unique_side() {
        let dbml = "Table users { id int [pk] }\nTable profiles { id int [pk] user_id int }\nRef: users.id - profiles.user_id";
        let sql = run(dbml, "postgresql");
        assert!(
            sql.contains("ALTER TABLE \"profiles\" ADD CONSTRAINT \"fk_profiles_user_id\""),
            "{sql}"
        );
    }

    #[test]
    fn many_to_many_creates_a_join_table() {
        let dbml = "Table posts { id int [pk] }\nTable tags { id int [pk] }\nRef: posts.id <> tags.id";
        let sql = run(dbml, "postgresql");
        assert!(sql.contains("CREATE TABLE \"posts_tags\""), "{sql}");
        assert!(sql.contains("\"posts_id\" INTEGER NOT NULL"), "{sql}");
        assert!(
            sql.contains("PRIMARY KEY (\"posts_id\", \"tags_id\")"),
            "{sql}"
        );
        assert!(
            sql.contains("ALTER TABLE \"posts_tags\" ADD CONSTRAINT \"fk_posts_tags_posts_id\" FOREIGN KEY (\"posts_id\") REFERENCES \"posts\" (\"id\") ON DELETE CASCADE;"),
            "{sql}"
        );
    }

    #[test]
    fn composite_refs_and_composite_primary_keys() {
        let dbml = r#"
Table orders {
  region varchar(2)
  no int
  indexes {
    (region, no) [pk]
  }
}
Table lines {
  region varchar(2)
  order_no int
}
Ref: lines.(region, order_no) > orders.(region, no)
"#;
        let sql = run(dbml, "postgresql");
        assert!(sql.contains("PRIMARY KEY (\"region\", \"no\")"), "{sql}");
        assert!(
            sql.contains("FOREIGN KEY (\"region\", \"order_no\") REFERENCES \"orders\" (\"region\", \"no\");"),
            "{sql}"
        );
    }

    #[test]
    fn composite_ref_arity_mismatch_errors() {
        let dbml = "Table a { x int y int }\nTable b { x int }\nRef: a.(x, y) > b.(x)";
        let err = convert(dbml, "postgresql", true, true, true, false, false, true).unwrap_err();
        assert!(err.contains("same count on both"), "{err}");
    }

    #[test]
    fn index_settings_unique_name_and_type() {
        let dbml = r#"
Table t {
  a int
  b int
  indexes {
    (a, b) [unique]
    a [type: hash]
    `lower(b)` [name: 'expr_idx']
  }
}
"#;
        let sql = run(dbml, "postgresql");
        assert!(
            sql.contains("CREATE UNIQUE INDEX \"t_a_b_key\" ON \"t\" (\"a\", \"b\");"),
            "{sql}"
        );
        assert!(
            sql.contains("CREATE INDEX \"t_a_idx\" ON \"t\" USING HASH (\"a\");"),
            "{sql}"
        );
        assert!(
            sql.contains("CREATE INDEX \"expr_idx\" ON \"t\" ((lower(b)));"),
            "{sql}"
        );
    }

    #[test]
    fn mysql_puts_using_after_the_column_list() {
        let dbml = "Table t {\n  a int\n  indexes {\n    a [type: hash]\n  }\n}";
        let sql = run(dbml, "mysql");
        assert!(
            sql.contains("CREATE INDEX `t_a_idx` ON `t` (`a`) USING HASH;"),
            "{sql}"
        );
    }

    #[test]
    fn toggles_can_suppress_indexes_and_foreign_keys() {
        let sql = convert(BLOG, "postgresql", false, false, true, false, false, true).unwrap();
        assert!(!sql.contains("CREATE INDEX"), "{sql}");
        assert!(!sql.contains("ALTER TABLE"), "{sql}");
        assert!(sql.contains("CREATE TABLE \"posts\""), "{sql}");
    }

    #[test]
    fn if_not_exists_and_drop_if_exists() {
        let dbml = "Table t {\n  id int [pk]\n  indexes {\n    id\n  }\n}\nEnum e { a b }";
        let sql = convert(dbml, "postgresql", true, true, true, true, true, true).unwrap();
        assert!(sql.starts_with("DROP TABLE IF EXISTS \"t\";"), "{sql}");
        assert!(sql.contains("DROP TYPE IF EXISTS \"e\";"), "{sql}");
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS \"t\""), "{sql}");
        assert!(
            sql.contains("CREATE INDEX IF NOT EXISTS \"t_id_idx\""),
            "{sql}"
        );
    }

    #[test]
    fn sqlserver_if_not_exists_uses_an_object_id_guard() {
        let sql = convert(
            "Table t { id int [pk] }",
            "sqlserver",
            true,
            true,
            true,
            true,
            false,
            true,
        )
        .unwrap();
        assert!(
            sql.starts_with("IF OBJECT_ID(N'[t]', N'U') IS NULL\nCREATE TABLE [t] ("),
            "{sql}"
        );
    }

    #[test]
    fn unquoted_identifiers() {
        let sql = convert(
            "Table t { id int [pk] name varchar(10) }",
            "postgresql",
            true,
            true,
            true,
            false,
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            sql,
            "CREATE TABLE t (\n  id INTEGER PRIMARY KEY,\n  name VARCHAR(10)\n);"
        );
    }

    #[test]
    fn notes_become_comments_per_dialect() {
        let dbml = r#"
Table users [note: 'People who sign in'] {
  id int [pk]
  email varchar(255) [note: 'Login address']
}
"#;
        let pg = run(dbml, "postgresql");
        assert!(
            pg.contains("COMMENT ON TABLE \"users\" IS 'People who sign in';"),
            "{pg}"
        );
        assert!(
            pg.contains("COMMENT ON COLUMN \"users\".\"email\" IS 'Login address';"),
            "{pg}"
        );
        let my = run(dbml, "mysql");
        assert!(my.contains("COMMENT 'Login address'"), "{my}");
        assert!(my.contains(") COMMENT='People who sign in';"), "{my}");
        let lite = run(dbml, "sqlite");
        assert!(lite.contains("-- People who sign in"), "{lite}");
        assert!(lite.contains("-- Login address"), "{lite}");
    }

    #[test]
    fn comments_toggle_off_removes_them() {
        let dbml = "Table users [note: 'x'] { id int [pk] }";
        let sql = convert(dbml, "postgresql", true, true, false, false, false, true).unwrap();
        assert!(!sql.contains("COMMENT"), "{sql}");
    }

    #[test]
    fn schema_qualified_tables_and_enums() {
        let dbml = "Table core.users {\n  id int [pk]\n  m core.mood\n}\nEnum core.mood { ok bad }";
        let sql = run(dbml, "postgresql");
        assert!(sql.contains("CREATE TYPE \"core\".\"mood\""), "{sql}");
        assert!(sql.contains("CREATE TABLE \"core\".\"users\""), "{sql}");
        assert!(sql.contains("\"m\" \"core\".\"mood\""), "{sql}");
    }

    #[test]
    fn table_alias_resolves_in_refs() {
        let dbml = "Table users as U { id int [pk] }\nTable posts { id int [pk] uid int }\nRef: posts.uid > U.id";
        let sql = run(dbml, "postgresql");
        assert!(sql.contains("REFERENCES \"users\" (\"id\");"), "{sql}");
    }

    #[test]
    fn comments_and_multiline_notes_are_stripped_and_kept() {
        let dbml = "// a line comment\nTable t { /* inline */ id int [pk] }\nNote t_note {\n  '''hi'''\n}";
        let sql = run(dbml, "postgresql");
        assert_eq!(sql, "CREATE TABLE \"t\" (\n  \"id\" INTEGER PRIMARY KEY\n);");
    }

    #[test]
    fn table_partial_columns_are_merged_in() {
        let dbml = r#"
TablePartial timestamps {
  created_at timestamp [default: `now()`]
  updated_at timestamp
}
Table t {
  ~timestamps
  id int [pk]
}
"#;
        let sql = run(dbml, "postgresql");
        assert!(sql.contains("\"created_at\" TIMESTAMP DEFAULT (now())"), "{sql}");
        assert!(sql.contains("\"updated_at\" TIMESTAMP"), "{sql}");
        assert!(sql.contains("\"id\" INTEGER PRIMARY KEY"), "{sql}");
    }

    #[test]
    fn column_check_and_table_checks_blocks() {
        let dbml = r#"
Table t {
  id int [pk]
  age int [check: `age >= 0`]
  checks {
    `id < 1000` [name: 'id_range']
  }
}
"#;
        let sql = run(dbml, "postgresql");
        assert!(sql.contains("\"age\" INTEGER CHECK (age >= 0)"), "{sql}");
        assert!(
            sql.contains("CONSTRAINT \"id_range\" CHECK (id < 1000)"),
            "{sql}"
        );
    }

    #[test]
    fn default_value_forms() {
        let dbml = r#"
Table t {
  id int [pk]
  a varchar(5) [default: 'hi']
  b int [default: 42]
  c boolean [default: false]
  d timestamp [default: `now()`]
  e varchar(5) [default: null]
}
"#;
        let pg = run(dbml, "postgresql");
        assert!(pg.contains("DEFAULT 'hi'"), "{pg}");
        assert!(pg.contains("DEFAULT 42"), "{pg}");
        assert!(pg.contains("DEFAULT FALSE"), "{pg}");
        assert!(pg.contains("DEFAULT (now())"), "{pg}");
        assert!(pg.contains("DEFAULT NULL"), "{pg}");
        let my = run(dbml, "mysql");
        assert!(my.contains("DEFAULT 0"), "mysql booleans are 0/1: {my}");
    }

    #[test]
    fn multi_word_types_survive() {
        let dbml = "Table t {\n  id int [pk]\n  a double precision\n  b timestamp with time zone\n}";
        let sql = run(dbml, "postgresql");
        assert!(sql.contains("\"a\" DOUBLE PRECISION"), "{sql}");
        assert!(sql.contains("\"b\" TIMESTAMPTZ"), "{sql}");
    }

    #[test]
    fn unknown_types_pass_through_verbatim() {
        let sql = run("Table t {\n  id int [pk]\n  g geometry(Point,4326)\n}", "postgresql");
        assert!(sql.contains("\"g\" geometry(Point,4326)"), "{sql}");
    }

    #[test]
    fn postgres_arrays_are_kept_and_flattened_elsewhere() {
        let dbml = "Table t {\n  id int [pk]\n  tags text[]\n}";
        assert!(run(dbml, "postgresql").contains("\"tags\" TEXT[]"));
        assert!(run(dbml, "mysql").contains("`tags` TEXT"));
    }

    #[test]
    fn quoted_names_with_spaces_are_escaped() {
        let sql = run("Table \"user data\" {\n  \"first name\" varchar(10) [pk]\n}", "postgresql");
        assert!(
            sql.contains("CREATE TABLE \"user data\" (\n  \"first name\" VARCHAR(10) PRIMARY KEY\n);"),
            "{sql}"
        );
    }
}
