//! gizza-ai/fft-analyzer — discrete Fourier transform of a pasted sample list.
//! Thin chat-skill wrapper around the pure core; the descriptor single-sources
//! the chat schema, the CLI, and the page form.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_sample_rate")]
    sample_rate: f64,
    #[serde(default = "default_window")]
    window: String,
    #[serde(default = "default_pad")]
    pad: String,
    #[serde(default = "default_spectrum")]
    spectrum: String,
    #[serde(default = "default_scale")]
    scale: String,
    #[serde(default = "default_phase_unit")]
    phase_unit: String,
    #[serde(default)]
    remove_dc: bool,
    #[serde(default = "default_peaks")]
    peaks: i64,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_format")]
    format: String,
}

fn default_sample_rate() -> f64 { 1.0 }
fn default_window() -> String { "rectangular".to_string() }
fn default_pad() -> String { "pow2".to_string() }
fn default_spectrum() -> String { "auto".to_string() }
fn default_scale() -> String { "amplitude".to_string() }
fn default_phase_unit() -> String { "degrees".to_string() }
fn default_peaks() -> i64 { 5 }
fn default_decimals() -> i64 { 4 }
fn default_format() -> String { "text".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data").required().describe(
                "The sample list to transform, in time order. Values may be separated by commas, semicolons, spaces, or newlines, and written as integers, decimals, or scientific notation (1, -2.5, 2.7e3). Complex samples are accepted in the forms 3+4i, -2i, or 1.25-0.5j (j is treated as i); any complex sample switches the default view to the two-sided spectrum. Minimum 2 samples, maximum 65536.",
            ),
        )
        .param(
            Param::number("sample_rate").default(1.0).describe(
                "Sampling rate of the input in samples per second (Hz). This only sets the frequency axis: bin k maps to k * sample_rate / transform_length, the bin resolution is sample_rate / transform_length, and Nyquist is sample_rate / 2. Default 1.0, which reports normalised frequency in cycles per sample. Use 48000 for 48 kHz audio.",
            ),
        )
        .param(
            Param::enumv(
                "window",
                ["rectangular", "hann", "hamming", "blackman", "blackman-harris", "flattop"],
            )
            .default("rectangular")
            .describe(
                "Window applied to the samples before the transform, to suppress spectral leakage from tones that do not land exactly on a bin. rectangular (default) applies no taper and gives the sharpest peaks for exactly-periodic input; hann and hamming are the general-purpose choices; blackman and blackman-harris trade a wider main lobe for much lower side lobes; flattop is the most accurate for measuring peak amplitude. Amplitudes are corrected for the window's coherent gain, so a windowed tone still reads its true amplitude.",
            ),
        )
        .param(
            Param::enumv("pad", ["pow2", "none"]).default("pow2").describe(
                "Transform length. pow2 (default) zero-pads the input up to the next power of two and uses the radix-2 FFT. none transforms the samples at their exact length with a direct DFT, which is O(N^2) and therefore capped at 4096 samples for non-power-of-two lengths. Zero padding interpolates the spectrum onto a finer grid; it does not add real resolution.",
            ),
        )
        .param(
            Param::enumv("spectrum", ["auto", "one-sided", "two-sided"])
                .default("auto")
                .describe(
                    "Which half of the spectrum to report. auto (default) shows the one-sided spectrum (bins 0 to N/2) for real input and the full two-sided spectrum for complex input. one-sided forces bins 0 to N/2 and doubles the amplitude of every bin except DC and Nyquist. two-sided forces all N bins and labels bins above N/2 as negative frequencies.",
                ),
        )
        .param(
            Param::enumv("scale", ["amplitude", "magnitude", "normalized", "db", "power"])
                .default("amplitude")
                .describe(
                    "How the level column is scaled. amplitude (default) reports the amplitude of the underlying sinusoid, so a unit cosine reads 1.0. magnitude reports the raw unnormalised |X[k]|, which grows with the number of samples. normalized divides the magnitude by the sample count without the one-sided doubling. db is 20*log10 of the amplitude, so a unit tone reads 0 dB and an empty bin reads -inf. power is the amplitude squared.",
                ),
        )
        .param(
            Param::enumv("phase_unit", ["degrees", "radians"]).default("degrees").describe(
                "Unit for the phase column. degrees (default) reports -180 to 180; radians reports -pi to pi. Bins whose magnitude is numerically zero report a phase of 0 rather than a meaningless rounding-noise angle.",
            ),
        )
        .param(
            Param::boolean("remove_dc").default(false).describe(
                "Subtract the mean of the samples before windowing and transforming (default false). Turn this on when a constant offset dominates bin 0 and hides the tones you care about; the removed offset is reported in the output header.",
            ),
        )
        .param(
            Param::integer("peaks").default(5).min(0.0).max(20.0).describe(
                "How many dominant peaks to list above the bin table, 0 to 20. Default 5. Peaks are local maxima of the spectrum ranked by magnitude, each reported with its bin index, frequency, level, and phase. Set 0 to omit the peak list.",
            ),
        )
        .param(
            Param::integer("decimals").default(4).min(0.0).max(12.0).describe(
                "Decimal places for every number in the output, 0 to 12. Default 4.",
            ),
        )
        .param(
            Param::enumv("format", ["text", "csv", "json", "chart"]).default("text").describe(
                "Output format. text (default) prints a header, the peak list, and the bin table. chart draws a Unicode bar chart of the spectrum by frequency (first 256 bins). csv emits the metadata, peaks, and a bin,frequency,level,phase,real,imaginary table. json emits the same data as a structured object. Default text.",
            ),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fft-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the FFT of a sample list and return the frequency spectrum",
    skill(
        description = "Compute the discrete Fourier transform of a pasted list of samples and return the frequency spectrum. Accepts real or complex samples separated by commas, semicolons, spaces, or newlines, in decimal or scientific notation. Supports a sample rate for a real frequency axis, six window functions with coherent-gain amplitude correction, zero-padding to the next power of two or an exact-length DFT, one-sided/two-sided views, amplitude/magnitude/normalized/dB/power scaling, phase in degrees or radians, optional DC removal, and a ranked dominant-peak list. Reports bin resolution and the Nyquist frequency, warns when a top peak sits in the Nyquist bin, and outputs text, a Unicode bar chart, CSV, or JSON with real and imaginary columns.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fft-analyzer", |a: Args| {
            gizza_ai_fft_analyzer_core::analyze(
                &a.data,
                a.sample_rate,
                &a.window,
                &a.pad,
                &a.spectrum,
                &a.scale,
                &a.phase_unit,
                a.remove_dc,
                a.peaks,
                a.decimals,
                &a.format,
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
                    "data": { "type": "string", "description": "The sample list to transform, in time order. Values may be separated by commas, semicolons, spaces, or newlines, and written as integers, decimals, or scientific notation (1, -2.5, 2.7e3). Complex samples are accepted in the forms 3+4i, -2i, or 1.25-0.5j (j is treated as i); any complex sample switches the default view to the two-sided spectrum. Minimum 2 samples, maximum 65536." },
                    "sample_rate": { "type": "number", "default": 1.0, "description": "Sampling rate of the input in samples per second (Hz). This only sets the frequency axis: bin k maps to k * sample_rate / transform_length, the bin resolution is sample_rate / transform_length, and Nyquist is sample_rate / 2. Default 1.0, which reports normalised frequency in cycles per sample. Use 48000 for 48 kHz audio." },
                    "window": { "type": "string", "enum": ["rectangular", "hann", "hamming", "blackman", "blackman-harris", "flattop"], "default": "rectangular", "description": "Window applied to the samples before the transform, to suppress spectral leakage from tones that do not land exactly on a bin. rectangular (default) applies no taper and gives the sharpest peaks for exactly-periodic input; hann and hamming are the general-purpose choices; blackman and blackman-harris trade a wider main lobe for much lower side lobes; flattop is the most accurate for measuring peak amplitude. Amplitudes are corrected for the window's coherent gain, so a windowed tone still reads its true amplitude." },
                    "pad": { "type": "string", "enum": ["pow2", "none"], "default": "pow2", "description": "Transform length. pow2 (default) zero-pads the input up to the next power of two and uses the radix-2 FFT. none transforms the samples at their exact length with a direct DFT, which is O(N^2) and therefore capped at 4096 samples for non-power-of-two lengths. Zero padding interpolates the spectrum onto a finer grid; it does not add real resolution." },
                    "spectrum": { "type": "string", "enum": ["auto", "one-sided", "two-sided"], "default": "auto", "description": "Which half of the spectrum to report. auto (default) shows the one-sided spectrum (bins 0 to N/2) for real input and the full two-sided spectrum for complex input. one-sided forces bins 0 to N/2 and doubles the amplitude of every bin except DC and Nyquist. two-sided forces all N bins and labels bins above N/2 as negative frequencies." },
                    "scale": { "type": "string", "enum": ["amplitude", "magnitude", "normalized", "db", "power"], "default": "amplitude", "description": "How the level column is scaled. amplitude (default) reports the amplitude of the underlying sinusoid, so a unit cosine reads 1.0. magnitude reports the raw unnormalised |X[k]|, which grows with the number of samples. normalized divides the magnitude by the sample count without the one-sided doubling. db is 20*log10 of the amplitude, so a unit tone reads 0 dB and an empty bin reads -inf. power is the amplitude squared." },
                    "phase_unit": { "type": "string", "enum": ["degrees", "radians"], "default": "degrees", "description": "Unit for the phase column. degrees (default) reports -180 to 180; radians reports -pi to pi. Bins whose magnitude is numerically zero report a phase of 0 rather than a meaningless rounding-noise angle." },
                    "remove_dc": { "type": "boolean", "default": false, "description": "Subtract the mean of the samples before windowing and transforming (default false). Turn this on when a constant offset dominates bin 0 and hides the tones you care about; the removed offset is reported in the output header." },
                    "peaks": { "type": "integer", "default": 5, "minimum": 0, "maximum": 20, "description": "How many dominant peaks to list above the bin table, 0 to 20. Default 5. Peaks are local maxima of the spectrum ranked by magnitude, each reported with its bin index, frequency, level, and phase. Set 0 to omit the peak list." },
                    "decimals": { "type": "integer", "default": 4, "minimum": 0, "maximum": 12, "description": "Decimal places for every number in the output, 0 to 12. Default 4." },
                    "format": { "type": "string", "enum": ["text", "csv", "json", "chart"], "default": "text", "description": "Output format. text (default) prints a header, the peak list, and the bin table. chart draws a Unicode bar chart of the spectrum by frequency (first 256 bins). csv emits the metadata, peaks, and a bin,frequency,level,phase,real,imaginary table. json emits the same data as a structured object. Default text." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
