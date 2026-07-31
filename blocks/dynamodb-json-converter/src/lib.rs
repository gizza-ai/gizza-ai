//! gizza-ai/dynamodb-json-converter — convert between DynamoDB typed attribute
//! JSON (the `AttributeValue` wire format) and plain JSON, in both directions.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_dynamodb_json_converter_core::{convert, Direction};
use serde::Deserialize;
use wafer_sdk::*;

fn default_direction() -> String {
    "auto".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input").required().multiline().describe(
                "The JSON to convert. Either plain JSON (e.g. {\"id\":\"1\",\"age\":30}) or \
                 DynamoDB typed JSON (e.g. {\"id\":{\"S\":\"1\"},\"age\":{\"N\":\"30\"}}).",
            ),
        )
        .param(
            Param::enumv("direction", ["auto", "to-dynamodb", "from-dynamodb"])
                .default("auto")
                .describe(
                    "auto detects the direction from the input shape; to-dynamodb marshals plain \
                     JSON into typed AttributeValues; from-dynamodb unmarshals typed JSON back to \
                     plain JSON.",
                ),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print (indent) the JSON output. Set false for compact single-line output."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dynamodb-json-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert between DynamoDB JSON and plain JSON, both directions",
    skill(
        description = "Convert between DynamoDB typed attribute JSON (the AttributeValue wire format like {\"id\":{\"S\":\"1\"},\"age\":{\"N\":\"30\"}}) and plain JSON ({\"id\":\"1\",\"age\":30}) in both directions. direction=auto detects which way to convert; to-dynamodb marshals plain JSON into typed AttributeValues (S, N as a string, BOOL, NULL, L, M); from-dynamodb unmarshals typed JSON back to plain JSON and also handles B (binary as base64) and the set types SS, NS, BS. pretty toggles indented vs compact output. Numbers keep their exact spelling. Errors on invalid JSON or a malformed AttributeValue. Runs locally; no AWS SDK or network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dynamodb-json-converter", |a: Args| {
            let dir = Direction::parse(&a.direction).map_err(SkillError::InvalidArgs)?;
            convert(&a.input, dir, a.pretty).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The JSON to convert. Either plain JSON (e.g. {\"id\":\"1\",\"age\":30}) or DynamoDB typed JSON (e.g. {\"id\":{\"S\":\"1\"},\"age\":{\"N\":\"30\"}})." },
                    "direction": { "type": "string", "enum": ["auto", "to-dynamodb", "from-dynamodb"], "default": "auto", "description": "auto detects the direction from the input shape; to-dynamodb marshals plain JSON into typed AttributeValues; from-dynamodb unmarshals typed JSON back to plain JSON." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print (indent) the JSON output. Set false for compact single-line output." }
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
