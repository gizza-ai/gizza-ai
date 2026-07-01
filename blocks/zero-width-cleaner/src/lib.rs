//! gizza-ai/zero-width-cleaner — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, ToolDescriptor};
use gizza_ai_zero_width_cleaner_core::clean;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_true")]
    remove_zero_width: bool,
    #[serde(default = "default_true")]
    remove_bidi: bool,
    #[serde(default = "default_true")]
    remove_soft_hyphen: bool,
    #[serde(default)]
    replace_nbsp: bool,
    #[serde(default)]
    replacement: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to strip invisible zero-width and formatting characters from."),
        )
        .param(
            Param::boolean("remove_zero_width").default(true).describe(
                "When true (default), remove zero-width characters: zero-width space (U+200B), non-joiner (U+200C), joiner (U+200D), word joiner (U+2060), invisible math operators (U+2061–U+2064), Mongolian vowel separator (U+180E) and the byte-order mark / ZWNBSP (U+FEFF).",
            ),
        )
        .param(
            Param::boolean("remove_bidi").default(true).describe(
                "When true (default), remove invisible bidirectional formatting controls (U+061C, U+200E/U+200F, U+202A–U+202E, U+2066–U+2069) that can visually reorder text.",
            ),
        )
        .param(
            Param::boolean("remove_soft_hyphen").default(true).describe(
                "When true (default), remove soft hyphens (U+00AD), the invisible optional line-break hint.",
            ),
        )
        .param(
            Param::boolean("replace_nbsp").default(false).describe(
                "When true, replace non-breaking and other unusual Unicode spaces (U+00A0, U+2000–U+200A, U+202F, U+205F, U+3000, U+1680) with a normal ASCII space. Off by default.",
            ),
        )
        .param(
            Param::string("replacement").default("").describe(
                "String substituted for each removed zero-width, bidi or soft-hyphen character. Defaults to an empty string, which deletes the character.",
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
    name = "gizza-ai/zero-width-cleaner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect and strip invisible zero-width spaces, joiners, BOMs and other non-printing characters from text",
    skill(
        description = "Remove invisible characters from text: zero-width spaces/joiners, the word joiner and invisible math operators, the byte-order mark (BOM), bidirectional formatting controls, and soft hyphens. remove_zero_width, remove_bidi and remove_soft_hyphen default true; replace_nbsp (default false) turns non-breaking and other odd Unicode spaces into a normal space; replacement is substituted for each removed character (empty string by default, which deletes it). Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "zero-width-cleaner", |a: Args| {
            Ok(clean(
                &a.text,
                a.remove_zero_width,
                a.remove_bidi,
                a.remove_soft_hyphen,
                a.replace_nbsp,
                &a.replacement,
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
                    "text": { "type": "string", "description": "The text to strip invisible zero-width and formatting characters from." },
                    "remove_zero_width": { "type": "boolean", "default": true, "description": "When true (default), remove zero-width characters: zero-width space (U+200B), non-joiner (U+200C), joiner (U+200D), word joiner (U+2060), invisible math operators (U+2061–U+2064), Mongolian vowel separator (U+180E) and the byte-order mark / ZWNBSP (U+FEFF)." },
                    "remove_bidi": { "type": "boolean", "default": true, "description": "When true (default), remove invisible bidirectional formatting controls (U+061C, U+200E/U+200F, U+202A–U+202E, U+2066–U+2069) that can visually reorder text." },
                    "remove_soft_hyphen": { "type": "boolean", "default": true, "description": "When true (default), remove soft hyphens (U+00AD), the invisible optional line-break hint." },
                    "replace_nbsp": { "type": "boolean", "default": false, "description": "When true, replace non-breaking and other unusual Unicode spaces (U+00A0, U+2000–U+200A, U+202F, U+205F, U+3000, U+1680) with a normal ASCII space. Off by default." },
                    "replacement": { "type": "string", "default": "", "description": "String substituted for each removed zero-width, bidi or soft-hyphen character. Defaults to an empty string, which deletes the character." }
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
