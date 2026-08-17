//! gizza-ai/json-array-pluck-field — pull ONE field out of every object in a JSON
//! array into a flat list. Chat schema single-sourced from descriptor() (which
//! also drives the CLI); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_array_pluck_field_core::{pluck, Complex, Format, Missing, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    field: String,
    #[serde(default)]
    root: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default)]
    quote: bool,
    #[serde(default = "default_missing")]
    missing: String,
    #[serde(default = "default_complex")]
    complex_values: String,
    #[serde(default)]
    unique: bool,
}

fn default_format() -> String {
    "lines".into()
}
fn default_delimiter() -> String {
    ", ".into()
}
fn default_missing() -> String {
    "skip".into()
}
fn default_complex() -> String {
    "json".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON to pluck from: an array of objects, a wrapper object containing one ({\"data\": [...]}), NDJSON / JSON Lines (one object per line), or a single object treated as one row. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("field")
                .required()
                .describe("Key to pull from every object. Use dot-notation for nested fields ('user.name') or an array index ('tags.0' or 'tags[0]'); '*' matches every element of an array or every value of an object, so 'orders.*.total' emits one value per order; '**' searches every depth, so '**.city' finds a city however deep it is nested. JSONPath spellings work too: a leading '$' is ignored, '..' means '**', and quoted keys ('$['user'].name') are unwrapped. Max 200 bytes."),
        )
        .param(
            Param::string("root")
                .default("")
                .describe("Dot-path of the array to pluck from, e.g. 'response.items'. Leave blank to auto-detect: a top-level array is used as-is, otherwise the first array-valued property of the top-level object."),
        )
        .param(
            Param::enumv("format", ["lines", "csv", "tsv", "json", "custom"])
                .default("lines")
                .describe("How the values are joined: 'lines' one per line (default), 'csv' comma-separated with RFC 4180 quoting, 'tsv' tab-separated, 'json' a JSON array keeping native types (numbers stay numbers), or 'custom' to join with 'delimiter'."),
        )
        .param(
            Param::string("delimiter")
                .default(", ")
                .describe("Separator used only when format=custom, e.g. ', ' or ' | '. Escapes \\t, \\n, \\r and \\\\ are honoured. Default ', '."),
        )
        .param(
            Param::boolean("quote")
                .default(false)
                .describe("Wrap every value in double quotes — handy for building a SQL IN list or a JS array. csv/tsv force RFC 4180 quoting (inner \" doubled); lines/custom use JSON escaping. Ignored when format=json. Off by default (csv/tsv still quote values that need it)."),
        )
        .param(
            Param::enumv("missing", ["skip", "empty", "null", "error"])
                .default("skip")
                .describe("What to do with an object whose field is absent or JSON null: 'skip' leave it out (default), 'empty' emit an empty value so the list stays row-aligned, 'null' emit the literal null, or 'error' fail and report the element index."),
        )
        .param(
            Param::enumv("complex_values", ["json", "label", "skip"])
                .default("json")
                .describe("What to do when the plucked value is itself an object or array: 'json' serialize it compactly (default), 'label' replace it with {object} / [array], or 'skip' leave it out."),
        )
        .param(
            Param::boolean("unique")
                .default(false)
                .describe("Drop repeated values, keeping the first occurrence in document order. Off by default, so the value count matches the row count."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Options {
    Options {
        field: a.field.clone(),
        root: a.root.clone(),
        format: Format::parse(&a.format),
        delimiter: a.delimiter.clone(),
        quote: a.quote,
        missing: Missing::parse(&a.missing),
        complex: Complex::parse(&a.complex_values),
        unique: a.unique,
    }
}

#[cfg(target_arch = "wasm32")]
struct JsonArrayPluckField;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-array-pluck-field",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pull one field out of every object in a JSON array into a flat list",
    skill(
        description = "Extract a single field from every object in a JSON array and return the values as a flat list (the _.map(rows,'field') / jq '.[].field' / single-column SELECT operation) — no query language to learn. json accepts a top-level array, a wrapper object holding the array, NDJSON/JSON Lines, or a single object; set root to a dot-path like 'response.items' to choose the array explicitly. field is a key name, a dot-path ('user.name'), an index ('tags.0'), a wildcard path ('orders.*.total'), or a recursive-descent path ('**.city', JSONPath '..city'). format joins the values as 'lines' (default), 'csv', 'tsv', 'json' (native types) or 'custom' with delimiter; quote wraps every value in double quotes; missing controls absent/null fields ('skip' default, 'empty', 'null', 'error'); complex_values renders object/array values as 'json' (default), 'label' or 'skip'; unique drops repeats. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonArrayPluckField {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-array-pluck-field", |a: Args| {
            let opts = build_options(&a);
            pluck(&a.json, &opts).map_err(SkillError::InvalidArgs)
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
                    "json":           { "type": "string", "description": "The JSON to pluck from: an array of objects, a wrapper object containing one ({\"data\": [...]}), NDJSON / JSON Lines (one object per line), or a single object treated as one row. Max 5,000,000 bytes." },
                    "field":          { "type": "string", "description": "Key to pull from every object. Use dot-notation for nested fields ('user.name') or an array index ('tags.0' or 'tags[0]'); '*' matches every element of an array or every value of an object, so 'orders.*.total' emits one value per order; '**' searches every depth, so '**.city' finds a city however deep it is nested. JSONPath spellings work too: a leading '$' is ignored, '..' means '**', and quoted keys ('$['user'].name') are unwrapped. Max 200 bytes." },
                    "root":           { "type": "string", "default": "", "description": "Dot-path of the array to pluck from, e.g. 'response.items'. Leave blank to auto-detect: a top-level array is used as-is, otherwise the first array-valued property of the top-level object." },
                    "format":         { "type": "string", "enum": ["lines", "csv", "tsv", "json", "custom"], "default": "lines", "description": "How the values are joined: 'lines' one per line (default), 'csv' comma-separated with RFC 4180 quoting, 'tsv' tab-separated, 'json' a JSON array keeping native types (numbers stay numbers), or 'custom' to join with 'delimiter'." },
                    "delimiter":      { "type": "string", "default": ", ", "description": "Separator used only when format=custom, e.g. ', ' or ' | '. Escapes \\t, \\n, \\r and \\\\ are honoured. Default ', '." },
                    "quote":          { "type": "boolean", "default": false, "description": "Wrap every value in double quotes — handy for building a SQL IN list or a JS array. csv/tsv force RFC 4180 quoting (inner \" doubled); lines/custom use JSON escaping. Ignored when format=json. Off by default (csv/tsv still quote values that need it)." },
                    "missing":        { "type": "string", "enum": ["skip", "empty", "null", "error"], "default": "skip", "description": "What to do with an object whose field is absent or JSON null: 'skip' leave it out (default), 'empty' emit an empty value so the list stays row-aligned, 'null' emit the literal null, or 'error' fail and report the element index." },
                    "complex_values": { "type": "string", "enum": ["json", "label", "skip"], "default": "json", "description": "What to do when the plucked value is itself an object or array: 'json' serialize it compactly (default), 'label' replace it with {object} / [array], or 'skip' leave it out." },
                    "unique":         { "type": "boolean", "default": false, "description": "Drop repeated values, keeping the first occurrence in document order. Off by default, so the value count matches the row count." }
                },
                "required": ["json", "field"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
