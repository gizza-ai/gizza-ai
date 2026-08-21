//! gizza-ai/mel-spectrogram-generator — render audio as a mel-scaled PNG spectrogram.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{build_media_envelope, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mel_spectrogram_generator_core::Options;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default = "d_n_fft")]
    n_fft: i64,
    #[serde(default)]
    hop_length: i64,
    #[serde(default = "d_n_mels")]
    n_mels: i64,
    #[serde(default)]
    fmin: f64,
    #[serde(default)]
    fmax: f64,
    #[serde(default)]
    mel_scale: String,
    #[serde(default)]
    window: String,
    #[serde(default)]
    scale: String,
    #[serde(default = "d_top_db")]
    top_db: f64,
    #[serde(default)]
    db_reference: String,
    #[serde(default)]
    colormap: String,
    #[serde(default = "d_width")]
    width: i64,
    #[serde(default = "d_height")]
    height: i64,
    #[serde(default)]
    channel: String,
    #[serde(default = "d_true")]
    center: bool,
    #[serde(default)]
    resample_hz: i64,
}

fn d_n_fft() -> i64 {
    2048
}
fn d_n_mels() -> i64 {
    128
}
fn d_top_db() -> f64 {
    80.0
}
fn d_width() -> i64 {
    1000
}
fn d_height() -> i64 {
    400
}
fn d_true() -> bool {
    true
}

impl Args {
    fn options(&self) -> Options {
        Options {
            n_fft: self.n_fft,
            hop_length: self.hop_length,
            n_mels: self.n_mels,
            fmin: self.fmin,
            fmax: self.fmax,
            mel_scale: self.mel_scale.clone(),
            window: self.window.clone(),
            scale: self.scale.clone(),
            top_db: self.top_db,
            db_reference: self.db_reference.clone(),
            colormap: self.colormap.clone(),
            width: self.width,
            height: self.height,
            channel: self.channel.clone(),
            center: self.center,
            resample_hz: self.resample_hz,
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe(
            "Audio file bytes encoded as base64 or hex. Decodes WAV, AIFF, CAF, FLAC, MP3, OGG/Vorbis, MP4/M4A, MKV/WebM and AAC-ADTS; the selected channel is rendered as a PNG mel spectrogram. Limit 24 MiB input bytes and 4,000,000 decoded samples.",
        ))
        .param(Param::enumv("input_format", ["base64", "hex"]).default("base64").describe(
            "Encoding used for the pasted audio bytes: base64 (default) or hex. The standalone page fills this from the uploaded file automatically.",
        ))
        .param(Param::integer("n_fft").min(64.0).max(8192.0).default(2048).describe(
            "FFT and analysis-window size in samples (64-8192, default 2048). Larger windows sharpen frequency detail but blur time detail.",
        ))
        .param(Param::integer("hop_length").min(0.0).max(8192.0).default(0).describe(
            "Samples between adjacent STFT frames (0 = n_fft/4, default 512 when n_fft is 2048). Smaller hops make a wider, denser image.",
        ))
        .param(Param::integer("n_mels").min(8.0).max(512.0).default(128).describe(
            "Number of triangular mel bands before image scaling (8-512, default 128). Common ML front ends use 64, 80 or 128 bands.",
        ))
        .param(Param::number("fmin").min(0.0).max(24000.0).default(0.0).describe(
            "Lowest mel filter edge in Hz (0-24000, default 0). Raise it to ignore rumble or telephone-band low cut.",
        ))
        .param(Param::number("fmax").min(0.0).max(24000.0).default(0.0).describe(
            "Highest mel filter edge in Hz (0-24000, default 0 = Nyquist for the analysis sample rate). Values above Nyquist are clamped.",
        ))
        .param(Param::enumv("mel_scale", ["slaney", "htk"]).default("slaney").describe(
            "Mel frequency formula: slaney (default, librosa-style area-normalized filters) or htk (classic 2595*log10 scale).",
        ))
        .param(Param::enumv("window", ["hann", "hamming", "blackman", "rectangular"]).default("hann").describe(
            "Window applied before each FFT: hann (default), hamming, blackman, or rectangular/no taper.",
        ))
        .param(Param::enumv("scale", ["db", "power", "magnitude"]).default("db").describe(
            "Pixel scaling: db (default log/dB spectrogram), power (linear mel power), or magnitude (square-root power).",
        ))
        .param(Param::number("top_db").min(10.0).max(160.0).default(80.0).describe(
            "Dynamic range shown below the reference level when scale=db (10-160 dB, default 80). Lower values increase contrast.",
        ))
        .param(Param::enumv("db_reference", ["peak", "full_scale"]).default("peak").describe(
            "dB reference for scale=db: peak maps the clip's loudest cell to the top color (default); full_scale keeps quiet clips visibly dimmer.",
        ))
        .param(Param::enumv("colormap", ["magma", "inferno", "viridis", "plasma", "turbo", "grayscale", "grayscale_inverted"]).default("magma").describe(
            "Color palette for the PNG: magma (default), inferno, viridis, plasma, turbo, grayscale, or grayscale_inverted.",
        ))
        .param(Param::integer("width").min(0.0).max(4096.0).default(1000).describe(
            "Output width in pixels (0 = one pixel per STFT frame, otherwise 32-4096; default 1000).",
        ))
        .param(Param::integer("height").min(0.0).max(2048.0).default(400).describe(
            "Output height in pixels (0 = one row per mel band, otherwise 32-2048; default 400).",
        ))
        .param(Param::enumv("channel", ["mix", "left", "right"]).default("mix").describe(
            "Audio channel selection before analysis: mix (default downmix to mono), left, or right. Mono sources fall back to their only channel.",
        ))
        .param(Param::boolean("center").default(true).describe(
            "Reflect-pad by n_fft/2 before framing, matching librosa center=True (default). Turn off for only complete in-file frames.",
        ))
        .param(Param::integer("resample_hz").min(0.0).max(48000.0).default(0).describe(
            "Resample before analysis (0 = keep the source rate, otherwise 4000-48000 Hz). Use 16000 or 22050 to match common ML pipelines.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MelSpectrogramGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mel-spectrogram-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render audio as a PNG mel spectrogram for ML debugging",
    skill(
        description = "Render an uploaded or pasted audio file as a mel-scaled PNG spectrogram. The tool decodes common audio containers locally, downmixes or selects a channel, optionally resamples, applies an STFT, triangular mel filterbank, dB/power scaling and a selectable colormap. It returns an image envelope plus a short report with the resolved FFT, hop, sample rate, mel bands, peak band and PNG size.",
        parameters = schema_json()
    ),
)]
impl MelSpectrogramGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid mel-spectrogram-generator args: {e}"))
    })?;
    let rendered = gizza_ai_mel_spectrogram_generator_core::run(
        &args.input,
        if args.input_format.trim().is_empty() {
            "base64"
        } else {
            &args.input_format
        },
        &args.options(),
    )
    .map_err(SkillError::InvalidArgs)?;
    let for_llm = rendered.summary.clone();
    build_media_envelope(
        &rendered.png,
        "image/png",
        "mel-spectrogram.png".to_string(),
        for_llm,
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_expected_surface() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();
        assert_eq!(schema.get("type").unwrap(), "object");
        assert_eq!(schema.get("additionalProperties").unwrap(), false);
        assert_eq!(
            schema.get("required").unwrap(),
            &serde_json::json!(["input"])
        );
        assert_eq!(props.as_object().unwrap().len(), 18);
        assert_eq!(props["input"]["type"], "string");
        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["base64", "hex"])
        );
        assert_eq!(props["n_fft"]["default"], 2048);
        assert_eq!(props["hop_length"]["default"], 0);
        assert_eq!(props["n_mels"]["default"], 128);
        assert_eq!(
            props["mel_scale"]["enum"],
            serde_json::json!(["slaney", "htk"])
        );
        assert_eq!(
            props["window"]["enum"],
            serde_json::json!(["hann", "hamming", "blackman", "rectangular"])
        );
        assert_eq!(
            props["scale"]["enum"],
            serde_json::json!(["db", "power", "magnitude"])
        );
        assert_eq!(
            props["db_reference"]["enum"],
            serde_json::json!(["peak", "full_scale"])
        );
        assert_eq!(props["colormap"]["default"], "magma");
        assert_eq!(props["width"]["default"], 1000);
        assert_eq!(props["height"]["default"], 400);
        assert_eq!(props["center"]["type"], "boolean");
        assert_eq!(props["center"]["default"], true);
        for key in props.as_object().unwrap().keys() {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing .describe() for {key}"
            );
        }
    }

    #[test]
    fn args_defaults_match_core_defaults() {
        let a: Args = serde_json::from_str(r#"{"input":"AA"}"#).unwrap();
        let o = a.options();
        let d = Options::default();
        assert_eq!(o.n_fft, d.n_fft);
        assert_eq!(o.hop_length, d.hop_length);
        assert_eq!(o.n_mels, d.n_mels);
        assert_eq!(o.fmin, d.fmin);
        assert_eq!(o.fmax, d.fmax);
        assert_eq!(o.top_db, d.top_db);
        assert_eq!(o.width, d.width);
        assert_eq!(o.height, d.height);
        assert_eq!(o.center, d.center);
        assert_eq!(o.resample_hz, d.resample_hz);
    }
}
