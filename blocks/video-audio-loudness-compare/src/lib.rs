//! gizza-ai/video-audio-loudness-compare — measure the ITU-R BS.1770 / EBU R128
//! loudness (integrated LUFS, true peak, LRA, momentary/short-term max) of two
//! videos or audio files and report the loudness GAP between them: which is
//! louder and by how many LU, the true-peak difference, and the exact gain that
//! would level one to the other (or to a target), with a headroom warning.
//!
//! Pipeline: resolve each source (URL/ref, `AssetKind::Any` — MP4/MKV video
//! containers and plain audio files both carry the audio we measure) → pure
//! `core::loudness_compare` (symphonia decode → streamed ebur128 measure → gap +
//! gain-to-match) → flat JSON `Resp`. `Input::None` + a required two-item `files`
//! source_list (the loudness-matched-ab-prep / video-audio-sync-offset-finder
//! multi-input pattern).
//!
//! Measure ONLY — it never rewrites the audio (use audio-normalize,
//! normalize-peak or loudness-matched-ab-prep to actually hit a target).
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
use gizza_ai_video_audio_loudness_compare_core as core;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wafer_sdk::*;

/// Per-file input cap — matches the video tool family (compress/trim first). PCM
/// is streamed into the analyzer, so only the compressed bytes are held.
const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    files: Vec<SourceFields>,
    #[serde(default = "default_match_target")]
    match_target: String,
    #[serde(default = "default_target_lufs")]
    target_lufs: f64,
    #[serde(default = "default_true_peak_ceiling")]
    true_peak_ceiling: f64,
}
fn default_match_target() -> String {
    "first".to_string()
}
fn default_target_lufs() -> f64 {
    -14.0
}
fn default_true_peak_ceiling() -> f64 {
    -1.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("files", 2)
                .required()
                .describe("Exactly two recordings to compare — video or audio in any mix (two exports, a video plus a reference track, before/after a master). File 1 is the reference timeline. Each item has exactly one of `url` or `ref`. Containers: MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS; audio codecs: AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM (Opus, AC-3 and DTS are not supported); mono or stereo, at least 1 s. Up to 32 MiB per file — compress or trim longer footage first."),
        )
        .param(
            Param::enumv("match_target", ["first", "second", "louder", "quieter", "target"])
                .default("first")
                .describe("Which loudness the gain-to-match suggestion levels both files to. first (default): match file 2 to file 1 (file 1 unchanged). second: match file 1 to file 2. louder: bring the quieter file UP to the louder one (may boost past the true-peak ceiling — a warning is added). quieter: bring the louder file DOWN to the quieter one (only attenuates — never clips). target: level BOTH files to target_lufs."),
        )
        .param(
            Param::number("target_lufs")
                .default(-14.0)
                .min(-40.0)
                .max(0.0)
                .describe("Loudness target in LUFS for match_target=target (ignored otherwise). Common values: -14 streaming (Spotify/YouTube/Tidal), -16 Apple Music, -23 EBU R128 broadcast, -24 ATSC A/85. Range -40 to 0."),
        )
        .param(
            Param::number("true_peak_ceiling")
                .default(-1.0)
                .min(-20.0)
                .max(0.0)
                .describe("True-peak ceiling in dBTP used only to warn when the gain-to-match would push a file's true peak above it (inter-sample clipping risk). -1 (default) matches EBU R128 / most streaming specs; -2 for ATSC A/85 / Amazon. Range -20 to 0."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// One decimal, or JSON `null` for a non-finite measurement (digital silence).
fn num_or_null(v: f64) -> Value {
    if v.is_finite() {
        serde_json::json!(core::round1(v))
    } else {
        Value::Null
    }
}

#[derive(Serialize)]
struct FileResp {
    label: &'static str,
    filename: String,
    integrated_lufs: Value,
    true_peak_dbtp: Value,
    sample_peak_dbfs: Value,
    lra_lu: Value,
    momentary_max_lufs: Value,
    short_term_max_lufs: Value,
    plr_lu: Value,
    duration_seconds: f64,
    sample_rate: u32,
    channels: u16,
    /// Offset vs common delivery targets: measured − target, LU. Positive = the
    /// file is louder than that target (a normalizer would turn it down).
    vs_targets: serde_json::Map<String, Value>,
}

fn file_resp(label: &'static str, filename: String, m: &core::Measured) -> FileResp {
    let mut vs_targets = serde_json::Map::new();
    for (name, target) in core::PRESET_TARGETS {
        vs_targets.insert((*name).to_string(), num_or_null(m.integrated_lufs - target));
    }
    FileResp {
        label,
        filename,
        integrated_lufs: num_or_null(m.integrated_lufs),
        true_peak_dbtp: num_or_null(m.true_peak_dbtp),
        sample_peak_dbfs: num_or_null(m.sample_peak_dbfs),
        lra_lu: num_or_null(m.lra),
        momentary_max_lufs: num_or_null(m.momentary_max_lufs),
        short_term_max_lufs: num_or_null(m.short_term_max_lufs),
        plr_lu: num_or_null(m.plr_lu()),
        duration_seconds: core::round1(m.duration_seconds),
        sample_rate: m.sample_rate,
        channels: m.channels,
        vs_targets,
    }
}

#[derive(Serialize)]
struct AdjustmentResp {
    file: &'static str,
    gain_db: f64,
    gain_linear: f64,
    true_peak_after_dbtp: Value,
    clips: bool,
}

#[derive(Serialize)]
struct ComparisonResp {
    louder: &'static str,
    loudness_gap_lu: f64,
    true_peak_gap_db: Value,
    lra_gap_lu: f64,
    match_target: String,
    reference_lufs: f64,
    adjustments: Vec<AdjustmentResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
}

#[derive(Serialize)]
struct Resp {
    files: Vec<FileResp>,
    comparison: ComparisonResp,
    summary: String,
}

/// Build the flat JSON response from a core report (shared by wasm + tests).
fn report_json(
    report: core::CompareReport,
    match_target: &str,
    a_name: String,
    b_name: String,
) -> Result<Vec<u8>, SkillError> {
    let c = &report.comparison;
    let resp = Resp {
        files: vec![
            file_resp("file 1", a_name, &report.a),
            file_resp("file 2", b_name, &report.b),
        ],
        comparison: ComparisonResp {
            louder: c.louder,
            loudness_gap_lu: core::round1(c.loudness_gap_lu),
            true_peak_gap_db: num_or_null(c.true_peak_gap_db),
            lra_gap_lu: core::round1(c.lra_gap_lu),
            match_target: match_target.to_string(),
            reference_lufs: core::round1(c.reference_lufs),
            adjustments: c
                .adjustments
                .iter()
                .map(|a| AdjustmentResp {
                    file: a.label,
                    gain_db: core::round2(a.gain_db),
                    gain_linear: core::round2(a.gain_linear),
                    true_peak_after_dbtp: num_or_null(a.true_peak_after_dbtp),
                    clips: a.clips,
                })
                .collect(),
            warning: c.warning.clone(),
        },
        summary: c.summary.clone(),
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!(
            "serialize video-audio-loudness-compare response: {e}"
        ))
    })
}

#[cfg(target_arch = "wasm32")]
struct VideoAudioLoudnessCompare;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-audio-loudness-compare",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare the loudness (LUFS, true peak) of two videos or audio files",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Measure the ITU-R BS.1770 / EBU R128 loudness of two recordings — video or audio in any mix (two exports, a video plus a reference track, before/after a master) — and report the loudness GAP between them. Provide files as a list of exactly two items (the reference first), each a url or a `ref` from a prior tool call; MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3 and AAC-ADTS containers are read with the pure-Rust decoder (audio codecs AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM — not Opus/AC-3/DTS), mono or stereo, at least 1 s, up to 32 MiB per file. For EACH file it returns integrated loudness (LUFS), 4×-oversampled true peak (dBTP), sample peak (dBFS), loudness range (LRA), the max momentary and short-term loudness, the peak-to-loudness ratio (PLR), and the offset versus common delivery targets (streaming -14, Apple Music -16, EBU R128 -23, ATSC A/85 -24). The comparison reports which file is louder and by how many LU, the true-peak and LRA differences, and the exact linear/dB gain that would level the files: match_target picks the reference — first (default, level file 2 to file 1), second, louder (bring the quieter file up — may boost), quieter (bring the louder file down — never clips), or target (level both to target_lufs). true_peak_ceiling (default -1 dBTP) triggers a headroom warning when a gain-to-match would push a file's true peak above it. This tool MEASURES only — it never rewrites the audio (use audio-normalize, normalize-peak or loudness-matched-ab-prep to actually hit a target). Returns flat JSON: a files[] array of per-file measurements, a comparison object (louder, loudness_gap_lu, true_peak_gap_db, reference_lufs, adjustments[], optional warning) and a one-line summary.",
        parameters = schema_json()
    ),
)]
impl VideoAudioLoudnessCompare {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("video-audio-loudness-compare")?;
    if args.files.len() != 2 {
        return Err(SkillError::InvalidArgs(format!(
            "video-audio-loudness-compare needs exactly two files (the reference first), got {}",
            args.files.len()
        )));
    }
    let target = core::parse_match_target(&args.match_target).map_err(SkillError::InvalidArgs)?;
    if !args.target_lufs.is_finite() || !(-40.0..=0.0).contains(&args.target_lufs) {
        return Err(SkillError::InvalidArgs(format!(
            "target_lufs must be between -40 and 0 LUFS, got {}",
            args.target_lufs
        )));
    }
    if !args.true_peak_ceiling.is_finite() || !(-20.0..=0.0).contains(&args.true_peak_ceiling) {
        return Err(SkillError::InvalidArgs(format!(
            "true_peak_ceiling must be between -20 and 0 dBTP, got {}",
            args.true_peak_ceiling
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

    let report = core::loudness_compare(
        a_bytes,
        &a_disp,
        b_bytes,
        &b_disp,
        target,
        args.target_lufs,
        args.true_peak_ceiling,
    )
    .map_err(SkillError::InvalidArgs)?;

    report_json(report, &args.match_target, a_disp, b_disp)
}

#[cfg(target_arch = "wasm32")]
fn display_name(name: &str, fallback: &str) -> String {
    if name.trim().is_empty() {
        fallback.to_string()
    } else {
        name.to_string()
    }
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
                        "description": "Exactly two recordings to compare — video or audio in any mix (two exports, a video plus a reference track, before/after a master). File 1 is the reference timeline. Each item has exactly one of `url` or `ref`. Containers: MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC-ADTS; audio codecs: AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM (Opus, AC-3 and DTS are not supported); mono or stereo, at least 1 s. Up to 32 MiB per file — compress or trim longer footage first.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "match_target": { "type": "string", "enum": ["first", "second", "louder", "quieter", "target"], "default": "first", "description": "Which loudness the gain-to-match suggestion levels both files to. first (default): match file 2 to file 1 (file 1 unchanged). second: match file 1 to file 2. louder: bring the quieter file UP to the louder one (may boost past the true-peak ceiling — a warning is added). quieter: bring the louder file DOWN to the quieter one (only attenuates — never clips). target: level BOTH files to target_lufs." },
                    "target_lufs": { "type": "number", "minimum": -40, "maximum": 0, "default": -14.0, "description": "Loudness target in LUFS for match_target=target (ignored otherwise). Common values: -14 streaming (Spotify/YouTube/Tidal), -16 Apple Music, -23 EBU R128 broadcast, -24 ATSC A/85. Range -40 to 0." },
                    "true_peak_ceiling": { "type": "number", "minimum": -20, "maximum": 0, "default": -1.0, "description": "True-peak ceiling in dBTP used only to warn when the gain-to-match would push a file's true peak above it (inter-sample clipping risk). -1 (default) matches EBU R128 / most streaming specs; -2 for ATSC A/85 / Amazon. Range -20 to 0." }
                },
                "required": ["files"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn report_json_shape_is_flat_and_complete() {
        // A synthetic report → JSON, asserting the advertised keys exist.
        let a = core::Measured {
            integrated_lufs: -10.2,
            true_peak_dbtp: -0.5,
            sample_peak_dbfs: -1.0,
            lra: 4.0,
            momentary_max_lufs: -8.0,
            short_term_max_lufs: -9.0,
            duration_seconds: 6.0,
            sample_rate: 44100,
            channels: 2,
        };
        let b = core::Measured {
            integrated_lufs: -20.4,
            true_peak_dbtp: -6.0,
            sample_peak_dbfs: -6.5,
            lra: 3.0,
            momentary_max_lufs: -18.0,
            short_term_max_lufs: -19.0,
            duration_seconds: 6.0,
            sample_rate: 44100,
            channels: 2,
        };
        let comparison = core::compare(&a, &b, core::MatchTarget::First, -14.0, -1.0);
        let report = core::CompareReport { a, b, comparison };
        let bytes = report_json(report, "first", "loud.wav".into(), "quiet.wav".into()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(v["files"].as_array().unwrap().len(), 2);
        assert_eq!(v["files"][0]["label"], "file 1");
        assert_eq!(v["files"][0]["filename"], "loud.wav");
        assert_eq!(v["files"][0]["integrated_lufs"], -10.2);
        assert_eq!(v["files"][0]["true_peak_dbtp"], -0.5);
        assert_eq!(v["files"][0]["vs_targets"]["streaming"], 3.8); // -10.2 − (-14)
        assert_eq!(v["comparison"]["louder"], "file 1");
        assert_eq!(v["comparison"]["loudness_gap_lu"], 10.2);
        assert_eq!(v["comparison"]["match_target"], "first");
        assert_eq!(v["comparison"]["reference_lufs"], -10.2);
        assert_eq!(v["comparison"]["adjustments"].as_array().unwrap().len(), 2);
        assert_eq!(v["comparison"]["adjustments"][0]["gain_db"], 0.0);
        assert!(
            v["comparison"]["adjustments"][1]["gain_db"]
                .as_f64()
                .unwrap()
                > 9.0
        );
        assert!(v["summary"].as_str().unwrap().starts_with("File 1 is"));
    }
}
