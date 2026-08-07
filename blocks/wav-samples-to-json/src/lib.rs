//! gizza-ai/wav-samples-to-json — export a WAV clip's PCM samples + format
//! metadata as JSON.
//!
//! Thin chat-skill wrapper around `gizza-ai-wav-samples-to-json-core`. The
//! descriptor is the single source for the chat schema, CLI, and generated page
//! controls; `handle()` delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    layout: String,
    #[serde(default)]
    value_scale: String,
    #[serde(default = "default_precision")]
    precision: u32,
    #[serde(default)]
    start_frame: u64,
    #[serde(default = "default_frame_step")]
    frame_step: u64,
    #[serde(default = "default_max_frames")]
    max_frames: u64,
    #[serde(default = "default_indent")]
    indent: u32,
}

fn default_precision() -> u32 {
    6
}
fn default_frame_step() -> u64 {
    1
}
fn default_max_frames() -> u64 {
    50_000
}
fn default_indent() -> u32 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Uncompressed WAV audio bytes encoded as base64 (default) or hex, e.g. the output of `base64 clip.wav`. Only RIFF/WAVE PCM 8/16/24/32-bit integer or 32/64-bit IEEE float is decoded; compressed MP3/AAC/FLAC/Ogg/A-law/mu-law is rejected with a message naming the detected format."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("Encoding of the pasted WAV bytes: 'base64' (default) or 'hex'. Hex may include whitespace, ':' or '-' separators."),
        )
        .param(
            Param::enumv("output", ["full", "samples", "metadata"])
                .default("full")
                .describe("What the JSON document contains. 'full' (default) = an object with a 'metadata' block (sampleRate, channels, bitDepth, encoding, formatTag, byteRate, blockAlign, totalFrames, durationSeconds), an 'export' block describing the window, and the 'samples' array. 'samples' = the bare sample array only, ready to paste into code. 'metadata' = the format object only, no samples."),
        )
        .param(
            Param::enumv("layout", ["interleaved", "channels"])
                .default("interleaved")
                .describe("Shape of the samples array. 'interleaved' (default) = one flat array with channels interleaved L,R,L,R... 'channels' = an array per channel (de-interleaved), so samples[0] is the left channel and samples[1] the right."),
        )
        .param(
            Param::enumv("value_scale", ["float", "int", "db"])
                .default("float")
                .describe("How each sample value is written. 'float' (default) = normalized amplitude in [-1,1]; 'int' = the raw PCM integer at the source bit depth (a 16-bit source gives -32768..32767; float-WAV sources scale to 32-bit); 'db' = dBFS magnitude (0 = full scale, silence = -120)."),
        )
        .param(
            Param::integer("precision")
                .min(0.0)
                .max(15.0)
                .default(6)
                .describe("Decimal places for 'float' and 'db' sample values (0-15, default 6). Ignored for the integer 'int' scale. Lower it to shrink the JSON: 3 is plenty for a waveform preview."),
        )
        .param(
            Param::integer("start_frame")
                .min(0.0)
                .default(0)
                .describe("Zero-based index of the first sample frame to export (default 0). One frame is one sample per channel, so at 44100 Hz frame 44100 is one second in. Errors if it is at or past the end of the clip."),
        )
        .param(
            Param::integer("frame_step")
                .min(1.0)
                .max(10_000.0)
                .default(1)
                .describe("Keep every Nth sample frame (1-10000, default 1 = keep every frame). Use it to decimate a long clip down to a waveform-preview-sized array, e.g. 441 gives about 100 points per second at 44100 Hz."),
        )
        .param(
            Param::integer("max_frames")
                .min(1.0)
                .max(200_000.0)
                .default(50_000)
                .describe("Maximum number of sample frames to export AFTER frame_step decimation (1-200000, default 50000). Bounds the JSON size; use start_frame to page through a longer clip."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("JSON indentation width in spaces (0-8, default 2). Set 0 for compact single-line JSON. Numeric sample arrays always stay on one line so the document does not explode to one value per line."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WavSamplesToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/wav-samples-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Export WAV PCM samples as a JSON array with the format metadata.",
    skill(
        description = "Decode an uncompressed WAV clip (pasted as base64 or hex bytes) and emit JSON: the format metadata read from the fmt chunk (sample rate, channels, bit depth, encoding, format tag, byte rate, block align, total frames, duration) alongside the decoded PCM samples as a JSON array you can paste straight into code. Choose interleaved or per-channel layout, the value scale (normalized float in [-1,1], raw PCM integer at the source bit depth, or dBFS), the decimal precision, the JSON indent (0 = compact), a start_frame/max_frames window, and a frame_step stride that decimates a long clip down to a waveform-preview-sized array. Output can be the full document, just the samples array, or just the metadata object. Decodes RIFF/WAVE PCM 8/16/24/32-bit integer and 32/64-bit IEEE float; compressed MP3/AAC/FLAC/Ogg/A-law/mu-law inputs are rejected clearly rather than guessed.",
        parameters = schema_json()
    ),
)]
impl WavSamplesToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "wav-samples-to-json", |a: Args| {
            gizza_ai_wav_samples_to_json_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                &a.layout,
                &a.value_scale,
                a.precision,
                a.start_frame,
                a.frame_step,
                a.max_frames,
                a.indent,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();

        assert_eq!(schema.get("required").unwrap(), &serde_json::json!(["input"]));
        assert_eq!(schema.get("additionalProperties").unwrap(), false);

        assert_eq!(props["input"]["type"], "string");

        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
        assert_eq!(props["input_format"]["default"], "base64");

        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["full", "samples", "metadata"])
        );
        assert_eq!(props["output"]["default"], "full");

        assert_eq!(
            props["layout"]["enum"],
            serde_json::json!(["interleaved", "channels"])
        );
        assert_eq!(props["layout"]["default"], "interleaved");

        assert_eq!(
            props["value_scale"]["enum"],
            serde_json::json!(["float", "int", "db"])
        );
        assert_eq!(props["value_scale"]["default"], "float");

        assert_eq!(props["precision"]["type"], "integer");
        assert_eq!(props["precision"]["minimum"], 0);
        assert_eq!(props["precision"]["maximum"], 15);
        assert_eq!(props["precision"]["default"], 6);

        assert_eq!(props["start_frame"]["type"], "integer");
        assert_eq!(props["start_frame"]["minimum"], 0);
        assert_eq!(props["start_frame"]["default"], 0);

        assert_eq!(props["frame_step"]["type"], "integer");
        assert_eq!(props["frame_step"]["minimum"], 1);
        assert_eq!(props["frame_step"]["maximum"], 10000);
        assert_eq!(props["frame_step"]["default"], 1);

        assert_eq!(props["max_frames"]["type"], "integer");
        assert_eq!(props["max_frames"]["minimum"], 1);
        assert_eq!(props["max_frames"]["maximum"], 200000);
        assert_eq!(props["max_frames"]["default"], 50000);

        assert_eq!(props["indent"]["type"], "integer");
        assert_eq!(props["indent"]["minimum"], 0);
        assert_eq!(props["indent"]["maximum"], 8);
        assert_eq!(props["indent"]["default"], 2);

        for key in [
            "input",
            "input_format",
            "output",
            "layout",
            "value_scale",
            "precision",
            "start_frame",
            "frame_step",
            "max_frames",
            "indent",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing description for {key}"
            );
        }
    }
}
