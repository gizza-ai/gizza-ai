//! gizza-ai/tabs-to-spaces — convert tabs to spaces or spaces to tabs, respecting
//! tab stops. Thin chat-skill wrapper around `gizza-ai-tabs-to-spaces-core`. The
//! chat schema is single-sourced from `descriptor()` (which also drives the CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    /// "expand" (tabs → spaces, default) or "unexpand" (spaces → tabs).
    #[serde(default)]
    direction: String,
    /// Columns per tab stop; the core clamps to 1..=16 (0 → 1). Default 4.
    #[serde(default = "default_tab_width")]
    tab_width: u32,
    /// "all" (leading + inline, default) or "leading" (indentation only).
    #[serde(default)]
    scope: String,
}

fn default_tab_width() -> u32 {
    4
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text or code to convert. Each line is handled independently and line breaks are preserved."),
        )
        .param(
            Param::enumv("direction", ["expand", "unexpand"])
                .default("expand")
                .describe("Direction: 'expand' (default) turns tabs into spaces; 'unexpand' turns runs of spaces back into tabs."),
        )
        .param(
            // Bounds reference the core clamp so the LLM-facing schema can't drift
            // from what `convert` enforces.
            Param::integer("tab_width")
                .default(4)
                .min(gizza_ai_tabs_to_spaces_core::MIN_TAB_WIDTH as f64)
                .max(gizza_ai_tabs_to_spaces_core::MAX_TAB_WIDTH as f64)
                .describe("Columns per tab stop, 1-16 (default 4). A tab advances to the next multiple of this width — e.g. at width 4 a tab after 'ab' fills only 2 spaces to reach column 4. Common values: 2, 4, 8 (the Unix default)."),
        )
        .param(
            Param::enumv("scope", ["all", "leading"])
                .default("all")
                .describe("Which whitespace to convert. 'all' (default) converts leading indentation and inline whitespace; 'leading' converts only the indentation before the first non-blank character on each line, leaving the rest untouched."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TabsToSpaces;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/tabs-to-spaces",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert tabs to spaces or spaces to tabs, respecting tab stops.",
    skill(
        description = "Convert tabs to spaces or spaces back to tabs, respecting tab stops. Use direction='expand' (default) to turn tabs into spaces, or direction='unexpand' to turn runs of spaces into tabs. tab_width (1-16, default 4) sets the columns per tab stop: a tab advances to the next multiple of this width, so a tab after 'ab' fills only 2 spaces to reach column 4 — not a fixed count. scope='all' (default) converts leading indentation and inline whitespace; scope='leading' converts only the indentation before the first non-blank character on each line. Each line is processed independently and line breaks are preserved.",
        parameters = schema_json()
    ),
)]
impl TabsToSpaces {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "tabs-to-spaces", |a: Args| {
            gizza_ai_tabs_to_spaces_core::convert(&a.text, &a.direction, a.tab_width, &a.scope)
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
                    "text": { "type": "string", "description": "The text or code to convert. Each line is handled independently and line breaks are preserved." },
                    "direction": { "type": "string", "enum": ["expand", "unexpand"], "default": "expand", "description": "Direction: 'expand' (default) turns tabs into spaces; 'unexpand' turns runs of spaces back into tabs." },
                    "tab_width": { "type": "integer", "minimum": 1, "maximum": 16, "default": 4, "description": "Columns per tab stop, 1-16 (default 4). A tab advances to the next multiple of this width — e.g. at width 4 a tab after 'ab' fills only 2 spaces to reach column 4. Common values: 2, 4, 8 (the Unix default)." },
                    "scope": { "type": "string", "enum": ["all", "leading"], "default": "all", "description": "Which whitespace to convert. 'all' (default) converts leading indentation and inline whitespace; 'leading' converts only the indentation before the first non-blank character on each line, leaving the rest untouched." }
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
