//! gizza-ai/spectral-eq-match — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    track: String,
    #[serde(default)]
    reference: String,
    #[serde(default = "default_target_curve")]
    target_curve: String,
    #[serde(default = "default_amount")]
    amount: f64,
    #[serde(default = "default_max_gain")]
    max_gain_db: f64,
    #[serde(default = "default_smoothing")]
    smoothing: u32,
    #[serde(default = "default_q")]
    q: f64,
    #[serde(default = "default_tone_only")]
    tone_only: bool,
    #[serde(default)]
    track_lufs: f64,
    #[serde(default)]
    target_lufs: f64,
    #[serde(default)]
    output: String,
}

fn default_amount() -> f64 {
    100.0
}
fn default_max_gain() -> f64 {
    6.0
}
fn default_smoothing() -> u32 {
    1
}
fn default_target_curve() -> String {
    "pink".to_string()
}
fn default_q() -> f64 {
    1.0
}
fn default_tone_only() -> bool {
    true
}

/// Single source for the chat schema (and CLI, and the page query params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("track").required().describe("Your track's measured per-band levels as frequency/level pairs, e.g. '63 -12, 250 -8, 1k -6, 4k -10, 12k -18' or one pair per line. Frequencies are Hz (a 'k' suffix means kHz), levels are dB on any consistent scale. Pairs may be separated by newlines, commas, semicolons, colons, equals signs or spaces. 2-64 bands."))
        .param(Param::string("reference").describe("The reference track's per-band levels in the same format. Used only when target_curve is 'reference'. The reference is interpolated over log frequency onto your track's band frequencies, so the two lists do not need the same band count."))
        .param(Param::enumv("target_curve", ["reference", "flat", "pink", "bright", "warm", "speech"]).default("pink").describe("What to match against. 'reference' uses the pasted reference levels. The built-in curves are relative to 0 dB at 1 kHz: 'flat' = 0 dB everywhere, 'pink' = -3 dB/octave (equal energy per octave, and the default so a track-only run succeeds), 'bright' = -2 dB/octave, 'warm' = -4 dB/octave, 'speech' = pink tilt with a rolloff below 150 Hz, a +3 dB presence lift at 3 kHz and an air cut above 10 kHz."))
        .param(Param::number("amount").default(100).min(0.0).max(100.0).describe("Percentage of the derived correction to apply, 0-100. 100 matches the target fully; 50 applies half the move, which is usually the safer starting point on music."))
        .param(Param::number("max_gain_db").default(6).min(0.0).max(24.0).describe("Symmetric boost/cut limit in dB, 0-24. Every corrective gain is clamped to +/- this value so a badly measured or out-of-range band cannot ask for an extreme filter. Default 6 dB."))
        .param(Param::integer("smoothing").default(1).min(0.0).max(4.0).describe("Neighbouring-band averaging radius, 0-4. 0 follows the measurement exactly (narrow resonances included); 1-2 favours broad tonal balance; 4 gives a very gentle tilt-style correction."))
        .param(Param::number("q").default(1).min(0.1).max(10.0).describe("Q (bandwidth) of every peaking 'equalizer' stage in the generated ffmpeg chain, 0.1-10. Roughly 1.4 for third-octave bands, 1.0 for octave bands; lower Q is broader. Ignored by the firequalizer output, which interpolates a continuous curve."))
        .param(Param::boolean("tone_only").default(true).describe("When true (default), the average level difference between the two curves is removed before matching, so the EQ corrects TONE only and level is handled by the loudness offset. Set false to bake the broadband level difference into the band gains."))
        .param(Param::number("track_lufs").default(0).min(-70.0).max(0.0).describe("Your track's integrated loudness in LUFS, e.g. -9.5. Leave at 0 to skip the loudness offset. Used with target_lufs to compute the dB offset and the trailing 'volume' stage."))
        .param(Param::number("target_lufs").default(0).min(-70.0).max(0.0).describe("The loudness you want to land on in LUFS, e.g. -14. Leave at 0 to skip the loudness offset. The reported offset is target_lufs minus track_lufs."))
        .param(Param::enumv("output", ["report", "ffmpeg", "firequalizer", "csv"]).default("report").describe("'report' (default) prints the per-band table with track, target, raw difference, smoothed difference and final gain, plus the loudness offset and the ffmpeg command. 'ffmpeg' returns just the ready-to-paste command using peaking equalizer stages, 'firequalizer' the single-stage firequalizer form, and 'csv' a machine-readable per-band table."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SpectralEqMatch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spectral-eq-match",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Match a track's EQ curve and loudness to a reference and get the corrective filter chain.",
    skill(
        description = "Derive a corrective EQ from a measured/target spectrum pair. Paste your track's per-band levels (from any spectrum analyser or RTA) plus either a reference track's band levels or a built-in target curve (flat, pink, bright, warm, speech), and the tool returns the per-band corrective gains, a ready-to-paste ffmpeg equalizer or firequalizer filter chain, and the loudness offset in dB between the two integrated-loudness figures. Use amount to scale the move, max_gain_db to limit boosts and cuts, smoothing to favour broad tonal balance over narrow resonances, q to set the band bandwidth, and tone_only to separate tone matching from level matching.",
        parameters = schema_json()
    ),
)]
impl SpectralEqMatch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "spectral-eq-match", |a: Args| {
            gizza_ai_spectral_eq_match_core::match_eq(
                &a.track,
                &a.reference,
                &a.target_curve,
                a.amount,
                a.max_gain_db,
                a.smoothing,
                a.q,
                a.tone_only,
                a.track_lufs,
                a.target_lufs,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "track":{"type":"string","description":"Your track's measured per-band levels as frequency/level pairs, e.g. '63 -12, 250 -8, 1k -6, 4k -10, 12k -18' or one pair per line. Frequencies are Hz (a 'k' suffix means kHz), levels are dB on any consistent scale. Pairs may be separated by newlines, commas, semicolons, colons, equals signs or spaces. 2-64 bands."},
                "reference":{"type":"string","description":"The reference track's per-band levels in the same format. Used only when target_curve is 'reference'. The reference is interpolated over log frequency onto your track's band frequencies, so the two lists do not need the same band count."},
                "target_curve":{"type":"string","enum":["reference","flat","pink","bright","warm","speech"],"default":"pink","description":"What to match against. 'reference' uses the pasted reference levels. The built-in curves are relative to 0 dB at 1 kHz: 'flat' = 0 dB everywhere, 'pink' = -3 dB/octave (equal energy per octave, and the default so a track-only run succeeds), 'bright' = -2 dB/octave, 'warm' = -4 dB/octave, 'speech' = pink tilt with a rolloff below 150 Hz, a +3 dB presence lift at 3 kHz and an air cut above 10 kHz."},
                "amount":{"type":"number","default":100,"minimum":0,"maximum":100,"description":"Percentage of the derived correction to apply, 0-100. 100 matches the target fully; 50 applies half the move, which is usually the safer starting point on music."},
                "max_gain_db":{"type":"number","default":6,"minimum":0,"maximum":24,"description":"Symmetric boost/cut limit in dB, 0-24. Every corrective gain is clamped to +/- this value so a badly measured or out-of-range band cannot ask for an extreme filter. Default 6 dB."},
                "smoothing":{"type":"integer","default":1,"minimum":0,"maximum":4,"description":"Neighbouring-band averaging radius, 0-4. 0 follows the measurement exactly (narrow resonances included); 1-2 favours broad tonal balance; 4 gives a very gentle tilt-style correction."},
                "q":{"type":"number","default":1,"minimum":0.1,"maximum":10,"description":"Q (bandwidth) of every peaking 'equalizer' stage in the generated ffmpeg chain, 0.1-10. Roughly 1.4 for third-octave bands, 1.0 for octave bands; lower Q is broader. Ignored by the firequalizer output, which interpolates a continuous curve."},
                "tone_only":{"type":"boolean","default":true,"description":"When true (default), the average level difference between the two curves is removed before matching, so the EQ corrects TONE only and level is handled by the loudness offset. Set false to bake the broadband level difference into the band gains."},
                "track_lufs":{"type":"number","default":0,"minimum":-70,"maximum":0,"description":"Your track's integrated loudness in LUFS, e.g. -9.5. Leave at 0 to skip the loudness offset. Used with target_lufs to compute the dB offset and the trailing 'volume' stage."},
                "target_lufs":{"type":"number","default":0,"minimum":-70,"maximum":0,"description":"The loudness you want to land on in LUFS, e.g. -14. Leave at 0 to skip the loudness offset. The reported offset is target_lufs minus track_lufs."},
                "output":{"type":"string","enum":["report","ffmpeg","firequalizer","csv"],"default":"report","description":"'report' (default) prints the per-band table with track, target, raw difference, smoothed difference and final gain, plus the loudness offset and the ffmpeg command. 'ffmpeg' returns just the ready-to-paste command using peaking equalizer stages, 'firequalizer' the single-stage firequalizer form, and 'csv' a machine-readable per-band table."}
            },
            "required":["track"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }
}
