//! gizza-ai/json-to-types — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_to_types_core::{generate, Language, OptionalStrategy, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_language")]
    output_language: String,
    #[serde(default = "default_root_name")]
    root_name: String,
    #[serde(default = "default_strategy")]
    optional_strategy: String,
    #[serde(default = "default_true")]
    json_annotations: bool,
    #[serde(default = "default_true")]
    export: bool,
}
fn default_language() -> String {
    "typescript".to_string()
}
fn default_root_name() -> String {
    "Root".to_string()
}
fn default_strategy() -> String {
    "optional".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json").required().describe(
                "The JSON sample to infer types from — one representative record is enough. Example: '{\"id\":1,\"name\":\"Ada\",\"tags\":[\"admin\"]}'. Objects, arrays, nested objects and arrays of objects all work; array elements are merged so a key missing from some of them becomes optional. Up to 2,000,000 bytes and 64 levels of nesting.",
            ),
        )
        .param(
            Param::enumv("output_language", ["typescript", "rust", "go", "python"])
                .default("typescript")
                .describe(
                    "Target language: typescript (default) emits interfaces, rust emits structs with serde derives, go emits structs with json tags, python emits @dataclass classes. Aliases ts, rs, golang and py are accepted.",
                ),
        )
        .param(
            Param::string("root_name")
                .default("Root")
                .describe(
                    "Name for the top-level type, PascalCased automatically (default Root). Example: 'User' produces 'interface User' / 'struct User' / 'class User'. When the root is an array or a primitive you get a type alias with this name instead.",
                ),
        )
        .param(
            Param::enumv("optional_strategy", ["optional", "nullable", "required"])
                .default("optional")
                .describe(
                    "How null and missing keys are typed. optional (default): a null value or a key absent from some array elements becomes optional — 'name?: string', 'Option<String>', '*string', 'Optional[str] = None'. nullable: only absent keys are optional; null widens the type instead — 'string | null'. required: every key required and nulls ignored, giving the narrowest types.",
                ),
        )
        .param(
            Param::boolean("json_annotations")
                .default(true)
                .describe(
                    "Emit serialization annotations: serde derives plus #[serde(rename)] for Rust, and json:\"…\" struct tags (with ,omitempty on optional fields) for Go. Default true. No effect on TypeScript or Python output.",
                ),
        )
        .param(
            Param::boolean("export")
                .default(true)
                .describe(
                    "Make the generated types public: 'export interface' in TypeScript, 'pub struct' with pub fields in Rust, capitalised (exported) type names in Go. Default true. No effect on Python.",
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
    name = "gizza-ai/json-to-types",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Infer TypeScript interfaces, Rust structs, Go structs or Python dataclasses from a JSON sample",
    skill(
        description = "Infer type definitions from a JSON sample and emit them as TypeScript interfaces, Rust structs, Go structs or Python dataclasses. `json` takes one representative record (object, array or array of objects). Nested objects and arrays of objects each get their own named type, key order is preserved, array elements are merged so a key missing from some of them becomes optional, integers and floats are told apart, mixed primitives become a union (TypeScript/Python) or the language's any type (Rust/Go), and structurally identical objects are emitted once and reused. output_language=typescript (default) | rust | go | python. root_name names the top-level type (default Root). optional_strategy=optional (default) | nullable | required controls how nulls and missing keys are typed. json_annotations toggles serde derives/renames (Rust) and json struct tags (Go); export toggles export/pub/exported names. Deterministic — the same JSON always yields the same code. Runs locally, nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-types", |a: Args| {
            let opts = Options {
                language: Language::parse(&a.output_language).map_err(SkillError::InvalidArgs)?,
                root_name: a.root_name,
                optional_strategy: OptionalStrategy::parse(&a.optional_strategy)
                    .map_err(SkillError::InvalidArgs)?,
                json_annotations: a.json_annotations,
                export: a.export,
            };
            generate(&a.json, &opts).map_err(SkillError::InvalidArgs)
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
                    "json": { "type": "string", "description": "The JSON sample to infer types from — one representative record is enough. Example: '{\"id\":1,\"name\":\"Ada\",\"tags\":[\"admin\"]}'. Objects, arrays, nested objects and arrays of objects all work; array elements are merged so a key missing from some of them becomes optional. Up to 2,000,000 bytes and 64 levels of nesting." },
                    "output_language": { "type": "string", "enum": ["typescript", "rust", "go", "python"], "default": "typescript", "description": "Target language: typescript (default) emits interfaces, rust emits structs with serde derives, go emits structs with json tags, python emits @dataclass classes. Aliases ts, rs, golang and py are accepted." },
                    "root_name": { "type": "string", "default": "Root", "description": "Name for the top-level type, PascalCased automatically (default Root). Example: 'User' produces 'interface User' / 'struct User' / 'class User'. When the root is an array or a primitive you get a type alias with this name instead." },
                    "optional_strategy": { "type": "string", "enum": ["optional", "nullable", "required"], "default": "optional", "description": "How null and missing keys are typed. optional (default): a null value or a key absent from some array elements becomes optional — 'name?: string', 'Option<String>', '*string', 'Optional[str] = None'. nullable: only absent keys are optional; null widens the type instead — 'string | null'. required: every key required and nulls ignored, giving the narrowest types." },
                    "json_annotations": { "type": "boolean", "default": true, "description": "Emit serialization annotations: serde derives plus #[serde(rename)] for Rust, and json:\"…\" struct tags (with ,omitempty on optional fields) for Go. Default true. No effect on TypeScript or Python output." },
                    "export": { "type": "boolean", "default": true, "description": "Make the generated types public: 'export interface' in TypeScript, 'pub struct' with pub fields in Rust, capitalised (exported) type names in Go. Default true. No effect on Python." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
