//! gizza-ai/png-optimizer — losslessly shrink a PNG by re-encoding it with
//! optimal filters + a compact color type, without changing a single pixel.
//! Returns a PNG. Pure Rust (`png` + `image`, no ffmpeg) → runs on all backends
//! incl. the chat SW. Surfaces: chat + CLI (PNG input + PNG bytes output → no
//! page, like gif-optimize / image-color-quantize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_png_optimizer_core::{optimize, Effort};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 24 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_effort")]
    effort: String,
    #[serde(default = "default_reduce")]
    reduce: bool,
}
fn default_effort() -> String {
    "default".to_string()
}
fn default_reduce() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("effort", ["fast", "default", "max"])
                .default("default")
                .describe("How hard to work for a smaller file: fast (quick, single pass), default (best compression + adaptive filter), or max (also brute-forces every scanline filter and keeps the smallest). Default \"default\"."),
        )
        .param(
            Param::boolean("reduce")
                .default(true)
                .describe("Lossless color-type reduction (default true): pack to an indexed palette when the image has ≤256 distinct colors, drop a fully-opaque alpha channel, and use grayscale when every pixel is gray. Set false to keep the original color type."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PngOptimizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/png-optimizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Losslessly shrink a PNG (optimal filters + palette, pixels unchanged)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Losslessly shrink a PNG by re-encoding it with optimal scanline filters, stronger compression, and a more compact color type — WITHOUT changing a single pixel. effort is fast|default|max (default default; max brute-forces every filter). reduce (default true) losslessly packs to an indexed palette when there are ≤256 colors, drops a fully-opaque alpha channel, and uses grayscale when possible. All metadata is stripped and the output is never interlaced or larger than the input. Non-PNG input is rejected. Provide the PNG as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl PngOptimizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("png-optimizer")?;
    let effort = Effort::parse(&args.effort).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let res = optimize(&bytes, effort, args.reduce).map_err(SkillError::InvalidArgs)?;

    let for_llm = if res.reused_original {
        format!(
            "already optimal: {}×{}, {} bytes unchanged (0% saved)",
            res.width, res.height, res.input_len
        )
    } else {
        format!(
            "optimized PNG: {}×{} → {} ({} → {} bytes, {:.1}% smaller)",
            res.width,
            res.height,
            res.color_type,
            res.input_len,
            res.output_len,
            res.percent_saved()
        )
    };

    build_media_envelope(&res.bytes, "image/png", "optimized.png".to_string(), for_llm, MAX_OUTPUT_BYTES)
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
                    "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "effort": { "type": "string", "enum": ["fast", "default", "max"], "default": "default", "description": "How hard to work for a smaller file: fast (quick, single pass), default (best compression + adaptive filter), or max (also brute-forces every scanline filter and keeps the smallest). Default \"default\"." },
                    "reduce": { "type": "boolean", "default": true, "description": "Lossless color-type reduction (default true): pack to an indexed palette when the image has ≤256 distinct colors, drop a fully-opaque alpha channel, and use grayscale when every pixel is gray. Set false to keep the original color type." }
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
