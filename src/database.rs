use std::collections::HashMap;

use wafer_core::interfaces::database::service::*;

use crate::bridge;

/// Browser-side DatabaseService backed by sql.js via the JS bridge.
///
/// All SQL execution is synchronous through `bridge::db_exec_raw` and
/// `bridge::db_query_raw`.  After every write, `bridge::dbFlush().await` is
/// called to persist the in-memory sql.js database to OPFS.
pub struct BrowserDatabaseService;

// Safety: wasm32-unknown-unknown is single-threaded — no data races possible.
unsafe impl Send for BrowserDatabaseService {}
unsafe impl Sync for BrowserDatabaseService {}

// ─── SQL helpers ─────────────────────────────────────────────────────────────

/// Allow only alphanumeric + underscore characters to prevent SQL injection.
fn sanitize_ident(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Quote an identifier using SQLite double-quote style.
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Serialize a slice of `serde_json::Value` params into a JSON array string
/// for passing to the bridge functions.
fn params_to_json(params: &[serde_json::Value]) -> String {
    serde_json::to_string(params).unwrap_or_else(|_| "[]".to_string())
}

/// Map a JSON value to a scalar suitable for embedding in a params array.
/// Arrays and objects are serialized as JSON strings (sql.js passes them as TEXT).
fn coerce_param(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::Value::String(v.to_string())
        }
        other => other.clone(),
    }
}

/// Build a WHERE clause and accompanying params from a filter slice.
///
/// Returns `(where_clause_string, params_vec)` where the params_vec starts
/// at index `start_idx` (1-based `?` placeholders are not used — sql.js uses
/// positional `?` without numbering, so params are appended in order).
fn build_where(filters: &[Filter]) -> (String, Vec<serde_json::Value>) {
    if filters.is_empty() {
        return (String::new(), Vec::new());
    }

    let mut parts = Vec::new();
    let mut params: Vec<serde_json::Value> = Vec::new();

    for f in filters {
        let col = quote_ident(&sanitize_ident(&f.field));
        match f.operator {
            FilterOp::IsNull => parts.push(format!("{} IS NULL", col)),
            FilterOp::IsNotNull => parts.push(format!("{} IS NOT NULL", col)),
            FilterOp::In => {
                if let serde_json::Value::Array(arr) = &f.value {
                    if arr.is_empty() {
                        // IN () is never true — use a literal false expression
                        parts.push("1=0".to_string());
                    } else {
                        let placeholders: Vec<&str> =
                            std::iter::repeat("?").take(arr.len()).collect();
                        parts.push(format!("{} IN ({})", col, placeholders.join(", ")));
                        for item in arr {
                            params.push(coerce_param(item));
                        }
                    }
                }
                // If value is not an array, skip the filter entirely.
            }
            _ => {
                let op = f.operator.as_sql();
                parts.push(format!("{} {} ?", col, op));
                params.push(coerce_param(&f.value));
            }
        }
    }

    if parts.is_empty() {
        return (String::new(), Vec::new());
    }

    (format!("WHERE {}", parts.join(" AND ")), params)
}

/// Parse the JSON array returned by `db_query_raw` into `Vec<Record>`.
fn parse_rows(json: &str) -> Result<Vec<Record>, DatabaseError> {
    let rows: Vec<serde_json::Value> = serde_json::from_str(json)
        .map_err(|e| DatabaseError::Internal(format!("parse rows: {}", e)))?;

    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        let obj = match row {
            serde_json::Value::Object(map) => map,
            _ => return Err(DatabaseError::Internal("expected row object".to_string())),
        };

        let mut data: HashMap<String, serde_json::Value> = HashMap::new();
        let mut id = String::new();

        for (k, v) in obj {
            // Try to parse JSON strings that look like objects/arrays back into
            // structured values (sql.js stores JSON columns as TEXT).
            let parsed = match &v {
                serde_json::Value::String(s)
                    if (s.starts_with('{') && s.ends_with('}'))
                        || (s.starts_with('[') && s.ends_with(']')) =>
                {
                    serde_json::from_str(s).unwrap_or(v.clone())
                }
                other => other.clone(),
            };

            if k == "id" {
                id = match &parsed {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => String::new(),
                };
            }
            data.insert(k, parsed);
        }

        records.push(Record { id, data });
    }

    Ok(records)
}

/// Parse the rows-modified count string returned by `db_exec_raw`.
fn parse_rows_modified(json: &str) -> i64 {
    json.trim().parse::<i64>().unwrap_or(0)
}

/// Return current UTC timestamp as an RFC 3339 string.
fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Generate a random hex ID using `getrandom`.
fn new_id() -> Result<String, DatabaseError> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| DatabaseError::Internal(format!("getrandom failed: {e}")))?;
    Ok(hex::encode(bytes))
}

// ─── ensure_schema_table helpers ─────────────────────────────────────────────

fn data_type_to_sqlite(dt: DataType) -> &'static str {
    match dt {
        DataType::String | DataType::Text => "TEXT",
        DataType::Int | DataType::Int64 => "INTEGER",
        DataType::Float => "REAL",
        DataType::Bool => "INTEGER",
        DataType::DateTime => "DATETIME",
        DataType::Json => "TEXT",
        DataType::Blob => "BLOB",
    }
}

fn default_val_to_sql(d: &DefaultValue) -> String {
    if d.is_null {
        return "NULL".to_string();
    }
    if d.is_raw {
        // sql.js uses CURRENT_TIMESTAMP directly
        return d.raw.clone();
    }
    match &d.value {
        Some(DefaultVal::String(s)) => format!("'{}'", s.replace('\'', "''")),
        Some(DefaultVal::Int(i)) => i.to_string(),
        Some(DefaultVal::Float(f)) => f.to_string(),
        Some(DefaultVal::Bool(b)) => if *b { "1" } else { "0" }.to_string(),
        None => "NULL".to_string(),
    }
}

fn column_def_to_sql(col: &Column) -> String {
    let qname = quote_ident(&col.name);

    if col.auto_increment {
        return format!("{} INTEGER PRIMARY KEY AUTOINCREMENT", qname);
    }

    let mut sql = format!("{} {}", qname, data_type_to_sqlite(col.data_type));

    if col.primary_key {
        sql.push_str(" PRIMARY KEY");
    } else {
        if !col.nullable {
            sql.push_str(" NOT NULL");
        }
        if col.unique {
            sql.push_str(" UNIQUE");
        }
    }

    if let Some(ref d) = col.default {
        sql.push_str(" DEFAULT ");
        sql.push_str(&default_val_to_sql(d));
    }

    sql
}

/// Build a CREATE TABLE IF NOT EXISTS statement from a schema Table.
fn build_create_table_sql(table: &Table) -> String {
    let qtable = quote_ident(&table.name);
    let mut parts: Vec<String> = table.columns.iter().map(column_def_to_sql).collect();

    if !table.primary_key.is_empty() {
        let quoted: Vec<String> = table.primary_key.iter().map(|k| quote_ident(k)).collect();
        parts.push(format!("PRIMARY KEY({})", quoted.join(", ")));
    }

    for uk in &table.unique_keys {
        let quoted: Vec<String> = uk.iter().map(|k| quote_ident(k)).collect();
        parts.push(format!("UNIQUE({})", quoted.join(", ")));
    }

    for col in &table.columns {
        if let Some(ref refs) = col.references {
            let mut fk = format!(
                "FOREIGN KEY ({}) REFERENCES {}({})",
                quote_ident(&col.name),
                quote_ident(&refs.table),
                quote_ident(&refs.column),
            );
            if !refs.on_delete.is_empty() {
                fk.push_str(&format!(" ON DELETE {}", sanitize_ident(&refs.on_delete)));
            }
            if !refs.on_update.is_empty() {
                fk.push_str(&format!(" ON UPDATE {}", sanitize_ident(&refs.on_update)));
            }
            parts.push(fk);
        }
    }

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n    {}\n)",
        qtable,
        parts.join(",\n    ")
    )
}

/// Build a CREATE INDEX IF NOT EXISTS statement.
fn build_create_index_sql(table_name: &str, idx: &Index) -> String {
    let unique = if idx.unique { "UNIQUE " } else { "" };
    let name = if idx.name.is_empty() {
        format!(
            "idx_{}_{}",
            sanitize_ident(table_name),
            idx.columns
                .iter()
                .map(|c| sanitize_ident(c))
                .collect::<Vec<_>>()
                .join("_")
        )
    } else {
        sanitize_ident(&idx.name)
    };
    let cols: Vec<String> = idx.columns.iter().map(|c| quote_ident(c)).collect();
    format!(
        "CREATE {}INDEX IF NOT EXISTS {} ON {}({})",
        unique,
        name,
        quote_ident(table_name),
        cols.join(", ")
    )
}

/// Retrieve existing column names for a table via PRAGMA table_info.
fn existing_columns(table: &str) -> Vec<String> {
    let sql = format!("PRAGMA table_info({})", quote_ident(table));
    let result = bridge::db_query_raw(&sql, "[]");
    let rows: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap_or_default();
    rows.into_iter()
        .filter_map(|r| {
            r.get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
        })
        .collect()
}

/// Check whether a table exists in the sqlite_master catalog.
fn table_exists_sync(name: &str) -> bool {
    let sql = "SELECT COUNT(*) as cnt FROM sqlite_master WHERE type='table' AND name=?";
    let params = params_to_json(&[serde_json::Value::String(name.to_string())]);
    let result = bridge::db_query_raw(sql, &params);
    let rows: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap_or_default();
    rows.first()
        .and_then(|r| r.get("cnt"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        > 0
}

// ─── DatabaseService impl ─────────────────────────────────────────────────────

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl DatabaseService for BrowserDatabaseService {
    // ── get ──────────────────────────────────────────────────────────────────

    async fn get(&self, collection: &str, id: &str) -> Result<Record, DatabaseError> {
        let table = sanitize_ident(collection);
        if !table_exists_sync(&table) {
            return Err(DatabaseError::NotFound);
        }

        let sql = format!("SELECT * FROM {} WHERE id = ?", quote_ident(&table));
        let params = params_to_json(&[serde_json::Value::String(id.to_string())]);
        let json = bridge::db_query_raw(&sql, &params);

        let records = parse_rows(&json)?;
        records.into_iter().next().ok_or(DatabaseError::NotFound)
    }

    // ── list ─────────────────────────────────────────────────────────────────

    async fn list(
        &self,
        collection: &str,
        opts: &ListOptions,
    ) -> Result<RecordList, DatabaseError> {
        let table = sanitize_ident(collection);
        if !table_exists_sync(&table) {
            return Ok(RecordList {
                records: Vec::new(),
                total_count: 0,
                page: 1,
                page_size: if opts.limit > 0 { opts.limit } else { 0 },
            });
        }

        let (where_clause, filter_params) = build_where(&opts.filters);

        // COUNT
        let count_sql = format!(
            "SELECT COUNT(*) as cnt FROM {} {}",
            quote_ident(&table),
            where_clause
        );
        let params_json = params_to_json(&filter_params);
        let count_json = bridge::db_query_raw(&count_sql, &params_json);
        let count_rows: Vec<serde_json::Value> =
            serde_json::from_str(&count_json).unwrap_or_default();
        let total_count: i64 = count_rows
            .first()
            .and_then(|r| r.get("cnt"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // SELECT
        let mut select_sql = format!("SELECT * FROM {} {}", quote_ident(&table), where_clause);

        if !opts.sort.is_empty() {
            let order_parts: Vec<String> = opts
                .sort
                .iter()
                .map(|s| {
                    let dir = if s.desc { "DESC" } else { "ASC" };
                    format!("{} {}", quote_ident(&sanitize_ident(&s.field)), dir)
                })
                .collect();
            select_sql.push_str(&format!(" ORDER BY {}", order_parts.join(", ")));
        }

        if opts.limit > 0 {
            select_sql.push_str(&format!(" LIMIT {}", opts.limit));
        }
        if opts.offset > 0 {
            select_sql.push_str(&format!(" OFFSET {}", opts.offset));
        }

        let rows_json = bridge::db_query_raw(&select_sql, &params_json);
        let records = parse_rows(&rows_json)?;

        let page = if opts.limit > 0 {
            (opts.offset / opts.limit) + 1
        } else {
            1
        };

        Ok(RecordList {
            records,
            total_count,
            page,
            page_size: if opts.limit > 0 {
                opts.limit
            } else {
                total_count
            },
        })
    }

    // ── create ────────────────────────────────────────────────────────────────

    async fn create(
        &self,
        collection: &str,
        mut data: HashMap<String, serde_json::Value>,
    ) -> Result<Record, DatabaseError> {
        let table = sanitize_ident(collection);

        // Auto-generate a random ID if not provided
        if !data.contains_key("id") {
            data.insert("id".to_string(), serde_json::Value::String(new_id()?));
        }

        // Auto-set timestamps
        let now = now_rfc3339();
        if !data.contains_key("created_at") {
            data.insert(
                "created_at".to_string(),
                serde_json::Value::String(now.clone()),
            );
        }
        if !data.contains_key("updated_at") {
            data.insert("updated_at".to_string(), serde_json::Value::String(now));
        }

        // Auto-create the table if it does not exist
        if !table_exists_sync(&table) {
            let mut col_defs = Vec::new();
            col_defs.push(format!("{} TEXT PRIMARY KEY", quote_ident("id")));
            for key in data.keys() {
                if key != "id" {
                    col_defs.push(format!("{} TEXT", quote_ident(&sanitize_ident(key))));
                }
            }
            let create_sql = format!(
                "CREATE TABLE IF NOT EXISTS {} ({})",
                quote_ident(&table),
                col_defs.join(", ")
            );
            bridge::db_exec_raw(&create_sql, "[]");
        } else {
            // Ensure any new columns exist
            let existing = existing_columns(&table);
            for key in data.keys() {
                let safe_key = sanitize_ident(key);
                if !existing.contains(&safe_key.to_lowercase()) {
                    let alter = format!(
                        "ALTER TABLE {} ADD COLUMN {} TEXT",
                        quote_ident(&table),
                        quote_ident(&safe_key)
                    );
                    bridge::db_exec_raw(&alter, "[]");
                }
            }
        }

        // Build INSERT
        let keys: Vec<&String> = data.keys().collect();
        let col_names: Vec<String> = keys
            .iter()
            .map(|k| quote_ident(&sanitize_ident(k)))
            .collect();
        let placeholders: Vec<&str> = std::iter::repeat("?").take(keys.len()).collect();
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_ident(&table),
            col_names.join(", "),
            placeholders.join(", ")
        );
        let params: Vec<serde_json::Value> = keys.iter().map(|k| coerce_param(&data[*k])).collect();
        let params_json = params_to_json(&params);

        bridge::db_exec_raw(&sql, &params_json);

        // Persist to OPFS
        bridge::dbFlush().await;

        // Re-fetch to return the canonical record
        let id = match data.get("id") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        self.get(collection, &id).await
    }

    // ── update ────────────────────────────────────────────────────────────────

    async fn update(
        &self,
        collection: &str,
        id: &str,
        mut data: HashMap<String, serde_json::Value>,
    ) -> Result<Record, DatabaseError> {
        let table = sanitize_ident(collection);

        if !data.contains_key("updated_at") {
            data.insert(
                "updated_at".to_string(),
                serde_json::Value::String(now_rfc3339()),
            );
        }

        // Ensure columns exist for the incoming data fields
        let existing = existing_columns(&table);
        for key in data.keys() {
            let safe_key = sanitize_ident(key);
            if !existing.contains(&safe_key.to_lowercase()) {
                let alter = format!(
                    "ALTER TABLE {} ADD COLUMN {} TEXT",
                    quote_ident(&table),
                    quote_ident(&safe_key)
                );
                bridge::db_exec_raw(&alter, "[]");
            }
        }

        let set_pairs: Vec<(String, serde_json::Value)> = data.into_iter().collect();
        let set_clauses: Vec<String> = set_pairs
            .iter()
            .map(|(k, _)| format!("{} = ?", quote_ident(&sanitize_ident(k))))
            .collect();
        let sql = format!(
            "UPDATE {} SET {} WHERE id = ?",
            quote_ident(&table),
            set_clauses.join(", ")
        );

        let mut params: Vec<serde_json::Value> =
            set_pairs.iter().map(|(_, v)| coerce_param(v)).collect();
        params.push(serde_json::Value::String(id.to_string()));
        let params_json = params_to_json(&params);

        let result = bridge::db_exec_raw(&sql, &params_json);
        if parse_rows_modified(&result) == 0 {
            return Err(DatabaseError::NotFound);
        }

        // Persist to OPFS
        bridge::dbFlush().await;

        self.get(collection, id).await
    }

    // ── delete ────────────────────────────────────────────────────────────────

    async fn delete(&self, collection: &str, id: &str) -> Result<(), DatabaseError> {
        let table = sanitize_ident(collection);
        if !table_exists_sync(&table) {
            return Err(DatabaseError::NotFound);
        }

        let sql = format!("DELETE FROM {} WHERE id = ?", quote_ident(&table));
        let params = params_to_json(&[serde_json::Value::String(id.to_string())]);
        let result = bridge::db_exec_raw(&sql, &params);

        if parse_rows_modified(&result) == 0 {
            return Err(DatabaseError::NotFound);
        }

        bridge::dbFlush().await;

        Ok(())
    }

    // ── count ─────────────────────────────────────────────────────────────────

    async fn count(&self, collection: &str, filters: &[Filter]) -> Result<i64, DatabaseError> {
        let table = sanitize_ident(collection);
        if !table_exists_sync(&table) {
            return Ok(0);
        }

        let (where_clause, filter_params) = build_where(filters);
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM {} {}",
            quote_ident(&table),
            where_clause
        );
        let params_json = params_to_json(&filter_params);
        let json = bridge::db_query_raw(&sql, &params_json);
        let rows: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap_or_default();
        Ok(rows
            .first()
            .and_then(|r| r.get("cnt"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0))
    }

    // ── sum ───────────────────────────────────────────────────────────────────

    async fn sum(
        &self,
        collection: &str,
        field: &str,
        filters: &[Filter],
    ) -> Result<f64, DatabaseError> {
        let table = sanitize_ident(collection);
        if !table_exists_sync(&table) {
            return Ok(0.0);
        }

        let safe_field = sanitize_ident(field);
        let (where_clause, filter_params) = build_where(filters);
        let sql = format!(
            "SELECT COALESCE(SUM({}), 0) as total FROM {} {}",
            quote_ident(&safe_field),
            quote_ident(&table),
            where_clause
        );
        let params_json = params_to_json(&filter_params);
        let json = bridge::db_query_raw(&sql, &params_json);
        let rows: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap_or_default();
        Ok(rows
            .first()
            .and_then(|r| r.get("total"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0))
    }

    // ── query_raw ─────────────────────────────────────────────────────────────

    async fn query_raw(
        &self,
        query: &str,
        args: &[serde_json::Value],
    ) -> Result<Vec<Record>, DatabaseError> {
        let coerced: Vec<serde_json::Value> = args.iter().map(coerce_param).collect();
        let params_json = params_to_json(&coerced);
        let json = bridge::db_query_raw(query, &params_json);
        parse_rows(&json)
    }

    // ── exec_raw ──────────────────────────────────────────────────────────────

    async fn exec_raw(
        &self,
        query: &str,
        args: &[serde_json::Value],
    ) -> Result<i64, DatabaseError> {
        let coerced: Vec<serde_json::Value> = args.iter().map(coerce_param).collect();
        let params_json = params_to_json(&coerced);
        let result = bridge::db_exec_raw(query, &params_json);

        bridge::dbFlush().await;

        Ok(parse_rows_modified(&result))
    }

    // ── delete_where ──────────────────────────────────────────────────────────

    async fn delete_where(
        &self,
        collection: &str,
        filters: &[Filter],
    ) -> Result<(), DatabaseError> {
        let table = sanitize_ident(collection);
        if !table_exists_sync(&table) {
            return Ok(());
        }

        let (where_clause, filter_params) = build_where(filters);
        let sql = format!("DELETE FROM {} {}", quote_ident(&table), where_clause);
        let params_json = params_to_json(&filter_params);
        bridge::db_exec_raw(&sql, &params_json);

        bridge::dbFlush().await;

        Ok(())
    }

    // ── update_where ──────────────────────────────────────────────────────────

    async fn update_where(
        &self,
        collection: &str,
        filters: &[Filter],
        mut data: HashMap<String, serde_json::Value>,
    ) -> Result<(), DatabaseError> {
        let table = sanitize_ident(collection);
        if !table_exists_sync(&table) {
            return Ok(());
        }

        if !data.contains_key("updated_at") {
            data.insert(
                "updated_at".to_string(),
                serde_json::Value::String(now_rfc3339()),
            );
        }

        let (where_clause, filter_params) = build_where(filters);
        let set_pairs: Vec<(String, serde_json::Value)> = data.into_iter().collect();
        let set_clauses: Vec<String> = set_pairs
            .iter()
            .map(|(k, _)| format!("{} = ?", quote_ident(&sanitize_ident(k))))
            .collect();
        let sql = format!(
            "UPDATE {} SET {} {}",
            quote_ident(&table),
            set_clauses.join(", "),
            where_clause
        );

        let mut params: Vec<serde_json::Value> =
            set_pairs.iter().map(|(_, v)| coerce_param(v)).collect();
        params.extend(filter_params);
        let params_json = params_to_json(&params);

        bridge::db_exec_raw(&sql, &params_json);

        bridge::dbFlush().await;

        Ok(())
    }

    // ── ensure_schema_table ───────────────────────────────────────────────────

    async fn ensure_schema_table(&self, table: &Table) -> Result<(), DatabaseError> {
        // CREATE TABLE IF NOT EXISTS
        let create_sql = build_create_table_sql(table);
        bridge::db_exec_raw(&create_sql, "[]");

        // Add any missing columns
        let existing = existing_columns(&table.name);
        for col in &table.columns {
            if !existing.contains(&col.name.to_lowercase()) {
                let alter = format!(
                    "ALTER TABLE {} ADD COLUMN {}",
                    quote_ident(&table.name),
                    column_def_to_sql(col)
                );
                bridge::db_exec_raw(&alter, "[]");
            }
        }

        // Create indexes
        for idx in &table.indexes {
            let idx_sql = build_create_index_sql(&table.name, idx);
            bridge::db_exec_raw(&idx_sql, "[]");
        }

        // Create FK indexes
        for col in &table.columns {
            if col.references.is_some() {
                let tbl = sanitize_ident(&table.name);
                let c = sanitize_ident(&col.name);
                let idx_name = format!("idx_{}_{}", tbl, c);
                let idx_sql = format!(
                    "CREATE INDEX IF NOT EXISTS {} ON {}({})",
                    idx_name,
                    quote_ident(&table.name),
                    quote_ident(&col.name)
                );
                bridge::db_exec_raw(&idx_sql, "[]");
            }
        }

        bridge::dbFlush().await;

        Ok(())
    }

    // ── schema_table_exists ───────────────────────────────────────────────────

    async fn schema_table_exists(&self, name: &str) -> Result<bool, DatabaseError> {
        Ok(table_exists_sync(name))
    }

    // ── schema_drop_table ─────────────────────────────────────────────────────

    async fn schema_drop_table(&self, name: &str) -> Result<(), DatabaseError> {
        let sql = format!("DROP TABLE IF EXISTS {}", quote_ident(name));
        bridge::db_exec_raw(&sql, "[]");

        bridge::dbFlush().await;

        Ok(())
    }

    // ── schema_add_column ─────────────────────────────────────────────────────

    async fn schema_add_column(&self, table: &str, column: &Column) -> Result<(), DatabaseError> {
        let sql = format!(
            "ALTER TABLE {} ADD COLUMN {}",
            quote_ident(table),
            column_def_to_sql(column)
        );
        bridge::db_exec_raw(&sql, "[]");

        bridge::dbFlush().await;

        Ok(())
    }
}
