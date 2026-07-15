//! gizza-ai/video-hdr-to-sdr — media (ffmpeg) chat skill on the shared abstraction.
//! Tone-maps an HDR (PQ/HLG, BT.2020) video down to standard-dynamic-range,
//! BT.709 / yuv420p so it stops rendering gray/dim on ordinary SDR screens.
//!
//! The chat schema is derived from `descriptor()` (single source shared across
//! chat + CLI + page); the pure argv builder + validation live in `core` and are
//! reused verbatim by the web page. `run()` resolves the source → builds argv via
//! `core::plan_hdr_to_sdr` → dispatches ffmpeg → returns a media envelope.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg / format_to_mime_and_ext call host imports → wasm-only.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, format_to_mime_and_ext, resolve_source};
use gizza_ai_video_hdr_to_sdr_core::{
    plan_hdr_to_sdr, DEFAULT_ALGORITHM, DEFAULT_DESAT, DEFAULT_FORMAT, DEFAULT_PEAK, DEFAULT_QUALITY,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    tonemap: Option<String>,
    #[serde(default)]
    peak: Option<u32>,
    #[serde(default)]
    desat: Option<f64>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    quality: Option<u8>,
}

/// Single-source param descriptor → chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("tonemap", ["hable", "mobius", "reinhard", "linear", "clip"])
                .default("hable")
                .describe("Tone-mapping curve (default hable). hable = filmic default; mobius keeps midtone saturation; reinhard is flatter; linear/clip are simplest."),
        )
        .param(
            Param::integer("peak")
                .default(100)
                .min(100.0)
                .max(10000.0)
                .describe("Target SDR nominal peak luminance in nits (default 100 = reference SDR white)."),
        )
        .param(
            Param::number("desat")
                .default(0.0)
                .min(0.0)
                .max(4.0)
                .describe("Highlight desaturation strength 0-4 (default 0 keeps highlight color)."),
        )
        .param(
            Param::enumv("format", ["mp4", "webm"])
                .default("mp4")
                .describe("Output container (default mp4 = H.264/AAC; webm = VP9/Opus)."),
        )
        .param(
            Param::integer("quality")
                .default(75)
                .min(1.0)
                .max(100.0)
                .describe("Quality 1-100 (default 75). Lower = smaller file, lower quality."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-hdr-to-sdr",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Tone-map an HDR video down to SDR (BT.709).",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Tone-map an HDR video (PQ/HLG, BT.2020) down to standard-dynamic-range BT.709 so it no longer looks gray, dim, or washed-out on ordinary SDR screens. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). tonemap picks the curve (hable|mobius|reinhard|linear|clip, default hable); peak is target nits (default 100); desat is highlight desaturation 0-4 (default 0); format is mp4 or webm (default mp4); quality is 1-100 (default 75).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-hdr-to-sdr args: {e}")))?;

    let tonemap = args
        .tonemap
        .unwrap_or_else(|| DEFAULT_ALGORITHM.as_arg().to_string());
    let peak = args.peak.unwrap_or(DEFAULT_PEAK);
    let desat = args.desat.unwrap_or(DEFAULT_DESAT);
    let format = args.format.unwrap_or_else(|| DEFAULT_FORMAT.ext().to_string());
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);

    // Output mime/ext from the chosen container (mp4 → video/mp4, webm → video/webm).
    let (out_mime, out_ext) = format_to_mime_and_ext(AssetKind::Video, &format).ok_or_else(|| {
        SkillError::InvalidArgs(format!(
            "invalid video-hdr-to-sdr args: format {format:?} not supported (mp4|webm)"
        ))
    })?;

    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");

    // Shared pure planner (validates params, builds the zscale+tonemap argv).
    let (argv, ffmpeg_out) = plan_hdr_to_sdr(&tonemap, peak, desat, &format, quality, &ffmpeg_in)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-hdr-to-sdr args: {e}")))?;

    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, ffmpeg_out)?;

    let output_size = output.len();
    let filename = replace_extension(&in_filename, out_ext);
    let for_llm = format!("tone-mapped {in_filename} from HDR to SDR {out_mime} ({output_size})");
    build_media_envelope(output.as_slice(), out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_enum_and_numeric_params() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = &schema["properties"];
        assert_eq!(props["tonemap"]["enum"][0], "hable");
        assert_eq!(props["tonemap"]["default"], "hable");
        assert_eq!(props["format"]["enum"], serde_json::json!(["mp4", "webm"]));
        assert_eq!(props["format"]["default"], "mp4");
        assert_eq!(props["peak"]["minimum"], 100);
        assert_eq!(props["peak"]["maximum"], 10000);
        assert_eq!(props["peak"]["default"], 100);
        assert_eq!(props["desat"]["maximum"], 4.0);
        assert_eq!(props["desat"]["default"], 0.0);
        assert_eq!(props["quality"]["type"], "integer");
        assert_eq!(props["quality"]["default"], 75);
        // url⊕ref oneOf from Input::Video.
        assert!(schema["oneOf"].is_array());
    }
}
