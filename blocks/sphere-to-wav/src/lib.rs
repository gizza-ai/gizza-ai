//! gizza-ai/sphere-to-wav — convert a NIST SPHERE (`.sph`) speech-corpus file
//! into a standard RIFF/WAVE file (or headerless raw PCM).
//!
//! Thin chat-skill wrapper around `gizza-ai-sphere-to-wav-core`. The descriptor
//! is the single source for the chat schema, the CLI, and the generated page
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
    encoding: String,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    container: String,
    #[serde(default)]
    byte_order: String,
    #[serde(default)]
    start_sample: u64,
    #[serde(default)]
    max_samples: u64,
}

/// Single source for the chat schema (and the CLI + page controls). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe(
                    "The .sph file's bytes, pasted as base64 (e.g. the output of `base64 utterance.sph`), \
                     as hex, or as a `data:…;base64,…` URI. A NIST SPHERE file starts with the ASCII magic \
                     \"NIST_1A\" followed by the header size (usually 1024). Decoded input is capped at 6 MiB.",
                ),
        )
        .param(
            Param::enumv("input_format", ["auto", "base64", "hex"])
                .default("auto")
                .describe(
                    "How the pasted bytes are encoded. 'auto' (default) treats an all-hex-digit even-length \
                     payload as hex and everything else as base64; 'base64' also accepts the URL-safe alphabet \
                     and missing padding; 'hex' additionally allows ':' and '-' separators.",
                ),
        )
        .param(
            Param::enumv("output", ["data_url", "base64", "hex", "info"])
                .default("data_url")
                .describe(
                    "How the converted audio is returned. 'data_url' (default) = a `data:audio/wav;base64,…` URI \
                     you can save or play directly. 'base64' = the audio bytes as plain base64 (pipe through \
                     `base64 -d > out.wav`). 'hex' = lowercase unbroken hex, `xxd -r -p` compatible, capped at \
                     4 MiB of audio. 'info' = a report of every SPHERE header field, the derived audio \
                     properties, byte order, duration, and what the conversion would produce — no audio bytes.",
                ),
        )
        .param(
            Param::enumv("encoding", ["pcm16", "source", "ulaw", "alaw"])
                .default("pcm16")
                .describe(
                    "Sample encoding of the output. 'pcm16' (default) writes 16-bit signed PCM, expanding \
                     mu-law/A-law corpora so every player opens the result. 'source' keeps the file's own \
                     encoding and bit depth, fixing only the byte order (and 8-bit PCM's signedness, which WAV \
                     requires to be unsigned). 'ulaw'/'alaw' re-encode to 8-bit G.711 companded samples, halving \
                     the size of a 16-bit corpus at telephone quality.",
                ),
        )
        .param(
            Param::enumv("channel", ["all", "1", "2", "mono"])
                .default("all")
                .describe(
                    "Which channels the output keeps. 'all' (default) keeps every channel interleaved; '1' or \
                     '2' keeps one side of a two-channel conversation recording (Switchboard/Fisher style); \
                     'mono' averages all channels into a single downmixed track. '2' errors on a mono file.",
                ),
        )
        .param(
            Param::enumv("container", ["wav", "raw"])
                .default("wav")
                .describe(
                    "Output container. 'wav' (default) writes a RIFF/WAVE file — a 44-byte header for PCM, or an \
                     18-byte `fmt ` chunk plus a `fact` chunk for mu-law/A-law. 'raw' writes only the interleaved \
                     sample bytes with no header, which the 'info' output pairs with a ready-to-run ffmpeg \
                     re-import command.",
                ),
        )
        .param(
            Param::enumv("byte_order", ["auto", "little", "big"])
                .default("auto")
                .describe(
                    "How to read multi-byte samples. 'auto' (default) follows the header's sample_byte_format \
                     ('01' = little-endian, '10' = big-endian) and errors if it is missing or unrecognized. \
                     'little'/'big' override the header — use them when a corpus ships a mislabelled byte order \
                     and the converted audio comes out as loud noise.",
                ),
        )
        .param(
            Param::integer("start_sample")
                .min(0.0)
                .default(0)
                .describe(
                    "Zero-based index of the first sample frame to convert (default 0). One frame is one sample \
                     per channel, so at 16000 Hz frame 16000 is one second in — multiply seconds by sample_rate \
                     to cut a time range. Errors if it is at or past the end of the recording.",
                ),
        )
        .param(
            Param::integer("max_samples")
                .min(0.0)
                .default(0)
                .describe(
                    "How many sample frames to convert, starting at start_sample (default 0 = everything to the \
                     end). Use it to excerpt a long conversation recording and stay under the 12 MiB output cap.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SphereToWav;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sphere-to-wav",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a NIST SPHERE (.sph) speech file to a standard WAV.",
    skill(
        description = "Convert a NIST SPHERE (.sph) speech-corpus file — TIMIT, Switchboard, Fisher, WSJ, TEDLIUM and friends — into a standard RIFF/WAVE file that any player opens. Paste the .sph bytes as base64, hex, or a data: URI. The ASCII SPHERE header is parsed in full (sample_rate, channel_count, sample_n_bytes, sample_coding, sample_byte_format, sample_count and every other field), big-endian samples are byte-swapped to WAV's little-endian order, and mu-law/A-law corpora are expanded to 16-bit PCM by default (encoding=source keeps the original companding, encoding=ulaw/alaw re-encodes). Pick one side of a two-channel conversation with channel=1|2, downmix with channel=mono, excerpt with start_sample/max_samples, and choose container=wav or container=raw headerless samples. output=info returns the full header field table plus duration and byte order instead of audio. Shorten-compressed payloads (sample_coding pcm,embedded-shorten-vX) are detected and reported, not decoded — decompress those with a desktop converter first.",
        parameters = schema_json()
    ),
)]
impl SphereToWav {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sphere-to-wav", |a: Args| {
            gizza_ai_sphere_to_wav_core::run(
                &a.input,
                &a.input_format,
                &a.output,
                &a.encoding,
                &a.channel,
                &a.container,
                &a.byte_order,
                a.start_sample,
                a.max_samples,
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
                    "input": { "type": "string", "description": "The .sph file's bytes, pasted as base64 (e.g. the output of `base64 utterance.sph`), as hex, or as a `data:…;base64,…` URI. A NIST SPHERE file starts with the ASCII magic \"NIST_1A\" followed by the header size (usually 1024). Decoded input is capped at 6 MiB." },
                    "input_format": { "type": "string", "enum": ["auto", "base64", "hex"], "default": "auto", "description": "How the pasted bytes are encoded. 'auto' (default) treats an all-hex-digit even-length payload as hex and everything else as base64; 'base64' also accepts the URL-safe alphabet and missing padding; 'hex' additionally allows ':' and '-' separators." },
                    "output": { "type": "string", "enum": ["data_url", "base64", "hex", "info"], "default": "data_url", "description": "How the converted audio is returned. 'data_url' (default) = a `data:audio/wav;base64,…` URI you can save or play directly. 'base64' = the audio bytes as plain base64 (pipe through `base64 -d > out.wav`). 'hex' = lowercase unbroken hex, `xxd -r -p` compatible, capped at 4 MiB of audio. 'info' = a report of every SPHERE header field, the derived audio properties, byte order, duration, and what the conversion would produce — no audio bytes." },
                    "encoding": { "type": "string", "enum": ["pcm16", "source", "ulaw", "alaw"], "default": "pcm16", "description": "Sample encoding of the output. 'pcm16' (default) writes 16-bit signed PCM, expanding mu-law/A-law corpora so every player opens the result. 'source' keeps the file's own encoding and bit depth, fixing only the byte order (and 8-bit PCM's signedness, which WAV requires to be unsigned). 'ulaw'/'alaw' re-encode to 8-bit G.711 companded samples, halving the size of a 16-bit corpus at telephone quality." },
                    "channel": { "type": "string", "enum": ["all", "1", "2", "mono"], "default": "all", "description": "Which channels the output keeps. 'all' (default) keeps every channel interleaved; '1' or '2' keeps one side of a two-channel conversation recording (Switchboard/Fisher style); 'mono' averages all channels into a single downmixed track. '2' errors on a mono file." },
                    "container": { "type": "string", "enum": ["wav", "raw"], "default": "wav", "description": "Output container. 'wav' (default) writes a RIFF/WAVE file — a 44-byte header for PCM, or an 18-byte `fmt ` chunk plus a `fact` chunk for mu-law/A-law. 'raw' writes only the interleaved sample bytes with no header, which the 'info' output pairs with a ready-to-run ffmpeg re-import command." },
                    "byte_order": { "type": "string", "enum": ["auto", "little", "big"], "default": "auto", "description": "How to read multi-byte samples. 'auto' (default) follows the header's sample_byte_format ('01' = little-endian, '10' = big-endian) and errors if it is missing or unrecognized. 'little'/'big' override the header — use them when a corpus ships a mislabelled byte order and the converted audio comes out as loud noise." },
                    "start_sample": { "type": "integer", "minimum": 0, "default": 0, "description": "Zero-based index of the first sample frame to convert (default 0). One frame is one sample per channel, so at 16000 Hz frame 16000 is one second in — multiply seconds by sample_rate to cut a time range. Errors if it is at or past the end of the recording." },
                    "max_samples": { "type": "integer", "minimum": 0, "default": 0, "description": "How many sample frames to convert, starting at start_sample (default 0 = everything to the end). Use it to excerpt a long conversation recording and stay under the 12 MiB output cap." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The block's arg plumbing must reach core with every param in order.
    #[test]
    fn args_deserialize_with_defaults() {
        let a: Args = serde_json::from_str(r#"{"input":"AAAA"}"#).unwrap();
        assert_eq!(a.input, "AAAA");
        assert_eq!(a.output, "");
        assert_eq!(a.start_sample, 0);
        assert_eq!(a.max_samples, 0);
    }
}
