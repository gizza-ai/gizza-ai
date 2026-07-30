//! gizza-ai/json-to-html-table — render a JSON array or object as a clean HTML
//! or Markdown table. Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_to_html_table_core::{to_table, Format, Nested, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_nested")]
    nested: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    null_text: String,
    #[serde(default)]
    caption: String,
    #[serde(default)]
    table_class: String,
    #[serde(default = "default_true")]
    pretty: bool,
}
fn default_format() -> String {
    "html".to_string()
}
fn default_nested() -> String {
    "json".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON to tabulate: an array of objects, an array of arrays, an array of scalars, or a single object (rendered as a key/value table)."),
        )
        .param(
            Param::enumv("format", ["html", "markdown"])
                .default("html")
                .describe("Output table format: html (default) for a <table>, or markdown for a GitHub-style pipe table."),
        )
        .param(
            Param::enumv("nested", ["json", "table", "flatten"])
                .default("json")
                .describe("How to render nested objects/arrays: 'json' (compact JSON string in the cell, default), 'table' (a nested <table>; HTML only, Markdown falls back to JSON), or 'flatten' (hoist nested objects into dotted-key columns like user.id)."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("For an array of arrays / scalars, treat the first row as the header (default true); otherwise 'Column N' headers are generated. Arrays of objects always use the object keys; a single object always uses key/value."),
        )
        .param(
            Param::string("null_text")
                .default("")
                .describe("Text rendered for a JSON null or a missing column value (default empty). Set e.g. to 'null' or '—' to mark gaps."),
        )
        .param(
            Param::string("caption")
                .default("")
                .describe("Optional <caption> text placed above the HTML table (HTML output only; ignored for Markdown). Default empty."),
        )
        .param(
            Param::string("table_class")
                .default("")
                .describe("CSS class(es) added to the top-level <table> tag, e.g. 'table table-striped' (HTML output only). Default empty."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print the HTML with indentation and line breaks (default true); set false for single-line, minified HTML (HTML output only)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Result<Options, String> {
    Ok(Options {
        format: Format::parse(&a.format)?,
        header: a.header,
        null_text: a.null_text.clone(),
        nested: Nested::parse(&a.nested)?,
        caption: a.caption.clone(),
        table_class: a.table_class.clone(),
        pretty: a.pretty,
    })
}

#[cfg(target_arch = "wasm32")]
struct JsonToHtmlTable;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-html-table",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a JSON array or object as a clean HTML or Markdown table",
    skill(
        description = "Render a JSON array or object as a clean HTML (default) or Markdown table. An array of objects becomes rows with the union of keys as columns; an array of arrays becomes rows (header=true uses the first row as the header, else Column N); an array of scalars becomes a single column; a single object becomes a two-column key/value table. nested controls nested objects/arrays: json (compact JSON string, default), table (nested <table>, HTML only), or flatten (dotted-key columns like user.id). null_text sets the text for JSON null / missing cells; caption adds an HTML <caption>; table_class adds CSS class(es) to the <table>; pretty=false emits minified single-line HTML. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonToHtmlTable {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-html-table", |a: Args| {
            let opt = build_options(&a).map_err(SkillError::InvalidArgs)?;
            to_table(&a.json, &opt).map_err(SkillError::InvalidArgs)
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
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The JSON to tabulate: an array of objects, an array of arrays, an array of scalars, or a single object (rendered as a key/value table)." },
                    "format": { "type": "string", "enum": ["html", "markdown"], "default": "html", "description": "Output table format: html (default) for a <table>, or markdown for a GitHub-style pipe table." },
                    "nested": { "type": "string", "enum": ["json", "table", "flatten"], "default": "json", "description": "How to render nested objects/arrays: 'json' (compact JSON string in the cell, default), 'table' (a nested <table>; HTML only, Markdown falls back to JSON), or 'flatten' (hoist nested objects into dotted-key columns like user.id)." },
                    "header": { "type": "boolean", "default": true, "description": "For an array of arrays / scalars, treat the first row as the header (default true); otherwise 'Column N' headers are generated. Arrays of objects always use the object keys; a single object always uses key/value." },
                    "null_text": { "type": "string", "default": "", "description": "Text rendered for a JSON null or a missing column value (default empty). Set e.g. to 'null' or '—' to mark gaps." },
                    "caption": { "type": "string", "default": "", "description": "Optional <caption> text placed above the HTML table (HTML output only; ignored for Markdown). Default empty." },
                    "table_class": { "type": "string", "default": "", "description": "CSS class(es) added to the top-level <table> tag, e.g. 'table table-striped' (HTML output only). Default empty." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print the HTML with indentation and line breaks (default true); set false for single-line, minified HTML (HTML output only)." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
