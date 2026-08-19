//! Browser-facing wasm-bindgen wrapper for /tools/er-diagram-from-sql/.
//! Compiled with wasm-pack for the standalone page.
//!
//! The standalone tool page passes every field value as a string, so the
//! boolean params arrive as strings and are parsed here (positive-truthy →
//! true); the core owns all validation.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Render SQL DDL as a Mermaid `erDiagram`.
///
/// - `sql`: the SQL / DDL text (CREATE TABLE / ALTER TABLE / CREATE INDEX).
/// - `dialect`: `auto` | `mysql` | `postgres` | `sqlite` | `mssql` | `generic`.
/// - `attributes`: `all` | `keys` | `none`.
/// - `relationship_label`: `column` | `constraint` | `none`.
/// - `direction`: `auto` | `LR` | `RL` | `TB` | `BT`.
/// - `key_markers` / `mark_nullable` / `infer_relations` / `fence`:
///   `"true"`/`"1"`/`"yes"`/`"on"` → on; anything else → off.
///
/// Throws a JS error string on invalid input or an invalid option.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    sql: &str,
    dialect: &str,
    attributes: &str,
    key_markers: &str,
    mark_nullable: &str,
    infer_relations: &str,
    relationship_label: &str,
    direction: &str,
    fence: &str,
) -> Result<String, JsValue> {
    gizza_ai_er_diagram_from_sql_core::generate(
        sql,
        dialect,
        attributes,
        truthy(key_markers),
        truthy(mark_nullable),
        truthy(infer_relations),
        relationship_label,
        direction,
        truthy(fence),
    )
    .map_err(|e| JsValue::from_str(&e))
}
