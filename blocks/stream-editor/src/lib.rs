//! gizza-ai/stream-editor — run a safe sed-style command script over pasted text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_stream_editor_core::{
    LineEnding, Options, RegexFlavor, DEFAULT_MAX_OUTPUT_LINES, DEFAULT_SCRIPT,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_script")]
    script: String,
    #[serde(default)]
    quiet: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    whole_buffer: bool,
    #[serde(default = "default_regex_flavor")]
    regex_flavor: String,
    #[serde(default = "default_line_ending")]
    line_ending: String,
    #[serde(default = "default_max_output_lines")]
    max_output_lines: u32,
}

fn default_script() -> String {
    DEFAULT_SCRIPT.into()
}
fn default_regex_flavor() -> String {
    "basic".into()
}
fn default_line_ending() -> String {
    "lf".into()
}
fn default_max_output_lines() -> u32 {
    DEFAULT_MAX_OUTPUT_LINES as u32
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("Input text to edit. The script is applied line-by-line by default, similar to sed's pattern-space cycle."))
        .param(Param::string("script").default(DEFAULT_SCRIPT).describe("ed/sed-style command script. Supports addresses (line numbers, $, /regex/, ranges), s/// substitutions, d/D delete, p/P print, i/a/c text, y transliteration, labels and branches, hold-space commands, q/Q quit, = line numbers and l list."))
        .param(Param::boolean("quiet").default(false).describe("Suppress automatic printing like sed -n; only explicit p/P/= /l commands emit output. Default false."))
        .param(Param::boolean("ignore_case").default(false).describe("Make every regex address and substitution pattern case-insensitive. Default false."))
        .param(Param::boolean("whole_buffer").default(false).describe("Treat the entire input as one pattern space instead of one line per cycle. Useful for multi-line substitutions. Default false."))
        .param(Param::enumv("regex_flavor", ["basic", "extended"]).default("basic").describe("Regex syntax for script patterns: basic (sed/BRE style, default) or extended (sed -E style)."))
        .param(Param::enumv("line_ending", ["lf", "crlf"]).default("lf").describe("Output line ending: lf (Unix, default) or crlf (Windows)."))
        .param(Param::integer("max_output_lines").default(DEFAULT_MAX_OUTPUT_LINES as i64).min(1.0).max(DEFAULT_MAX_OUTPUT_LINES as f64).describe("Safety cap on emitted output lines. Default 100000."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StreamEditor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/stream-editor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Apply sed-style edit scripts to pasted text",
    skill(
        description = "Apply a safe ed/sed-style command script to pasted text in one pass. Supports line/regex addresses, ranges, substitutions, delete, print, insert/append/change text, transliteration, labels/branches, hold-space commands, line numbers and list output. Filesystem and shell sed commands are intentionally unavailable in the sandbox. Use quiet=true for sed -n behaviour, regex_flavor=extended for sed -E syntax, whole_buffer=true for multi-line edits, and max_output_lines as a safety cap.",
        parameters = schema_json()
    ),
)]
impl StreamEditor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "stream-editor", |a: Args| {
            let opts = Options {
                quiet: a.quiet,
                ignore_case: a.ignore_case,
                whole_buffer: a.whole_buffer,
                flavor: RegexFlavor::parse(&a.regex_flavor).map_err(SkillError::InvalidArgs)?,
                line_ending: LineEnding::parse(&a.line_ending).map_err(SkillError::InvalidArgs)?,
                max_output_lines: a.max_output_lines as usize,
            };
            gizza_ai_stream_editor_core::run(&a.text, &a.script, &opts)
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
    fn schema_has_expected_params() {
        let names: Vec<String> = descriptor().params.iter().map(|p| p.name.clone()).collect();
        assert_eq!(
            names,
            [
                "text",
                "script",
                "quiet",
                "ignore_case",
                "whole_buffer",
                "regex_flavor",
                "line_ending",
                "max_output_lines"
            ]
        );
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(
            schema["properties"]["regex_flavor"]["enum"],
            serde_json::json!(["basic", "extended"])
        );
    }
}
