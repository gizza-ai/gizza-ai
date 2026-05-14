//! gizza-ai/image-crop — fetch an image URL or attachment ref, crop a rectangle via ffmpeg.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    derive_filename, dispatch_ffmpeg_runtime, mime_to_ext, pick_source, Envelope, FfmpegReq,
    FfmpegResp, ForUi, Source,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    r#ref: Option<String>,
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
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("invalid image-crop args: {e}"),
            )),
        };
        if args.width == 0 || args.height == 0 {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                "invalid image-crop args: width and height must be > 0",
            ));
        }

        // Resolve source — URL fetch or attachment lookup.
        let (input_bytes, mime, in_filename) = match pick_source(args.url.as_deref(), args.r#ref.as_deref()) {
            Err(e) => return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("invalid image-crop args: {e}"),
            )),
            Ok(Source::Url(u)) => {
                let net = match wafer_sdk::clients::network::do_request("GET", &u, &HashMap::new(), None) {
                    Ok(r) => r,
                    Err(e) => return GuestResult::error(e),
                };
                if net.status_code >= 400 {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::UNAVAILABLE,
                        format!("HTTP {} for {}", net.status_code, u),
                    ));
                }
                let raw_mime = net.headers.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .and_then(|(_, vs)| vs.first().cloned())
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                let mime: String = raw_mime.split(';').next().unwrap_or("").trim().to_lowercase();
                if !mime.starts_with("image/") {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::INVALID_ARGUMENT,
                        format!("expected image/* content-type, got {mime}"),
                    ));
                }
                if let Some(cl) = net.headers.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, vs)| vs.first())
                    .and_then(|v| v.trim().parse::<usize>().ok())
                {
                    if cl > MAX_INPUT_BYTES {
                        return GuestResult::error(WaferError::new(
                            ErrorCode::OUT_OF_RANGE,
                            format!("input image too large: {cl} bytes (cap {MAX_INPUT_BYTES} bytes)"),
                        ));
                    }
                }
                if net.body.len() > MAX_INPUT_BYTES {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::OUT_OF_RANGE,
                        format!("input image too large: {} bytes (cap {} bytes)", net.body.len(), MAX_INPUT_BYTES),
                    ));
                }
                let filename = derive_filename(&u, "image");
                (net.body, mime, filename)
            }
            Ok(Source::Ref(id)) => {
                let att = match wafer_sdk::lookup_attachment(&id) {
                    Ok(Some(a)) => a,
                    Ok(None) => return GuestResult::error(WaferError::new(
                        ErrorCode::NOT_FOUND,
                        format!("no attachment found for ref {:?}", id),
                    )),
                    Err(e) => return GuestResult::error(WaferError::new(
                        ErrorCode::INTERNAL,
                        e.to_string(),
                    )),
                };
                if !att.mime.starts_with("image/") {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::INVALID_ARGUMENT,
                        format!("expected image/* attachment, got {}", att.mime),
                    ));
                }
                if att.bytes.len() > MAX_INPUT_BYTES {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::OUT_OF_RANGE,
                        format!("input image too large: {} bytes (cap {} bytes)", att.bytes.len(), MAX_INPUT_BYTES),
                    ));
                }
                let filename = att.filename.unwrap_or_else(|| "image".into());
                (att.bytes, att.mime, filename)
            }
        };

        let ext = match mime_to_ext(&mime) {
            Some(e) => e,
            None => return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("unsupported input mime: {mime}"),
            )),
        };
        let ffmpeg_in = format!("in.{ext}");
        let ffmpeg_out = format!("out.{ext}");
        let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.x, args.y, args.width, args.height);

        let req = FfmpegReq { args: argv, inputs: vec![(ffmpeg_in, input_bytes)], output: ffmpeg_out };
        let req_body = match serde_json::to_vec(&req) {
            Ok(b) => b,
            Err(e) => return GuestResult::error(WaferError::new(ErrorCode::INTERNAL, format!("serialize ffmpeg request: {e}"))),
        };
        let ff_resp_bytes = match dispatch_ffmpeg_runtime(&req_body) {
            Ok(b) => b,
            Err(e) => return GuestResult::error(e),
        };
        let ff: FfmpegResp = match serde_json::from_slice(&ff_resp_bytes) {
            Ok(r) => r,
            Err(e) => return GuestResult::error(WaferError::new(ErrorCode::INTERNAL, format!("malformed ffmpeg response: {e}"))),
        };

        if ff.exit_code != 0 {
            let snippet: String = ff.log.chars().take(200).collect();
            return GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("ffmpeg failed (exit {}): {snippet}", ff.exit_code),
            ));
        }
        if ff.output.len() > MAX_OUTPUT_BYTES {
            return GuestResult::error(WaferError::new(
                ErrorCode::OUT_OF_RANGE,
                format!("output image too large: {} bytes (cap {} bytes)", ff.output.len(), MAX_OUTPUT_BYTES),
            ));
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
            for_ui: ForUi { data_url, mime, filename },
        };
        match serde_json::to_vec(&env) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(WaferError::new(ErrorCode::INTERNAL, format!("serialize envelope: {e}"))),
        }
    }
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
