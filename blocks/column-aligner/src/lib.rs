//! gizza-ai/column-aligner — align whitespace- or delimiter-separated text into fixed-width columns.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_align")]
    align: String,
    #[serde(default)]
    column_align: String,
    #[serde(default = "default_gap")]
    gap: u32,
    #[serde(default)]
    separator: String,
    #[serde(default = "default_trim")]
    trim: bool,
}

fn default_delimiter() -> String { "whitespace".to_string() }
fn default_align() -> String { "left".to_string() }
fn default_gap() -> u32 { 2 }
fn default_trim() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Rows to align, one record per line. Default delimiter is runs of whitespace, like `column -t`. Blank lines are preserved; max 20,000 lines and 512 columns."),
        )
        .param(
            Param::enumv("delimiter", ["whitespace", "tab", "comma", "semicolon", "colon", "pipe", "space"])
                .default("whitespace")
                .describe("How to split each input row into fields. `whitespace` splits on runs of spaces/tabs; named literal delimiters include tab, comma, semicolon, colon, pipe, and space. Default whitespace."),
        )
        .param(
            Param::enumv("align", ["left", "right", "center", "auto"])
                .default("left")
                .describe("Alignment applied to every column unless column_align overrides it. `auto` right-aligns columns whose non-empty cells are all numeric and left-aligns the rest. Default left."),
        )
        .param(
            Param::string("column_align")
                .default("")
                .describe("Optional per-column alignment override, e.g. `lrr` or `left,right,center`. Use `-` to inherit the main align setting. Short specs leave remaining columns on align."),
        )
        .param(
            Param::integer("gap")
                .default(2)
                .min(0.0)
                .max(16.0)
                .describe("Spaces between columns. With a separator, the gap is placed on both sides of that separator. Default 2, allowed 0–16."),
        )
        .param(
            Param::string("separator")
                .default("")
                .describe("Optional string drawn between columns, such as `|`. Leave empty for plain spaces only."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("Trim whitespace around fields after splitting on a literal delimiter. Whitespace-delimited input is always trimmed by the split. Default true."),
        )
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/column-aligner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Align delimited text into fixed-width columns",
    skill(
        description = "Align whitespace-, tab-, comma-, pipe-, or other delimiter-separated text into neat fixed-width plain-text columns, like `column -t`. Supports left, right, center, automatic numeric alignment, per-column overrides, configurable gaps, optional separator text, Unicode display-width padding, blank-line preservation, and no trailing whitespace. Limits: 20,000 lines and 512 columns.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "column-aligner", |a: Args| {
            gizza_ai_column_aligner_core::run(
                &a.input,
                &a.delimiter,
                &a.align,
                &a.column_align,
                a.gap,
                &a.separator,
                a.trim,
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
    fn schema_has_real_params_and_no_todos() {
        let schema = schema_json();
        assert!(!schema.contains("TODO"), "{schema}");
        let v: serde_json::Value = serde_json::from_str(&schema).unwrap();
        assert_eq!(v["required"], serde_json::json!(["input"]));
        assert_eq!(v["properties"]["delimiter"]["enum"], serde_json::json!(["whitespace", "tab", "comma", "semicolon", "colon", "pipe", "space"]));
        assert_eq!(v["properties"]["align"]["enum"], serde_json::json!(["left", "right", "center", "auto"]));
        assert_eq!(v["properties"]["gap"]["minimum"], 0);
        assert_eq!(v["properties"]["gap"]["maximum"], 16);
        assert_eq!(v["properties"]["trim"]["default"], true);
    }

    #[test]
    fn descriptor_defaults_match_core_wrapper() {
        let out = gizza_ai_column_aligner_core::run(
            "name age\nalice 30",
            &default_delimiter(),
            &default_align(),
            "",
            default_gap(),
            "",
            default_trim(),
        )
        .unwrap();
        assert_eq!(out, "name   age\nalice  30");
    }
}
