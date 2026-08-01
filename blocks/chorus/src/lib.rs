//! gizza-ai/chorus — audio (ffmpeg) chat skill on the shared abstraction.
//! Input::Audio emits a url⊕ref oneOf; run() uses resolve_source → core::plan_chorus
//! → dispatch_ffmpeg → build_media_envelope. The pure argv planner lives in core
//! so the CLI descriptor, chat block, and browser page share the same validation.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use serde::Deserialize;
use wafer_sdk::*;

use gizza_ai_chorus_core::{
    fmt_num, parse_format, plan_chorus, DEFAULT_DECAY, DEFAULT_DELAY_MS, DEFAULT_DEPTH_MS,
    DEFAULT_SPEED_HZ, DEFAULT_VOICES, MAX_DECAY, MAX_DELAY_MS, MAX_DEPTH_MS, MAX_SPEED_HZ,
    MAX_VOICES, MIN_DECAY, MIN_DELAY_MS, MIN_DEPTH_MS, MIN_SPEED_HZ, MIN_VOICES,
};

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    voices: Option<i64>,
    #[serde(default)]
    delay_ms: Option<f64>,
    #[serde(default)]
    depth_ms: Option<f64>,
    #[serde(default)]
    speed_hz: Option<f64>,
    #[serde(default)]
    decay: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(Param::integer("voices").min(MIN_VOICES as f64).max(MAX_VOICES as f64).default(DEFAULT_VOICES as f64).describe("Number of delayed modulation voices to mix in (2-4, default 2). More voices sound wider and thicker."))
        .param(Param::number("delay_ms").min(MIN_DELAY_MS).max(MAX_DELAY_MS).default(DEFAULT_DELAY_MS).describe("Base delay before the first chorus voice, in milliseconds (20-80, default 50). Later voices are staggered by 8 ms."))
        .param(Param::number("depth_ms").min(MIN_DEPTH_MS).max(MAX_DEPTH_MS).default(DEFAULT_DEPTH_MS).describe("Modulation depth in milliseconds (1-8, default 2). Higher values make the pitch movement more obvious."))
        .param(Param::number("speed_hz").min(MIN_SPEED_HZ).max(MAX_SPEED_HZ).default(DEFAULT_SPEED_HZ).describe("Modulation rate in Hertz (0.1-5, default 0.4). Higher values shimmer or wobble faster."))
        .param(Param::number("decay").min(MIN_DECAY).max(MAX_DECAY).default(DEFAULT_DECAY).describe("Level of each delayed voice (0.1-0.9, default 0.4). Higher values make the chorus wetter and louder."))
        .param(Param::enumv("format", ["mp3", "wav", "ogg", "flac", "m4a"]).default("mp3").describe("Output audio format. Default mp3 (192 kbps); wav/flac are lossless re-encodes, ogg is Vorbis, m4a is AAC."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/chorus",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add a configurable chorus effect to an audio file",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Add a chorus effect to an audio file by mixing in short modulated delay voices with ffmpeg. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). voices controls how many delayed copies are mixed (2-4); delay_ms is the base delay; depth_ms and speed_hz control the modulation; decay is the delayed voice level. Output is re-encoded to mp3, wav, ogg, flac, or m4a and embedded album art is dropped. Runs locally through the ffmpeg runtime.",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid chorus args: {e}")))?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{ext}");
    let format = args.format.as_deref().unwrap_or("mp3");
    let voices = args.voices.unwrap_or(DEFAULT_VOICES);
    let delay_ms = args.delay_ms.unwrap_or(DEFAULT_DELAY_MS);
    let depth_ms = args.depth_ms.unwrap_or(DEFAULT_DEPTH_MS);
    let speed_hz = args.speed_hz.unwrap_or(DEFAULT_SPEED_HZ);
    let decay = args.decay.unwrap_or(DEFAULT_DECAY);

    let (argv, ffmpeg_out) = plan_chorus(
        &ffmpeg_in, voices, delay_ms, depth_ms, speed_hz, decay, format,
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, ffmpeg_in, bytes, ffmpeg_out)?;
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let filename = filename_with_suffix(&in_name, "-chorus", fmt.ext());
    let for_llm = format!("added {}-voice chorus to {in_name} (delay {} ms, depth {} ms, rate {} Hz, decay {}, {} bytes {})", voices, fmt_num(delay_ms), fmt_num(depth_ms), fmt_num(speed_hz), fmt_num(decay), output.len(), fmt.ext());
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_audio_source_and_chorus_controls() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(schema["oneOf"][0]["required"][0], "url");
        assert_eq!(schema["oneOf"][1]["required"][0], "ref");
        let props = &schema["properties"];
        assert_eq!(props["voices"]["minimum"], MIN_VOICES);
        assert_eq!(props["voices"]["maximum"], MAX_VOICES);
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["mp3", "wav", "ogg", "flac", "m4a"])
        );
        for name in ["delay_ms", "depth_ms", "speed_hz", "decay"] {
            assert!(props[name]["description"].as_str().unwrap().len() > 20);
        }
    }

    #[test]
    fn output_filename_uses_chorus_suffix() {
        assert_eq!(
            filename_with_suffix("guitar.wav", "-chorus", "mp3"),
            "guitar-chorus.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-chorus", "flac"),
            "voice memo-chorus.flac"
        );
    }
}
