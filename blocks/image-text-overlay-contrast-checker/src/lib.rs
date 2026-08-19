//! gizza-ai/image-text-overlay-contrast-checker — find the worst place on a
//! photo to drop overlaid text of a given colour, and say what would fix it.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::analyze` (decode →
//! linear-light box-downscale → summed-area table → one WCAG contrast ratio per
//! text-shaped window) → JSON an LLM can answer "where would this caption go
//! blind?" from directly, with a one-line `note` for batch checks. `output`
//! decides how much of the window grid travels with the verdict: `summary`
//! (verdict + worst/best + placements), `full` (also the ratio grid) or `csv`
//! (also the grid as a spreadsheet table).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report — the no-page
//! file-input pattern shared with image-histogram-analyzer / image-average-color
//! / background-color-detector; the generator's pure-tool page cannot hand
//! uploaded bytes to a wasm decoder).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_text_overlay_contrast_checker_core::{
    analyze, grid_csv, AlphaBackground, Analysis, Level, Options, Region, TextSize,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_text_color")]
    text_color: String,
    #[serde(default = "d_level")]
    level: String,
    #[serde(default = "d_text_size")]
    text_size: String,
    #[serde(default = "d_region")]
    region: String,
    #[serde(default = "d_window_width")]
    window_width: f64,
    #[serde(default = "d_window_height")]
    window_height: f64,
    #[serde(default = "d_alpha_background")]
    alpha_background: String,
    #[serde(default = "d_output")]
    output: String,
}

fn d_text_color() -> String {
    "#ffffff".into()
}
fn d_level() -> String {
    "aa".into()
}
fn d_text_size() -> String {
    "normal".into()
}
fn d_region() -> String {
    "full".into()
}
fn d_window_width() -> f64 {
    30.0
}
fn d_window_height() -> f64 {
    10.0
}
fn d_alpha_background() -> String {
    "white".into()
}
fn d_output() -> String {
    "summary".into()
}

/// How much of the per-window grid travels with the verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    /// Verdict, worst/best window and the placement ranking — the compact answer.
    Summary,
    /// Also every window's contrast ratio, as rows of numbers ready to chart.
    Full,
    /// Also the same grid as a row,column,contrast_ratio,passes table.
    Csv,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "summary" | "stats" => Ok(Output::Summary),
            "full" | "grid" | "json" => Ok(Output::Full),
            "csv" => Ok(Output::Csv),
            other => Err(format!(
                "output must be one of summary, full, csv (got \"{other}\")"
            )),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Output::Summary => "summary",
            Output::Full => "full",
            Output::Csv => "csv",
        }
    }
    fn wants_grid(self) -> bool {
        !matches!(self, Output::Summary)
    }
}

fn to_options(args: &Args, output: Output) -> Result<Options, String> {
    Ok(Options {
        text_color: gizza_ai_image_text_overlay_contrast_checker_core::parse_color(
            &args.text_color,
        )?,
        level: Level::parse(&args.level)?,
        text_size: TextSize::parse(&args.text_size)?,
        region: Region::parse(&args.region)?,
        window_width: args.window_width,
        window_height: args.window_height,
        alpha_background: AlphaBackground::parse(&args.alpha_background)?,
        want_grid: output.wants_grid(),
    })
}

/// The one line a batch check prints per image: the verdict, the evidence that
/// decided it, and the cheapest fix.
fn note(a: &Analysis) -> String {
    let verdict = if a.passes { "PASS" } else { "FAIL" };
    let where_ = if a.passes {
        format!(
            "weakest spot {}:1 in the {} area",
            a.worst.contrast_ratio, a.worst.position
        )
    } else {
        format!(
            "{} of {} sampled windows fall short — worst {}:1 over {} in the {} area",
            a.failing_windows,
            a.windows_checked,
            a.worst.contrast_ratio,
            a.worst.mean_hex,
            a.worst.position
        )
    };
    let best_area = a
        .placements
        .first()
        .map(|p| format!(" Safest placement: {} ({}:1).", p.area, p.worst_ratio))
        .unwrap_or_default();
    let fix = if a.passes {
        String::new()
    } else {
        match (a.scrim.recommended.as_deref(), a.scrim.css.as_deref()) {
            (Some(_), Some(css)) => format!(" A {css} scrim would clear the whole area."),
            _ => " No flat black or white scrim can rescue this text colour here.".to_string(),
        }
    };
    let hex = &a.text_color.hex;
    let needed = a.required_ratio;
    let size = &a.text_size;
    let level = a.level.to_uppercase();
    format!("{verdict} — {hex} text needs {needed}:1 for {size} text at {level}; {where_}.{best_area}{fix}")
}

/// Shape the response for the requested `output` mode. The verdict is identical
/// in every mode — only the window-grid payload changes.
fn response(
    a: &Analysis,
    output: Output,
    text_color_input: &str,
    filename: Option<String>,
) -> Result<Value, String> {
    let note = note(a);
    let csv = matches!(output, Output::Csv).then(|| grid_csv(a));
    let mut v = serde_json::to_value(a).map_err(|e| format!("serialize analysis: {e}"))?;
    let obj: &mut Map<String, Value> = v
        .as_object_mut()
        .ok_or_else(|| "analysis did not serialize to an object".to_string())?;
    if let Some(tc) = obj.get_mut("text_color").and_then(|t| t.as_object_mut()) {
        tc.insert("input".into(), Value::from(text_color_input));
    }
    if let Some(csv) = csv {
        obj.remove("grid");
        obj.insert("csv".into(), Value::from(csv));
    }
    obj.insert("output".into(), Value::from(output.name()));
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
            Param::string("text_color")
                .default("#ffffff")
                .describe(
                    "The colour the overlaid text will be drawn in. Accepts a hex code (#ffffff, \
                     #fff, bare ffffff), an rgb triple (rgb(255,255,255) or 255,255,255), an hsl \
                     triple (hsl(0,0%,100%)), or a CSS colour name (white, gold, navy). Default \
                     #ffffff, the usual hero-caption colour.",
                ),
        )
        .param(
            Param::enumv("level", ["aa", "aaa"])
                .default("aa")
                .describe(
                    "Which WCAG 2.x conformance level the text must reach. aa (default) is the \
                     legal baseline most sites are held to: 4.5:1 for normal text, 3:1 for large. \
                     aaa is the enhanced level: 7:1 normal, 4.5:1 large. UI graphics stay at 3:1 \
                     under both (SC 1.4.11 has no AAA variant).",
                ),
        )
        .param(
            Param::enumv("text_size", ["normal", "large", "ui"])
                .default("normal")
                .describe(
                    "Which success criterion the overlay falls under, because it sets the bar. \
                     normal (default) = body/caption text under 24px (or under 18.66px bold), \
                     needs 4.5:1 at AA. large = 24px+ or 18.66px+ bold headlines, needs 3:1 at AA. \
                     ui = icons, logo strokes, focus rings and other non-text graphics, 3:1.",
                ),
        )
        .param(
            Param::enumv(
                "region",
                ["full", "top", "middle", "bottom", "left", "center", "right"],
            )
            .default("full")
            .describe(
                "Which slice of the picture to scan. full (default) scans everything and ranks \
                 the areas for you; top / middle / bottom scan a horizontal third and left / \
                 center / right a vertical third — use one when you already know the caption goes \
                 there and want the verdict for that band only. All coordinates and placements in \
                 the answer are then relative to that slice.",
            ),
        )
        .param(
            Param::number("window_width")
                .min(1.0)
                .max(100.0)
                .default(30.0)
                .describe(
                    "Width of the sliding text block, as a percent of the scanned area's width, \
                     1-100 (default 30). Match it to how wide the caption really is: a narrow \
                     window finds small hot spots a wide headline would average away, a wide one \
                     models a full-bleed strapline.",
                ),
        )
        .param(
            Param::number("window_height")
                .min(1.0)
                .max(100.0)
                .default(10.0)
                .describe(
                    "Height of the sliding text block, as a percent of the scanned area's height, \
                     1-100 (default 10). Roughly one line of caption on a 16:9 hero; raise it for \
                     a multi-line block. The window steps a quarter of its own size at a time, so \
                     smaller windows mean more sample positions.",
                ),
        )
        .param(
            Param::enumv("alpha_background", ["white", "black"])
                .default("white")
                .describe(
                    "What transparent and semi-transparent pixels are measured against, i.e. the \
                     page colour showing through a PNG with alpha. white (default) matches a light \
                     page; use black for a dark-themed page. Fully opaque images ignore this.",
                ),
        )
        .param(
            Param::enumv("output", ["summary", "full", "csv"])
                .default("summary")
                .describe(
                    "How much window detail comes back. summary (default) = verdict, worst and \
                     best window, the ranked placements and the scrim fix. full = also every \
                     window's contrast ratio as rows of numbers, ready to chart as a heat map. \
                     csv = also the same grid as a row,column,contrast_ratio,passes table. full \
                     and csv are capped at 10000 windows.",
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
    name = "gizza-ai/image-text-overlay-contrast-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find the worst place on a photo where overlaid text of a given colour fails WCAG contrast.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Scan a photo for the worst-case region where overlaid text of a given colour would fail WCAG contrast, and say what would fix it. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call). A text-block-shaped window slides across the picture; every position is scored with the WCAG 2.x contrast ratio between the text colour and that window's gamma-correct mean colour. Returns the verdict for the whole area, the worst window (its pixel box, its position as a percentage, its mean colour and its ratio), the best window, how many sampled windows fall short, the twelve candidate caption areas (three full-width bands plus the nine thirds cells) ranked best-first so you can see where the caption is safe, the minimum black or white scrim opacity that would lift the whole area over the bar (with a ready-to-paste rgba() value), how pure black and pure white text would fare instead, and a one-line note. Parameters: text_color hex/rgb/hsl/CSS name (default #ffffff), level aa (default) | aaa, text_size normal (default) | large | ui, region full (default) | top | middle | bottom | left | center | right, window_width 1-100 percent (default 30), window_height 1-100 percent (default 10), alpha_background white (default) | black for what shows through transparency, output summary (default) | full for the per-window ratio grid | csv for a spreadsheet table. PNG, JPEG, WebP, GIF, BMP and TIFF are supported; the scan runs on a box-downscaled copy at most 512px on the long edge, which is exact for text-sized averages. This is an analyser only; it never modifies the image.",
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
    let args: Args =
        serde_json::from_slice(&body).invalid_args("image-text-overlay-contrast-checker")?;
    let output = Output::parse(&args.output).map_err(SkillError::InvalidArgs)?;
    let opts = to_options(&args, output).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let analysis = analyze(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let resp = response(
        &analysis,
        output,
        &args.text_color,
        (!filename.is_empty()).then_some(filename),
    )
    .map_err(SkillError::Serialize)?;
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!(
            "serialize image-text-overlay-contrast-checker response: {e}"
        ))
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

    /// Top half black, bottom half white.
    fn split_analysis(output: Output) -> Analysis {
        let bytes = png(120, 120, |_, y| {
            if y < 60 {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        });
        let mut o = Options::default();
        o.want_grid = output.wants_grid();
        o.window_width = 50.0;
        o.window_height = 50.0;
        analyze(&bytes, &o).unwrap()
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(d_text_color(), "#ffffff");
        assert_eq!(d_level(), "aa");
        assert_eq!(d_text_size(), "normal");
        assert_eq!(d_region(), "full");
        assert_eq!(d_window_width(), 30.0);
        assert_eq!(d_window_height(), 10.0);
        assert_eq!(d_alpha_background(), "white");
        assert_eq!(d_output(), "summary");
    }

    #[test]
    fn args_parse_from_a_bare_url_using_defaults() {
        let a: Args = serde_json::from_str(r#"{"url":"https://example.com/hero.jpg"}"#).unwrap();
        let out = Output::parse(&a.output).unwrap();
        assert_eq!(out, Output::Summary);
        let o = to_options(&a, out).unwrap();
        assert_eq!(o.text_color.to_hex(), "#ffffff");
        assert_eq!(o.level, Level::Aa);
        assert_eq!(o.text_size, TextSize::Normal);
        assert_eq!(o.region, Region::Full);
        assert_eq!(o.window_width, 30.0);
        assert_eq!(o.window_height, 10.0);
        assert_eq!(o.alpha_background, AlphaBackground::White);
        assert!(!o.want_grid);
    }

    #[test]
    fn bad_options_are_rejected_with_the_expected_values() {
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","text_color":"puce"}"#).unwrap();
        let err = to_options(&bad, Output::Summary).unwrap_err();
        assert!(err.contains("not a CSS named colour"), "{err}");
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","level":"a11y"}"#).unwrap();
        assert!(to_options(&bad, Output::Summary)
            .unwrap_err()
            .contains("aa or aaa"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","text_size":"tiny"}"#).unwrap();
        assert!(to_options(&bad, Output::Summary)
            .unwrap_err()
            .contains("normal, large, ui"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","region":"corner"}"#).unwrap();
        assert!(to_options(&bad, Output::Summary).unwrap_err().contains("full"));
        let bad: Args =
            serde_json::from_str(r#"{"url":"https://e.com/a.png","alpha_background":"grey"}"#)
                .unwrap();
        assert!(to_options(&bad, Output::Summary)
            .unwrap_err()
            .contains("white or black"));
        assert!(Output::parse("heatmap").unwrap_err().contains("summary"));
    }

    #[test]
    fn the_note_leads_with_the_verdict_and_names_the_fix() {
        let a = split_analysis(Output::Summary);
        let n = note(&a);
        assert!(n.starts_with("FAIL — #ffffff text needs 4.5:1 for normal text at AA;"), "{n}");
        assert!(n.contains("worst 1:1 over #ffffff"), "{n}");
        assert!(n.contains("Safest placement: top (21:1)."), "{n}");
        assert!(n.contains("scrim would clear the whole area."), "{n}");
    }

    #[test]
    fn summary_omits_the_grid_and_echoes_the_colour_as_typed() {
        let v = response(&split_analysis(Output::Summary), Output::Summary, "White", None).unwrap();
        let o = v.as_object().unwrap();
        assert!(o.get("grid").is_none(), "summary must stay compact");
        assert!(o.get("csv").is_none());
        assert_eq!(o["output"], "summary");
        assert_eq!(o["passes"], false);
        assert_eq!(o["required_ratio"], 4.5);
        assert_eq!(o["text_color"]["input"], "White");
        assert_eq!(o["text_color"]["hex"], "#ffffff");
        assert_eq!(o["worst"]["contrast_ratio"], 1.0);
        assert_eq!(o["worst"]["mean_hex"], "#ffffff");
        assert_eq!(o["best"]["contrast_ratio"], 21.0);
        assert_eq!(o["placements"][0]["area"], "top");
        assert!(o["note"].as_str().unwrap().starts_with("FAIL"));
        assert!(o.get("filename").is_none());
    }

    #[test]
    fn full_carries_the_grid_and_csv_carries_the_table() {
        let a = split_analysis(Output::Full);
        let full = response(&a, Output::Full, "#ffffff", None).unwrap();
        assert_eq!(full["grid"]["rows"], full["grid"]["ratios"].as_array().unwrap().len());
        assert_eq!(full["grid"]["ratios"][0][0], 21.0);
        assert!(full.get("csv").is_none());

        let csv = response(&a, Output::Csv, "#ffffff", None).unwrap();
        assert!(csv.get("grid").is_none());
        let text = csv["csv"].as_str().unwrap();
        assert!(text.starts_with("row,column,contrast_ratio,passes\n"), "{text}");
        assert!(text.contains("\n0,0,21,yes\n"), "{text}");
    }

    #[test]
    fn filename_is_passed_through_when_known() {
        let v = response(
            &split_analysis(Output::Summary),
            Output::Summary,
            "#ffffff",
            Some("hero.jpg".into()),
        )
        .unwrap();
        assert_eq!(v["filename"], "hero.jpg");
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "text_color": { "type": "string", "default": "#ffffff", "description": "The colour the overlaid text will be drawn in. Accepts a hex code (#ffffff, #fff, bare ffffff), an rgb triple (rgb(255,255,255) or 255,255,255), an hsl triple (hsl(0,0%,100%)), or a CSS colour name (white, gold, navy). Default #ffffff, the usual hero-caption colour." },
                    "level": { "type": "string", "enum": ["aa", "aaa"], "default": "aa", "description": "Which WCAG 2.x conformance level the text must reach. aa (default) is the legal baseline most sites are held to: 4.5:1 for normal text, 3:1 for large. aaa is the enhanced level: 7:1 normal, 4.5:1 large. UI graphics stay at 3:1 under both (SC 1.4.11 has no AAA variant)." },
                    "text_size": { "type": "string", "enum": ["normal", "large", "ui"], "default": "normal", "description": "Which success criterion the overlay falls under, because it sets the bar. normal (default) = body/caption text under 24px (or under 18.66px bold), needs 4.5:1 at AA. large = 24px+ or 18.66px+ bold headlines, needs 3:1 at AA. ui = icons, logo strokes, focus rings and other non-text graphics, 3:1." },
                    "region": { "type": "string", "enum": ["full", "top", "middle", "bottom", "left", "center", "right"], "default": "full", "description": "Which slice of the picture to scan. full (default) scans everything and ranks the areas for you; top / middle / bottom scan a horizontal third and left / center / right a vertical third — use one when you already know the caption goes there and want the verdict for that band only. All coordinates and placements in the answer are then relative to that slice." },
                    "window_width": { "type": "number", "minimum": 1, "maximum": 100, "default": 30.0, "description": "Width of the sliding text block, as a percent of the scanned area's width, 1-100 (default 30). Match it to how wide the caption really is: a narrow window finds small hot spots a wide headline would average away, a wide one models a full-bleed strapline." },
                    "window_height": { "type": "number", "minimum": 1, "maximum": 100, "default": 10.0, "description": "Height of the sliding text block, as a percent of the scanned area's height, 1-100 (default 10). Roughly one line of caption on a 16:9 hero; raise it for a multi-line block. The window steps a quarter of its own size at a time, so smaller windows mean more sample positions." },
                    "alpha_background": { "type": "string", "enum": ["white", "black"], "default": "white", "description": "What transparent and semi-transparent pixels are measured against, i.e. the page colour showing through a PNG with alpha. white (default) matches a light page; use black for a dark-themed page. Fully opaque images ignore this." },
                    "output": { "type": "string", "enum": ["summary", "full", "csv"], "default": "summary", "description": "How much window detail comes back. summary (default) = verdict, worst and best window, the ranked placements and the scrim fix. full = also every window's contrast ratio as rows of numbers, ready to chart as a heat map. csv = also the same grid as a row,column,contrast_ratio,passes table. full and csv are capped at 10000 windows." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
