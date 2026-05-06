//! gizza-ai/video-trim — fetch a video URL or attachment ref, trim to [start, start+duration], stream-copy to mp4.

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    r#ref: Option<String>,
    start: f64,
    duration: f64,
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

fn build_argv(in_name: &str, out_name: &str, start: f64, duration: f64) -> Vec<String> {
    vec![
        "-ss".into(),
        format!("{start}"),
        "-i".into(),
        in_name.into(),
        "-t".into(),
        format!("{duration}"),
        "-c".into(),
        "copy".into(),
        out_name.into(),
    ]
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
struct VideoTrim;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-trim",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Trim a video to a [start, start+duration] window",
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Trim a video to a [start, start+duration] window using stream-copy (no re-encode). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Output is mp4. Stream-copy preserves the source codecs and is fast — but requires the source streams be mp4-compatible (h264/aac); otherwise ffmpeg will fail with a clear error.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":      { "type": "string" },
                "ref":      { "type": "string" },
                "start":    { "type": "number", "minimum": 0, "description": "Start time in seconds." },
                "duration": { "type": "number", "exclusiveMinimum": 0, "description": "Duration in seconds." }
            },
            "required": ["start", "duration"],
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl VideoTrim {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid video-trim args: {e}"),
                ))
            }
        };
        if args.start < 0.0 || !args.start.is_finite() {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("invalid video-trim args: start must be >= 0 and finite, got {}", args.start),
            ));
        }
        if args.duration <= 0.0 || !args.duration.is_finite() {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("invalid video-trim args: duration must be > 0 and finite, got {}", args.duration),
            ));
        }

        let (input_bytes, in_mime, in_filename) = match args.source() {
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid video-trim args: {e}"),
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
        let ffmpeg_out = "out.mp4".to_string();
        let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.start, args.duration);

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
        let data_url = format!("data:video/mp4;base64,{encoded}");
        let filename = output_filename(&in_filename, "mp4");

        let env = Envelope {
            for_llm: format!(
                "trimmed {} to [{}s, {}s+{}s] ({} bytes mp4)",
                in_filename, args.start, args.start, args.duration, output_size
            ),
            for_ui: ForUi {
                data_url,
                mime: "video/mp4".to_string(),
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
    fn argv_default() {
        let argv = build_argv("in.mp4", "out.mp4", 1.5, 3.0);
        assert_eq!(argv[0], "-ss");
        assert_eq!(argv[1], "1.5");
        assert_eq!(argv[2], "-i");
        assert_eq!(argv[3], "in.mp4");
        assert_eq!(argv[4], "-t");
        assert_eq!(argv[5], "3");
        assert!(argv.iter().any(|a| a == "copy"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn args_url_only_ok() {
        let a = Args {
            url: Some("u".into()),
            r#ref: None,
            start: 0.0,
            duration: 1.0,
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
            start: 0.0,
            duration: 1.0,
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
            start: 0.0,
            duration: 1.0,
        };
        assert!(a.source().is_err());
    }

    #[test]
    fn args_neither_errors() {
        let a = Args {
            url: None,
            r#ref: None,
            start: 0.0,
            duration: 1.0,
        };
        assert!(a.source().is_err());
    }
}
