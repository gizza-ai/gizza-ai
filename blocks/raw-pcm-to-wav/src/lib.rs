//! gizza-ai/raw-pcm-to-wav — wrap headerless raw PCM samples in a RIFF/WAVE
//! header, using the five facts the bytes themselves cannot carry.
//!
//! Thin chat-skill wrapper around `gizza-ai-raw-pcm-to-wav-core`. The descriptor
//! is the single source for the chat schema, the CLI, and the generated page
//! controls; `handle()` delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_sample_rate() -> u32 {
    44100
}
fn default_channels() -> u32 {
    2
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default = "default_sample_rate")]
    sample_rate: u32,
    #[serde(default = "default_channels")]
    channels: u32,
    #[serde(default)]
    bit_depth: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    byte_order: String,
    #[serde(default)]
    skip_bytes: u64,
    #[serde(default)]
    max_frames: u64,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and the CLI + page controls). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .multiline()
                .describe(
                    "The headerless PCM sample bytes, pasted as base64 (e.g. the output of `base64 dump.pcm`), \
                     as hex (e.g. `xxd -p dump.pcm`), or as a `data:…;base64,…` URI. Raw PCM has no magic bytes \
                     and no header, so the five parameters below describe it instead. Decoded input is capped \
                     at 12 MiB.",
                ),
        )
        .param(
            Param::enumv("input_format", ["auto", "base64", "hex"])
                .default("auto")
                .describe(
                    "How the pasted bytes are encoded. 'auto' (default) treats an all-hex-digit even-length \
                     payload as hex and everything else as base64; 'base64' also accepts the URL-safe alphabet \
                     and missing padding; 'hex' additionally allows ':', '-' and ',' separators.",
                ),
        )
        .param(
            Param::integer("sample_rate")
                .min(1.0)
                .max(768_000.0)
                .default(44100)
                .describe(
                    "Sample rate in Hz the dump was recorded at (default 44100). Raw PCM stores no rate, so a \
                     wrong value plays the audio at the wrong speed and pitch without any error. Common values: \
                     8000 and 16000 (telephony/speech), 22050, 44100 (CD), 48000 (video).",
                ),
        )
        .param(
            Param::integer("channels")
                .min(1.0)
                .max(16.0)
                .default(2)
                .describe(
                    "Number of interleaved channels (default 2). Use 1 for a mono dump and 2 for stereo, where \
                     samples alternate L,R,L,R. A wrong channel count changes the frame size, which usually \
                     shows up as a leftover partial frame in the info report.",
                ),
        )
        .param(
            Param::enumv("bit_depth", ["8", "16", "24", "32", "64"])
                .default("16")
                .describe(
                    "Bits per sample per channel (default 16). 8/16/24/32 are valid for integer samples, 32 and \
                     64 for encoding=float; encoding=mulaw/alaw ignore this and use 8. This is ffmpeg's `-f \
                     s16le` width and SoX's `-b 16`.",
                ),
        )
        .param(
            Param::enumv("encoding", ["signed", "unsigned", "float", "mulaw", "alaw"])
                .default("signed")
                .describe(
                    "How one sample value is encoded (default signed). 'signed' = two's complement, the usual \
                     form for 16/24/32-bit dumps. 'unsigned' = offset binary, common for 8-bit. 'float' = IEEE \
                     754 at 32 or 64 bits, what audio engines dump. 'mulaw'/'alaw' = 8-bit G.711 companded \
                     telephony samples, kept companded in the WAV (format tag 7 / 6).",
                ),
        )
        .param(
            Param::enumv("byte_order", ["little", "big"])
                .default("little")
                .describe(
                    "Byte order of multi-byte samples (default little, what a PC or phone produces). 'big' \
                     byte-swaps every sample into the little-endian order WAVE requires — use it for network \
                     or big-endian-machine captures. Irrelevant for 8-bit and G.711 data.",
                ),
        )
        .param(
            Param::integer("skip_bytes")
                .min(0.0)
                .default(0)
                .describe(
                    "How many leading bytes to drop before the samples start (default 0). Use it to skip a \
                     proprietary or already-known header in front of the raw data. Errors if it lands at or \
                     past the end of the input.",
                ),
        )
        .param(
            Param::integer("max_frames")
                .min(0.0)
                .default(0)
                .describe(
                    "How many sample frames to wrap, after skip_bytes (default 0 = everything). One frame is \
                     one sample per channel, so at 44100 Hz 44100 frames is one second — multiply seconds by \
                     sample_rate to cut a window and stay under the 12 MiB output cap.",
                ),
        )
        .param(
            Param::enumv("output", ["data_url", "base64", "hex", "info"])
                .default("data_url")
                .describe(
                    "What to return. 'data_url' (default) = a `data:audio/wav;base64,…` URI you can save or \
                     play directly. 'base64' = the WAV bytes as plain base64 (pipe through `base64 -d > \
                     out.wav`). 'hex' = lowercase unbroken hex, `xxd -r -p` compatible, capped at 4 MiB of \
                     audio. 'info' = a report of the frame maths, the exact header that would be written, and \
                     the equivalent ffmpeg / SoX / import-dialog settings — no audio bytes.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RawPcmToWav;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/raw-pcm-to-wav",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Wrap headerless raw PCM samples in a RIFF/WAVE header.",
    skill(
        description = "Wrap headerless raw PCM sample bytes in a RIFF/WAVE header so ordinary players and editors open them. Raw PCM carries no magic bytes and no header, so you supply what it cannot say: sample_rate, channels, bit_depth (8/16/24/32/64), encoding (signed, unsigned, float, mulaw, alaw) and byte_order (little/big). Paste the bytes as base64, hex, or a data: URI. Samples are never decoded or resampled — the only edits are the two WAVE rules: multi-byte samples are byte-swapped to little-endian, and 8-bit PCM is stored unsigned while wider integer depths are stored signed. Float and G.711 data get the 18-byte `fmt ` chunk plus the `fact` chunk WAVE requires (format tag 3, 6 or 7); plain integer PCM gets the canonical 44-byte header. Use skip_bytes to drop a leading proprietary header and max_frames to wrap only a window. output=data_url|base64|hex returns the WAV, and output=info returns the frame maths, the header that would be written, any leftover partial frame (the usual sign that bit_depth or channels is wrong), plus the equivalent ffmpeg and SoX commands. It does not resample, re-encode, or auto-detect the layout, and compressed sources such as GSM or ADPCM are not linear PCM and cannot be wrapped.",
        parameters = schema_json()
    ),
)]
impl RawPcmToWav {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "raw-pcm-to-wav", |a: Args| {
            gizza_ai_raw_pcm_to_wav_core::run(
                &a.input,
                &a.input_format,
                a.sample_rate,
                a.channels,
                &a.bit_depth,
                &a.encoding,
                &a.byte_order,
                a.skip_bytes,
                a.max_frames,
                &a.output,
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

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing shape never changes silently.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "required": ["input"],
                "properties": {
                    "input": { "type": "string", "description": "The headerless PCM sample bytes, pasted as base64 (e.g. the output of `base64 dump.pcm`), as hex (e.g. `xxd -p dump.pcm`), or as a `data:…;base64,…` URI. Raw PCM has no magic bytes and no header, so the five parameters below describe it instead. Decoded input is capped at 12 MiB." },
                    "input_format": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the pasted bytes are encoded. 'auto' (default) treats an all-hex-digit even-length payload as hex and everything else as base64; 'base64' also accepts the URL-safe alphabet and missing padding; 'hex' additionally allows ':', '-' and ',' separators." },
                    "sample_rate": { "type": "integer", "minimum": 1, "maximum": 768000, "default": 44100, "description": "Sample rate in Hz the dump was recorded at (default 44100). Raw PCM stores no rate, so a wrong value plays the audio at the wrong speed and pitch without any error. Common values: 8000 and 16000 (telephony/speech), 22050, 44100 (CD), 48000 (video)." },
                    "channels": { "type": "integer", "minimum": 1, "maximum": 16, "default": 2, "description": "Number of interleaved channels (default 2). Use 1 for a mono dump and 2 for stereo, where samples alternate L,R,L,R. A wrong channel count changes the frame size, which usually shows up as a leftover partial frame in the info report." },
                    "bit_depth": { "type": "string", "enum": ["8", "16", "24", "32", "64"], "default": "16", "description": "Bits per sample per channel (default 16). 8/16/24/32 are valid for integer samples, 32 and 64 for encoding=float; encoding=mulaw/alaw ignore this and use 8. This is ffmpeg's `-f s16le` width and SoX's `-b 16`." },
                    "encoding": { "type": "string", "enum": ["signed", "unsigned", "float", "mulaw", "alaw"], "default": "signed", "description": "How one sample value is encoded (default signed). 'signed' = two's complement, the usual form for 16/24/32-bit dumps. 'unsigned' = offset binary, common for 8-bit. 'float' = IEEE 754 at 32 or 64 bits, what audio engines dump. 'mulaw'/'alaw' = 8-bit G.711 companded telephony samples, kept companded in the WAV (format tag 7 / 6)." },
                    "byte_order": { "type": "string", "enum": ["little", "big"], "default": "little", "description": "Byte order of multi-byte samples (default little, what a PC or phone produces). 'big' byte-swaps every sample into the little-endian order WAVE requires — use it for network or big-endian-machine captures. Irrelevant for 8-bit and G.711 data." },
                    "skip_bytes": { "type": "integer", "minimum": 0, "default": 0, "description": "How many leading bytes to drop before the samples start (default 0). Use it to skip a proprietary or already-known header in front of the raw data. Errors if it lands at or past the end of the input." },
                    "max_frames": { "type": "integer", "minimum": 0, "default": 0, "description": "How many sample frames to wrap, after skip_bytes (default 0 = everything). One frame is one sample per channel, so at 44100 Hz 44100 frames is one second — multiply seconds by sample_rate to cut a window and stay under the 12 MiB output cap." },
                    "output": { "type": "string", "enum": ["data_url", "base64", "hex", "info"], "default": "data_url", "description": "What to return. 'data_url' (default) = a `data:audio/wav;base64,…` URI you can save or play directly. 'base64' = the WAV bytes as plain base64 (pipe through `base64 -d > out.wav`). 'hex' = lowercase unbroken hex, `xxd -r -p` compatible, capped at 4 MiB of audio. 'info' = a report of the frame maths, the exact header that would be written, and the equivalent ffmpeg / SoX / import-dialog settings — no audio bytes." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The block's arg plumbing must reach core with every param in order, and
    /// the two numeric defaults must survive an omitted field.
    #[test]
    fn args_deserialize_with_defaults() {
        let a: Args = serde_json::from_str(r#"{"input":"0102"}"#).unwrap();
        assert_eq!(a.input, "0102");
        assert_eq!(a.sample_rate, 44100);
        assert_eq!(a.channels, 2);
        assert_eq!(a.bit_depth, "");
        assert_eq!(a.skip_bytes, 0);
        assert_eq!(a.max_frames, 0);
        assert_eq!(a.output, "");
    }
}
