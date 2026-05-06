//! gizza-ai/image-crop — fetch an image URL or attachment ref, crop a rectangle via ffmpeg.

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
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

enum Source {
    Url(String),
    Ref(String),
}

impl Args {
    fn source(&self) -> Result<Source, String> {
        match (&self.url, &self.r#ref) {
            (Some(u), None) => Ok(Source::Url(u.clone())),
            (None, Some(r)) => Ok(Source::Ref(r.clone())),
            (Some(_), Some(_)) => Err("provide exactly one of `url` or `ref`".into()),
            (None, None) => Err("`url` or `ref` is required".into()),
        }
    }
}

#[derive(Serialize)]
struct FfmpegReq {
    args: Vec<String>,
    inputs: Vec<(String, Vec<u8>)>,
    output: String,
}

#[derive(Deserialize)]
struct FfmpegResp {
    exit_code: i32,
    output: Vec<u8>,
    log: String,
}

#[derive(Serialize)]
struct ForUi {
    data_url: String,
    mime: String,
    filename: String,
}

#[derive(Serialize)]
struct Envelope {
    #[serde(rename = "_for_llm")] for_llm: String,
    #[serde(rename = "_for_ui")]  for_ui: ForUi,
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

#[allow(dead_code)]
fn mime_to_ext(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png"  => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

#[allow(dead_code)]
fn derive_filename(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let path = after_scheme.split('/').skip(1).collect::<Vec<_>>().join("/");
    let path = path.split('?').next().unwrap_or("");
    let path = path.split('#').next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(last);
    let cleaned: String = decoded.chars().filter(|c| !c.is_control() && *c != '\u{FFFD}').collect();
    if cleaned.is_empty() { "image".to_string() } else { cleaned }
}

#[allow(dead_code)]
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[allow(dead_code)]
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn output_filename(input_filename: &str, ext: &str) -> String {
    let base = input_filename.rsplitn(2, '.').last().unwrap_or(input_filename);
    format!("{base}-cropped.{ext}")
}

#[allow(dead_code)]
fn dispatch_ffmpeg_runtime(payload: &[u8]) -> Result<Vec<u8>, WaferError> {
    let msg = Message::new("ffmpeg.exec");
    let mut call = wafer_sdk::stream::CallStream::open("gizza-ai/ffmpeg-runtime", &msg)?;
    call.write_chunk(payload)?;
    let mut resp = call.finish()?;
    let mut out = Vec::new();
    while let Some(chunk) = resp.next_chunk()? {
        out.extend(chunk);
    }
    Ok(out)
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
        let (input_bytes, mime, in_filename) = match args.source() {
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
                let filename = derive_filename(&u);
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

    #[test]
    fn args_url_only_ok() {
        let a = Args { url: Some("u".into()), r#ref: None, x: 0, y: 0, width: 100, height: 100 };
        match a.source().expect("ok") {
            Source::Url(u) => assert_eq!(u, "u"),
            _ => panic!("expected Url"),
        }
    }

    #[test]
    fn args_ref_only_ok() {
        let a = Args { url: None, r#ref: Some("call_1".into()), x: 0, y: 0, width: 100, height: 100 };
        match a.source().expect("ok") {
            Source::Ref(r) => assert_eq!(r, "call_1"),
            _ => panic!("expected Ref"),
        }
    }

    #[test]
    fn args_both_url_and_ref_errors() {
        let a = Args { url: Some("u".into()), r#ref: Some("r".into()), x: 0, y: 0, width: 100, height: 100 };
        assert!(a.source().is_err());
    }

    #[test]
    fn args_neither_errors() {
        let a = Args { url: None, r#ref: None, x: 0, y: 0, width: 100, height: 100 };
        assert!(a.source().is_err());
    }
}
