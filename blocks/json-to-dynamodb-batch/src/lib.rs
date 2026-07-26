//! gizza-ai/json-to-dynamodb-batch — build a DynamoDB BatchWriteItem payload
//! from a JSON array of objects. Thin wrapper around the core; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn operation_default() -> String {
    "put".to_string()
}
fn pretty_default() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    json: String,
    table_name: String,
    #[serde(default = "operation_default")]
    operation: String,
    #[serde(default = "pretty_default")]
    pretty: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("json").required().multiline().describe("JSON array of objects to convert. Each object becomes one write request (max 25 per DynamoDB BatchWriteItem call)."))
        .param(Param::string("table_name").required().describe("Target DynamoDB table name that the write requests are keyed under."))
        .param(Param::enumv("operation", ["put", "delete"]).default("put").describe("put emits PutRequest/Item entries; delete emits DeleteRequest/Key entries (objects should hold only key attributes)."))
        .param(Param::boolean("pretty").default(true).describe("Pretty-print the JSON output. Set false for compact single-line output."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonToDynamodbBatch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-dynamodb-batch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a DynamoDB BatchWriteItem payload from a JSON array",
    skill(
        description = "Convert a JSON array of objects into a DynamoDB BatchWriteItem request payload. Emits { \"RequestItems\": { table: [ ... ] } } with each object mapped to typed AttributeValues (S, N, BOOL, NULL, L, M). Choose put (PutRequest/Item) or delete (DeleteRequest/Key), and pretty or compact output. Numbers keep their exact JSON spelling. Errors on invalid JSON, a non-array root, a non-object item, an empty table name, or more than 25 items (the DynamoDB batch limit).",
        parameters = schema_json()
    ),
)]
impl JsonToDynamodbBatch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-dynamodb-batch", |a: Args| {
            gizza_ai_json_to_dynamodb_batch_core::run(
                &a.json,
                &a.table_name,
                &a.operation,
                a.pretty,
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
                "type":"object",
                "properties":{
                    "json":{"type":"string","description":"JSON array of objects to convert. Each object becomes one write request (max 25 per DynamoDB BatchWriteItem call)."},
                    "table_name":{"type":"string","description":"Target DynamoDB table name that the write requests are keyed under."},
                    "operation":{"type":"string","enum":["put","delete"],"default":"put","description":"put emits PutRequest/Item entries; delete emits DeleteRequest/Key entries (objects should hold only key attributes)."},
                    "pretty":{"type":"boolean","default":true,"description":"Pretty-print the JSON output. Set false for compact single-line output."}
                },
                "required":["json","table_name"],
                "additionalProperties":false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
