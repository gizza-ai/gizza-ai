//! gizza-ai/loudness-spec-compliance — measure an audio file's ITU-R BS.1770 /
//! EBU R128 integrated loudness (LUFS), true peak (dBTP) and loudness range
//! (LRA), then check them against a named delivery spec (EBU R128, ATSC A/85, or
//! a streaming target) and return a per-criterion PASS/FAIL verdict.
//!
//! Pipeline: resolve the audio source (URL/ref) → pure `core::check_compliance`
//! (symphonia decode → streamed ebur128 measure → spec verdict) → flat JSON the
//! LLM reads directly.
//!
//! Measure + verdict ONLY — it never rewrites the audio. To actually hit a
//! target, use audio-normalize (LUFS), normalize-peak (sample peak) or
//! loudness-matched-ab-prep.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a single audio file → text report fits neither
//! the pure-text page nor the ffmpeg file→media page shape — the "no-page
//! file-input" pattern, like media-info / normalize-peak).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{resolve_source, AssetKind};
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_loudness_spec_compliance_core as core;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 48 * 1024 * 1024; // compressed input; PCM is streamed, never fully buffered

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_standard")]
    standard: String,
    #[serde(default = "default_target_lufs")]
    target_lufs: f64,
    #[serde(default = "default_tolerance_lu")]
    tolerance_lu: f64,
    #[serde(default = "default_max_true_peak")]
    max_true_peak: f64,
    #[serde(default = "default_max_lra")]
    max_lra: f64,
}
fn default_standard() -> String {
    "ebu-r128".to_string()
}
fn default_target_lufs() -> f64 {
    -23.0
}
fn default_tolerance_lu() -> f64 {
    1.0
}
fn default_max_true_peak() -> f64 {
    -1.0
}
fn default_max_lra() -> f64 {
    0.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Audio)
        .param(
            Param::enumv(
                "standard",
                [
                    "ebu-r128",
                    "atsc-a85",
                    "spotify",
                    "youtube",
                    "apple-music",
                    "amazon-music",
                    "tidal",
                    "custom",
                ],
            )
            .default("ebu-r128")
            .describe("Delivery spec to check against. ebu-r128 (default): -23 LUFS ±1, true peak ≤ -1 dBTP (EU broadcast). atsc-a85: -24 LKFS ±2, ≤ -2 dBTP (US CALM Act). spotify/youtube/tidal: -14 LUFS ±1, ≤ -1 dBTP. amazon-music: -14 LUFS ±1, ≤ -2 dBTP. apple-music: -16 LUFS ±1, ≤ -1 dBTP. custom: use the target_lufs / tolerance_lu / max_true_peak / max_lra values below."),
        )
        .param(
            Param::number("target_lufs")
                .default(-23.0)
                .min(-40.0)
                .max(0.0)
                .describe("Target integrated loudness in LUFS for standard=custom (ignored for every preset). Range -40 to 0."),
        )
        .param(
            Param::number("tolerance_lu")
                .default(1.0)
                .min(0.0)
                .max(12.0)
                .describe("Permitted ± deviation from target_lufs in LU, for standard=custom (ignored for presets). E.g. 1.0 = the file passes if it is within ±1 LU of the target. Range 0 to 12."),
        )
        .param(
            Param::number("max_true_peak")
                .default(-1.0)
                .min(-20.0)
                .max(0.0)
                .describe("Maximum allowed true peak in dBTP for standard=custom (ignored for presets). The measured 4×-oversampled true peak must be at or below this. Range -20 to 0."),
        )
        .param(
            Param::number("max_lra")
                .default(0.0)
                .min(0.0)
                .max(60.0)
                .describe("Optional loudness-range (LRA) ceiling in LU for standard=custom (ignored for presets). 0 (default) = do not check LRA; any value > 0 adds a pass/fail loudness_range criterion. Range 0 to 60."),
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
struct CheckResp {
    criterion: &'static str,
    measured: Value,
    limit: String,
    pass: bool,
    detail: String,
}

#[derive(Serialize)]
struct MeasuredResp {
    integrated_lufs: Value,
    true_peak_dbtp: Value,
    lra_lu: Value,
    duration_seconds: f64,
    sample_rate: u32,
    channels: u16,
}

#[derive(Serialize)]
struct Resp {
    standard: String,
    target_lufs: f64,
    tolerance_lu: f64,
    max_true_peak_dbtp: f64,
    max_lra_lu: Option<f64>,
    measured: MeasuredResp,
    checks: Vec<CheckResp>,
    pass: bool,
    summary: String,
}

/// Build the flat JSON response from a core report (shared by wasm + tests).
fn report_json(r: core::Report) -> Result<Vec<u8>, SkillError> {
    let resp = Resp {
        standard: r.spec.name.to_string(),
        target_lufs: r.spec.target_lufs,
        tolerance_lu: r.spec.tolerance_lu,
        max_true_peak_dbtp: r.spec.max_true_peak,
        max_lra_lu: r.spec.max_lra,
        measured: MeasuredResp {
            integrated_lufs: num_or_null(r.measured.integrated_lufs),
            true_peak_dbtp: num_or_null(r.measured.true_peak_dbtp),
            lra_lu: num_or_null(r.measured.lra),
            duration_seconds: core::round1(r.measured.duration_seconds),
            sample_rate: r.measured.sample_rate,
            channels: r.measured.channels,
        },
        checks: r
            .checks
            .into_iter()
            .map(|c| CheckResp {
                criterion: c.criterion,
                measured: num_or_null(c.measured),
                limit: c.limit,
                pass: c.pass,
                detail: c.detail,
            })
            .collect(),
        pass: r.pass,
        summary: r.summary,
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize loudness-spec-compliance response: {e}"))
    })
}

#[cfg(target_arch = "wasm32")]
struct LoudnessSpecCompliance;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/loudness-spec-compliance",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check an audio file's loudness (LUFS), true peak and LRA against a delivery spec",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Measure an audio file's ITU-R BS.1770 / EBU R128 integrated loudness (LUFS), 4×-oversampled true peak (dBTP) and loudness range (LRA), then check them against a named delivery spec and return a per-criterion PASS/FAIL verdict plus an overall pass. standard picks the spec: ebu-r128 (-23 LUFS ±1, ≤ -1 dBTP; EU broadcast), atsc-a85 (-24 LKFS ±2, ≤ -2 dBTP; US CALM Act), spotify / youtube / tidal (-14 LUFS ±1, ≤ -1 dBTP), amazon-music (-14 LUFS ±1, ≤ -2 dBTP), apple-music (-16 LUFS ±1, ≤ -1 dBTP), or custom (supply target_lufs, tolerance_lu, max_true_peak, and optionally max_lra > 0 to also cap loudness range). Integrated loudness passes when it is within target ± tolerance; true peak passes when at or below the ceiling; LRA is always reported and only pass/fail when a spec caps it. This tool MEASURES and reports only — it never rewrites the audio (use audio-normalize, normalize-peak or loudness-matched-ab-prep to actually hit a target). Input may be wav, flac, mp3, ogg (Vorbis) or m4a (AAC-LC/ALAC), mono or stereo, at least 1 s. Provide the audio as either url (HTTP/HTTPS) or ref (id from a prior tool call). Returns JSON: the resolved spec, the measured values, a checks[] array (criterion, measured, limit, pass, detail) and the overall pass boolean.",
        parameters = schema_json()
    ),
)]
impl LoudnessSpecCompliance {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("loudness-spec-compliance")?;
    let spec = core::resolve_spec(
        &args.standard,
        args.target_lufs,
        args.tolerance_lu,
        args.max_true_peak,
        args.max_lra,
    )
    .map_err(SkillError::InvalidArgs)?;

    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Audio, MAX_INPUT_BYTES)?;

    let report = core::check_compliance(bytes, spec).map_err(SkillError::InvalidArgs)?;
    report_json(report)
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
                    "url":           { "type": "string", "description": "Audio URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":           { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "standard":      { "type": "string", "enum": ["ebu-r128", "atsc-a85", "spotify", "youtube", "apple-music", "amazon-music", "tidal", "custom"], "default": "ebu-r128", "description": "Delivery spec to check against. ebu-r128 (default): -23 LUFS ±1, true peak ≤ -1 dBTP (EU broadcast). atsc-a85: -24 LKFS ±2, ≤ -2 dBTP (US CALM Act). spotify/youtube/tidal: -14 LUFS ±1, ≤ -1 dBTP. amazon-music: -14 LUFS ±1, ≤ -2 dBTP. apple-music: -16 LUFS ±1, ≤ -1 dBTP. custom: use the target_lufs / tolerance_lu / max_true_peak / max_lra values below." },
                    "target_lufs":   { "type": "number", "minimum": -40, "maximum": 0, "default": -23.0, "description": "Target integrated loudness in LUFS for standard=custom (ignored for every preset). Range -40 to 0." },
                    "tolerance_lu":  { "type": "number", "minimum": 0, "maximum": 12, "default": 1.0, "description": "Permitted ± deviation from target_lufs in LU, for standard=custom (ignored for presets). E.g. 1.0 = the file passes if it is within ±1 LU of the target. Range 0 to 12." },
                    "max_true_peak": { "type": "number", "minimum": -20, "maximum": 0, "default": -1.0, "description": "Maximum allowed true peak in dBTP for standard=custom (ignored for presets). The measured 4×-oversampled true peak must be at or below this. Range -20 to 0." },
                    "max_lra":       { "type": "number", "minimum": 0, "maximum": 60, "default": 0.0, "description": "Optional loudness-range (LRA) ceiling in LU for standard=custom (ignored for presets). 0 (default) = do not check LRA; any value > 0 adds a pass/fail loudness_range criterion. Range 0 to 60." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn report_json_shape_is_flat_and_complete() {
        // A synthetic report → JSON, asserting the advertised keys exist.
        let measured = core::Measured {
            integrated_lufs: -22.6,
            true_peak_dbtp: -1.4,
            lra: 3.2,
            duration_seconds: 6.0,
            sample_rate: 44100,
            channels: 2,
        };
        let spec = core::resolve_spec("ebu-r128", 0.0, 0.0, 0.0, 0.0).unwrap();
        let report = core::verdict(measured, spec);
        let bytes = report_json(report).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["standard"], "EBU R128");
        assert_eq!(v["target_lufs"], -23.0);
        assert_eq!(v["pass"], true);
        assert_eq!(v["measured"]["integrated_lufs"], -22.6);
        assert_eq!(v["measured"]["channels"], 2);
        assert_eq!(v["checks"].as_array().unwrap().len(), 2);
        assert_eq!(v["checks"][0]["criterion"], "integrated_loudness");
        assert!(v["max_lra_lu"].is_null());
        assert!(v["summary"].as_str().unwrap().starts_with("PASS"));
    }
}
