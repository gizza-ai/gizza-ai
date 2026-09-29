//! gizza-ai/charcode-codec — converts text into a list of numeric character
//! codes (Unicode code points, UTF-8 bytes, UTF-16 units or ASCII) in a chosen
//! base, and rebuilds the text from such a list. Thin chat-skill wrapper around
//! `gizza-ai-charcode-codec-core`. The chat schema is derived from `descriptor()`
//! (single source — shared shape across chat + CLI); the handler delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    base: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    padding: String,
    /// Emit uppercase A–F hex digits when encoding (default true).
    #[serde(default = "default_uppercase")]
    uppercase: bool,
    #[serde(default)]
    format: String,
}

/// `Param::boolean("uppercase").default(true)` — serde's `bool` default is
/// `false`, so the advertised default has to be spelled out here too.
fn default_uppercase() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The text to convert into character codes, or the list of codes to turn back into text (e.g. '72 101 108 108 111')."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) turns text into character codes, 'decode' turns a list of codes back into text."),
        )
        .param(
            Param::enumv("base", ["dec", "hex", "bin", "oct"])
                .default("dec")
                .describe("Number base the codes are written in: 'dec' decimal (default, e.g. 72), 'hex' hexadecimal (48), 'bin' binary (1001000) or 'oct' octal (110)."),
        )
        .param(
            Param::enumv("scope", ["unicode-scalar", "utf8-bytes", "utf16-units", "ascii"])
                .default("unicode-scalar")
                .describe("What one code counts: 'unicode-scalar' (default) one code per Unicode code point, so 😀 is 128512; 'utf8-bytes' one code per UTF-8 byte (240 159 152 128); 'utf16-units' one code per UTF-16 code unit (55357 56832); 'ascii' one code per character, rejecting anything above 127."),
        )
        .param(
            Param::enumv("delimiter", ["space", "comma", "newline", "none"])
                .default("space")
                .describe("Separator placed between codes when encoding: 'space' (default), 'comma' (', '), 'newline', or 'none' for one unbroken run. Decoding ignores all of these regardless."),
        )
        .param(
            Param::enumv("prefix", ["none", "0x", "\\x", "U+"])
                .default("none")
                .describe("Marker placed before each code when encoding: 'none' (default), '0x' (0x48), '\\x' (\\x48) or 'U+' (U+0048). Decoding strips 0x, \\x, \\u, U+, 0b and 0o automatically."),
        )
        .param(
            Param::enumv("padding", ["none", "fixed"])
                .default("none")
                .describe("Zero-padding when encoding: 'none' (default) writes the shortest form, 'fixed' pads every code to the scope's natural width (ASCII decimal 072, UTF-8 binary 01001000, Unicode hex a 4-digit minimum like 0041). Ignored on decode."),
        )
        .param(
            Param::boolean("uppercase")
                .default(true)
                .describe("When true (default), hexadecimal codes use uppercase A-F, matching the U+1F600 convention; set false for lowercase (1f600). Decoding accepts either case."),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe("Output shape: 'text' (default) returns just the codes (or the decoded text); 'json' returns an object with mode, base, scope, count, the numeric codes array, and the result."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CharcodeCodec;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/charcode-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert text to a list of Unicode, UTF-8, UTF-16 or ASCII character codes in any base, and back.",
    skill(
        description = "Convert text into a list of numeric character codes and rebuild text from such a list. Use mode='encode' (default, e.g. 'Hello' -> '72 101 108 108 111') or mode='decode' (e.g. '72 101 108 108 111' -> 'Hello'). base picks the number base: 'dec' (default), 'hex', 'bin' or 'oct'. scope picks what one code counts: 'unicode-scalar' (default, one code per code point, so 😀 is 128512), 'utf8-bytes' (240 159 152 128), 'utf16-units' (55357 56832) or 'ascii' (rejects characters above 127). delimiter sets the separator when encoding ('space' default, 'comma', 'newline', 'none'); prefix adds '0x', '\\x' or 'U+' before each code; padding='fixed' zero-pads each code to the scope's natural width; uppercase controls hex digit case. Decoding is tolerant: it ignores whitespace and common separators, strips 0x/\\x/\\u/U+/0b/0o prefixes, and splits an unseparated run of fixed-width codes, so any encoded form round-trips. Out-of-range codes, surrogates, invalid UTF-8 and digits illegal in the chosen base are reported as errors. format='json' returns the numeric codes as an array alongside the result.",
        parameters = schema_json()
    ),
)]
impl CharcodeCodec {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "charcode-codec", |a: Args| {
            gizza_ai_charcode_codec_core::convert(
                &a.input,
                &a.mode,
                &a.base,
                &a.scope,
                &a.delimiter,
                &a.prefix,
                &a.padding,
                a.uppercase,
                &a.format,
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
                    "input": { "type": "string", "description": "The text to convert into character codes, or the list of codes to turn back into text (e.g. '72 101 108 108 111')." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) turns text into character codes, 'decode' turns a list of codes back into text." },
                    "base": { "type": "string", "enum": ["dec", "hex", "bin", "oct"], "default": "dec", "description": "Number base the codes are written in: 'dec' decimal (default, e.g. 72), 'hex' hexadecimal (48), 'bin' binary (1001000) or 'oct' octal (110)." },
                    "scope": { "type": "string", "enum": ["unicode-scalar", "utf8-bytes", "utf16-units", "ascii"], "default": "unicode-scalar", "description": "What one code counts: 'unicode-scalar' (default) one code per Unicode code point, so 😀 is 128512; 'utf8-bytes' one code per UTF-8 byte (240 159 152 128); 'utf16-units' one code per UTF-16 code unit (55357 56832); 'ascii' one code per character, rejecting anything above 127." },
                    "delimiter": { "type": "string", "enum": ["space", "comma", "newline", "none"], "default": "space", "description": "Separator placed between codes when encoding: 'space' (default), 'comma' (', '), 'newline', or 'none' for one unbroken run. Decoding ignores all of these regardless." },
                    "prefix": { "type": "string", "enum": ["none", "0x", "\\x", "U+"], "default": "none", "description": "Marker placed before each code when encoding: 'none' (default), '0x' (0x48), '\\x' (\\x48) or 'U+' (U+0048). Decoding strips 0x, \\x, \\u, U+, 0b and 0o automatically." },
                    "padding": { "type": "string", "enum": ["none", "fixed"], "default": "none", "description": "Zero-padding when encoding: 'none' (default) writes the shortest form, 'fixed' pads every code to the scope's natural width (ASCII decimal 072, UTF-8 binary 01001000, Unicode hex a 4-digit minimum like 0041). Ignored on decode." },
                    "uppercase": { "type": "boolean", "default": true, "description": "When true (default), hexadecimal codes use uppercase A-F, matching the U+1F600 convention; set false for lowercase (1f600). Decoding accepts either case." },
                    "format": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Output shape: 'text' (default) returns just the codes (or the decoded text); 'json' returns an object with mode, base, scope, count, the numeric codes array, and the result." }
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
