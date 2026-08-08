//! gizza-ai/graphql-mock-from-sdl — mock JSON response data from a GraphQL SDL schema.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn mode_default() -> String {
    "all-types".to_string()
}
fn nulls_default() -> String {
    "fill".to_string()
}
fn list_length_default() -> u32 {
    2
}
fn depth_default() -> u32 {
    3
}
fn seed_default() -> u64 {
    1
}
fn true_default() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    sdl: String,
    #[serde(default = "mode_default")]
    mode: String,
    #[serde(default)]
    type_name: String,
    #[serde(default = "list_length_default")]
    list_length: u32,
    #[serde(default = "depth_default")]
    depth: u32,
    #[serde(default = "nulls_default")]
    nullable_fields: String,
    #[serde(default = "true_default")]
    smart_values: bool,
    #[serde(default)]
    typename: bool,
    #[serde(default = "seed_default")]
    seed: u64,
    #[serde(default = "true_default")]
    pretty: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("sdl").required().multiline().describe("GraphQL SDL schema text. Supports type, interface, union, enum, input, scalar, schema, and extend definitions, descriptions, comments, field arguments, nested list/non-null wrappers, and the @fake(type: firstName), @examples(values: [...]) and @listLength(min: 1, max: 3) mocking directives."))
        .param(Param::enumv("mode", ["all-types", "query-response", "single-type"]).default("all-types").describe("What to mock: one object per type in the schema, a {\"data\": ...} response for the Query root, or a single named type."))
        .param(Param::string("type_name").describe("Type to mock in single-type mode, e.g. User. Ignored in the other modes."))
        .param(Param::integer("list_length").default(2).min(0.0).max(10.0).describe("Number of items generated for each list field (0-10)."))
        .param(Param::integer("depth").default(3).min(1.0).max(6.0).describe("Maximum object nesting depth (1-6). Beyond it, nested objects collapse to {} so recursive schemas terminate."))
        .param(Param::enumv("nullable_fields", ["fill", "null", "omit"]).default("fill").describe("How nullable fields are mocked: fill them with values, force them to null, or leave them out. Non-null fields are always generated."))
        .param(Param::boolean("smart_values").default(true).describe("Infer realistic values from field names (email, fullName, createdAt, price, latitude, ...) instead of generic placeholders."))
        .param(Param::boolean("typename").default(false).describe("Add __typename to every mocked object. Interface and union mocks always include it, because it names the concrete type chosen."))
        .param(Param::integer("seed").default(1).min(0.0).describe("Deterministic seed. The same seed and schema always produce identical output; 0 uses the core fallback seed."))
        .param(Param::boolean("pretty").default(true).describe("Pretty-print the JSON output."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct GraphqlMockFromSdl;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/graphql-mock-from-sdl",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate mock JSON response data from a GraphQL SDL schema",
    skill(
        description = "Parse a GraphQL SDL schema and generate deterministic mock JSON for every type in it, for a single named type, or as a {\"data\": ...} response envelope for the Query root. Handles interfaces (resolved to a concrete implementor), unions, enums, well-known custom scalars such as DateTime and JSON, nested list and non-null wrappers, extend definitions, field arguments, and the @fake / @examples / @listLength mocking directives. Controls cover list length, nesting depth, nullable-field handling (fill/null/omit), __typename injection, field-name-aware realistic values, and a reproducible seed.",
        parameters = schema_json()
    ),
)]
impl GraphqlMockFromSdl {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "graphql-mock-from-sdl", |a: Args| {
            gizza_ai_graphql_mock_from_sdl_core::generate(
                &a.sdl,
                &a.mode,
                &a.type_name,
                a.list_length,
                a.depth,
                &a.nullable_fields,
                a.smart_values,
                a.typename,
                a.seed,
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
                    "sdl":{"type":"string","description":"GraphQL SDL schema text. Supports type, interface, union, enum, input, scalar, schema, and extend definitions, descriptions, comments, field arguments, nested list/non-null wrappers, and the @fake(type: firstName), @examples(values: [...]) and @listLength(min: 1, max: 3) mocking directives."},
                    "mode":{"type":"string","enum":["all-types","query-response","single-type"],"default":"all-types","description":"What to mock: one object per type in the schema, a {\"data\": ...} response for the Query root, or a single named type."},
                    "type_name":{"type":"string","description":"Type to mock in single-type mode, e.g. User. Ignored in the other modes."},
                    "list_length":{"type":"integer","default":2,"minimum":0,"maximum":10,"description":"Number of items generated for each list field (0-10)."},
                    "depth":{"type":"integer","default":3,"minimum":1,"maximum":6,"description":"Maximum object nesting depth (1-6). Beyond it, nested objects collapse to {} so recursive schemas terminate."},
                    "nullable_fields":{"type":"string","enum":["fill","null","omit"],"default":"fill","description":"How nullable fields are mocked: fill them with values, force them to null, or leave them out. Non-null fields are always generated."},
                    "smart_values":{"type":"boolean","default":true,"description":"Infer realistic values from field names (email, fullName, createdAt, price, latitude, ...) instead of generic placeholders."},
                    "typename":{"type":"boolean","default":false,"description":"Add __typename to every mocked object. Interface and union mocks always include it, because it names the concrete type chosen."},
                    "seed":{"type":"integer","default":1,"minimum":0,"description":"Deterministic seed. The same seed and schema always produce identical output; 0 uses the core fallback seed."},
                    "pretty":{"type":"boolean","default":true,"description":"Pretty-print the JSON output."}
                },
                "required":["sdl"],
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
