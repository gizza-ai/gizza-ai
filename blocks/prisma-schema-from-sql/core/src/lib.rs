//! prisma-schema-from-sql core — turn SQL `CREATE TABLE` / `ALTER TABLE` DDL
//! into a Prisma `schema.prisma` (models, fields, attributes, relations). Pure
//! compute, no wafer/wasm-bindgen deps; shared by the chat skill block and the
//! web page.
//!
//! The DDL parsing itself is reused from `blocks/sql-schema-extractor` (a
//! lenient parser that survives a real dump): we ask it for its JSON model and
//! map that onto Prisma. This crate therefore owns only the SQL→Prisma type
//! mapping, attribute rendering, and relation inference — never SQL execution.

use serde_json::Value;

/// Which datasource provider to emit, and how to interpret the DDL.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Provider {
    Postgresql,
    Mysql,
    Sqlite,
    Sqlserver,
}

impl Provider {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "postgresql" | "postgres" | "pg" => Ok(Provider::Postgresql),
            "mysql" | "mariadb" => Ok(Provider::Mysql),
            "sqlite" | "sqlite3" => Ok(Provider::Sqlite),
            "sqlserver" | "mssql" | "tsql" => Ok(Provider::Sqlserver),
            other => Err(format!(
                "invalid provider {other:?}: expected postgresql, mysql, sqlite, or sqlserver"
            )),
        }
    }
    /// Datasource `provider = "…"` value (Prisma's spelling).
    fn prisma_name(self) -> &'static str {
        match self {
            Provider::Postgresql => "postgresql",
            Provider::Mysql => "mysql",
            Provider::Sqlite => "sqlite",
            Provider::Sqlserver => "sqlserver",
        }
    }
    /// Dialect hint passed to the reused DDL parser.
    fn dialect(self) -> &'static str {
        match self {
            Provider::Postgresql => "postgres",
            Provider::Mysql => "mysql",
            Provider::Sqlite => "sqlite",
            Provider::Sqlserver => "mssql",
        }
    }
    /// SQLite has no native-type attributes in Prisma.
    fn supports_native_types(self) -> bool {
        !matches!(self, Provider::Sqlite)
    }
}

/// Options controlling the generated schema (defaults mirror the descriptor).
struct Options {
    provider: Provider,
    /// Emit `generator client` + `datasource db` header blocks.
    header: bool,
    /// Infer `@relation` fields from foreign keys.
    relations: bool,
    /// Emit native DB types (`@db.VarChar(n)`, `@db.Char(n)`, `@db.Decimal(p,s)`).
    native_types: bool,
    /// PascalCase model names + camelCase field names with `@map`/`@@map`.
    map_names: bool,
}

/// Convert `sql` DDL into a Prisma schema string.
pub fn convert(
    sql: &str,
    provider: &str,
    header: bool,
    relations: bool,
    native_types: bool,
    map_names: bool,
) -> Result<String, String> {
    let provider = Provider::parse(provider)?;
    let opts = Options {
        provider,
        header,
        relations,
        native_types,
        map_names,
    };

    // Reuse the lenient DDL parser for the heavy lifting; ask for its JSON model
    // with ALTER folded in and indexes included.
    let model_json =
        gizza_ai_sql_schema_extractor_core::extract(sql, "json", provider.dialect(), true, true)?;
    let model: Value = serde_json::from_str(&model_json)
        .map_err(|e| format!("internal: could not parse schema model: {e}"))?;

    let tables = model["tables"]
        .as_array()
        .ok_or("internal: schema model had no tables array")?;

    // First pass: assign each SQL table a Prisma model name so relations can
    // reference them (case-insensitively).
    let model_names: Vec<(String, String)> = tables
        .iter()
        .map(|t| {
            let sql_name = t["name"].as_str().unwrap_or("Table").to_string();
            (sql_name.clone(), model_name(&sql_name, opts.map_names))
        })
        .collect();
    let lookup = |sql_name: &str| -> Option<String> {
        model_names
            .iter()
            .find(|(s, _)| s.eq_ignore_ascii_case(sql_name))
            .map(|(_, m)| m.clone())
    };

    let mut out = String::new();
    if opts.header {
        out.push_str(&render_header(&opts));
    }

    for (idx, table) in tables.iter().enumerate() {
        if idx > 0 || opts.header {
            out.push('\n');
        }
        out.push_str(&render_model(table, &model_names[idx].1, &opts, &lookup));
    }

    Ok(out)
}

fn render_header(opts: &Options) -> String {
    format!(
        "generator client {{\n  provider = \"prisma-client-js\"\n}}\n\n\
         datasource db {{\n  provider = \"{}\"\n  url      = env(\"DATABASE_URL\")\n}}\n",
        opts.provider.prisma_name()
    )
}

/// One `model { … }` block.
fn render_model(
    table: &Value,
    model: &str,
    opts: &Options,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> String {
    let sql_table = table["name"].as_str().unwrap_or("Table");
    let empty = Vec::new();
    let columns = table["columns"].as_array().unwrap_or(&empty);
    let pk: Vec<String> = table["primary_key"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let composite_pk = pk.len() > 1;

    // Track field names already used in this model so relation fields don't
    // collide with a scalar column.
    let mut used: Vec<String> = Vec::new();

    let mut lines: Vec<String> = Vec::new();
    for col in columns {
        let (line, field) = render_field(col, &pk, composite_pk, opts);
        used.push(field);
        lines.push(line);
    }

    // Forward `@relation` fields from this table's foreign keys.
    if opts.relations {
        let empty_fk = Vec::new();
        let fks = table["foreign_keys"].as_array().unwrap_or(&empty_fk);
        for fk in fks {
            if let Some(line) = render_forward_relation(fk, columns, opts, lookup, &mut used, fks) {
                lines.push(line);
            }
        }
    }

    let mut block = String::new();
    block.push_str(&format!("model {model} {{\n"));
    for l in &lines {
        block.push_str("  ");
        block.push_str(l);
        block.push('\n');
    }

    // Block-level attributes.
    let mut attrs: Vec<String> = Vec::new();
    if composite_pk {
        let fields: Vec<String> = pk.iter().map(|c| field_name(c, opts.map_names)).collect();
        attrs.push(format!("@@id([{}])", fields.join(", ")));
    }
    for u in table["unique_constraints"].as_array().unwrap_or(&empty) {
        let cols = str_vec(&u["columns"]);
        if cols.len() > 1 {
            let fields: Vec<String> = cols.iter().map(|c| field_name(c, opts.map_names)).collect();
            attrs.push(format!("@@unique([{}])", fields.join(", ")));
        }
    }
    for i in table["indexes"].as_array().unwrap_or(&empty) {
        let cols = str_vec(&i["columns"]);
        if cols.is_empty() {
            continue;
        }
        let fields: Vec<String> = cols.iter().map(|c| field_name(c, opts.map_names)).collect();
        let kw = if i["unique"].as_bool().unwrap_or(false) {
            "@@unique"
        } else {
            "@@index"
        };
        attrs.push(format!("{kw}([{}])", fields.join(", ")));
    }
    if opts.map_names && model != sql_table {
        attrs.push(format!("@@map(\"{}\")", escape_str(sql_table)));
    }
    if !attrs.is_empty() {
        block.push('\n');
        for a in attrs {
            block.push_str("  ");
            block.push_str(&a);
            block.push('\n');
        }
    }
    block.push_str("}\n");
    block
}

/// Render one scalar field line; returns `(line, field_name)`.
fn render_field(col: &Value, pk: &[String], composite_pk: bool, opts: &Options) -> (String, String) {
    let sql_col = col["name"].as_str().unwrap_or("column");
    let field = field_name(sql_col, opts.map_names);
    let sql_type = col["type"].as_str().unwrap_or("");
    let (ptype, native) = prisma_type(sql_type, opts);
    // Postgres SERIAL-family types are the data type itself, so the DDL parser
    // records them as the column type rather than an auto-increment modifier;
    // treat them as autoincrement here.
    let auto_inc = col["auto_increment"].as_bool().unwrap_or(false) || is_serial(sql_type);
    let is_pk = pk.iter().any(|c| c.eq_ignore_ascii_case(sql_col));
    // A single-column PK is never optional; otherwise honour nullability.
    let nullable = col["nullable"].as_bool().unwrap_or(true) && !is_pk;

    let mut ty = ptype.to_string();
    if nullable {
        ty.push('?');
    }

    let mut attrs: Vec<String> = Vec::new();
    if is_pk && !composite_pk {
        attrs.push("@id".to_string());
    }
    if col["unique"].as_bool().unwrap_or(false) && !(is_pk && !composite_pk) {
        attrs.push("@unique".to_string());
    }
    if let Some(def) = render_default(col, ptype, auto_inc) {
        attrs.push(def);
    }
    if opts.native_types {
        if let Some(n) = native {
            attrs.push(n);
        }
    }
    if opts.map_names && field != sql_col {
        attrs.push(format!("@map(\"{}\")", escape_str(sql_col)));
    }

    let mut line = format!("{field} {ty}");
    if !attrs.is_empty() {
        line.push(' ');
        line.push_str(&attrs.join(" "));
    }
    (line, field)
}

/// Render a forward `@relation` field for a foreign key. Returns the field line,
/// or `None` if the referenced table isn't in the schema.
fn render_forward_relation(
    fk: &Value,
    columns: &[Value],
    opts: &Options,
    lookup: &dyn Fn(&str) -> Option<String>,
    used: &mut Vec<String>,
    all_fks: &[Value],
) -> Option<String> {
    let ref_table = fk["references"]["table"].as_str()?;
    let target_model = lookup(ref_table)?;
    let fk_cols = str_vec(&fk["columns"]);
    if fk_cols.is_empty() {
        return None;
    }
    let ref_cols = str_vec(&fk["references"]["columns"]);

    // The relation field is optional iff every FK column is nullable.
    let nullable = fk_cols.iter().all(|c| {
        columns
            .iter()
            .find(|col| col["name"].as_str().map(|n| n.eq_ignore_ascii_case(c)) == Some(true))
            .map(|col| col["nullable"].as_bool().unwrap_or(true))
            .unwrap_or(true)
    });

    // Name the field after the FK column with a trailing id stripped
    // (`user_id` → `user`), falling back to the target model name.
    let base = relation_field_base(&fk_cols, ref_table, opts.map_names);
    let field = unique_name(&base, used);

    let fields_list: Vec<String> = fk_cols.iter().map(|c| field_name(c, opts.map_names)).collect();
    let refs_list: Vec<String> = if ref_cols.is_empty() {
        vec!["id".to_string()]
    } else {
        ref_cols.iter().map(|c| field_name(c, opts.map_names)).collect()
    };

    // If two FKs point at the same target model, Prisma needs an explicit
    // relation name on both sides to disambiguate.
    let dup_target = all_fks
        .iter()
        .filter(|other| {
            other["references"]["table"]
                .as_str()
                .map(|t| t.eq_ignore_ascii_case(ref_table))
                == Some(true)
        })
        .count()
        > 1;
    let rel_name = if dup_target {
        Some(format!("{}_{}", target_model, fields_list.join("_")))
    } else {
        None
    };

    let mut rel = String::from("@relation(");
    if let Some(n) = &rel_name {
        rel.push_str(&format!("\"{}\", ", escape_str(n)));
    }
    rel.push_str(&format!(
        "fields: [{}], references: [{}]",
        fields_list.join(", "),
        refs_list.join(", ")
    ));
    if let Some(a) = fk["on_delete"].as_str() {
        rel.push_str(&format!(", onDelete: {}", referential_action(a)));
    }
    if let Some(a) = fk["on_update"].as_str() {
        rel.push_str(&format!(", onUpdate: {}", referential_action(a)));
    }
    rel.push(')');

    let ty = if nullable {
        format!("{target_model}?")
    } else {
        target_model
    };
    Some(format!("{field} {ty} {rel}"))
}

/// Map a SQL `ON DELETE/UPDATE` action onto a Prisma referential action.
fn referential_action(a: &str) -> &'static str {
    match a.trim().to_ascii_uppercase().as_str() {
        "CASCADE" => "Cascade",
        "RESTRICT" => "Restrict",
        "SET NULL" => "SetNull",
        "SET DEFAULT" => "SetDefault",
        _ => "NoAction",
    }
}

/// Field name for a relation, derived from the FK column(s).
fn relation_field_base(fk_cols: &[String], ref_table: &str, map_names: bool) -> String {
    if fk_cols.len() == 1 {
        let c = &fk_cols[0];
        let stripped = c
            .strip_suffix("_id")
            .or_else(|| c.strip_suffix("_ID"))
            .or_else(|| c.strip_suffix("Id"))
            .or_else(|| c.strip_suffix("ID"))
            .unwrap_or(c);
        if !stripped.is_empty() {
            return field_name(stripped, map_names);
        }
    }
    field_name(ref_table, map_names)
}

/// Ensure `base` is unique among `used`, suffixing `_rel2`, `_rel3`, … as needed.
fn unique_name(base: &str, used: &mut Vec<String>) -> String {
    if !used.iter().any(|u| u == base) {
        used.push(base.to_string());
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let cand = format!("{base}_rel{n}");
        if !used.iter().any(|u| *u == cand) {
            used.push(cand.clone());
            return cand;
        }
        n += 1;
    }
}

/// Render an `@default(...)` attribute, or `None` when there is no usable default.
fn render_default(col: &Value, ptype: &str, auto_inc: bool) -> Option<String> {
    if auto_inc {
        return Some("@default(autoincrement())".to_string());
    }
    let raw = col["default"].as_str()?.trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();
    if lower == "null" {
        return None;
    }

    // Time defaults → now().
    if matches!(
        lower.as_str(),
        "current_timestamp"
            | "current_timestamp()"
            | "now()"
            | "getdate()"
            | "sysdatetime()"
            | "localtimestamp"
            | "current_date"
    ) {
        return Some("@default(now())".to_string());
    }
    // UUID generators → uuid().
    if lower.contains("uuid") || lower.contains("newid()") || lower.contains("gen_random") {
        return Some("@default(uuid())".to_string());
    }

    // Quoted string literal.
    if (raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2)
        || (raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2)
    {
        let inner = &raw[1..raw.len() - 1];
        return match ptype {
            "Boolean" => bool_default(inner),
            "Int" | "BigInt" | "Float" | "Decimal" => {
                if is_number(inner) {
                    Some(format!("@default({inner})"))
                } else {
                    None
                }
            }
            _ => Some(format!("@default(\"{}\")", escape_str(inner))),
        };
    }

    // Boolean keyword / numeric bool.
    if ptype == "Boolean" {
        return bool_default(&raw);
    }

    // Bare numeric literal.
    if is_number(&raw) {
        return match ptype {
            "String" => Some(format!("@default(\"{}\")", escape_str(&raw))),
            _ => Some(format!("@default({raw})")),
        };
    }

    // Anything else (function call, complex expression) → dbgenerated.
    Some(format!("@default(dbgenerated(\"{}\"))", escape_str(&raw)))
}

fn bool_default(s: &str) -> Option<String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "t" => Some("@default(true)".to_string()),
        "false" | "0" | "f" => Some("@default(false)".to_string()),
        _ => None,
    }
}

fn is_number(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty()
        && s.parse::<f64>().is_ok()
        && s.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
}

/// Map a SQL type (e.g. `VARCHAR(255)`, `DECIMAL(10,2)`, `TIMESTAMP WITH TIME
/// ZONE`) onto a Prisma scalar type plus an optional native-type attribute.
fn prisma_type(sql_type: &str, opts: &Options) -> (&'static str, Option<String>) {
    let (base, args) = split_type(sql_type);
    let first = base.split_whitespace().next().unwrap_or("");
    let native_ok = opts.native_types && opts.provider.supports_native_types();

    let scalar = match first {
        "INT" | "INTEGER" | "INT4" | "MEDIUMINT" | "SMALLINT" | "INT2" | "YEAR" | "SERIAL"
        | "SERIAL4" | "SMALLSERIAL" | "SERIAL2" => "Int",
        "TINYINT" => {
            if opts.provider == Provider::Mysql && args.as_deref() == Some("1") {
                "Boolean"
            } else {
                "Int"
            }
        }
        "BIGINT" | "INT8" | "BIGSERIAL" | "SERIAL8" => "BigInt",
        "BOOLEAN" | "BOOL" | "BIT" => "Boolean",
        "DECIMAL" | "NUMERIC" | "DEC" | "MONEY" | "SMALLMONEY" | "FIXED" => "Decimal",
        "FLOAT" | "REAL" | "DOUBLE" | "FLOAT4" | "FLOAT8" | "BINARY_DOUBLE" | "BINARY_FLOAT" => {
            "Float"
        }
        "DATE" | "DATETIME" | "DATETIME2" | "SMALLDATETIME" | "TIMESTAMP" | "TIMESTAMPTZ"
        | "TIME" | "TIMETZ" | "DATETIMEOFFSET" => "DateTime",
        "JSON" | "JSONB" => "Json",
        "BYTEA" | "BLOB" | "BINARY" | "VARBINARY" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB"
        | "IMAGE" | "RAW" => "Bytes",
        // Everything textual / identifier-ish → String (Prisma's fallback too).
        _ => "String",
    };

    // Native-type attribute for the three near-universal, high-value cases.
    let native = if native_ok {
        match first {
            "VARCHAR" | "NVARCHAR" | "VARCHAR2" | "NVARCHAR2" if scalar == "String" => {
                digits(&args).map(|n| format!("@db.VarChar({n})"))
            }
            "CHAR" | "NCHAR" | "CHARACTER" if scalar == "String" => {
                digits(&args).map(|n| format!("@db.Char({n})"))
            }
            "DECIMAL" | "NUMERIC" if scalar == "Decimal" => args.as_ref().and_then(|a| {
                let parts: Vec<&str> = a.split(',').map(|p| p.trim()).collect();
                if parts.len() == 2
                    && parts
                        .iter()
                        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
                {
                    Some(format!("@db.Decimal({}, {})", parts[0], parts[1]))
                } else {
                    None
                }
            }),
            _ => None,
        }
    } else {
        None
    };

    (scalar, native)
}

/// Whether a SQL type is a Postgres SERIAL-family pseudo-type (which implies an
/// auto-incrementing integer sequence).
fn is_serial(sql_type: &str) -> bool {
    let (base, _) = split_type(sql_type);
    matches!(
        base.split_whitespace().next().unwrap_or(""),
        "SERIAL" | "SERIAL2" | "SERIAL4" | "SERIAL8" | "SMALLSERIAL" | "BIGSERIAL"
    )
}

/// First comma-part of `args` if it is all digits (e.g. `"255"` from `VARCHAR(255)`).
fn digits(args: &Option<String>) -> Option<String> {
    let a = args.as_ref()?;
    let n = a.split(',').next().unwrap_or("").trim();
    (!n.is_empty() && n.chars().all(|c| c.is_ascii_digit())).then(|| n.to_string())
}

/// Split `VARCHAR(255)` → ("VARCHAR", Some("255")); `TIMESTAMP WITH TIME ZONE`
/// → ("TIMESTAMP WITH TIME ZONE", None). The base keeps multi-word keywords,
/// upper-cased; args is the inside of the first paren group, if any.
fn split_type(sql_type: &str) -> (String, Option<String>) {
    let up = sql_type.trim().to_ascii_uppercase();
    if let Some(open) = up.find('(') {
        let base_head = up[..open].trim().to_string();
        let rest = &up[open + 1..];
        let close = rest.find(')').unwrap_or(rest.len());
        let args = rest[..close].trim().to_string();
        let tail = rest.get(close + 1..).unwrap_or("").trim();
        let base = if tail.is_empty() {
            base_head
        } else {
            format!("{base_head} {tail}")
        };
        (base, if args.is_empty() { None } else { Some(args) })
    } else {
        (up, None)
    }
}

// ---------------------------------------------------------------------------
// Naming helpers
// ---------------------------------------------------------------------------

fn model_name(sql_table: &str, map_names: bool) -> String {
    if map_names {
        pascal_case(&singularize(sql_table))
    } else {
        sanitize_ident(sql_table)
    }
}

fn field_name(sql_col: &str, map_names: bool) -> String {
    if map_names {
        camel_case(sql_col)
    } else {
        sanitize_ident(sql_col)
    }
}

/// Prisma identifiers must match `[A-Za-z][A-Za-z0-9_]*`; replace anything else
/// with `_` and prefix a leading digit. (`@map`/`@@map` keep the DB name.)
fn sanitize_ident(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_alphanumeric() || c == '_' {
            if i == 0 && c.is_ascii_digit() {
                out.push('_');
            }
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

fn split_words(s: &str) -> Vec<String> {
    // Split on non-alphanumerics AND camelCase humps.
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_lower && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            cur.push(c);
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn pascal_case(s: &str) -> String {
    let mut out = String::new();
    for w in split_words(s) {
        let mut chars = w.chars();
        if let Some(f) = chars.next() {
            out.extend(f.to_uppercase());
            out.push_str(&chars.as_str().to_ascii_lowercase());
        }
    }
    if out.is_empty() {
        out.push('_');
    } else if out.chars().next().map(|c| c.is_ascii_digit()) == Some(true) {
        out.insert(0, '_');
    }
    out
}

fn camel_case(s: &str) -> String {
    let p = pascal_case(s);
    let mut chars = p.chars();
    match chars.next() {
        Some(f) => {
            let mut out: String = f.to_lowercase().collect();
            out.push_str(chars.as_str());
            out
        }
        None => "_".to_string(),
    }
}

/// Crude English singularizer for model names (best-effort; only used with
/// `map_names`, and always paired with `@@map` to keep the DB name exact).
fn singularize(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if lower.ends_with("ies") && s.len() > 3 {
        return format!("{}y", &s[..s.len() - 3]);
    }
    for suf in ["ses", "xes", "zes", "ches", "shes"] {
        if lower.ends_with(suf) {
            return s[..s.len() - 2].to_string();
        }
    }
    if lower.ends_with('s') && !lower.ends_with("ss") && s.len() > 1 {
        return s[..s.len() - 1].to_string();
    }
    s.to_string()
}

fn str_vec(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Escape `"` and `\` for a Prisma double-quoted string literal.
fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(sql: &str) -> String {
        convert(sql, "postgresql", true, true, true, false).unwrap()
    }

    #[test]
    fn happy_users_table_with_pk_unique_default() {
        let sql = "CREATE TABLE users (\n  id SERIAL PRIMARY KEY,\n  \
                   email VARCHAR(255) NOT NULL UNIQUE,\n  \
                   created_at TIMESTAMP DEFAULT now()\n);";
        let out = gen(sql);
        assert!(out.contains("model users {"), "got:\n{out}");
        assert!(
            out.contains("id Int @id @default(autoincrement())"),
            "got:\n{out}"
        );
        assert!(
            out.contains("email String @unique @db.VarChar(255)"),
            "got:\n{out}"
        );
        assert!(
            out.contains("created_at DateTime? @default(now())"),
            "got:\n{out}"
        );
        assert!(out.contains("datasource db {"));
        assert!(out.contains("provider = \"postgresql\""));
    }

    #[test]
    fn foreign_key_becomes_relation() {
        let sql = "CREATE TABLE users (id SERIAL PRIMARY KEY);\n\
                   CREATE TABLE orders (\n  id SERIAL PRIMARY KEY,\n  \
                   user_id INT NOT NULL REFERENCES users(id) ON DELETE CASCADE\n);";
        let out = convert(sql, "postgresql", false, true, false, false).unwrap();
        assert!(out.contains("model orders {"), "got:\n{out}");
        assert!(
            out.contains(
                "user users @relation(fields: [user_id], references: [id], onDelete: Cascade)"
            ),
            "got:\n{out}"
        );
    }

    #[test]
    fn composite_pk_emits_block_id() {
        let sql = "CREATE TABLE line_items (\n  order_id INT,\n  sku VARCHAR(32),\n  \
                   qty INT DEFAULT 1,\n  PRIMARY KEY (order_id, sku)\n);";
        let out = convert(sql, "mysql", false, true, true, false).unwrap();
        assert!(out.contains("@@id([order_id, sku])"), "got:\n{out}");
        assert!(out.contains("qty Int? @default(1)"), "got:\n{out}");
        // neither PK column is optional
        assert!(out.contains("order_id Int"));
        assert!(!out.contains("order_id Int?"));
    }

    #[test]
    fn map_names_pascal_and_camel_with_maps() {
        let sql = "CREATE TABLE blog_posts (post_id SERIAL PRIMARY KEY, full_title TEXT);";
        let out = convert(sql, "postgresql", false, true, false, true).unwrap();
        assert!(out.contains("model BlogPost {"), "got:\n{out}");
        assert!(out.contains("@@map(\"blog_posts\")"), "got:\n{out}");
        assert!(
            out.contains("postId Int @id @default(autoincrement()) @map(\"post_id\")"),
            "got:\n{out}"
        );
        assert!(
            out.contains("fullTitle String? @map(\"full_title\")"),
            "got:\n{out}"
        );
    }

    #[test]
    fn type_mapping_covers_common_types() {
        let sql = "CREATE TABLE t (\n  a BIGINT,\n  b BOOLEAN,\n  c DECIMAL(10,2),\n  \
                   d DOUBLE PRECISION,\n  e JSONB,\n  f BYTEA,\n  g TIMESTAMP WITH TIME ZONE\n);";
        let out = convert(sql, "postgresql", false, false, false, false).unwrap();
        assert!(out.contains("a BigInt?"), "got:\n{out}");
        assert!(out.contains("b Boolean?"), "got:\n{out}");
        assert!(out.contains("c Decimal?"), "got:\n{out}");
        assert!(out.contains("d Float?"), "got:\n{out}");
        assert!(out.contains("e Json?"), "got:\n{out}");
        assert!(out.contains("f Bytes?"), "got:\n{out}");
        assert!(out.contains("g DateTime?"), "got:\n{out}");
    }

    #[test]
    fn native_types_off_for_sqlite() {
        let sql = "CREATE TABLE t (name VARCHAR(50));";
        let out = convert(sql, "sqlite", false, true, true, false).unwrap();
        assert!(out.contains("name String?"), "got:\n{out}");
        assert!(
            !out.contains("@db."),
            "sqlite must not emit native types, got:\n{out}"
        );
    }

    #[test]
    fn error_empty_input() {
        let err = convert("   ", "postgresql", true, true, true, false).unwrap_err();
        assert!(err.contains("no SQL provided"), "got: {err}");
    }

    #[test]
    fn error_no_ddl() {
        let err =
            convert("INSERT INTO t VALUES (1);", "postgresql", true, true, true, false).unwrap_err();
        assert!(err.contains("no CREATE TABLE"), "got: {err}");
    }

    #[test]
    fn error_bad_provider() {
        let err =
            convert("CREATE TABLE t (id INT);", "oracle", true, true, true, false).unwrap_err();
        assert!(err.contains("invalid provider"), "got: {err}");
    }

    #[test]
    fn string_default_quoted() {
        let sql = "CREATE TABLE t (id INT PRIMARY KEY, status VARCHAR(20) DEFAULT 'active');";
        let out = convert(sql, "mysql", false, false, false, false).unwrap();
        assert!(
            out.contains("status String? @default(\"active\")"),
            "got:\n{out}"
        );
    }

    #[test]
    fn tinyint1_is_boolean_in_mysql() {
        let sql = "CREATE TABLE t (id INT PRIMARY KEY, is_active TINYINT(1) DEFAULT 1);";
        let out = convert(sql, "mysql", false, false, false, false).unwrap();
        assert!(
            out.contains("is_active Boolean? @default(true)"),
            "got:\n{out}"
        );
    }
}
