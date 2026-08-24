//! gizza-ai/pdf-resize-pages — change a PDF's page size and scale the content.
//!
//! Loads the source PDF (URL/ref), rewrites the selected pages' `/MediaBox` to
//! a named paper size (or a custom width/height) and wraps their content in a
//! `q … cm … Q` transform so the artwork is scaled and centred on the new page
//! instead of being clipped. Returns the new PDF as a base64 envelope.
//! `Input::Document` + size/orientation/scale/zoom/margin/pages.
//! Chat + CLI only (PDF bytes have no page surface).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

/// The `size` variants, in the order the descriptor advertises them. Kept as a
/// literal so the hygiene gate can read the enum list out of the source; a test
/// asserts it still matches the core crate's preset table.
const SIZES: [&str; 15] = [
    "a4", "letter", "legal", "a0", "a1", "a2", "a3", "a5", "a6", "b4", "b5", "tabloid",
    "executive", "statement", "custom",
];

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_size")]
    size: String,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
    #[serde(default = "default_unit")]
    unit: String,
    #[serde(default = "default_orientation")]
    orientation: String,
    #[serde(default = "default_scale")]
    scale: String,
    #[serde(default = "default_zoom")]
    zoom: f64,
    #[serde(default)]
    margin: f64,
    #[serde(default = "default_pages")]
    pages: String,
}
fn default_size() -> String {
    "a4".to_string()
}
fn default_unit() -> String {
    "mm".to_string()
}
fn default_orientation() -> String {
    "auto".to_string()
}
fn default_scale() -> String {
    "fit".to_string()
}
fn default_zoom() -> f64 {
    100.0
}
fn default_pages() -> String {
    "all".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("size", SIZES)
                .default("a4")
                .describe("Target paper size (default a4). Use 'custom' with width/height for anything else."),
        )
        .param(Param::number("width").default(0.0).min(0.0).describe(
            "Page width, in `unit`. Only used when size=custom (e.g. width=210 height=297 unit=mm).",
        ))
        .param(Param::number("height").default(0.0).min(0.0).describe(
            "Page height, in `unit`. Only used when size=custom.",
        ))
        .param(
            Param::enumv("unit", ["mm", "cm", "in", "pt"])
                .default("mm")
                .describe("Unit for width/height/margin: mm (default), cm, in, or pt (1/72\")."),
        )
        .param(
            Param::enumv("orientation", ["auto", "portrait", "landscape"])
                .default("auto")
                .describe(
                    "Page orientation: 'auto' (default) keeps each page's current orientation, or force 'portrait'/'landscape'.",
                ),
        )
        .param(
            Param::enumv("scale", ["fit", "fill", "stretch", "none"])
                .default("fit")
                .describe(
                    "How the old content is fitted: 'fit' (default) shrinks/grows it proportionally until it all fits, 'fill' covers the page (edges may overflow), 'stretch' distorts it to fill exactly, 'none' keeps it at original size and centres it.",
                ),
        )
        .param(Param::number("zoom").default(100.0).min(1.0).max(1000.0).describe(
            "Extra zoom on top of `scale`, in percent (default 100 = none). Over 100 enlarges the content (it may overflow); under 100 shrinks it, leaving more whitespace.",
        ))
        .param(Param::number("margin").default(0.0).min(0.0).describe(
            "Blank margin to keep on all four sides of the new page, in `unit` (default 0). The content is fitted inside what's left.",
        ))
        .param(Param::string("pages").default("all").describe(
            "Which pages to resize, 1-based: 'all' (default), 'odd', 'even', or a list/range like '1,3-5'. Pages not listed keep their current size.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfResizePages;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-resize-pages",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Change a PDF's page size and scale the content to fit",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Change a PDF's page size — e.g. A4 to US Letter — and scale the existing content to fit the new page. Pick `size` from the standard presets (a4, letter, legal, a0-a6, b4, b5, tabloid, executive, statement) or 'custom' with width/height in `unit` (mm, cm, in, pt). `scale` chooses the fitting: fit (proportional, whole page visible — the default), fill (cover, edges may overflow), stretch (distort to fill exactly), or none (keep original size, centred). `zoom` adds an extra percentage on top, `margin` keeps blank space on all four sides, `orientation` forces portrait/landscape, and `pages` limits it to some pages ('all', 'odd', 'even', or '1,3-5'). Text and vector art stay vector — nothing is rasterised — and links/highlights move with the content. Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfResizePages {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// `595.0 x 842.0` → `595 x 842`; keeps a fraction only when there is one.
fn dim(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;
    use gizza_ai_pdf_resize_pages_core::Options;

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-resize-pages")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;
    let out = gizza_ai_pdf_resize_pages_core::resize(
        &bytes,
        &Options {
            size: &args.size,
            width: args.width,
            height: args.height,
            unit: &args.unit,
            orientation: &args.orientation,
            scale: &args.scale,
            zoom: args.zoom,
            margin: args.margin,
            pages: &args.pages,
        },
    )
    .map_err(SkillError::InvalidArgs)?;

    let out_len = out.pdf.len();
    let changed = out.pages.len();
    // Every resized page ends up the same size unless orientation=auto met a
    // mixed document, so report the one size when there is only one.
    let mut sizes: Vec<String> = out
        .pages
        .iter()
        .map(|p| format!("{}pt x {}pt", dim(p.width_pt), dim(p.height_pt)))
        .collect();
    sizes.dedup();
    let size_note = if sizes.len() == 1 {
        sizes.remove(0)
    } else {
        format!("{} different sizes", sizes.len())
    };
    let scale_note = out
        .pages
        .first()
        .map(|p| {
            if (p.scale_x - p.scale_y).abs() < 1e-6 {
                format!("content scaled {:.1}%", p.scale_x * 100.0)
            } else {
                format!(
                    "content scaled {:.1}% x {:.1}%",
                    p.scale_x * 100.0,
                    p.scale_y * 100.0
                )
            }
        })
        .unwrap_or_else(|| "no pages changed".to_string());

    let encoded = B64.encode(&out.pdf);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|s| format!("{s}-resized.pdf"))
        .unwrap_or_else(|| "resized.pdf".to_string());

    let env = Envelope {
        for_llm: format!(
            "resized {changed} of {} page(s) of {filename} to {size_note} (scale={}, {scale_note}); {out_len}-byte PDF",
            out.total_pages, args.scale
        ),
        for_ui: ForUi {
            data_url,
            mime: "application/pdf".to_string(),
            filename: out_name,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertised_sizes_match_the_core_preset_table() {
        let mut advertised: Vec<&str> = SIZES.to_vec();
        let mut core = gizza_ai_pdf_resize_pages_core::size_names();
        advertised.sort_unstable();
        core.sort_unstable();
        assert_eq!(advertised, core, "descriptor `size` enum drifted from core");
    }

    #[test]
    fn dim_prints_whole_points_without_a_fraction() {
        assert_eq!(dim(595.0), "595");
        assert_eq!(dim(841.89), "841.9");
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "size":   { "type": "string", "enum": ["a4","letter","legal","a0","a1","a2","a3","a5","a6","b4","b5","tabloid","executive","statement","custom"], "default": "a4", "description": "Target paper size (default a4). Use 'custom' with width/height for anything else." },
                    "width":  { "type": "number", "minimum": 0, "default": 0.0, "description": "Page width, in `unit`. Only used when size=custom (e.g. width=210 height=297 unit=mm)." },
                    "height": { "type": "number", "minimum": 0, "default": 0.0, "description": "Page height, in `unit`. Only used when size=custom." },
                    "unit":   { "type": "string", "enum": ["mm","cm","in","pt"], "default": "mm", "description": "Unit for width/height/margin: mm (default), cm, in, or pt (1/72\")." },
                    "orientation": { "type": "string", "enum": ["auto","portrait","landscape"], "default": "auto", "description": "Page orientation: 'auto' (default) keeps each page's current orientation, or force 'portrait'/'landscape'." },
                    "scale":  { "type": "string", "enum": ["fit","fill","stretch","none"], "default": "fit", "description": "How the old content is fitted: 'fit' (default) shrinks/grows it proportionally until it all fits, 'fill' covers the page (edges may overflow), 'stretch' distorts it to fill exactly, 'none' keeps it at original size and centres it." },
                    "zoom":   { "type": "number", "minimum": 1, "maximum": 1000, "default": 100.0, "description": "Extra zoom on top of `scale`, in percent (default 100 = none). Over 100 enlarges the content (it may overflow); under 100 shrinks it, leaving more whitespace." },
                    "margin": { "type": "number", "minimum": 0, "default": 0.0, "description": "Blank margin to keep on all four sides of the new page, in `unit` (default 0). The content is fitted inside what's left." },
                    "pages":  { "type": "string", "default": "all", "description": "Which pages to resize, 1-based: 'all' (default), 'odd', 'even', or a list/range like '1,3-5'. Pages not listed keep their current size." }
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
