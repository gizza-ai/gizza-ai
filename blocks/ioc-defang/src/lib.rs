//! gizza-ai/ioc-defang — defang or refang IOCs (chat skill block).
//!
//! Thin chat-skill wrapper around `gizza-ai-ioc-defang-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    style: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text containing IOCs (URLs, IPs, domains, emails) to transform."),
        )
        .param(
            Param::enumv("mode", ["defang", "refang"])
                .default("defang")
                .describe(
                    "What to do. 'defang' (default) neutralizes IOCs so they are not clickable \
or auto-linked: 'http'/'https'/'ftp' -> 'hxxp'/'hxxps'/'fxp', every '.' -> '[.]', '@' -> '[at]', \
and '://' -> '[://]'. 'refang' is the inverse — restore a defanged blob to the real, clickable \
indicator (recognizes [], (), {} and [dot]/[at]/meow:// variants).",
                ),
        )
        .param(
            Param::enumv("style", ["square", "round", "curly", "dot"])
                .default("square")
                .describe(
                    "Bracket style used when defanging: 'square' (default) -> '[.]' '[at]' '[://]'; \
'round' -> '(.)'; 'curly' -> '{.}'; 'dot' spells the separator -> '[dot]' '[at]'. Ignored when \
mode is 'refang' (all styles are recognized on the way back).",
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
    name = "gizza-ai/ioc-defang",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Defang or refang IOCs (URLs, IPs, domains, emails).",
    skill(
        description = "Defang or refang indicators of compromise (IOCs) in a block of text. Set mode to 'defang' (default) to neutralize URLs, IP addresses, domains and email addresses so they are safe to share in a report, ticket or email and won't auto-link or auto-execute — it rewrites 'http'/'https'/'ftp' to 'hxxp'/'hxxps'/'fxp', every '.' to '[.]', '@' to '[at]', and '://' to '[://]'. Set mode to 'refang' to do the inverse and restore a defanged blob back to the real, clickable indicator (it recognizes square [], round (), curly {} brackets plus the [dot]/[at] and meow:// conventions). The 'style' parameter picks the bracket style for defanging: 'square' (default, [.]), 'round' ((.)), 'curly' ({.}), or 'dot' (spelled out, [dot]). Surrounding prose is preserved; only the indicator characters are rewritten.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ioc-defang", |a: Args| {
            gizza_ai_ioc_defang_core::defang(&a.text, &a.mode, &a.style)
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
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text containing IOCs (URLs, IPs, domains, emails) to transform." },
                    "mode": {
                        "type": "string",
                        "enum": ["defang", "refang"],
                        "default": "defang",
                        "description": "What to do. 'defang' (default) neutralizes IOCs so they are not clickable or auto-linked: 'http'/'https'/'ftp' -> 'hxxp'/'hxxps'/'fxp', every '.' -> '[.]', '@' -> '[at]', and '://' -> '[://]'. 'refang' is the inverse — restore a defanged blob to the real, clickable indicator (recognizes [], (), {} and [dot]/[at]/meow:// variants)."
                    },
                    "style": {
                        "type": "string",
                        "enum": ["square", "round", "curly", "dot"],
                        "default": "square",
                        "description": "Bracket style used when defanging: 'square' (default) -> '[.]' '[at]' '[://]'; 'round' -> '(.)'; 'curly' -> '{.}'; 'dot' spells the separator -> '[dot]' '[at]'. Ignored when mode is 'refang' (all styles are recognized on the way back)."
                    }
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
