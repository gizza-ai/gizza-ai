//! gizza-ai/json-schema-to-sample — build a minimal valid example JSON instance
//! from a JSON Schema. Thin wrapper around the core; the chat schema is
//! single-sourced from descriptor(); handle() delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn pretty_default() -> bool {
    true
}
fn array_items_default() -> u32 {
    1
}

#[derive(Deserialize)]
struct Args {
    schema: String,
    #[serde(default)]
    include_optional: bool,
    #[serde(default = "array_items_default")]
    array_items: u32,
    #[serde(default = "pretty_default")]
    pretty: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("schema").required().multiline().describe(
            "The JSON Schema to build an example for (draft-07 / 2019-09 / 2020-12). Supported: type, properties, required, default, const, enum, examples, string format, minLength/maxLength, minimum/maximum/exclusive bounds, multipleOf, items/prefixItems/minItems/maxItems/uniqueItems, allOf (merged), oneOf/anyOf (first branch), and local $ref into $defs or definitions. Example: {\"type\":\"object\",\"properties\":{\"id\":{\"type\":\"integer\"}},\"required\":[\"id\"]}",
        ))
        .param(Param::boolean("include_optional").default(false).describe(
            "Include optional properties as well as required ones. Default false, so the sample stays minimal: only required properties plus any property that declares a default.",
        ))
        .param(Param::integer("array_items").default(1).min(0.0).max(50.0).describe(
            "How many entries to put in an array that has no minItems of its own (0-50, default 1). A larger minItems always wins, and maxItems still caps the result.",
        ))
        .param(Param::boolean("pretty").default(true).describe(
            "Pretty-print the sample with 2-space indentation. Turn off for one compact line.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonSchemaToSample;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-schema-to-sample",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a minimal valid example JSON instance from a JSON Schema",
    skill(
        description = "Generate the smallest JSON document that satisfies a JSON Schema, for API docs, fixtures and request bodies. Honours const, default, examples and enum in that order, emits required properties only unless include_optional is set, resolves local $ref into $defs/definitions, merges allOf, takes the first oneOf/anyOf branch, and respects string formats (email, uuid, date-time, uri, ipv4, ...), length/range/multipleOf bounds and array item bounds. Fully deterministic: the same schema always yields the same sample. pattern, if/then/else, not, dependentSchemas and remote $refs are not synthesised.",
        parameters = schema_json()
    ),
)]
impl JsonSchemaToSample {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-schema-to-sample", |a: Args| {
            gizza_ai_json_schema_to_sample_core::sample(
                &a.schema,
                gizza_ai_json_schema_to_sample_core::Options {
                    include_optional: a.include_optional,
                    array_items: a.array_items,
                    pretty: a.pretty,
                },
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
                    "schema":{"type":"string","description":"The JSON Schema to build an example for (draft-07 / 2019-09 / 2020-12). Supported: type, properties, required, default, const, enum, examples, string format, minLength/maxLength, minimum/maximum/exclusive bounds, multipleOf, items/prefixItems/minItems/maxItems/uniqueItems, allOf (merged), oneOf/anyOf (first branch), and local $ref into $defs or definitions. Example: {\"type\":\"object\",\"properties\":{\"id\":{\"type\":\"integer\"}},\"required\":[\"id\"]}"},
                    "include_optional":{"type":"boolean","default":false,"description":"Include optional properties as well as required ones. Default false, so the sample stays minimal: only required properties plus any property that declares a default."},
                    "array_items":{"type":"integer","minimum":0,"maximum":50,"default":1,"description":"How many entries to put in an array that has no minItems of its own (0-50, default 1). A larger minItems always wins, and maxItems still caps the result."},
                    "pretty":{"type":"boolean","default":true,"description":"Pretty-print the sample with 2-space indentation. Turn off for one compact line."}
                },
                "required":["schema"],
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
