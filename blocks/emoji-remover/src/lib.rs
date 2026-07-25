//! gizza-ai/emoji-remover — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_emoji_remover_core::{remove_emoji, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    placeholder: String,
    #[serde(default)]
    collapse_whitespace: bool,
    #[serde(default)]
    keep_text_symbols: bool,
}
fn default_mode() -> String {
    "remove".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to strip emoji and pictographic symbols from."),
        )
        .param(
            Param::enumv("mode", ["remove", "space", "placeholder"])
                .default("remove")
                .describe(
                    "What to leave in place of each removed emoji. 'remove' (default) deletes it; 'space' leaves a single space; 'placeholder' inserts the `placeholder` string.",
                ),
        )
        .param(
            Param::string("placeholder").default("").describe(
                "Text inserted for each removed emoji when mode is 'placeholder' (e.g. '[emoji]'). Ignored for the other modes.",
            ),
        )
        .param(
            Param::boolean("collapse_whitespace").default(false).describe(
                "When true, collapse runs of whitespace (including gaps left where an emoji was deleted) into a single space, preserve paragraph breaks as one newline, and trim the ends. Default false.",
            ),
        )
        .param(
            Param::boolean("keep_text_symbols").default(false).describe(
                "When true, keep pictographic symbols that default to text presentation (©, ®, ™, and hearts/arrows without an emoji variation selector) instead of removing them. Emoji-styled symbols (with VS16), flags, keycaps, and skin-toned/ZWJ emoji are still removed. Default false.",
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
    name = "gizza-ai/emoji-remover",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip emoji and pictographic symbols from text",
    skill(
        description = "Remove emoji and pictographic symbols from text. Detection is per Unicode grapheme cluster, so ZWJ families (👨‍👩‍👧‍👦), regional-indicator flags (🇬🇧), skin-tone modifiers (👍🏽), keycaps (1️⃣), and variation selectors come out cleanly and whole. `mode` chooses what to leave behind (remove / space / placeholder + `placeholder` text); `collapse_whitespace` tidies the gaps; `keep_text_symbols` preserves text-default symbols like © ® ™ and un-styled hearts. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "emoji-remover", |a: Args| {
            let mode = Mode::parse(&a.mode).ok_or_else(|| {
                SkillError::InvalidArgs(format!(
                    "expected mode to be 'remove', 'space', or 'placeholder', got '{}'",
                    a.mode
                ))
            })?;
            Ok(remove_emoji(
                &a.text,
                mode,
                &a.placeholder,
                a.collapse_whitespace,
                a.keep_text_symbols,
            ))
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
                    "text": { "type": "string", "description": "The text to strip emoji and pictographic symbols from." },
                    "mode": { "type": "string", "enum": ["remove", "space", "placeholder"], "default": "remove", "description": "What to leave in place of each removed emoji. 'remove' (default) deletes it; 'space' leaves a single space; 'placeholder' inserts the `placeholder` string." },
                    "placeholder": { "type": "string", "default": "", "description": "Text inserted for each removed emoji when mode is 'placeholder' (e.g. '[emoji]'). Ignored for the other modes." },
                    "collapse_whitespace": { "type": "boolean", "default": false, "description": "When true, collapse runs of whitespace (including gaps left where an emoji was deleted) into a single space, preserve paragraph breaks as one newline, and trim the ends. Default false." },
                    "keep_text_symbols": { "type": "boolean", "default": false, "description": "When true, keep pictographic symbols that default to text presentation (©, ®, ™, and hearts/arrows without an emoji variation selector) instead of removing them. Emoji-styled symbols (with VS16), flags, keycaps, and skin-toned/ZWJ emoji are still removed. Default false." }
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
