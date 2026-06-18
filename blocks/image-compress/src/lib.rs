//! gizza-ai/image-compress — fetch an image URL or attachment ref, re-encode it
//! at a chosen quality to shrink its file size, keeping the SAME format.

// The #[wafer_block] macro emits the impl gated to wasm32; the supporting
// imports, constants, and the Args type are only used there. See image-resize
// for the full rationale. The argv-building logic lives in `core` (the single
// source shared with the standalone web page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, filename_with_suffix, mime_to_ext, validate_quality_1_100, AssetKind,
    Envelope, FfmpegReq, FfmpegResp, ForUi, SkillError, SkillResultExt, Source, SourceFields,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};
use gizza_ai_image_compress_core::plan_compress;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_QUALITY: u8 = 80;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    quality: Option<u8>,
}

#[cfg(target_arch = "wasm32")]
struct ImageCompress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-encode an image at a chosen quality to shrink its file size, keeping the same format",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Compress (re-encode) an image at a chosen quality to shrink its file size, keeping the same format (jpg/png/webp). Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call). Lower quality = smaller file. For JPEG/WebP this trades visual fidelity for size; PNG is lossless so quality only tunes compression effort.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":     { "type": "string", "description": "Image URL (HTTP/HTTPS). PNG, JPEG, or WebP." },
                "ref":     { "type": "string", "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref." },
                "quality": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Output quality 1-100 (default 80). Lower = smaller file." }
            },
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl ImageCompress {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("image-compress")?;
    validate_quality_1_100(args.quality, "image-compress")?;
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Image, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Image, MAX_INPUT_BYTES)?,
    };
    let input_size = input_bytes.len();

    // 3. Build ffmpeg argv via the shared core (keeps the same format, infers
    //    the encoder flag from the input extension).
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{ext}");
    let (argv, ffmpeg_out) = plan_compress(quality, &ffmpeg_in).map_err(SkillError::InvalidArgs)?;

    // 4. Call ffmpeg-runtime.
    let req = FfmpegReq {
        args: argv,
        inputs: vec![(ffmpeg_in, input_bytes)],
        output: ffmpeg_out,
    };
    let req_body = serde_json::to_vec(&req)
        .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let ff_resp_bytes = dispatch_ffmpeg_runtime(&req_body)?;
    let ff: FfmpegResp = serde_json::from_slice(&ff_resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;

    if ff.exit_code != 0 {
        let snippet: String = ff.log.chars().take(200).collect();
        return Err(SkillError::FfmpegExitNonZero {
            exit: ff.exit_code,
            snippet,
        });
    }
    if ff.output.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::TooLarge {
            kind: "output image",
            bytes: ff.output.len(),
            cap: MAX_OUTPUT_BYTES,
        });
    }

    // 5. Envelope (same mime as the input — format is unchanged).
    let output_size = ff.output.len();
    let encoded = B64.encode(&ff.output);
    let data_url = format!("data:{mime};base64,{encoded}");
    let filename = filename_with_suffix(&in_filename, "-compressed", ext);
    let env = Envelope {
        for_llm: summary(&in_filename, quality, input_size, output_size, &mime),
        for_ui: ForUi {
            data_url,
            mime,
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

/// One-line summary for the LLM: what was compressed, at what quality, and the
/// byte delta.
fn summary(source: &str, quality: u8, input_size: usize, output_size: usize, mime: &str) -> String {
    format!("compressed {source} at quality {quality}: {input_size} → {output_size} bytes ({mime})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_reports_byte_delta() {
        let s = summary("cat.jpg", 60, 12000, 7000, "image/jpeg");
        assert!(s.contains("cat.jpg"));
        assert!(s.contains("quality 60"));
        assert!(s.contains("12000"));
        assert!(s.contains("7000"));
    }

    #[test]
    fn compressed_filename_keeps_extension() {
        assert_eq!(
            filename_with_suffix("cat.png", "-compressed", "png"),
            "cat-compressed.png"
        );
        assert_eq!(
            filename_with_suffix("photo.jpg", "-compressed", "jpg"),
            "photo-compressed.jpg"
        );
    }
}
