//! gizza-ai/color-contrast-checker — chat skill block on the shared tool abstraction.
//! Computes the WCAG 2.x contrast ratio between a foreground and a background
//! colour and reports AA/AAA pass/fail (and an optional accessible-colour
//! suggestion). The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    foreground: String,
    background: String,
    /// Output shape: "text" (default), "json", or "suggest".
    #[serde(default)]
    format: String,
    /// Target level for 'suggest' mode: "aa" (default), "aaa", or "large".
    #[serde(default)]
    target: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("foreground")
                .required()
                .describe("Foreground / text colour. Accepts a hex code (#1a2b3c, #fff, 1a2b3c), an rgb triple (rgb(26,43,60) or 26,43,60), an hsl triple (hsl(210,40%,17%)), or a CSS colour name (navy, tomato, rebeccapurple)."),
        )
        .param(
            Param::string("background")
                .required()
                .describe("Background colour. Accepts a hex code (#ffffff, #fff), an rgb triple (rgb(255,255,255)), an hsl triple (hsl(0,0%,100%)), or a CSS colour name (white, gainsboro)."),
        )
        .param(
            Param::enumv("format", ["text", "json", "suggest"])
                .default("text")
                .describe("Output format: 'text' (default) is a human-readable WCAG report; 'json' is a machine-readable object with the ratio and each AA/AAA pass flag; 'suggest' adds the nearest accessible foreground (same hue, lightness nudged) that meets the chosen target."),
        )
        .param(
            Param::enumv("target", ["aa", "aaa", "large"])
                .default("aa")
                .describe("Used only by format='suggest': which threshold to reach — 'aa' (4.5:1 normal text, default), 'aaa' (7:1), or 'large' (3:1, large text / UI components)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/color-contrast-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Color Contrast Checker skill",
    skill(
        description = "Compute the WCAG 2.x contrast ratio between a foreground (text) colour and a background colour and report whether it passes WCAG AA and AAA for normal text (>= 4.5 / >= 7), large text (>= 3 / >= 4.5), and UI components/graphics (>= 3). Each colour is a hex code (#1a2b3c, #fff, bare 1a2b3c), an rgb triple (rgb(26,43,60)), an hsl triple (hsl(210,40%,17%)), or a CSS colour name (navy, tomato). Set format='json' for a machine-readable result; set format='suggest' (with target='aa'|'aaa'|'large') to also get the nearest accessible foreground that passes.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "color-contrast-checker", |a: Args| {
            gizza_ai_color_contrast_checker_core::run(
                &a.foreground,
                &a.background,
                &a.format,
                &a.target,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    /// Regenerated when format gained 'suggest' and the 'target' param was added.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "foreground": { "type": "string", "description": "Foreground / text colour. Accepts a hex code (#1a2b3c, #fff, 1a2b3c), an rgb triple (rgb(26,43,60) or 26,43,60), an hsl triple (hsl(210,40%,17%)), or a CSS colour name (navy, tomato, rebeccapurple)." },
                    "background": { "type": "string", "description": "Background colour. Accepts a hex code (#ffffff, #fff), an rgb triple (rgb(255,255,255)), an hsl triple (hsl(0,0%,100%)), or a CSS colour name (white, gainsboro)." },
                    "format": { "type": "string", "enum": ["text", "json", "suggest"], "default": "text", "description": "Output format: 'text' (default) is a human-readable WCAG report; 'json' is a machine-readable object with the ratio and each AA/AAA pass flag; 'suggest' adds the nearest accessible foreground (same hue, lightness nudged) that meets the chosen target." },
                    "target": { "type": "string", "enum": ["aa", "aaa", "large"], "default": "aa", "description": "Used only by format='suggest': which threshold to reach — 'aa' (4.5:1 normal text, default), 'aaa' (7:1), or 'large' (3:1, large text / UI components)." }
                },
                "required": ["foreground", "background"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
