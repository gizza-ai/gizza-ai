//! sql-playground core — run SQL against a fresh in-memory database, pure Rust.
//!
//! Backed by GlueSQL (an SQL engine written entirely in Rust), so it
//! instantiates inside the wafer wasm runtime with no C/SQLite/native deps. A
//! new in-memory store is created per call; the result of the LAST statement is
//! rendered as an aligned table, CSV, or JSON.

use futures::executor::block_on;
use gluesql_core::prelude::{Glue, Payload, Value};
use gluesql_memory_storage::MemoryStorage;

/// Run `sql` (one or more `;`-separated statements) against a fresh in-memory
/// database and render the LAST statement's result in `format`
/// (`"table"` | `"csv"` | `"json"`; blank → `"table"`).
pub fn run(sql: &str, format: &str) -> Result<String, String> {
    if sql.trim().is_empty() {
        return Err("no SQL provided".into());
    }
    validate_format(format)?;

    let storage = MemoryStorage::default();
    let mut glue = Glue::new(storage);
    let payloads = block_on(glue.execute(sql)).map_err(|e| e.to_string())?;
    let last = payloads
        .last()
        .ok_or("no SQL statements were executed")?;

    Ok(render(last, format))
}

fn validate_format(format: &str) -> Result<(), String> {
    match format {
        "" | "table" | "csv" | "json" => Ok(()),
        other => Err(format!(
            "invalid format {other:?}: expected \"table\", \"csv\", or \"json\""
        )),
    }
}

fn render(payload: &Payload, format: &str) -> String {
    match payload {
        Payload::Select { labels, rows } => {
            let cells: Vec<Vec<String>> = rows
                .iter()
                .map(|r| r.iter().map(value_to_string).collect())
                .collect();
            match format {
                "csv" => render_csv(labels, &cells),
                "json" => render_json(labels, &cells),
                _ => render_table(labels, &cells),
            }
        }
        Payload::SelectMap(maps) => {
            // Schemaless SELECT: derive column order from the first row.
            let mut labels: Vec<String> = Vec::new();
            for m in maps {
                for k in m.keys() {
                    if !labels.contains(k) {
                        labels.push(k.clone());
                    }
                }
            }
            let cells: Vec<Vec<String>> = maps
                .iter()
                .map(|m| {
                    labels
                        .iter()
                        .map(|l| m.get(l).map(value_to_string).unwrap_or_default())
                        .collect()
                })
                .collect();
            match format {
                "csv" => render_csv(&labels, &cells),
                "json" => render_json(&labels, &cells),
                _ => render_table(&labels, &cells),
            }
        }
        Payload::Insert(n) => format!("INSERT: {n} row(s) inserted"),
        Payload::Update(n) => format!("UPDATE: {n} row(s) updated"),
        Payload::Delete(n) => format!("DELETE: {n} row(s) deleted"),
        Payload::Create => "CREATE TABLE: ok".into(),
        Payload::DropTable(n) => format!("DROP TABLE: {n} table(s) dropped"),
        Payload::AlterTable => "ALTER TABLE: ok".into(),
        Payload::CreateIndex => "CREATE INDEX: ok".into(),
        Payload::DropIndex => "DROP INDEX: ok".into(),
        other => format!("{other:?}"),
    }
}

/// Render a gluesql `Value` to a plain display string (no Rust-debug quoting).
fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null => "NULL".into(),
        Value::Bool(b) => b.to_string(),
        Value::I8(n) => n.to_string(),
        Value::I16(n) => n.to_string(),
        Value::I32(n) => n.to_string(),
        Value::I64(n) => n.to_string(),
        Value::I128(n) => n.to_string(),
        Value::U8(n) => n.to_string(),
        Value::U16(n) => n.to_string(),
        Value::U32(n) => n.to_string(),
        Value::U64(n) => n.to_string(),
        Value::U128(n) => n.to_string(),
        Value::F32(n) => n.to_string(),
        Value::F64(n) => n.to_string(),
        Value::Decimal(d) => d.to_string(),
        Value::Str(s) => s.clone(),
        Value::Bytea(b) => hex_encode(b),
        Value::Date(d) => d.to_string(),
        Value::Timestamp(t) => t.to_string(),
        Value::Time(t) => t.to_string(),
        Value::Uuid(u) => format!("{u:032x}"),
        other => format!("{other:?}"),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn render_table(labels: &[String], rows: &[Vec<String>]) -> String {
    if labels.is_empty() {
        return "(no columns)".into();
    }
    let mut widths: Vec<usize> = labels.iter().map(|l| l.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
    }
    let sep = |out: &mut String| {
        out.push('+');
        for w in &widths {
            out.push_str(&"-".repeat(w + 2));
            out.push('+');
        }
        out.push('\n');
    };
    let line = |out: &mut String, cols: &[String]| {
        out.push('|');
        for (i, w) in widths.iter().enumerate() {
            let cell = cols.get(i).map(String::as_str).unwrap_or("");
            let pad = w - cell.chars().count();
            out.push(' ');
            out.push_str(cell);
            out.push_str(&" ".repeat(pad));
            out.push(' ');
            out.push('|');
        }
        out.push('\n');
    };
    let mut out = String::new();
    sep(&mut out);
    line(&mut out, labels);
    sep(&mut out);
    for row in rows {
        line(&mut out, row);
    }
    sep(&mut out);
    out.push_str(&format!("{} row(s)\n", rows.len()));
    out
}

fn render_csv(labels: &[String], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str(&labels.iter().map(|s| csv_escape(s)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for row in rows {
        out.push_str(&row.iter().map(|s| csv_escape(s)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_json(labels: &[String], rows: &[Vec<String>]) -> String {
    let arr: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (i, label) in labels.iter().enumerate() {
                let cell = row.get(i).cloned().unwrap_or_default();
                obj.insert(label.clone(), serde_json::Value::String(cell));
            }
            serde_json::Value::Object(obj)
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_else(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_insert_select_table() {
        let out = run(
            "CREATE TABLE t (id INTEGER, name TEXT); INSERT INTO t VALUES (1, 'Ann'), (2, 'Bob'); SELECT * FROM t ORDER BY id;",
            "table",
        )
        .unwrap();
        assert!(out.contains("Ann"), "got: {out}");
        assert!(out.contains("Bob"), "got: {out}");
        assert!(out.contains("| id"), "header missing: {out}");
        assert!(out.contains("2 row(s)"), "row count missing: {out}");
    }

    #[test]
    fn select_csv() {
        let out = run(
            "CREATE TABLE t (id INTEGER, name TEXT); INSERT INTO t VALUES (1, 'Ann'); SELECT * FROM t;",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "id,name\n1,Ann\n");
    }

    #[test]
    fn select_json() {
        let out = run(
            "CREATE TABLE t (id INTEGER, name TEXT); INSERT INTO t VALUES (1, 'Ann'); SELECT * FROM t;",
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["id"], "1");
        assert_eq!(v[0]["name"], "Ann");
    }

    #[test]
    fn where_and_aggregate() {
        let out = run(
            "CREATE TABLE n (x INTEGER); INSERT INTO n VALUES (1),(2),(3),(4); SELECT SUM(x) AS total FROM n WHERE x > 1;",
            "csv",
        )
        .unwrap();
        // 2+3+4 = 9
        assert!(out.contains("9"), "got: {out}");
    }

    #[test]
    fn non_select_reports_affected() {
        let out = run(
            "CREATE TABLE t (a INTEGER); INSERT INTO t VALUES (1),(2),(3);",
            "table",
        )
        .unwrap();
        assert!(out.contains("INSERT"), "got: {out}");
        assert!(out.contains('3'), "got: {out}");
    }

    #[test]
    fn null_renders_as_null() {
        let out = run(
            "CREATE TABLE t (a INTEGER, b TEXT); INSERT INTO t VALUES (1, NULL); SELECT * FROM t;",
            "csv",
        )
        .unwrap();
        assert!(out.contains("NULL"), "got: {out}");
    }

    #[test]
    fn syntax_error_is_reported() {
        let err = run("SELECT FROM WHERE", "table").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn rejects_bad_format() {
        let err = run("SELECT 1 AS x;", "xml").unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }

    #[test]
    fn empty_sql_rejected() {
        let err = run("   ", "table").unwrap_err();
        assert!(err.contains("no SQL"), "got: {err}");
    }
}
