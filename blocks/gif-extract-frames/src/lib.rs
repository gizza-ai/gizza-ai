//! gizza-ai/gif-extract-frames — split an animated GIF into its individual frame
//! images, returned as a ZIP of PNGs plus a `manifest.json` with the frame order
//! and per-frame delays.
//!
//! Pipeline: resolve the source GIF via `block-utils` `resolve_source` (URL fetch
//! through `wafer-run/network`, or an uploaded attachment `ref`) → the pure
//! `gif-extract-frames-core` (`image` GIF decode — its frame iterator coalesces
//! partial/optimized frames onto the canvas — → PNG per frame → `zip`) →
//! base64 `{_for_llm, _for_ui}` envelope carrying a `data:application/zip` URL.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP-of-images output fits neither the
//! pure-text nor the ffmpeg media page shape — the same "no-page file-input"
//! pattern as collage-splitter / file-splitter / extract-pdf-images).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_gif_extract_frames_core::{Manifest, FRAME_CAP};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MiB — a long, high-res GIF.
const MAX_OUTPUT_BYTES: usize = 96 * 1024 * 1024; // PNG frames are much larger than the GIF.

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    max_frames: Option<u32>,
}

/// `Input::Image` emits the scalar `url`⊕`ref` `oneOf`; the two knobs map 1:1
/// onto `core::ExtractParams`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("prefix")
                .default("frame")
                .describe(
                    "Filename base for each extracted frame, e.g. frame -> frame-0001.png, frame-0002.png. Numbering is 1-based, zero-padded to 4 digits, in playback order. Default 'frame'.",
                ),
        )
        .param(
            Param::integer("max_frames")
                .min(1.0)
                .max(500.0)
                .default(500)
                .describe(
                    "Stop after this many frames (1-500, default 500, which is also the hard cap). Longer GIFs are truncated and the manifest records the source's real frame count.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// `cat.gif` → `cat-frames.zip`; a nameless source falls back to `gif-frames.zip`.
fn output_filename(in_filename: &str) -> String {
    let stem = in_filename
        .rsplit('/')
        .next()
        .unwrap_or(in_filename)
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(in_filename)
        .trim();
    if stem.is_empty() {
        "gif-frames.zip".to_string()
    } else {
        format!("{stem}-frames.zip")
    }
}

/// One exact, useful LLM/CLI line describing what was produced.
fn summary_line(out_filename: &str, zip_len: usize, m: &Manifest) -> String {
    let truncated = if m.truncated {
        format!(
            " (truncated from {} frames in the source)",
            m.total_frames
        )
    } else {
        String::new()
    };
    format!(
        "Extracted {} frame(s) from {} at {}x{}{} → {} ({}-byte ZIP of PNGs + manifest.json; {} ms total playback).",
        m.frame_count, m.source, m.width, m.height, truncated, out_filename, zip_len, m.total_duration_ms
    )
}

#[cfg(target_arch = "wasm32")]
struct GifExtractFrames;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gif-extract-frames",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split an animated GIF into its individual frame images (ZIP of PNGs)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Split an animated GIF into its individual frames, returned as a ZIP of PNG images plus a manifest.json. Every frame is fully composited (coalesced/unoptimized) to the GIF's canvas size with its disposal method applied, so partial frames come out as complete pictures instead of sliver artifacts, and transparency is preserved in the PNG alpha channel. prefix names the files (frame -> frame-0001.png, frame-0002.png, … 1-based, zero-padded, in playback order; default 'frame'). max_frames stops extraction early (1-500, default 500, also the hard cap) — the manifest still records the source's real frame count and flags the truncation. manifest.json lists the canvas width/height, frame count, total playback duration, and every frame's index, filename and delay in milliseconds. A static single-frame GIF yields one PNG. Provide the GIF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl GifExtractFrames {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;
    use gizza_ai_gif_extract_frames_core::ExtractParams;

    let args: Args = serde_json::from_slice(&body).invalid_args("gif-extract-frames")?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let params = ExtractParams {
        prefix: args.prefix.unwrap_or_else(|| "frame".to_string()),
        max_frames: args.max_frames.unwrap_or(FRAME_CAP),
        source_name: in_filename.clone(),
    };

    let (zip, manifest) = gizza_ai_gif_extract_frames_core::extract_frames(&bytes, &params)
        .map_err(SkillError::InvalidArgs)?;

    let zip_len = zip.len();
    if zip_len > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "the extracted frames total {zip_len} bytes, over the {MAX_OUTPUT_BYTES}-byte limit — lower max_frames or resize the GIF first"
        )));
    }

    let filename = output_filename(&in_filename);
    let encoded = B64.encode(&zip);
    let data_url = format!("data:application/zip;base64,{encoded}");
    let for_llm = summary_line(&filename, zip_len, &manifest);

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/zip".to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Image url⊕ref oneOf + prefix/max_frames).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "prefix": { "type": "string", "default": "frame", "description": "Filename base for each extracted frame, e.g. frame -> frame-0001.png, frame-0002.png. Numbering is 1-based, zero-padded to 4 digits, in playback order. Default 'frame'." },
                    "max_frames": { "type": "integer", "minimum": 1, "maximum": 500, "default": 500, "description": "Stop after this many frames (1-500, default 500, which is also the hard cap). Longer GIFs are truncated and the manifest records the source's real frame count." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for (name, prop) in schema["properties"].as_object().unwrap() {
            assert!(
                prop["description"].as_str().is_some_and(|d| d.len() > 20),
                "param {name} needs a useful .describe()"
            );
        }
    }

    #[test]
    fn output_filename_derives_from_the_source() {
        assert_eq!(output_filename("cat.gif"), "cat-frames.zip");
        assert_eq!(output_filename("/tmp/loop.anim.gif"), "loop.anim-frames.zip");
        assert_eq!(output_filename("noext"), "noext-frames.zip");
        assert_eq!(output_filename(".gif"), "gif-frames.zip");
    }

    #[test]
    fn summary_line_is_exact_and_useful() {
        let m = Manifest {
            source: "cat.gif".into(),
            width: 320,
            height: 240,
            frame_count: 3,
            total_frames: 3,
            truncated: false,
            total_duration_ms: 350,
            frames: vec![],
        };
        assert_eq!(
            summary_line("cat-frames.zip", 4096, &m),
            "Extracted 3 frame(s) from cat.gif at 320x240 → cat-frames.zip (4096-byte ZIP of PNGs + manifest.json; 350 ms total playback)."
        );
    }

    #[test]
    fn summary_line_reports_truncation() {
        let m = Manifest {
            source: "long.gif".into(),
            width: 100,
            height: 100,
            frame_count: 500,
            total_frames: 1200,
            truncated: true,
            total_duration_ms: 25_000,
            frames: vec![],
        };
        assert!(summary_line("long-frames.zip", 9, &m)
            .contains("(truncated from 1200 frames in the source)"));
    }
}
