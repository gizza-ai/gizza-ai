//! gizza-ai/video-audio-sync-offset-finder — cross-correlate the audio of two
//! recordings (video or audio, any mix) and report the time offset that best
//! aligns them, with a BBC-audio-offset-finder-style standard score, an NCC
//! correlation value, and a concrete how-to-align hint.
//!
//! Pipeline: resolve each source (URL/ref, `AssetKind::Any` — MP4/MKV video
//! containers and plain audio files both carry the audio we correlate) → pure
//! `core::sync_offset` (symphonia decode → mono 8 kHz → envelope NCC coarse
//! pass → waveform NCC fine pass) → flat JSON `Resp`. `Input::None` + a
//! required two-item `files` source_list (the loudness-matched-ab-prep /
//! image-collage multi-input pattern).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker.
//! Surfaces: chat + CLI. No standalone page (the page driver takes a single
//! upload; this tool needs two).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_video_audio_sync_offset_finder_core as core;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Per-file input cap — matches the video tool family (compress/trim first).
const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    files: Vec<SourceFields>,
    #[serde(default = "default_analyze_seconds")]
    analyze_seconds: f64,
    #[serde(default)]
    max_offset: f64,
}

fn default_analyze_seconds() -> f64 {
    core::DEFAULT_ANALYZE_SECONDS
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("files", 2)
                .required()
                .describe("Exactly two recordings of the same event — video or audio in any mix (two camera angles, or a camera file plus an external audio recorder). The FIRST file is the reference timeline. Each item has exactly one of `url` or `ref`. Containers: MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS; audio codecs: AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM (Opus, AC-3 and DTS are not supported). Up to 10 MiB per file — compress or trim longer footage first."),
        )
        .param(
            Param::number("analyze_seconds")
                .default(120.0)
                .min(5.0)
                .max(240.0)
                .describe("How many seconds from the start of each file to analyze (default 120, range 5–240). The true offset must be smaller than the analyzed span; a longer window correlates more reliably but runs longer."),
        )
        .param(
            Param::number("max_offset")
                .default(0.0)
                .min(0.0)
                .max(240.0)
                .describe("Largest absolute offset in seconds to consider; 0 (default) searches the whole analyzed range. Set it when the recordings are known to have started within a few seconds of each other — it suppresses spurious far-away correlation peaks."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[derive(Serialize)]
struct FileResp {
    filename: String,
    sample_rate: u32,
    channels: u16,
    analyzed_seconds: f64,
    truncated: bool,
}

#[derive(Serialize)]
struct Resp {
    offset_seconds: f64,
    offset: String,
    confidence: &'static str,
    score: f64,
    correlation: f64,
    method: &'static str,
    precision_seconds: f64,
    polarity_inverted: bool,
    overlap_seconds: f64,
    align_hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    files: Vec<FileResp>,
}

#[cfg(target_arch = "wasm32")]
struct VideoAudioSyncOffsetFinder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-sync-offset-finder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find the audio time offset that aligns two recordings",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Cross-correlate the audio of two recordings of the same event — video or audio in any mix (two camera angles, or a camera plus a separate audio recorder) — and report the time offset that best aligns them. Provide files as a list of exactly two items (the reference first), each a url or a `ref` from a prior tool call; MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3 and AAC-ADTS containers are read with the pure-Rust decoder (audio codecs AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM — not Opus/AC-3/DTS), up to 10 MiB per file. A coarse pass cross-correlates 100 Hz loudness-change envelopes over every candidate lag, then a fine pass cross-correlates the 8 kHz waveforms around the top candidates: when the waveform locks the offset is reported to about a millisecond, otherwise the envelope result stands at ±10 ms (typical for two different microphones). offset_seconds > 0 means file 2 starts that many seconds AFTER file 1. The result includes a confidence label backed by a standard score (peak z-score across all candidate lags: >= 10 high, 5–10 medium; below 5 the envelope evidence alone is unreliable, so the label is low when the waveform pass still locks and otherwise none — meaning the recordings likely do not share audio), the normalized correlation at the peak, the aligned overlap length, a polarity-inversion flag, and a concrete align_hint (what to trim or delay, with ffmpeg -ss / -itsoffset values). analyze_seconds (default 120, 5–240) sets how much of each file is examined — the true offset must be smaller than that span; max_offset (default 0 = unlimited) restricts the search when the start difference is known to be small.",
        parameters = schema_json()
    ),
)]
impl VideoAudioSyncOffsetFinder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args =
        serde_json::from_slice(&body).invalid_args("video-audio-sync-offset-finder")?;
    if args.files.len() != 2 {
        return Err(SkillError::InvalidArgs(format!(
            "video-audio-sync-offset-finder needs exactly two files (the reference first), got {}",
            args.files.len()
        )));
    }
    if !(core::MIN_ANALYZE_SECONDS..=core::MAX_ANALYZE_SECONDS).contains(&args.analyze_seconds) {
        return Err(SkillError::InvalidArgs(format!(
            "analyze_seconds must be between {:.0} and {:.0}, got {}",
            core::MIN_ANALYZE_SECONDS,
            core::MAX_ANALYZE_SECONDS,
            args.analyze_seconds
        )));
    }
    if !(0.0..=core::MAX_ANALYZE_SECONDS).contains(&args.max_offset) {
        return Err(SkillError::InvalidArgs(format!(
            "max_offset must be between 0 (no limit) and {:.0} seconds, got {}",
            core::MAX_ANALYZE_SECONDS,
            args.max_offset
        )));
    }

    let mut sources = args.files.into_iter();
    let (a_bytes, _a_mime, a_name) = resolve_source(
        sources.next().expect("len checked").into_inner(),
        AssetKind::Any,
        MAX_INPUT_BYTES,
    )?;
    let (b_bytes, _b_mime, b_name) = resolve_source(
        sources.next().expect("len checked").into_inner(),
        AssetKind::Any,
        MAX_INPUT_BYTES,
    )?;
    let a_disp = display_name(&a_name, "file 1");
    let b_disp = display_name(&b_name, "file 2");

    let report = core::sync_offset(
        a_bytes,
        &format!("file 1 ({a_disp})"),
        b_bytes,
        &format!("file 2 ({b_disp})"),
        args.analyze_seconds,
        args.max_offset,
    )
    .map_err(SkillError::InvalidArgs)?;

    let al = &report.alignment;
    let confidence = core::confidence_label(al.score, al.method == "waveform");
    let off = round4(al.offset_seconds);
    let abs = format!("{:.3}", off.abs());

    let offset = if off.abs() < al.precision_seconds {
        format!("0.000 s — {a_disp} and {b_disp} already start in sync")
    } else if off > 0.0 {
        format!("+{abs} s — file 2 ({b_disp}) starts {abs} s after file 1 ({a_disp})")
    } else {
        format!("-{abs} s — file 2 ({b_disp}) starts {abs} s before file 1 ({a_disp})")
    };
    let align_hint = if off.abs() < al.precision_seconds {
        "No shift needed — the recordings already start together.".to_string()
    } else {
        // The file that started EARLIER has extra leading content: trim it, or
        // delay the other one by the same amount when muxing.
        let (early, late) = if off > 0.0 {
            (a_disp.as_str(), b_disp.as_str())
        } else {
            (b_disp.as_str(), a_disp.as_str())
        };
        format!(
            "To align, trim the first {abs} s from {early} (ffmpeg -ss {abs} -i {early} ...), \
             or delay {late} by {abs} s when muxing (ffmpeg -itsoffset {abs} -i {late} ...)."
        )
    };

    let mut warnings: Vec<String> = Vec::new();
    if confidence == "none" {
        warnings.push(
            "The recordings do not appear to share audio — treat the reported offset as \
             meaningless."
                .to_string(),
        );
    } else if confidence == "low" {
        warnings.push("Low confidence — verify the alignment manually.".to_string());
    } else if confidence == "medium" {
        warnings.push(
            "Moderate confidence — spot-check the alignment before committing to it."
                .to_string(),
        );
    }
    if al.at_search_edge {
        warnings.push(
            "The best match sits at the edge of the searched range; the true offset may lie \
             beyond it — raise max_offset or analyze_seconds."
                .to_string(),
        );
    }
    if confidence != "none" && al.method == "envelope" {
        warnings.push(
            "The waveform fine pass did not lock (common when the recordings come from \
             different microphones) — precision is ±10 ms."
                .to_string(),
        );
    }
    if al.polarity_inverted {
        warnings.push(
            "The recordings have opposite polarity (one signal chain inverts the waveform); \
             the offset is still valid."
                .to_string(),
        );
    }

    let resp = Resp {
        offset_seconds: off,
        offset,
        confidence,
        score: round1(al.score),
        correlation: round3(al.correlation),
        method: al.method,
        precision_seconds: al.precision_seconds,
        polarity_inverted: al.polarity_inverted,
        overlap_seconds: round2(al.overlap_seconds),
        align_hint,
        warning: (!warnings.is_empty()).then(|| warnings.join(" ")),
        files: vec![
            FileResp {
                filename: a_disp,
                sample_rate: report.a.sample_rate,
                channels: report.a.channels,
                analyzed_seconds: report.a.analyzed_seconds,
                truncated: report.a.truncated,
            },
            FileResp {
                filename: b_disp,
                sample_rate: report.b.sample_rate,
                channels: report.b.channels,
                analyzed_seconds: report.b.analyzed_seconds,
                truncated: report.b.truncated,
            },
        ],
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize sync response: {e}")))
}

#[cfg(target_arch = "wasm32")]
fn display_name(name: &str, fallback: &str) -> String {
    if name.trim().is_empty() {
        fallback.to_string()
    } else {
        name.to_string()
    }
}

#[cfg(target_arch = "wasm32")]
fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}
#[cfg(target_arch = "wasm32")]
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
#[cfg(target_arch = "wasm32")]
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
#[cfg(target_arch = "wasm32")]
fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
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
                    "files": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Exactly two recordings of the same event — video or audio in any mix (two camera angles, or a camera file plus an external audio recorder). The FIRST file is the reference timeline. Each item has exactly one of `url` or `ref`. Containers: MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS; audio codecs: AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM (Opus, AC-3 and DTS are not supported). Up to 10 MiB per file — compress or trim longer footage first.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "analyze_seconds": {
                        "type": "number",
                        "minimum": 5,
                        "maximum": 240,
                        "default": 120.0,
                        "description": "How many seconds from the start of each file to analyze (default 120, range 5–240). The true offset must be smaller than the analyzed span; a longer window correlates more reliably but runs longer."
                    },
                    "max_offset": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 240,
                        "default": 0.0,
                        "description": "Largest absolute offset in seconds to consider; 0 (default) searches the whole analyzed range. Set it when the recordings are known to have started within a few seconds of each other — it suppresses spurious far-away correlation peaks."
                    }
                },
                "required": ["files"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
