//! gizza-ai/css-autoprefixer — adds vendor prefixes (-webkit-, -moz-, -ms-, -o-) to CSS.
//!
//! Thin chat-skill wrapper around `gizza-ai-css-autoprefixer-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely inside
//! the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    css: String,
    /// Skip emitting a prefixed declaration that is already present in the same rule
    /// body (default true).
    #[serde(default = "default_true")]
    dedup: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("css")
                .required()
                .describe("The CSS source to add vendor prefixes to. Pass one or more rules (selectors + { … } bodies); declarations inside comments and strings are left untouched."),
        )
        .param(
            Param::boolean("dedup")
                .default(true)
                .describe("When true (default), do not emit a prefixed declaration that already appears verbatim in the same rule body, so re-running the tool is idempotent."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CssAutoprefixer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/css-autoprefixer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "CSS Autoprefixer skill",
    skill(
        description = "Add vendor prefixes (-webkit-, -moz-, -ms-, -o-) to CSS for the properties and values that still need them in modern browsers. Pass the CSS in `css`; the tool inserts the needed prefixed clones immediately before each original declaration (standard form last, so a fully-supporting browser wins). It covers property prefixes (e.g. user-select, appearance, backdrop-filter, clip-path, mask, hyphens, text-size-adjust, background-clip:text), property renames (tab-size -> -moz-/-o-tab-size), and value prefixes (display:flex -> -webkit-box/-webkit-flex/-ms-flexbox, position:sticky -> -webkit-sticky, width:max-content -> -webkit-/-moz-). Declarations inside /* comments */ and strings, and CSS custom properties (--vars), are left untouched. Set dedup=false to also emit a prefix that already appears in the rule. The prefix set is curated for current browser targets, not an exhaustive historical list.",
        parameters = schema_json()
    ),
)]
impl CssAutoprefixer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "css-autoprefixer", |a: Args| {
            gizza_ai_css_autoprefixer_core::prefix(&a.css, a.dedup).map_err(SkillError::InvalidArgs)
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
    /// schema, so any future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "css": { "type": "string", "description": "The CSS source to add vendor prefixes to. Pass one or more rules (selectors + { … } bodies); declarations inside comments and strings are left untouched." },
                    "dedup": { "type": "boolean", "default": true, "description": "When true (default), do not emit a prefixed declaration that already appears verbatim in the same rule body, so re-running the tool is idempotent." }
                },
                "required": ["css"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
