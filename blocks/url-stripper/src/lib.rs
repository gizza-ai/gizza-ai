//! gizza-ai/url-stripper — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page query-params); handle() delegates to block_utils::run_skill. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_url_stripper_core::{render, strip, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The text to strip URLs (and optionally emails) from.
    input: String,
    /// Also remove bare email addresses. Default false.
    #[serde(default)]
    remove_emails: bool,
    /// Also remove scheme-less `www.` links. Default true.
    #[serde(default = "default_true")]
    remove_www: bool,
    /// Text to put in place of each removed link. Empty = delete. Default empty.
    #[serde(default)]
    replacement: String,
    /// Tidy the spacing left behind so the result reads as clean prose. Default true.
    #[serde(default = "default_true")]
    collapse_whitespace: bool,
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
                .describe("The text to strip URLs (and optionally email addresses) from."),
        )
        .param(
            Param::boolean("remove_emails")
                .default(false)
                .describe("Also remove bare email addresses (name@example.com). Default false."),
        )
        .param(
            Param::boolean("remove_www")
                .default(true)
                .describe("Also remove scheme-less links that begin with www. (e.g. www.example.com), not just http/https/ftp URLs. Default true."),
        )
        .param(
            Param::string("replacement")
                .default("")
                .describe("Text to put in place of each removed URL/email. Leave empty to delete the link entirely; use e.g. [link] to keep a placeholder. Default empty."),
        )
        .param(
            Param::boolean("collapse_whitespace")
                .default(true)
                .describe("Tidy the whitespace left behind after deleting links — collapse double spaces, drop spaces before punctuation, remove now-empty brackets, and trim each line — so the result reads as clean prose. Newlines and blank lines are kept. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct UrlStripper;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/url-stripper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove URLs and optionally emails from text, leaving clean prose.",
    skill(
        description = "Strip web links out of a block of text. Removes http/https/ftp URLs and, by default (remove_www=true), scheme-less www. links; set remove_emails=true to also remove bare email addresses. Each match is replaced with `replacement` (empty by default = delete it, or use e.g. [link] to keep a placeholder). By default (collapse_whitespace=true) the leftover spacing is tidied — double spaces collapsed, spaces before punctuation dropped, now-empty brackets removed, each line trimmed — while newlines and blank lines are preserved, so the output reads as clean prose. Trailing sentence punctuation stuck to a URL (the period in `See https://x.com/y.`) is kept. Returns the cleaned text plus counts of URLs and emails removed. Runs entirely in the sandbox; nothing is fetched.",
        parameters = schema_json()
    ),
)]
impl UrlStripper {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. strip() returns
        // { text, urls_removed, emails_removed }; render() is the page-facing
        // text-only variant (used by the web wasm export).
        let _ = render;
        match run_skill(&body, "url-stripper", |a: Args| {
            let opts = Options {
                remove_emails: a.remove_emails,
                remove_www: a.remove_www,
                replacement: a.replacement,
                collapse_whitespace: a.collapse_whitespace,
            };
            Ok(strip(&a.input, &opts))
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
                    "input": { "type": "string", "description": "The text to strip URLs (and optionally email addresses) from." },
                    "remove_emails": { "type": "boolean", "default": false, "description": "Also remove bare email addresses (name@example.com). Default false." },
                    "remove_www": { "type": "boolean", "default": true, "description": "Also remove scheme-less links that begin with www. (e.g. www.example.com), not just http/https/ftp URLs. Default true." },
                    "replacement": { "type": "string", "default": "", "description": "Text to put in place of each removed URL/email. Leave empty to delete the link entirely; use e.g. [link] to keep a placeholder. Default empty." },
                    "collapse_whitespace": { "type": "boolean", "default": true, "description": "Tidy the whitespace left behind after deleting links — collapse double spaces, drop spaces before punctuation, remove now-empty brackets, and trim each line — so the result reads as clean prose. Newlines and blank lines are kept. Default true." }
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
