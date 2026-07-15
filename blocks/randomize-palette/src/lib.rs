//! gizza-ai/randomize-palette — randomly remap the color palette of an indexed
//! image (GIF / PNG-8) to expose hidden shapes and stego payloads. Pure Rust
//! (image + color_quant) — runs on all backends incl. the chat SW. Surfaces:
//! chat + CLI (image input + image bytes output → no page, like
//! image-color-quantize). The remap is seeded so every surface is deterministic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_randomize_palette_core::randomize_palette;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    seed: u64,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image).param(
        Param::integer("seed")
            .min(0.0)
            .default(0)
            .describe("Seed for the random palette permutation (default 0). The same seed always produces the same remap; change it to try a different shuffle and reveal different hidden structure."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RandomizePalette;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/randomize-palette",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Randomly remap an indexed image's palette",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Randomly remap the color palette of an indexed GIF/PNG-8 image — every pixel keeps its index but each palette slot is reassigned a different color, so shapes and stego payloads hidden by near-identical palette entries become visible. Images with more than 256 colors are quantized to 256 first. The shuffle is driven by seed (default 0) so results are reproducible; change seed to try a different shuffle. Returns a PNG. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl RandomizePalette {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("randomize-palette")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = randomize_palette(&bytes, args.seed).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "randomized-palette.png".to_string(),
        format!(
            "remapped the palette with seed {} ({} bytes PNG)",
            args.seed,
            png.len()
        ),
        MAX_OUTPUT_BYTES,
    )
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
                    "url":  { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "seed": { "type": "integer", "minimum": 0, "default": 0, "description": "Seed for the random palette permutation (default 0). The same seed always produces the same remap; change it to try a different shuffle and reveal different hidden structure." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
