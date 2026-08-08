//! gizza-ai/json5-convert — converts JSON5/JSONC to strict JSON and back.
//!
//! Thin chat-skill wrapper around `gizza-ai-json5-convert-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

/// serde default for `unquote_keys`, which is on unless explicitly disabled.
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    indent: String,
    #[serde(default)]
    sort_keys: bool,
    #[serde(default)]
    nonfinite: String,
    #[serde(default)]
    quote_style: String,
    #[serde(default = "default_true")]
    unquote_keys: bool,
    #[serde(default)]
    trailing_commas: bool,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The JSON5, JSONC or strict JSON text to convert. JSON5 input may use // and /* */ comments, trailing commas, unquoted keys, single quotes, hex numbers and NaN/Infinity. Example: {port: 8080, /* dev */ hosts: ['a','b',],}"),
        )
        .param(
            Param::enumv("direction", ["to-json", "to-json5", "auto"])
                .default("to-json")
                .describe("Which way to convert. 'to-json' (default) emits strict RFC 8259 JSON — comments and trailing commas are dropped, keys and strings become double-quoted. 'to-json5' emits JSON5 — unquoted keys, single quotes, optional trailing commas. 'auto' emits strict JSON when the input used any JSON5-only syntax, and JSON5 when the input was already strict JSON."),
        )
        .param(
            Param::enumv("indent", ["2", "4", "tab", "minify"])
                .default("2")
                .describe("Output formatting: '2' (default) or '4' spaces of indentation per level, 'tab' for tab indentation, or 'minify' for a single compact line."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("When true, sort every object's keys alphabetically (by Unicode code point) at every nesting level. Default false, which keeps the input's key order."),
        )
        .param(
            Param::enumv("nonfinite", ["null", "string", "error"])
                .default("null")
                .describe("What to do with the JSON5 literals NaN, Infinity and -Infinity when writing strict JSON, which cannot express them: 'null' (default, what JSON.stringify does), 'string' to emit them as \"NaN\"/\"Infinity\"/\"-Infinity\", or 'error' to refuse the conversion. Ignored when direction='to-json5', which keeps the literals as-is."),
        )
        .param(
            Param::enumv("quote_style", ["single", "double"])
                .default("single")
                .describe("Quote character for strings and quoted keys in JSON5 output: 'single' (default, the JSON5 house style) or 'double'. Ignored when writing strict JSON, which always uses double quotes."),
        )
        .param(
            Param::boolean("unquote_keys")
                .default(true)
                .describe("When true (default), JSON5 output leaves object keys unquoted where they are valid ASCII identifiers (letters, digits, _ and $, not starting with a digit); other keys stay quoted. Set false to quote every key. Ignored when writing strict JSON."),
        )
        .param(
            Param::boolean("trailing_commas")
                .default(false)
                .describe("When true, JSON5 output adds a trailing comma after the last element of every non-empty array and object, so later diffs touch one line. Default false. Ignored when writing strict JSON."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Json5Convert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json5-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert JSON5/JSONC to strict JSON and back.",
    skill(
        description = "Convert JSON5 or JSONC (JSON with comments) to strict RFC 8259 JSON, or turn strict JSON back into JSON5. Reads the full JSON5 grammar: // and /* */ comments, trailing commas, unquoted identifier keys, single-quoted strings, escaped line continuations, \\x escapes, hexadecimal / leading-dot / trailing-dot / leading-plus numbers, and NaN / Infinity / -Infinity. Use direction='to-json' (default) to strip comments and normalize a tsconfig.json or VS Code settings.json for a strict parser; direction='to-json5' to emit unquoted keys and single quotes; direction='auto' to convert whichever way the input is not. indent picks 2/4/tab spaces or 'minify'; sort_keys=true sorts keys at every level; nonfinite says how NaN/Infinity become strict JSON (null, string or error). Object key order is preserved and repeated keys collapse last-wins. Parse errors report the line and column. Comments carry no data and are always dropped — output is data only.",
        parameters = schema_json()
    )
)]
impl Json5Convert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json5-convert", |a: Args| {
            gizza_ai_json5_convert_core::convert(
                &a.text,
                &a.direction,
                &a.indent,
                a.sort_keys,
                &a.nonfinite,
                &a.quote_style,
                a.unquote_keys,
                a.trailing_commas,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The JSON5, JSONC or strict JSON text to convert. JSON5 input may use // and /* */ comments, trailing commas, unquoted keys, single quotes, hex numbers and NaN/Infinity. Example: {port: 8080, /* dev */ hosts: ['a','b',],}" },
                    "direction": { "type": "string", "enum": ["to-json", "to-json5", "auto"], "default": "to-json", "description": "Which way to convert. 'to-json' (default) emits strict RFC 8259 JSON — comments and trailing commas are dropped, keys and strings become double-quoted. 'to-json5' emits JSON5 — unquoted keys, single quotes, optional trailing commas. 'auto' emits strict JSON when the input used any JSON5-only syntax, and JSON5 when the input was already strict JSON." },
                    "indent": { "type": "string", "enum": ["2", "4", "tab", "minify"], "default": "2", "description": "Output formatting: '2' (default) or '4' spaces of indentation per level, 'tab' for tab indentation, or 'minify' for a single compact line." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "When true, sort every object's keys alphabetically (by Unicode code point) at every nesting level. Default false, which keeps the input's key order." },
                    "nonfinite": { "type": "string", "enum": ["null", "string", "error"], "default": "null", "description": "What to do with the JSON5 literals NaN, Infinity and -Infinity when writing strict JSON, which cannot express them: 'null' (default, what JSON.stringify does), 'string' to emit them as \"NaN\"/\"Infinity\"/\"-Infinity\", or 'error' to refuse the conversion. Ignored when direction='to-json5', which keeps the literals as-is." },
                    "quote_style": { "type": "string", "enum": ["single", "double"], "default": "single", "description": "Quote character for strings and quoted keys in JSON5 output: 'single' (default, the JSON5 house style) or 'double'. Ignored when writing strict JSON, which always uses double quotes." },
                    "unquote_keys": { "type": "boolean", "default": true, "description": "When true (default), JSON5 output leaves object keys unquoted where they are valid ASCII identifiers (letters, digits, _ and $, not starting with a digit); other keys stay quoted. Set false to quote every key. Ignored when writing strict JSON." },
                    "trailing_commas": { "type": "boolean", "default": false, "description": "When true, JSON5 output adds a trailing comma after the last element of every non-empty array and object, so later diffs touch one line. Default false. Ignored when writing strict JSON." }
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
