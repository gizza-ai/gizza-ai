//! gizza-ai/ffmpeg-filtergraph-builder — compile an ordered list of described
//! filter steps into a validated ffmpeg filtergraph string.
//!
//! Pure text → text. The block **builds a string and never runs ffmpeg**: no
//! process is spawned and no user-supplied filter is executed (see the core
//! crate for the escaping/allowlist rules that keep the emitted string safe to
//! paste). The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ffmpeg_filtergraph_builder_core::build_from_strs;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    steps: String,
    #[serde(default = "default_stream")]
    stream: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_input_label")]
    input_label: String,
    #[serde(default = "default_output_label")]
    output_label: String,
    #[serde(default = "default_input_file")]
    input_file: String,
    #[serde(default = "default_output_file")]
    output_file: String,
    #[serde(default)]
    explain: bool,
}

fn default_stream() -> String {
    "video".into()
}
fn default_output() -> String {
    "filter_complex".into()
}
fn default_input_label() -> String {
    "auto".into()
}
fn default_output_label() -> String {
    "out".into()
}
fn default_input_file() -> String {
    "input.mp4".into()
}
fn default_output_file() -> String {
    "output.mp4".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("steps")
                .required()
                .describe(
                    "The filter steps in order, one per line (or separated by ';' / 'then'). Video steps: scale, crop, pad, fade, rotate, flip, grayscale, blur, sharpen, fps, speed, trim, reverse, brightness, contrast, saturation, hue, text, raw. Audio steps: volume, fade, trim, speed, reverse, normalize, mono, highpass, lowpass, raw. Example: 'scale to 720p' / 'crop to square' / 'fade in 1s'. Max 30 steps.",
                ),
        )
        .param(
            Param::enumv("stream", ["video", "audio"])
                .default("video")
                .describe("Which stream the steps apply to: 'video' (default) or 'audio'. Audio steps use the a-prefixed ffmpeg filters (afade, atrim, atempo)."),
        )
        .param(
            Param::enumv("output", ["filter_complex", "filter_chain", "command"])
                .default("filter_complex")
                .describe("Shape of the returned string: 'filter_complex' (default) wraps the chain in [in]…[out] pad labels; 'filter_chain' is the bare comma-separated chain for -vf/-af; 'command' is a complete ffmpeg command line."),
        )
        .param(
            Param::string("input_label")
                .default("auto")
                .describe("Source pad label for filter_complex/command output, e.g. '0:v' or '1:a'. 'auto' (default) uses 0:v for video and 0:a for audio."),
        )
        .param(
            Param::string("output_label")
                .default("out")
                .describe("Sink pad label for filter_complex/command output (default 'out'), used as [out] and in the -map argument."),
        )
        .param(
            Param::string("input_file")
                .default("input.mp4")
                .describe("Input file name used by the 'command' output form (default input.mp4). Letters, digits, '.', '_', '-', '+' and '/' only — shell metacharacters are rejected."),
        )
        .param(
            Param::string("output_file")
                .default("output.mp4")
                .describe("Output file name used by the 'command' output form (default output.mp4). Same allowed characters as input_file."),
        )
        .param(
            Param::boolean("explain")
                .default(false)
                .describe("When true, append '#' comment lines showing which ffmpeg filter each step compiled to (default false)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ffmpeg-filtergraph-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a validated ffmpeg filtergraph from described filter steps",
    skill(
        description = "Turn an ordered list of described filter steps (one per line, e.g. 'scale to 720p', 'crop to square', 'fade in 1s') into a validated ffmpeg filtergraph string. Returns a -filter_complex graph with [in]/[out] pad labels, a bare -vf/-af chain, or a complete ffmpeg command line. Handles video (scale, crop, pad, fade, rotate, flip, grayscale, blur, sharpen, fps, speed, trim, reverse, brightness, contrast, saturation, hue, text) and audio (volume, fade, trim, speed, reverse, normalize, mono, highpass, lowpass) steps, plus a syntax-checked 'raw <filter>' escape hatch. NOTHING is executed and no media is read — it only composes and validates the string you would paste into your own ffmpeg invocation. To actually transform a file, use one of the video-*/audio-* tools instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ffmpeg-filtergraph-builder", |a: Args| {
            build_from_strs(
                &a.steps,
                &a.stream,
                &a.output,
                &a.input_label,
                &a.output_label,
                &a.input_file,
                &a.output_file,
                a.explain,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "steps": { "type": "string", "description": "The filter steps in order, one per line (or separated by ';' / 'then'). Video steps: scale, crop, pad, fade, rotate, flip, grayscale, blur, sharpen, fps, speed, trim, reverse, brightness, contrast, saturation, hue, text, raw. Audio steps: volume, fade, trim, speed, reverse, normalize, mono, highpass, lowpass, raw. Example: 'scale to 720p' / 'crop to square' / 'fade in 1s'. Max 30 steps." },
                    "stream": { "type": "string", "enum": ["video", "audio"], "default": "video", "description": "Which stream the steps apply to: 'video' (default) or 'audio'. Audio steps use the a-prefixed ffmpeg filters (afade, atrim, atempo)." },
                    "output": { "type": "string", "enum": ["filter_complex", "filter_chain", "command"], "default": "filter_complex", "description": "Shape of the returned string: 'filter_complex' (default) wraps the chain in [in]…[out] pad labels; 'filter_chain' is the bare comma-separated chain for -vf/-af; 'command' is a complete ffmpeg command line." },
                    "input_label": { "type": "string", "default": "auto", "description": "Source pad label for filter_complex/command output, e.g. '0:v' or '1:a'. 'auto' (default) uses 0:v for video and 0:a for audio." },
                    "output_label": { "type": "string", "default": "out", "description": "Sink pad label for filter_complex/command output (default 'out'), used as [out] and in the -map argument." },
                    "input_file": { "type": "string", "default": "input.mp4", "description": "Input file name used by the 'command' output form (default input.mp4). Letters, digits, '.', '_', '-', '+' and '/' only — shell metacharacters are rejected." },
                    "output_file": { "type": "string", "default": "output.mp4", "description": "Output file name used by the 'command' output form (default output.mp4). Same allowed characters as input_file." },
                    "explain": { "type": "boolean", "default": false, "description": "When true, append '#' comment lines showing which ffmpeg filter each step compiled to (default false)." }
                },
                "required": ["steps"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
