//! gizza-ai/openapi-to-typescript-types — chat skill block on the shared tool
//! abstraction. Extracts the schema objects from an OpenAPI 3.x / Swagger 2.0
//! document and emits matching TypeScript type declarations. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. No host calls — runs entirely inside the
//! WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    spec: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    declaration: String,
    #[serde(default)]
    enum_style: String,
    #[serde(default)]
    optional_style: String,
    #[serde(default = "default_true")]
    export: bool,
    #[serde(default)]
    readonly: bool,
    #[serde(default)]
    sort: bool,
    /// Spaces per nesting level. The core clamps to 0..=8.
    #[serde(default = "default_indent")]
    indent: u32,
}

fn default_true() -> bool {
    true
}
fn default_indent() -> u32 {
    2
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("spec")
                .required()
                .describe("The OpenAPI 3.x or Swagger 2.0 document to read, as JSON or YAML text. Types are generated from its components/schemas (3.x) or definitions (2.0) object."),
        )
        .param(
            Param::enumv("input_format", ["auto", "json", "yaml"])
                .default("auto")
                .describe("How to parse the input. 'auto' (default) tries JSON then YAML; force 'json' or 'yaml' to control the error message."),
        )
        .param(
            Param::enumv("declaration", ["interface", "type"])
                .default("interface")
                .describe("How to declare object schemas: 'interface' (default, `export interface X { … }`) or 'type' (`export type X = { … }`). Non-object schemas are always type aliases."),
        )
        .param(
            Param::enumv("enum_style", ["union", "enum"])
                .default("union")
                .describe("How to render a schema whose values are a string 'enum': 'union' (default, a `\"a\" | \"b\"` string-literal union) or 'enum' (a real TypeScript `enum`)."),
        )
        .param(
            Param::enumv("optional_style", ["spec", "optional", "required"])
                .default("spec")
                .describe("How property optionality is decided: 'spec' (default, mark a property optional `?` unless it is in the schema's `required` array), 'optional' (every property `?`), or 'required' (no property is optional)."),
        )
        .param(
            Param::boolean("export")
                .default(true)
                .describe("Prefix every declaration with `export` (default true). Set false for plain, non-exported declarations."),
        )
        .param(
            Param::boolean("readonly")
                .default(false)
                .describe("Mark every object property and index signature `readonly` (default false)."),
        )
        .param(
            Param::boolean("sort")
                .default(false)
                .describe("Alphabetize object properties (default false, which preserves the document's own key order)."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(0.0)
                .max(gizza_ai_openapi_to_typescript_types_core::MAX_INDENT as f64)
                .describe("Number of spaces per nesting level, 0-8. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/openapi-to-typescript-types",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate TypeScript types from an OpenAPI schema.",
    skill(
        description = "Extract the schema objects from an OpenAPI 3.x (components.schemas) or Swagger 2.0 (definitions) document and emit matching TypeScript type declarations. Accepts JSON or YAML (input_format='auto' by default). Handles $ref (as named type references), type (incl. 3.1 type arrays like [\"string\",\"null\"]), enum, const, nullable, required, properties, additionalProperties (index signatures), array items, tuple items/prefixItems, and allOf/oneOf/anyOf (intersection/union); descriptions become JSDoc comments. Choose declaration='interface' (default) or 'type' for object schemas, enum_style='union' (default) or 'enum', optional_style='spec'/'optional'/'required', plus export, readonly, sort, and indent. Returns the TypeScript source.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "openapi-to-typescript-types", |a: Args| {
            gizza_ai_openapi_to_typescript_types_core::convert(
                &a.spec,
                &a.input_format,
                &a.declaration,
                &a.enum_style,
                &a.optional_style,
                a.export,
                a.readonly,
                a.sort,
                a.indent,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "spec": { "type": "string", "description": "The OpenAPI 3.x or Swagger 2.0 document to read, as JSON or YAML text. Types are generated from its components/schemas (3.x) or definitions (2.0) object." },
                    "input_format": { "type": "string", "enum": ["auto", "json", "yaml"], "default": "auto", "description": "How to parse the input. 'auto' (default) tries JSON then YAML; force 'json' or 'yaml' to control the error message." },
                    "declaration": { "type": "string", "enum": ["interface", "type"], "default": "interface", "description": "How to declare object schemas: 'interface' (default, `export interface X { … }`) or 'type' (`export type X = { … }`). Non-object schemas are always type aliases." },
                    "enum_style": { "type": "string", "enum": ["union", "enum"], "default": "union", "description": "How to render a schema whose values are a string 'enum': 'union' (default, a `\"a\" | \"b\"` string-literal union) or 'enum' (a real TypeScript `enum`)." },
                    "optional_style": { "type": "string", "enum": ["spec", "optional", "required"], "default": "spec", "description": "How property optionality is decided: 'spec' (default, mark a property optional `?` unless it is in the schema's `required` array), 'optional' (every property `?`), or 'required' (no property is optional)." },
                    "export": { "type": "boolean", "default": true, "description": "Prefix every declaration with `export` (default true). Set false for plain, non-exported declarations." },
                    "readonly": { "type": "boolean", "default": false, "description": "Mark every object property and index signature `readonly` (default false)." },
                    "sort": { "type": "boolean", "default": false, "description": "Alphabetize object properties (default false, which preserves the document's own key order)." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Number of spaces per nesting level, 0-8. Default 2." }
                },
                "required": ["spec"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
