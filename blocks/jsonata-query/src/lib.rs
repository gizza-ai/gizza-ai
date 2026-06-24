//! gizza-ai/jsonata-query — evaluate a JSONata query/transform expression against
//! a JSON document (pure-Rust jsonata-rs). Thin wrapper around the core; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends incl. chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jsonata_query_core::run_jsonata;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    query: String,
    json: String,
    #[serde(default)]
    pretty: bool,
}

#[derive(Serialize)]
struct Resp {
    /// The JSONata result, serialized as a JSON string.
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("query").required().describe("The JSONata expression, e.g. '$sum(items.price)' or 'Account.Order.Product.{ \"name\": ProductName, \"total\": Price * Quantity }'."))
        .param(Param::string("json").required().describe("The JSON document to evaluate the expression against."))
        .param(Param::boolean("pretty").default(false).describe("Pretty-print (indent) the JSON output. Default false (compact)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonataQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jsonata-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Evaluate a JSONata expression against JSON",
    skill(
        description = "Evaluate a JSONata query/transformation expression against a JSON document and return the result, using a pure-Rust JSONata engine. JSONata is a lightweight language for querying, filtering, aggregating, and reshaping JSON — e.g. navigate paths ('Account.Order.Product.Price'), filter ('items[price > 10].name'), aggregate ('$sum(items.price)', '$count(orders)'), and construct output ('{ \"total\": $sum(lines.amount) }'). The result is returned as a serialized JSON string in `output`; set pretty=true to indent. A query that matches nothing returns JSON null.",
        parameters = schema_json()
    ),
)]
impl JsonataQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jsonata-query", |a: Args| {
            let output = run_jsonata(&a.query, &a.json, a.pretty).map_err(SkillError::InvalidArgs)?;
            Ok(Resp { output })
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
                    "query":  { "type": "string", "description": "The JSONata expression, e.g. '$sum(items.price)' or 'Account.Order.Product.{ \"name\": ProductName, \"total\": Price * Quantity }'." },
                    "json":   { "type": "string", "description": "The JSON document to evaluate the expression against." },
                    "pretty": { "type": "boolean", "default": false, "description": "Pretty-print (indent) the JSON output. Default false (compact)." }
                },
                "required": ["query", "json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
