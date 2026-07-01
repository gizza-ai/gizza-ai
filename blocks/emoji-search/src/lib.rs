//! gizza-ai/emoji-search — search a curated emoji set by keyword, shortcode, or
//! category. Thin chat-skill wrapper around `gizza-ai-emoji-search-core`. The
//! chat schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox against an embedded dataset.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    query: String,
    /// Max number of matches to return; core clamps to 1..=100 (0 → 20).
    #[serde(default)]
    limit: u32,
    /// When true, return only the bare glyphs (space-separated) instead of the
    /// labelled `glyph :shortcode: name (category)` lines.
    #[serde(default)]
    glyphs_only: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("query")
                .required()
                .describe("What to search for: a keyword (e.g. 'happy', 'rocket'), a :shortcode: (e.g. ':smile:'), or a category name (smileys, people, animals, food, activities, symbols, flags) to list that whole category."),
        )
        .param(
            Param::integer("limit")
                .default(gizza_ai_emoji_search_core::DEFAULT_LIMIT as i64)
                .min(1.0)
                .max(gizza_ai_emoji_search_core::MAX_LIMIT as f64)
                .describe("Maximum number of matching emoji to return, 1-100. Default 20."),
        )
        .param(
            Param::boolean("glyphs_only")
                .default(false)
                .describe("When true, return only the bare emoji glyphs (space-separated). When false (default), each match is a line: 'glyph :shortcode: name (category)'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EmojiSearch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/emoji-search",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Search emoji by keyword, shortcode, or category.",
    skill(
        description = "Search a curated set of common emoji by keyword, :shortcode:, or category and return the matching glyphs. The query matches each emoji's name, shortcode, keywords, and category (case-insensitively; surrounding colons in a :shortcode: are ignored). Search by meaning ('happy', 'love', 'celebrate', 'fire'), by shortcode (':rocket:' -> 🚀), or by a category name (smileys, people, animals, food, activities, symbols, flags) to list that whole category. Use limit (1-100, default 20) to bound the result count, and glyphs_only=true to get just the bare emoji. Results are ranked best-match-first. Fully offline — no network.",
        parameters = schema_json()
    )
)]
impl EmojiSearch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "emoji-search", |a: Args| {
            if a.glyphs_only {
                gizza_ai_emoji_search_core::glyphs(&a.query, a.limit)
            } else {
                gizza_ai_emoji_search_core::run(&a.query, a.limit)
            }
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
                    "query": { "type": "string", "description": "What to search for: a keyword (e.g. 'happy', 'rocket'), a :shortcode: (e.g. ':smile:'), or a category name (smileys, people, animals, food, activities, symbols, flags) to list that whole category." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20, "description": "Maximum number of matching emoji to return, 1-100. Default 20." },
                    "glyphs_only": { "type": "boolean", "default": false, "description": "When true, return only the bare emoji glyphs (space-separated). When false (default), each match is a line: 'glyph :shortcode: name (category)'." }
                },
                "required": ["query"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
