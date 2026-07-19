//! gizza-ai/extract-numbers-from-text — pull every numeric value out of prose,
//! logs, or CSV-ish text into a clean list. Recognises integers, decimals,
//! scientific notation, signed numbers, and thousands-separator-grouped numbers
//! (`1,000,000`). Optional filter (all/integers/decimals), de-duplication,
//! sorting, output delimiter, and summary statistics. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_numbers_from_text_core::{extract, render, Delimiter, Mode, Sort};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    unique: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default)]
    stats: bool,
}

fn default_mode() -> String {
    "all".to_string()
}
fn default_sort() -> String {
    "original".to_string()
}
fn default_delimiter() -> String {
    "newline".to_string()
}

#[derive(Serialize)]
struct Resp {
    /// The extracted numbers joined by the chosen delimiter, with an optional
    /// trailing statistics block when `stats` is set.
    output: String,
    /// Count of numbers returned (after filter + optional de-duplication).
    count: usize,
    /// The number tokens exactly as they appeared, post-filter/dedupe/sort.
    numbers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    average: Option<f64>,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text, log, or document to pull numbers out of."),
        )
        .param(
            Param::enumv("mode", ["all", "integers", "decimals"])
                .default("all")
                .describe("Which numbers to keep: all, only integers (no decimal point), or only decimals."),
        )
        .param(
            Param::boolean("unique")
                .default(false)
                .describe("When true, drop duplicate values (1,000 == 1000, +5 == 5); first-seen wins."),
        )
        .param(
            Param::enumv("sort", ["original", "ascending", "descending"])
                .default("original")
                .describe("Output order: original (first-seen), ascending, or descending by numeric value."),
        )
        .param(
            Param::enumv("delimiter", ["newline", "comma", "space", "tab", "semicolon"])
                .default("newline")
                .describe("How to join the extracted numbers in the output text. Default newline."),
        )
        .param(
            Param::boolean("stats")
                .default(false)
                .describe("When true, append a summary block: count, sum, min, max, and average."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-numbers-from-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pull every number out of text into a clean list",
    skill(
        description = "Pull every numeric value out of prose, logs, or CSV-ish text into a clean list. Recognizes integers, decimals, scientific notation (6.022e23), signed numbers (-7, +5), and thousands-separator-grouped numbers (1,000,000); a hyphen glued to a preceding digit (as in a date like 2024-01-15) is treated as a separator, not a minus sign. Set mode to keep all numbers, only integers, or only decimals. Set unique=true to drop duplicate values (1,000 equals 1000). sort orders the output original/ascending/descending. delimiter joins the results (newline, comma, space, tab, or semicolon). Set stats=true to append count, sum, min, max, and average. Returns the joined output text plus the structured list and statistics. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-numbers-from-text", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            let sort = Sort::parse(&a.sort).map_err(SkillError::InvalidArgs)?;
            let delimiter = Delimiter::parse(&a.delimiter).map_err(SkillError::InvalidArgs)?;
            let r = extract(&a.text, mode, a.unique, sort);
            let output = render(&a.text, mode, a.unique, sort, delimiter, a.stats);
            Ok::<Resp, SkillError>(Resp {
                output,
                count: r.count,
                numbers: r.numbers,
                sum: r.sum,
                min: r.min,
                max: r.max,
                average: r.average,
            })
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
                    "text": { "type": "string", "description": "The text, log, or document to pull numbers out of." },
                    "mode": { "type": "string", "enum": ["all", "integers", "decimals"], "default": "all", "description": "Which numbers to keep: all, only integers (no decimal point), or only decimals." },
                    "unique": { "type": "boolean", "default": false, "description": "When true, drop duplicate values (1,000 == 1000, +5 == 5); first-seen wins." },
                    "sort": { "type": "string", "enum": ["original", "ascending", "descending"], "default": "original", "description": "Output order: original (first-seen), ascending, or descending by numeric value." },
                    "delimiter": { "type": "string", "enum": ["newline", "comma", "space", "tab", "semicolon"], "default": "newline", "description": "How to join the extracted numbers in the output text. Default newline." },
                    "stats": { "type": "boolean", "default": false, "description": "When true, append a summary block: count, sum, min, max, and average." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
