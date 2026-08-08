//! gizza-ai/wav-to-raw-pcm-extractor — strip a WAV file's RIFF header and return
//! only the `data` chunk's raw interleaved PCM bytes.
//!
//! Thin chat-skill wrapper around `gizza-ai-wav-to-raw-pcm-extractor-core`. The
//! descriptor is the single source for the chat schema, the CLI, and the
//! generated page controls; `handle()` delegates to `block_utils::run_skill`.
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
    sample_format: String,
    #[serde(default)]
    channels: String,
    #[serde(default)]
    start_frame: u64,
    #[serde(default)]
    max_frames: u64,
    #[serde(default = "default_line_bytes")]
    line_bytes: u32,
}

fn default_line_bytes() -> u32 {
    16
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Uncompressed WAV file bytes encoded as base64 (default) or hex, e.g. the output of `base64 clip.wav`. The RIFF header, the `fmt ` chunk and every other chunk (LIST, fact, cue) are discarded; only the `data` chunk payload is returned."),
        )
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe("Encoding of the pasted WAV bytes: 'base64' (default) or 'hex'. Hex may contain whitespace, ':' or '-' separators."),
        )
        .param(
            Param::enumv("output", ["base64", "hex", "c_array", "info"])
                .default("base64")
                .describe("How the extracted PCM is rendered. 'base64' (default) = one base64 string, ready to decode back to a .pcm/.raw file. 'hex' = lowercase hex bytes. 'c_array' = a compilable `const unsigned char pcm_data[]` C array plus its length. 'info' = a format report (sample rate, channels, encoding, frame count, chunk map) with ready-to-run ffmpeg, SoX and Audacity re-import settings, and no sample bytes."),
        )
        .param(
            Param::enumv(
                "sample_format",
                [
                    "source", "u8", "s16le", "s16be", "s24le", "s24be", "s32le", "s32be", "f32le",
                    "f32be",
                ],
            )
            .default("source")
            .describe("Sample encoding of the extracted PCM. 'source' (default) copies the data chunk byte-for-byte with no decode or re-encode. Any other value decodes each sample and re-encodes it: 'u8' unsigned 8-bit, 's16le'/'s16be' signed 16-bit, 's24le'/'s24be' signed 24-bit, 's32le'/'s32be' signed 32-bit, 'f32le'/'f32be' IEEE float — the same names ffmpeg's `-f` and SoX's raw types use. A-law/mu-law files can only be extracted with 'source'."),
        )
        .param(
            Param::enumv("channels", ["all", "mono", "left", "right"])
                .default("all")
                .describe("Which channels end up in the output. 'all' (default) keeps every channel interleaved as stored; 'mono' averages all channels into one; 'left' keeps channel 1 only; 'right' keeps channel 2 only (errors on a mono clip). Anything other than 'all' forces the decode/re-encode path."),
        )
        .param(
            Param::integer("start_frame")
                .min(0.0)
                .default(0)
                .describe("Zero-based index of the first sample frame to extract (default 0). One frame is one sample per channel, so at 44100 Hz frame 44100 is one second in. Errors if it is at or past the end of the clip."),
        )
        .param(
            Param::integer("max_frames")
                .min(0.0)
                .default(0)
                .describe("How many sample frames to extract, starting at start_frame (default 0 = extract everything to the end of the clip). Use it with start_frame to cut a window out of a long recording and stay under the output size cap."),
        )
        .param(
            Param::integer("line_bytes")
                .min(0.0)
                .max(64.0)
                .default(16)
                .describe("Bytes per line for the 'hex' and 'c_array' outputs (0-64, default 16). Set 0 for one unbroken hex run, which is what `xxd -r -p` expects. Ignored by the 'base64' and 'info' outputs."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WavToRawPcmExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/wav-to-raw-pcm-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip a WAV header and extract the raw interleaved PCM data chunk.",
    skill(
        description = "Extract the bare PCM payload from an uncompressed WAV file (pasted as base64 or hex bytes): the RIFF header, the fmt chunk and every other chunk (LIST, fact, cue, trailing metadata) are removed and only the data chunk's interleaved sample bytes come back. The default sample_format='source' with channels='all' slices the payload out byte-for-byte with no decode or re-encode, so the output is exactly what the container stored; asking for another sample format (u8, s16le/be, s24le/be, s32le/be, f32le/be) or a channel selection (mono downmix, left, right) decodes and re-encodes each sample the way `ffmpeg -f s16le` or `sox -t raw` would. Extract a window with start_frame/max_frames, and render the result as base64, hex, a compilable C array, or an 'info' report giving the sample rate, channel count, encoding, frame count and chunk map plus ready-to-run ffmpeg, SoX and Audacity re-import settings — raw PCM is headerless, so those parameters have to travel with it.",
        parameters = schema_json()
    ),
)]
impl WavToRawPcmExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "wav-to-raw-pcm-extractor", |a: Args| {
            gizza_ai_wav_to_raw_pcm_extractor_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                &a.sample_format,
                &a.channels,
                a.start_frame,
                a.max_frames,
                a.line_bytes,
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

    /// Drift guard: the live descriptor must keep emitting the schema chat, the
    /// CLI and the page form all consume. Regenerate this when the descriptor
    /// changes intentionally.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();

        assert_eq!(schema.get("type").unwrap(), "object");
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
            serde_json::json!(["base64", "hex", "c_array", "info"])
        );
        assert_eq!(props["output"]["default"], "base64");

        assert_eq!(
            props["sample_format"]["enum"],
            serde_json::json!([
                "source", "u8", "s16le", "s16be", "s24le", "s24be", "s32le", "s32be", "f32le",
                "f32be"
            ])
        );
        assert_eq!(props["sample_format"]["default"], "source");

        assert_eq!(
            props["channels"]["enum"],
            serde_json::json!(["all", "mono", "left", "right"])
        );
        assert_eq!(props["channels"]["default"], "all");

        assert_eq!(props["start_frame"]["type"], "integer");
        assert_eq!(props["start_frame"]["minimum"], 0);
        assert_eq!(props["start_frame"]["default"], 0);

        assert_eq!(props["max_frames"]["type"], "integer");
        assert_eq!(props["max_frames"]["minimum"], 0);
        assert_eq!(props["max_frames"]["default"], 0);

        assert_eq!(props["line_bytes"]["type"], "integer");
        assert_eq!(props["line_bytes"]["minimum"], 0);
        assert_eq!(props["line_bytes"]["maximum"], 64);
        assert_eq!(props["line_bytes"]["default"], 16);

        for key in [
            "input",
            "input_format",
            "output",
            "sample_format",
            "channels",
            "start_frame",
            "max_frames",
            "line_bytes",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing description for {key}"
            );
        }
    }
}
