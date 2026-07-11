//! gizza-ai/video-cut-segments — fetch a video URL or attachment ref, keep or
//! remove multiple time windows from a typed list, join the kept parts, and
//! return an mp4 envelope.
//!
//! Single ffmpeg pass built from the shared pure `core`: one `trim`/`atrim` per
//! kept window, each re-based (`setpts`/`asetpts`), then `concat`-ed into one
//! H.264/AAC mp4. `mode = keep` keeps the listed windows; `mode = remove` keeps
//! their complement (the tail window is open-ended, so no duration probe is
//! needed and the single-pass page driver works). The chat schema is derived
//! from `descriptor()` (single source across chat + CLI + page).

// The #[wafer_block] macro emits the impl gated to wasm32 (it generates a native
// registration call that requires ::new()). Supporting imports, constants, and
// the Args type are only used inside that impl, so they look "unused" under a
// native build; `descriptor()`/`schema_json()` stay native-compilable so the
// drift-guard + sanity tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_video_cut_segments_core::plan;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

const DEFAULT_MODE: &str = "keep";

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    segments: String,
    #[serde(default)]
    mode: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). `Input::Video`
/// adds the `url`⊕`ref` `oneOf`. `mode` is a fixed-choice enum (renders as a
/// `<select>` on the page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::string("segments")
                .required()
                .describe(
                    "Time windows as a comma- or newline-separated list of start-end, e.g. \
                     '0:05-0:10, 1:30-1:45'. Each time is SS, MM:SS, or HH:MM:SS (fractions ok).",
                )
                .placeholder("0:00-0:05, 0:20-0:30"),
        )
        .param(
            Param::enumv("mode", ["keep", "remove"])
                .default("keep")
                .describe(
                    "keep = keep only the listed windows and join them (default); \
                     remove = cut the listed windows out and keep everything else.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoCutSegments;

// The #[wafer_block] macro emits a native registration call requiring ::new();
// skill-style impls don't have one. Gate the struct + impl to wasm32 so the
// descriptor tests still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-cut-segments",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Keep or remove multiple time windows from a video and join the parts",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Cut a video down to multiple time windows from a typed list and join the kept parts into one clip. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). `segments` is a comma- or newline-separated list of start-end windows (each time SS, MM:SS, or HH:MM:SS, fractions allowed), e.g. '0:05-0:10, 1:30-1:45'. `mode` = keep keeps only those windows and joins them (default); mode = remove cuts them out and keeps the rest. Audio stays in sync (trim+concat). Output is mp4 (re-encoded H.264/AAC).",
        parameters = schema_json()
    ),
)]
impl VideoCutSegments {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-cut-segments")?;
    let mode = args.mode.as_deref().unwrap_or(DEFAULT_MODE);

    // 2. Resolve source.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");

    // 3. Build ffmpeg argv (shared pure core; validates segments + mode).
    let (argv, out_name) =
        plan(&ffmpeg_in, &args.segments, mode).map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, input_bytes, out_name)?;

    // 5. Envelope. Output is always mp4 (the join re-encodes to H.264/AAC).
    let output_size = output.len();
    let filename = filename_with_suffix(&in_filename, "-cut", "mp4");
    let verb = if mode == "remove" { "removed" } else { "kept" };
    let for_llm = format!(
        "{verb} the listed window(s) of {in_filename} and joined the parts \
         ({output_size} bytes video/mp4)"
    );
    build_media_envelope(&output, "video/mp4", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The descriptor renders to a valid JSON schema with the string `segments`
    /// param, the `mode` enum, and the `url`⊕`ref` `oneOf` (from `Input::Video`).
    #[test]
    fn schema_json_is_valid_and_has_params() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).expect("valid JSON schema");
        let props = v.get("properties").expect("properties");
        assert!(props.get("segments").is_some());
        assert_eq!(props["segments"]["type"], serde_json::json!("string"));
        assert_eq!(props["mode"]["enum"], serde_json::json!(["keep", "remove"]));
        assert_eq!(props["mode"]["default"], serde_json::json!("keep"));
        assert!(props.get("url").is_some());
        assert!(props.get("ref").is_some());
        assert!(v.get("oneOf").is_some(), "Input::Video adds url/ref oneOf");
        assert_eq!(
            v["required"],
            serde_json::json!(["segments"]),
            "segments is the only required scalar param"
        );
    }
}
