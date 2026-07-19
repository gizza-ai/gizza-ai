//! gizza-ai/loudness-matched-ab-prep — measure the integrated loudness (EBU
//! R128) of two audio files and return gain-matched WAV copies of both, zipped,
//! so they can be A/B compared without the "louder sounds better" bias.
//!
//! Pipeline: resolve each audio source (URL/ref) → pure
//! `core::matched_ab_prep` (symphonia decode → ebur128 measure → linear gain →
//! hound WAV → stored zip) → application/zip media envelope. `Input::None` + a
//! required two-item `files` source_list (the image-collage multi-input
//! pattern).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker.
//! Surfaces: chat + CLI. No standalone page (the page driver takes a single
//! upload; this tool needs two).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_loudness_matched_ab_prep_core as core;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // per file, matches the audio family
const MAX_ENVELOPE_BYTES: usize = 11 * 1024 * 1024; // zip is stored WAV ≤ 10 MiB + headers

#[derive(Deserialize, Debug)]
struct Args {
    files: Vec<SourceFields>,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_target_lufs")]
    target_lufs: f64,
    #[serde(default = "default_bit_depth")]
    bit_depth: String,
}
fn default_mode() -> String {
    "quieter".to_string()
}
fn default_target_lufs() -> f64 {
    -16.0
}
fn default_bit_depth() -> String {
    "16".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("files", 2)
                .required()
                .describe("Exactly two audio sources to loudness-match — A first, then B. Each item has exactly one of `url` or `ref`. Formats: wav, flac, mp3, ogg (Vorbis) or m4a (AAC-LC/ALAC); mono or stereo; 3 s to ~45 s each (10 MiB max per file). Trim longer masters to a representative section first."),
        )
        .param(
            Param::enumv("mode", ["quieter", "target"])
                .default("quieter")
                .describe("quieter (default): turn the LOUDER file down to the quieter one's loudness — never boosts, so there is no clipping risk. target: gain BOTH files to target_lufs (may boost; linear gain only, no limiter — int output hard-clamps and the report counts clipped samples)."),
        )
        .param(
            Param::number("target_lufs")
                .default(-16.0)
                .min(-36.0)
                .max(-6.0)
                .describe("Loudness target in LUFS for mode=target (ignored for mode=quieter). Common values: -14 streaming (Spotify/YouTube), -16 podcasts (default), -23 EBU R128 broadcast. Range -36 to -6."),
        )
        .param(
            Param::enumv("bit_depth", ["16", "24", "32f"])
                .default("16")
                .describe("Output WAV sample format: 16 (default, smallest — fits ~30 s per file in the output cap), 24 (mastering-grade, ~20 s per file), or 32f (32-bit float — boosted peaks are preserved instead of clamped, ~15 s per file)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LoudnessMatchedAbPrep;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/loudness-matched-ab-prep",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Loudness-match two audio files for fair A/B comparison",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Measure the integrated loudness (EBU R128 / ITU-R BS.1770) of two audio files and return gain-matched WAV copies of both — zipped as a-matched.wav + b-matched.wav + report.txt — so they can be A/B compared without the louder-sounds-better bias. Provide files as a list of exactly two items (A then B), each a url or a `ref` (wav, flac, mp3, ogg-Vorbis or m4a; mono/stereo; 3 s to ~45 s and 10 MiB max each — trim longer masters to a representative section first). mode=quieter (default) turns the louder file down to the quieter one (never boosts); mode=target gains both to target_lufs (-14 streaming, -16 podcasts, -23 broadcast; linear gain only — no limiter; int output hard-clamps, and clipped samples are counted in the report). bit_depth picks the output WAV format: 16 (default), 24, or 32f (float — preserves boosted peaks). The report also lists each file's true peak (dBTP), loudness range (LRA) and RMS, plus the LU difference and the exact gain applied. Both files are re-encoded through the identical pipeline, so the comparison path is codec-fair. Output is capped at 10 MiB of WAV (~30 s per file at 16-bit 44.1 kHz stereo).",
        parameters = schema_json()
    ),
)]
impl LoudnessMatchedAbPrep {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("loudness-matched-ab-prep")?;
    if args.files.len() != 2 {
        return Err(SkillError::InvalidArgs(format!(
            "loudness-matched-ab-prep needs exactly two audio sources (A and B), got {}",
            args.files.len()
        )));
    }
    let mode = core::parse_mode(&args.mode).map_err(SkillError::InvalidArgs)?;
    let depth = core::parse_bit_depth(&args.bit_depth).map_err(SkillError::InvalidArgs)?;
    if !(-36.0..=-6.0).contains(&args.target_lufs) {
        return Err(SkillError::InvalidArgs(format!(
            "target_lufs must be between -36 and -6 LUFS, got {}",
            args.target_lufs
        )));
    }

    let mut sources = args.files.into_iter();
    let (a_bytes, _a_mime, a_name) = resolve_source(
        sources.next().expect("len checked").into_inner(),
        AssetKind::Audio,
        MAX_INPUT_BYTES,
    )?;
    let (b_bytes, _b_mime, b_name) = resolve_source(
        sources.next().expect("len checked").into_inner(),
        AssetKind::Audio,
        MAX_INPUT_BYTES,
    )?;

    let m = core::matched_ab_prep(
        a_bytes,
        &a_name,
        b_bytes,
        &b_name,
        mode,
        args.target_lufs,
        depth,
    )
    .map_err(SkillError::InvalidArgs)?;

    let delta = m.a.lufs - m.b.lufs;
    let fmt_gain = |g: f64| {
        if g == 0.0 {
            "0.00 dB (unchanged)".to_string()
        } else {
            format!("{g:+.2} dB")
        }
    };
    let mut for_llm = format!(
        "Loudness-matched A/B pair ready (a-matched.wav + b-matched.wav + report.txt, {} WAV, zipped). \
         A ({a_name}): {:.2} LUFS, true peak {:.2} dBTP, gain {}. \
         B ({b_name}): {:.2} LUFS, true peak {:.2} dBTP, gain {}. \
         Difference before: {:.2} LU ({}). Both files now at {:.2} LUFS — any remaining preference in an A/B listen is real, not loudness bias.",
        depth.label(),
        m.a.lufs,
        m.a.true_peak_dbtp,
        fmt_gain(m.gain_a_db),
        m.b.lufs,
        m.b.true_peak_dbtp,
        fmt_gain(m.gain_b_db),
        delta.abs(),
        if delta.abs() < 0.05 {
            "already matched"
        } else if delta > 0.0 {
            "A was louder"
        } else {
            "B was louder"
        },
        m.matched_lufs,
    );
    let clipped = m.clipped_a + m.clipped_b;
    if clipped > 0 {
        for_llm.push_str(&format!(
            " WARNING: {clipped} samples exceeded full scale{}.",
            if depth == core::BitDepth::Float32 {
                " (preserved by 32-bit float)"
            } else {
                " and were hard-clamped — retry with bit_depth=32f or a lower target_lufs"
            }
        ));
    }

    build_media_envelope(
        &m.zip,
        "application/zip",
        "loudness-matched-ab.zip".to_string(),
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
                    "files": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Exactly two audio sources to loudness-match — A first, then B. Each item has exactly one of `url` or `ref`. Formats: wav, flac, mp3, ogg (Vorbis) or m4a (AAC-LC/ALAC); mono or stereo; 3 s to ~45 s each (10 MiB max per file). Trim longer masters to a representative section first.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "mode": { "type": "string", "enum": ["quieter", "target"], "default": "quieter", "description": "quieter (default): turn the LOUDER file down to the quieter one's loudness — never boosts, so there is no clipping risk. target: gain BOTH files to target_lufs (may boost; linear gain only, no limiter — int output hard-clamps and the report counts clipped samples)." },
                    "target_lufs": { "type": "number", "minimum": -36, "maximum": -6, "default": -16.0, "description": "Loudness target in LUFS for mode=target (ignored for mode=quieter). Common values: -14 streaming (Spotify/YouTube), -16 podcasts (default), -23 EBU R128 broadcast. Range -36 to -6." },
                    "bit_depth": { "type": "string", "enum": ["16", "24", "32f"], "default": "16", "description": "Output WAV sample format: 16 (default, smallest — fits ~30 s per file in the output cap), 24 (mastering-grade, ~20 s per file), or 32f (32-bit float — boosted peaks are preserved instead of clamped, ~15 s per file)." }
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
