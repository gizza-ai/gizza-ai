//! gizza-ai/flask-session-decode — chat skill block on the shared tool abstraction.
//! Decodes a Flask / itsdangerous session cookie into readable JSON WITHOUT the
//! secret key. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    cookie: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("cookie")
            .required()
            .describe("The Flask session cookie value to decode. Paste the bare value or a full 'session=...' fragment (a leading name=, surrounding quotes, and trailing '; Path=/; HttpOnly' attributes are stripped automatically). Format is the itsdangerous signed 'payload.timestamp.signature'."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/flask-session-decode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a Flask session cookie into readable JSON (no secret key)",
    skill(
        description = "Decode a Flask / itsdangerous session cookie into readable JSON WITHOUT the SECRET_KEY. Flask's default session uses itsdangerous URLSafeTimedSerializer, whose cookie value is three base64url segments 'payload.timestamp.signature'. This base64url-decodes the payload (transparently zlib-inflating a '.'-prefixed compressed payload), parses it as JSON, and converts the itsdangerous timestamp (offset from its 2011-01-01 epoch) to a Unix time + ISO-8601 UTC string. Pass 'cookie' as the bare value or a full 'session=...; Path=/' fragment — the name, quotes, and attributes are stripped. The signature is reported verbatim but NOT verified (no key is used, signature_verified is always false). Returns {payload, compressed, timestamp, timestamp_iso, signature, signature_verified}. Runs locally — the cookie never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "flask-session-decode", |a: Args| {
            gizza_ai_flask_session_decode_core::decode(&a.cookie).map_err(SkillError::InvalidArgs)
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
                    "cookie": { "type": "string", "description": "The Flask session cookie value to decode. Paste the bare value or a full 'session=...' fragment (a leading name=, surrounding quotes, and trailing '; Path=/; HttpOnly' attributes are stripped automatically). Format is the itsdangerous signed 'payload.timestamp.signature'." }
                },
                "required": ["cookie"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
