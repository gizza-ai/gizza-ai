//! gizza-ai/base64-diff — decode two Base64 / Base64url blobs and diff the
//! decoded BYTES: which offsets differ, by how much, and whether two
//! different-looking encodings actually carry the same payload. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_base64_diff_core::{
    diff_base64, parse_align, parse_alphabet, parse_output, Options,
};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    #[serde(default = "default_alphabet")]
    alphabet: String,
    #[serde(default)]
    strict: bool,
    #[serde(default = "default_align")]
    align: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_bytes_per_row")]
    bytes_per_row: u64,
    #[serde(default = "default_context_rows")]
    context_rows: u64,
}

fn default_alphabet() -> String {
    "auto".into()
}
fn default_align() -> String {
    "offset".into()
}
fn default_output() -> String {
    "report".into()
}
fn default_bytes_per_row() -> u64 {
    8
}
fn default_context_rows() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("left")
                .required()
                .describe("The first (baseline/old) Base64 or Base64url string. Line breaks, spaces and a leading data: URI prefix are ignored unless strict is on."),
        )
        .param(
            Param::string("right")
                .required()
                .describe("The second (candidate/new) Base64 or Base64url string, compared against left after both are decoded."),
        )
        .param(
            Param::enumv("alphabet", ["auto", "standard", "url"])
                .default("auto")
                .describe("Which Base64 alphabet to decode with: \"auto\" detects per side from the characters used (default), \"standard\" is RFC 4648 §4 (+ and /), \"url\" is RFC 4648 §5 Base64url (- and _). Auto rejects a side that mixes both alphabets."),
        )
        .param(
            Param::boolean("strict")
                .default(false)
                .describe("Require canonical RFC 4648 input: reject embedded whitespace/line wrapping, missing or extra '=' padding and non-zero trailing bits instead of repairing them. Default false (lenient)."),
        )
        .param(
            Param::enumv("align", ["offset", "shift"])
                .default("offset")
                .describe("How the payloads are lined up: \"offset\" compares byte i with byte i (default, best for fixed-layout data), \"shift\" trims the common prefix and suffix first so a single insertion or deletion is reported as one range instead of cascading through the rest of the file."),
        )
        .param(
            Param::enumv("output", ["report", "summary", "hexdump", "text"])
                .default("report")
                .describe("Result shape: \"report\" full JSON with per-side alphabet/padding/size/sha256/detected type plus the difference ranges (default); \"summary\" a readable verdict plus one line per differing range; \"hexdump\" side-by-side hex+ASCII with differing rows marked '*'; \"text\" a unified line diff of the decoded text (both payloads must be UTF-8)."),
        )
        .param(
            Param::integer("bytes_per_row")
                .min(4.0)
                .max(32.0)
                .default(8)
                .describe("Bytes per row in the hexdump output (4-32). Use 16 for a classic hexdump on a wide screen. Default 8."),
        )
        .param(
            Param::integer("context_rows")
                .min(0.0)
                .max(64.0)
                .default(2)
                .describe("How much unchanged context to keep around each change: hexdump rows for output=hexdump, lines for output=text (0-64). Everything further away is collapsed. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from_args(a: &Args) -> Result<Options, String> {
    if !(4..=32).contains(&a.bytes_per_row) {
        return Err(format!(
            "bytes_per_row must be between 4 and 32, got {}",
            a.bytes_per_row
        ));
    }
    if a.context_rows > 64 {
        return Err(format!(
            "context_rows must be between 0 and 64, got {}",
            a.context_rows
        ));
    }
    Ok(Options {
        alphabet: parse_alphabet(&a.alphabet)?,
        strict: a.strict,
        align: parse_align(&a.align)?,
        output: parse_output(&a.output)?,
        bytes_per_row: a.bytes_per_row as usize,
        context_rows: a.context_rows as usize,
    })
}

#[cfg(target_arch = "wasm32")]
struct Base64Diff;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base64-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode two Base64 or Base64url strings and diff the underlying bytes",
    skill(
        description = "Compare two Base64 (or Base64url) strings by DECODING them first and diffing the underlying bytes, so encoding noise never masquerades as a payload change. Whitespace/line wrapping, missing padding and a leading `data:...;base64,` prefix are repaired by default (set strict=true to reject them); the alphabet is detected per side (alphabet=standard|url to force one). Returns a JSON report { equal, identical_encoding, notes, left/right: { alphabet, base64_chars, padding, bytes, sha256, detected_type, utf8, text_preview }, diff: { align, first_difference_offset, differing_bytes, size_delta, common_prefix_bytes, common_suffix_bytes, ranges:[{ offset, length, kind: changed|added|removed, left_hex, right_hex }] } }, or output=summary for a readable verdict plus one line per range, output=hexdump for a side-by-side hex+ASCII dump with differing rows marked, or output=text for a unified line diff of the decoded text. align=shift trims the common prefix/suffix so one inserted byte reads as an insertion instead of shifting everything after it. Runs locally; each side is capped at 4 MiB of Base64.",
        parameters = schema_json()
    ),
)]
impl Base64Diff {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "base64-diff", |a: Args| {
            let opts = options_from_args(&a).map_err(SkillError::InvalidArgs)?;
            diff_base64(&a.left, &a.right, &opts).map_err(SkillError::InvalidArgs)
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
                    "left": {
                        "type": "string",
                        "description": "The first (baseline/old) Base64 or Base64url string. Line breaks, spaces and a leading data: URI prefix are ignored unless strict is on."
                    },
                    "right": {
                        "type": "string",
                        "description": "The second (candidate/new) Base64 or Base64url string, compared against left after both are decoded."
                    },
                    "alphabet": {
                        "type": "string",
                        "enum": ["auto", "standard", "url"],
                        "default": "auto",
                        "description": "Which Base64 alphabet to decode with: \"auto\" detects per side from the characters used (default), \"standard\" is RFC 4648 §4 (+ and /), \"url\" is RFC 4648 §5 Base64url (- and _). Auto rejects a side that mixes both alphabets."
                    },
                    "strict": {
                        "type": "boolean",
                        "default": false,
                        "description": "Require canonical RFC 4648 input: reject embedded whitespace/line wrapping, missing or extra '=' padding and non-zero trailing bits instead of repairing them. Default false (lenient)."
                    },
                    "align": {
                        "type": "string",
                        "enum": ["offset", "shift"],
                        "default": "offset",
                        "description": "How the payloads are lined up: \"offset\" compares byte i with byte i (default, best for fixed-layout data), \"shift\" trims the common prefix and suffix first so a single insertion or deletion is reported as one range instead of cascading through the rest of the file."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["report", "summary", "hexdump", "text"],
                        "default": "report",
                        "description": "Result shape: \"report\" full JSON with per-side alphabet/padding/size/sha256/detected type plus the difference ranges (default); \"summary\" a readable verdict plus one line per differing range; \"hexdump\" side-by-side hex+ASCII with differing rows marked '*'; \"text\" a unified line diff of the decoded text (both payloads must be UTF-8)."
                    },
                    "bytes_per_row": {
                        "type": "integer",
                        "minimum": 4,
                        "maximum": 32,
                        "default": 8,
                        "description": "Bytes per row in the hexdump output (4-32). Use 16 for a classic hexdump on a wide screen. Default 8."
                    },
                    "context_rows": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 64,
                        "default": 2,
                        "description": "How much unchanged context to keep around each change: hexdump rows for output=hexdump, lines for output=text (0-64). Everything further away is collapsed. Default 2."
                    }
                },
                "required": ["left", "right"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }

    #[test]
    fn args_defaults_match_descriptor_defaults() {
        let a: Args =
            serde_json::from_str(r#"{"left":"SGk=","right":"SGk="}"#).unwrap();
        assert_eq!(a.alphabet, "auto");
        assert_eq!(a.align, "offset");
        assert_eq!(a.output, "report");
        assert_eq!(a.bytes_per_row, 8);
        assert_eq!(a.context_rows, 2);
        assert!(!a.strict);
        let opts = options_from_args(&a).unwrap();
        let out = diff_base64(&a.left, &a.right, &opts).unwrap();
        assert!(out.contains("\"equal\": true"), "got {out}");
    }

    #[test]
    fn rejects_unknown_choice_values() {
        let a: Args =
            serde_json::from_str(r#"{"left":"SGk=","right":"SGk=","output":"chart"}"#).unwrap();
        let err = options_from_args(&a).unwrap_err();
        assert!(err.contains("report, summary, hexdump, text"), "got {err}");
    }

    #[test]
    fn rejects_out_of_range_hexdump_width() {
        let a: Args =
            serde_json::from_str(r#"{"left":"SGk=","right":"SGk=","bytes_per_row":64}"#).unwrap();
        let err = options_from_args(&a).unwrap_err();
        assert!(err.contains("between 4 and 32"), "got {err}");
    }

    #[test]
    fn summary_output_reports_the_first_differing_offset() {
        let a: Args = serde_json::from_str(
            r#"{"left":"SGVsbG8gd29ybGQh","right":"SGVsbG8gV29ybGQh","output":"summary"}"#,
        )
        .unwrap();
        let opts = options_from_args(&a).unwrap();
        let out = diff_base64(&a.left, &a.right, &opts).unwrap();
        assert!(out.contains("First difference at offset 0x0006 (6)."), "got {out}");
    }
}
