//! gizza-ai/table-heatmap — chat skill block on the shared tool abstraction.
//! Applies spreadsheet-style color-scale conditional formatting to a CSV/table
//! and returns a styled HTML <table> with shaded numeric cells. Pure → all backends.
//! Chat schema single-sourced from descriptor(); handle() delegates to run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_table_heatmap_core::{heatmap, Bounds, Scale};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_scale")]
    scale: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_true")]
    per_column: bool,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    midpoint: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    delimiter: String,
}
fn default_scale() -> String {
    "red-yellow-green".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV/table text (rows separated by newlines)."))
        .param(
            Param::enumv("scale", ["red-yellow-green", "green-yellow-red", "blue-white-red", "green", "blue"])
                .default("red-yellow-green")
                .describe("Color scale for numeric cells: red-yellow-green (low=red→high=green, default), green-yellow-red, blue-white-red (diverging), green (white→green), or blue (white→blue)."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header (not shaded). Default true."),
        )
        .param(
            Param::boolean("per_column")
                .default(true)
                .describe("Scale each numeric column against its own min/max (default true); set false to use one global min/max across the whole table."),
        )
        .param(
            Param::number("min")
                .describe("Optional fixed value for the low end of the scale (defaults to the data minimum). When both min and max are set, per_column is ignored."),
        )
        .param(
            Param::number("midpoint")
                .describe("Optional fixed midpoint value for diverging scales (red-yellow-green / green-yellow-red / blue-white-red), mapped to the neutral middle color. Defaults to halfway between min and max. Ignored for sequential scales (green/blue)."),
        )
        .param(
            Param::number("max")
                .describe("Optional fixed value for the high end of the scale (defaults to the data maximum)."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/table-heatmap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Color-scale conditional formatting for a CSV/table as styled HTML",
    skill(
        description = "Apply spreadsheet-style color-scale conditional formatting to a CSV/table and return a styled HTML <table> with shaded numeric cells. Each cell that parses as a number (tolerating commas, $/€/£, %, and (parentheses) negatives) is shaded by its value on the chosen color scale; non-numeric and header cells are left plain. scale is red-yellow-green (default), green-yellow-red, blue-white-red, green, or blue. header=true (default) keeps the first row as an unshaded header. per_column=true (default) scales each column independently; false uses one global min/max. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "table-heatmap", |a: Args| {
            let scale = Scale::parse(&a.scale).map_err(SkillError::InvalidArgs)?;
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            let bounds = Bounds { min: a.min, midpoint: a.midpoint, max: a.max };
            heatmap(&a.data, scale, a.header, a.per_column, bounds, &delim).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The CSV/table text (rows separated by newlines)." },
                    "scale": { "type": "string", "enum": ["red-yellow-green", "green-yellow-red", "blue-white-red", "green", "blue"], "default": "red-yellow-green", "description": "Color scale for numeric cells: red-yellow-green (low=red→high=green, default), green-yellow-red, blue-white-red (diverging), green (white→green), or blue (white→blue)." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header (not shaded). Default true." },
                    "per_column": { "type": "boolean", "default": true, "description": "Scale each numeric column against its own min/max (default true); set false to use one global min/max across the whole table." },
                    "min": { "type": "number", "description": "Optional fixed value for the low end of the scale (defaults to the data minimum). When both min and max are set, per_column is ignored." },
                    "midpoint": { "type": "number", "description": "Optional fixed midpoint value for diverging scales (red-yellow-green / green-yellow-red / blue-white-red), mapped to the neutral middle color. Defaults to halfway between min and max. Ignored for sequential scales (green/blue)." },
                    "max": { "type": "number", "description": "Optional fixed value for the high end of the scale (defaults to the data maximum)." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
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
