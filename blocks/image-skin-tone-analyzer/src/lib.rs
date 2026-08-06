//! gizza-ai/image-skin-tone-analyzer — find the skin-tone pixels in a photo,
//! report their average hue/undertone/depth, and suggest the warm-cool
//! white-balance correction that would neutralise the cast.
//! Pure Rust image analysis; chat + CLI JSON report (no standalone page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_skin_tone_analyzer_core::{analyze, Analysis, Sensitivity};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_sensitivity")]
    sensitivity: String,
    #[serde(default = "d_reference_hue")]
    reference_hue: f64,
    #[serde(default = "d_tolerance")]
    tolerance: f64,
    #[serde(default = "d_min_skin_percent")]
    min_skin_percent: f64,
}

fn d_sensitivity() -> String {
    "normal".into()
}
fn d_reference_hue() -> f64 {
    52.0
}
fn d_tolerance() -> f64 {
    4.0
}
fn d_min_skin_percent() -> f64 {
    0.5
}

#[derive(Serialize)]
struct Resp {
    width: u32,
    height: u32,
    sampled_pixels: u64,
    skin_pixels: u64,
    skin_percent: f64,
    sensitivity: String,
    hex: String,
    rgb: String,
    hue_degrees: f64,
    saturation_percent: f64,
    brightness_percent: f64,
    lab_l: f64,
    lab_a: f64,
    lab_b: f64,
    lab_chroma: f64,
    lab_hue_degrees: f64,
    ita_degrees: f64,
    depth: String,
    undertone: String,
    reference_hue_degrees: f64,
    hue_offset_degrees: f64,
    cast: String,
    is_balanced: bool,
    assumed_capture_kelvin: f64,
    suggested_kelvin: f64,
    suggested_kelvin_shift: f64,
    kelvin_clamped: bool,
    clipped_percent: f64,
    confidence: f64,
    warnings: Vec<String>,
    note: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("sensitivity", ["strict", "normal", "loose"])
                .default("normal")
                .describe(
                    "How permissive the skin-pixel test is: strict adds an RGB rule and a high luma floor (fewest false positives), normal uses the classic YCbCr chroma band, loose widens the band for photos with a heavy colour cast or deep skin tones in shadow (default normal).",
                ),
        )
        .param(
            Param::number("reference_hue")
                .min(0.0)
                .max(90.0)
                .default(52.0)
                .describe(
                    "CIELAB a*b* hue angle, in degrees, that correctly white-balanced skin is expected to sit at; the cast and the suggested temperature are measured against it (default 52, typical facial skin is 45-60).",
                ),
        )
        .param(
            Param::number("tolerance")
                .min(0.0)
                .max(30.0)
                .default(4.0)
                .describe(
                    "Hue offset in degrees still counted as balanced; |hue_offset_degrees| at or below this reports cast=neutral and is_balanced=true (default 4, range 0-30).",
                ),
        )
        .param(
            Param::number("min_skin_percent")
                .min(0.0)
                .max(100.0)
                .default(0.5)
                .describe(
                    "Minimum share of opaque pixels that must read as skin before a result is returned; below it the tool errors instead of reporting from a tiny sample. Set 0 to always report (default 0.5).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageSkinToneAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-skin-tone-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Measure a photo's skin-tone hue and suggest a warm/cool white-balance correction",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Detect the skin-tone pixels in a photo and report their average hue, undertone and depth, plus the warm/cool white-balance correction that would neutralise any colour cast. Params: sensitivity=strict|normal|loose skin-mask width (default normal), reference_hue = the CIELAB a*b* hue angle balanced skin should sit at (0-90, default 52), tolerance in degrees still counted as balanced (0-30, default 4), min_skin_percent coverage gate (0-100, default 0.5, 0 disables). Returns JSON with hue_degrees (HSV hue of the mean skin colour) and hex, the CIELAB values plus lab_hue_degrees, undertone (cool under 45 deg, neutral 45-58, warm 58-68, olive 68+), depth from the Individual Typology Angle (very light/light/intermediate/tan/brown/dark), cast (warm|cool|neutral), hue_offset_degrees, is_balanced, and suggested_kelvin / suggested_kelvin_shift against an assumed 5500 K capture (negative = cool the image down). Also returns skin_percent coverage, clipped_percent, a confidence score and warnings. Analysis only: it never edits the image, and there is no face detection, so every skin-coloured pixel in the frame counts. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageSkinToneAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-skin-tone-analyzer")?;
    let sensitivity = Sensitivity::parse(&args.sensitivity).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let analysis = analyze(
        &bytes,
        sensitivity,
        args.reference_hue,
        args.tolerance,
        args.min_skin_percent,
    )
    .map_err(SkillError::InvalidArgs)?;
    let resp = response(analysis);
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize image-skin-tone-analyzer response: {e}"))
    })
}

/// The actionable one-liner: what to do with the numbers above.
fn note(a: &Analysis) -> String {
    let action = if a.is_balanced {
        format!(
            "Skin sits {:+.2}° from the {}° reference, within tolerance — no white-balance change needed.",
            a.hue_offset_degrees, a.reference_hue_degrees
        )
    } else {
        format!(
            "Skin reads {} by {:.2}° — set the white balance to about {:.0} K ({:+.0} K from the assumed {:.0} K) to neutralise it, or run auto-white-balance for a parameter-free fix.",
            a.cast,
            a.hue_offset_degrees.abs(),
            a.suggested_kelvin,
            a.suggested_kelvin_shift,
            a.assumed_capture_kelvin
        )
    };
    format!(
        "{action} The suggestion is a starting point, not a calibrated measurement: the capture temperature is assumed, so a subject's own undertone and a lighting cast are not fully separable from one rendered image."
    )
}

fn response(a: Analysis) -> Resp {
    let note = note(&a);
    Resp {
        width: a.width,
        height: a.height,
        sampled_pixels: a.sampled_pixels,
        skin_pixels: a.skin_pixels,
        skin_percent: a.skin_percent,
        sensitivity: a.sensitivity.to_string(),
        hex: a.hex,
        rgb: a.rgb,
        hue_degrees: a.hue_degrees,
        saturation_percent: a.saturation_percent,
        brightness_percent: a.brightness_percent,
        lab_l: a.lab_l,
        lab_a: a.lab_a,
        lab_b: a.lab_b,
        lab_chroma: a.lab_chroma,
        lab_hue_degrees: a.lab_hue_degrees,
        ita_degrees: a.ita_degrees,
        depth: a.depth.to_string(),
        undertone: a.undertone.to_string(),
        reference_hue_degrees: a.reference_hue_degrees,
        hue_offset_degrees: a.hue_offset_degrees,
        cast: a.cast.to_string(),
        is_balanced: a.is_balanced,
        assumed_capture_kelvin: a.assumed_capture_kelvin,
        suggested_kelvin: a.suggested_kelvin,
        suggested_kelvin_shift: a.suggested_kelvin_shift,
        kelvin_clamped: a.kelvin_clamped,
        clipped_percent: a.clipped_percent,
        confidence: a.confidence,
        warnings: a.warnings,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(cast: &'static str, balanced: bool) -> Analysis {
        Analysis {
            width: 800,
            height: 600,
            sampled_pixels: 480_000,
            skin_pixels: 96_000,
            skin_percent: 20.0,
            sensitivity: "normal",
            hex: "#cc8866".into(),
            rgb: "rgb(204, 136, 102)".into(),
            r: 204,
            g: 136,
            b: 102,
            hue_degrees: 20.0,
            saturation_percent: 50.0,
            brightness_percent: 80.0,
            lab_l: 62.85,
            lab_a: 22.25,
            lab_b: 28.83,
            lab_chroma: 36.42,
            lab_hue_degrees: 62.35,
            ita_degrees: 24.02,
            depth: "tan",
            undertone: "warm",
            reference_hue_degrees: 52.0,
            hue_offset_degrees: 10.35,
            cast,
            is_balanced: balanced,
            assumed_capture_kelvin: 5500.0,
            suggested_kelvin: 4700.0,
            suggested_kelvin_shift: -800.0,
            kelvin_clamped: false,
            clipped_percent: 0.0,
            confidence: 1.0,
            warnings: vec![],
        }
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(
            Sensitivity::parse(&d_sensitivity()).unwrap(),
            Sensitivity::Normal
        );
        assert_eq!(d_reference_hue(), 52.0);
        assert_eq!(d_tolerance(), 4.0);
        assert_eq!(d_min_skin_percent(), 0.5);
        assert!(Sensitivity::parse("aggressive").is_err());
    }

    #[test]
    fn args_parse_from_a_bare_url_using_every_default() {
        let a: Args =
            serde_json::from_str(r#"{"url":"https://example.com/portrait.jpg"}"#).unwrap();
        assert_eq!(a.sensitivity, "normal");
        assert_eq!(a.reference_hue, 52.0);
        assert_eq!(a.tolerance, 4.0);
        assert_eq!(a.min_skin_percent, 0.5);
    }

    #[test]
    fn an_unbalanced_note_names_the_temperature_and_the_follow_up_tool() {
        let resp = response(sample("warm", false));
        assert!(resp.note.contains("4700 K"), "{}", resp.note);
        assert!(resp.note.contains("-800 K"), "{}", resp.note);
        assert!(resp.note.contains("auto-white-balance"), "{}", resp.note);
        assert!(resp.note.contains("starting point"), "{}", resp.note);
        assert_eq!(resp.cast, "warm");
        assert_eq!(resp.undertone, "warm");
        assert_eq!(resp.hex, "#cc8866");
    }

    #[test]
    fn a_balanced_note_says_no_change_is_needed() {
        let mut a = sample("neutral", true);
        a.hue_offset_degrees = 0.35;
        let resp = response(a);
        assert!(
            resp.note.contains("no white-balance change needed"),
            "{}",
            resp.note
        );
        assert!(resp.is_balanced);
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "sensitivity": { "type": "string", "enum": ["strict", "normal", "loose"], "default": "normal", "description": "How permissive the skin-pixel test is: strict adds an RGB rule and a high luma floor (fewest false positives), normal uses the classic YCbCr chroma band, loose widens the band for photos with a heavy colour cast or deep skin tones in shadow (default normal)." },
                    "reference_hue": { "type": "number", "minimum": 0, "maximum": 90, "default": 52.0, "description": "CIELAB a*b* hue angle, in degrees, that correctly white-balanced skin is expected to sit at; the cast and the suggested temperature are measured against it (default 52, typical facial skin is 45-60)." },
                    "tolerance": { "type": "number", "minimum": 0, "maximum": 30, "default": 4.0, "description": "Hue offset in degrees still counted as balanced; |hue_offset_degrees| at or below this reports cast=neutral and is_balanced=true (default 4, range 0-30)." },
                    "min_skin_percent": { "type": "number", "minimum": 0, "maximum": 100, "default": 0.5, "description": "Minimum share of opaque pixels that must read as skin before a result is returned; below it the tool errors instead of reporting from a tiny sample. Set 0 to always report (default 0.5)." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
