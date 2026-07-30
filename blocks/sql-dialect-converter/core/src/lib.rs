//! sql-dialect-converter core — convert SQL between PostgreSQL, MySQL and SQLite.
//!
//! Pure compute, shared by the chat skill block, the CLI and the web page. No
//! wafer/wasm-bindgen deps. This is a forgiving TOKENIZER, not a full per-dialect
//! grammar: it reliably rewrites the three portability pain-points that differ
//! across the three dialects —
//!   1. delimited-identifier quoting (`"x"` / `` `x` `` / `[x]` → the target style),
//!   2. auto-increment columns (`SERIAL` ↔ `AUTO_INCREMENT` ↔ `INTEGER PRIMARY KEY
//!      AUTOINCREMENT`) inside `CREATE TABLE`,
//!   3. column data types inside `CREATE TABLE` (bool, varchar, timestamp, blob,
//!      double, json, uuid, …), plus stripping MySQL table options (`ENGINE=…`,
//!      `DEFAULT CHARSET=…`) when the target is not MySQL.
//! String literals and comments are preserved verbatim. Expression/function
//! rewriting, procedures, views and dialects beyond the three named are out of
//! scope (documented on the page).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dialect {
    Postgres,
    Mysql,
    Sqlite,
}

impl Dialect {
    /// Parse a dialect name (case-insensitive, common aliases accepted).
    pub fn parse(s: &str) -> Result<Dialect, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" | "pg" | "psql" => Ok(Dialect::Postgres),
            "mysql" | "mariadb" => Ok(Dialect::Mysql),
            "sqlite" | "sqlite3" => Ok(Dialect::Sqlite),
            other => Err(format!(
                "unknown dialect '{other}' (expected postgres, mysql, or sqlite)"
            )),
        }
    }
}

#[derive(Clone, Debug)]
enum Token {
    Ws(String),      // whitespace, verbatim
    Word(String),    // bareword: keyword / unquoted identifier / number chunk
    Str(String),     // string literal INCLUDING quotes, verbatim (never rewritten)
    Comment(String), // -- line or /* block */ comment, verbatim
    Punct(char),     // single punctuation char: ( ) , ; . = etc
    Ident(String),   // delimited identifier, holding the UNescaped inner text
}

impl Token {
    fn word_lower(&self) -> Option<String> {
        match self {
            Token::Word(w) => Some(w.to_ascii_lowercase()),
            _ => None,
        }
    }
    fn is_word(&self, kw: &str) -> bool {
        matches!(self.word_lower(), Some(w) if w == kw)
    }
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

fn tokenize(sql: &str) -> Vec<Token> {
    let chars: Vec<char> = sql.chars().collect();
    let n = chars.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c.is_whitespace() {
            let start = i;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(Token::Ws(chars[start..i].iter().collect()));
        } else if c == '-' && i + 1 < n && chars[i + 1] == '-' {
            // line comment to end of line (newline stays as separate Ws)
            let start = i;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            out.push(Token::Comment(chars[start..i].iter().collect()));
        } else if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            let start = i;
            i += 2;
            while i < n && !(chars[i] == '*' && i + 1 < n && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(n); // consume closing */
            out.push(Token::Comment(chars[start..i].iter().collect()));
        } else if c == '\'' {
            // single-quoted string literal with '' escape — verbatim
            let start = i;
            i += 1;
            while i < n {
                if chars[i] == '\'' {
                    if i + 1 < n && chars[i + 1] == '\'' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(Token::Str(chars[start..i].iter().collect()));
        } else if c == '"' || c == '`' {
            // delimited identifier with doubled-delimiter escape
            let delim = c;
            i += 1;
            let mut inner = String::new();
            while i < n {
                if chars[i] == delim {
                    if i + 1 < n && chars[i + 1] == delim {
                        inner.push(delim);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                inner.push(chars[i]);
                i += 1;
            }
            out.push(Token::Ident(inner));
        } else if c == '[' {
            // SQL Server / SQLite bracket identifier: [x], ]] escapes ]
            i += 1;
            let mut inner = String::new();
            while i < n {
                if chars[i] == ']' {
                    if i + 1 < n && chars[i + 1] == ']' {
                        inner.push(']');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                inner.push(chars[i]);
                i += 1;
            }
            out.push(Token::Ident(inner));
        } else if is_word_char(c) {
            let start = i;
            while i < n && is_word_char(chars[i]) {
                i += 1;
            }
            out.push(Token::Word(chars[start..i].iter().collect()));
        } else {
            out.push(Token::Punct(c));
            i += 1;
        }
    }
    out
}

fn quote_ident(inner: &str, to: Dialect) -> String {
    match to {
        Dialect::Mysql => format!("`{}`", inner.replace('`', "``")),
        _ => format!("\"{}\"", inner.replace('"', "\"\"")),
    }
}

fn serialize_token(tok: &Token, to: Dialect) -> String {
    match tok {
        Token::Ws(s) | Token::Word(s) | Token::Str(s) | Token::Comment(s) => s.clone(),
        Token::Punct(c) => c.to_string(),
        Token::Ident(inner) => quote_ident(inner, to),
    }
}

/// Canonical type category resolved from one or two source type words. Returns
/// the category key and whether a second word (`double PRECISION`,
/// `character VARYING`) was consumed.
fn resolve_type(w1: &str, w2: Option<&str>) -> Option<(&'static str, bool)> {
    let w1 = w1.to_ascii_lowercase();
    let w2 = w2.map(|s| s.to_ascii_lowercase());
    let key = match w1.as_str() {
        "boolean" | "bool" => "bool",
        "tinyint" => "tinyint",
        "smallint" | "int2" => "smallint",
        "int" | "integer" | "int4" => "int",
        "bigint" | "int8" => "bigint",
        "text" | "longtext" | "mediumtext" | "tinytext" | "clob" => "text",
        "varchar" | "nvarchar" => "varchar",
        "char" | "nchar" if w2.as_deref() != Some("varying") => "char",
        "character" => {
            if w2.as_deref() == Some("varying") {
                return Some(("varchar", true));
            }
            "char"
        }
        "timestamp" | "timestamptz" | "datetime" => "timestamp",
        "date" => "date",
        "time" | "timetz" => "time",
        "blob" | "bytea" | "binary" | "varbinary" | "longblob" | "mediumblob" | "tinyblob" => "blob",
        "double" => {
            return Some(("double", w2.as_deref() == Some("precision")));
        }
        "float8" | "float" => "double",
        "real" | "float4" => "real",
        "numeric" | "decimal" | "dec" => "numeric",
        "json" | "jsonb" => "json",
        "uuid" => "uuid",
        _ => return None,
    };
    Some((key, false))
}

/// Map a canonical type category to the target dialect's spelling. Returns the
/// spelling and whether trailing `(...)` args (precision/length) are kept.
fn map_type(canon: &str, to: Dialect) -> (&'static str, bool) {
    use Dialect::*;
    match (canon, to) {
        ("bool", Postgres) => ("BOOLEAN", false),
        ("bool", Mysql) => ("TINYINT(1)", false),
        ("bool", Sqlite) => ("INTEGER", false),
        ("tinyint", Postgres) => ("SMALLINT", false),
        ("tinyint", Mysql) => ("TINYINT", false),
        ("tinyint", Sqlite) => ("INTEGER", false),
        ("smallint", Sqlite) => ("INTEGER", false),
        ("smallint", _) => ("SMALLINT", false),
        ("int", Mysql) => ("INT", false),
        ("int", _) => ("INTEGER", false),
        ("bigint", Sqlite) => ("INTEGER", false),
        ("bigint", _) => ("BIGINT", false),
        ("text", _) => ("TEXT", false),
        ("varchar", Sqlite) => ("TEXT", false),
        ("varchar", _) => ("VARCHAR", true),
        ("char", Sqlite) => ("TEXT", false),
        ("char", _) => ("CHAR", true),
        ("timestamp", Postgres) => ("TIMESTAMP", true),
        ("timestamp", Mysql) => ("DATETIME", true),
        ("timestamp", Sqlite) => ("TEXT", false),
        ("date", Sqlite) => ("TEXT", false),
        ("date", _) => ("DATE", false),
        ("time", Sqlite) => ("TEXT", false),
        ("time", _) => ("TIME", false),
        ("blob", Postgres) => ("BYTEA", false),
        ("blob", _) => ("BLOB", false),
        ("double", Postgres) => ("DOUBLE PRECISION", false),
        ("double", Mysql) => ("DOUBLE", false),
        ("double", Sqlite) => ("REAL", false),
        ("real", Postgres) => ("REAL", false),
        ("real", Mysql) => ("FLOAT", false),
        ("real", Sqlite) => ("REAL", false),
        ("numeric", Mysql) => ("DECIMAL", true),
        ("numeric", _) => ("NUMERIC", true),
        ("json", Postgres) => ("JSONB", false),
        ("json", Mysql) => ("JSON", false),
        ("json", Sqlite) => ("TEXT", false),
        ("uuid", Postgres) => ("UUID", false),
        ("uuid", Mysql) => ("CHAR(36)", false),
        ("uuid", Sqlite) => ("TEXT", false),
        // unreachable for known keys; passthrough guard
        _ => ("", true),
    }
}

const AUTOINC_INT_WORDS: &[&str] = &[
    "int", "integer", "int4", "int8", "bigint", "smallint", "int2", "tinyint", "serial",
    "bigserial", "smallserial",
];

/// The out-index list is 1:1 with `tokens`; `None` = drop, `Some(s)` = emit.
type OutBuf = Vec<Option<String>>;

pub fn convert(sql: &str, from: Dialect, to: Dialect) -> Result<String, String> {
    if sql.trim().is_empty() {
        return Err("empty SQL input".into());
    }
    // Converting a dialect to itself is a no-op: return the SQL unchanged.
    if from == to {
        return Ok(sql.to_string());
    }

    let tokens = tokenize(sql);
    let mut out: OutBuf = tokens.iter().map(|t| Some(serialize_token(t, to))).collect();

    // `code` = indices of significant tokens (skip whitespace + comments) so the
    // structural walk over CREATE TABLE ignores formatting.
    let code: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !matches!(t, Token::Ws(_) | Token::Comment(_)))
        .map(|(i, _)| i)
        .collect();
    let is_code = |ci: usize| -> &Token { &tokens[code[ci]] };

    let mut ci = 0;
    while ci < code.len() {
        if !is_code(ci).is_word("create") {
            ci += 1;
            continue;
        }
        // CREATE [TEMP|TEMPORARY|UNLOGGED|GLOBAL|LOCAL]* TABLE …
        let mut j = ci + 1;
        while j < code.len()
            && matches!(
                is_code(j).word_lower().as_deref(),
                Some("temp" | "temporary" | "unlogged" | "global" | "local")
            )
        {
            j += 1;
        }
        if j >= code.len() || !is_code(j).is_word("table") {
            ci += 1;
            continue;
        }
        // Find the opening paren of the column list; bail if we hit a
        // non-column-list form (AS SELECT / LIKE / end of statement) first.
        let mut open = None;
        let mut k = j + 1;
        while k < code.len() {
            match &tokens[code[k]] {
                Token::Punct('(') => {
                    open = Some(k);
                    break;
                }
                Token::Punct(';') => break,
                t if matches!(t.word_lower().as_deref(), Some("as" | "select" | "like")) => break,
                _ => k += 1,
            }
        }
        let Some(open) = open else {
            ci = j + 1;
            continue;
        };
        // Match the closing paren.
        let mut depth = 0i32;
        let mut close = None;
        let mut k = open;
        while k < code.len() {
            match &tokens[code[k]] {
                Token::Punct('(') => depth += 1,
                Token::Punct(')') => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(k);
                        break;
                    }
                }
                _ => {}
            }
            k += 1;
        }
        let Some(close) = close else {
            ci = open + 1;
            continue;
        };

        // Split the body into top-level definitions at depth-0 commas.
        let mut defs: Vec<Vec<usize>> = Vec::new();
        let mut cur: Vec<usize> = Vec::new();
        let mut d = 0i32;
        for k in (open + 1)..close {
            match &tokens[code[k]] {
                Token::Punct('(') => {
                    d += 1;
                    cur.push(code[k]);
                }
                Token::Punct(')') => {
                    d -= 1;
                    cur.push(code[k]);
                }
                Token::Punct(',') if d == 0 => {
                    if !cur.is_empty() {
                        defs.push(std::mem::take(&mut cur));
                    }
                }
                _ => cur.push(code[k]),
            }
        }
        if !cur.is_empty() {
            defs.push(cur);
        }

        for def in &defs {
            process_column_def(def, &tokens, to, &mut out);
        }

        // Strip a MySQL table-option tail (ENGINE=…, DEFAULT CHARSET=…) when the
        // target is not MySQL.
        if to != Dialect::Mysql {
            // Tail = code tokens after the close paren up to the statement `;`.
            let mut end = close + 1;
            while end < code.len() && !matches!(tokens[code[end]], Token::Punct(';')) {
                end += 1;
            }
            let has_option = (close + 1..end).any(|t| {
                matches!(
                    tokens[code[t]].word_lower().as_deref(),
                    Some(
                        "engine"
                            | "charset"
                            | "collate"
                            | "auto_increment"
                            | "row_format"
                            | "comment"
                            | "default"
                            | "character"
                            | "pack_keys"
                            | "checksum"
                    )
                )
            });
            if has_option {
                // Drop the raw token range between `)` and `;` (keep both).
                let start_tok = code[close] + 1;
                let end_tok = if end < code.len() { code[end] } else { tokens.len() };
                for entry in out.iter_mut().take(end_tok).skip(start_tok) {
                    *entry = None;
                }
            }
        }

        ci = close + 1;
    }

    Ok(out.into_iter().flatten().collect())
}

const CONSTRAINT_KWS: &[&str] = &[
    "primary",
    "foreign",
    "unique",
    "key",
    "check",
    "constraint",
    "index",
    "exclude",
    "fulltext",
    "spatial",
];

fn process_column_def(def: &[usize], tokens: &[Token], to: Dialect, out: &mut OutBuf) {
    if def.is_empty() {
        return;
    }
    let first = &tokens[def[0]];
    if let Some(w) = first.word_lower() {
        if CONSTRAINT_KWS.contains(&w.as_str()) {
            return; // table-level constraint: only identifier requoting (handled on serialize)
        }
    }
    if def.len() < 2 {
        return; // just a name, nothing to map
    }

    // Detect an auto-increment column anywhere in the definition.
    let is_auto = def.iter().any(|&t| {
        matches!(
            tokens[t].word_lower().as_deref(),
            Some("serial" | "bigserial" | "smallserial" | "auto_increment" | "autoincrement")
        )
    });

    if is_auto {
        rebuild_autoinc_def(def, tokens, to, out);
        return;
    }

    // Non-auto column: map the type token in place (types are only converted
    // inside CREATE TABLE column definitions).
    let type_ti = def[1];
    let Some(w1) = tokens[type_ti].word_lower() else {
        return;
    };
    let w2 = def.get(2).and_then(|&t| tokens[t].word_lower());
    let Some((canon, consumed_second)) = resolve_type(&w1, w2.as_deref()) else {
        return; // unknown type: passthrough
    };
    let (target, keep_args) = map_type(canon, to);
    if target.is_empty() {
        return;
    }
    out[type_ti] = Some(target.to_string());
    let mut next_arg_pos = 2;
    if consumed_second {
        out[def[2]] = None; // drop PRECISION / VARYING second word
        next_arg_pos = 3;
    }
    // Drop trailing (n) / (p,s) args when the target type takes none.
    if !keep_args {
        if let Some(&open_ti) = def.get(next_arg_pos) {
            if matches!(tokens[open_ti], Token::Punct('(')) {
                let mut d = 0i32;
                for &t in &def[next_arg_pos..] {
                    match &tokens[t] {
                        Token::Punct('(') => {
                            d += 1;
                            out[t] = None;
                        }
                        Token::Punct(')') => {
                            out[t] = None;
                            d -= 1;
                            if d == 0 {
                                break;
                            }
                        }
                        _ if d > 0 => out[t] = None,
                        _ => break,
                    }
                }
            }
        }
    }
}

fn rebuild_autoinc_def(def: &[usize], tokens: &[Token], to: Dialect, out: &mut OutBuf) {
    let name_ti = def[0];
    // Size: big if any bigint/int8/bigserial word appears, else regular int.
    let big = def.iter().any(|&t| {
        matches!(
            tokens[t].word_lower().as_deref(),
            Some("bigint" | "int8" | "bigserial")
        )
    });
    let has_pk = def.windows(2).any(|w| {
        tokens[w[0]].is_word("primary") && tokens[w[1]].is_word("key")
    });

    // Collect "other" modifiers to preserve (DEFAULT, UNIQUE, …), skipping the
    // int/serial type + its (n) args, the auto-inc markers, PRIMARY/KEY,
    // NOT/NULL and mysql UNSIGNED.
    let mut other = Vec::new();
    let mut idx = 1; // skip name
    while idx < def.len() {
        let t = def[idx];
        let wl = tokens[t].word_lower();
        let drop_word = matches!(
            wl.as_deref(),
            Some(w) if AUTOINC_INT_WORDS.contains(&w)
                || matches!(w, "auto_increment" | "autoincrement" | "primary" | "key"
                    | "not" | "null" | "unsigned")
        );
        if drop_word {
            idx += 1;
            // If a type word is followed by (n) args, skip them too.
            if idx < def.len() && matches!(tokens[def[idx]], Token::Punct('(')) {
                let mut d = 0i32;
                while idx < def.len() {
                    match &tokens[def[idx]] {
                        Token::Punct('(') => d += 1,
                        Token::Punct(')') => {
                            d -= 1;
                            idx += 1;
                            if d == 0 {
                                break;
                            }
                            continue;
                        }
                        _ => {}
                    }
                    idx += 1;
                }
            }
            continue;
        }
        other.push(serialize_token(&tokens[t], to));
        idx += 1;
    }
    let mods = if other.is_empty() {
        String::new()
    } else {
        format!(" {}", other.join(" "))
    };

    let name = serialize_token(&tokens[name_ti], to);
    let tail = match to {
        Dialect::Postgres => {
            let base = if big { "BIGSERIAL" } else { "SERIAL" };
            let pk = if has_pk { " PRIMARY KEY" } else { "" };
            format!("{base}{pk}{mods}")
        }
        Dialect::Mysql => {
            let base = if big { "BIGINT" } else { "INT" };
            let pk = if has_pk { " PRIMARY KEY" } else { "" };
            format!("{base}{mods} AUTO_INCREMENT{pk}")
        }
        Dialect::Sqlite => {
            // SQLite requires exactly INTEGER PRIMARY KEY AUTOINCREMENT.
            format!("INTEGER PRIMARY KEY AUTOINCREMENT{mods}")
        }
    };
    out[name_ti] = Some(format!("{name} {tail}"));
    // Drop every other raw token spanned by the definition.
    let last = *def.last().unwrap();
    for entry in out.iter_mut().take(last + 1).skip(name_ti + 1) {
        *entry = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(sql: &str, from: Dialect, to: Dialect) -> String {
        convert(sql, from, to).unwrap()
    }

    #[test]
    fn parse_dialects() {
        assert_eq!(Dialect::parse("PostgreSQL").unwrap(), Dialect::Postgres);
        assert_eq!(Dialect::parse("mariadb").unwrap(), Dialect::Mysql);
        assert_eq!(Dialect::parse("sqlite3").unwrap(), Dialect::Sqlite);
        assert!(Dialect::parse("oracle").is_err());
    }

    #[test]
    fn pg_to_mysql_create_table() {
        let out = conv(
            r#"CREATE TABLE "users" (id SERIAL PRIMARY KEY, name VARCHAR(100), active BOOLEAN);"#,
            Dialect::Postgres,
            Dialect::Mysql,
        );
        assert!(out.contains("`users`"), "identifiers requoted: {out}");
        assert!(out.contains("id INT AUTO_INCREMENT PRIMARY KEY"), "{out}");
        assert!(out.contains("name VARCHAR(100)"), "{out}");
        assert!(out.contains("active TINYINT(1)"), "{out}");
    }

    #[test]
    fn mysql_to_pg_strips_table_options() {
        let out = conv(
            "CREATE TABLE `users` (`id` INT AUTO_INCREMENT PRIMARY KEY, `data` JSON) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
            Dialect::Mysql,
            Dialect::Postgres,
        );
        assert!(out.contains("\"users\""), "{out}");
        assert!(out.contains("\"id\" SERIAL PRIMARY KEY"), "{out}");
        assert!(out.contains("\"data\" JSONB"), "{out}");
        assert!(!out.to_uppercase().contains("ENGINE"), "engine stripped: {out}");
        assert!(!out.to_uppercase().contains("CHARSET"), "charset stripped: {out}");
        assert!(out.trim_end().ends_with(");"), "{out}");
    }

    #[test]
    fn pg_to_sqlite_types_and_autoinc() {
        let out = conv(
            "CREATE TABLE t (id SERIAL PRIMARY KEY, ts TIMESTAMP, amt DOUBLE PRECISION, tag VARCHAR(20));",
            Dialect::Postgres,
            Dialect::Sqlite,
        );
        assert!(out.contains("id INTEGER PRIMARY KEY AUTOINCREMENT"), "{out}");
        assert!(out.contains("ts TEXT"), "{out}");
        assert!(out.contains("amt REAL"), "{out}");
        assert!(out.contains("tag TEXT"), "varchar->text: {out}");
        assert!(!out.contains("DOUBLE"), "double precision consumed: {out}");
    }

    #[test]
    fn sqlite_autoinc_to_mysql() {
        let out = conv(
            "CREATE TABLE t (id INTEGER PRIMARY KEY AUTOINCREMENT, note TEXT);",
            Dialect::Sqlite,
            Dialect::Mysql,
        );
        assert!(out.contains("id INT AUTO_INCREMENT PRIMARY KEY"), "{out}");
        assert!(out.contains("note TEXT"), "{out}");
    }

    #[test]
    fn requotes_identifiers_in_query() {
        let out = conv(
            r#"SELECT "a" FROM "t" WHERE "a" > 1 LIMIT 5;"#,
            Dialect::Postgres,
            Dialect::Mysql,
        );
        assert_eq!(out, "SELECT `a` FROM `t` WHERE `a` > 1 LIMIT 5;");
    }

    #[test]
    fn string_literals_and_comments_preserved() {
        let out = conv(
            "SELECT \"col\" FROM t WHERE name = 'O''Brien' -- keep `this`\n",
            Dialect::Postgres,
            Dialect::Mysql,
        );
        assert!(out.contains("`col`"), "{out}");
        assert!(out.contains("'O''Brien'"), "string literal untouched: {out}");
        assert!(out.contains("-- keep `this`"), "comment untouched: {out}");
    }

    #[test]
    fn bracket_identifiers_convert() {
        let out = conv("SELECT [order] FROM [my table];", Dialect::Sqlite, Dialect::Postgres);
        assert_eq!(out, r#"SELECT "order" FROM "my table";"#);
    }

    #[test]
    fn same_dialect_is_noop() {
        let src = "CREATE TABLE `x` (id INT AUTO_INCREMENT) ENGINE=InnoDB;";
        assert_eq!(conv(src, Dialect::Mysql, Dialect::Mysql), src);
    }

    #[test]
    fn unknown_types_pass_through() {
        let out = conv(
            "CREATE TABLE t (loc GEOMETRY, id BIGINT);",
            Dialect::Mysql,
            Dialect::Postgres,
        );
        assert!(out.contains("loc GEOMETRY"), "unknown type kept: {out}");
        assert!(out.contains("id BIGINT"), "{out}");
    }

    #[test]
    fn errors_on_empty() {
        assert!(convert("   ", Dialect::Postgres, Dialect::Mysql).is_err());
    }
}
