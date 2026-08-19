//! gizza-ai/category-canonicalize — apply a supplied variant→canonical mapping to
//! categorical column(s). Thin wrapper around the core; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → runs
//! on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_category_canonicalize_core::canonicalize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    mapping: String,
    #[serde(default)]
    column: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    header: bool,
    #[serde(default = "default_true")]
    ignore_case: bool,
    #[serde(default = "default_true")]
    ignore_spacing: bool,
    #[serde(default)]
    unmatched: String,
    #[serde(default = "default_threshold")]
    fuzzy_threshold: f64,
    #[serde(default)]
    output: String,
}
fn default_true() -> bool {
    true
}
fn default_threshold() -> f64 {
    85.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The table to clean: CSV/TSV text, or a plain list with one value per line."))
        .param(Param::string("mapping").required().describe("The vocabulary, one rule per line: 'variant => canonical'. Separate with '=>', '->', '=', a tab, a comma, or a semicolon; share one canonical between several variants with '|' (e.g. 'USA|U.S.A.|us => United States'). A line with no separator declares a canonical value that has no variants. '#' comments a line out."))
        .param(Param::string("column").default("").describe("Which column(s) to canonicalize: header name(s) (needs header=true) or 1-based index(es), comma-separated. Blank uses the only column (e.g. a newline list)."))
        .param(Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"]).default("auto").describe("Field separator: auto-detect from the first line, comma, tab, semicolon, or pipe. Default auto."))
        .param(Param::boolean("header").default(false).describe("Treat the first row as a header — it is never rewritten, and lets you select columns by name. Default false."))
        .param(Param::boolean("ignore_case").default(true).describe("Ignore letter case when matching a value against the mapping (so 'usa' matches the variant 'USA'). Default true."))
        .param(Param::boolean("ignore_spacing").default(true).describe("Collapse and trim whitespace when matching (so 'New  York ' matches 'New York'). Default true."))
        .param(Param::enumv("unmatched", ["keep", "fuzzy", "blank", "error"]).default("keep").describe("What to do with a value the mapping doesn't cover: keep it as-is (default), fuzzy (replace with the closest canonical when it scores at or above fuzzy_threshold), blank (empty the cell), or error (fail and list the offending values)."))
        .param(Param::integer("fuzzy_threshold").default(85).min(0.0).max(100.0).describe("Similarity 0–100 (edit-distance ratio) a suggestion needs before unmatched=fuzzy applies it. Higher is stricter. Also shown next to every suggestion in the other outputs. Default 85."))
        .param(Param::enumv("output", ["csv", "markdown", "json", "suggestions"]).default("csv").describe("Result format: csv (the rewritten table), markdown (table plus a what-changed report), json (stats, changes and the table), or suggestions (a review CSV of the values still not covered, with the closest canonical)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CategoryCanonicalize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/category-canonicalize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rewrite category variants to canonical values from a supplied mapping",
    skill(
        description = "Canonicalize categorical column(s) against a mapping you supply: every spelling, case, and spacing variant listed is rewritten to its canonical value. Feed `data` as CSV/TSV (pick `column` by header name or 1-based index, comma-separated for several) or a plain newline list (leave `column` blank). `mapping` is one rule per line, 'variant => canonical', with '|' sharing a canonical between variants and a bare line declaring a canonical that has no variants. `ignore_case`/`ignore_spacing` fold those differences before matching. Values the mapping doesn't cover follow `unmatched`: keep, fuzzy (apply the closest canonical at or above `fuzzy_threshold`, a 0–100 edit-distance ratio), blank, or error. `output` is csv (the rewritten table), markdown (table + report), json, or suggestions (a review CSV of what is still uncovered and its closest canonical — paste the rows you accept back into the mapping and re-run). Runs locally.",
        parameters = schema_json()
    )
)]
impl CategoryCanonicalize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "category-canonicalize", |a: Args| {
            let delim = if a.delimiter.is_empty() {
                "auto".to_string()
            } else {
                a.delimiter
            };
            canonicalize(
                &a.data,
                &a.mapping,
                &a.column,
                &delim,
                a.header,
                a.ignore_case,
                a.ignore_spacing,
                &a.unmatched,
                a.fuzzy_threshold,
                &a.output,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":            { "type": "string", "description": "The table to clean: CSV/TSV text, or a plain list with one value per line." },
                    "mapping":         { "type": "string", "description": "The vocabulary, one rule per line: 'variant => canonical'. Separate with '=>', '->', '=', a tab, a comma, or a semicolon; share one canonical between several variants with '|' (e.g. 'USA|U.S.A.|us => United States'). A line with no separator declares a canonical value that has no variants. '#' comments a line out." },
                    "column":          { "type": "string", "default": "", "description": "Which column(s) to canonicalize: header name(s) (needs header=true) or 1-based index(es), comma-separated. Blank uses the only column (e.g. a newline list)." },
                    "delimiter":       { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe"], "default": "auto", "description": "Field separator: auto-detect from the first line, comma, tab, semicolon, or pipe. Default auto." },
                    "header":          { "type": "boolean", "default": false, "description": "Treat the first row as a header — it is never rewritten, and lets you select columns by name. Default false." },
                    "ignore_case":     { "type": "boolean", "default": true, "description": "Ignore letter case when matching a value against the mapping (so 'usa' matches the variant 'USA'). Default true." },
                    "ignore_spacing":  { "type": "boolean", "default": true, "description": "Collapse and trim whitespace when matching (so 'New  York ' matches 'New York'). Default true." },
                    "unmatched":       { "type": "string", "enum": ["keep", "fuzzy", "blank", "error"], "default": "keep", "description": "What to do with a value the mapping doesn't cover: keep it as-is (default), fuzzy (replace with the closest canonical when it scores at or above fuzzy_threshold), blank (empty the cell), or error (fail and list the offending values)." },
                    "fuzzy_threshold": { "type": "integer", "minimum": 0, "maximum": 100, "default": 85, "description": "Similarity 0–100 (edit-distance ratio) a suggestion needs before unmatched=fuzzy applies it. Higher is stricter. Also shown next to every suggestion in the other outputs. Default 85." },
                    "output":          { "type": "string", "enum": ["csv", "markdown", "json", "suggestions"], "default": "csv", "description": "Result format: csv (the rewritten table), markdown (table plus a what-changed report), json (stats, changes and the table), or suggestions (a review CSV of the values still not covered, with the closest canonical)." }
                },
                "required": ["data", "mapping"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
