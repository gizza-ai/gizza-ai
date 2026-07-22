//! gizza-ai/otpauth-migration-decoder — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    payload: String,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("payload")
                .required()
                .describe("Google Authenticator migration payload: paste the full otpauth-migration://offline?data=... URI or just the data value."),
        )
        .param(
            Param::enumv("format", ["uri", "json"])
                .default("uri")
                .describe("Output format: uri returns one otpauth:// URI per line; json returns a pretty-printed account array."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/otpauth-migration-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode Google Authenticator migration exports into otpauth URIs",
    skill(
        description = "Decode a Google Authenticator otpauth-migration://offline?data=... export payload into standard otpauth:// provisioning URIs, one per account. Accepts either the full migration URI or the bare data value. Set format=uri for newline-separated otpauth:// links, or format=json for account fields plus each rebuilt URI. Secrets are parsed locally from the protobuf payload and base32-encoded without padding; TOTP entries get period=30 and HOTP entries preserve their counter.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "otpauth-migration-decoder", |a: Args| {
            gizza_ai_otpauth_migration_decoder_core::run_with_format(
                &a.payload,
                a.format.as_deref().unwrap_or("uri"),
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "payload": { "type": "string", "description": "Google Authenticator migration payload: paste the full otpauth-migration://offline?data=... URI or just the data value." },
                    "format": { "type": "string", "enum": ["uri", "json"], "default": "uri", "description": "Output format: uri returns one otpauth:// URI per line; json returns a pretty-printed account array." }
                },
                "required": ["payload"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
