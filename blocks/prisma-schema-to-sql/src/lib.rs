//! gizza-ai/prisma-schema-to-sql — chat skill block on the shared tool
//! abstraction. Turns a Prisma `schema.prisma` (models, enums, attributes,
//! relations) into `CREATE TABLE` DDL for PostgreSQL, MySQL, SQLite or SQL
//! Server. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI); `handle()` delegates to `block_utils::run_skill`. No host
//! calls — the schema is parsed and mapped entirely inside the WASM sandbox and
//! no SQL is ever executed.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    dialect: String,
    #[serde(default = "default_true")]
    foreign_keys: bool,
    #[serde(default = "default_true")]
    indexes: bool,
    #[serde(default)]
    if_not_exists: bool,
    #[serde(default)]
    drop_if_exists: bool,
    #[serde(default = "default_true")]
    quote_identifiers: bool,
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
                .describe("The Prisma schema to convert. Paste the contents of schema.prisma — one or more `model` blocks (one field per line, as Prisma writes them), plus any `enum`, `datasource` and `generator` blocks, e.g. \"model User {\\n  id Int @id @default(autoincrement())\\n  email String @unique\\n}\". Max 200000 bytes. Nothing is executed."),
        )
        .param(
            Param::enumv("dialect", ["auto", "postgresql", "mysql", "sqlite", "sqlserver"])
                .default("auto")
                .describe("Target SQL dialect for the generated DDL. `auto` reads the schema's `datasource` provider and falls back to postgresql when there is none. The dialect picks the column types (String → TEXT on postgresql, VARCHAR(191) on mysql, NVARCHAR(1000) on sqlserver), the auto-increment syntax (SERIAL / AUTO_INCREMENT / AUTOINCREMENT / IDENTITY(1,1)), the identifier quoting and how enums are emitted. Default auto."),
        )
        .param(
            Param::boolean("foreign_keys")
                .default(true)
                .describe("Emit `ALTER TABLE … ADD CONSTRAINT … FOREIGN KEY` statements for every `@relation(fields: […], references: […])`, including ON DELETE / ON UPDATE from onDelete/onUpdate (defaults follow Prisma: RESTRICT for required relations, SET NULL for optional ones, CASCADE on update). When false, only the plain scalar columns are emitted. Default true."),
        )
        .param(
            Param::boolean("indexes")
                .default(true)
                .describe("Emit `CREATE UNIQUE INDEX` for `@unique`/`@@unique` and `CREATE INDEX` for `@@index`, named the way Prisma names them (Table_column_key / Table_column_idx, or the `map:` name when given). When false, no index statements are produced. Default true."),
        )
        .param(
            Param::boolean("if_not_exists")
                .default(false)
                .describe("Make the script re-runnable: `CREATE TABLE IF NOT EXISTS` on postgresql/mysql/sqlite, an `IF OBJECT_ID(…) IS NULL` guard on sqlserver, and `CREATE INDEX IF NOT EXISTS` where the dialect supports it (postgresql, sqlite). Default false."),
        )
        .param(
            Param::boolean("drop_if_exists")
                .default(false)
                .describe("Prepend `DROP TABLE IF EXISTS` statements (reverse creation order, plus `DROP TYPE IF EXISTS` for postgresql enums) so the script can rebuild a scratch database from zero. Destructive — only for throwaway databases. Default false."),
        )
        .param(
            Param::boolean("quote_identifiers")
                .default(true)
                .describe("Quote table and column names with the dialect's delimiter (\"x\" on postgresql/sqlite, `x` on mysql, [x] on sqlserver), which preserves camelCase and reserved words exactly. When false, bare unquoted identifiers are emitted. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/prisma-schema-to-sql",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Prisma schema into CREATE TABLE DDL for PostgreSQL, MySQL, SQLite or SQL Server.",
    skill(
        description = "Convert a Prisma schema.prisma into SQL CREATE TABLE DDL. Each `model` becomes a CREATE TABLE with columns typed the way Prisma's own migrations type them for the chosen provider (String/Boolean/Int/BigInt/Float/Decimal/DateTime/Json/Bytes, `@db.*` native types used verbatim), NOT NULL for required fields, DEFAULT for literals, now() and dbgenerated() (client-side uuid()/cuid()/ulid() produce no database default), and auto-increment as SERIAL, AUTO_INCREMENT, AUTOINCREMENT or IDENTITY(1,1). `@id`/`@@id` become PRIMARY KEY constraints, `@unique`/`@@unique`/`@@index` become CREATE [UNIQUE] INDEX statements, `@map`/`@@map` rename columns/tables, `enum` blocks become a CREATE TYPE (postgresql), an inline ENUM(...) column (mysql) or a CHECK constraint (sqlite, sqlserver), and `@relation` foreign keys become ALTER TABLE … ADD CONSTRAINT … FOREIGN KEY with ON DELETE/ON UPDATE actions. Implicit many-to-many relations get their `_AToB` join table. Optional toggles add IF NOT EXISTS guards, DROP TABLE IF EXISTS statements, or drop identifier quoting. Supports PostgreSQL, MySQL/MariaDB, SQLite and SQL Server; `auto` reads the schema's datasource provider. It is a lenient parser, not a database: nothing is executed and no connection is made.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "prisma-schema-to-sql", |a: Args| {
            gizza_ai_prisma_schema_to_sql_core::convert(
                &a.input,
                &a.dialect,
                a.foreign_keys,
                a.indexes,
                a.if_not_exists,
                a.drop_if_exists,
                a.quote_identifiers,
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
    /// reviewed. Authored 2026-08-12 for the initial prisma-schema-to-sql release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The Prisma schema to convert. Paste the contents of schema.prisma — one or more `model` blocks (one field per line, as Prisma writes them), plus any `enum`, `datasource` and `generator` blocks, e.g. \"model User {\\n  id Int @id @default(autoincrement())\\n  email String @unique\\n}\". Max 200000 bytes. Nothing is executed." },
                    "dialect": { "type": "string", "enum": ["auto", "postgresql", "mysql", "sqlite", "sqlserver"], "default": "auto", "description": "Target SQL dialect for the generated DDL. `auto` reads the schema's `datasource` provider and falls back to postgresql when there is none. The dialect picks the column types (String → TEXT on postgresql, VARCHAR(191) on mysql, NVARCHAR(1000) on sqlserver), the auto-increment syntax (SERIAL / AUTO_INCREMENT / AUTOINCREMENT / IDENTITY(1,1)), the identifier quoting and how enums are emitted. Default auto." },
                    "foreign_keys": { "type": "boolean", "default": true, "description": "Emit `ALTER TABLE … ADD CONSTRAINT … FOREIGN KEY` statements for every `@relation(fields: […], references: […])`, including ON DELETE / ON UPDATE from onDelete/onUpdate (defaults follow Prisma: RESTRICT for required relations, SET NULL for optional ones, CASCADE on update). When false, only the plain scalar columns are emitted. Default true." },
                    "indexes": { "type": "boolean", "default": true, "description": "Emit `CREATE UNIQUE INDEX` for `@unique`/`@@unique` and `CREATE INDEX` for `@@index`, named the way Prisma names them (Table_column_key / Table_column_idx, or the `map:` name when given). When false, no index statements are produced. Default true." },
                    "if_not_exists": { "type": "boolean", "default": false, "description": "Make the script re-runnable: `CREATE TABLE IF NOT EXISTS` on postgresql/mysql/sqlite, an `IF OBJECT_ID(…) IS NULL` guard on sqlserver, and `CREATE INDEX IF NOT EXISTS` where the dialect supports it (postgresql, sqlite). Default false." },
                    "drop_if_exists": { "type": "boolean", "default": false, "description": "Prepend `DROP TABLE IF EXISTS` statements (reverse creation order, plus `DROP TYPE IF EXISTS` for postgresql enums) so the script can rebuild a scratch database from zero. Destructive — only for throwaway databases. Default false." },
                    "quote_identifiers": { "type": "boolean", "default": true, "description": "Quote table and column names with the dialect's delimiter (\"x\" on postgresql/sqlite, `x` on mysql, [x] on sqlserver), which preserves camelCase and reserved words exactly. When false, bare unquoted identifiers are emitted. Default true." }
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
