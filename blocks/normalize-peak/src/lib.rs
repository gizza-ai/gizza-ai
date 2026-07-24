//! gizza-ai/normalize-peak — sample-peak audio normalizer. Measures a file's
//! loudest sample (sample peak, dBFS) and applies one constant gain so that peak
//! lands exactly on a target dBFS. Dynamics are untouched — only the level
//! changes. Distinct from loudness (LUFS) normalization (audio-normalize) and
//! from a fixed-dB volume change (audio-volume-adjust): it *measures* the peak
//! and computes the exact gain.
//!
//! Pipeline: resolve the audio source (URL/ref) → pure `core::normalize_peak`
//! (symphonia decode → optional DC removal → sample-peak measure → linear gain →
//! hound WAV) → audio/wav media envelope.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a single audio file into a pure-wasm block
//! has no generic page runtime; same shape as loudness-matched-ab-prep /
//! normalize-image).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_normalize_peak_core as core;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 12 * 1024 * 1024; // compressed input, matches the audio family
const MAX_ENVELOPE_BYTES: usize = 20 * 1024 * 1024; // WAV ≤ 16 MiB + base64/header slack

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_target")]
    target: f64,
    #[serde(default)]
    remove_dc: bool,
    #[serde(default)]
    per_channel: bool,
    #[serde(default = "default_bit_depth")]
    bit_depth: String,
}
fn default_target() -> f64 {
    -1.0
}
fn default_bit_depth() -> String {
    "16".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::number("target")
                .default(-1.0)
                .min(-60.0)
                .max(0.0)
                .describe("Target sample-peak level in dBFS (range -60 to 0, default -1.0). The file's loudest sample is scaled to land exactly here. -1.0 leaves headroom for downstream encoders; 0 maximizes (peak at full scale); -3 or -6 is more conservative."),
        )
        .param(
            Param::boolean("remove_dc")
                .default(false)
                .describe("Subtract each channel's average (DC offset) before scaling so the waveform is centred on 0 (default false). Enable when a waveform looks vertically off-centre or a click is audible at the start/end; leave off for a pure, predictable peak scale."),
        )
        .param(
            Param::boolean("per_channel")
                .default(false)
                .describe("How stereo is scaled. false (default = linked): one common gain is applied to both channels so the L/R balance is preserved and only the loudest channel reaches the target. true: each channel is scaled independently so both hit the target, which can shift the stereo image."),
        )
        .param(
            Param::enumv("bit_depth", ["16", "24", "32f"])
                .default("16")
                .describe("Output WAV sample format: 16 (default, CD-quality int), 24 (mastering-grade int), or 32f (32-bit float). Output is always WAV; input may be wav/flac/mp3/ogg/m4a."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NormalizePeak;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/normalize-peak",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Normalize an audio file to a target sample peak (dBFS)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Sample-peak normalize an audio file: measure its loudest single sample (sample peak, in dBFS) and apply one constant gain so that peak lands exactly on a target dBFS. Dynamics are untouched — only the overall level changes. This is NOT loudness (LUFS) normalization and NOT a fixed-dB volume change: it measures the peak and computes the exact gain, then reports the measured peak, the applied gain and the resulting peak. target is the goal peak in dBFS (-60 to 0, default -1.0; 0 maximizes, -1 leaves encoder headroom). remove_dc (default false) subtracts each channel's DC offset before scaling. per_channel (default false = linked) scales both stereo channels by one common gain to preserve the L/R balance; true scales each channel independently. bit_depth picks the output WAV format: 16 (default), 24, or 32f (float). Input may be wav, flac, mp3, ogg (Vorbis) or m4a (mono or stereo); output is always WAV. Refuses digital silence (no peak to normalize). Provide the audio as either url (HTTP/HTTPS) or ref.",
        parameters = schema_json()
    ),
)]
impl NormalizePeak {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("normalize-peak")?;
    let depth = core::parse_bit_depth(&args.bit_depth).map_err(SkillError::InvalidArgs)?;
    if !(-60.0..=0.0).contains(&args.target) {
        return Err(SkillError::InvalidArgs(format!(
            "target must be between -60 and 0 dBFS, got {}",
            args.target
        )));
    }

    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    let n = core::normalize_peak(bytes, args.target, args.remove_dc, args.per_channel, depth)
        .map_err(SkillError::InvalidArgs)?;

    let fmt_gain = |g: f64| {
        if g.abs() < 1e-9 {
            "0.00 dB (unchanged)".to_string()
        } else {
            format!("{g:+.2} dB")
        }
    };
    let for_llm = format!(
        "Peak-normalized WAV ready ({} WAV, {:.2} s @ {} Hz, {}). \
         Measured sample peak {:.2} dBFS -> applied gain {} -> new sample peak {:.2} dBFS (target {:.2} dBFS). \
         Mode: {}{}.",
        depth.label(),
        n.duration_secs,
        n.rate,
        if n.channels == 1 { "mono" } else { "stereo" },
        n.measured_peak_dbfs,
        fmt_gain(n.applied_gain_db),
        n.new_peak_dbfs,
        n.target_dbfs,
        if n.per_channel { "per-channel" } else { "linked" },
        if n.removed_dc { ", DC offset removed" } else { "" },
    );

    build_media_envelope(
        &n.wav,
        "audio/wav",
        "normalized-peak.wav".to_string(),
        for_llm,
        MAX_ENVELOPE_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":         { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":         { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "target":      { "type": "number", "minimum": -60, "maximum": 0, "default": -1.0, "description": "Target sample-peak level in dBFS (range -60 to 0, default -1.0). The file's loudest sample is scaled to land exactly here. -1.0 leaves headroom for downstream encoders; 0 maximizes (peak at full scale); -3 or -6 is more conservative." },
                    "remove_dc":   { "type": "boolean", "default": false, "description": "Subtract each channel's average (DC offset) before scaling so the waveform is centred on 0 (default false). Enable when a waveform looks vertically off-centre or a click is audible at the start/end; leave off for a pure, predictable peak scale." },
                    "per_channel": { "type": "boolean", "default": false, "description": "How stereo is scaled. false (default = linked): one common gain is applied to both channels so the L/R balance is preserved and only the loudest channel reaches the target. true: each channel is scaled independently so both hit the target, which can shift the stereo image." },
                    "bit_depth":   { "type": "string", "enum": ["16", "24", "32f"], "default": "16", "description": "Output WAV sample format: 16 (default, CD-quality int), 24 (mastering-grade int), or 32f (32-bit float). Output is always WAV; input may be wav/flac/mp3/ogg/m4a." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
