//! gizza-ai/video-transcode — fetch a video URL or attachment ref, transcode to a target container.

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

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
    #[serde(rename = "_for_llm")]
    for_llm: String,
    #[serde(rename = "_for_ui")]
    for_ui: ForUi,
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

#[allow(dead_code)]
fn mime_to_ext(mime: &str) -> Option<&'static str> {
    match mime {
        "video/mp4" => Some("mp4"),
        "video/webm" => Some("webm"),
        "video/quicktime" => Some("mov"),
        "video/x-matroska" => Some("mkv"),
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

#[allow(dead_code)]
fn derive_filename(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let path = after_scheme.split('/').skip(1).collect::<Vec<_>>().join("/");
    let path = path.split('?').next().unwrap_or("");
    let path = path.split('#').next().unwrap_or("");
    let last = path.rsplit('/').next().unwrap_or("");
    let decoded = percent_decode(last);
    if decoded.is_empty() {
        "video".into()
    } else {
        decoded
    }
}

#[allow(dead_code)]
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

#[allow(dead_code)]
fn output_filename(in_filename: &str, out_ext: &str) -> String {
    let stem = in_filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(in_filename);
    format!("{stem}.{out_ext}")
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
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid video-transcode args: {e}"),
                ))
            }
        };
        let (out_mime, out_ext) = match format_to_mime_and_ext(&args.format) {
            Some(t) => t,
            None => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!(
                        "invalid video-transcode args: format {:?} not supported (mp4|webm)",
                        args.format
                    ),
                ))
            }
        };
        if let Some(q) = args.quality {
            if !(1..=100).contains(&q) {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid video-transcode args: quality must be 1-100, got {q}"),
                ));
            }
        }
        let crf = quality_to_crf(args.quality.unwrap_or(DEFAULT_QUALITY));

        let (input_bytes, in_mime, in_filename) = match args.source() {
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid video-transcode args: {e}"),
                ))
            }
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
                let raw_mime = net
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .and_then(|(_, vs)| vs.first().cloned())
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                let in_mime: String = raw_mime
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_lowercase();
                if !in_mime.starts_with("video/") {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::INVALID_ARGUMENT,
                        format!("expected video/* content-type, got {in_mime}"),
                    ));
                }
                if let Some(cl) = net
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, vs)| vs.first())
                    .and_then(|v| v.trim().parse::<usize>().ok())
                {
                    if cl > MAX_INPUT_BYTES {
                        return GuestResult::error(WaferError::new(
                            ErrorCode::OUT_OF_RANGE,
                            format!("input video too large: {cl} bytes (cap {MAX_INPUT_BYTES} bytes)"),
                        ));
                    }
                }
                if net.body.len() > MAX_INPUT_BYTES {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::OUT_OF_RANGE,
                        format!(
                            "input video too large: {} bytes (cap {} bytes)",
                            net.body.len(),
                            MAX_INPUT_BYTES
                        ),
                    ));
                }
                let filename = derive_filename(&u);
                (net.body, in_mime, filename)
            }
            Ok(Source::Ref(id)) => {
                let att = match wafer_sdk::lookup_attachment(&id) {
                    Ok(Some(a)) => a,
                    Ok(None) => {
                        return GuestResult::error(WaferError::new(
                            ErrorCode::NOT_FOUND,
                            format!("no attachment found for ref {:?}", id),
                        ))
                    }
                    Err(e) => {
                        return GuestResult::error(WaferError::new(
                            ErrorCode::INTERNAL,
                            e.to_string(),
                        ))
                    }
                };
                if !att.mime.starts_with("video/") {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::INVALID_ARGUMENT,
                        format!("expected video/* attachment, got {}", att.mime),
                    ));
                }
                if att.bytes.len() > MAX_INPUT_BYTES {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::OUT_OF_RANGE,
                        format!(
                            "input video too large: {} bytes (cap {} bytes)",
                            att.bytes.len(),
                            MAX_INPUT_BYTES
                        ),
                    ));
                }
                let filename = att.filename.unwrap_or_else(|| "video".into());
                (att.bytes, att.mime, filename)
            }
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
        let req_body = match serde_json::to_vec(&req) {
            Ok(b) => b,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("serialize ffmpeg request: {e}"),
                ))
            }
        };
        let ff_resp_bytes = match dispatch_ffmpeg_runtime(&req_body) {
            Ok(b) => b,
            Err(e) => return GuestResult::error(e),
        };
        let ff: FfmpegResp = match serde_json::from_slice(&ff_resp_bytes) {
            Ok(r) => r,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("malformed ffmpeg response: {e}"),
                ))
            }
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
                format!(
                    "output video too large: {} bytes (cap {} bytes)",
                    ff.output.len(),
                    MAX_OUTPUT_BYTES
                ),
            ));
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
        match serde_json::to_vec(&env) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("serialize envelope: {e}"),
            )),
        }
    }
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

    #[test]
    fn args_url_only_ok() {
        let a = Args {
            url: Some("u".into()),
            r#ref: None,
            format: "mp4".into(),
            quality: None,
        };
        match a.source().expect("ok") {
            Source::Url(u) => assert_eq!(u, "u"),
            _ => panic!("expected Url"),
        }
    }

    #[test]
    fn args_ref_only_ok() {
        let a = Args {
            url: None,
            r#ref: Some("call_1".into()),
            format: "mp4".into(),
            quality: None,
        };
        match a.source().expect("ok") {
            Source::Ref(r) => assert_eq!(r, "call_1"),
            _ => panic!("expected Ref"),
        }
    }

    #[test]
    fn args_both_url_and_ref_errors() {
        let a = Args {
            url: Some("u".into()),
            r#ref: Some("r".into()),
            format: "mp4".into(),
            quality: None,
        };
        assert!(a.source().is_err());
    }

    #[test]
    fn args_neither_errors() {
        let a = Args {
            url: None,
            r#ref: None,
            format: "mp4".into(),
            quality: None,
        };
        assert!(a.source().is_err());
    }
}
