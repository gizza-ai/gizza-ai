//! gizza-ai/animated-heatmap — render a time-ordered sequence of numeric
//! matrices as an animated heatmap GIF showing how values evolve. Pure-Rust
//! (the `image` crate's GIF encoder, no ffmpeg) so it runs on all backends incl.
//! the chat Service Worker. The GIF is wrapped as image/gif via
//! build_media_envelope (like gif-from-images). Surfaces: chat + CLI (no page
//! mode — image-bytes output has no page render mode).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_animated_heatmap_core::animated_heatmap;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    data: String,
    #[serde(default = "default_delay")]
    delay_ms: u64,
    #[serde(default = "default_cell")]
    cell_px: u64,
    #[serde(default)]
    scale_min: Option<f64>,
    #[serde(default)]
    scale_max: Option<f64>,
}
fn default_delay() -> u64 {
    400
}
fn default_cell() -> u64 {
    24
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The time-ordered matrices: each frame is one matrix (rows of comma/space-separated numbers, one row per line); separate consecutive frames with a BLANK line. Every frame must have the same dimensions."))
        .param(Param::integer("delay_ms").min(10.0).max(60000.0).default(400).describe("Delay per frame in milliseconds (10-60000, default 400)."))
        .param(Param::integer("cell_px").min(4.0).max(200.0).default(24).describe("Pixel size of each matrix cell (4-200, default 24)."))
        .param(Param::number("scale_min").describe("Optional fixed color-scale minimum. When omitted, the global minimum over all frames is used (so colors are comparable across frames)."))
        .param(Param::number("scale_max").describe("Optional fixed color-scale maximum. When omitted, the global maximum over all frames is used. Must be greater than scale_min when both are set."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AnimatedHeatmap;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/animated-heatmap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a sequence of matrices as an animated heatmap GIF",
    skill(
        description = "Render a time-ordered sequence of numeric matrices as an animated heatmap GIF showing how the values evolve over time. Each matrix is one frame (rows of comma/space-separated numbers, one row per line); separate frames with a BLANK line. All frames must share the same dimensions. A single global color scale (blue=low -> yellow -> red=high) is applied across every frame so equal values keep the same color, making the animation a meaningful comparison; set scale_min/scale_max to fix the bounds instead. delay_ms sets the per-frame delay and cell_px the size of each cell. Loops forever.",
        parameters = schema_json()
    )
)]
impl AnimatedHeatmap {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("animated-heatmap")?;
    let delay = args.delay_ms.clamp(10, 60_000) as u16;
    let cell = args.cell_px.clamp(4, 200) as u32;
    let gif = animated_heatmap(&args.data, delay, cell, args.scale_min, args.scale_max)
        .map_err(SkillError::InvalidArgs)?;
    let nbytes = gif.len();
    build_media_envelope(
        &gif,
        "image/gif",
        "animated-heatmap.gif".to_string(),
        format!("rendered an animated heatmap GIF ({nbytes} bytes, {delay}ms/frame)"),
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
                    "data":      { "type": "string", "description": "The time-ordered matrices: each frame is one matrix (rows of comma/space-separated numbers, one row per line); separate consecutive frames with a BLANK line. Every frame must have the same dimensions." },
                    "delay_ms":  { "type": "integer", "minimum": 10, "maximum": 60000, "default": 400, "description": "Delay per frame in milliseconds (10-60000, default 400)." },
                    "cell_px":   { "type": "integer", "minimum": 4, "maximum": 200, "default": 24, "description": "Pixel size of each matrix cell (4-200, default 24)." },
                    "scale_min": { "type": "number", "description": "Optional fixed color-scale minimum. When omitted, the global minimum over all frames is used (so colors are comparable across frames)." },
                    "scale_max": { "type": "number", "description": "Optional fixed color-scale maximum. When omitted, the global maximum over all frames is used. Must be greater than scale_min when both are set." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
