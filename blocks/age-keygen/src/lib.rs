//! gizza-ai/age-keygen — chat skill block on the shared tool abstraction.
//!
//! Generate an age X25519 identity and its shareable public recipient, or derive
//! the recipient from a pasted identity. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    comment: String,
    #[serde(default = "default_include_created")]
    include_created: bool,
    #[serde(default)]
    seed_or_identity: String,
}

fn default_format() -> String {
    "text".to_string()
}
fn default_include_created() -> bool {
    true
}

impl Args {
    fn run(&self) -> Result<String, String> {
        gizza_ai_age_keygen_core::run(
            &self.format,
            &self.comment,
            self.include_created,
            &self.seed_or_identity,
        )
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("format", ["text", "json", "recipient_only", "identity_only"])
                .default("text")
                .describe(
                    "Output shape. text writes an age-keygen-compatible identity file with comment \
                     lines; json returns recipient and identity fields; recipient_only is the \
                     public age1... string for sharing; identity_only prints only the secret \
                     AGE-SECRET-KEY-1... line.",
                ),
        )
        .param(Param::string("comment").describe(
            "Optional one-line label (maximum 200 characters) inserted as a # comment in text \
             output and as comment in JSON output. Newlines are folded to spaces.",
        ))
        .param(Param::boolean("include_created").default(true).describe(
            "Include an age-keygen-style # created: timestamp comment in text output. Turn off \
             for deterministic tests or when you want the shortest key file.",
        ))
        .param(Param::string("seed_or_identity").describe(
            "Leave empty to generate a fresh random identity with the platform CSPRNG. Paste an \
             existing AGE-SECRET-KEY-1... identity to derive its public recipient or to produce \
             deterministic test output. Raw hex seeds are not accepted.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/age-keygen",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an age identity and public recipient",
    skill(
        description = "Generate an age X25519 identity locally and return both the secret AGE-SECRET-KEY-1... identity and its shareable age1... public recipient. The default text output matches the age-keygen identity-file shape: optional # created timestamp, optional user comment, # public key, then the secret key. Choose json for machine-readable output, recipient_only for the public key only, or identity_only for the secret line only. Leave seed_or_identity empty for a fresh random key from the browser/OS CSPRNG, or paste an existing AGE-SECRET-KEY-1... identity to derive its recipient like age-keygen -y. This is pure local Rust/WASM: no upload, no network, no filesystem writes, no post-quantum hybrid identities, no vanity-prefix brute force.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "age-keygen", |a: Args| {
            a.run().map_err(SkillError::InvalidArgs)
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
                "type":"object",
                "properties":{
                    "format":{"type":"string","enum":["text","json","recipient_only","identity_only"],"default":"text","description":"Output shape. text writes an age-keygen-compatible identity file with comment lines; json returns recipient and identity fields; recipient_only is the public age1... string for sharing; identity_only prints only the secret AGE-SECRET-KEY-1... line."},
                    "comment":{"type":"string","description":"Optional one-line label (maximum 200 characters) inserted as a # comment in text output and as comment in JSON output. Newlines are folded to spaces."},
                    "include_created":{"type":"boolean","default":true,"description":"Include an age-keygen-style # created: timestamp comment in text output. Turn off for deterministic tests or when you want the shortest key file."},
                    "seed_or_identity":{"type":"string","description":"Leave empty to generate a fresh random identity with the platform CSPRNG. Paste an existing AGE-SECRET-KEY-1... identity to derive its public recipient or to produce deterministic test output. Raw hex seeds are not accepted."}
                },
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_layer_derives_known_recipient() {
        let a = Args {
            format: "recipient_only".into(),
            comment: String::new(),
            include_created: false,
            seed_or_identity: gizza_ai_age_keygen_core::TEST_IDENTITY.into(),
        };
        assert_eq!(a.run().unwrap(), gizza_ai_age_keygen_core::TEST_RECIPIENT);
    }

    #[test]
    fn args_layer_reports_bad_format() {
        let a = Args {
            format: "yaml".into(),
            comment: String::new(),
            include_created: false,
            seed_or_identity: String::new(),
        };
        assert!(a.run().unwrap_err().contains("unknown format"));
    }
}
