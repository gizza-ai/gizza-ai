//! gizza-ai/video-transcode — fetch a video URL or attachment ref, transcode to a target container.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, mime_to_ext, pick_source, AssetKind, Envelope, FfmpegReq, FfmpegResp,
    ForUi, SkillError, SkillResultExt, Source,
};
use serde::Deserialize;
use wafer_sdk::*;

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;
const DEFAULT_QUALITY: u8 = 75;

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

/// Map web-conventional quality 1-100 to ffmpeg's CRF range 0 (best) – 51 (worst).
fn quality_to_crf(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let crf = 51.0 - (q - 1.0) * (51.0 / 99.0);
    crf.round().clamp(0.0, 51.0) as u8
}

fn format_to_mime_and_ext(fmt: &str) -> Option<(&'static str, &'static str)> {
    match fmt {
        "mp4" => Some(("video/mp4", "mp4")),
        "webm" => Some(("video/webm", "webm")),
        _ => None,
    }
}

fn build_argv(in_name: &str, out_name: &str, format: &str, crf: u8) -> Vec<String> {
    match format {
        "mp4" => vec![
            "-i".into(),
            in_name.into(),
            "-c:v".into(),
            "libx264".into(),
            "-c:a".into(),
            "aac".into(),
            "-crf".into(),
            crf.to_string(),
            "-movflags".into(),
            "+faststart".into(),
            out_name.into(),
        ],
        "webm" => vec![
            "-i".into(),
            in_name.into(),
            "-c:v".into(),
            "libvpx-vp9".into(),
            "-c:a".into(),
            "libopus".into(),
            "-crf".into(),
            crf.to_string(),
            "-b:v".into(),
            "0".into(),
            out_name.into(),
        ],
        _ => Vec::new(),
    }
}

fn output_filename(in_filename: &str, out_ext: &str) -> String {
    let stem = in_filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(in_filename);
    format!("{stem}.{out_ext}")
}

#[cfg(target_arch = "wasm32")]
struct VideoTranscode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-transcode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Transcode a video to a different container format",
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Transcode a video to a different format (mp4 or webm). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Quality 1-100 maps to ffmpeg CRF (default 75).",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":     { "type": "string", "description": "Video URL (HTTP/HTTPS)." },
                "ref":     { "type": "string", "description": "Reference id from a prior tool call (e.g. \"call_42\"). Use either url or ref." },
                "format":  { "type": "string", "enum": ["mp4", "webm"], "description": "Output container format." },
                "quality": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Quality 1-100 (default 75). Lower = smaller file, lower quality." }
            },
            "required": ["format"],
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl VideoTranscode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-transcode")?;
    let (out_mime, out_ext) = format_to_mime_and_ext(&args.format).ok_or_else(|| {
        SkillError::InvalidArgs(format!(
            "invalid video-transcode args: format {:?} not supported (mp4|webm)",
            args.format
        ))
    })?;
    if let Some(q) = args.quality {
        if !(1..=100).contains(&q) {
            return Err(SkillError::InvalidArgs(format!(
                "invalid video-transcode args: quality must be 1-100, got {q}"
            )));
        }
    }
    let crf = quality_to_crf(args.quality.unwrap_or(DEFAULT_QUALITY));

    let (input_bytes, in_mime, in_filename) =
        match pick_source(args.url.as_deref(), args.r#ref.as_deref()).invalid_args("video-transcode")? {
            Source::Url(u) => fetch_from_url(&u, AssetKind::Video, MAX_INPUT_BYTES)?,
            Source::Ref(id) => load_from_attachment(&id, AssetKind::Video, MAX_INPUT_BYTES)?,
        };

    let in_ext = mime_to_ext(&in_mime).unwrap_or("bin");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = format!("out.{out_ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, &args.format, crf);

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
            kind: "output video",
            bytes: ff.output.len(),
            cap: MAX_OUTPUT_BYTES,
        });
    }

    let output_size = ff.output.len();
    let encoded = B64.encode(&ff.output);
    let data_url = format!("data:{out_mime};base64,{encoded}");
    let filename = output_filename(&in_filename, out_ext);
    let env = Envelope {
        for_llm: format!(
            "transcoded {} from {} to {} ({} bytes)",
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
    fn quality_to_crf_endpoints() {
        assert_eq!(quality_to_crf(100), 0);
        assert_eq!(quality_to_crf(1), 51);
        let mid = quality_to_crf(75);
        assert!((12..=14).contains(&mid), "expected 12-14, got {mid}");
    }

    #[test]
    fn argv_mp4_default() {
        let argv = build_argv("in.mp4", "out.mp4", "mp4", 13);
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert!(argv.iter().any(|a| a == "libx264"));
        assert!(argv.iter().any(|a| a == "aac"));
        assert!(argv.iter().any(|a| a == "13"));
        assert!(argv.iter().any(|a| a == "+faststart"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn argv_webm_default() {
        let argv = build_argv("in.mp4", "out.webm", "webm", 13);
        assert!(argv.iter().any(|a| a == "libvpx-vp9"));
        assert!(argv.iter().any(|a| a == "libopus"));
        assert!(argv.iter().any(|a| a == "13"));
        assert!(argv.iter().any(|a| a == "0")); // -b:v 0
        assert_eq!(argv.last().map(String::as_str), Some("out.webm"));
    }
}
