//! gizza-ai/animated-webp-to-frames — split an animated WebP into its individual
//! frame images, returned as a ZIP of PNG/JPG/WebP stills plus a `manifest.json`
//! with the frame order and per-frame timing.
//!
//! Pipeline: resolve the source WebP via `block-utils` `resolve_source` (URL fetch
//! through `wafer-run/network`, or an uploaded attachment `ref`) → the pure
//! `animated-webp-to-frames-core` (`image` WebP decode — its frame iterator
//! coalesces each ANMF sub-rectangle onto the canvas, applying blend + disposal
//! — → PNG/JPEG/lossless-WebP per frame → `zip`) → base64 `{_for_llm, _for_ui}`
//! envelope carrying a `data:application/zip` URL.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a ZIP-of-images output fits neither the
//! pure-text nor the ffmpeg media page shape — the same "no-page file-input"
//! pattern as gif-extract-frames / collage-splitter / extract-pdf-images).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_animated_webp_to_frames_core::{Manifest, FRAME_CAP};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MiB — a long, high-res WebP.
const MAX_OUTPUT_BYTES: usize = 96 * 1024 * 1024; // PNG frames are far larger than the WebP.

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    max_frames: Option<u32>,
    #[serde(default)]
    format: Option<String>,
}

/// `Input::Image` emits the scalar `url`⊕`ref` `oneOf`; the three knobs map 1:1
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
                    "Stop after this many frames (1-500, default 500, which is also the hard cap). Longer animations are truncated and the manifest records the source's real frame count.",
                ),
        )
        .param(
            Param::enumv("format", ["png", "jpg", "webp"])
                .default("png")
                .describe(
                    "Image format for each extracted frame: png (lossless, keeps transparency — the default), jpg (much smaller but lossy, and transparent pixels are flattened onto white because JPEG has no alpha channel), or webp (lossless, keeps transparency, smaller than png).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// `cat.webp` → `cat-frames.zip`; a nameless source falls back to
/// `webp-frames.zip`.
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
        "webp-frames.zip".to_string()
    } else {
        format!("{stem}-frames.zip")
    }
}

/// One exact, useful LLM/CLI line describing what was produced.
fn summary_line(out_filename: &str, zip_len: usize, m: &Manifest) -> String {
    let truncated = if m.truncated {
        format!(" (truncated from {} frames in the source)", m.total_frames)
    } else {
        String::new()
    };
    let kind = if m.animated {
        "animated"
    } else {
        "still (non-animated)"
    };
    let ext = m.format.ext();
    format!(
        "Extracted {} frame(s) from {} {} at {}x{}{} → {} ({}-byte ZIP of {} images + manifest.json; {} ms total playback).",
        m.frame_count,
        kind,
        m.source,
        m.width,
        m.height,
        truncated,
        out_filename,
        zip_len,
        ext.to_uppercase(),
        m.total_duration_ms
    )
}

#[cfg(target_arch = "wasm32")]
struct AnimatedWebpToFrames;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/animated-webp-to-frames",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split an animated WebP into its individual frame images (ZIP of PNGs)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Split an animated WebP into its individual frames, returned as a ZIP of images plus a manifest.json carrying the timing. An animated WebP stores each frame as an ANMF sub-rectangle with its own blend and disposal method; every frame is fully composited (coalesced) onto the canvas first, so frames come out as complete pictures instead of the small patches the file actually holds, and transparency is preserved in the alpha channel. format picks the frame image type: png (lossless, keeps transparency, default), jpg (smaller but lossy, and transparent pixels are flattened onto white), or webp (lossless, keeps transparency). prefix names the files (frame -> frame-0001.png, frame-0002.png, … 1-based, zero-padded, in playback order; default 'frame'). max_frames stops extraction early (1-500, default 500, also the hard cap) — the manifest still records the source's real frame count and flags the truncation. manifest.json lists the canvas width/height, whether the source was animated, the frame count, total playback duration, and every frame's index, filename, delay in milliseconds and start time in the animation. A still (non-animated) WebP yields one image. Provide the WebP as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl AnimatedWebpToFrames {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_animated_webp_to_frames_core::{ExtractParams, FrameFormat};
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("animated-webp-to-frames")?;
    let format = FrameFormat::parse(args.format.as_deref().unwrap_or("png"))
        .map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let params = ExtractParams {
        prefix: args.prefix.unwrap_or_else(|| "frame".to_string()),
        max_frames: args.max_frames.unwrap_or(FRAME_CAP),
        format,
        source_name: in_filename.clone(),
    };

    let (zip, manifest) = gizza_ai_animated_webp_to_frames_core::extract_frames(&bytes, &params)
        .map_err(SkillError::InvalidArgs)?;

    let zip_len = zip.len();
    if zip_len > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "the extracted frames total {zip_len} bytes, over the {MAX_OUTPUT_BYTES}-byte limit — lower max_frames, pick format=jpg, or resize the WebP first"
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
    use gizza_ai_animated_webp_to_frames_core::FrameFormat;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Image url⊕ref oneOf + prefix/max_frames/format).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "prefix": { "type": "string", "default": "frame", "description": "Filename base for each extracted frame, e.g. frame -> frame-0001.png, frame-0002.png. Numbering is 1-based, zero-padded to 4 digits, in playback order. Default 'frame'." },
                    "max_frames": { "type": "integer", "minimum": 1, "maximum": 500, "default": 500, "description": "Stop after this many frames (1-500, default 500, which is also the hard cap). Longer animations are truncated and the manifest records the source's real frame count." },
                    "format": { "type": "string", "enum": ["png", "jpg", "webp"], "default": "png", "description": "Image format for each extracted frame: png (lossless, keeps transparency — the default), jpg (much smaller but lossy, and transparent pixels are flattened onto white because JPEG has no alpha channel), or webp (lossless, keeps transparency, smaller than png)." }
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
        assert_eq!(output_filename("cat.webp"), "cat-frames.zip");
        assert_eq!(
            output_filename("/tmp/loop.anim.webp"),
            "loop.anim-frames.zip"
        );
        assert_eq!(output_filename("noext"), "noext-frames.zip");
        assert_eq!(output_filename(".webp"), "webp-frames.zip");
    }

    fn manifest(format: FrameFormat, animated: bool, truncated: bool) -> Manifest {
        Manifest {
            source: "cat.webp".into(),
            width: 320,
            height: 240,
            animated,
            format,
            frame_count: 3,
            total_frames: if truncated { 1200 } else { 3 },
            truncated,
            total_duration_ms: 350,
            frames: vec![],
        }
    }

    #[test]
    fn summary_line_is_exact_and_useful() {
        assert_eq!(
            summary_line("cat-frames.zip", 4096, &manifest(FrameFormat::Png, true, false)),
            "Extracted 3 frame(s) from animated cat.webp at 320x240 → cat-frames.zip (4096-byte ZIP of PNG images + manifest.json; 350 ms total playback)."
        );
    }

    #[test]
    fn summary_line_names_the_chosen_format() {
        let line = summary_line("cat-frames.zip", 99, &manifest(FrameFormat::Jpg, true, false));
        assert!(line.contains("ZIP of JPG images"), "{line}");
    }

    #[test]
    fn summary_line_flags_a_still_source() {
        let line = summary_line("cat-frames.zip", 99, &manifest(FrameFormat::Png, false, false));
        assert!(line.contains("from still (non-animated) cat.webp"), "{line}");
    }

    #[test]
    fn summary_line_reports_truncation() {
        let line = summary_line("long-frames.zip", 9, &manifest(FrameFormat::Png, true, true));
        assert!(
            line.contains("(truncated from 1200 frames in the source)"),
            "{line}"
        );
    }
}
