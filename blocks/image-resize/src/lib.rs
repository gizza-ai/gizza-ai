//! gizza-ai/image-resize — fetch an image URL or attachment ref, resize via ffmpeg, return envelope.

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. The block-local helpers
// (`build_argv`, `output_filename`, etc.) remain native-compilable so the unit
// tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, filename_with_suffix, mime_to_ext, AssetKind, Envelope, FfmpegReq,
    FfmpegResp, ForUi, SkillError, SkillResultExt, Source, SourceFields,
};
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


fn summary(source: &str, w: Option<u32>, h: Option<u32>, output_size: usize, mime: &str) -> String {
    match (w, h) {
        (Some(w), Some(h)) => format!("resized {source} to {w}x{h} ({output_size} {mime})"),
        (Some(w), None)    => format!("resized {source} to {w} wide ({output_size} {mime})"),
        (None, Some(h))    => format!("resized {source} to {h} tall ({output_size} {mime})"),
        (None, None)       => format!("resized {source} ({output_size} {mime})"),
    }
}

#[cfg(target_arch = "wasm32")]
struct ImageResize;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-resize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Resize an image fetched by URL or from a prior tool call ref",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Resize an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call).",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":    { "type": "string", "description": "Image URL (HTTP/HTTPS)." },
                "ref":    { "type": "string", "description": "Reference id from a prior image tool call (e.g. \"call_42\"). Use either url or ref." },
                "width":  { "type": "integer", "minimum": 1, "description": "Target width in pixels." },
                "height": { "type": "integer", "minimum": 1, "description": "Target height in pixels." },
                "fit":    { "type": "string", "enum": ["contain", "cover", "stretch"], "description": "Resize mode (default: contain)." }
            },
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl ImageResize {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("image-resize")?;
    if args.width.is_none() && args.height.is_none() {
        return Err(SkillError::InvalidArgs(
            "invalid image-resize args: at least one of width/height required".into(),
        ));
    }
    if args.width == Some(0) || args.height == Some(0) {
        return Err(SkillError::InvalidArgs(
            "invalid image-resize args: width/height must be > 0".into(),
        ));
    }
    let fit = parse_fit(args.fit.as_deref()).invalid_args("image-resize")?;
    if fit == Fit::Cover && (args.width.is_none() || args.height.is_none()) {
        return Err(SkillError::InvalidArgs(
            "invalid image-resize args: fit=cover requires both width and height".into(),
        ));
    }

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Image, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Image, MAX_INPUT_BYTES)?,
    };

    // 3. Build ffmpeg argv.
    let ext = mime_to_ext(&mime).ok_or_else(|| {
        SkillError::InvalidArgs(format!("unsupported input mime: {mime}"))
    })?;
    let ffmpeg_in = format!("in.{ext}");
    let ffmpeg_out = format!("out.{ext}");
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.width, args.height, fit);

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
    let dim_suffix = match (args.width, args.height) {
        (Some(w), Some(h)) => format!("-{}x{}", w, h),
        _ => "-resized".to_string(),
    };
    let filename = filename_with_suffix(&in_filename, &dim_suffix, ext);
    let env = Envelope {
        for_llm: summary(&in_filename, args.width, args.height, output_size, &mime),
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
        assert_eq!(filename_with_suffix("cat.png", "-640x480", "png"), "cat-640x480.png");
    }

    #[test]
    fn output_filename_uses_resized_suffix_when_one_dim() {
        assert_eq!(filename_with_suffix("cat.png", "-resized", "png"), "cat-resized.png");
    }
}
