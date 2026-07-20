//! gizza-ai/secret-scanner — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
//! Static heuristic: flags hardcoded provider API keys/tokens by their prefix
//! shape, PEM private-key headers, JWT-shaped strings, and generic keyword+entropy
//! assignments. Never runs the code or contacts any provider.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default = "default_redact")]
    redact: bool,
    #[serde(default = "default_format")]
    format: String,
}

fn default_min_severity() -> String {
    "all".into()
}
fn default_redact() -> bool {
    true
}
fn default_format() -> String {
    "text".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The code, config, or text to scan for hardcoded secrets (one or more lines)."),
        )
        .param(
            Param::enumv("min_severity", ["all", "high"])
                .default("all")
                .describe(
                    "Which findings to report: all (default) shows high (named provider keys, \
                     private-key headers) plus medium (generic keyword+entropy assignments, \
                     JWT-shaped strings); high shows only the high-confidence findings.",
                ),
        )
        .param(
            Param::boolean("redact")
                .default(true)
                .describe(
                    "When true (default), matched secret values are masked in the output — only a \
                     short non-secret prefix is shown. Set false to reveal the full matched value.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe(
                    "Output format: text (default) a readable report, or json (structured findings).",
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
    name = "gizza-ai/secret-scanner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect hardcoded API keys, tokens, and private keys in code by provider pattern + entropy",
    skill(
        description = "Statically scan a code/config/text snippet for hardcoded secrets. Matches known provider prefixes — AWS access key IDs (AKIA…), GitHub tokens (ghp_…, github_pat_…), GitLab PATs (glpat-…), Slack tokens (xox…) and webhooks, Stripe keys (sk_live_…/sk_test_…), Google API keys (AIza…), OpenAI keys (sk-…), Twilio, SendGrid, npm, Shopify, and Square tokens — plus PEM private-key headers (-----BEGIN … PRIVATE KEY-----). It also flags JWT-shaped strings (medium) and generic keyword assignments (api_key/password/secret/token = \"…\") whose value has high Shannon entropy and isn't an obvious placeholder (medium). Each finding reports line, column, severity, a rule id, and the provider; matched values are redacted by default. Params: text (the snippet), min_severity (all|high), redact (bool, default true), format (text|json). It never runs the code, never contacts any provider, and never verifies whether a credential is live — a heuristic aid, not a guarantee. A clean result means nothing matched, not that the code is secret-free. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "secret-scanner", |a: Args| {
            gizza_ai_secret_scanner_core::run(&a.text, &a.min_severity, a.redact, &a.format)
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The code, config, or text to scan for hardcoded secrets (one or more lines)."
                    },
                    "min_severity": {
                        "type": "string",
                        "enum": ["all", "high"],
                        "default": "all",
                        "description": "Which findings to report: all (default) shows high (named provider keys, private-key headers) plus medium (generic keyword+entropy assignments, JWT-shaped strings); high shows only the high-confidence findings."
                    },
                    "redact": {
                        "type": "boolean",
                        "default": true,
                        "description": "When true (default), matched secret values are masked in the output — only a short non-secret prefix is shown. Set false to reveal the full matched value."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json"],
                        "default": "text",
                        "description": "Output format: text (default) a readable report, or json (structured findings)."
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
