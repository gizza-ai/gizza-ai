//! gizza-ai/text-to-table — render delimited text as an aligned ASCII or
//! Markdown table. Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_text_to_table_core::{to_table, Align, Format};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_align")]
    align: String,
}
fn default_format() -> String {
    "ascii".to_string()
}
fn default_align() -> String {
    "left".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The delimited text (CSV/TSV/etc.)."))
        .param(
            Param::enumv("format", ["ascii", "markdown"]).default("ascii").describe(
                "Output table format: ascii (default, aligned box-drawing grid) or markdown (padded pipe table).",
            ),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as the header (default true); otherwise 'Column N' headers are generated."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'/'space'. Default ','."),
        )
        .param(
            Param::enumv("align", ["left", "right", "center"]).default("left").describe(
                "Column text alignment: left (default), right or center.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextToTable;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-to-table",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render delimited text as an aligned ASCII or Markdown table",
    skill(
        description = "Render delimited text (CSV/TSV/custom delimiter) as an aligned table. format=ascii (default) produces a box-drawing grid (+---+ borders, padded cells); format=markdown produces a GitHub-style pipe table padded so columns line up (with alignment markers in the separator row). header=true (default) uses the first row as the header, otherwise 'Column N' headers are generated. delimiter is a single char or comma/tab/semicolon/pipe/space. align is left/right/center. Runs locally.",
        parameters = schema_json()
    ),
)]
impl TextToTable {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-to-table", |a: Args| {
            let fmt = Format::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let align = Align::parse(&a.align).map_err(SkillError::InvalidArgs)?;
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            to_table(&a.data, fmt, a.header, &delim, align).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The delimited text (CSV/TSV/etc.)." },
                    "format": { "type": "string", "enum": ["ascii", "markdown"], "default": "ascii", "description": "Output table format: ascii (default, aligned box-drawing grid) or markdown (padded pipe table)." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as the header (default true); otherwise 'Column N' headers are generated." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'/'space'. Default ','." },
                    "align": { "type": "string", "enum": ["left", "right", "center"], "default": "left", "description": "Column text alignment: left (default), right or center." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
