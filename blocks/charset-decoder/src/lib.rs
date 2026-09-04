//! gizza-ai/charset-decoder — chat skill block on the shared tool abstraction.
//!
//! Decodes pasted hex/base64 bytes under a specified or detected character set.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill and returns the decoded
//! text plus diagnostics from the core crate.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_charset")]
    charset: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_errors")]
    errors: String,
    #[serde(default = "default_true")]
    strip_bom: bool,
    #[serde(default)]
    per_line: bool,
}

fn default_input_format() -> String {
    "auto".to_string()
}
fn default_charset() -> String {
    "auto".to_string()
}
fn default_output() -> String {
    "text".to_string()
}
fn default_errors() -> String {
    "replace".to_string()
}
fn default_true() -> bool {
    true
}

const INPUT_FORMATS: [&str; 3] = ["auto", "hex", "base64"];
const OUTPUTS: [&str; 5] = ["text", "escaped", "hexdump", "compare", "report"];
const ERRORS: [&str; 2] = ["replace", "strict"];

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Bytes to decode, pasted as hex (e.g. 48 65 6c 6c 6f) or base64 (e.g. SGVsbG8=). Hex may include spaces, colons, dashes, 0x or \\x byte prefixes; base64 may be padded, unpadded, URL-safe, multiline, or a data: URI."),
        )
        .param(
            Param::enumv("input_format", INPUT_FORMATS)
                .default("auto")
                .describe("How to read input: auto detects even-length hex dumps and otherwise base64; hex forces hexadecimal; base64 forces standard or URL-safe base64. Default auto."),
        )
        .param(
            Param::string("charset")
                .default("auto")
                .describe("Character set label to decode with, or auto. Accepts WHATWG labels such as utf-8, utf-16le, windows-1252/latin1, windows-1251, koi8-r, shift_jis/sjis, euc-jp, gbk, big5, euc-kr, plus utf-32le and utf-32be. Default auto."),
        )
        .param(
            Param::enumv("output", OUTPUTS)
                .default("text")
                .describe("Output view: text for decoded text, escaped to reveal control/invisible characters, hexdump for raw bytes, compare to show common charsets side by side, or report for diagnostics. Default text."),
        )
        .param(
            Param::enumv("errors", ERRORS)
                .default("replace")
                .describe("Malformed byte handling: replace substitutes U+FFFD and counts replacements; strict fails at the first invalid byte offset. Default replace."),
        )
        .param(
            Param::boolean("strip_bom")
                .default(true)
                .describe("Drop a leading byte-order mark when it matches the chosen charset (UTF-8/UTF-16/UTF-32). Default true."),
        )
        .param(
            Param::boolean("per_line")
                .default(false)
                .describe("Decode each non-empty input line independently, useful for log files containing one encoded value per line. Works with output=text or output=escaped. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/charset-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode hex or base64 byte dumps into text using a chosen or detected character set.",
    skill(
        description = "Decode raw bytes pasted as hex or base64 into readable text under a chosen character set. input is the hex dump or base64 body. input_format can be auto, hex or base64. charset can be auto or a label such as utf-8, utf-16le, windows-1252/latin1, windows-1251, koi8-r, shift_jis, euc-jp, gbk, big5, euc-kr, utf-32le or utf-32be. output can be text, escaped, hexdump, compare or report. errors controls malformed byte handling (replace or strict). strip_bom drops a matching BOM by default. per_line decodes each non-empty line independently for output=text or escaped. Returns the rendered text plus charset/input diagnostics.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "charset-decoder", |a: Args| {
            gizza_ai_charset_decoder_core::run(
                &a.input,
                &a.input_format,
                &a.charset,
                &a.output,
                &a.errors,
                a.strip_bom,
                a.per_line,
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

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so chat, CLI, manifest sync and page controls stay aligned.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"object",
                "properties":{
                    "input":{"type":"string","description":"Bytes to decode, pasted as hex (e.g. 48 65 6c 6c 6f) or base64 (e.g. SGVsbG8=). Hex may include spaces, colons, dashes, 0x or \\x byte prefixes; base64 may be padded, unpadded, URL-safe, multiline, or a data: URI."},
                    "input_format":{"type":"string","enum":["auto","hex","base64"],"default":"auto","description":"How to read input: auto detects even-length hex dumps and otherwise base64; hex forces hexadecimal; base64 forces standard or URL-safe base64. Default auto."},
                    "charset":{"type":"string","default":"auto","description":"Character set label to decode with, or auto. Accepts WHATWG labels such as utf-8, utf-16le, windows-1252/latin1, windows-1251, koi8-r, shift_jis/sjis, euc-jp, gbk, big5, euc-kr, plus utf-32le and utf-32be. Default auto."},
                    "output":{"type":"string","enum":["text","escaped","hexdump","compare","report"],"default":"text","description":"Output view: text for decoded text, escaped to reveal control/invisible characters, hexdump for raw bytes, compare to show common charsets side by side, or report for diagnostics. Default text."},
                    "errors":{"type":"string","enum":["replace","strict"],"default":"replace","description":"Malformed byte handling: replace substitutes U+FFFD and counts replacements; strict fails at the first invalid byte offset. Default replace."},
                    "strip_bom":{"type":"boolean","default":true,"description":"Drop a leading byte-order mark when it matches the chosen charset (UTF-8/UTF-16/UTF-32). Default true."},
                    "per_line":{"type":"boolean","default":false,"description":"Decode each non-empty input line independently, useful for log files containing one encoded value per line. Works with output=text or output=escaped. Default false."}
                },
                "required":["input"],
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(actual, authored);
    }
}
