//! gizza-ai/text-file-inspector — chat skill block on the shared tool
//! abstraction. Reads a text file's raw bytes (as literal text, base64 or hex)
//! and reports the encoding, BOM, line-ending style, line/character metrics,
//! whitespace hygiene and the longest lines. The chat schema is single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! run_skill. Pure Rust → runs on all backends including the chat Service
//! Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output: String,
    #[serde(default = "d_longest_lines")]
    longest_lines: i64,
    #[serde(default)]
    max_line_length: i64,
    #[serde(default)]
    preview_lines: i64,
}

fn d_longest_lines() -> i64 {
    3
}

/// Single source for the chat schema (and the CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "The file's bytes. Read according to input_format: literal text (default), a base64 string, or a hex string. Base64 is how you pass a real file, e.g. `--input \"$(base64 -w0 notes.txt)\" --input-format base64`; pasting text instead measures the pasted text's UTF-8 bytes, which cannot carry a BOM or CRLF endings.",
                ),
        )
        .param(
            Param::enumv("input_format", ["text", "base64", "hex"])
                .default("text")
                .describe(
                    "How to read 'input' into bytes: 'text' (default, its UTF-8 bytes), 'base64' (standard or URL-safe, whitespace and padding tolerated), or 'hex' (whitespace, ':' '-' '_' ',' and 0x / \\x prefixes ignored).",
                ),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe(
                    "Output format: 'report' (default) is an aligned human-readable summary; 'json' is a machine-readable object with the same facts (bytes, encoding, detection, bom, line_endings, lines, indentation, characters, max_line_length, preview, notes).",
                ),
        )
        .param(
            Param::integer("longest_lines")
                .default(3)
                .min(0.0)
                .max(50.0)
                .describe(
                    "How many of the longest lines to list with their line numbers and text (truncated to 120 chars). Default 3; 1 lists none separately and only summarises the single longest line. Clamped to 0-50.",
                ),
        )
        .param(
            Param::integer("max_line_length")
                .default(0)
                .min(0.0)
                .max(100000.0)
                .describe(
                    "Flag every line longer than this many characters and list its line number — the linter check (80, 100, 120 are common). Default 0 = off.",
                ),
        )
        .param(
            Param::integer("preview_lines")
                .default(0)
                .min(0.0)
                .max(200.0)
                .describe(
                    "Show the first N lines with visible markers: ␍ = CR, ␊ = LF, → = tab, ⏎̸ = no terminator. Default 0 = off. Clamped to 0-200.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-file-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Report a text file's encoding, BOM, line endings, line count and longest lines",
    skill(
        description = "Inspect a text file and report what an editor's status bar shows plus what a linter would complain about: detected character encoding (BOM, ASCII, valid UTF-8, BOM-less UTF-16 heuristic, or a chardetng guess for legacy bytes), BOM presence and its bytes, line-ending style with separate CRLF/LF/CR counts and percentages (flagging MIXED), line count split into terminated and unterminated, whether the file ends with a final newline, the longest lines with their numbers, average line length, empty and whitespace-only lines, lines with trailing whitespace, tab vs space indentation (flagging lines that mix both), character counts (total, non-ASCII, control, NUL) and U+2028/U+2029 separators. Pass the file's bytes in 'input' — base64 for a real file (`base64 -w0 file.txt`), or literal text. Set max_line_length to flag over-long lines and preview_lines to see the first N lines with visible ␍/␊ markers. output=json returns the same facts as an object. Diagnoses only; it never rewrites the file (use text-encoding-converter to convert charsets). Up to 8 MiB. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-file-inspector", |a: Args| {
            gizza_ai_text_file_inspector_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                a.longest_lines,
                a.max_line_length,
                a.preview_lines,
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
                    "input": { "type": "string", "description": "The file's bytes. Read according to input_format: literal text (default), a base64 string, or a hex string. Base64 is how you pass a real file, e.g. `--input \"$(base64 -w0 notes.txt)\" --input-format base64`; pasting text instead measures the pasted text's UTF-8 bytes, which cannot carry a BOM or CRLF endings." },
                    "input_format": { "type": "string", "enum": ["text", "base64", "hex"], "default": "text", "description": "How to read 'input' into bytes: 'text' (default, its UTF-8 bytes), 'base64' (standard or URL-safe, whitespace and padding tolerated), or 'hex' (whitespace, ':' '-' '_' ',' and 0x / \\x prefixes ignored)." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output format: 'report' (default) is an aligned human-readable summary; 'json' is a machine-readable object with the same facts (bytes, encoding, detection, bom, line_endings, lines, indentation, characters, max_line_length, preview, notes)." },
                    "longest_lines": { "type": "integer", "minimum": 0, "maximum": 50, "default": 3, "description": "How many of the longest lines to list with their line numbers and text (truncated to 120 chars). Default 3; 1 lists none separately and only summarises the single longest line. Clamped to 0-50." },
                    "max_line_length": { "type": "integer", "minimum": 0, "maximum": 100000, "default": 0, "description": "Flag every line longer than this many characters and list its line number — the linter check (80, 100, 120 are common). Default 0 = off." },
                    "preview_lines": { "type": "integer", "minimum": 0, "maximum": 200, "default": 0, "description": "Show the first N lines with visible markers: ␍ = CR, ␊ = LF, → = tab, ⏎̸ = no terminator. Default 0 = off. Clamped to 0-200." }
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
