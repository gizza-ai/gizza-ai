//! gizza-ai/mongodb-query-to-sql — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    query: String,
    #[serde(default = "default_where")]
    output: String,
    #[serde(default = "default_ansi")]
    dialect: String,
    #[serde(default)]
    table: String,
    #[serde(default = "default_column")]
    nested: String,
    #[serde(default = "default_true")]
    quote_identifiers: bool,
    #[serde(default)]
    rename_id: bool,
}

fn default_where() -> String {
    "where".to_string()
}
fn default_ansi() -> String {
    "ansi".to_string()
}
fn default_column() -> String {
    "column".to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("query").required().describe("The MongoDB query to translate. Either a bare filter document ({ age: { $gte: 21 } }) or a full shell call (db.users.find({...}, {...}).sort({...}).limit(10).skip(20)), including findOne, count, and countDocuments. Relaxed shell syntax is accepted: unquoted keys, single quotes, trailing commas, // and /* */ comments, /regex/i literals, ObjectId()/ISODate()/new Date()/NumberLong() helpers, and MongoDB Extended JSON. Max 100,000 characters."))
        .param(Param::enumv("output", ["where", "condition", "select"]).default("where").describe("What SQL to emit: 'where' (default) is the condition prefixed with WHERE; 'condition' is the bare boolean expression for pasting into a larger statement; 'select' is a full SELECT ... FROM ... WHERE ... ORDER BY ... statement built from the projection, sort, limit, and skip in the query."))
        .param(Param::enumv("dialect", ["ansi", "postgres", "mysql", "sqlserver"]).default("ansi").describe("SQL dialect for identifier quoting, regex, JSON extraction, and paging: 'ansi' (default, \"double quotes\"), 'postgres' (~*, ->>, jsonb_array_length), 'mysql' (`backticks`, REGEXP_LIKE, JSON_EXTRACT), or 'sqlserver' ([brackets], TOP (n), OFFSET ... ROWS FETCH NEXT ... ROWS ONLY)."))
        .param(Param::string("table").default("").describe("Table name used by the 'select' output, e.g. orders or sales.orders (dotted names are quoted per part). Leave empty (default) to use the collection from the pasted db.<collection>.find(...) call; a bare filter document with 'select' output needs this filled in."))
        .param(Param::enumv("nested", ["column", "json"]).default("column").describe("How dotted paths like 'address.city' are translated: 'column' (default) treats the whole path as one column name; 'json' emits a JSON extraction for the chosen dialect (->>/JSON_UNQUOTE(JSON_EXTRACT(..))/JSON_VALUE(..)), with Postgres casts so numeric and boolean comparisons stay type-correct."))
        .param(Param::boolean("quote_identifiers").default(true).describe("Wrap column and table names in the dialect's quoting characters (default true). Turn it off for bare identifiers like age >= 21 when your schema needs no quoting."))
        .param(Param::boolean("rename_id").default(false).describe("Rewrite MongoDB's _id field as id in the generated SQL (default false). Useful when the relational schema renamed the primary key during migration."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mongodb-query-to-sql",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a MongoDB find filter or shell query into a SQL WHERE clause or SELECT statement",
    skill(
        description = "Translate a MongoDB query into SQL. Accepts a bare filter document or a full db.<collection>.find(...).sort(...).limit(...).skip(...) shell call in relaxed syntax, and emits a bare boolean condition, a WHERE clause, or a complete SELECT statement. Supports $eq $ne $gt $gte $lt $lte $in $nin $and $or $nor $not $exists $regex $mod $size, projections, sort/limit/skip/count, Extended JSON and shell helpers (ObjectId, ISODate, /re/i), and dotted paths as either column names or JSON extractions. Options: output where|condition|select, dialect ansi|postgres|mysql|sqlserver, table (for select output), nested column|json, quote_identifiers (default true), rename_id (default false). Aggregation pipelines and writes are rejected with an explanation. Max 100,000 characters.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mongodb-query-to-sql", |a: Args| {
            gizza_ai_mongodb_query_to_sql_core::run(
                &a.query,
                &a.output,
                &a.dialect,
                &a.table,
                &a.nested,
                a.quote_identifiers,
                a.rename_id,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["properties"]["query"]["type"], "string");
        assert_eq!(v["required"], serde_json::json!(["query"]));
        assert_eq!(
            v["properties"]["output"]["enum"],
            serde_json::json!(["where", "condition", "select"])
        );
        assert_eq!(v["properties"]["output"]["default"], "where");
        assert_eq!(
            v["properties"]["dialect"]["enum"],
            serde_json::json!(["ansi", "postgres", "mysql", "sqlserver"])
        );
        assert_eq!(v["properties"]["dialect"]["default"], "ansi");
        assert_eq!(v["properties"]["table"]["type"], "string");
        assert_eq!(v["properties"]["table"]["default"], "");
        assert_eq!(
            v["properties"]["nested"]["enum"],
            serde_json::json!(["column", "json"])
        );
        assert_eq!(v["properties"]["nested"]["default"], "column");
        assert_eq!(v["properties"]["quote_identifiers"]["type"], "boolean");
        assert_eq!(v["properties"]["quote_identifiers"]["default"], true);
        assert_eq!(v["properties"]["rename_id"]["type"], "boolean");
        assert_eq!(v["properties"]["rename_id"]["default"], false);
        assert_eq!(v["additionalProperties"], false);
        for p in [
            "query",
            "output",
            "dialect",
            "table",
            "nested",
            "quote_identifiers",
            "rename_id",
        ] {
            assert!(
                v["properties"][p]["description"]
                    .as_str()
                    .is_some_and(|d| d.len() > 30),
                "{p} needs a real description"
            );
        }
    }

    #[test]
    fn args_defaults_match_the_schema_defaults() {
        let a: Args = serde_json::from_str(r#"{"query":"{ a: 1 }"}"#).unwrap();
        assert_eq!(a.output, "where");
        assert_eq!(a.dialect, "ansi");
        assert_eq!(a.table, "");
        assert_eq!(a.nested, "column");
        assert!(a.quote_identifiers);
        assert!(!a.rename_id);
        assert_eq!(
            gizza_ai_mongodb_query_to_sql_core::run(
                &a.query,
                &a.output,
                &a.dialect,
                &a.table,
                &a.nested,
                a.quote_identifiers,
                a.rename_id,
            )
            .unwrap(),
            "WHERE \"a\" = 1"
        );
    }
}
