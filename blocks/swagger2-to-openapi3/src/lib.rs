//! gizza-ai/swagger2-to-openapi3 — chat skill block on the shared tool abstraction.
//! Upgrades a Swagger 2.0 spec (JSON or YAML) into an equivalent OpenAPI 3.0
//! document. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. No host calls —
//! runs entirely inside the WASM sandbox.
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
    output_format: String,
    #[serde(default)]
    target_version: String,
    /// JSON indent spaces (0 = minified). The core clamps to 0..=10.
    #[serde(default)]
    indent: u32,
    /// Fill the few 3.0-required fields a 2.0 spec may omit. Default true.
    #[serde(default = "default_true")]
    patch: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("spec")
                .required()
                .describe("The Swagger 2.0 (OpenAPI 2.0) document to upgrade, as JSON or YAML text."),
        )
        .param(
            Param::enumv("input_format", ["auto", "json", "yaml"])
                .default("auto")
                .describe("How to parse the input. 'auto' (default) tries JSON then YAML; force 'json' or 'yaml' to control error messages."),
        )
        .param(
            Param::enumv("output_format", ["json", "yaml"])
                .default("json")
                .describe("Format of the converted OpenAPI 3.0 document: 'json' (default) or 'yaml'."),
        )
        .param(
            Param::enumv("target_version", ["3.0.3", "3.0.2", "3.0.1", "3.0.0"])
                .default("3.0.3")
                .describe("Which OpenAPI 3.0.x version string to stamp into the 'openapi' field. Default '3.0.3' (the latest 3.0 patch)."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(0.0)
                .max(gizza_ai_swagger2_to_openapi3_core::MAX_INDENT as f64)
                .describe("Number of spaces to indent JSON output, 0-10. Use 0 for minified single-line JSON. Ignored for YAML output. Default 2."),
        )
        .param(
            Param::boolean("patch")
                .default(true)
                .describe("When true (default), fill in the fields OpenAPI 3.0 requires but Swagger 2.0 may omit (a missing info.title/info.version, missing response descriptions) so the result validates. When false, only structural changes are made."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/swagger2-to-openapi3",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Upgrade a Swagger 2.0 spec to OpenAPI 3.0.",
    skill(
        description = "Upgrade a Swagger 2.0 (OpenAPI 2.0) API specification into an equivalent OpenAPI 3.0 document. Accepts JSON or YAML (input_format='auto' by default) and emits JSON (output_format='json') or YAML. Transforms swagger->openapi version, host/basePath/schemes into a servers array, definitions into components.schemas, in:body/in:formData parameters into a requestBody (with consumes/produces folded into content media types), non-body parameters into schema-wrapped 3.0 parameters, securityDefinitions into components.securitySchemes (basic->http, oauth2 flow renames), response schemas into content, and retargets every $ref. Choose target_version (default 3.0.3) and JSON indent; patch=true (default) fills the fields 3.0 requires that 2.0 may omit.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "swagger2-to-openapi3", |a: Args| {
            gizza_ai_swagger2_to_openapi3_core::convert(
                &a.spec,
                &a.input_format,
                &a.output_format,
                &a.target_version,
                a.indent,
                a.patch,
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
                    "spec": { "type": "string", "description": "The Swagger 2.0 (OpenAPI 2.0) document to upgrade, as JSON or YAML text." },
                    "input_format": { "type": "string", "enum": ["auto", "json", "yaml"], "default": "auto", "description": "How to parse the input. 'auto' (default) tries JSON then YAML; force 'json' or 'yaml' to control error messages." },
                    "output_format": { "type": "string", "enum": ["json", "yaml"], "default": "json", "description": "Format of the converted OpenAPI 3.0 document: 'json' (default) or 'yaml'." },
                    "target_version": { "type": "string", "enum": ["3.0.3", "3.0.2", "3.0.1", "3.0.0"], "default": "3.0.3", "description": "Which OpenAPI 3.0.x version string to stamp into the 'openapi' field. Default '3.0.3' (the latest 3.0 patch)." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 10, "default": 2, "description": "Number of spaces to indent JSON output, 0-10. Use 0 for minified single-line JSON. Ignored for YAML output. Default 2." },
                    "patch": { "type": "boolean", "default": true, "description": "When true (default), fill in the fields OpenAPI 3.0 requires but Swagger 2.0 may omit (a missing info.title/info.version, missing response descriptions) so the result validates. When false, only structural changes are made." }
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
