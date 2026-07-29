//! gizza-ai/prisma-schema-from-sql — chat skill block on the shared tool
//! abstraction. Turns SQL `CREATE TABLE` / `ALTER TABLE` DDL into a Prisma
//! `schema.prisma` (models, fields, attributes, relations). The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill`. No host calls — the DDL is parsed and
//! mapped entirely inside the WASM sandbox; nothing is executed.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    provider: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_true")]
    relations: bool,
    #[serde(default = "default_true")]
    native_types: bool,
    #[serde(default)]
    map_names: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The SQL DDL to convert. Include one or more CREATE TABLE (and optionally ALTER TABLE / CREATE INDEX) statements — e.g. \"CREATE TABLE users (id SERIAL PRIMARY KEY, email VARCHAR(255) NOT NULL UNIQUE);\". Comments (-- , #, /* */), INSERT/SELECT and other non-DDL statements are ignored. Nothing is executed."),
        )
        .param(
            Param::enumv("provider", ["postgresql", "mysql", "sqlite", "sqlserver"])
                .default("postgresql")
                .describe("Target Prisma datasource provider. Controls the emitted datasource `provider`, how the SQL is parsed, and which native-type attributes are valid — e.g. MySQL maps TINYINT(1) to Boolean, and SQLite emits no @db.* native types. Default postgresql."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit the `generator client` and `datasource db` header blocks at the top of the schema. When false, only the `model` blocks are emitted (handy for pasting into an existing schema.prisma). Default true."),
        )
        .param(
            Param::boolean("relations")
                .default(true)
                .describe("Infer Prisma `@relation` fields from foreign keys — including `fields`/`references`, `onDelete`/`onUpdate` referential actions, and disambiguating relation names when two FKs point at the same model. When false, foreign-key columns stay as plain scalar fields. Default true."),
        )
        .param(
            Param::boolean("native_types")
                .default(true)
                .describe("Emit native database-type attributes where they carry information — `@db.VarChar(n)`, `@db.Char(n)`, `@db.Decimal(p,s)`. Ignored for SQLite (which has no native types in Prisma). When false, only the base Prisma scalar type is emitted. Default true."),
        )
        .param(
            Param::boolean("map_names")
                .default(false)
                .describe("Rewrite snake_case SQL names to Prisma conventions — PascalCase (singularized) model names and camelCase field names — keeping the original database names via `@@map`/`@map`. When false, table and column names are used verbatim (only sanitized to valid identifiers). Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/prisma-schema-from-sql",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert SQL CREATE TABLE / ALTER TABLE DDL into a Prisma schema (models, fields, attributes, relations).",
    skill(
        description = "Convert SQL CREATE TABLE / ALTER TABLE DDL into a Prisma schema.prisma. For each table it emits a `model` block with fields mapped to Prisma scalar types (Int, BigInt, Boolean, Decimal, Float, DateTime, Json, Bytes, String), carrying `@id`, `@unique`, `@default(...)` (autoincrement(), now(), uuid(), literals, and dbgenerated() for anything else), and — when enabled — native-type attributes (@db.VarChar(n)/@db.Char(n)/@db.Decimal(p,s)). Primary keys (including composite @@id), unique constraints (@@unique), and indexes (@@index) become block-level attributes. Foreign keys are inferred into `@relation` fields with fields/references and onDelete/onUpdate actions. Optionally rewrites names to Prisma conventions (PascalCase models, camelCase fields) while preserving the DB names with @@map/@map. Supports PostgreSQL, MySQL, SQLite and SQL Server. It is a lenient parser, not a SQL engine: comments and non-DDL statements are skipped and nothing is executed.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "prisma-schema-from-sql", |a: Args| {
            gizza_ai_prisma_schema_from_sql_core::convert(
                &a.input,
                &a.provider,
                a.header,
                a.relations,
                a.native_types,
                a.map_names,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-07-29 for the initial prisma-schema-from-sql release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The SQL DDL to convert. Include one or more CREATE TABLE (and optionally ALTER TABLE / CREATE INDEX) statements — e.g. \"CREATE TABLE users (id SERIAL PRIMARY KEY, email VARCHAR(255) NOT NULL UNIQUE);\". Comments (-- , #, /* */), INSERT/SELECT and other non-DDL statements are ignored. Nothing is executed." },
                    "provider": { "type": "string", "enum": ["postgresql", "mysql", "sqlite", "sqlserver"], "default": "postgresql", "description": "Target Prisma datasource provider. Controls the emitted datasource `provider`, how the SQL is parsed, and which native-type attributes are valid — e.g. MySQL maps TINYINT(1) to Boolean, and SQLite emits no @db.* native types. Default postgresql." },
                    "header": { "type": "boolean", "default": true, "description": "Emit the `generator client` and `datasource db` header blocks at the top of the schema. When false, only the `model` blocks are emitted (handy for pasting into an existing schema.prisma). Default true." },
                    "relations": { "type": "boolean", "default": true, "description": "Infer Prisma `@relation` fields from foreign keys — including `fields`/`references`, `onDelete`/`onUpdate` referential actions, and disambiguating relation names when two FKs point at the same model. When false, foreign-key columns stay as plain scalar fields. Default true." },
                    "native_types": { "type": "boolean", "default": true, "description": "Emit native database-type attributes where they carry information — `@db.VarChar(n)`, `@db.Char(n)`, `@db.Decimal(p,s)`. Ignored for SQLite (which has no native types in Prisma). When false, only the base Prisma scalar type is emitted. Default true." },
                    "map_names": { "type": "boolean", "default": false, "description": "Rewrite snake_case SQL names to Prisma conventions — PascalCase (singularized) model names and camelCase field names — keeping the original database names via `@@map`/`@map`. When false, table and column names are used verbatim (only sanitized to valid identifiers). Default false." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
