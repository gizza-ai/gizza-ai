//! gizza-ai/image-histogram-analyzer — per-channel RGB + luminance histograms
//! and the exposure report that comes with them: clipping at both ends, dynamic
//! range in stops, contrast, colour cast and the tonal split.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::analyze` (decode +
//! exact 256-level counting) → JSON the LLM reads directly, with a one-line
//! `note` so a batch check reads without post-processing. `output` decides how
//! much of the histogram travels with the stats: `summary` (stats only),
//! `full` (the binned arrays) or `csv` (a spreadsheet-ready table).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report — the no-page
//! file-input pattern, like image-blank-detector / image-info /
//! image-average-color; the generator's pure-tool page can't hand uploaded bytes
//! to a wasm decoder).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_image_histogram_analyzer_core::{analyze, histogram_csv, Analysis, Luma, Options};
use serde::Deserialize;
use serde_json::{Map, Value};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_bins")]
    bins: f64,
    #[serde(default = "d_luma")]
    luma: String,
    #[serde(default = "d_clip_margin")]
    clip_margin: f64,
    #[serde(default = "d_clip_percent")]
    clip_percent: f64,
    #[serde(default = "d_ignore_transparent")]
    ignore_transparent: bool,
    #[serde(default = "d_output")]
    output: String,
}

fn d_bins() -> f64 {
    256.0
}
fn d_luma() -> String {
    "rec601".into()
}
fn d_clip_margin() -> f64 {
    0.0
}
fn d_clip_percent() -> f64 {
    0.5
}
fn d_ignore_transparent() -> bool {
    true
}
fn d_output() -> String {
    "summary".into()
}

/// How much of the histogram travels with the statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    /// Statistics + verdicts only — the compact answer for a chat reply.
    Summary,
    /// Statistics + the binned per-channel arrays, for charting.
    Full,
    /// Statistics + the same bins as a CSV table, for a spreadsheet.
    Csv,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "summary" | "stats" => Ok(Output::Summary),
            "full" | "json" => Ok(Output::Full),
            "csv" => Ok(Output::Csv),
            other => Err(format!(
                "output must be one of summary, full, csv (got \"{other}\")"
            )),
        }
    }
}

fn to_options(args: &Args) -> Result<Options, String> {
    if !args.bins.is_finite() || args.bins.fract() != 0.0 {
        return Err(format!(
            "bins must be a whole number between 2 and 256 (got {})",
            args.bins
        ));
    }
    if !(2.0..=256.0).contains(&args.bins) {
        return Err(format!(
            "bins must be between 2 and 256 (got {}) — 256 keeps every level, 64 or 32 make a \
             chart-sized summary",
            args.bins
        ));
    }
    if !args.clip_margin.is_finite() || args.clip_margin.fract() != 0.0 {
        return Err(format!(
            "clip_margin must be a whole number of levels between 0 and 32 (got {})",
            args.clip_margin
        ));
    }
    if !(0.0..=32.0).contains(&args.clip_margin) {
        return Err(format!(
            "clip_margin must be between 0 and 32 levels (got {})",
            args.clip_margin
        ));
    }
    Ok(Options {
        bins: args.bins as u32,
        luma: Luma::parse(&args.luma)?,
        clip_margin: args.clip_margin as u8,
        clip_percent: args.clip_percent,
        ignore_transparent: args.ignore_transparent,
    })
}

/// The one-line summary a batch check prints per image: the verdict first, the
/// numbers that decided it second.
fn note(a: &Analysis) -> String {
    let clip = match (a.shadow_clipped, a.highlight_clipped) {
        (true, true) => "both ends clipped".to_string(),
        (true, false) => format!("shadows clipped ({:.2}%)", a.luma.clipped_shadow_percent),
        (false, true) => format!(
            "highlights clipped ({:.2}%)",
            a.luma.clipped_highlight_percent
        ),
        (false, false) => "no clipping".to_string(),
    };
    format!(
        "{} — {}x{}, mean luma {:.1}, {} contrast, dynamic range {} levels ({:.1} stops), {clip}, \
         {} cast.",
        a.exposure.to_uppercase(),
        a.width,
        a.height,
        a.luma.mean,
        a.contrast,
        a.dynamic_range_levels,
        a.dynamic_range_stops,
        a.color_cast
    )
}

/// Shape the response for the requested `output` mode. The statistics are
/// identical in every mode — only the histogram payload changes.
fn response(a: &Analysis, output: Output, filename: Option<String>) -> Result<Value, String> {
    let note = note(a);
    let mut v = serde_json::to_value(a).map_err(|e| format!("serialize analysis: {e}"))?;
    let obj: &mut Map<String, Value> = v
        .as_object_mut()
        .ok_or_else(|| "analysis did not serialize to an object".to_string())?;
    obj.insert("bins".into(), Value::from(a.histogram.bins));
    match output {
        Output::Summary => {
            obj.remove("histogram");
        }
        Output::Full => {}
        Output::Csv => {
            obj.remove("histogram");
            obj.insert("csv".into(), Value::from(histogram_csv(a)));
        }
    }
    obj.insert("output".into(), Value::from(output_name(output)));
    obj.insert("note".into(), Value::from(note));
    if let Some(f) = filename {
        obj.insert("filename".into(), Value::from(f));
    }
    Ok(v)
}

fn output_name(o: Output) -> &'static str {
    match o {
        Output::Summary => "summary",
        Output::Full => "full",
        Output::Csv => "csv",
    }
}

/// Single source for the chat schema (and the CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("bins")
                .min(2.0)
                .max(256.0)
                .default(256)
                .describe(
                    "How many buckets the REPORTED histogram is folded into, 2-256. Every \
                     statistic is always computed at full 256-level precision, so this only \
                     changes the shape of the returned arrays: 256 keeps one bucket per level, 64 \
                     or 32 give a chart-sized summary (default 256).",
                ),
        )
        .param(
            Param::enumv("luma", ["rec601", "rec709", "average", "max"])
                .default("rec601")
                .describe(
                    "Which brightness formula builds the luminance histogram. rec601 = 0.299R + \
                     0.587G + 0.114B, what camera and editor histograms show (default); rec709 = \
                     0.2126R + 0.7152G + 0.0722B, the sRGB/HD coefficients; average = (R+G+B)/3; \
                     max = max(R,G,B), the strictest highlight-clipping view because a single \
                     blown channel shows up immediately.",
                ),
        )
        .param(
            Param::integer("clip_margin")
                .min(0.0)
                .max(32.0)
                .default(0)
                .describe(
                    "How many levels in from each end still count as clipped, 0-32. 0 (default) \
                     counts only pure 0 and pure 255; 2 also catches near-black and near-white \
                     pixels that a JPEG re-save nudged off the exact end.",
                ),
        )
        .param(
            Param::number("clip_percent")
                .min(0.0)
                .max(100.0)
                .default(0.5)
                .describe(
                    "Percent of pixels that must be pinned at an end before shadow_clipped / \
                     highlight_clipped is flagged, 0-100. 0 flags a single pinned pixel; the \
                     default 0.5 ignores the specular dots and deep-shadow noise every real photo \
                     has (default 0.5).",
                ),
        )
        .param(
            Param::boolean("ignore_transparent")
                .default(true)
                .describe(
                    "When true (default), fully transparent pixels are counted separately and \
                     kept out of the histogram, so the junk RGB some encoders leave under alpha=0 \
                     cannot fake a spike at level 0; set false to measure the stored RGB values as \
                     they are.",
                ),
        )
        .param(
            Param::enumv("output", ["summary", "full", "csv"])
                .default("summary")
                .describe(
                    "How much histogram data comes back with the statistics. summary (default) = \
                     statistics and verdicts only; full = also the per-channel bin counts as \
                     arrays, ready to chart; csv = also a bin,level_start,level_end,red,green,\
                     blue,luma table as text for a spreadsheet.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-histogram-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Per-channel RGB and luminance histograms with clipping, dynamic range and exposure stats.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Measure an image's tones: per-channel red, green, blue and luminance histograms plus the exposure report a photographer would read off them. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). Every pixel is counted at full 256-level precision — nothing is sub-sampled — and the answer carries its evidence: per channel min, max, mean, median, standard deviation, the 1st/5th/95th/99th percentiles, the modal level and its share, distinct levels used, and the pixels pinned at each end as counts and percentages. On top of that: shadow_clipped / highlight_clipped verdicts, dynamic range as levels and photographic stops (99th minus 1st percentile of luma), luma entropy, the shadow/midtone/highlight split, an exposure verdict (underexposed, balanced, overexposed), a contrast verdict (low, normal, high), a colour cast reading (neutral, warm, cool, green, magenta) with the level gap behind it, and a one-line note. Parameters: bins 2-256 for the reported histogram shape (default 256, statistics are unaffected), luma rec601 (default) | rec709 | average | max, clip_margin 0-32 levels in from each end (default 0), clip_percent 0-100 pixels pinned before clipping is flagged (default 0.5), ignore_transparent boolean (default true), output summary (default) | full for the binned arrays | csv for a spreadsheet table. PNG, JPEG, WebP, GIF, BMP and TIFF are supported. This is an analyser only; it never modifies the image.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-histogram-analyzer")?;
    let opts = to_options(&args).map_err(SkillError::InvalidArgs)?;
    let output = Output::parse(&args.output).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let analysis = analyze(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let resp = response(&analysis, output, (!filename.is_empty()).then_some(filename))
        .map_err(SkillError::Serialize)?;
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize image-histogram-analyzer response: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba(f(x, y));
        }
        let mut buf = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn analysis() -> Analysis {
        // Left half black, right half white: clipped at both ends.
        let bytes = png(16, 16, |x, _| {
            if x < 8 {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        });
        analyze(&bytes, &Options::default()).unwrap()
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(d_bins(), 256.0);
        assert_eq!(d_luma(), "rec601");
        assert_eq!(d_clip_margin(), 0.0);
        assert_eq!(d_clip_percent(), 0.5);
        assert!(d_ignore_transparent());
        assert_eq!(d_output(), "summary");
    }

    #[test]
    fn args_parse_from_a_bare_url_using_defaults() {
        let a: Args = serde_json::from_str(r#"{"url":"https://example.com/photo.jpg"}"#).unwrap();
        let o = to_options(&a).unwrap();
        assert_eq!(o.bins, 256);
        assert_eq!(o.luma, Luma::Rec601);
        assert_eq!(o.clip_margin, 0);
        assert_eq!(o.clip_percent, 0.5);
        assert!(o.ignore_transparent);
        assert_eq!(Output::parse(&a.output).unwrap(), Output::Summary);
    }

    #[test]
    fn bad_options_are_rejected_with_the_expected_range() {
        let bad: Args = serde_json::from_str(r#"{"url":"https://e.com/a.png","bins":1}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("between 2 and 256"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","bins":33.5}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("whole number"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","clip_margin":64}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("0 and 32"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","luma":"hsl"}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("rec601"));
        assert!(Output::parse("chart").unwrap_err().contains("summary"));
    }

    #[test]
    fn note_leads_with_the_exposure_verdict_and_the_clipping() {
        let n = note(&analysis());
        assert!(n.starts_with("BALANCED — 16x16"), "{n}");
        assert!(n.contains("mean luma 127.5"), "{n}");
        assert!(n.contains("high contrast"), "{n}");
        assert!(n.contains("both ends clipped"), "{n}");
        assert!(n.contains("neutral cast"), "{n}");
    }

    #[test]
    fn summary_omits_the_histogram_arrays() {
        let v = response(&analysis(), Output::Summary, None).unwrap();
        let o = v.as_object().unwrap();
        assert!(o.get("histogram").is_none(), "summary must stay compact");
        assert!(o.get("csv").is_none());
        assert_eq!(o["bins"], 256);
        assert_eq!(o["output"], "summary");
        assert_eq!(o["exposure"], "balanced");
        assert_eq!(o["luma"]["clipped_highlight_percent"], 50.0);
        assert_eq!(o["red"]["mean"], 127.5);
        assert!(o["note"].as_str().unwrap().starts_with("BALANCED"));
        assert!(o.get("filename").is_none());
    }

    #[test]
    fn full_carries_the_binned_arrays_and_csv_carries_the_table() {
        let a = analyze(
            &png(16, 16, |x, _| {
                if x < 8 {
                    [0, 0, 0, 255]
                } else {
                    [255, 255, 255, 255]
                }
            }),
            &Options {
                bins: 4,
                ..Options::default()
            },
        )
        .unwrap();

        let full = response(&a, Output::Full, None).unwrap();
        let h = &full["histogram"];
        assert_eq!(h["bins"], 4);
        assert_eq!(h["luma"].as_array().unwrap().len(), 4);
        assert_eq!(h["luma"][0], 128);
        assert_eq!(h["luma"][3], 128);
        assert_eq!(full["bins"], 4);

        let csv = response(&a, Output::Csv, None).unwrap();
        assert!(csv.get("histogram").is_none());
        let text = csv["csv"].as_str().unwrap();
        assert!(text.starts_with("bin,level_start,level_end,red,green,blue,luma\n"));
        assert!(text.contains("\n0,0,63,128,128,128,128\n"));
    }

    #[test]
    fn filename_is_passed_through_when_known() {
        let v = response(&analysis(), Output::Summary, Some("sunset.jpg".into())).unwrap();
        assert_eq!(v["filename"], "sunset.jpg");
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "bins": { "type": "integer", "minimum": 2, "maximum": 256, "default": 256, "description": "How many buckets the REPORTED histogram is folded into, 2-256. Every statistic is always computed at full 256-level precision, so this only changes the shape of the returned arrays: 256 keeps one bucket per level, 64 or 32 give a chart-sized summary (default 256)." },
                    "luma": { "type": "string", "enum": ["rec601", "rec709", "average", "max"], "default": "rec601", "description": "Which brightness formula builds the luminance histogram. rec601 = 0.299R + 0.587G + 0.114B, what camera and editor histograms show (default); rec709 = 0.2126R + 0.7152G + 0.0722B, the sRGB/HD coefficients; average = (R+G+B)/3; max = max(R,G,B), the strictest highlight-clipping view because a single blown channel shows up immediately." },
                    "clip_margin": { "type": "integer", "minimum": 0, "maximum": 32, "default": 0, "description": "How many levels in from each end still count as clipped, 0-32. 0 (default) counts only pure 0 and pure 255; 2 also catches near-black and near-white pixels that a JPEG re-save nudged off the exact end." },
                    "clip_percent": { "type": "number", "minimum": 0, "maximum": 100, "default": 0.5, "description": "Percent of pixels that must be pinned at an end before shadow_clipped / highlight_clipped is flagged, 0-100. 0 flags a single pinned pixel; the default 0.5 ignores the specular dots and deep-shadow noise every real photo has (default 0.5)." },
                    "ignore_transparent": { "type": "boolean", "default": true, "description": "When true (default), fully transparent pixels are counted separately and kept out of the histogram, so the junk RGB some encoders leave under alpha=0 cannot fake a spike at level 0; set false to measure the stored RGB values as they are." },
                    "output": { "type": "string", "enum": ["summary", "full", "csv"], "default": "summary", "description": "How much histogram data comes back with the statistics. summary (default) = statistics and verdicts only; full = also the per-channel bin counts as arrays, ready to chart; csv = also a bin,level_start,level_end,red,green,blue,luma table as text for a spreadsheet." }
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
