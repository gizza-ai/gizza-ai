//! gizza-ai/jwt-claims-diff — decode two compact JWTs offline and report which
//! claims were added, removed or changed between them. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jwt_claims_diff_core::diff_jwts;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    #[serde(default = "default_true")]
    include_header: bool,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_true() -> bool {
    true
}
fn default_indent() -> u64 {
    2
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("left")
                .required()
                .describe("The first (left/old) compact JWT (header.payload.signature)."),
        )
        .param(
            Param::string("right")
                .required()
                .describe("The second (right/new) compact JWT to compare against the first."),
        )
        .param(
            Param::boolean("include_header")
                .default(true)
                .describe("Also diff the JOSE header parameters (alg, typ, kid…), not just the payload claims. Default true."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Output indentation in spaces (1-8). Use 0 to minify. Default 2."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jwt-claims-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Diff the claims of two JWTs offline",
    skill(
        description = "Decode two compact JSON Web Tokens (JWTs) offline — no verification key — and report which claims were added, removed or changed between them. Claims are compared at the top level and classified as added (only in the second token), removed (only in the first) or changed (present in both with a different value). Registered time claims (exp/nbf/iat) get human-readable UTC annotations, and when both tokens carry exp the report includes the expiry delta. Set include_header=false to compare payload claims only. Output: { equal, similarity, summary:{added,removed,changed,unchanged}, payload:[{claim,kind,old?,new?,old_time?,new_time?}], header?:[…], expiry?:{…} }. Signatures are never verified. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jwt-claims-diff", |a: Args| {
            diff_jwts(&a.left, &a.right, a.include_header, a.indent as usize)
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
                "type": "object",
                "properties": {
                    "left":           { "type": "string", "description": "The first (left/old) compact JWT (header.payload.signature)." },
                    "right":          { "type": "string", "description": "The second (right/new) compact JWT to compare against the first." },
                    "include_header": { "type": "boolean", "default": true, "description": "Also diff the JOSE header parameters (alg, typ, kid…), not just the payload claims. Default true." },
                    "indent":         { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Output indentation in spaces (1-8). Use 0 to minify. Default 2." }
                },
                "required": ["left", "right"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
