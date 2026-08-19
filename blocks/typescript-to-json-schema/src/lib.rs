//! gizza-ai/typescript-to-json-schema — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_typescript_to_json_schema_core::{convert, Draft, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    typescript: String,
    #[serde(default)]
    root_type: String,
    #[serde(default = "default_draft")]
    draft: String,
    #[serde(default = "default_true")]
    required: bool,
    #[serde(default)]
    additional_properties: bool,
    #[serde(default = "default_true")]
    jsdoc: bool,
}
fn default_draft() -> String {
    "2020-12".to_string()
}
fn default_true() -> bool {
    true
}

fn draft_from(s: &str) -> Draft {
    match s.trim() {
        "draft-07" | "draft7" | "07" | "7" => Draft::Draft07,
        _ => Draft::Draft2020,
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("typescript")
                .required()
                .describe("TypeScript source containing one or more `interface`, `type` or `enum` declarations (a bare type literal like `{ id: string }` also works). Supported: primitives (string/number/boolean/null/bigint/any/unknown/never/Date), arrays (`T[]`, `Array<T>`), tuples, optional `?` members, string/number/boolean literal unions, enums, nested object literals, index signatures, `Record<string, T>`, `extends`, object intersections, and references between declarations. Generics, utility/mapped types (Partial, Pick, Omit, keyof, …), imports and functions are rejected with a line-numbered message. Example: 'interface User { id: number; name: string; role: \"admin\" | \"user\" }'."),
        )
        .param(
            Param::string("root_type")
                .default("")
                .describe("Name of the declaration to use as the schema root, e.g. 'User'. Empty (default) uses the FIRST declaration in the source. Other declarations are emitted under $defs/definitions only when the root references them."),
        )
        .param(
            Param::enumv("draft", ["2020-12", "draft-07"])
                .default("2020-12")
                .describe("JSON Schema dialect to emit. '2020-12' (default) uses $defs and prefixItems for tuples; 'draft-07' uses definitions, an items array and additionalItems. Also sets the $schema URL."),
        )
        .param(
            Param::boolean("required")
                .default(true)
                .describe("Emit a 'required' array listing every member that is not marked optional with `?` (or `| undefined`). Default true; false omits 'required' entirely."),
        )
        .param(
            Param::boolean("additional_properties")
                .default(false)
                .describe("Allow properties beyond those declared. false (default) emits 'additionalProperties: false' for strict validation; true omits it. Objects with an index signature or `Record<string, T>` always use that value type instead."),
        )
        .param(
            Param::boolean("jsdoc")
                .default(true)
                .describe("Read `/** … */` JSDoc comments. Prose becomes 'description'; the tags @title @format @pattern @minimum @maximum @exclusiveMinimum @exclusiveMaximum @multipleOf @minLength @maxLength @minItems @maxItems @minProperties @maxProperties @uniqueItems @readOnly @writeOnly @default @example @deprecated @nullable and @asType (alias @TJS-type) become the matching JSON Schema keywords. Default true; false ignores comments."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TypescriptToJsonSchema;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/typescript-to-json-schema",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a TypeScript interface or type alias to JSON Schema",
    skill(
        description = "Convert pasted TypeScript into an equivalent JSON Schema (Draft 2020-12 or Draft-07). Handles `interface`, `type` aliases, `enum` declarations and bare type literals: primitives (string, number, boolean, null, bigint, any/unknown, never, Date), arrays (`T[]` / `Array<T>`), tuples, optional `?` members, string/number/boolean literal unions (emitted as `enum`), nested object literals, string index signatures and `Record<string, T>` (emitted as `additionalProperties`), `extends` and object intersections (member-merged), and references between declarations (emitted as `$ref` into `$defs`, pruned to what the root actually reaches — recursive types are fine). JSDoc comments become `description` plus constraint keywords such as `@format`, `@minimum`, `@pattern` and `@asType`. Options: root_type (which declaration is the root; default the first), draft, required, additional_properties (strict by default), jsdoc. Constructs that need a real TypeScript type checker — generics, utility/mapped types like Partial or Pick, `keyof`, imports, functions/methods — are rejected with a line-numbered error naming the construct. Returns the pretty-printed schema.",
        parameters = schema_json()
    ),
)]
impl TypescriptToJsonSchema {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "typescript-to-json-schema", |a: Args| {
            let opts = Options {
                draft: draft_from(&a.draft),
                root_type: a.root_type,
                required: a.required,
                additional_properties: a.additional_properties,
                jsdoc: a.jsdoc,
            };
            convert(&a.typescript, &opts).map_err(SkillError::InvalidArgs)
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
                    "typescript":            { "type": "string", "description": "TypeScript source containing one or more `interface`, `type` or `enum` declarations (a bare type literal like `{ id: string }` also works). Supported: primitives (string/number/boolean/null/bigint/any/unknown/never/Date), arrays (`T[]`, `Array<T>`), tuples, optional `?` members, string/number/boolean literal unions, enums, nested object literals, index signatures, `Record<string, T>`, `extends`, object intersections, and references between declarations. Generics, utility/mapped types (Partial, Pick, Omit, keyof, …), imports and functions are rejected with a line-numbered message. Example: 'interface User { id: number; name: string; role: \"admin\" | \"user\" }'." },
                    "root_type":             { "type": "string", "default": "", "description": "Name of the declaration to use as the schema root, e.g. 'User'. Empty (default) uses the FIRST declaration in the source. Other declarations are emitted under $defs/definitions only when the root references them." },
                    "draft":                 { "type": "string", "enum": ["2020-12", "draft-07"], "default": "2020-12", "description": "JSON Schema dialect to emit. '2020-12' (default) uses $defs and prefixItems for tuples; 'draft-07' uses definitions, an items array and additionalItems. Also sets the $schema URL." },
                    "required":              { "type": "boolean", "default": true, "description": "Emit a 'required' array listing every member that is not marked optional with `?` (or `| undefined`). Default true; false omits 'required' entirely." },
                    "additional_properties": { "type": "boolean", "default": false, "description": "Allow properties beyond those declared. false (default) emits 'additionalProperties: false' for strict validation; true omits it. Objects with an index signature or `Record<string, T>` always use that value type instead." },
                    "jsdoc":                 { "type": "boolean", "default": true, "description": "Read `/** … */` JSDoc comments. Prose becomes 'description'; the tags @title @format @pattern @minimum @maximum @exclusiveMinimum @exclusiveMaximum @multipleOf @minLength @maxLength @minItems @maxItems @minProperties @maxProperties @uniqueItems @readOnly @writeOnly @default @example @deprecated @nullable and @asType (alias @TJS-type) become the matching JSON Schema keywords. Default true; false ignores comments." }
                },
                "required": ["typescript"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
