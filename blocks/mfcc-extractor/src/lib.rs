//! gizza-ai/mfcc-extractor — chat skill block on the shared tool abstraction.
//!
//! Thin wrapper around `gizza-ai-mfcc-extractor-core`. The descriptor is the
//! single source for the chat schema, the CLI, and the generated page controls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mfcc_extractor_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output: String,
    #[serde(default = "d_n_mfcc")]
    n_mfcc: i64,
    #[serde(default = "d_n_mels")]
    n_mels: i64,
    #[serde(default = "d_frame_ms")]
    frame_ms: f64,
    #[serde(default = "d_hop_ms")]
    hop_ms: f64,
    #[serde(default)]
    fmin: f64,
    #[serde(default)]
    fmax: f64,
    #[serde(default)]
    window: String,
    #[serde(default = "d_preemphasis")]
    preemphasis: f64,
    #[serde(default = "d_lifter")]
    lifter: f64,
    #[serde(default)]
    mel_scale: String,
    #[serde(default = "d_true")]
    append_energy: bool,
    #[serde(default)]
    deltas: String,
    #[serde(default = "d_true")]
    include_time: bool,
    #[serde(default = "d_decimals")]
    decimals: i64,
    #[serde(default)]
    resample_hz: i64,
}

fn d_n_mfcc() -> i64 {
    13
}
fn d_n_mels() -> i64 {
    26
}
fn d_frame_ms() -> f64 {
    25.0
}
fn d_hop_ms() -> f64 {
    10.0
}
fn d_preemphasis() -> f64 {
    0.97
}
fn d_lifter() -> f64 {
    22.0
}
fn d_decimals() -> i64 {
    6
}
fn d_true() -> bool {
    true
}

impl Args {
    fn options(&self) -> Options {
        Options {
            output: self.output.clone(),
            n_mfcc: self.n_mfcc,
            n_mels: self.n_mels,
            frame_ms: self.frame_ms,
            hop_ms: self.hop_ms,
            fmin: self.fmin,
            fmax: self.fmax,
            window: self.window.clone(),
            preemphasis: self.preemphasis,
            lifter: self.lifter,
            mel_scale: self.mel_scale.clone(),
            append_energy: self.append_energy,
            deltas: self.deltas.clone(),
            include_time: self.include_time,
            decimals: self.decimals,
            resample_hz: self.resample_hz,
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe(
            "Audio file bytes encoded as base64 or hex, e.g. 'UklGRi...' for a WAV. Decodes \
             WAV, AIFF, CAF, FLAC, MP3, OGG/Vorbis, MP4/M4A (AAC-LC, ALAC), MKV/WebM and \
             AAC-ADTS; the first audio track is used and every channel is downmixed to mono. \
             Limit 24 MiB of decoded bytes, 4,000,000 analysed samples and 200,000 output frames.",
        ))
        .param(
            Param::enumv("input_format", ["base64", "hex"])
                .default("base64")
                .describe(
                    "Encoding used for the pasted audio bytes: 'base64' (default) or 'hex'. \
                     Base64 may be standard or URL-safe and padding is optional; hex may include \
                     whitespace, ':' or '-' separators.",
                ),
        )
        .param(
            Param::enumv("output", ["csv", "tsv", "json"])
                .default("csv")
                .describe(
                    "Output format. 'csv' (default) is one header row plus one row per analysis \
                     frame; 'tsv' is the same matrix tab-separated; 'json' wraps the matrix in an \
                     object that also reports the resolved sample rate, frame/hop/FFT sizes and \
                     every setting actually used.",
                ),
        )
        .param(
            Param::integer("n_mfcc")
                .min(1.0)
                .max(64.0)
                .default(13)
                .describe(
                    "Number of cepstral coefficients kept per frame (1-64, default 13, the usual \
                     speech-recognition front end; music work often uses 20). Must not exceed \
                     n_mels. C0 is the overall energy/loudness term, C1 upward describe the \
                     spectral envelope shape.",
                ),
        )
        .param(
            Param::integer("n_mels")
                .min(4.0)
                .max(256.0)
                .default(26)
                .describe(
                    "Number of triangular mel filters in the filterbank the DCT is applied to \
                     (4-256, default 26; 40 and 128 are the other common choices). More filters \
                     need a longer frame_ms — a filter narrower than one FFT bin is rejected.",
                ),
        )
        .param(
            Param::number("frame_ms")
                .min(1.0)
                .max(200.0)
                .default(25.0)
                .describe(
                    "Analysis frame (window) length in milliseconds (1-200, default 25). The FFT \
                     size is the next power of two at or above this many samples. Speech uses \
                     20-30 ms; longer frames give finer frequency detail and coarser timing.",
                ),
        )
        .param(
            Param::number("hop_ms")
                .min(1.0)
                .max(200.0)
                .default(10.0)
                .describe(
                    "Step between consecutive frames in milliseconds (1-200, default 10, i.e. 100 \
                     frames per second). Smaller values overlap more and produce more rows.",
                ),
        )
        .param(
            Param::number("fmin")
                .min(0.0)
                .max(24000.0)
                .default(0.0)
                .describe(
                    "Lowest frequency covered by the filterbank, in Hz (0-24000, default 0). \
                     Telephone-band speech pipelines often set 133 or 300 Hz to drop rumble.",
                ),
        )
        .param(
            Param::number("fmax")
                .min(0.0)
                .max(24000.0)
                .default(0.0)
                .describe(
                    "Highest frequency covered by the filterbank, in Hz (0-24000, default 0 which \
                     means the Nyquist rate, half the analysis sample rate). Values above Nyquist \
                     are clamped down to it.",
                ),
        )
        .param(
            Param::enumv("window", ["hamming", "hann", "blackman", "rectangular"])
                .default("hamming")
                .describe(
                    "Analysis window applied to each frame before the FFT: 'hamming' (default, \
                     the speech-toolkit standard), 'hann', 'blackman' (lowest spectral leakage) \
                     or 'rectangular' (no window).",
                ),
        )
        .param(
            Param::number("preemphasis")
                .min(0.0)
                .max(1.0)
                .default(0.97)
                .describe(
                    "Pre-emphasis coefficient a in y[n] = x[n] - a*x[n-1], applied before framing \
                     (0-1, default 0.97). It boosts high frequencies to offset the natural \
                     spectral tilt of voiced speech; set 0 to disable the filter.",
                ),
        )
        .param(
            Param::number("lifter")
                .min(0.0)
                .max(100.0)
                .default(22.0)
                .describe(
                    "Sinusoidal cepstral lifter L, scaling coefficient k by 1 + (L/2)*sin(pi*k/L) \
                     (0-100, default 22). It evens out the dynamic range of the higher \
                     coefficients; set 0 to disable liftering.",
                ),
        )
        .param(
            Param::enumv("mel_scale", ["htk", "slaney"])
                .default("htk")
                .describe(
                    "Mel-scale formula for spacing the filters: 'htk' (default) uses \
                     2595*log10(1+f/700) with unit-height filters; 'slaney' uses the linear-below \
                     -1kHz auditory scale and normalises each filter by its bandwidth so all \
                     filters carry equal area.",
                ),
        )
        .param(Param::boolean("append_energy").default(true).describe(
            "Replace coefficient C0 with the natural log of the frame's total energy \
                     (default true, the speech-toolkit convention). Turn it off to keep the plain \
                     DCT term, which is the mean log-mel energy instead.",
        ))
        .param(
            Param::enumv("deltas", ["none", "delta", "delta_delta"])
                .default("none")
                .describe(
                    "Append derivative features computed over a +/-2 frame span: 'none' \
                     (default), 'delta' adds first-order columns d0..dN, 'delta_delta' also adds \
                     second-order columns dd0..dN. Standard extras for acoustic models.",
                ),
        )
        .param(Param::boolean("include_time").default(true).describe(
            "Prepend a 'time_s' column holding each frame's start time in seconds \
                     (default true). Turn it off for a bare coefficient matrix ready to load \
                     straight into a feature array.",
        ))
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(8.0)
                .default(6)
                .describe(
                    "Decimal places used when printing the coefficients (0-8, default 6). \
                     Timestamps always keep at least 3 decimals.",
                ),
        )
        .param(
            Param::integer("resample_hz")
                .min(0.0)
                .max(48000.0)
                .default(0)
                .describe(
                    "Resample the audio to this rate in Hz before analysis (0 = keep the file's \
                     own rate, otherwise 4000-48000). Set 16000 to match the usual speech feature \
                     pipeline so files of different rates give comparable matrices.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MfccExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mfcc-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract MFCC coefficients from audio as a CSV, TSV or JSON matrix.",
    skill(
        description = "Extract Mel-frequency cepstral coefficients (MFCCs) from a speech or music file and return them as a frames-by-coefficients matrix. Paste the file bytes as base64 or hex. The tool decodes WAV, AIFF, CAF, FLAC, MP3, OGG, MP4/M4A, MKV/WebM and AAC-ADTS with a pure-Rust decoder, downmixes to mono, optionally resamples, then runs the standard pipeline: pre-emphasis, framing every hop_ms into frame_ms windows, windowing, FFT power spectrum, a triangular mel filterbank (HTK or Slaney scale), natural log, orthonormal DCT-II, cepstral liftering, and optional log-energy in C0 plus delta and delta-delta columns. Output as CSV (default), TSV, or JSON with the resolved frame, hop, FFT size and settings alongside the matrix. Frames are complete windows only, taken from sample 0 with no centring or padding.",
        parameters = schema_json()
    ),
)]
impl MfccExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mfcc-extractor", |a: Args| {
            gizza_ai_mfcc_extractor_core::run(&a.input, &a.input_format, &a.options())
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

    /// Drift guard: the chat/CLI/page schema is generated from descriptor(), so
    /// this pins every param the surfaces depend on.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();
        assert_eq!(schema.get("type").unwrap(), "object");
        assert_eq!(schema.get("additionalProperties").unwrap(), false);
        assert_eq!(
            schema.get("required").unwrap(),
            &serde_json::json!(["input"])
        );

        assert_eq!(props["input"]["type"], "string");
        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
        assert_eq!(props["input_format"]["default"], "base64");
        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["csv", "tsv", "json"])
        );
        assert_eq!(props["output"]["default"], "csv");
        assert_eq!(
            props["window"]["enum"],
            serde_json::json!(["hamming", "hann", "blackman", "rectangular"])
        );
        assert_eq!(props["window"]["default"], "hamming");
        assert_eq!(
            props["mel_scale"]["enum"],
            serde_json::json!(["htk", "slaney"])
        );
        assert_eq!(props["mel_scale"]["default"], "htk");
        assert_eq!(
            props["deltas"]["enum"],
            serde_json::json!(["none", "delta", "delta_delta"])
        );
        assert_eq!(props["deltas"]["default"], "none");

        assert_eq!(props["n_mfcc"]["type"], "integer");
        assert_eq!(props["n_mfcc"]["minimum"], 1.0);
        assert_eq!(props["n_mfcc"]["maximum"], 64.0);
        assert_eq!(props["n_mfcc"]["default"], 13);
        assert_eq!(props["n_mels"]["type"], "integer");
        assert_eq!(props["n_mels"]["minimum"], 4.0);
        assert_eq!(props["n_mels"]["maximum"], 256.0);
        assert_eq!(props["n_mels"]["default"], 26);
        assert_eq!(props["frame_ms"]["type"], "number");
        assert_eq!(props["frame_ms"]["minimum"], 1.0);
        assert_eq!(props["frame_ms"]["maximum"], 200.0);
        assert_eq!(props["frame_ms"]["default"], 25.0);
        assert_eq!(props["hop_ms"]["type"], "number");
        assert_eq!(props["hop_ms"]["default"], 10.0);
        assert_eq!(props["fmin"]["type"], "number");
        assert_eq!(props["fmin"]["default"], 0.0);
        assert_eq!(props["fmax"]["type"], "number");
        assert_eq!(props["fmax"]["maximum"], 24000.0);
        assert_eq!(props["fmax"]["default"], 0.0);
        assert_eq!(props["preemphasis"]["type"], "number");
        assert_eq!(props["preemphasis"]["default"], 0.97);
        assert_eq!(props["lifter"]["type"], "number");
        assert_eq!(props["lifter"]["maximum"], 100.0);
        assert_eq!(props["lifter"]["default"], 22.0);
        assert_eq!(props["append_energy"]["type"], "boolean");
        assert_eq!(props["append_energy"]["default"], true);
        assert_eq!(props["include_time"]["type"], "boolean");
        assert_eq!(props["include_time"]["default"], true);
        assert_eq!(props["decimals"]["type"], "integer");
        assert_eq!(props["decimals"]["maximum"], 8.0);
        assert_eq!(props["decimals"]["default"], 6);
        assert_eq!(props["resample_hz"]["type"], "integer");
        assert_eq!(props["resample_hz"]["maximum"], 48000.0);
        assert_eq!(props["resample_hz"]["default"], 0);

        for key in [
            "input",
            "input_format",
            "output",
            "n_mfcc",
            "n_mels",
            "frame_ms",
            "hop_ms",
            "fmin",
            "fmax",
            "window",
            "preemphasis",
            "lifter",
            "mel_scale",
            "append_energy",
            "deltas",
            "include_time",
            "decimals",
            "resample_hz",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing .describe() for {key}"
            );
        }
        assert_eq!(props.as_object().unwrap().len(), 18);
    }

    /// The serde defaults on Args must agree with the descriptor defaults, or
    /// a chat call that omits a field would behave differently from the page.
    #[test]
    fn args_defaults_match_core_defaults() {
        let a: Args = serde_json::from_str(r#"{"input":"AA"}"#).unwrap();
        let o = a.options();
        let d = Options::default();
        assert_eq!(o.n_mfcc, d.n_mfcc);
        assert_eq!(o.n_mels, d.n_mels);
        assert_eq!(o.frame_ms, d.frame_ms);
        assert_eq!(o.hop_ms, d.hop_ms);
        assert_eq!(o.fmin, d.fmin);
        assert_eq!(o.fmax, d.fmax);
        assert_eq!(o.preemphasis, d.preemphasis);
        assert_eq!(o.lifter, d.lifter);
        assert_eq!(o.append_energy, d.append_energy);
        assert_eq!(o.include_time, d.include_time);
        assert_eq!(o.decimals, d.decimals);
        assert_eq!(o.resample_hz, d.resample_hz);
    }
}
