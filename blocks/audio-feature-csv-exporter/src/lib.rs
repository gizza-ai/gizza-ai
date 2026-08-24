//! gizza-ai/audio-feature-csv-exporter — chat skill block on the shared tool
//! abstraction.
//!
//! Thin wrapper around `gizza-ai-audio-feature-csv-exporter-core`. The
//! descriptor is the single source for the chat schema, the CLI, and the
//! generated page controls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_audio_feature_csv_exporter_core::Options;
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
    #[serde(default = "d_frame_ms")]
    frame_ms: f64,
    #[serde(default = "d_hop_ms")]
    hop_ms: f64,
    #[serde(default)]
    window: String,
    #[serde(default)]
    center: bool,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    resample_hz: i64,
    #[serde(default = "d_true")]
    rms: bool,
    #[serde(default = "d_true")]
    centroid: bool,
    #[serde(default = "d_true")]
    zcr: bool,
    #[serde(default = "d_true")]
    rolloff: bool,
    #[serde(default = "d_true")]
    flatness: bool,
    #[serde(default)]
    bandwidth: bool,
    #[serde(default)]
    flux: bool,
    #[serde(default = "d_rolloff_percent")]
    rolloff_percent: f64,
    #[serde(default)]
    rms_scale: String,
    #[serde(default)]
    flatness_scale: String,
    #[serde(default = "d_true")]
    include_time: bool,
    #[serde(default)]
    include_frame: bool,
    #[serde(default = "d_decimals")]
    decimals: i64,
}

fn d_frame_ms() -> f64 {
    25.0
}
fn d_hop_ms() -> f64 {
    10.0
}
fn d_rolloff_percent() -> f64 {
    85.0
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
            frame_ms: self.frame_ms,
            hop_ms: self.hop_ms,
            window: self.window.clone(),
            center: self.center,
            channel: self.channel.clone(),
            resample_hz: self.resample_hz,
            rms: self.rms,
            centroid: self.centroid,
            zcr: self.zcr,
            rolloff: self.rolloff,
            flatness: self.flatness,
            bandwidth: self.bandwidth,
            flux: self.flux,
            rolloff_percent: self.rolloff_percent,
            rms_scale: self.rms_scale.clone(),
            flatness_scale: self.flatness_scale.clone(),
            include_time: self.include_time,
            include_frame: self.include_frame,
            decimals: self.decimals,
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe(
            "Audio file bytes encoded as base64 or hex, e.g. 'UklGRi...' for a WAV. Decodes \
             WAV, AIFF, CAF, FLAC, MP3, OGG/Vorbis, MP4/M4A (AAC-LC, ALAC), MKV/WebM and \
             AAC-ADTS; the first decodable audio track is analysed. Limit 24 MiB of decoded \
             bytes, 4,000,000 analysed samples and 200,000 output rows.",
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
                     frame, ready for pandas, R or a spreadsheet; 'tsv' is the same table \
                     tab-separated; 'json' wraps the table in an object that also reports the \
                     resolved sample rate, frame/hop/FFT sizes, column names and every setting \
                     actually used.",
                ),
        )
        .param(
            Param::number("frame_ms")
                .min(1.0)
                .max(500.0)
                .default(25.0)
                .describe(
                    "Analysis frame (window) length in milliseconds (1-500, default 25). The FFT \
                     size is the next power of two at or above this many samples. Short frames \
                     track transients; long frames resolve low frequencies more finely.",
                ),
        )
        .param(
            Param::number("hop_ms")
                .min(1.0)
                .max(500.0)
                .default(10.0)
                .describe(
                    "Step between consecutive frames in milliseconds (1-500, default 10, i.e. 100 \
                     rows per second of audio). Smaller values overlap more and produce more rows.",
                ),
        )
        .param(
            Param::enumv("window", ["hann", "hamming", "blackman", "rectangular"])
                .default("hann")
                .describe(
                    "Analysis window applied to each frame before the FFT: 'hann' (default, the \
                     librosa standard), 'hamming', 'blackman' (lowest spectral leakage) or \
                     'rectangular' (no window). Time-domain columns (rms, zcr) always read the \
                     raw, unwindowed frame.",
                ),
        )
        .param(Param::boolean("center").default(false).describe(
            "Pad half a frame of zeros at both ends so frame t is centred on t * hop_ms \
             (librosa's center=True). Default false, which takes complete frames from sample 0. \
             Centring adds two partly-silent edge frames but makes timestamps line up with \
             librosa output.",
        ))
        .param(
            Param::enumv("channel", ["mix", "left", "right"])
                .default("mix")
                .describe(
                    "Which channel feeds the analysis: 'mix' (default) averages every channel to \
                     mono, 'left' and 'right' read one side of a stereo file. A mono file falls \
                     back to its only channel.",
                ),
        )
        .param(
            Param::integer("resample_hz")
                .min(0.0)
                .max(48000.0)
                .default(0)
                .describe(
                    "Resample the audio to this rate in Hz before analysis (0 = keep the file's \
                     own rate, otherwise 4000-48000). Set 16000 or 22050 so files recorded at \
                     different rates produce comparable feature tables.",
                ),
        )
        .param(Param::boolean("rms").default(true).describe(
            "Emit the frame loudness column: the root-mean-square of the raw frame, printed \
             as 'rms_dbfs' or 'rms' depending on rms_scale. The workhorse column for silence \
             detection, level plots and gating.",
        ))
        .param(Param::boolean("centroid").default(true).describe(
            "Emit 'centroid_hz', the magnitude spectrum's centre of mass — the standard \
             correlate of perceived brightness, and a staple feature for timbre and \
             instrument classification.",
        ))
        .param(Param::boolean("zcr").default(true).describe(
            "Emit 'zcr', the fraction of neighbouring sample pairs in the frame that change \
             sign (0-1). Cheap and effective for separating voiced speech from unvoiced \
             fricatives, and for percussive-onset heuristics.",
        ))
        .param(Param::boolean("rolloff").default(true).describe(
            "Emit 'rolloff_hz', the lowest frequency below which rolloff_percent of the \
             frame's spectral magnitude sits. Summarises where a frame's energy stops, so it \
             separates bright from dark or band-limited material.",
        ))
        .param(Param::boolean("flatness").default(true).describe(
            "Emit the spectral flatness column ('flatness' or 'flatness_db'): the geometric \
             mean of the power spectrum over its arithmetic mean. Near 1 for noise-like \
             frames, near 0 for tonal ones — the usual noise-vs-tone discriminator.",
        ))
        .param(Param::boolean("bandwidth").default(false).describe(
            "Emit 'bandwidth_hz', the magnitude-weighted spread of the spectrum around the \
             centroid (librosa's order-2 spectral bandwidth). Off by default; turn it on for \
             timbre work where brightness alone is not enough.",
        ))
        .param(Param::boolean("flux").default(false).describe(
            "Emit 'flux', the L2 norm of the positive frame-to-frame change in the \
             L1-normalised magnitude spectrum. Off by default; it is the classic onset-detection \
             function, and the first row is 0 because it has no predecessor.",
        ))
        .param(
            Param::number("rolloff_percent")
                .min(1.0)
                .max(99.0)
                .default(85.0)
                .describe(
                    "Share of the frame's spectral magnitude that must fall below rolloff_hz \
                     (1-99, default 85, the usual MIR value). 95 is the other common choice; low \
                     values such as 15 approximate a spectral-minimum contour instead.",
                ),
        )
        .param(
            Param::enumv("rms_scale", ["dbfs", "linear"])
                .default("dbfs")
                .describe(
                    "Units for the loudness column: 'dbfs' (default) prints 20*log10(rms) with a \
                     -200 dBFS floor, so digital silence stays a finite number; 'linear' prints \
                     the raw 0-1 amplitude and names the column 'rms'.",
                ),
        )
        .param(
            Param::enumv("flatness_scale", ["ratio", "db"])
                .default("ratio")
                .describe(
                    "Units for the flatness column: 'ratio' (default) prints the raw 0-1 value as \
                     'flatness'; 'db' prints 10*log10(ratio) as 'flatness_db', which spreads out \
                     the tonal end of the range.",
                ),
        )
        .param(Param::boolean("include_time").default(true).describe(
            "Prepend a 'time_s' column holding each frame's start time in seconds (default \
             true) so the table plots straight against a time axis. Timestamps always keep at \
             least 3 decimals.",
        ))
        .param(Param::boolean("include_frame").default(false).describe(
            "Prepend a 'frame' column holding the zero-based frame index (default false). \
             Useful when you need to join the table back to another frame-indexed export such \
             as an MFCC matrix.",
        ))
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(8.0)
                .default(6)
                .describe(
                    "Decimal places used when printing the feature values (0-8, default 6). \
                     Lower values make a much smaller CSV; timestamps always keep at least 3.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AudioFeatureCsvExporter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/audio-feature-csv-exporter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Export per-frame audio features (RMS, centroid, ZCR, rolloff, flatness) as CSV.",
    skill(
        description = "Export frame-level audio features from an audio file as a CSV, TSV or JSON table, one row per analysis frame. Paste the file bytes as base64 or hex. The tool decodes WAV, AIFF, CAF, FLAC, MP3, OGG, MP4/M4A, MKV/WebM and AAC-ADTS with a pure-Rust decoder, picks or downmixes a channel, optionally resamples, then frames the signal every hop_ms into frame_ms windows and computes RMS level (linear or dBFS), spectral centroid, zero-crossing rate, spectral rolloff at a chosen energy percentage, spectral flatness (ratio or dB), and optionally spectral bandwidth and spectral flux. Column definitions follow librosa's feature family, with an optional librosa-style centred framing mode. Output as CSV (default), TSV, or JSON with the resolved sample rate, frame/hop/FFT sizes and settings alongside the table.",
        parameters = schema_json()
    ),
)]
impl AudioFeatureCsvExporter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "audio-feature-csv-exporter", |a: Args| {
            gizza_ai_audio_feature_csv_exporter_core::run(&a.input, &a.input_format, &a.options())
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
            serde_json::json!(["hann", "hamming", "blackman", "rectangular"])
        );
        assert_eq!(props["window"]["default"], "hann");
        assert_eq!(
            props["channel"]["enum"],
            serde_json::json!(["mix", "left", "right"])
        );
        assert_eq!(props["channel"]["default"], "mix");
        assert_eq!(
            props["rms_scale"]["enum"],
            serde_json::json!(["dbfs", "linear"])
        );
        assert_eq!(props["rms_scale"]["default"], "dbfs");
        assert_eq!(
            props["flatness_scale"]["enum"],
            serde_json::json!(["ratio", "db"])
        );
        assert_eq!(props["flatness_scale"]["default"], "ratio");

        assert_eq!(props["frame_ms"]["type"], "number");
        assert_eq!(props["frame_ms"]["minimum"], 1.0);
        assert_eq!(props["frame_ms"]["maximum"], 500.0);
        assert_eq!(props["frame_ms"]["default"], 25.0);
        assert_eq!(props["hop_ms"]["type"], "number");
        assert_eq!(props["hop_ms"]["minimum"], 1.0);
        assert_eq!(props["hop_ms"]["maximum"], 500.0);
        assert_eq!(props["hop_ms"]["default"], 10.0);
        assert_eq!(props["rolloff_percent"]["type"], "number");
        assert_eq!(props["rolloff_percent"]["minimum"], 1.0);
        assert_eq!(props["rolloff_percent"]["maximum"], 99.0);
        assert_eq!(props["rolloff_percent"]["default"], 85.0);
        assert_eq!(props["resample_hz"]["type"], "integer");
        assert_eq!(props["resample_hz"]["minimum"], 0.0);
        assert_eq!(props["resample_hz"]["maximum"], 48000.0);
        assert_eq!(props["resample_hz"]["default"], 0);
        assert_eq!(props["decimals"]["type"], "integer");
        assert_eq!(props["decimals"]["minimum"], 0.0);
        assert_eq!(props["decimals"]["maximum"], 8.0);
        assert_eq!(props["decimals"]["default"], 6);

        for (key, default) in [
            ("center", false),
            ("rms", true),
            ("centroid", true),
            ("zcr", true),
            ("rolloff", true),
            ("flatness", true),
            ("bandwidth", false),
            ("flux", false),
            ("include_time", true),
            ("include_frame", false),
        ] {
            assert_eq!(props[key]["type"], "boolean", "{key}");
            assert_eq!(props[key]["default"], default, "{key}");
        }

        for key in [
            "input",
            "input_format",
            "output",
            "frame_ms",
            "hop_ms",
            "window",
            "center",
            "channel",
            "resample_hz",
            "rms",
            "centroid",
            "zcr",
            "rolloff",
            "flatness",
            "bandwidth",
            "flux",
            "rolloff_percent",
            "rms_scale",
            "flatness_scale",
            "include_time",
            "include_frame",
            "decimals",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing .describe() for {key}"
            );
        }
        assert_eq!(props.as_object().unwrap().len(), 22);
    }

    /// The serde defaults on Args must agree with the core defaults, or a chat
    /// call that omits a field would behave differently from the page.
    #[test]
    fn args_defaults_match_core_defaults() {
        let a: Args = serde_json::from_str(r#"{"input":"AA"}"#).unwrap();
        let o = a.options();
        let d = Options::default();
        assert_eq!(o.frame_ms, d.frame_ms);
        assert_eq!(o.hop_ms, d.hop_ms);
        assert_eq!(o.center, d.center);
        assert_eq!(o.resample_hz, d.resample_hz);
        assert_eq!(o.rms, d.rms);
        assert_eq!(o.centroid, d.centroid);
        assert_eq!(o.zcr, d.zcr);
        assert_eq!(o.rolloff, d.rolloff);
        assert_eq!(o.flatness, d.flatness);
        assert_eq!(o.bandwidth, d.bandwidth);
        assert_eq!(o.flux, d.flux);
        assert_eq!(o.rolloff_percent, d.rolloff_percent);
        assert_eq!(o.include_time, d.include_time);
        assert_eq!(o.include_frame, d.include_frame);
        assert_eq!(o.decimals, d.decimals);
        // Blank enum strings resolve to the descriptor defaults inside core.
        assert!(o.output.is_empty() && o.window.is_empty() && o.channel.is_empty());
    }
}
