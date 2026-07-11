//! gizza-ai/base-decoder — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Auto-detects and peels
//! layered Base16/32/45/58/64/85 encodings via the shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    max_depth: Option<u64>,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The encoded blob to decode. May be wrapped in several base layers (e.g. Base64 of Base32); whitespace and newlines are ignored except for Base45 where space is a data symbol."),
        )
        .param(
            Param::integer("max_depth")
                .min(1.0)
                .max(30.0)
                .default(8)
                .describe("Maximum number of encoding layers to peel before stopping. Default 8, clamped to 1-30."),
        )
        .param(
            Param::enumv("output", ["report", "plain"])
                .default("report")
                .describe("Output style. 'report' (default) annotates the detected layer chain and shows text or a binary hex preview; 'plain' returns only the final decoded text (or hex for binary)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_logic(a: Args) -> Result<String, String> {
    let depth = a
        .max_depth
        .map(|d| d as usize)
        .unwrap_or(gizza_ai_base_decoder_core::DEFAULT_DEPTH);
    gizza_ai_base_decoder_core::decode(&a.input, depth, &a.output)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto-detect and peel layered Base16/32/45/58/64/85 encodings",
    skill(
        description = "Auto-detect the encoding of an opaque blob and recursively peel layered base encodings until plaintext, a known binary file signature, or the depth cap is reached. Handles Base16 (hex), Base32, Base45, Base58, Base64 (standard and URL-safe), and Base85 (Ascii85), including nested combinations such as Base64 over Base32. Returns the detected layer chain (e.g. base64 -> base32) plus the decoded text, or a magic-byte signature and hex preview for binary output. Set max_depth (default 8, 1-30) to bound the peel and output='plain' to return only the final decoded value. Runs locally and never uploads data. For a single known scheme with full control, use the per-scheme codec tools instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "base-decoder", |a: Args| {
            run_logic(a).map_err(SkillError::InvalidArgs)
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
    fn nested_layers_reported() {
        let r = run_logic(Args {
            input: "U0dWc2JHOHNJRmR2Y214a0lRPT0=".to_string(),
            max_depth: None,
            output: String::new(),
        })
        .unwrap();
        assert!(r.starts_with("Detected 2 layer(s): base64 \u{2192} base64"));
        assert!(r.contains("Hello, World!"));
    }

    #[test]
    fn plain_output_path() {
        let r = run_logic(Args {
            input: "SGVsbG8sIFdvcmxkIQ==".to_string(),
            max_depth: Some(8),
            output: "plain".to_string(),
        })
        .unwrap();
        assert_eq!(r, "Hello, World!");
    }

    #[test]
    fn depth_cap_honored() {
        let r = run_logic(Args {
            input: "U0dWc2JHOHNJRmR2Y214a0lRPT0=".to_string(),
            max_depth: Some(1),
            output: "plain".to_string(),
        })
        .unwrap();
        assert_eq!(r, "SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn invalid_output_errors() {
        let r = run_logic(Args {
            input: "SGVsbG8sIFdvcmxkIQ==".to_string(),
            max_depth: None,
            output: "sideways".to_string(),
        });
        assert!(r.is_err());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The encoded blob to decode. May be wrapped in several base layers (e.g. Base64 of Base32); whitespace and newlines are ignored except for Base45 where space is a data symbol." },
                    "max_depth": { "type": "integer", "minimum": 1, "maximum": 30, "default": 8, "description": "Maximum number of encoding layers to peel before stopping. Default 8, clamped to 1-30." },
                    "output": { "type": "string", "enum": ["report", "plain"], "default": "report", "description": "Output style. 'report' (default) annotates the detected layer chain and shows text or a binary hex preview; 'plain' returns only the final decoded text (or hex for binary)." }
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
