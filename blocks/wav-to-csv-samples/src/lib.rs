//! gizza-ai/wav-to-csv-samples — export decoded WAV PCM samples to CSV.
//!
//! Thin chat-skill wrapper around `gizza-ai-wav-to-csv-samples-core`. The
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
    value_scale: String,
    #[serde(default = "default_precision")]
    precision: u32,
    #[serde(default)]
    index_column: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    start_frame: u64,
    #[serde(default = "default_max_frames")]
    max_frames: u64,
}

fn default_precision() -> u32 {
    6
}
fn default_true() -> bool {
    true
}
fn default_max_frames() -> u64 {
    100_000
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Uncompressed WAV audio bytes encoded as base64 (default) or hex. Only RIFF/WAVE PCM 8/16/24/32-bit integer or 32/64-bit IEEE float is decoded; compressed MP3/AAC/FLAC/Ogg/A-law/mu-law is rejected with a clear message."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("Encoding of the pasted WAV bytes: 'base64' (default) or 'hex'. Hex may include whitespace, ':' or '-' separators."),
        )
        .param(
            Param::enumv("value_scale", ["float", "int", "db"])
                .default("float")
                .describe("How each sample value is written. 'float' (default) = normalized amplitude in [-1,1]; 'int' = the raw PCM integer at the source bit depth (float-WAV sources scale to 32-bit); 'db' = dBFS magnitude (0 = full scale, silence = -120)."),
        )
        .param(
            Param::integer("precision")
                .min(0.0)
                .max(15.0)
                .default(6)
                .describe("Decimal places for 'float' and 'db' sample values and for the time column (0-15, default 6). Ignored for the integer 'int' scale."),
        )
        .param(
            Param::enumv("index_column", ["time", "sample", "both", "none"])
                .default("time")
                .describe("First column(s) before the channel columns: 'time' (seconds, default), 'sample' (absolute sample-frame index), 'both', or 'none'."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab"])
                .default("comma")
                .describe("Field separator between columns: 'comma' (default, standard CSV), 'semicolon', or 'tab' (TSV)."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Write a header row naming the columns (e.g. time_s, channel_1, channel_2). Default true; set false for headerless numeric data."),
        )
        .param(
            Param::integer("start_frame")
                .min(0.0)
                .default(0)
                .describe("Zero-based index of the first sample frame to export (default 0). Use with max_frames to export a window of a longer clip. Errors if it is at or past the end of the clip."),
        )
        .param(
            Param::integer("max_frames")
                .min(1.0)
                .max(500_000.0)
                .default(100_000)
                .describe("Maximum number of sample frames (one frame = one sample per channel) to export (1-500000, default 100000). Export covers frames start_frame..start_frame+max_frames, clamped to the clip length."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WavToCsvSamples;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/wav-to-csv-samples",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Export decoded WAV PCM samples to CSV, one column per channel.",
    skill(
        description = "Decode an uncompressed WAV clip (pasted as base64 or hex bytes) and export its PCM samples to CSV: one column per audio channel, optionally preceded by a time (seconds) and/or sample-index column. Choose the sample value scale (normalized float in [-1,1], raw PCM integer at the source bit depth, or dBFS), the decimal precision, the delimiter (comma/semicolon/tab), whether to write a header row, and a start_frame/max_frames window for long clips. Decodes RIFF/WAVE PCM 8/16/24/32-bit integer and 32/64-bit IEEE float; compressed MP3/AAC/FLAC/Ogg/A-law/mu-law inputs are rejected clearly rather than guessed.",
        parameters = schema_json()
    ),
)]
impl WavToCsvSamples {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "wav-to-csv-samples", |a: Args| {
            gizza_ai_wav_to_csv_samples_core::run(
                &a.input,
                &a.input_format,
                &a.value_scale,
                a.precision,
                &a.index_column,
                &a.delimiter,
                a.header,
                a.start_frame,
                a.max_frames,
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

        assert_eq!(props["input_format"]["enum"], serde_json::json!(["base64", "hex"]));
        assert_eq!(props["input_format"]["default"], "base64");

        assert_eq!(
            props["value_scale"]["enum"],
            serde_json::json!(["float", "int", "db"])
        );
        assert_eq!(props["value_scale"]["default"], "float");

        assert_eq!(props["precision"]["type"], "integer");
        assert_eq!(props["precision"]["minimum"], 0);
        assert_eq!(props["precision"]["maximum"], 15);
        assert_eq!(props["precision"]["default"], 6);

        assert_eq!(
            props["index_column"]["enum"],
            serde_json::json!(["time", "sample", "both", "none"])
        );
        assert_eq!(props["index_column"]["default"], "time");

        assert_eq!(
            props["delimiter"]["enum"],
            serde_json::json!(["comma", "semicolon", "tab"])
        );
        assert_eq!(props["delimiter"]["default"], "comma");

        assert_eq!(props["header"]["type"], "boolean");
        assert_eq!(props["header"]["default"], true);

        assert_eq!(props["start_frame"]["type"], "integer");
        assert_eq!(props["start_frame"]["minimum"], 0);
        assert_eq!(props["start_frame"]["default"], 0);

        assert_eq!(props["max_frames"]["type"], "integer");
        assert_eq!(props["max_frames"]["minimum"], 1);
        assert_eq!(props["max_frames"]["maximum"], 500000);
        assert_eq!(props["max_frames"]["default"], 100000);

        for key in [
            "input",
            "input_format",
            "value_scale",
            "precision",
            "index_column",
            "delimiter",
            "header",
            "start_frame",
            "max_frames",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing description for {key}"
            );
        }
    }
}
