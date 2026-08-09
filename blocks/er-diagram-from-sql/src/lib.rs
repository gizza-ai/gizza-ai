//! gizza-ai/er-diagram-from-sql — chat skill block on the shared tool
//! abstraction. Turns SQL `CREATE TABLE` / `ALTER TABLE` DDL into a Mermaid
//! `erDiagram`: one entity per table, typed attributes with `PK`/`FK`/`UK`
//! markers, and one crow's-foot relationship per foreign key. The chat schema
//! is single-sourced from `descriptor()` (which also drives the CLI);
//! `handle()` delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sql: String,
    #[serde(default)]
    dialect: String,
    #[serde(default)]
    attributes: String,
    #[serde(default = "default_true")]
    key_markers: bool,
    #[serde(default)]
    mark_nullable: bool,
    #[serde(default)]
    infer_relations: bool,
    #[serde(default)]
    relationship_label: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    fence: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sql")
                .required()
                .describe("The SQL DDL to diagram. Include one or more CREATE TABLE statements (ALTER TABLE ... ADD FOREIGN KEY and CREATE UNIQUE INDEX are folded in) — e.g. \"CREATE TABLE users (id INT PRIMARY KEY); CREATE TABLE orders (id INT PRIMARY KEY, user_id INT NOT NULL REFERENCES users(id));\". Comments and non-DDL statements (INSERT, SELECT, ...) are ignored and nothing is executed. Maximum 500 tables."),
        )
        .param(
            Param::enumv("dialect", ["auto", "mysql", "postgres", "sqlite", "mssql", "generic"])
                .default("auto")
                .describe("SQL dialect hint. Identifiers are normalized for every dialect (backticks, \"double quotes\" and [brackets] are stripped); this mainly controls whether '#' starts a line comment (mysql/auto). Default auto."),
        )
        .param(
            Param::enumv("attributes", ["all", "keys", "none"])
                .default("all")
                .describe("Which columns to list inside each entity block. 'all' (default) lists every column as 'TYPE name'. 'keys' keeps only primary-key, foreign-key and unique columns, which keeps a wide schema readable. 'none' emits bare entity names plus the relationship lines only. Default all."),
        )
        .param(
            Param::boolean("key_markers")
                .default(true)
                .describe("Append Mermaid's PK / FK / UK markers to attribute lines (e.g. 'INT user_id FK'). UK is only added for single-column uniqueness, since a member of a composite UNIQUE is not unique on its own. Ignored when attributes is 'none'. Default true."),
        )
        .param(
            Param::boolean("mark_nullable")
                .default(false)
                .describe("Render nullable columns with Mermaid's optional-attribute form 'TYPE? name' so NULL-able columns are visible in the diagram. Default false."),
        )
        .param(
            Param::boolean("infer_relations")
                .default(false)
                .describe("Also draw a relationship for a '<name>_id' column that has no explicit FOREIGN KEY, when '<name>' (or its plural) matches a table in the same DDL — useful for schemas that enforce references in the application instead of the database. Never duplicates an explicit foreign key. Default false."),
        )
        .param(
            Param::enumv("relationship_label", ["column", "constraint", "none"])
                .default("column")
                .describe("What to write after the ':' on each relationship line. 'column' (default) uses the foreign-key column name(s). 'constraint' uses the FOREIGN KEY constraint name, falling back to the columns when it is unnamed. 'none' emits an empty label, which keeps a busy diagram clean. Default column."),
        )
        .param(
            Param::enumv("direction", ["auto", "LR", "RL", "TB", "BT"])
                .default("auto")
                .describe("Layout direction emitted as a 'direction' statement inside the diagram: LR (left to right), RL, TB (top to bottom, TD is accepted as an alias) or BT. 'auto' (default) omits the statement and lets the renderer choose. Default auto."),
        )
        .param(
            Param::boolean("fence")
                .default(false)
                .describe("Wrap the diagram in a ```mermaid code fence so it can be pasted straight into a Markdown file, a GitHub issue or a pull-request comment. Default false (bare diagram source)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/er-diagram-from-sql",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn SQL CREATE TABLE / ALTER TABLE DDL into a Mermaid erDiagram with crow's-foot relationships.",
    skill(
        description = "Turn SQL DDL into a Mermaid ER diagram. Paste CREATE TABLE statements (ALTER TABLE ... ADD FOREIGN KEY and CREATE UNIQUE INDEX are folded in) and it emits erDiagram source: one entity per table, its columns as 'TYPE name' attributes with PK / FK / UK markers, and one crow's-foot relationship per foreign key. Cardinality is derived from the schema — the parent side is '||' (exactly one) when every FK column is NOT NULL and '|o' (zero or one) otherwise; the child side is 'o{' (zero or more), or 'o|' when the FK columns are themselves unique in the child (a 1:1); the line is solid '--' for a NOT NULL FK and dashed '..' for a nullable one. Options: limit attributes to keys only or hide them entirely for wide schemas, toggle the PK/FK/UK markers, mark nullable columns with Mermaid's 'TYPE?' form, infer relationships from '<table>_id' columns that lack a foreign key, label relationships by column or by constraint name (or not at all), set a layout direction, and wrap the result in a ```mermaid code fence. Types and identifiers are sanitized to Mermaid-safe tokens (DECIMAL(10,2) becomes DECIMAL(10_2), TIMESTAMP WITH TIME ZONE becomes TIMESTAMP_WITH_TIME_ZONE) so the diagram actually renders. Supports MySQL, PostgreSQL, SQLite, SQL Server and generic dialects. It is a lenient text parser, not a SQL engine: comments and non-DDL statements are skipped and nothing is executed. It emits diagram source, not an image; paste it anywhere Mermaid renders. Maximum 500 tables.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "er-diagram-from-sql", |a: Args| {
            gizza_ai_er_diagram_from_sql_core::generate(
                &a.sql,
                &a.dialect,
                &a.attributes,
                a.key_markers,
                a.mark_nullable,
                a.infer_relations,
                &a.relationship_label,
                &a.direction,
                a.fence,
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
    /// reviewed. Authored 2026-08-09 for the initial er-diagram-from-sql release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "sql": { "type": "string", "description": "The SQL DDL to diagram. Include one or more CREATE TABLE statements (ALTER TABLE ... ADD FOREIGN KEY and CREATE UNIQUE INDEX are folded in) — e.g. \"CREATE TABLE users (id INT PRIMARY KEY); CREATE TABLE orders (id INT PRIMARY KEY, user_id INT NOT NULL REFERENCES users(id));\". Comments and non-DDL statements (INSERT, SELECT, ...) are ignored and nothing is executed. Maximum 500 tables." },
                    "dialect": { "type": "string", "enum": ["auto", "mysql", "postgres", "sqlite", "mssql", "generic"], "default": "auto", "description": "SQL dialect hint. Identifiers are normalized for every dialect (backticks, \"double quotes\" and [brackets] are stripped); this mainly controls whether '#' starts a line comment (mysql/auto). Default auto." },
                    "attributes": { "type": "string", "enum": ["all", "keys", "none"], "default": "all", "description": "Which columns to list inside each entity block. 'all' (default) lists every column as 'TYPE name'. 'keys' keeps only primary-key, foreign-key and unique columns, which keeps a wide schema readable. 'none' emits bare entity names plus the relationship lines only. Default all." },
                    "key_markers": { "type": "boolean", "default": true, "description": "Append Mermaid's PK / FK / UK markers to attribute lines (e.g. 'INT user_id FK'). UK is only added for single-column uniqueness, since a member of a composite UNIQUE is not unique on its own. Ignored when attributes is 'none'. Default true." },
                    "mark_nullable": { "type": "boolean", "default": false, "description": "Render nullable columns with Mermaid's optional-attribute form 'TYPE? name' so NULL-able columns are visible in the diagram. Default false." },
                    "infer_relations": { "type": "boolean", "default": false, "description": "Also draw a relationship for a '<name>_id' column that has no explicit FOREIGN KEY, when '<name>' (or its plural) matches a table in the same DDL — useful for schemas that enforce references in the application instead of the database. Never duplicates an explicit foreign key. Default false." },
                    "relationship_label": { "type": "string", "enum": ["column", "constraint", "none"], "default": "column", "description": "What to write after the ':' on each relationship line. 'column' (default) uses the foreign-key column name(s). 'constraint' uses the FOREIGN KEY constraint name, falling back to the columns when it is unnamed. 'none' emits an empty label, which keeps a busy diagram clean. Default column." },
                    "direction": { "type": "string", "enum": ["auto", "LR", "RL", "TB", "BT"], "default": "auto", "description": "Layout direction emitted as a 'direction' statement inside the diagram: LR (left to right), RL, TB (top to bottom, TD is accepted as an alias) or BT. 'auto' (default) omits the statement and lets the renderer choose. Default auto." },
                    "fence": { "type": "boolean", "default": false, "description": "Wrap the diagram in a ```mermaid code fence so it can be pasted straight into a Markdown file, a GitHub issue or a pull-request comment. Default false (bare diagram source)." }
                },
                "required": ["sql"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
