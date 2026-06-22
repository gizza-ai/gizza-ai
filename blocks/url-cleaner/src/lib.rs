//! gizza-ai/url-cleaner — strip tracking/analytics params from URLs.
//!
//! Thin chat-skill wrapper around `gizza-ai-url-cleaner-core`. The chat schema is
//! derived from `descriptor()` (single source — shared across chat + CLI + page
//! query-params); the handler delegates to `block_utils::run_skill`. No host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_url_cleaner_core::clean;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default)]
    per_line: bool,
    #[serde(default)]
    extra: String,
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("url")
                .required()
                .describe("The URL to clean (or a batch list of URLs, one per line with per_line=true)."),
        )
        .param(
            Param::boolean("per_line")
                .default(false)
                .describe("When true, clean each line of the input independently (rejoined with newlines) — for a batch list of URLs. Default false."),
        )
        .param(
            Param::string("extra")
                .default("")
                .describe("Additional query-parameter names to strip, comma-separated (on top of the built-in tracking list). Default none."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct UrlCleaner;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/url-cleaner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip tracking parameters from a URL",
    skill(
        description = "Remove tracking and analytics query parameters from a URL to produce a clean, shareable link. Strips utm_* and other analytics prefixes (pk_, mtm_, ga_, hsa_, mc_, ...) plus known click ids (fbclid, gclid, msclkid, igshid, yclid, ttclid, ...), while preserving the scheme, host, path, fragment, and all other query params in their original order and encoding. Set per_line=true to clean a batch list of URLs (one per line). Pass extra='name1,name2' to also strip custom parameter names.",
        parameters = schema_json()
    )
)]
impl UrlCleaner {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "url-cleaner", |a: Args| {
            clean(&a.url, a.per_line, &a.extra).map_err(SkillError::InvalidArgs)
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
                    "url": { "type": "string", "description": "The URL to clean (or a batch list of URLs, one per line with per_line=true)." },
                    "per_line": { "type": "boolean", "default": false, "description": "When true, clean each line of the input independently (rejoined with newlines) — for a batch list of URLs. Default false." },
                    "extra": { "type": "string", "default": "", "description": "Additional query-parameter names to strip, comma-separated (on top of the built-in tracking list). Default none." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
