//! gizza-ai/extract-domains — extract hostnames and registrable domains (eTLD+1)
//! from text, URLs, and emails; validated against the Public Suffix List and
//! deduplicated. Chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_extract_domains_core::Mode;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    sort: bool,
}

fn default_mode() -> String {
    "both".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan for domains (plain text, URLs, and email addresses are all supported)."),
        )
        .param(
            Param::enumv("mode", ["hostname", "registrable", "both"])
                .default("both")
                .describe("Which domains to return: full hostnames, registrable domains (eTLD+1, e.g. example.co.uk), or both."),
        )
        .param(
            Param::boolean("sort")
                .default(false)
                .describe("When true, return the domains in alphabetical order instead of first-seen order."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ExtractDomains;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/extract-domains",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract hostnames and registrable domains from text",
    skill(
        description = "Scan text and extract every domain it references — from plain text, http(s)/ftp URLs, and email addresses. Each candidate is validated against the Public Suffix List, so IP addresses, version numbers, and bogus TLDs are dropped. Results are deduplicated in first-seen order. mode=hostname returns full hostnames (www.blog.example.co.uk), mode=registrable returns the registrable domain / eTLD+1 (example.co.uk), mode=both (default) returns both lists with counts. Set sort=true to order the results alphabetically instead of first-seen. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ExtractDomains {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "extract-domains", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            Ok::<_, SkillError>(gizza_ai_extract_domains_core::extract_opts(&a.text, mode, a.sort))
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
                    "text": { "type": "string", "description": "The text to scan for domains (plain text, URLs, and email addresses are all supported)." },
                    "mode": { "type": "string", "enum": ["hostname", "registrable", "both"], "default": "both", "description": "Which domains to return: full hostnames, registrable domains (eTLD+1, e.g. example.co.uk), or both." },
                    "sort": { "type": "boolean", "default": false, "description": "When true, return the domains in alphabetical order instead of first-seen order." }
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
