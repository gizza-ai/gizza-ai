//! sqlite-db-inspector core — inspect an SQLite database file by reading the
//! on-disk schema catalog directly (no SQL engine, no user queries).

use gizza_ai_sqlite_table_to_csv_core::{count_rows, master_entries, MasterEntry};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!("unknown format {other:?} (expected: markdown or json)")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub format: OutputFormat,
    pub include_internal: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: OutputFormat::Markdown,
            include_internal: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseReport {
    pub table_count: usize,
    pub index_count: usize,
    pub view_count: usize,
    pub trigger_count: usize,
    pub tables: Vec<TableInfo>,
    pub views: Vec<SchemaObject>,
    pub triggers: Vec<SchemaObject>,
    pub internal_objects_omitted: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub row_count: Option<u64>,
    pub row_count_note: Option<String>,
    pub without_rowid: bool,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub create_sql: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
    pub not_null: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexInfo {
    pub name: String,
    pub unique: bool,
    pub auto_created: bool,
    pub columns: Vec<String>,
    pub create_sql: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForeignKeyInfo {
    pub columns: Vec<String>,
    pub references_table: String,
    pub references_columns: Vec<String>,
    pub clause: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaObject {
    pub name: String,
    pub table: String,
    pub create_sql: String,
}

pub fn inspect_database(bytes: &[u8], opts: &Options) -> Result<DatabaseReport, String> {
    let entries = master_entries(bytes)?;
    let mut internal_objects_omitted = 0usize;
    let visible: Vec<MasterEntry> = entries
        .into_iter()
        .filter(|e| {
            let internal = e.name.starts_with("sqlite_") || e.tbl_name.starts_with("sqlite_");
            if internal && !opts.include_internal {
                internal_objects_omitted += 1;
                false
            } else {
                true
            }
        })
        .collect();

    let mut tables = Vec::new();
    for entry in visible.iter().filter(|e| e.kind == "table") {
        let without_rowid = sql_has_without_rowid(&entry.sql);
        let is_internal = entry.name.starts_with("sqlite_");
        let row_count = if is_internal {
            None
        } else {
            match count_rows(bytes, &entry.name) {
                Ok(v) => v,
                Err(_) if without_rowid => None,
                Err(e) => return Err(format!("count rows for table {:?}: {e}", entry.name)),
            }
        };
        let row_count_note = if is_internal {
            Some("row count skipped for SQLite internal catalog table".to_string())
        } else if without_rowid {
            Some("row count unavailable: WITHOUT ROWID tables use an index b-tree layout this inspector does not count yet".to_string())
        } else {
            None
        };
        let indexes = visible
            .iter()
            .filter(|idx| idx.kind == "index" && idx.tbl_name.eq_ignore_ascii_case(&entry.name))
            .map(index_info)
            .collect();
        tables.push(TableInfo {
            name: entry.name.clone(),
            row_count,
            row_count_note,
            without_rowid,
            columns: parse_columns(&entry.sql),
            indexes,
            foreign_keys: parse_foreign_keys(&entry.sql),
            create_sql: entry.sql.clone(),
        });
    }

    let views: Vec<SchemaObject> = visible
        .iter()
        .filter(|e| e.kind == "view")
        .map(schema_object)
        .collect();
    let triggers: Vec<SchemaObject> = visible
        .iter()
        .filter(|e| e.kind == "trigger")
        .map(schema_object)
        .collect();
    let index_count = visible.iter().filter(|e| e.kind == "index").count();

    Ok(DatabaseReport {
        table_count: tables.len(),
        index_count,
        view_count: views.len(),
        trigger_count: triggers.len(),
        tables,
        views,
        triggers,
        internal_objects_omitted,
    })
}

pub fn render_report(report: &DatabaseReport, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string()),
        OutputFormat::Markdown => render_markdown(report),
    }
}

fn render_markdown(report: &DatabaseReport) -> String {
    let mut out = String::new();
    out.push_str("# SQLite database inspection\n\n");
    out.push_str(&format!(
        "Found {} table(s), {} index(es), {} view(s), and {} trigger(s).",
        report.table_count, report.index_count, report.view_count, report.trigger_count
    ));
    if report.internal_objects_omitted > 0 {
        out.push_str(&format!(" Omitted {} internal sqlite_* object(s).", report.internal_objects_omitted));
    }
    out.push_str("\n\n");

    for table in &report.tables {
        out.push_str(&format!("## Table `{}`\n\n", table.name));
        match table.row_count {
            Some(n) => out.push_str(&format!("Rows: **{}**\n\n", n)),
            None => out.push_str(&format!(
                "Rows: not counted ({})\n\n",
                table.row_count_note.as_deref().unwrap_or("unsupported table layout")
            )),
        }
        if table.columns.is_empty() {
            out.push_str("Columns: not parsed from CREATE TABLE SQL.\n\n");
        } else {
            out.push_str("| Column | Type | NOT NULL | Primary key | Default |\n");
            out.push_str("|---|---|---:|---:|---|\n");
            for c in &table.columns {
                out.push_str(&format!(
                    "| `{}` | {} | {} | {} | {} |\n",
                    escape_md(&c.name),
                    if c.type_name.is_empty() { "".to_string() } else { format!("`{}`", escape_md(&c.type_name)) },
                    yes_no(c.not_null),
                    yes_no(c.primary_key),
                    c.default_value.as_deref().map(escape_md).unwrap_or_default()
                ));
            }
            out.push('\n');
        }
        if !table.indexes.is_empty() {
            out.push_str("Indexes:\n");
            for idx in &table.indexes {
                let cols = if idx.columns.is_empty() { "columns not parsed".to_string() } else { idx.columns.join(", ") };
                out.push_str(&format!(
                    "- `{}`{}{} on {}\n",
                    idx.name,
                    if idx.unique { " (unique)" } else { "" },
                    if idx.auto_created { " (auto-created)" } else { "" },
                    cols
                ));
            }
            out.push('\n');
        }
        if !table.foreign_keys.is_empty() {
            out.push_str("Foreign keys:\n");
            for fk in &table.foreign_keys {
                out.push_str(&format!(
                    "- ({}) references `{}` ({})\n",
                    fk.columns.join(", "),
                    fk.references_table,
                    fk.references_columns.join(", ")
                ));
            }
            out.push('\n');
        }
    }

    if !report.views.is_empty() {
        out.push_str("## Views\n\n");
        for v in &report.views {
            out.push_str(&format!("- `{}`\n", v.name));
        }
        out.push('\n');
    }
    if !report.triggers.is_empty() {
        out.push_str("## Triggers\n\n");
        for t in &report.triggers {
            out.push_str(&format!("- `{}` on `{}`\n", t.name, t.table));
        }
        out.push('\n');
    }
    out
}

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

fn escape_md(s: &str) -> String {
    s.replace('|', "\\|")
}

fn schema_object(e: &MasterEntry) -> SchemaObject {
    SchemaObject { name: e.name.clone(), table: e.tbl_name.clone(), create_sql: e.sql.clone() }
}

fn index_info(e: &MasterEntry) -> IndexInfo {
    IndexInfo {
        name: e.name.clone(),
        unique: e.sql.to_ascii_uppercase().contains("CREATE UNIQUE INDEX"),
        auto_created: e.sql.trim().is_empty() || e.name.starts_with("sqlite_autoindex"),
        columns: parse_index_columns(&e.sql),
        create_sql: e.sql.clone(),
    }
}

fn parse_columns(sql: &str) -> Vec<ColumnInfo> {
    let Some(body) = paren_body(sql) else { return Vec::new(); };
    split_top_level(&body, ',')
        .into_iter()
        .filter_map(|part| parse_column_def(&part))
        .collect()
}

fn parse_column_def(def: &str) -> Option<ColumnInfo> {
    let def = def.trim();
    if def.is_empty() { return None; }
    let first = first_word(def)?.to_ascii_uppercase();
    if matches!(first.as_str(), "CONSTRAINT" | "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK") {
        return None;
    }
    let (name, rest) = take_ident(def)?;
    let tokens = tokenize_sql(rest);
    let mut type_parts = Vec::new();
    let mut not_null = false;
    let mut primary_key = false;
    let mut default_value = None;
    let constraints = ["CONSTRAINT", "PRIMARY", "NOT", "NULL", "DEFAULT", "COLLATE", "REFERENCES", "CHECK", "UNIQUE", "GENERATED", "AS"];
    let mut i = 0usize;
    while i < tokens.len() {
        let up = tokens[i].to_ascii_uppercase();
        if constraints.contains(&up.as_str()) { break; }
        type_parts.push(tokens[i].clone());
        i += 1;
    }
    while i < tokens.len() {
        let up = tokens[i].to_ascii_uppercase();
        if up == "NOT" && tokens.get(i + 1).map(|t| t.eq_ignore_ascii_case("NULL")).unwrap_or(false) {
            not_null = true;
            i += 2;
        } else if up == "PRIMARY" && tokens.get(i + 1).map(|t| t.eq_ignore_ascii_case("KEY")).unwrap_or(false) {
            primary_key = true;
            i += 2;
        } else if up == "DEFAULT" {
            if let Some(v) = tokens.get(i + 1) { default_value = Some(v.clone()); }
            i += 2;
        } else {
            i += 1;
        }
    }
    Some(ColumnInfo { name, type_name: type_parts.join(" "), not_null, primary_key, default_value })
}

fn parse_index_columns(sql: &str) -> Vec<String> {
    paren_body(sql)
        .map(|body| split_top_level(&body, ',').into_iter().map(|s| clean_ident(s.trim())).collect())
        .unwrap_or_default()
}

fn parse_foreign_keys(sql: &str) -> Vec<ForeignKeyInfo> {
    let mut out = Vec::new();
    let Some(body) = paren_body(sql) else { return out; };
    for part in split_top_level(&body, ',') {
        let up = part.to_ascii_uppercase();
        if up.trim_start().starts_with("FOREIGN KEY") {
            if let Some(fk) = parse_table_fk(&part) { out.push(fk); }
        } else if up.contains(" REFERENCES ") {
            if let Some(fk) = parse_inline_fk(&part) { out.push(fk); }
        }
    }
    out
}

fn parse_table_fk(part: &str) -> Option<ForeignKeyInfo> {
    let local_body = paren_body(part)?;
    let after_refs = after_keyword(part, "REFERENCES")?;
    let (table, rest) = take_ident(after_refs.trim())?;
    let refs = paren_body(rest).map(|b| split_top_level(&b, ',').into_iter().map(|s| clean_ident(s.trim())).collect()).unwrap_or_default();
    Some(ForeignKeyInfo { columns: split_top_level(&local_body, ',').into_iter().map(|s| clean_ident(s.trim())).collect(), references_table: table, references_columns: refs, clause: part.trim().to_string() })
}

fn parse_inline_fk(part: &str) -> Option<ForeignKeyInfo> {
    let (col, rest) = take_ident(part.trim())?;
    let after_refs = after_keyword(rest, "REFERENCES")?;
    let (table, rest) = take_ident(after_refs.trim())?;
    let refs = paren_body(rest).map(|b| split_top_level(&b, ',').into_iter().map(|s| clean_ident(s.trim())).collect()).unwrap_or_default();
    Some(ForeignKeyInfo { columns: vec![col], references_table: table, references_columns: refs, clause: part.trim().to_string() })
}

fn paren_body(sql: &str) -> Option<String> {
    let start = sql.find('(')?;
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut end = None;
    for (idx, ch) in sql.char_indices().skip_while(|(i, _)| *i < start) {
        if let Some(q) = quote {
            if ch == q { quote = None; }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') { quote = Some(ch); continue; }
        if ch == '(' { depth += 1; }
        if ch == ')' {
            depth -= 1;
            if depth == 0 { end = Some(idx); break; }
        }
    }
    sql.get(start + 1..end?).map(str::to_string)
}

fn split_top_level(s: &str, delim: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0usize;
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    for (idx, ch) in chars {
        if let Some(q) = quote {
            if ch == q { quote = None; }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') { quote = Some(ch); continue; }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == delim && depth == 0 => {
                parts.push(s[start..idx].trim().to_string());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(s[start..].trim().to_string());
    parts
}

fn tokenize_sql(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        if let Some(q) = quote {
            cur.push(ch);
            if ch == q { quote = None; }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
            cur.push(ch);
        } else if ch.is_whitespace() || matches!(ch, ',' | '(' | ')') {
            if !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

fn first_word(s: &str) -> Option<&str> {
    s.split_whitespace().next()
}

fn take_ident(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let mut chars = s.char_indices();
    let (_, first) = chars.next()?;
    if matches!(first, '"' | '`' | '[') {
        let close = if first == '[' { ']' } else { first };
        for (idx, ch) in chars {
            if ch == close {
                return Some((clean_ident(&s[..=idx]), &s[idx + ch.len_utf8()..]));
            }
        }
        None
    } else {
        let end = s.find(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | ',')).unwrap_or(s.len());
        Some((clean_ident(&s[..end]), &s[end..]))
    }
}

fn clean_ident(s: &str) -> String {
    s.trim().trim_matches('"').trim_matches('`').trim_matches('[').trim_matches(']').to_string()
}

fn after_keyword<'a>(s: &'a str, kw: &str) -> Option<&'a str> {
    let up = s.to_ascii_uppercase();
    let pos = up.find(kw)?;
    s.get(pos + kw.len()..)
}

fn sql_has_without_rowid(sql: &str) -> bool {
    let normalized = sql.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase();
    normalized.contains("WITHOUT ROWID")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARY: &[u8] = include_bytes!("../tests/fixtures/library.db");

    #[test]
    fn inspects_tables_indexes_views_foreign_keys_and_counts() {
        let report = inspect_database(LIBRARY, &Options::default()).unwrap();
        assert_eq!(report.table_count, 4);
        assert_eq!(report.index_count, 2); // two explicit indexes; sqlite_* autoindexes are omitted by default
        assert_eq!(report.view_count, 1);
        let books = report.tables.iter().find(|t| t.name == "books").unwrap();
        assert_eq!(books.row_count, Some(4));
        assert!(books.columns.iter().any(|c| c.name == "title" && c.type_name == "TEXT" && c.not_null));
        assert!(books.indexes.iter().any(|i| i.name == "idx_books_title" && i.unique));
        assert!(books.indexes.iter().any(|i| i.name == "idx_books_author" && i.columns == vec!["author_id", "year"]));
        assert!(books.foreign_keys.iter().any(|fk| fk.columns == vec!["author_id"] && fk.references_table == "authors"));
        assert!(report.views.iter().any(|v| v.name == "book_titles"));
    }

    #[test]
    fn reports_without_rowid_row_count_as_unavailable() {
        let report = inspect_database(LIBRARY, &Options::default()).unwrap();
        let settings = report.tables.iter().find(|t| t.name == "settings").unwrap();
        assert!(settings.without_rowid);
        assert_eq!(settings.row_count, None);
        assert!(settings.row_count_note.as_deref().unwrap().contains("WITHOUT ROWID"));
    }

    #[test]
    fn invalid_bytes_error() {
        let err = inspect_database(b"not sqlite", &Options::default()).unwrap_err();
        assert!(err.to_ascii_lowercase().contains("sqlite"));
    }

    #[test]
    fn renders_markdown_and_json() {
        let report = inspect_database(LIBRARY, &Options::default()).unwrap();
        let md = render_report(&report, OutputFormat::Markdown);
        assert!(md.contains("Table `books`"));
        assert!(md.contains("idx_books_author"));
        let json = render_report(&report, OutputFormat::Json);
        assert!(json.contains("\"tables\""));
        assert!(json.contains("book_titles"));
    }

    #[test]
    fn output_format_parse_rejects_unknown() {
        assert_eq!(OutputFormat::parse("md").unwrap(), OutputFormat::Markdown);
        assert!(OutputFormat::parse("xml").is_err());
    }
}
