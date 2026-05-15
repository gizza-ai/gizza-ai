//! gizza-ai/image-convert — fetch an image URL or attachment ref, transcode to a different format.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, mime_to_ext, pick_source, replace_extension, validate_quality_1_100,
    AssetKind, Envelope, FfmpegReq, FfmpegResp, ForUi, SkillError, SkillResultExt, Source,
};
use serde::Deserialize;
use wafer_sdk::*;

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_QUALITY: u8 = 85;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    r#ref: Option<String>,
    format: String,
    #[serde(default)]
    quality: Option<u8>,
}

/// Map web-conventional quality 1-100 to ffmpeg's -q:v range 31 (worst) – 2 (best).
fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

fn format_to_mime_and_ext(fmt: &str) -> Option<(&'static str, &'static str)> {
    match fmt {
        "jpeg" => Some(("image/jpeg", "jpg")),
        "png"  => Some(("image/png",  "png")),
        "webp" => Some(("image/webp", "webp")),
        _ => None,
    }
}

fn build_argv(in_name: &str, out_name: &str, format: &str, quality: u8) -> Vec<String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    if format != "png" {
        argv.push("-q:v".into());
        argv.push(quality_to_qv(quality).to_string());
    }
    argv.push(out_name.to_string());
    argv
}

#[cfg(target_arch = "wasm32")]
struct ImageConvert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an image to a different format",
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert an image to a different format (jpeg, png, webp). Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":     { "type": "string", "description": "Image URL (HTTP/HTTPS)." },
                "ref":     { "type": "string", "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref." },
                "format":  { "type": "string", "enum": ["jpeg", "png", "webp"], "description": "Target image format." },
                "quality": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Output quality 1-100 (default 85, ignored for png)." }
            },
            "required": ["format"],
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl ImageConvert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-convert")?;
    let (out_mime, out_ext) = format_to_mime_and_ext(&args.format).ok_or_else(|| {
        SkillError::InvalidArgs(format!(
            "invalid image-convert args: format {:?} not supported (jpeg|png|webp)",
            args.format
        ))
    })?;
    validate_quality_1_100(args.quality, "image-convert")?;
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);

    let (input_bytes, in_mime, in_filename) =
        match pick_source(args.url.as_deref(), args.r#ref.as_deref()).invalid_args("image-convert")? {
            Source::Url(u) => fetch_from_url(&u, AssetKind::Image, MAX_INPUT_BYTES)?,
            Source::Ref(id) => load_from_attachment(&id, AssetKind::Image, MAX_INPUT_BYTES)?,
        };

    let in_ext = mime_to_ext(&in_mime).ok_or_else(|| {
        SkillError::InvalidArgs(format!("unsupported input mime: {in_mime}"))
    })?;
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, &args.format, quality);

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
    let data_url = format!("data:{out_mime};base64,{encoded}");
    let filename = replace_extension(&in_filename, out_ext);
    let env = Envelope {
        for_llm: format!(
            "converted {} from {} to {} ({})",
            in_filename, in_mime, out_mime, output_size
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
    fn quality_85_maps_to_qv_about_6() {
        let qv = quality_to_qv(85);
        assert!((5..=7).contains(&qv), "expected 5-7, got {qv}");
    }

    #[test]
    fn quality_100_maps_to_qv_2_best() {
        assert_eq!(quality_to_qv(100), 2);
    }

    #[test]
    fn quality_1_maps_to_qv_31_worst() {
        assert_eq!(quality_to_qv(1), 31);
    }

    #[test]
    fn argv_jpeg_includes_qv() {
        let argv = build_argv("in.png", "out.jpg", "jpeg", 85);
        assert!(argv.iter().any(|a| a == "-q:v"));
        let idx = argv.iter().position(|a| a == "-q:v").unwrap();
        let qv: u8 = argv[idx + 1].parse().unwrap();
        assert!((5..=7).contains(&qv));
    }

    #[test]
    fn argv_png_omits_qv() {
        let argv = build_argv("in.jpg", "out.png", "png", 85);
        assert!(!argv.iter().any(|a| a == "-q:v"), "png argv should not include -q:v: {argv:?}");
    }

    #[test]
    fn argv_webp_includes_qv() {
        let argv = build_argv("in.png", "out.webp", "webp", 50);
        assert!(argv.iter().any(|a| a == "-q:v"));
    }

}
