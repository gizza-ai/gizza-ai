//! gizza-ai/heic-to-jpg — fetch an HEIC/HEIF photo by URL or attachment ref,
//! decode + re-encode it to JPEG or PNG via ffmpeg, return an image envelope.
//!
//! FEASIBILITY: HEIC decoding needs an ffmpeg with HEIF support (7.0+ native
//! `heif` demuxer, or `--enable-libheif`). When the runtime ffmpeg lacks it the
//! decode fails ("moov atom not found"), which surfaces as a non-zero ffmpeg
//! exit here. See the PR for the support matrix across this stack's ffmpeg builds.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, replace_extension, AssetKind, Envelope, FfmpegReq, FfmpegResp, ForUi,
    SkillError, SkillResultExt, Source, SourceFields,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};
use gizza_ai_heic_to_jpg_core::{parse_format, plan};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024; // HEIC photos run larger than JPEG; 8 MiB
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024; // decoded PNG can be much larger than the HEIC

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    format: Option<String>,
}

#[cfg(target_arch = "wasm32")]
struct HeicToJpg;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/heic-to-jpg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an Apple HEIC/HEIF photo to JPEG or PNG",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Convert an Apple HEIC/HEIF photo to JPEG (default) or PNG. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":    { "type": "string", "description": "HEIC/HEIF image URL (HTTP/HTTPS)." },
                "ref":    { "type": "string", "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref." },
                "format": { "type": "string", "enum": ["jpg", "png"], "description": "Target format (default: jpg)." }
            },
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl HeicToJpg {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("heic-to-jpg")?;
    let fmt = parse_format(args.format.as_deref()).invalid_args("heic-to-jpg")?;

    // 2. Resolve source — URL fetch or attachment lookup. HEIC is an image.
    let (input_bytes, _in_mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Image, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Image, MAX_INPUT_BYTES)?,
    };

    // 3. Build ffmpeg argv (decode HEIC → encode jpg/png). The input is always
    //    treated as HEIC in ffmpeg's virtual FS; ffmpeg infers the demuxer from
    //    content, not the name, so a fixed `in.heic` is fine.
    let ffmpeg_in = "in.heic";
    let (argv, ffmpeg_out) = plan(ffmpeg_in, fmt);

    // 4. Call ffmpeg-runtime.
    let req = FfmpegReq {
        args: argv,
        inputs: vec![(ffmpeg_in.to_string(), input_bytes)],
        output: ffmpeg_out.clone(),
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
    let out_mime = fmt.mime();
    let encoded = B64.encode(&ff.output);
    let data_url = format!("data:{out_mime};base64,{encoded}");
    let filename = replace_extension(&in_filename, fmt.ext());
    let env = Envelope {
        for_llm: format!(
            "converted {} from HEIC to {} ({} bytes)",
            in_filename, out_mime, output_size
        ),
        for_ui: ForUi {
            data_url,
            mime: out_mime.to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_swaps_extension_to_chosen_format() {
        assert_eq!(replace_extension("IMG_0042.heic", "jpg"), "IMG_0042.jpg");
        assert_eq!(replace_extension("photo.HEIC", "png"), "photo.png");
    }
}
