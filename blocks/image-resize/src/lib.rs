//! gizza-ai/image-resize — fetch an image URL, resize via ffmpeg, return envelope.

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024; // 4 MiB
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    url: String,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    fit: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Fit { Contain, Cover, Stretch }

fn parse_fit(s: Option<&str>) -> Result<Fit, String> {
    match s.unwrap_or("contain") {
        "contain" => Ok(Fit::Contain),
        "cover"   => Ok(Fit::Cover),
        "stretch" => Ok(Fit::Stretch),
        other     => Err(format!("invalid fit {other:?}; expected contain|cover|stretch")),
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

fn build_argv(in_name: &str, out_name: &str, w: Option<u32>, h: Option<u32>, fit: Fit) -> Vec<String> {
    let (sw, sh) = (
        w.map(|v| v.to_string()).unwrap_or_else(|| "-1".to_string()),
        h.map(|v| v.to_string()).unwrap_or_else(|| "-1".to_string()),
    );
    let vf = match fit {
        Fit::Stretch => format!("scale={sw}:{sh}"),
        Fit::Contain => format!("scale={sw}:{sh}:force_original_aspect_ratio=decrease"),
        Fit::Cover   => format!("scale={sw}:{sh}:force_original_aspect_ratio=increase,crop={sw}:{sh}"),
    };
    vec!["-i".into(), in_name.into(), "-vf".into(), vf, out_name.into()]
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

fn output_filename(input_filename: &str, w: Option<u32>, h: Option<u32>, ext: &str) -> String {
    let base = input_filename.rsplitn(2, '.').last().unwrap_or(input_filename);
    let suffix = match (w, h) {
        (Some(w), Some(h)) => format!("-{}x{}", w, h),
        _ => "-resized".to_string(),
    };
    format!("{base}{suffix}.{ext}")
}

#[allow(dead_code)]
fn summary(url: &str, w: Option<u32>, h: Option<u32>, output_size: usize, mime: &str) -> String {
    match (w, h) {
        (Some(w), Some(h)) => format!("resized {url} to {w}x{h} ({output_size} {mime})"),
        (Some(w), None)    => format!("resized {url} to {w} wide ({output_size} {mime})"),
        (None, Some(h))    => format!("resized {url} to {h} tall ({output_size} {mime})"),
        (None, None)       => format!("resized {url} ({output_size} {mime})"),
    }
}

/// Dispatch a request to `gizza-ai/ffmpeg-runtime` via the raw streaming ABI.
///
/// ffmpeg-runtime uses a consumer-controlled JSON wire format (FfmpegReq/Resp
/// serde_json), NOT a wafer-run service. We hand it an opaque `Vec<u8>`
/// payload and accept opaque chunks back. The transport (CallStream/
/// ResponseStream) is the new binary-transport ABI; only the encoding inside
/// the chunks is JSON.
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
struct ImageResize;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively (the helpers above are
// unconditional).
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-resize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Resize an image fetched by URL",
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
)]
impl ImageResize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // 1. Validate args (LLM tool-call args — JSON wire format).
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("invalid image-resize args: {e}"),
            )),
        };
        if args.width.is_none() && args.height.is_none() {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                "invalid image-resize args: at least one of width/height required",
            ));
        }
        if args.width == Some(0) || args.height == Some(0) {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                "invalid image-resize args: width/height must be > 0",
            ));
        }
        let fit = match parse_fit(args.fit.as_deref()) {
            Ok(f) => f,
            Err(msg) => return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("invalid image-resize args: {msg}"),
            )),
        };
        if fit == Fit::Cover && (args.width.is_none() || args.height.is_none()) {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                "invalid image-resize args: fit=cover requires both width and height",
            ));
        }

        // 2. Fetch via wafer-run/network typed client.
        let net = match wafer_sdk::clients::network::do_request(
            "GET",
            &args.url,
            &HashMap::new(),
            None,
        ) {
            Ok(r) => r,
            Err(e) => return GuestResult::error(e),
        };

        // 3. Status check.
        if net.status_code >= 400 {
            return GuestResult::error(WaferError::new(
                ErrorCode::UNAVAILABLE,
                format!("HTTP {} for {}", net.status_code, args.url),
            ));
        }

        // 4. Mime check.
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

        // 5a. Content-Length pre-check (defensive UX guard, not OOM avoidance).
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
        // 5b. Body size check.
        if net.body.len() > MAX_INPUT_BYTES {
            return GuestResult::error(WaferError::new(
                ErrorCode::OUT_OF_RANGE,
                format!("input image too large: {} bytes (cap {} bytes)", net.body.len(), MAX_INPUT_BYTES),
            ));
        }

        // 6. Build ffmpeg argv.
        let ext = match mime_to_ext(&mime) {
            Some(e) => e,
            None => return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("unsupported input mime: {mime}"),
            )),
        };
        let in_name = format!("in.{ext}");
        let out_name = format!("out.{ext}");
        let argv = build_argv(&in_name, &out_name, args.width, args.height, fit);

        // 7. Call ffmpeg-runtime (consumer-controlled JSON protocol).
        let req = FfmpegReq { args: argv, inputs: vec![(in_name, net.body)], output: out_name };
        let req_body = match serde_json::to_vec(&req) {
            Ok(b) => b,
            Err(e) => return GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("serialize ffmpeg request: {e}"),
            )),
        };
        let ff_resp_bytes = match dispatch_ffmpeg_runtime(&req_body) {
            Ok(b) => b,
            Err(e) => return GuestResult::error(e),
        };
        let ff: FfmpegResp = match serde_json::from_slice(&ff_resp_bytes) {
            Ok(r) => r,
            Err(e) => return GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("malformed ffmpeg response: {e}"),
            )),
        };

        // 8. Exit-code check.
        if ff.exit_code != 0 {
            let snippet: String = ff.log.chars().take(200).collect();
            return GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("ffmpeg failed (exit {}): {snippet}", ff.exit_code),
            ));
        }
        // 9. Output size check.
        if ff.output.len() > MAX_OUTPUT_BYTES {
            return GuestResult::error(WaferError::new(
                ErrorCode::OUT_OF_RANGE,
                format!("output image too large: {} bytes (cap {} bytes)", ff.output.len(), MAX_OUTPUT_BYTES),
            ));
        }

        // 10. Base64-encode.
        let output_size = ff.output.len();
        let encoded = B64.encode(&ff.output);
        let data_url = format!("data:{mime};base64,{encoded}");

        // 11. Filename.
        let in_filename = derive_filename(&args.url);
        let filename = output_filename(&in_filename, args.width, args.height, ext);

        // 12. Envelope (LLM tool-call result — JSON wire format).
        let env = Envelope {
            for_llm: summary(&args.url, args.width, args.height, output_size, &mime),
            for_ui: ForUi { data_url, mime, filename },
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
    fn argv_contain_both_dims() {
        let argv = build_argv("in.png", "out.png", Some(640), Some(480), Fit::Contain);
        assert_eq!(argv, vec![
            "-i".to_string(),
            "in.png".to_string(),
            "-vf".to_string(),
            "scale=640:480:force_original_aspect_ratio=decrease".to_string(),
            "out.png".to_string(),
        ]);
    }

    #[test]
    fn argv_contain_width_only_uses_minus_one_height() {
        let argv = build_argv("in.png", "out.png", Some(640), None, Fit::Contain);
        assert!(argv.iter().any(|a| a == "scale=640:-1:force_original_aspect_ratio=decrease"));
    }

    #[test]
    fn argv_contain_height_only_uses_minus_one_width() {
        let argv = build_argv("in.png", "out.png", None, Some(480), Fit::Contain);
        assert!(argv.iter().any(|a| a == "scale=-1:480:force_original_aspect_ratio=decrease"));
    }

    #[test]
    fn argv_stretch_no_aspect_ratio_flag() {
        let argv = build_argv("in.png", "out.png", Some(640), Some(480), Fit::Stretch);
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(vf, "scale=640:480");
    }

    #[test]
    fn argv_cover_uses_two_stage_filter() {
        let argv = build_argv("in.png", "out.png", Some(640), Some(480), Fit::Cover);
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(vf, "scale=640:480:force_original_aspect_ratio=increase,crop=640:480");
    }

    #[test]
    fn parse_fit_default_is_contain() {
        assert_eq!(parse_fit(None).unwrap(), Fit::Contain);
    }

    #[test]
    fn parse_fit_rejects_unknown() {
        assert!(parse_fit(Some("squish")).is_err());
    }

    #[test]
    fn output_filename_uses_dim_suffix_when_both_given() {
        assert_eq!(output_filename("cat.png", Some(640), Some(480), "png"), "cat-640x480.png");
    }

    #[test]
    fn output_filename_uses_resized_suffix_when_one_dim() {
        assert_eq!(output_filename("cat.png", Some(640), None, "png"), "cat-resized.png");
    }
}
