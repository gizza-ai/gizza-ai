//! gizza-ai/cmyk-jpeg-to-rgb — convert a CMYK/YCCK (usually Adobe/print) JPEG
//! into a standard RGB PNG, JPEG or WebP, via ffmpeg on the shared tool
//! abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. The chat schema is derived from
//! `descriptor()` (single source across chat + CLI + page) and the drift-guard
//! test below pins it.
//!
//! Beyond the transcode, the block reads the input JPEG's frame header + Adobe
//! APP14 marker (pure core, no deps) so the result summary says what the file
//! actually was — a genuine CMYK conversion or just a re-encode of an
//! already-RGB image.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_cmyk_jpeg_to_rgb_core::{
    detect_input_color, parse_format, plan, summarize, DEFAULT_QUALITY,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    quality: Option<u8>,
    #[serde(default)]
    chroma: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. PNG is the default because a CMYK source is
    // normally print artwork (flat colour + type) where a lossy default is wrong.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("format", ["png", "jpeg", "webp"])
                .default("png")
                .describe(
                    "Output image format. png (default) is lossless and the safest for print \
                     artwork, logos and type; jpeg gives a small file for photographs; webp is \
                     the smallest for the web. quality applies to jpeg and webp only.",
                ),
        )
        .param(
            Param::integer("quality")
                .min(1.0)
                .max(100.0)
                .default(DEFAULT_QUALITY as i64)
                .describe(
                    "Quality 1-100 for jpeg and webp output (default 90; ignored for png, which \
                     is lossless). Higher keeps more detail but makes a bigger file; 100 is \
                     near-lossless, 70-85 is a good size/quality trade.",
                ),
        )
        .param(
            Param::enumv("chroma", ["4:2:0", "4:4:4"])
                .default("4:2:0")
                .describe(
                    "Chroma subsampling for jpeg output. 4:2:0 (default) stores colour at half \
                     resolution for the smallest file; 4:4:4 keeps full-resolution colour, which \
                     matters for the coloured text, logos and flat fills typical of CMYK print \
                     files. Ignored for png (always full RGB) and webp (always 4:2:0).",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cmyk-jpeg-to-rgb",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a CMYK (often Adobe/print) JPEG into a standard RGB PNG, JPEG or WebP.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert a CMYK or YCCK JPEG — the four-ink kind Photoshop and print workflows export, which many apps refuse to open or render with wrong colours — into a standard RGB image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call). Optional format (png default, jpeg, webp), quality 1-100 for jpeg/webp (default 90), and chroma subsampling 4:2:0 (default) or 4:4:4 for jpeg. The four ink channels are collapsed to true three-channel RGB, so the black/K channel is not left behind as a stray alpha channel. Output is untagged sRGB, which is what browsers assume; the conversion is arithmetic, not an ICC-profiled press proof. Already-RGB images are accepted and simply re-encoded, and the result says which happened.",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid cmyk-jpeg-to-rgb args: {e}")))?;
    // Reject 0/out-of-range explicitly: the core treats 0.0 as "unset" for the
    // page's cleared-field convention, but a typed 0 from chat/CLI is an error.
    gizza_ai_block_utils::validate_quality_1_100(args.quality, "cmyk-jpeg-to-rgb")?;

    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let in_ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let ffmpeg_in = format!("in.{in_ext}");

    let format = args.format.as_deref().unwrap_or("png");
    let (argv, out_name) = plan(
        &ffmpeg_in,
        format,
        args.quality.map(f64::from).unwrap_or(0.0),
        args.chroma.as_deref().unwrap_or(""),
    )
    .map_err(|e| SkillError::InvalidArgs(format!("invalid cmyk-jpeg-to-rgb args: {e}")))?;
    let fmt = parse_format(format)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid cmyk-jpeg-to-rgb args: {e}")))?;

    // What the source actually stored — reported so an already-RGB file is not
    // passed off as a CMYK conversion.
    let color = detect_input_color(&bytes);

    let output = dispatch_ffmpeg(argv, ffmpeg_in, bytes, out_name)?;
    let filename = filename_with_suffix(&in_name, "-rgb", fmt.ext());
    let for_llm = summarize(color, fmt, output.len());
    build_media_envelope(output.as_slice(), fmt.mime(), filename, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema below, so the LLM-facing tool definition never silently changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": {
                        "type": "string",
                        "enum": ["png", "jpeg", "webp"],
                        "default": "png",
                        "description": "Output image format. png (default) is lossless and the safest for print artwork, logos and type; jpeg gives a small file for photographs; webp is the smallest for the web. quality applies to jpeg and webp only."
                    },
                    "quality": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 90,
                        "description": "Quality 1-100 for jpeg and webp output (default 90; ignored for png, which is lossless). Higher keeps more detail but makes a bigger file; 100 is near-lossless, 70-85 is a good size/quality trade."
                    },
                    "chroma": {
                        "type": "string",
                        "enum": ["4:2:0", "4:4:4"],
                        "default": "4:2:0",
                        "description": "Chroma subsampling for jpeg output. 4:2:0 (default) stores colour at half resolution for the smallest file; 4:4:4 keeps full-resolution colour, which matters for the coloured text, logos and flat fills typical of CMYK print files. Ignored for png (always full RGB) and webp (always 4:2:0)."
                    }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_marks_the_conversion_and_swaps_extension() {
        assert_eq!(
            filename_with_suffix("press-ad.jpg", "-rgb", "png"),
            "press-ad-rgb.png"
        );
        assert_eq!(
            filename_with_suffix("flyer.jpg", "-rgb", "jpg"),
            "flyer-rgb.jpg"
        );
    }
}
