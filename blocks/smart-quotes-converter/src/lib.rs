//! gizza-ai/smart-quotes-converter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page query-params); handle() delegates to block_utils::run_skill. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_smart_quotes_converter_core::{convert, Direction, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    /// "to_curly" (default) straightens ASCII quotes into curly ones; "to_straight" reverses.
    #[serde(default = "default_direction")]
    direction: String,
    /// Convert double quotes (" ⇄ “ ”). Default true.
    #[serde(default = "default_true")]
    convert_double: bool,
    /// Convert single quotes / apostrophes (' ⇄ ‘ ’). Default true.
    #[serde(default = "default_true")]
    convert_single: bool,
    /// Feet/inches mode: digit-adjacent straight quotes become prime marks. Default false.
    #[serde(default)]
    feet_inches: bool,
}

fn default_direction() -> String {
    "to_curly".to_string()
}
fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The text whose quotation marks you want to convert."),
        )
        .param(
            Param::enumv("direction", ["to_curly", "to_straight"])
                .default("to_curly")
                .describe("Conversion direction. 'to_curly' (default) turns straight ASCII quotes (\" ') into curly typographic quotes (“ ” ‘ ’), choosing opening vs closing marks from the surrounding characters. 'to_straight' turns curly quotes (and prime marks) back into plain ASCII."),
        )
        .param(
            Param::boolean("convert_double")
                .default(true)
                .describe("Convert double quotes (\" ⇄ “ ”). Set false to leave double quotes untouched. Default true."),
        )
        .param(
            Param::boolean("convert_single")
                .default(true)
                .describe("Convert single quotes and apostrophes (' ⇄ ‘ ’). Set false to leave them untouched. Default true."),
        )
        .param(
            Param::boolean("feet_inches")
                .default(false)
                .describe("Feet/inches mode (to_curly only): a straight quote directly after a digit becomes a prime mark instead of a curly quote, so 6'4\" becomes 6′4″. to_straight always folds prime marks back to ASCII regardless of this flag. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SmartQuotesConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/smart-quotes-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert quotes between straight ASCII and curly typographic form.",
    skill(
        description = "Convert quotation marks between straight ASCII (\" ') and curly typographic (“ ” ‘ ’) forms. direction='to_curly' (default) educates straight quotes into curly ones, choosing opening vs closing marks from the surrounding characters (start/space/bracket -> opening; after a letter/digit -> closing or apostrophe, e.g. It's, dogs', '89); direction='to_straight' reverses, turning curly quotes and prime marks back into plain ASCII (useful before pasting into code, CSV, or JSON). convert_double / convert_single (both default true) toggle each quote kind independently. feet_inches=true (to_curly only) turns digit-adjacent straight quotes into prime marks (6'4\" -> 6′4″). Everything else — accents, CJK, emoji, dashes, ellipses — passes through untouched.",
        parameters = schema_json()
    ),
)]
impl SmartQuotesConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "smart-quotes-converter", |a: Args| {
            let opts = Options {
                direction: Direction::parse(&a.direction),
                convert_double: a.convert_double,
                convert_single: a.convert_single,
                feet_inches: a.feet_inches,
            };
            convert(&a.input, opts).map_err(SkillError::InvalidArgs)
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
    /// reviewed. (Regenerate this literal when the descriptor changes.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The text whose quotation marks you want to convert." },
                    "direction": { "type": "string", "enum": ["to_curly", "to_straight"], "default": "to_curly", "description": "Conversion direction. 'to_curly' (default) turns straight ASCII quotes (\" ') into curly typographic quotes (“ ” ‘ ’), choosing opening vs closing marks from the surrounding characters. 'to_straight' turns curly quotes (and prime marks) back into plain ASCII." },
                    "convert_double": { "type": "boolean", "default": true, "description": "Convert double quotes (\" ⇄ “ ”). Set false to leave double quotes untouched. Default true." },
                    "convert_single": { "type": "boolean", "default": true, "description": "Convert single quotes and apostrophes (' ⇄ ‘ ’). Set false to leave them untouched. Default true." },
                    "feet_inches": { "type": "boolean", "default": false, "description": "Feet/inches mode (to_curly only): a straight quote directly after a digit becomes a prime mark instead of a curly quote, so 6'4\" becomes 6′4″. to_straight always folds prime marks back to ASCII regardless of this flag. Default false." }
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
