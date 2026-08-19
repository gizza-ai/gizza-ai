//! gizza-ai/bitcrush — audio (ffmpeg) chat skill on the shared abstraction.
//! Input::Audio emits a url⊕ref oneOf; run() uses resolve_source → core::plan_bitcrush
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

use gizza_ai_bitcrush_core::{
    effective_rate_hz, fmt_num, parse_format, plan_bitcrush, DEFAULT_ANTI_ALIAS, DEFAULT_BITS,
    DEFAULT_DRIVE, DEFAULT_MIX, DEFAULT_OUTPUT_GAIN, DEFAULT_SAMPLE_RATE_HZ, MAX_ANTI_ALIAS,
    MAX_BITS, MAX_DRIVE, MAX_MIX, MAX_OUTPUT_GAIN, MAX_SAMPLE_RATE_HZ, MIN_ANTI_ALIAS, MIN_BITS,
    MIN_DRIVE, MIN_MIX, MIN_OUTPUT_GAIN, MIN_SAMPLE_RATE_HZ,
};

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    bits: Option<f64>,
    #[serde(default)]
    sample_rate_hz: Option<f64>,
    #[serde(default)]
    mix: Option<f64>,
    #[serde(default)]
    drive: Option<f64>,
    #[serde(default)]
    output_gain: Option<f64>,
    #[serde(default)]
    anti_alias: Option<f64>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(Param::number("bits").min(MIN_BITS).max(MAX_BITS).default(DEFAULT_BITS).describe("Bit depth to quantise each sample to (1-16, default 8). Lower values add more gritty quantisation noise: 12 is a mild lo-fi tint, 8 is the classic sampler crunch, 4 or less is heavy digital distortion."))
        .param(Param::number("sample_rate_hz").min(MIN_SAMPLE_RATE_HZ).max(MAX_SAMPLE_RATE_HZ).default(DEFAULT_SAMPLE_RATE_HZ).describe("Target effective sample rate in Hz (192-48000, default 8000). Audio is resampled to 48000 Hz and then each sample is held for a whole number of samples, so the rate snaps to the nearest 48000/N (22050 becomes 24000). Lower rates dull the highs and add aliasing."))
        .param(Param::number("mix").min(MIN_MIX).max(MAX_MIX).default(DEFAULT_MIX).describe("Wet/dry blend from 0 to 1 (default 1 = fully crushed). 0 returns the untouched signal; 0.3-0.6 leaves the original audible underneath the crushed layer."))
        .param(Param::number("drive").min(MIN_DRIVE).max(MAX_DRIVE).default(DEFAULT_DRIVE).describe("Input drive into the quantiser (0.25-4, default 1). Above 1 pushes harder for a louder, more distorted crush; below 1 backs the level off. Pair a hot drive with a lower output_gain to keep the distortion without clipping."))
        .param(Param::number("output_gain").min(MIN_OUTPUT_GAIN).max(MAX_OUTPUT_GAIN).default(DEFAULT_OUTPUT_GAIN).describe("Level applied after the quantiser (0.25-4, default 1). Use it to pull back a hot drive: the crush already happened, so lowering this restores headroom without softening the distortion."))
        .param(Param::number("anti_alias").min(MIN_ANTI_ALIAS).max(MAX_ANTI_ALIAS).default(DEFAULT_ANTI_ALIAS).describe("Anti-aliasing amount from 0 to 1 (default 0.5). 0 keeps the raw stepped edges for maximum grit; 1 smooths the hardest aliasing artefacts."))
        .param(Param::enumv("mode", ["lin", "log"]).default("lin").describe("Quantiser curve. lin (default) uses evenly spaced steps like classic fixed-point hardware; log uses coarser steps near silence, closer to companded gear."))
        .param(Param::enumv("format", ["mp3", "wav", "ogg", "flac", "m4a"]).default("mp3").describe("Output audio format. Default mp3 (192 kbps); wav/flac are lossless re-encodes, ogg is Vorbis, m4a is AAC."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bitcrush",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bitcrush audio by reducing bit depth and sample rate for a lo-fi retro sound",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Bitcrush an audio file with ffmpeg: quantise samples to fewer bits and hold each sample so the effective sample rate drops, for a deliberate lo-fi, 8-bit, or retro-console sound. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). bits sets the depth (1-16); sample_rate_hz sets the target effective rate (192-48000, snapped to the nearest 48000/N); mix blends wet against dry; drive pushes the level into the quantiser and output_gain pulls it back afterwards; anti_alias trades grit for smoothness; mode picks a linear or logarithmic quantiser. Output is re-encoded to mp3, wav, ogg, flac, or m4a and embedded album art is dropped. Runs locally through the ffmpeg runtime.",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid bitcrush args: {e}")))?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime).unwrap_or("mp3");
    let ffmpeg_in = format!("in.{ext}");
    let format = args.format.as_deref().unwrap_or("mp3");
    let mode = args.mode.as_deref().unwrap_or("lin");
    let bits = args.bits.unwrap_or(DEFAULT_BITS);
    let sample_rate_hz = args.sample_rate_hz.unwrap_or(DEFAULT_SAMPLE_RATE_HZ);
    let mix = args.mix.unwrap_or(DEFAULT_MIX);
    let drive = args.drive.unwrap_or(DEFAULT_DRIVE);
    let output_gain = args.output_gain.unwrap_or(DEFAULT_OUTPUT_GAIN);
    let anti_alias = args.anti_alias.unwrap_or(DEFAULT_ANTI_ALIAS);

    let (argv, ffmpeg_out) = plan_bitcrush(
        &ffmpeg_in,
        bits,
        sample_rate_hz,
        mix,
        drive,
        output_gain,
        anti_alias,
        mode,
        format,
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, ffmpeg_in, bytes, ffmpeg_out)?;
    let fmt = parse_format(format).map_err(SkillError::InvalidArgs)?;
    let filename = filename_with_suffix(&in_name, "-bitcrush", fmt.ext());
    let for_llm = format!(
        "bitcrushed {in_name} to {} bit at {} Hz effective (mix {}, drive {}, output gain {}, anti-alias {}, {} quantiser, {} bytes {})",
        fmt_num(bits),
        fmt_num(effective_rate_hz(sample_rate_hz)),
        fmt_num(mix),
        fmt_num(drive),
        fmt_num(output_gain),
        fmt_num(anti_alias),
        mode,
        output.len(),
        fmt.ext()
    );
    build_media_envelope(&output, fmt.mime(), filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_audio_source_and_crush_controls() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(schema["oneOf"][0]["required"][0], "url");
        assert_eq!(schema["oneOf"][1]["required"][0], "ref");
        let props = &schema["properties"];
        assert_eq!(props["bits"]["minimum"], MIN_BITS);
        assert_eq!(props["bits"]["maximum"], MAX_BITS);
        assert_eq!(props["sample_rate_hz"]["default"], DEFAULT_SAMPLE_RATE_HZ);
        assert_eq!(props["mode"]["enum"], serde_json::json!(["lin", "log"]));
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["mp3", "wav", "ogg", "flac", "m4a"])
        );
        assert_eq!(props["output_gain"]["minimum"], MIN_OUTPUT_GAIN);
        assert_eq!(props["output_gain"]["maximum"], MAX_OUTPUT_GAIN);
        for name in [
            "bits",
            "sample_rate_hz",
            "mix",
            "drive",
            "output_gain",
            "anti_alias",
        ] {
            assert!(props[name]["description"].as_str().unwrap().len() > 20);
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Audio URL (HTTP/HTTPS). Use either url or ref."
                },
                "ref": {
                    "type": "string",
                    "description": "Reference id from a prior tool call. Use either url or ref."
                },
                "bits": {
                    "type": "number",
                    "description": "Bit depth to quantise each sample to (1-16, default 8). Lower values add more gritty quantisation noise: 12 is a mild lo-fi tint, 8 is the classic sampler crunch, 4 or less is heavy digital distortion.",
                    "minimum": 1,
                    "maximum": 16,
                    "default": 8.0
                },
                "sample_rate_hz": {
                    "type": "number",
                    "description": "Target effective sample rate in Hz (192-48000, default 8000). Audio is resampled to 48000 Hz and then each sample is held for a whole number of samples, so the rate snaps to the nearest 48000/N (22050 becomes 24000). Lower rates dull the highs and add aliasing.",
                    "minimum": 192,
                    "maximum": 48000,
                    "default": 8000.0
                },
                "mix": {
                    "type": "number",
                    "description": "Wet/dry blend from 0 to 1 (default 1 = fully crushed). 0 returns the untouched signal; 0.3-0.6 leaves the original audible underneath the crushed layer.",
                    "minimum": 0,
                    "maximum": 1,
                    "default": 1.0
                },
                "drive": {
                    "type": "number",
                    "description": "Input drive into the quantiser (0.25-4, default 1). Above 1 pushes harder for a louder, more distorted crush; below 1 backs the level off. Pair a hot drive with a lower output_gain to keep the distortion without clipping.",
                    "minimum": 0.25,
                    "maximum": 4,
                    "default": 1.0
                },
                "output_gain": {
                    "type": "number",
                    "description": "Level applied after the quantiser (0.25-4, default 1). Use it to pull back a hot drive: the crush already happened, so lowering this restores headroom without softening the distortion.",
                    "minimum": 0.25,
                    "maximum": 4,
                    "default": 1.0
                },
                "anti_alias": {
                    "type": "number",
                    "description": "Anti-aliasing amount from 0 to 1 (default 0.5). 0 keeps the raw stepped edges for maximum grit; 1 smooths the hardest aliasing artefacts.",
                    "minimum": 0,
                    "maximum": 1,
                    "default": 0.5
                },
                "mode": {
                    "type": "string",
                    "description": "Quantiser curve. lin (default) uses evenly spaced steps like classic fixed-point hardware; log uses coarser steps near silence, closer to companded gear.",
                    "enum": ["lin", "log"],
                    "default": "lin"
                },
                "format": {
                    "type": "string",
                    "description": "Output audio format. Default mp3 (192 kbps); wav/flac are lossless re-encodes, ogg is Vorbis, m4a is AAC.",
                    "enum": ["mp3", "wav", "ogg", "flac", "m4a"],
                    "default": "mp3"
                }
            },
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ],
            "additionalProperties": false
        });
        assert_eq!(derived, authored);
    }

    #[test]
    fn output_filename_uses_bitcrush_suffix() {
        assert_eq!(
            filename_with_suffix("drums.wav", "-bitcrush", "mp3"),
            "drums-bitcrush.mp3"
        );
        assert_eq!(
            filename_with_suffix("voice memo.m4a", "-bitcrush", "flac"),
            "voice memo-bitcrush.flac"
        );
    }
}
