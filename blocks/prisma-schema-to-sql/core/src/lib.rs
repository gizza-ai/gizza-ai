//! prisma-schema-to-sql core — turn a Prisma `schema.prisma` (models, enums,
//! attributes, relations) into `CREATE TABLE` DDL for a chosen SQL dialect.
//! Pure compute, no wafer/wasm-bindgen deps; shared by the chat skill block and
//! the standalone web page.
//!
//! The parser is deliberately lenient and structural (block splitter + per-line
//! field parser) rather than a full Prisma grammar: it reads `model`, `enum`,
//! `view`, `datasource` and `generator` blocks, ignores what it does not model,
//! and never executes anything. Type mapping follows Prisma's own default
//! column types per provider, so the output is close to what
//! `prisma migrate diff --from-empty` would produce.

/// Largest schema accepted, in bytes. Anything bigger is rejected instead of
/// being silently truncated.
pub const MAX_INPUT_BYTES: usize = 200_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Postgres,
    Mysql,
    Sqlite,
    SqlServer,
}

impl Dialect {
    /// Parse the `dialect` param. `auto` (or empty) means "read the
    /// `datasource` block's provider", handled by the caller.
    fn parse(s: &str) -> Result<Option<Self>, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(None),
            "postgresql" | "postgres" | "pg" | "cockroachdb" => Ok(Some(Dialect::Postgres)),
            "mysql" | "mariadb" => Ok(Some(Dialect::Mysql)),
            "sqlite" | "sqlite3" => Ok(Some(Dialect::Sqlite)),
            "sqlserver" | "mssql" | "tsql" => Ok(Some(Dialect::SqlServer)),
            other => Err(format!(
                "invalid dialect {other:?}: expected auto, postgresql, mysql, sqlite, or sqlserver"
            )),
        }
    }
}

/// One `@attr(...)` / `@@attr(...)` occurrence.
#[derive(Clone)]
struct Attr {
    name: String,
    args: String,
}

impl Attr {
    /// Value of a named argument (`fields: [a, b]` → `[a, b]`).
    fn named(&self, key: &str) -> Option<String> {
        for part in split_top_level(&self.args, ',') {
            let part = part.trim();
            if let Some(rest) = strip_key(part, key) {
                return Some(rest.trim().to_string());
            }
        }
        None
    }

    /// First positional argument (`@relation("AuthorPosts", fields: [...])`).
    fn positional(&self) -> Option<String> {
        let first = split_top_level(&self.args, ',').into_iter().next()?;
        let first = first.trim();
        if first.is_empty() || first.contains(':') && !first.starts_with('"') {
            return None;
        }
        Some(first.to_string())
    }

    /// Identifier list argument (`[a, b]` or a bare `a`) as field names.
    fn ident_list(&self, key: Option<&str>) -> Vec<String> {
        let raw = match key {
            Some(k) => self.named(k),
            None => split_top_level(&self.args, ',')
                .into_iter()
                .next()
                .map(|s| s.trim().to_string()),
        };
        let raw = match raw {
            Some(r) => r,
            None => return Vec::new(),
        };
        let inner = raw.trim();
        let inner = inner
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(inner);
        split_top_level(inner, ',')
            .into_iter()
            .map(|s| {
                // `@@index([title(sort: Desc)])` → keep just the field name.
                let s = s.trim();
                let s = s.split('(').next().unwrap_or(s);
                s.trim().trim_matches('"').to_string()
            })
            .filter(|s| !s.is_empty())
            .collect()
    }
}

fn strip_key<'a>(part: &'a str, key: &str) -> Option<&'a str> {
    let rest = part.strip_prefix(key)?;
    let rest = rest.trim_start();
    rest.strip_prefix(':')
}

#[derive(Clone)]
struct Field {
    name: String,
    db_name: String,
    ty: String,
    /// `Unsupported("…")` payload, if the type is one.
    unsupported: Option<String>,
    optional: bool,
    list: bool,
    attrs: Vec<Attr>,
}

impl Field {
    fn attr(&self, name: &str) -> Option<&Attr> {
        self.attrs.iter().find(|a| a.name == name)
    }
    fn native(&self) -> Option<&Attr> {
        self.attrs.iter().find(|a| a.name.starts_with("db."))
    }
    fn default_expr(&self) -> Option<String> {
        self.attr("default").map(|a| a.args.trim().to_string())
    }
    fn is_autoincrement(&self) -> bool {
        matches!(self.default_expr().as_deref(), Some(d) if d.replace(' ', "") == "autoincrement()")
    }
}

struct Model {
    name: String,
    table: String,
    fields: Vec<Field>,
    block_attrs: Vec<Attr>,
}

impl Model {
    fn block_attrs_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Attr> {
        self.block_attrs.iter().filter(move |a| a.name == name)
    }
    fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

struct EnumDef {
    name: String,
    db_name: String,
    /// (prisma value, database value after `@map`)
    values: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Lexing helpers
// ---------------------------------------------------------------------------

/// Strip `//` line comments (Prisma has no block comments), preserving string
/// literals so a `//` inside `@default("http://x")` survives.
fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut in_str = false;
        let mut i = 0;
        let mut cut = chars.len();
        while i < chars.len() {
            match chars[i] {
                '\\' if in_str => i += 1,
                '"' => in_str = !in_str,
                '/' if !in_str && i + 1 < chars.len() && chars[i + 1] == '/' => {
                    cut = i;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        out.extend(chars[..cut].iter());
        out.push('\n');
    }
    out
}

/// Split on a separator that is not nested inside quotes, (), [] or {}.
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if in_str => {
                cur.push(c);
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
                continue;
            }
            '"' => {
                in_str = !in_str;
                cur.push(c);
            }
            '(' | '[' | '{' if !in_str => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' | '}' if !in_str => {
                depth -= 1;
                cur.push(c);
            }
            c if c == sep && !in_str && depth == 0 => {
                out.push(cur.clone());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

struct Block {
    kind: String,
    name: String,
    body: String,
}

fn read_ident(chars: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < chars.len() && (chars[*i].is_alphanumeric() || chars[*i] == '_') {
        *i += 1;
    }
    chars[start..*i].iter().collect()
}

fn skip_ws(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() {
        *i += 1;
    }
}

/// Split the schema into top-level `kind name { body }` blocks.
fn split_blocks(src: &str) -> Result<Vec<Block>, String> {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < n {
        skip_ws(&chars, &mut i);
        if i >= n {
            break;
        }
        if !(chars[i].is_alphabetic() || chars[i] == '_') {
            i += 1;
            continue;
        }
        let kind = read_ident(&chars, &mut i);
        skip_ws(&chars, &mut i);
        let name = read_ident(&chars, &mut i);
        skip_ws(&chars, &mut i);
        if i < n && chars[i] == '{' {
            i += 1;
            let start = i;
            let mut depth = 1i32;
            let mut in_str = false;
            while i < n && depth > 0 {
                match chars[i] {
                    '\\' if in_str => i += 1,
                    '"' => in_str = !in_str,
                    '{' if !in_str => depth += 1,
                    '}' if !in_str => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            if depth > 0 {
                return Err(format!(
                    "unbalanced braces: block `{kind} {name}` is never closed"
                ));
            }
            let body: String = chars[start..i - 1].iter().collect();
            out.push(Block { kind, name, body });
        }
    }
    Ok(out)
}

/// Parse the `@attr(...)` / `@@attr(...)` tail of a line.
fn parse_attrs(s: &str) -> Vec<Attr> {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < n {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        i += 1;
        if i < n && chars[i] == '@' {
            i += 1;
        }
        let mut name = read_ident(&chars, &mut i);
        while i < n && chars[i] == '.' {
            i += 1;
            name.push('.');
            name.push_str(&read_ident(&chars, &mut i));
        }
        let mut args = String::new();
        if i < n && chars[i] == '(' {
            i += 1;
            let start = i;
            let mut depth = 1i32;
            let mut in_str = false;
            while i < n && depth > 0 {
                match chars[i] {
                    '\\' if in_str => i += 1,
                    '"' => in_str = !in_str,
                    '(' | '[' if !in_str => depth += 1,
                    ')' | ']' if !in_str => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            args = chars[start..i.saturating_sub(1)].iter().collect();
        }
        if !name.is_empty() {
            out.push(Attr { name, args });
        }
    }
    out
}

/// String-literal argument (`"users"` → `users`).
fn literal_str(s: &str) -> Option<String> {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        Some(t[1..t.len() - 1].replace("\\\"", "\""))
    } else {
        None
    }
}

fn parse_model(name: &str, body: &str) -> Model {
    let mut fields = Vec::new();
    let mut block_attrs = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("@@") {
            block_attrs.extend(parse_attrs(line));
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0usize;
        let fname = read_ident(&chars, &mut i);
        if fname.is_empty() {
            continue;
        }
        skip_ws(&chars, &mut i);
        let mut ty = read_ident(&chars, &mut i);
        if ty.is_empty() {
            continue;
        }
        // `Unsupported("polygon")`
        let mut unsupported = None;
        if i < chars.len() && chars[i] == '(' {
            let start = i + 1;
            let mut depth = 1i32;
            i += 1;
            while i < chars.len() && depth > 0 {
                match chars[i] {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            let inner: String = chars[start..i.saturating_sub(1)].iter().collect();
            if ty == "Unsupported" {
                unsupported = literal_str(&inner).or(Some(inner));
            }
            ty = "Unsupported".to_string();
        }
        let mut optional = false;
        let mut list = false;
        while i < chars.len() {
            match chars[i] {
                '?' => {
                    optional = true;
                    i += 1;
                }
                '[' if i + 1 < chars.len() && chars[i + 1] == ']' => {
                    list = true;
                    i += 2;
                }
                _ => break,
            }
        }
        let rest: String = chars[i..].iter().collect();
        let attrs = parse_attrs(&rest);
        let db_name = attrs
            .iter()
            .find(|a| a.name == "map")
            .and_then(|a| literal_str(&a.args))
            .unwrap_or_else(|| fname.clone());
        fields.push(Field {
            name: fname,
            db_name,
            ty,
            unsupported,
            optional,
            list,
            attrs,
        });
    }
    let table = block_attrs
        .iter()
        .find(|a| a.name == "map")
        .and_then(|a| literal_str(&a.args))
        .unwrap_or_else(|| name.to_string());
    Model {
        name: name.to_string(),
        table,
        fields,
        block_attrs,
    }
}

fn parse_enum(name: &str, body: &str) -> EnumDef {
    let mut values = Vec::new();
    let mut db_name = name.to_string();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("@@") {
            for a in parse_attrs(line) {
                if a.name == "map" {
                    if let Some(v) = literal_str(&a.args) {
                        db_name = v;
                    }
                }
            }
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0usize;
        let v = read_ident(&chars, &mut i);
        if v.is_empty() {
            continue;
        }
        let rest: String = chars[i..].iter().collect();
        let mapped = parse_attrs(&rest)
            .iter()
            .find(|a| a.name == "map")
            .and_then(|a| literal_str(&a.args))
            .unwrap_or_else(|| v.clone());
        values.push((v, mapped));
    }
    EnumDef {
        name: name.to_string(),
        db_name,
        values,
    }
}

// ---------------------------------------------------------------------------
// SQL rendering helpers
// ---------------------------------------------------------------------------

struct Opts {
    dialect: Dialect,
    if_not_exists: bool,
    quote: bool,
}

fn quote_ident(name: &str, o: &Opts) -> String {
    if !o.quote {
        return name.to_string();
    }
    match o.dialect {
        Dialect::Postgres | Dialect::Sqlite => format!("\"{}\"", name.replace('"', "\"\"")),
        Dialect::Mysql => format!("`{}`", name.replace('`', "``")),
        Dialect::SqlServer => format!("[{}]", name.replace(']', "]]")),
    }
}

fn sql_string(v: &str) -> String {
    format!("'{}'", v.replace('\'', "''"))
}

fn scalar_type(ty: &str, d: Dialect) -> Result<String, String> {
    let s = match (ty, d) {
        ("String", Dialect::Postgres) => "TEXT",
        ("String", Dialect::Mysql) => "VARCHAR(191)",
        ("String", Dialect::Sqlite) => "TEXT",
        ("String", Dialect::SqlServer) => "NVARCHAR(1000)",
        ("Boolean", Dialect::Postgres) => "BOOLEAN",
        ("Boolean", Dialect::Mysql) => "TINYINT(1)",
        ("Boolean", Dialect::Sqlite) => "BOOLEAN",
        ("Boolean", Dialect::SqlServer) => "BIT",
        ("Int", Dialect::Postgres) => "INTEGER",
        ("Int", Dialect::Mysql) => "INT",
        ("Int", Dialect::Sqlite) => "INTEGER",
        ("Int", Dialect::SqlServer) => "INT",
        ("BigInt", _) => "BIGINT",
        ("Float", Dialect::Postgres) => "DOUBLE PRECISION",
        ("Float", Dialect::Mysql) => "DOUBLE",
        ("Float", Dialect::Sqlite) => "REAL",
        ("Float", Dialect::SqlServer) => "FLOAT(53)",
        ("Decimal", Dialect::Postgres) => "DECIMAL(65,30)",
        ("Decimal", Dialect::Mysql) => "DECIMAL(65,30)",
        ("Decimal", Dialect::Sqlite) => "DECIMAL",
        ("Decimal", Dialect::SqlServer) => "DECIMAL(32,16)",
        ("DateTime", Dialect::Postgres) => "TIMESTAMP(3)",
        ("DateTime", Dialect::Mysql) => "DATETIME(3)",
        ("DateTime", Dialect::Sqlite) => "DATETIME",
        ("DateTime", Dialect::SqlServer) => "DATETIME2",
        ("Json", Dialect::Postgres) => "JSONB",
        ("Json", Dialect::Mysql) => "JSON",
        ("Json", Dialect::Sqlite) => "JSONB",
        ("Json", Dialect::SqlServer) => {
            return Err(
                "Json fields are not supported on sqlserver: change the field type or pick another dialect"
                    .to_string(),
            )
        }
        ("Bytes", Dialect::Postgres) => "BYTEA",
        ("Bytes", Dialect::Mysql) => "LONGBLOB",
        ("Bytes", Dialect::Sqlite) => "BLOB",
        ("Bytes", Dialect::SqlServer) => "VARBINARY(MAX)",
        _ => {
            return Err(format!(
                "unknown field type `{ty}`: expected a Prisma scalar (String, Boolean, Int, BigInt, Float, Decimal, DateTime, Json, Bytes), an enum, or a model defined in this schema"
            ))
        }
    };
    Ok(s.to_string())
}

/// `@db.VarChar(255)` → `VARCHAR(255)`.
fn native_type(attr: &Attr) -> String {
    let n = attr.name.trim_start_matches("db.");
    let base = match n {
        "DoublePrecision" => "DOUBLE PRECISION",
        "SmallInt" => "SMALLINT",
        "TinyInt" => "TINYINT",
        "MediumInt" => "MEDIUMINT",
        "UnsignedInt" => "INT UNSIGNED",
        "UnsignedSmallInt" => "SMALLINT UNSIGNED",
        "UnsignedTinyInt" => "TINYINT UNSIGNED",
        "UnsignedMediumInt" => "MEDIUMINT UNSIGNED",
        "UnsignedBigInt" => "BIGINT UNSIGNED",
        "VarChar" => "VARCHAR",
        "NVarChar" => "NVARCHAR",
        "Char" => "CHAR",
        "NChar" => "NCHAR",
        "TinyText" => "TINYTEXT",
        "MediumText" => "MEDIUMTEXT",
        "LongText" => "LONGTEXT",
        "NText" => "NTEXT",
        "Uuid" => "UUID",
        "UniqueIdentifier" => "UNIQUEIDENTIFIER",
        "JsonB" => "JSONB",
        "ByteA" => "BYTEA",
        "VarBit" => "VARBIT",
        "Citext" => "CITEXT",
        "Timetz" => "TIMETZ",
        "Timestamptz" => "TIMESTAMPTZ",
        "DateTime2" => "DATETIME2",
        "SmallDateTime" => "SMALLDATETIME",
        "VarBinary" => "VARBINARY",
        "TinyBlob" => "TINYBLOB",
        "MediumBlob" => "MEDIUMBLOB",
        "LongBlob" => "LONGBLOB",
        "Bool" => "BOOLEAN",
        other => return with_args(&other.to_uppercase(), &attr.args),
    };
    with_args(base, &attr.args)
}

fn with_args(base: &str, args: &str) -> String {
    let a = args.trim();
    if a.is_empty() {
        base.to_string()
    } else {
        format!("{base}({a})")
    }
}

/// Referential action keyword → SQL.
fn referential_action(a: &str) -> Option<&'static str> {
    match a.trim() {
        "Cascade" => Some("CASCADE"),
        "Restrict" => Some("RESTRICT"),
        "NoAction" => Some("NO ACTION"),
        "SetNull" => Some("SET NULL"),
        "SetDefault" => Some("SET DEFAULT"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// Convert a Prisma schema into `CREATE TABLE` DDL.
///
/// * `input` — the `schema.prisma` text.
/// * `dialect` — `auto` | `postgresql` | `mysql` | `sqlite` | `sqlserver`
///   (`auto` reads the schema's `datasource` provider, falling back to
///   PostgreSQL).
/// * `foreign_keys` — emit `ALTER TABLE … ADD CONSTRAINT … FOREIGN KEY`.
/// * `indexes` — emit `CREATE [UNIQUE] INDEX` for `@unique`/`@@unique`/`@@index`.
/// * `if_not_exists` — guard `CREATE TABLE` (and, where supported, indexes).
/// * `drop_if_exists` — prepend `DROP TABLE IF EXISTS` statements.
/// * `quote_identifiers` — quote table/column names with the dialect's quoting.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    dialect: &str,
    foreign_keys: bool,
    indexes: bool,
    if_not_exists: bool,
    drop_if_exists: bool,
    quote_identifiers: bool,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty: paste a Prisma schema containing at least one `model` block".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "schema is too large: {} bytes (limit {MAX_INPUT_BYTES})",
            input.len()
        ));
    }
    let requested = Dialect::parse(dialect)?;

    let cleaned = strip_comments(input);
    let blocks = split_blocks(&cleaned)?;

    let mut models: Vec<Model> = Vec::new();
    let mut enums: Vec<EnumDef> = Vec::new();
    let mut datasource_provider: Option<String> = None;
    for b in &blocks {
        match b.kind.as_str() {
            "model" | "view" => models.push(parse_model(&b.name, &b.body)),
            "enum" => enums.push(parse_enum(&b.name, &b.body)),
            "datasource" => {
                for line in b.body.lines() {
                    let line = line.trim();
                    if let Some(rest) = line.strip_prefix("provider") {
                        if let Some(v) = rest.trim().strip_prefix('=') {
                            datasource_provider = literal_str(v).or(Some(v.trim().to_string()));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let dialect = match requested {
        Some(d) => d,
        None => match datasource_provider.as_deref() {
            Some(p) => Dialect::parse(p).ok().flatten().unwrap_or(Dialect::Postgres),
            None => Dialect::Postgres,
        },
    };

    if models.is_empty() {
        return Err(
            "no `model` blocks found: the input does not look like a Prisma schema (expected e.g. `model User { id Int @id }`)"
                .into(),
        );
    }

    let o = Opts {
        dialect,
        if_not_exists,
        quote: quote_identifiers,
    };

    let mut drops: Vec<String> = Vec::new();
    let mut types: Vec<String> = Vec::new();
    let mut tables: Vec<String> = Vec::new();
    let mut index_stmts: Vec<String> = Vec::new();
    let mut fk_stmts: Vec<String> = Vec::new();

    // --- enums -------------------------------------------------------------
    if dialect == Dialect::Postgres {
        for e in &enums {
            if e.values.is_empty() {
                continue;
            }
            let vals: Vec<String> = e.values.iter().map(|(_, db)| sql_string(db)).collect();
            types.push(format!(
                "CREATE TYPE {} AS ENUM ({});",
                quote_ident(&e.db_name, &o),
                vals.join(", ")
            ));
        }
    }

    // --- tables ------------------------------------------------------------
    for m in &models {
        let mut cols: Vec<String> = Vec::new();
        let mut pk_cols: Vec<String> = Vec::new();
        let mut inline_pk = false;

        // Composite @@id wins over a field-level @id when both are present.
        let composite_id: Vec<String> = m
            .block_attrs_named("id")
            .next()
            .map(|a| a.ident_list(None))
            .unwrap_or_default();

        for f in &m.fields {
            if models.iter().any(|x| x.name == f.ty) {
                continue; // relation field — no column of its own
            }
            if f.list && dialect != Dialect::Postgres {
                cols.push(format!(
                    "-- skipped list field `{}`: scalar lists exist only on PostgreSQL",
                    f.name
                ));
                continue;
            }
            let enum_def = enums.iter().find(|e| e.name == f.ty);
            let mut ty_sql = if let Some(u) = &f.unsupported {
                u.clone()
            } else if let Some(nat) = f.native() {
                native_type(nat)
            } else if let Some(e) = enum_def {
                match dialect {
                    Dialect::Postgres => quote_ident(&e.db_name, &o),
                    Dialect::Mysql => {
                        let vals: Vec<String> =
                            e.values.iter().map(|(_, db)| sql_string(db)).collect();
                        format!("ENUM({})", vals.join(", "))
                    }
                    Dialect::Sqlite => "TEXT".to_string(),
                    Dialect::SqlServer => "NVARCHAR(1000)".to_string(),
                }
            } else {
                scalar_type(&f.ty, dialect)?
            };
            if f.list && dialect == Dialect::Postgres {
                ty_sql.push_str("[]");
            }

            let is_pk_field = f.attr("id").is_some() && composite_id.is_empty();
            let mut parts = vec![quote_ident(&f.db_name, &o), String::new()];
            let auto = f.is_autoincrement();

            if auto {
                match dialect {
                    Dialect::Postgres => {
                        ty_sql = if f.ty == "BigInt" { "BIGSERIAL" } else { "SERIAL" }.to_string();
                    }
                    Dialect::Sqlite if is_pk_field => {
                        ty_sql = "INTEGER".to_string();
                        inline_pk = true;
                    }
                    _ => {}
                }
            }
            parts[1] = ty_sql;
            let mut col = parts.join(" ");
            if !f.optional {
                col.push_str(" NOT NULL");
            }
            if auto {
                match dialect {
                    Dialect::Mysql => col.push_str(" AUTO_INCREMENT"),
                    Dialect::SqlServer => col.push_str(" IDENTITY(1,1)"),
                    Dialect::Sqlite if is_pk_field => col.push_str(" PRIMARY KEY AUTOINCREMENT"),
                    _ => {}
                }
            } else if let Some(expr) = f.default_expr() {
                if let Some(d) = render_default(&expr, dialect, enum_def) {
                    col.push_str(&format!(" DEFAULT {d}"));
                }
            }
            // enums without a native enum type get a CHECK constraint instead
            if let Some(e) = enum_def {
                if matches!(dialect, Dialect::Sqlite | Dialect::SqlServer) && !e.values.is_empty() {
                    let vals: Vec<String> =
                        e.values.iter().map(|(_, db)| sql_string(db)).collect();
                    col.push_str(&format!(
                        " CHECK ({} IN ({}))",
                        quote_ident(&f.db_name, &o),
                        vals.join(", ")
                    ));
                }
            }
            cols.push(col);

            if is_pk_field && !inline_pk {
                pk_cols.push(f.db_name.clone());
            }
        }

        if !composite_id.is_empty() {
            for name in &composite_id {
                if let Some(f) = m.field(name) {
                    pk_cols.push(f.db_name.clone());
                }
            }
        }
        if !pk_cols.is_empty() {
            let quoted: Vec<String> = pk_cols.iter().map(|c| quote_ident(c, &o)).collect();
            let pk = match dialect {
                Dialect::Postgres | Dialect::SqlServer => format!(
                    "CONSTRAINT {} PRIMARY KEY ({})",
                    quote_ident(&format!("{}_pkey", m.table), &o),
                    quoted.join(", ")
                ),
                _ => format!("PRIMARY KEY ({})", quoted.join(", ")),
            };
            cols.push(pk);
        }

        tables.push(render_create_table(&m.table, &cols, &o));

        if drop_if_exists {
            drops.push(drop_table(&m.table, &o));
        }

        // --- indexes -------------------------------------------------------
        if indexes {
            for f in &m.fields {
                if models.iter().any(|x| x.name == f.ty) {
                    continue;
                }
                if let Some(a) = f.attr("unique") {
                    let name = a
                        .named("map")
                        .and_then(|v| literal_str(&v))
                        .unwrap_or_else(|| format!("{}_{}_key", m.table, f.db_name));
                    index_stmts.push(render_index(&name, &m.table, &[f.db_name.clone()], true, &o));
                }
            }
            for a in m.block_attrs_named("unique") {
                let cols = resolve_cols(m, &a.ident_list(None));
                if cols.is_empty() {
                    continue;
                }
                let name = a
                    .named("map")
                    .and_then(|v| literal_str(&v))
                    .unwrap_or_else(|| format!("{}_{}_key", m.table, cols.join("_")));
                index_stmts.push(render_index(&name, &m.table, &cols, true, &o));
            }
            for a in m.block_attrs_named("index") {
                let cols = resolve_cols(m, &a.ident_list(None));
                if cols.is_empty() {
                    continue;
                }
                let name = a
                    .named("map")
                    .and_then(|v| literal_str(&v))
                    .unwrap_or_else(|| format!("{}_{}_idx", m.table, cols.join("_")));
                index_stmts.push(render_index(&name, &m.table, &cols, false, &o));
            }
        }
    }

    // --- foreign keys ------------------------------------------------------
    if foreign_keys {
        for m in &models {
            for f in &m.fields {
                let target = match models.iter().find(|x| x.name == f.ty) {
                    Some(t) => t,
                    None => continue,
                };
                let rel = match f.attr("relation") {
                    Some(r) => r,
                    None => continue,
                };
                let local = resolve_cols(m, &rel.ident_list(Some("fields")));
                if local.is_empty() {
                    continue; // back-relation side: the FK lives on the other model
                }
                let refs = rel.ident_list(Some("references"));
                let remote: Vec<String> = refs
                    .iter()
                    .map(|r| {
                        target
                            .field(r)
                            .map(|tf| tf.db_name.clone())
                            .unwrap_or_else(|| r.clone())
                    })
                    .collect();
                if remote.is_empty() {
                    continue;
                }
                let on_delete = rel
                    .named("onDelete")
                    .and_then(|v| referential_action(&v))
                    .unwrap_or(if f.optional { "SET NULL" } else { "RESTRICT" });
                let on_update = rel
                    .named("onUpdate")
                    .and_then(|v| referential_action(&v))
                    .unwrap_or("CASCADE");
                let name = rel
                    .named("map")
                    .and_then(|v| literal_str(&v))
                    .unwrap_or_else(|| format!("{}_{}_fkey", m.table, local.join("_")));
                fk_stmts.push(format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {} ON UPDATE {};",
                    quote_ident(&m.table, &o),
                    quote_ident(&name, &o),
                    local.iter().map(|c| quote_ident(c, &o)).collect::<Vec<_>>().join(", "),
                    quote_ident(&target.table, &o),
                    remote.iter().map(|c| quote_ident(c, &o)).collect::<Vec<_>>().join(", "),
                    on_delete,
                    on_update
                ));
            }
        }
    }

    // --- implicit many-to-many join tables ---------------------------------
    for (join_table, a_model, a_col_ty, b_model, b_col_ty) in implicit_m2m(&models, dialect)? {
        let cols = vec![
            format!("{} {} NOT NULL", quote_ident("A", &o), a_col_ty),
            format!("{} {} NOT NULL", quote_ident("B", &o), b_col_ty),
            match dialect {
                Dialect::Postgres | Dialect::SqlServer => format!(
                    "CONSTRAINT {} PRIMARY KEY ({}, {})",
                    quote_ident(&format!("{join_table}_AB_pkey"), &o),
                    quote_ident("A", &o),
                    quote_ident("B", &o)
                ),
                _ => format!(
                    "PRIMARY KEY ({}, {})",
                    quote_ident("A", &o),
                    quote_ident("B", &o)
                ),
            },
        ];
        tables.push(render_create_table(&join_table, &cols, &o));
        if drop_if_exists {
            drops.push(drop_table(&join_table, &o));
        }
        if indexes {
            index_stmts.push(render_index(
                &format!("{join_table}_B_index"),
                &join_table,
                &["B".to_string()],
                false,
                &o,
            ));
        }
        if foreign_keys {
            for (col, target) in [("A", &a_model), ("B", &b_model)] {
                let tm = models.iter().find(|x| &x.name == target).unwrap();
                let pk = model_pk(tm).unwrap_or_else(|| "id".to_string());
                fk_stmts.push(format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE CASCADE ON UPDATE CASCADE;",
                    quote_ident(&join_table, &o),
                    quote_ident(&format!("{join_table}_{col}_fkey"), &o),
                    quote_ident(col, &o),
                    quote_ident(&tm.table, &o),
                    quote_ident(&pk, &o)
                ));
            }
        }
    }

    // --- assemble ----------------------------------------------------------
    let mut sections: Vec<String> = Vec::new();
    if !drops.is_empty() {
        drops.reverse();
        if dialect == Dialect::Postgres {
            for e in &enums {
                if !e.values.is_empty() {
                    drops.push(format!(
                        "DROP TYPE IF EXISTS {};",
                        quote_ident(&e.db_name, &o)
                    ));
                }
            }
        }
        sections.push(drops.join("\n"));
    }
    if !types.is_empty() {
        sections.push(types.join("\n"));
    }
    sections.push(tables.join("\n\n"));
    if !index_stmts.is_empty() {
        sections.push(index_stmts.join("\n"));
    }
    if !fk_stmts.is_empty() {
        sections.push(fk_stmts.join("\n"));
    }
    Ok(format!("{}\n", sections.join("\n\n")))
}

fn render_create_table(table: &str, cols: &[String], o: &Opts) -> String {
    let name = quote_ident(table, o);
    let body: Vec<String> = cols
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let last = i + 1 == cols.len();
            let comma = if last || c.starts_with("--") { "" } else { "," };
            format!("    {c}{comma}")
        })
        .collect();
    let head = if o.if_not_exists {
        match o.dialect {
            Dialect::SqlServer => format!(
                "IF OBJECT_ID(N'{}', N'U') IS NULL\nCREATE TABLE {name} (",
                table.replace('\'', "''")
            ),
            _ => format!("CREATE TABLE IF NOT EXISTS {name} ("),
        }
    } else {
        format!("CREATE TABLE {name} (")
    };
    format!("{head}\n{}\n);", body.join("\n"))
}

fn drop_table(table: &str, o: &Opts) -> String {
    match o.dialect {
        Dialect::Postgres => format!("DROP TABLE IF EXISTS {} CASCADE;", quote_ident(table, o)),
        _ => format!("DROP TABLE IF EXISTS {};", quote_ident(table, o)),
    }
}

fn render_index(name: &str, table: &str, cols: &[String], unique: bool, o: &Opts) -> String {
    let kind = if unique { "CREATE UNIQUE INDEX" } else { "CREATE INDEX" };
    let guard = if o.if_not_exists && matches!(o.dialect, Dialect::Postgres | Dialect::Sqlite) {
        "IF NOT EXISTS "
    } else {
        ""
    };
    format!(
        "{kind} {guard}{} ON {}({});",
        quote_ident(name, o),
        quote_ident(table, o),
        cols.iter()
            .map(|c| quote_ident(c, o))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Map Prisma field names to database column names for a model.
fn resolve_cols(m: &Model, names: &[String]) -> Vec<String> {
    names
        .iter()
        .map(|n| {
            m.field(n)
                .map(|f| f.db_name.clone())
                .unwrap_or_else(|| n.clone())
        })
        .collect()
}

/// Database column name of a model's single-field primary key.
fn model_pk(m: &Model) -> Option<String> {
    m.fields
        .iter()
        .find(|f| f.attr("id").is_some())
        .map(|f| f.db_name.clone())
}

/// SQL column type of a model's primary key, for join tables.
fn model_pk_type(m: &Model, d: Dialect) -> Result<String, String> {
    let f = m
        .fields
        .iter()
        .find(|f| f.attr("id").is_some())
        .ok_or_else(|| {
            format!(
                "model `{}` takes part in an implicit many-to-many relation but has no `@id` field",
                m.name
            )
        })?;
    if let Some(nat) = f.native() {
        return Ok(native_type(nat));
    }
    scalar_type(&f.ty, d)
}

type M2m = (String, String, String, String, String);

/// Implicit many-to-many relations (list on both sides, no `fields:`) become a
/// `_AToB` join table, matching Prisma's own convention.
fn implicit_m2m(models: &[Model], d: Dialect) -> Result<Vec<M2m>, String> {
    let mut out: Vec<M2m> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for m in models {
        for f in &m.fields {
            if !f.list {
                continue;
            }
            let target = match models.iter().find(|x| x.name == f.ty) {
                Some(t) => t,
                None => continue,
            };
            let rel = f.attr("relation");
            if rel.map(|r| !r.ident_list(Some("fields")).is_empty()) == Some(true) {
                continue;
            }
            let rel_name = rel
                .and_then(|r| r.positional())
                .and_then(|p| literal_str(&p));
            // the other side must also be a list for this to be m-n
            let back = target.fields.iter().find(|bf| {
                bf.ty == m.name
                    && bf.list
                    && bf
                        .attr("relation")
                        .and_then(|r| r.positional())
                        .and_then(|p| literal_str(&p))
                        == rel_name
            });
            if back.is_none() {
                continue;
            }
            let (first, second) = if m.name <= target.name {
                (m, target)
            } else {
                (target, m)
            };
            let join = match &rel_name {
                Some(n) => format!("_{n}"),
                None => format!("_{}To{}", first.name, second.name),
            };
            if seen.contains(&join) {
                continue;
            }
            seen.push(join.clone());
            out.push((
                join,
                first.name.clone(),
                model_pk_type(first, d)?,
                second.name.clone(),
                model_pk_type(second, d)?,
            ));
        }
    }
    Ok(out)
}

/// Render a Prisma `@default(...)` expression as a SQL `DEFAULT` value.
/// Client-side generators (`uuid()`, `cuid()`, `ulid()`, `nanoid()`, `auto()`)
/// produce no database default — Prisma fills them in application code.
fn render_default(expr: &str, d: Dialect, enum_def: Option<&EnumDef>) -> Option<String> {
    let e = expr.trim();
    let bare = e.replace(' ', "");
    if bare.is_empty() || bare.starts_with('[') {
        return None;
    }
    if bare.starts_with("uuid(")
        || bare.starts_with("cuid(")
        || bare.starts_with("ulid(")
        || bare.starts_with("nanoid(")
        || bare.starts_with("auto(")
        || bare.starts_with("sequence(")
    {
        return None;
    }
    if bare == "now()" {
        return Some(match d {
            Dialect::Mysql => "CURRENT_TIMESTAMP(3)".to_string(),
            _ => "CURRENT_TIMESTAMP".to_string(),
        });
    }
    if let Some(inner) = e.strip_prefix("dbgenerated(") {
        let inner = inner.trim_end().trim_end_matches(')');
        return literal_str(inner).filter(|s| !s.is_empty());
    }
    if let Some(s) = literal_str(e) {
        return Some(sql_string(&s));
    }
    match bare.as_str() {
        "true" => {
            return Some(match d {
                Dialect::SqlServer => "1".to_string(),
                _ => "true".to_string(),
            })
        }
        "false" => {
            return Some(match d {
                Dialect::SqlServer => "0".to_string(),
                _ => "false".to_string(),
            })
        }
        _ => {}
    }
    if bare.parse::<f64>().is_ok() {
        return Some(bare);
    }
    if let Some(en) = enum_def {
        if let Some((_, db)) = en.values.iter().find(|(v, _)| v == &bare) {
            return Some(sql_string(db));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(schema: &str, dialect: &str) -> String {
        convert(schema, dialect, true, true, false, false, true).unwrap()
    }

    const BLOG: &str = r#"
        datasource db {
          provider = "postgresql"
          url      = env("DATABASE_URL")
        }

        model User {
          id    Int     @id @default(autoincrement())
          email String  @unique
          name  String?
          posts Post[]
        }

        model Post {
          id        Int      @id @default(autoincrement())
          title     String   @db.VarChar(200)
          published Boolean  @default(false)
          createdAt DateTime @default(now())
          author    User     @relation(fields: [authorId], references: [id], onDelete: Cascade)
          authorId  Int

          @@index([authorId])
        }
    "#;

    #[test]
    fn postgres_happy_path() {
        let out = run(BLOG, "postgresql");
        assert!(out.contains("CREATE TABLE \"User\" ("), "{out}");
        assert!(out.contains("\"id\" SERIAL NOT NULL"), "{out}");
        assert!(out.contains("\"email\" TEXT NOT NULL"), "{out}");
        assert!(out.contains("\"name\" TEXT,"), "{out}");
        assert!(
            out.contains("CONSTRAINT \"User_pkey\" PRIMARY KEY (\"id\")"),
            "{out}"
        );
        assert!(out.contains("\"title\" VARCHAR(200) NOT NULL"), "{out}");
        assert!(out.contains("\"published\" BOOLEAN NOT NULL DEFAULT false"), "{out}");
        assert!(
            out.contains("\"createdAt\" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP"),
            "{out}"
        );
        assert!(
            out.contains("CREATE UNIQUE INDEX \"User_email_key\" ON \"User\"(\"email\");"),
            "{out}"
        );
        assert!(
            out.contains("CREATE INDEX \"Post_authorId_idx\" ON \"Post\"(\"authorId\");"),
            "{out}"
        );
        assert!(
            out.contains("ALTER TABLE \"Post\" ADD CONSTRAINT \"Post_authorId_fkey\" FOREIGN KEY (\"authorId\") REFERENCES \"User\"(\"id\") ON DELETE CASCADE ON UPDATE CASCADE;"),
            "{out}"
        );
        // the relation field itself is not a column
        assert!(!out.contains("\"author\""), "{out}");
    }

    #[test]
    fn auto_dialect_reads_the_datasource_provider() {
        let schema = r#"
            datasource db { provider = "mysql" }
            model A {
              id   Int    @id @default(autoincrement())
              name String
            }
        "#;
        let out = run(schema, "auto");
        assert!(out.contains("`id` INT NOT NULL AUTO_INCREMENT"), "{out}");
        assert!(out.contains("`name` VARCHAR(191) NOT NULL"), "{out}");
        assert!(out.contains("PRIMARY KEY (`id`)"), "{out}");
    }

    #[test]
    fn sqlite_autoincrement_is_inline() {
        let out = run("model A { id Int @id @default(autoincrement()) }", "sqlite");
        assert!(
            out.contains("\"id\" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            "{out}"
        );
        assert!(!out.contains("CONSTRAINT"), "{out}");
    }

    #[test]
    fn sqlserver_identity_and_bit_default() {
        let out = run(
            "model A {\n  id Int @id @default(autoincrement())\n  ok Boolean @default(true)\n}",
            "sqlserver",
        );
        assert!(out.contains("[id] INT NOT NULL IDENTITY(1,1)"), "{out}");
        assert!(out.contains("[ok] BIT NOT NULL DEFAULT 1"), "{out}");
        assert!(out.contains("CONSTRAINT [A_pkey] PRIMARY KEY ([id])"), "{out}");
    }

    #[test]
    fn enums_become_a_type_on_postgres_and_a_check_elsewhere() {
        let schema = r#"
            enum Role {
              USER
              ADMIN
            }
            model A {
              id   Int  @id
              role Role @default(USER)
            }
        "#;
        let pg = run(schema, "postgresql");
        assert!(
            pg.contains("CREATE TYPE \"Role\" AS ENUM ('USER', 'ADMIN');"),
            "{pg}"
        );
        assert!(pg.contains("\"role\" \"Role\" NOT NULL DEFAULT 'USER'"), "{pg}");
        let my = run(schema, "mysql");
        assert!(my.contains("`role` ENUM('USER', 'ADMIN') NOT NULL"), "{my}");
        let lite = run(schema, "sqlite");
        assert!(
            lite.contains("CHECK (\"role\" IN ('USER', 'ADMIN'))"),
            "{lite}"
        );
    }

    #[test]
    fn map_and_composite_attributes() {
        let schema = r#"
            model UserRole {
              userId Int
              roleId Int
              label  String @map("label_text")

              @@id([userId, roleId])
              @@map("user_roles")
              @@unique([label])
            }
        "#;
        let out = run(schema, "postgresql");
        assert!(out.contains("CREATE TABLE \"user_roles\" ("), "{out}");
        assert!(out.contains("\"label_text\" TEXT NOT NULL"), "{out}");
        assert!(
            out.contains("CONSTRAINT \"user_roles_pkey\" PRIMARY KEY (\"userId\", \"roleId\")"),
            "{out}"
        );
        assert!(
            out.contains("CREATE UNIQUE INDEX \"user_roles_label_text_key\" ON \"user_roles\"(\"label_text\");"),
            "{out}"
        );
    }

    #[test]
    fn implicit_many_to_many_join_table() {
        let schema = r#"
            model Post {
              id   Int   @id
              tags Tag[]
            }
            model Tag {
              id    Int    @id
              posts Post[]
            }
        "#;
        let out = run(schema, "postgresql");
        assert!(out.contains("CREATE TABLE \"_PostToTag\" ("), "{out}");
        assert!(out.contains("\"A\" INTEGER NOT NULL"), "{out}");
        assert!(
            out.contains("CONSTRAINT \"_PostToTag_AB_pkey\" PRIMARY KEY (\"A\", \"B\")"),
            "{out}"
        );
        assert!(
            out.contains("CREATE INDEX \"_PostToTag_B_index\" ON \"_PostToTag\"(\"B\");"),
            "{out}"
        );
        assert!(
            out.contains("ALTER TABLE \"_PostToTag\" ADD CONSTRAINT \"_PostToTag_A_fkey\""),
            "{out}"
        );
    }

    #[test]
    fn toggles_drop_sections() {
        let out = convert(BLOG, "postgresql", false, false, true, true, false).unwrap();
        assert!(out.contains("DROP TABLE IF EXISTS Post CASCADE;"), "{out}");
        assert!(out.contains("CREATE TABLE IF NOT EXISTS User ("), "{out}");
        assert!(!out.contains("CREATE UNIQUE INDEX"), "{out}");
        assert!(!out.contains("ALTER TABLE"), "{out}");
        assert!(!out.contains('"'), "{out}");
    }

    #[test]
    fn optional_relation_defaults_to_set_null() {
        let schema = r#"
            model User { id Int @id posts Post[] }
            model Post {
              id       Int   @id
              author   User? @relation(fields: [authorId], references: [id])
              authorId Int?
            }
        "#;
        let out = run(schema, "postgresql");
        assert!(out.contains("ON DELETE SET NULL ON UPDATE CASCADE;"), "{out}");
    }

    #[test]
    fn comments_and_unsupported_types_survive() {
        let schema = r#"
            /// a doc comment
            model A {
              id  Int    @id
              // a line comment
              loc Unsupported("polygon")?
              raw String @default(dbgenerated("gen_random_uuid()"))
            }
        "#;
        let out = run(schema, "postgresql");
        assert!(out.contains("\"loc\" polygon,"), "{out}");
        assert!(
            out.contains("\"raw\" TEXT NOT NULL DEFAULT gen_random_uuid()"),
            "{out}"
        );
    }

    #[test]
    fn err_on_empty_input() {
        let e = convert("   ", "postgresql", true, true, false, false, true).unwrap_err();
        assert!(e.contains("input is empty"), "{e}");
    }

    #[test]
    fn err_when_no_models() {
        let e = convert(
            "generator client { provider = \"prisma-client-js\" }",
            "postgresql",
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap_err();
        assert!(e.contains("no `model` blocks"), "{e}");
    }

    #[test]
    fn err_on_bad_dialect() {
        let e = convert("model A { id Int @id }", "oracle", true, true, false, false, true)
            .unwrap_err();
        assert!(e.contains("invalid dialect"), "{e}");
    }

    #[test]
    fn err_on_unknown_field_type() {
        let e = convert(
            "model A {\n  id Int @id\n  thing Widget\n}",
            "postgresql",
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap_err();
        assert!(e.contains("unknown field type `Widget`"), "{e}");
    }

    #[test]
    fn err_on_json_for_sqlserver() {
        let e = convert(
            "model A {\n  id Int @id\n  meta Json\n}",
            "sqlserver",
            true,
            true,
            false,
            false,
            true,
        )
        .unwrap_err();
        assert!(e.contains("Json fields are not supported on sqlserver"), "{e}");
    }

    #[test]
    fn err_when_input_exceeds_cap() {
        let big = format!(
            "model A {{ id Int @id }}\n{}",
            "// pad\n".repeat(MAX_INPUT_BYTES / 7 + 2)
        );
        let e = convert(&big, "postgresql", true, true, false, false, true).unwrap_err();
        assert!(e.contains("schema is too large"), "{e}");
    }
}
