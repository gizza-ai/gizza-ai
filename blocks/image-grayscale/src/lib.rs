//! gizza-ai/image-grayscale — fetch an image URL or attachment ref, convert to
//! grayscale via ffmpeg, return envelope.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. The block-local helpers
// remain native-compilable so the unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, filename_with_suffix, mime_to_ext, AssetKind, Envelope, FfmpegReq,
    FfmpegResp, ForUi, SkillError, SkillResultExt, Source, SourceFields,
};
use gizza_ai_image_grayscale_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[cfg(target_arch = "wasm32")]
struct ImageGrayscale;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-grayscale",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an image to grayscale",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Convert an image to grayscale. Provide url (HTTP/HTTPS) or ref from a prior image tool call.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Image URL (HTTP/HTTPS)." },
                "ref": { "type": "string", "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref." }
            },
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl ImageGrayscale {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("image-grayscale")?;

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Image, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Image, MAX_INPUT_BYTES)?,
    };

    // 3. Build ffmpeg argv via core::plan.
    let ext = mime_to_ext(&mime).ok_or_else(|| {
        SkillError::InvalidArgs(format!("unsupported input mime: {mime}"))
    })?;
    let ffmpeg_in = format!("in.{ext}");
    let (argv, ffmpeg_out) = plan(&ffmpeg_in).map_err(SkillError::InvalidArgs)?;

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

    // 5. Envelope.
    let output_size = ff.output.len();
    let encoded = B64.encode(&ff.output);
    let data_url = format!("data:{mime};base64,{encoded}");
    let filename = filename_with_suffix(&in_filename, "-gray", ext);
    let env = Envelope {
        for_llm: format!("converted {in_filename} to grayscale ({output_size} bytes, {mime})"),
        for_ui: ForUi {
            data_url,
            mime,
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_with_gray_suffix() {
        assert_eq!(filename_with_suffix("cat.png", "-gray", "png"), "cat-gray.png");
    }

    #[test]
    fn filename_with_gray_suffix_jpg() {
        assert_eq!(filename_with_suffix("photo.jpg", "-gray", "jpg"), "photo-gray.jpg");
    }
}
