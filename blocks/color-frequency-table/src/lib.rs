//! gizza-ai/color-frequency-table — the exact-colour census of an image: which
//! RGBA values are actually present, how many pixels each covers, and what share
//! of the frame that is, plus the unique-colour count and the tail the table
//! doesn't list.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::analyze` (decode + exact
//! per-colour counting) → JSON the LLM reads directly, carrying a ready-made
//! aligned `table` and a `csv` so the whole report is one copy-paste, and a
//! one-line `note` so a batch check reads without post-processing.
//!
//! Distinct from its neighbours: `color-palette-extract` reports palette
//! centroids that need not occur in the image, `image-histogram-analyzer`
//! reports per-channel marginals that can never name a combined RGB triple, and
//! `image-average-color` / `background-color-detector` answer with one colour.
//! This block counts the colours that are literally there.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report — the no-page
//! file-input pattern, like image-histogram-analyzer / image-average-color /
//! background-color-detector; the generator's pure-tool page can't hand uploaded
//! bytes to a wasm decoder).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_color_frequency_table_core::{
    analyze, render_csv, render_table, Analysis, ColorFormat, Options, Sort, MAX_QUANTIZE, MAX_TOP,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_top")]
    top: f64,
    #[serde(default = "d_quantize")]
    quantize: f64,
    #[serde(default = "d_min_percent")]
    min_percent: f64,
    #[serde(default = "d_ignore_transparency")]
    ignore_transparency: bool,
    #[serde(default = "d_sort")]
    sort: String,
    #[serde(default = "d_color_format")]
    color_format: String,
}

fn d_top() -> f64 {
    10.0
}
fn d_quantize() -> f64 {
    1.0
}
fn d_min_percent() -> f64 {
    0.0
}
fn d_ignore_transparency() -> bool {
    true
}
fn d_sort() -> String {
    "frequency".into()
}
fn d_color_format() -> String {
    "hex".into()
}

/// Whole-number guard shared by the two integer params — wasm hands numbers over
/// as f64, so "10.5 colours" has to be caught here rather than by the type.
fn whole(name: &str, v: f64, lo: u32, hi: u32) -> Result<u32, String> {
    if !v.is_finite() || v.fract() != 0.0 {
        return Err(format!(
            "{name} must be a whole number between {lo} and {hi} (got {v})"
        ));
    }
    if v < f64::from(lo) || v > f64::from(hi) {
        return Err(format!("{name} must be between {lo} and {hi} (got {v})"));
    }
    Ok(v as u32)
}

fn to_options(args: &Args) -> Result<Options, String> {
    if !args.min_percent.is_finite() || !(0.0..=100.0).contains(&args.min_percent) {
        return Err(format!(
            "min_percent must be between 0 and 100 (got {})",
            args.min_percent
        ));
    }
    Ok(Options {
        top: whole("top", args.top, 1, MAX_TOP)?,
        quantize: whole("quantize", args.quantize, 1, MAX_QUANTIZE)?,
        min_percent: args.min_percent,
        ignore_transparency: args.ignore_transparency,
        sort: Sort::parse(&args.sort)?,
        color_format: ColorFormat::parse(&args.color_format)?,
    })
}

/// The one-line summary a batch check prints per image: the census headline
/// first, the evidence behind it second.
fn note(a: &Analysis) -> String {
    if a.counted_pixels == 0 {
        return format!(
            "NO VISIBLE PIXELS — {}x{}, all {} pixels are transparent.",
            a.width, a.height, a.total_pixels
        );
    }
    let coverage = if a.remaining_colors == 0 {
        format!("the {} listed cover every pixel", a.listed_colors)
    } else {
        format!(
            "the {} listed cover {:.2}%, {} more colours hold the rest",
            a.listed_colors, a.listed_percent, a.remaining_colors
        )
    };
    format!(
        "{} unique colours in {}x{} ({} pixels{}) — most common {} ({}) at {:.2}%; {coverage}.",
        a.unique_colors,
        a.width,
        a.height,
        a.total_pixels,
        if a.sampled { ", sampled" } else { "" },
        a.dominant_hex,
        a.dominant_name,
        a.dominant_percent,
    )
}

/// Serialise the census and attach the copy-paste renderings.
fn response(a: &Analysis, format: ColorFormat, filename: Option<String>) -> Result<Value, String> {
    let note = note(a);
    let mut v = serde_json::to_value(a).map_err(|e| format!("serialize analysis: {e}"))?;
    let obj: &mut Map<String, Value> = v
        .as_object_mut()
        .ok_or_else(|| "analysis did not serialize to an object".to_string())?;
    obj.insert("table".into(), Value::from(render_table(a, format)));
    obj.insert("csv".into(), Value::from(render_csv(a, format)));
    obj.insert("note".into(), Value::from(note));
    if let Some(f) = filename {
        obj.insert("filename".into(), Value::from(f));
    }
    Ok(v)
}

/// Single source for the chat schema (and the CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("top")
                .min(1.0)
                .max(f64::from(MAX_TOP))
                .default(10)
                .describe(
                    "How many colours to list, 1-256, most common first (default 10). The census \
                     itself always covers every colour: whatever the table leaves out is reported \
                     as unique_colors, remaining_colors and remaining_percent, so the tail is \
                     never silently dropped. Use 256 for a full inventory of a logo or icon.",
                ),
        )
        .param(
            Param::integer("quantize")
                .min(1.0)
                .max(f64::from(MAX_QUANTIZE))
                .default(1)
                .describe(
                    "Bucket width in levels per channel, 1-64. 1 (default) counts EXACT colours — \
                     a 4-colour logo reports exactly those 4. Larger values group near-identical \
                     shades and report each bucket's mean colour instead, so a JPEG-noisy sky \
                     collapses into one row rather than thousands of near-duplicates: try 8 or 16 \
                     for photographs. Grouped rows are averages, so the hex need not occur \
                     literally in the image.",
                ),
        )
        .param(
            Param::number("min_percent")
                .min(0.0)
                .max(100.0)
                .default(0.0)
                .describe(
                    "Drop colours covering less than this percent of the counted pixels, 0-100 \
                     (default 0 = keep everything). Applied before top, so min_percent=1 with \
                     top=256 lists every colour worth at least 1% and nothing else.",
                ),
        )
        .param(
            Param::boolean("ignore_transparency")
                .default(true)
                .describe(
                    "When true (default), pixels with alpha under 16 are counted separately \
                     (transparent_pixels / transparent_percent) and kept out of the census, so the \
                     junk RGB some encoders leave under alpha=0 cannot invent a colour nothing \
                     visible uses; set false to census those stored RGB values too. Alpha is part \
                     of the colour identity either way — every row carries hex_rgba.",
                ),
        )
        .param(
            Param::enumv("sort", ["frequency", "luminance", "hue"])
                .default("frequency")
                .describe(
                    "Order the listed rows are presented in. frequency (default) = most pixels \
                     first; luminance = darkest first, a tone ramp; hue = around the colour wheel \
                     from red, with greys leading. The top-N selection is ALWAYS by frequency, so \
                     \"top 10\" keeps meaning the 10 most common colours; each row's rank field \
                     stays its frequency rank.",
                ),
        )
        .param(
            Param::enumv("color_format", ["hex", "rgb", "rgba", "hsl"])
                .default("hex")
                .describe(
                    "Which notation fills the colour column of the rendered table and csv: hex \
                     #rrggbb (default), rgb(r, g, b), rgba(r, g, b, a) with alpha 0-1, or \
                     hsl(h, s%, l%). Nothing is lost by choosing — every JSON row always carries \
                     all four notations plus hex_rgba and the raw r/g/b/a numbers.",
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
    name = "gizza-ai/color-frequency-table",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Counts the exact pixel colors in an image and lists the top ones with counts and percentage coverage.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Count the colours in an image exactly: the top-N pixel colours with their pixel counts and percentage coverage, plus how many distinct colours the image holds in total. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). Unlike a palette extractor, the reported colours are the values literally present — a 4-colour logo reports exactly those 4. Every row carries rank, hex, hex_rgba, rgb(), rgba(), hsl(), the raw r/g/b/a, a plain-English colour name, the pixel count, the percentage and the Rec. 601 luminance. Around them: width, height, total_pixels, megapixels, unique_colors, grayscale_unique_colors, opaque_unique_colors, translucent_unique_colors, transparent_pixels/percent, the dominant colour, and listed_percent / remaining_colors / remaining_percent so the untabled tail is never silently dropped. The answer also carries a ready-made aligned table and a csv string for one-shot copy-paste. Parameters: top 1-256 rows (default 10), quantize 1-64 levels per channel to group near-identical shades and report each bucket's mean (default 1 = exact), min_percent 0-100 minimum share (default 0), ignore_transparency boolean (default true), sort frequency (default) | luminance | hue for presentation order only, color_format hex (default) | rgb | rgba | hsl for the table and csv column. PNG, JPEG, WebP, GIF, BMP and TIFF are supported. Images above 4 megapixels are read from a stride sample, flagged with sampled/stride and a warning. This is an analyser only; it never modifies the image.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("color-frequency-table")?;
    let opts = to_options(&args).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let analysis = analyze(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let resp = response(
        &analysis,
        opts.color_format,
        (!filename.is_empty()).then_some(filename),
    )
    .map_err(SkillError::Serialize)?;
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize color-frequency-table response: {e}")))
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

    /// 4x4: 8 red, 4 green, 4 blue.
    fn analysis() -> Analysis {
        let bytes = png(4, 4, |_, y| match y {
            0 | 1 => [255, 0, 0, 255],
            2 => [0, 128, 0, 255],
            _ => [0, 0, 255, 255],
        });
        analyze(&bytes, &Options::default()).unwrap()
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(d_top(), 10.0);
        assert_eq!(d_quantize(), 1.0);
        assert_eq!(d_min_percent(), 0.0);
        assert!(d_ignore_transparency());
        assert_eq!(d_sort(), "frequency");
        assert_eq!(d_color_format(), "hex");
    }

    #[test]
    fn args_parse_from_a_bare_url_using_defaults() {
        let a: Args = serde_json::from_str(r#"{"url":"https://example.com/logo.png"}"#).unwrap();
        let o = to_options(&a).unwrap();
        assert_eq!(o.top, 10);
        assert_eq!(o.quantize, 1);
        assert_eq!(o.min_percent, 0.0);
        assert!(o.ignore_transparency);
        assert_eq!(o.sort, Sort::Frequency);
        assert_eq!(o.color_format, ColorFormat::Hex);
    }

    #[test]
    fn bad_options_are_rejected_with_the_expected_range() {
        let bad: Args = serde_json::from_str(r#"{"url":"https://e.com/a.png","top":0}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("between 1 and 256"));
        let bad: Args = serde_json::from_str(r#"{"url":"https://e.com/a.png","top":10.5}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("whole number"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","quantize":65}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("between 1 and 64"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","min_percent":101}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("0 and 100"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","sort":"size"}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("frequency"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","color_format":"lab"}"#).unwrap();
        assert!(to_options(&bad).unwrap_err().contains("hex"));
    }

    #[test]
    fn note_leads_with_the_census_headline() {
        let n = note(&analysis());
        assert!(n.starts_with("3 unique colours in 4x4 (16 pixels)"), "{n}");
        assert!(n.contains("most common #ff0000 (red) at 50.00%"), "{n}");
        assert!(n.contains("the 3 listed cover every pixel"), "{n}");
    }

    #[test]
    fn note_reports_the_untabled_tail_when_the_table_is_truncated() {
        let bytes = png(4, 4, |_, y| match y {
            0 | 1 => [255, 0, 0, 255],
            2 => [0, 128, 0, 255],
            _ => [0, 0, 255, 255],
        });
        let a = analyze(
            &bytes,
            &Options {
                top: 1,
                ..Options::default()
            },
        )
        .unwrap();
        let n = note(&a);
        assert!(
            n.contains("the 1 listed cover 50.00%, 2 more colours hold the rest"),
            "{n}"
        );
    }

    #[test]
    fn response_carries_the_census_plus_the_table_and_csv() {
        let v = response(&analysis(), ColorFormat::Hex, None).unwrap();
        let o = v.as_object().unwrap();
        assert_eq!(o["unique_colors"], 3);
        assert_eq!(o["total_pixels"], 16);
        assert_eq!(o["counted_pixels"], 16);
        assert_eq!(o["dominant_hex"], "#ff0000");
        assert_eq!(o["colors"][0]["hex"], "#ff0000");
        assert_eq!(o["colors"][0]["count"], 8);
        assert_eq!(o["colors"][0]["percent"], 50.0);
        assert_eq!(o["colors"][0]["color_name"], "red");
        assert_eq!(o["sampled"], false);
        assert_eq!(o["remaining_colors"], 0);

        let table = o["table"].as_str().unwrap();
        assert!(table.starts_with("#  COLOR    PIXELS    SHARE  NAME\n"), "{table}");
        assert!(table.contains("1  #ff0000       8   50.00%  red\n"), "{table}");
        let csv = o["csv"].as_str().unwrap();
        assert!(csv.starts_with("rank,color,pixels,percent,name\n"));
        assert!(csv.contains("1,\"#ff0000\",8,50,red\n"));
        assert!(o["note"].as_str().unwrap().starts_with("3 unique colours"));
        assert!(o.get("filename").is_none());
    }

    #[test]
    fn the_rendered_column_follows_color_format() {
        let v = response(&analysis(), ColorFormat::Hsl, None).unwrap();
        assert!(v["table"].as_str().unwrap().contains("hsl(0, 100%, 50%)"));
        assert!(v["csv"].as_str().unwrap().contains("\"hsl(0, 100%, 50%)\""));
        // The JSON rows keep every notation whatever the column shows.
        assert_eq!(v["colors"][0]["hex"], "#ff0000");
        assert_eq!(v["colors"][0]["rgb"], "rgb(255, 0, 0)");
    }

    #[test]
    fn filename_is_passed_through_when_known() {
        let v = response(&analysis(), ColorFormat::Hex, Some("logo.png".into())).unwrap();
        assert_eq!(v["filename"], "logo.png");
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "top": { "type": "integer", "minimum": 1, "maximum": 256, "default": 10, "description": "How many colours to list, 1-256, most common first (default 10). The census itself always covers every colour: whatever the table leaves out is reported as unique_colors, remaining_colors and remaining_percent, so the tail is never silently dropped. Use 256 for a full inventory of a logo or icon." },
                    "quantize": { "type": "integer", "minimum": 1, "maximum": 64, "default": 1, "description": "Bucket width in levels per channel, 1-64. 1 (default) counts EXACT colours — a 4-colour logo reports exactly those 4. Larger values group near-identical shades and report each bucket's mean colour instead, so a JPEG-noisy sky collapses into one row rather than thousands of near-duplicates: try 8 or 16 for photographs. Grouped rows are averages, so the hex need not occur literally in the image." },
                    "min_percent": { "type": "number", "minimum": 0, "maximum": 100, "default": 0.0, "description": "Drop colours covering less than this percent of the counted pixels, 0-100 (default 0 = keep everything). Applied before top, so min_percent=1 with top=256 lists every colour worth at least 1% and nothing else." },
                    "ignore_transparency": { "type": "boolean", "default": true, "description": "When true (default), pixels with alpha under 16 are counted separately (transparent_pixels / transparent_percent) and kept out of the census, so the junk RGB some encoders leave under alpha=0 cannot invent a colour nothing visible uses; set false to census those stored RGB values too. Alpha is part of the colour identity either way — every row carries hex_rgba." },
                    "sort": { "type": "string", "enum": ["frequency", "luminance", "hue"], "default": "frequency", "description": "Order the listed rows are presented in. frequency (default) = most pixels first; luminance = darkest first, a tone ramp; hue = around the colour wheel from red, with greys leading. The top-N selection is ALWAYS by frequency, so \"top 10\" keeps meaning the 10 most common colours; each row's rank field stays its frequency rank." },
                    "color_format": { "type": "string", "enum": ["hex", "rgb", "rgba", "hsl"], "default": "hex", "description": "Which notation fills the colour column of the rendered table and csv: hex #rrggbb (default), rgb(r, g, b), rgba(r, g, b, a) with alpha 0-1, or hsl(h, s%, l%). Nothing is lost by choosing — every JSON row always carries all four notations plus hex_rgba and the raw r/g/b/a numbers." }
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
