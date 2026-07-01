//! gizza-ai/list-converter — reformat a list between comma/newline/bulleted/
//! numbered/quoted/space/tab/pipe/json/xml/sql forms. Thin wrapper; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_list_converter_core::{
    convert, parse_case_transform, parse_in_sep, parse_out_format, parse_sort_mode,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_separator: String,
    #[serde(default)]
    custom_input_separator: String,
    #[serde(default = "default_out")]
    output_format: String,
    #[serde(default)]
    custom_output_separator: String,
    #[serde(default)]
    sort_mode: String,
    #[serde(default)]
    dedupe: bool,
    #[serde(default)]
    case_transform: String,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    seed: u64,
}
fn default_out() -> String {
    "newline".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The list text to reformat."))
        .param(Param::enumv("input_separator", ["auto", "comma", "newline", "semicolon", "space", "tab", "pipe", "custom"]).default("auto").describe("How to split the input. 'auto' (default) detects separators."))
        .param(Param::string("custom_input_separator").default("").describe("Custom string delimiter to split the input if input_separator is 'custom'."))
        .param(Param::enumv("output_format", ["comma", "newline", "bulleted", "numbered", "quoted", "space", "tab", "pipe", "json", "xml", "sql", "custom"]).default("newline").describe("Output format."))
        .param(Param::string("custom_output_separator").default("").describe("Custom string delimiter to join output items if output_format is 'custom'."))
        .param(Param::enumv("sort_mode", ["none", "asc", "desc", "length_asc", "length_desc", "shuffle"]).default("none").describe("Sorting mode."))
        .param(Param::boolean("dedupe").default(false).describe("Remove duplicate items."))
        .param(Param::enumv("case_transform", ["none", "lowercase", "uppercase", "titlecase"]).default("none").describe("Case transformation."))
        .param(Param::string("prefix").default("").describe("Prefix to prepended to each item."))
        .param(Param::string("suffix").default("").describe("Suffix to appended to each item."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ListConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/list-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reformat a list (comma/newline/bullets/numbered/quoted/sql/json/xml).",
    skill(
        description = "Reformat a list between forms: comma-separated, newline, bulleted, numbered, quoted, space, tab, pipe, json, xml, sql, or custom separators. input_separator controls splitting; output_format controls layout. Optionally sort (alphabetical, length, reverse, shuffle), dedupe, case-transform (lower, upper, title), or prepend prefix/suffix. Items are trimmed and blanks dropped.",
        parameters = schema_json()
    )
)]
impl ListConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "list-converter", |a: Args| {
            let insep = parse_in_sep(&a.input_separator).map_err(SkillError::InvalidArgs)?;
            let outf = parse_out_format(&a.output_format).map_err(SkillError::InvalidArgs)?;
            let smode = parse_sort_mode(&a.sort_mode).map_err(SkillError::InvalidArgs)?;
            let ctrans = parse_case_transform(&a.case_transform).map_err(SkillError::InvalidArgs)?;
            
            // If shuffling and seed is 0, use system clock or basic hashing to avoid same shuffle
            let seed = if a.seed == 0 {
                match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(dur) => dur.as_nanos() as u64,
                    Err(_) => 42,
                }
            } else {
                a.seed
            };

            convert(
                &a.input,
                insep,
                &a.custom_input_separator,
                outf,
                &a.custom_output_separator,
                smode,
                a.dedupe,
                ctrans,
                &a.prefix,
                &a.suffix,
                seed,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input":                  { "type": "string", "description": "The list text to reformat." },
                    "input_separator":        { "type": "string", "enum": ["auto", "comma", "newline", "semicolon", "space", "tab", "pipe", "custom"], "default": "auto", "description": "How to split the input. 'auto' (default) detects separators." },
                    "custom_input_separator": { "type": "string", "default": "", "description": "Custom string delimiter to split the input if input_separator is 'custom'." },
                    "output_format":          { "type": "string", "enum": ["comma", "newline", "bulleted", "numbered", "quoted", "space", "tab", "pipe", "json", "xml", "sql", "custom"], "default": "newline", "description": "Output format." },
                    "custom_output_separator":{ "type": "string", "default": "", "description": "Custom string delimiter to join output items if output_format is 'custom'." },
                    "sort_mode":              { "type": "string", "enum": ["none", "asc", "desc", "length_asc", "length_desc", "shuffle"], "default": "none", "description": "Sorting mode." },
                    "dedupe":                 { "type": "boolean", "default": false, "description": "Remove duplicate items." },
                    "case_transform":         { "type": "string", "enum": ["none", "lowercase", "uppercase", "titlecase"], "default": "none", "description": "Case transformation." },
                    "prefix":                 { "type": "string", "default": "", "description": "Prefix to prepended to each item." },
                    "suffix":                 { "type": "string", "default": "", "description": "Suffix to appended to each item." }
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
