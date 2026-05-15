//! gizza-ai/image-crop — fetch an image URL or attachment ref, crop a rectangle via ffmpeg.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, mime_to_ext, AssetKind, Envelope, FfmpegReq, FfmpegResp, ForUi,
    SkillError, SkillResultExt, Source, SourceFields,
};
use serde::Deserialize;
use wafer_sdk::*;

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn build_argv(in_name: &str, out_name: &str, x: u32, y: u32, w: u32, h: u32) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        format!("crop={w}:{h}:{x}:{y}"),
        out_name.into(),
    ]
}

fn output_filename(input_filename: &str, ext: &str) -> String {
    let base = input_filename.rsplitn(2, '.').last().unwrap_or(input_filename);
    format!("{base}-cropped.{ext}")
}

#[cfg(target_arch = "wasm32")]
struct ImageCrop;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-crop",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Crop a rectangular region from an image fetched by URL or from a prior tool call ref",
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Crop a rectangular region from an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS)." },
                "ref":    { "type": "string", "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref." },
                "x":      { "type": "integer", "minimum": 0, "description": "Left offset of the crop rectangle in pixels." },
                "y":      { "type": "integer", "minimum": 0, "description": "Top offset of the crop rectangle in pixels." },
                "width":  { "type": "integer", "minimum": 1, "description": "Width of the crop rectangle in pixels." },
                "height": { "type": "integer", "minimum": 1, "description": "Height of the crop rectangle in pixels." }
            },
            "required": ["x", "y", "width", "height"],
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl ImageCrop {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-crop")?;
    if args.width == 0 || args.height == 0 {
        return Err(SkillError::InvalidArgs(
            "invalid image-crop args: width and height must be > 0".into(),
        ));
    }

    let (input_bytes, mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Image, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Image, MAX_INPUT_BYTES)?,
    };

    let ext = mime_to_ext(&mime).ok_or_else(|| {
        SkillError::InvalidArgs(format!("unsupported input mime: {mime}"))
    })?;
    let ffmpeg_in = format!("in.{ext}");
    let ffmpeg_out = format!("out.{ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.x, args.y, args.width, args.height);

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

    let output_size = ff.output.len();
    let encoded = B64.encode(&ff.output);
    let data_url = format!("data:{mime};base64,{encoded}");
    let filename = output_filename(&in_filename, ext);
    let env = Envelope {
        for_llm: format!(
            "cropped {} at ({},{}) {}x{} ({} {})",
            in_filename, args.x, args.y, args.width, args.height, output_size, mime
        ),
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
    fn argv_crop_positional_order() {
        let argv = build_argv("in.png", "out.png", 10, 20, 200, 300);
        assert_eq!(argv, vec![
            "-i".to_string(),
            "in.png".to_string(),
            "-vf".to_string(),
            "crop=200:300:10:20".to_string(),
            "out.png".to_string(),
        ]);
    }

    #[test]
    fn output_filename_appends_cropped_suffix() {
        assert_eq!(output_filename("cat.png", "png"), "cat-cropped.png");
    }
}
