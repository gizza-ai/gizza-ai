//! gizza-ai/cyberchef-pipeline — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Chains byte-level
//! decode/transform steps into one client-side recipe (a focused take on the
//! CyberChef recipe model). Runs entirely inside the WASM sandbox — no upload,
//! no arbitrary code execution, only the fixed operation set.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_cyberchef_pipeline_core::{run, OutputFormat, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_recipe")]
    recipe: String,
    #[serde(default = "default_output_format")]
    output_format: String,
}

fn default_recipe() -> String {
    "from-base64".to_string()
}
fn default_output_format() -> String {
    "auto".to_string()
}

const RECIPE_DESC: &str = "The recipe: ONE operation per line, applied top to bottom over the byte \
buffer. Blank lines and lines starting with '#' are ignored. Operations: 'from-base64' / 'to-base64' \
(decode tolerates whitespace, URL-safe alphabet and missing padding); 'from-hex' (ignores whitespace, \
':' , ',' and a '0x' prefix) / 'to-hex' (lowercase, no separator); 'url-decode' / 'url-encode' \
(percent-encoding); 'rot13'; 'gunzip' / 'gzip'; 'zlib-inflate' / 'zlib-deflate'; 'raw-inflate' / \
'raw-deflate' (raw DEFLATE); 'xor KEY [hex|utf8|base64|decimal]' (repeating-key XOR, key format \
defaults to hex, e.g. 'xor 2a' or 'xor secret utf8'); 'add N' / 'sub N' (add/subtract a byte mod 256, \
N decimal or 0x..); 'not' (bitwise NOT); 'reverse' (reverse byte order); 'upper' / 'lower' (ASCII \
case). Example: from-base64 / gunzip / xor 2a.";

const OUTPUT_FORMAT_DESC: &str = "How the final byte buffer is rendered as text. 'auto' (default) \
shows UTF-8 if the whole result is valid UTF-8, otherwise lowercase hex; 'utf8' forces lossy UTF-8; \
'hex' forces lowercase hex; 'base64' forces standard padded Base64. Use hex or base64 when a step \
produces binary bytes.";

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The data to transform, fed into the first recipe step as UTF-8 bytes."),
        )
        .param(
            Param::string("recipe")
                .required()
                .default(default_recipe())
                .describe(RECIPE_DESC),
        )
        .param(
            Param::enumv("output_format", ["auto", "utf8", "hex", "base64"])
                .default("auto")
                .describe(OUTPUT_FORMAT_DESC),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from(a: &Args) -> Result<Options, SkillError> {
    Ok(Options {
        output_format: OutputFormat::parse(&a.output_format).map_err(SkillError::InvalidArgs)?,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cyberchef-pipeline",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Chain byte-level decode/transform steps (Base64, hex, gzip, XOR…) into one client-side recipe.",
    skill(
        description = "Chain byte-level decode and transform steps into a single client-side recipe, applied top to bottom over a byte buffer — a focused take on the CyberChef recipe model that runs as WASM in the browser (no upload). The 'recipe' is one operation per line: from-base64/to-base64 (tolerant decode), from-hex/to-hex, url-decode/url-encode, rot13, gunzip/gzip, zlib-inflate/zlib-deflate, raw-inflate/raw-deflate, xor KEY [hex|utf8|base64|decimal] (repeating key), add N / sub N (byte arithmetic mod 256), not, reverse, upper, lower. Blank lines and #comments are ignored. output_format = auto|utf8|hex|base64 controls how the final bytes render (auto = UTF-8 if printable else hex). Does NOT execute arbitrary code — only these fixed operations; keyed modern crypto and hashing live in the dedicated cipher/hash blocks.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cyberchef-pipeline", |a: Args| {
            let opts = options_from(&a)?;
            run(&a.input, &a.recipe, &opts).map_err(SkillError::InvalidArgs)
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The data to transform, fed into the first recipe step as UTF-8 bytes." },
                    "recipe": {
                        "type": "string",
                        "default": "from-base64",
                        "description": "The recipe: ONE operation per line, applied top to bottom over the byte buffer. Blank lines and lines starting with '#' are ignored. Operations: 'from-base64' / 'to-base64' (decode tolerates whitespace, URL-safe alphabet and missing padding); 'from-hex' (ignores whitespace, ':' , ',' and a '0x' prefix) / 'to-hex' (lowercase, no separator); 'url-decode' / 'url-encode' (percent-encoding); 'rot13'; 'gunzip' / 'gzip'; 'zlib-inflate' / 'zlib-deflate'; 'raw-inflate' / 'raw-deflate' (raw DEFLATE); 'xor KEY [hex|utf8|base64|decimal]' (repeating-key XOR, key format defaults to hex, e.g. 'xor 2a' or 'xor secret utf8'); 'add N' / 'sub N' (add/subtract a byte mod 256, N decimal or 0x..); 'not' (bitwise NOT); 'reverse' (reverse byte order); 'upper' / 'lower' (ASCII case). Example: from-base64 / gunzip / xor 2a."
                    },
                    "output_format": {
                        "type": "string",
                        "enum": ["auto", "utf8", "hex", "base64"],
                        "default": "auto",
                        "description": "How the final byte buffer is rendered as text. 'auto' (default) shows UTF-8 if the whole result is valid UTF-8, otherwise lowercase hex; 'utf8' forces lossy UTF-8; 'hex' forces lowercase hex; 'base64' forces standard padded Base64. Use hex or base64 when a step produces binary bytes."
                    }
                },
                "required": ["input", "recipe"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
