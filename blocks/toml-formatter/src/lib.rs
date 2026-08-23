//! gizza-ai/toml-formatter — format and validate TOML while preserving comments.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    indent: usize,
    #[serde(default = "default_sort_keys")]
    sort_keys: String,
    #[serde(default = "default_spacing")]
    spacing: String,
    #[serde(default = "default_array_style")]
    array_style: String,
    #[serde(default = "default_column_width")]
    column_width: usize,
    #[serde(default)]
    align_values: bool,
    #[serde(default = "default_true")]
    blank_line_before_tables: bool,
    #[serde(default = "default_true")]
    keep_comments: bool,
}

fn default_sort_keys() -> String {
    "preserve".to_string()
}
fn default_spacing() -> String {
    "standard".to_string()
}
fn default_array_style() -> String {
    "auto".to_string()
}
fn default_column_width() -> usize {
    80
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("TOML document to validate and format. Comments, scalar literal spelling and key/value content are preserved where possible."))
        .param(Param::integer("indent").default(0).min(0.0).max(8.0).describe("Spaces to indent entries inside tables, from 0 to 8. Default 0 keeps common Cargo.toml and pyproject.toml flat."))
        .param(Param::enumv("sort_keys", ["preserve", "asc", "desc"]).default("preserve").describe("Key ordering within each table and inline table: preserve the source order, sort ascending, or sort descending."))
        .param(Param::enumv("spacing", ["standard", "compact"]).default("standard").describe("Whitespace around equals signs. 'standard' emits `key = value`; 'compact' emits `key=value`."))
        .param(Param::enumv("array_style", ["auto", "expand", "collapse"]).default("auto").describe("Array layout. 'auto' keeps short arrays on one line and expands long/commented arrays; 'expand' writes one element per line; 'collapse' forces one line."))
        .param(Param::integer("column_width").default(80).min(20.0).max(200.0).describe("Line-width budget used by array_style=auto before expanding arrays, from 20 to 200 columns."))
        .param(Param::boolean("align_values").default(false).describe("Pad keys so equals signs align within a run of table entries. Ignored when spacing=compact."))
        .param(Param::boolean("blank_line_before_tables").default(true).describe("Insert exactly one blank line before each table or array-of-tables header. Default true."))
        .param(Param::boolean("keep_comments").default(true).describe("Preserve own-line and end-of-line comments when the selected layout can represent them. Disable to strip comments."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TomlFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/toml-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Format TOML with validation, sorting and comment preservation",
    skill(
        description = "Validate and format a pasted TOML document. Pass the TOML text as `input`; choose indentation, key sorting, equals spacing, array layout, column width, value alignment, blank-line handling and whether to preserve comments. Invalid TOML returns a line and column error instead of emitting partial output. The formatter preserves comments and scalar literal spellings such as hex integers, underscores and literal strings where the requested layout can represent them.",
        parameters = schema_json()
    ),
)]
impl TomlFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "toml-formatter", |a: Args| {
            gizza_ai_toml_formatter_core::run(
                &a.input,
                a.indent,
                &a.sort_keys,
                &a.spacing,
                &a.array_style,
                a.column_width,
                a.align_values,
                a.blank_line_before_tables,
                a.keep_comments,
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
    fn schema_has_expected_parameters_and_defaults() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for key in [
            "input",
            "indent",
            "sort_keys",
            "spacing",
            "array_style",
            "column_width",
            "align_values",
            "blank_line_before_tables",
            "keep_comments",
        ] {
            assert!(props.contains_key(key), "missing {key}");
            assert!(props[key]["description"].as_str().unwrap_or_default().len() > 20);
        }
        assert_eq!(schema["required"], serde_json::json!(["input"]));
        assert_eq!(props["sort_keys"]["default"], "preserve");
        assert_eq!(props["spacing"]["default"], "standard");
        assert_eq!(props["array_style"]["default"], "auto");
        assert_eq!(props["indent"]["minimum"], 0);
        assert_eq!(props["indent"]["maximum"], 8);
        assert_eq!(props["column_width"]["minimum"], 20);
        assert_eq!(props["column_width"]["maximum"], 200);
        assert_eq!(props["blank_line_before_tables"]["default"], true);
        assert_eq!(props["keep_comments"]["default"], true);
    }

    #[test]
    fn args_defaults_match_descriptor() {
        let a: Args = serde_json::from_str(r#"{"input":"a=1"}"#).unwrap();
        assert_eq!(a.indent, 0);
        assert_eq!(a.sort_keys, "preserve");
        assert_eq!(a.spacing, "standard");
        assert_eq!(a.array_style, "auto");
        assert_eq!(a.column_width, 80);
        assert!(!a.align_values);
        assert!(a.blank_line_before_tables);
        assert!(a.keep_comments);
    }
}
