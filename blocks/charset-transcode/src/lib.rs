//! gizza-ai/charset-transcode — re-decode text from a legacy charset into clean
//! UTF-8 (fix mojibake). Thin chat-skill wrapper around
//! `gizza-ai-charset-transcode-core`. The chat schema is single-sourced from
//! descriptor() (shared shape across chat + CLI); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    from: String,
    #[serde(default)]
    errors: String,
    /// Repair passes; the core clamps to 1..=8 (0 → 1).
    #[serde(default)]
    passes: u32,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The garbled / mis-decoded text to re-decode into clean UTF-8."),
        )
        .param(
            Param::string("from")
                .default("auto")
                .describe("Source charset the text was wrongly decoded AS, a WHATWG label (case-insensitive). Examples: 'windows-1252', 'iso-8859-1' (alias 'latin1'), 'iso-8859-15', 'windows-1251', 'koi8-r', 'shift_jis' (alias 'sjis'), 'euc-jp', 'euc-kr', 'gbk', 'big5'. Use 'auto' (default) to try the common charsets and keep the cleanest result."),
        )
        .param(
            Param::enumv("errors", ["replace", "strict"])
                .default("replace")
                .describe("How to handle bytes that aren't valid in 'from'. 'replace' (default) substitutes the Unicode replacement character (U+FFFD) and continues; 'strict' fails instead."),
        )
        .param(
            // Bounds reference the core clamp (MAX_PASSES) so the schema can't
            // drift from what `transcode` actually enforces.
            Param::integer("passes")
                .default(1)
                .min(1.0)
                .max(gizza_ai_charset_transcode_core::MAX_PASSES as f64)
                .describe("Apply the repair this many times, 1-8. Use >1 to un-nest double-encoded ('double mojibake') input. A pass that no longer changes the text stops early. Default 1."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CharsetTranscode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/charset-transcode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-decode garbled text from a legacy charset into clean UTF-8 (fix mojibake).",
    skill(
        description = "Re-decode garbled text ('mojibake') from a legacy charset into clean UTF-8. Leave 'from' as 'auto' (default) to detect the charset automatically, or set it to the charset the text was wrongly decoded as (WHATWG label, e.g. 'windows-1252', 'iso-8859-1'/'latin1', 'shift_jis'/'sjis', 'euc-jp', 'euc-kr', 'gbk', 'big5', 'windows-1251', 'koi8-r'). The tool re-encodes the input under that charset and decodes the bytes as UTF-8. Use this to fix text where 'é' shows as 'Ã©' or '“' shows as 'â€œ'. Set 'passes'>1 to un-nest double-encoded mojibake. The 'errors' option is 'replace' (default — undecodable bytes become U+FFFD) or 'strict' (fail instead).",
        parameters = schema_json()
    ),
)]
impl CharsetTranscode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "charset-transcode", |a: Args| {
            gizza_ai_charset_transcode_core::transcode(&a.text, &a.from, &a.errors, a.passes)
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
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The garbled / mis-decoded text to re-decode into clean UTF-8." },
                    "from": { "type": "string", "default": "auto", "description": "Source charset the text was wrongly decoded AS, a WHATWG label (case-insensitive). Examples: 'windows-1252', 'iso-8859-1' (alias 'latin1'), 'iso-8859-15', 'windows-1251', 'koi8-r', 'shift_jis' (alias 'sjis'), 'euc-jp', 'euc-kr', 'gbk', 'big5'. Use 'auto' (default) to try the common charsets and keep the cleanest result." },
                    "errors": { "type": "string", "enum": ["replace", "strict"], "default": "replace", "description": "How to handle bytes that aren't valid in 'from'. 'replace' (default) substitutes the Unicode replacement character (U+FFFD) and continues; 'strict' fails instead." },
                    "passes": { "type": "integer", "minimum": 1, "maximum": 8, "default": 1, "description": "Apply the repair this many times, 1-8. Use >1 to un-nest double-encoded ('double mojibake') input. A pass that no longer changes the text stops early. Default 1." }
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
