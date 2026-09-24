//! gizza-ai/dbml-to-sql — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    dbml: String,
    #[serde(default = "default_dialect")]
    dialect: String,
    #[serde(default = "default_true")]
    foreign_keys: bool,
    #[serde(default = "default_true")]
    indexes: bool,
    #[serde(default = "default_true")]
    comments: bool,
    #[serde(default)]
    if_not_exists: bool,
    #[serde(default)]
    drop_if_exists: bool,
    #[serde(default = "default_true")]
    quote_identifiers: bool,
}
fn default_dialect() -> String { "auto".to_string() }
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("dbml").required().describe("DBML schema text to compile. Include one or more Table blocks, plus optional Project database_type, Enum blocks, indexes and Ref relationships. Maximum 200000 bytes. Example: `Table users { id int [pk, increment] email varchar [unique, not null] }`."))
        .param(Param::enumv("dialect", ["auto", "postgresql", "mysql", "sqlite", "sqlserver", "oracle"]).default("auto").describe("SQL dialect to emit. auto (default) reads Project { database_type: '...' } when present, otherwise PostgreSQL. Choices: postgresql, mysql, sqlite, sqlserver, oracle."))
        .param(Param::boolean("foreign_keys").default(true).describe("Emit ALTER TABLE foreign-key statements for DBML Ref definitions and inline ref settings. Default true; false emits tables without FK constraints."))
        .param(Param::boolean("indexes").default(true).describe("Emit CREATE INDEX / UNIQUE INDEX statements for DBML indexes and single-column unique flags. Default true."))
        .param(Param::boolean("comments").default(true).describe("Emit COMMENT ON statements for table and column notes where the dialect supports them (PostgreSQL and Oracle). Default true."))
        .param(Param::boolean("if_not_exists").default(false).describe("Use CREATE TABLE IF NOT EXISTS / CREATE INDEX IF NOT EXISTS where the dialect supports it. Default false for plain DDL."))
        .param(Param::boolean("drop_if_exists").default(false).describe("Prepend DROP TABLE IF EXISTS (and PostgreSQL DROP TYPE IF EXISTS for enums) so the script can rebuild objects. Default false."))
        .param(Param::boolean("quote_identifiers").default(true).describe("Quote table, column, index and constraint identifiers with the dialect delimiter. Default true; false emits bare identifiers for simple schemas."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct DbmlToSql;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dbml-to-sql",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compile DBML database schemas into SQL DDL.",
    skill(
        description = "Compile a DBML (Database Markup Language) schema into CREATE TABLE SQL DDL. Supports Project database_type auto-detection; PostgreSQL, MySQL/MariaDB, SQLite, SQL Server and Oracle output; Table and schema-qualified table blocks; TablePartial injection; Enum blocks; column settings including pk, increment, not null, unique, default, check and note; indexes; inline and standalone Ref relationships, including many-to-many join tables; optional foreign keys, indexes, comments, IF NOT EXISTS, DROP IF EXISTS and quoted identifiers. Cross-dialect type mapping covers common DBML types such as int, varchar, text, boolean, decimal, timestamp, json, uuid and blobs while unknown native types pass through. Unsupported DBML sections are skipped rather than executed. Input is capped at 200000 bytes. Returns SQL text.",
        parameters = schema_json()
    ),
)]
impl DbmlToSql {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dbml-to-sql", |a: Args| {
            gizza_ai_dbml_to_sql_core::convert(
                &a.dbml,
                &a.dialect,
                a.foreign_keys,
                a.indexes,
                a.comments,
                a.if_not_exists,
                a.drop_if_exists,
                a.quote_identifiers,
            ).map_err(SkillError::InvalidArgs)
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
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "dbml":              { "type": "string", "description": "DBML schema text to compile. Include one or more Table blocks, plus optional Project database_type, Enum blocks, indexes and Ref relationships. Maximum 200000 bytes. Example: `Table users { id int [pk, increment] email varchar [unique, not null] }`." },
                    "dialect":           { "type": "string", "enum": ["auto", "postgresql", "mysql", "sqlite", "sqlserver", "oracle"], "default": "auto", "description": "SQL dialect to emit. auto (default) reads Project { database_type: '...' } when present, otherwise PostgreSQL. Choices: postgresql, mysql, sqlite, sqlserver, oracle." },
                    "foreign_keys":      { "type": "boolean", "default": true, "description": "Emit ALTER TABLE foreign-key statements for DBML Ref definitions and inline ref settings. Default true; false emits tables without FK constraints." },
                    "indexes":           { "type": "boolean", "default": true, "description": "Emit CREATE INDEX / UNIQUE INDEX statements for DBML indexes and single-column unique flags. Default true." },
                    "comments":          { "type": "boolean", "default": true, "description": "Emit COMMENT ON statements for table and column notes where the dialect supports them (PostgreSQL and Oracle). Default true." },
                    "if_not_exists":     { "type": "boolean", "default": false, "description": "Use CREATE TABLE IF NOT EXISTS / CREATE INDEX IF NOT EXISTS where the dialect supports it. Default false for plain DDL." },
                    "drop_if_exists":    { "type": "boolean", "default": false, "description": "Prepend DROP TABLE IF EXISTS (and PostgreSQL DROP TYPE IF EXISTS for enums) so the script can rebuild objects. Default false." },
                    "quote_identifiers": { "type": "boolean", "default": true, "description": "Quote table, column, index and constraint identifiers with the dialect delimiter. Default true; false emits bare identifiers for simple schemas." }
                },
                "required": ["dbml"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Every param the page/CLI advertises must carry an LLM-actionable
    /// `.describe()`, and the fixed-choice `dialect` must stay an enum.
    #[test]
    fn every_param_is_described_and_dialect_is_an_enum() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().expect("object schema");
        assert_eq!(props.len(), 8, "param count changed — update the drift guard");
        for (name, p) in props {
            let d = p["description"].as_str().unwrap_or("");
            assert!(d.len() > 40, "param `{name}` needs a fuller .describe()");
        }
        assert_eq!(
            props["dialect"]["enum"],
            serde_json::json!(["auto", "postgresql", "mysql", "sqlite", "sqlserver", "oracle"])
        );
    }
}
