//! gizza-ai/tiff-to-pdf — turn a multi-page TIFF straight into a multi-page PDF.
//!
//! File-input → document-bytes tool: the descriptor single-sources the chat
//! schema, the CLI and the page's query params; the handler resolves the TIFF
//! bytes, runs the pure Rust core, and returns the PDF in the shared
//! `application/pdf` media envelope. `Input::File`/`AssetKind::Any` (not
//! `Image`) on purpose — TIFF is served as `image/tiff`, `application/
//! octet-stream` and `application/x-tiff` by different hosts, and rejecting a
//! valid scan on a content-type quirk would be the wrong failure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_tiff_to_pdf_core::{tiff_to_pdf, ColorMode, Options, PageSize, Orientation};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 48 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 48 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_page_size")]
    page_size: String,
    #[serde(default = "d_orientation")]
    orientation: String,
    #[serde(default = "d_color")]
    color: String,
    #[serde(default)]
    margin_pt: f64,
    #[serde(default)]
    pages: String,
    #[serde(default = "d_rotate")]
    rotate: String,
    #[serde(default)]
    dpi: f64,
}

fn d_page_size() -> String {
    "fit".into()
}
fn d_orientation() -> String {
    "auto".into()
}
fn d_color() -> String {
    "auto".into()
}
fn d_rotate() -> String {
    "0".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("page_size", ["fit", "a4", "letter", "legal", "a3", "tabloid"])
                .default("fit")
                .describe("PDF page geometry. 'fit' (default) makes every page exactly the size of its own TIFF page, so a 300-DPI A4 scan comes out A4-sized and nothing is scaled. The fixed sizes (a4, letter, legal, a3, tabloid) put each image, centred and aspect-preserved, on that standard sheet instead."),
        )
        .param(
            Param::enumv("orientation", ["auto", "portrait", "landscape"])
                .default("auto")
                .describe("Sheet orientation when page_size is a fixed size: 'auto' (default) matches each page to the shape of its own image, 'portrait' and 'landscape' force one for every page. Ignored when page_size=fit, where the page already follows the image."),
        )
        .param(
            Param::enumv("color", ["auto", "color", "grayscale"])
                .default("auto")
                .describe("How samples are carried into the PDF. 'auto' (default) keeps the source as-is — a 1-bit fax/scan page stays 1-bit (smallest possible), grayscale stays grayscale, colour stays colour. 'grayscale' converts colour pages to 8-bit gray (roughly a third the size). 'color' forces every page to 8-bit RGB."),
        )
        .param(
            Param::number("margin_pt")
                .min(0.0)
                .max(144.0)
                .default(0.0)
                .describe("Blank border around the image on every page, in PDF points (72 pt = 1 inch), 0-144. Default 0 (edge to edge). With page_size=fit the margin grows the page; with a fixed page size it shrinks the area the image is fitted into. Try 36 for a half-inch border."),
        )
        .param(
            Param::string("pages")
                .default("")
                .describe("Which TIFF pages to convert, 1-based, e.g. '1-3', '2,5,9' or '4-' for page 4 to the end. Empty (the default) converts every page in file order. Duplicates are dropped and the output follows the order you list."),
        )
        .param(
            Param::enumv("rotate", ["0", "90", "180", "270"])
                .default("0")
                .describe("Clockwise rotation in degrees applied to every page: 0 (default), 90, 180 or 270. Rotation is done with the PDF placement matrix, so no pixels are resampled and the file stays exactly as sharp. A quarter turn also swaps the page's width and height."),
        )
        .param(
            Param::number("dpi")
                .min(0.0)
                .max(2400.0)
                .default(0.0)
                .describe("Resolution used to turn pixels into physical page size. 0 (default) reads the TIFF's own resolution tags and falls back to 72 DPI when it has none. Set it explicitly (e.g. 200, 300, 600) when a scan carries wrong or missing tags — a 2550x3300 page at 300 DPI is exactly US Letter."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Args → core options. Every enum is parsed here so a bad value fails with the
/// core's own message before a single byte is fetched.
fn build_options(a: &Args) -> Result<Options, String> {
    let rotate: u32 = a.rotate.trim().parse().map_err(|_| {
        format!(
            "rotate must be 0, 90, 180 or 270 degrees, got `{}`",
            a.rotate
        )
    })?;
    let opts = Options {
        page_size: PageSize::parse(&a.page_size)?,
        orientation: Orientation::parse(&a.orientation)?,
        color: ColorMode::parse(&a.color)?,
        margin_pt: a.margin_pt,
        pages: a.pages.clone(),
        rotate,
        dpi: a.dpi,
    };
    opts.validate()?;
    Ok(opts)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/tiff-to-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a multi-page TIFF into a multi-page PDF, one PDF page per TIFF page.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert a multi-page (multi-IFD) TIFF into a multi-page PDF — every page inside the TIFF becomes its own PDF page, in file order. Provide the TIFF as url or ref. Uncompressed, LZW, PackBits, Deflate, CCITT Group 3/4 fax and JPEG-in-TIFF pages all decode, in bilevel, grayscale, palette, RGB, CMYK or YCbCr, at 1/2/4/8/16 bits per sample. Nothing is re-encoded lossily: a 1-bit fax scan stays 1-bit in the PDF and rotation uses the page matrix, so the output is pixel-for-pixel the source. Parameters: page_size=fit|a4|letter|legal|a3|tabloid (default fit — the page matches the image's own physical size), orientation=auto|portrait|landscape (default auto, only used with a fixed page size), color=auto|color|grayscale (default auto keeps the source depth), margin_pt 0-144 points (default 0), pages like '1-3' or '2,5' (default: all pages), rotate=0|90|180|270 (default 0), dpi 0-2400 (default 0 = read the TIFF's resolution tags, falling back to 72). Returns a PDF. Runs locally on the device.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("tiff-to-pdf")?;
    let opts = build_options(&args).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, name) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
    let out = tiff_to_pdf(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let note = summary(&out);
    build_media_envelope(
        &out.pdf,
        "application/pdf",
        pdf_name(&name),
        note,
        MAX_OUTPUT_BYTES,
    )
}

/// `scan.tif` → `scan.pdf`; anything unnamed → `converted.pdf`.
fn pdf_name(source: &str) -> String {
    let stem = source
        .rsplit('/')
        .next()
        .unwrap_or("")
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or("")
        .trim();
    if stem.is_empty() {
        "converted.pdf".to_string()
    } else {
        format!("{stem}.pdf")
    }
}

/// One human line describing what came out, for the chat/CLI response.
fn summary(out: &gizza_ai_tiff_to_pdf_core::Conversion) -> String {
    let selected = if out.pages_written == out.source_pages {
        format!(
            "all {} page{}",
            out.source_pages,
            if out.source_pages == 1 { "" } else { "s" }
        )
    } else {
        format!("{} of {} pages", out.pages_written, out.source_pages)
    };
    match out.pages.first() {
        Some(p) => format!(
            "Converted {selected} of this TIFF to PDF. First page: {}x{} px at {} DPI ({}), \
             {} x {} pt.",
            p.width_px, p.height_px, p.dpi, p.color, p.page_width_pt, p.page_height_pt
        ),
        None => format!("Converted {selected} of this TIFF to PDF."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizza_ai_tiff_to_pdf_core::{Conversion, PageReport};

    fn args(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn defaults_match_the_descriptor() {
        let a = args(r#"{"url":"https://example.com/scan.tif"}"#);
        assert_eq!(a.page_size, "fit");
        assert_eq!(a.orientation, "auto");
        assert_eq!(a.color, "auto");
        assert_eq!(a.margin_pt, 0.0);
        assert_eq!(a.pages, "");
        assert_eq!(a.rotate, "0");
        assert_eq!(a.dpi, 0.0);
        assert_eq!(build_options(&a).unwrap(), Options::default());
    }

    #[test]
    fn every_option_reaches_the_core() {
        let a = args(
            r#"{"url":"https://example.com/scan.tif","page_size":"a4","orientation":"landscape",
                "color":"grayscale","margin_pt":36,"pages":"1-3","rotate":"90","dpi":300}"#,
        );
        let o = build_options(&a).unwrap();
        assert_eq!(o.page_size, PageSize::A4);
        assert_eq!(o.orientation, Orientation::Landscape);
        assert_eq!(o.color, ColorMode::Grayscale);
        assert_eq!(o.margin_pt, 36.0);
        assert_eq!(o.pages, "1-3");
        assert_eq!(o.rotate, 90);
        assert_eq!(o.dpi, 300.0);
    }

    #[test]
    fn bad_params_are_rejected_before_the_file_is_fetched() {
        let a = args(r#"{"url":"https://example.com/a.tif","page_size":"a5"}"#);
        assert!(build_options(&a).unwrap_err().contains("unknown page_size"));
        let a = args(r#"{"url":"https://example.com/a.tif","orientation":"sideways"}"#);
        assert!(build_options(&a)
            .unwrap_err()
            .contains("unknown orientation"));
        let a = args(r#"{"url":"https://example.com/a.tif","color":"cmyk"}"#);
        assert!(build_options(&a).unwrap_err().contains("unknown color"));
        let a = args(r#"{"url":"https://example.com/a.tif","rotate":"45"}"#);
        assert!(build_options(&a).unwrap_err().contains("rotate must be"));
        let a = args(r#"{"url":"https://example.com/a.tif","margin_pt":300}"#);
        assert!(build_options(&a).unwrap_err().contains("margin_pt"));
        let a = args(r#"{"url":"https://example.com/a.tif","dpi":9000}"#);
        assert!(build_options(&a).unwrap_err().contains("dpi"));
    }

    #[test]
    fn output_is_named_after_the_source_file() {
        assert_eq!(pdf_name("/uploads/board-minutes.tiff"), "board-minutes.pdf");
        assert_eq!(pdf_name("scan.tif"), "scan.pdf");
        assert_eq!(pdf_name(""), "converted.pdf");
        assert_eq!(pdf_name("no-extension"), "converted.pdf");
    }

    #[test]
    fn summary_reports_whole_and_partial_conversions() {
        let page = PageReport {
            source_page: 1,
            width_px: 2550,
            height_px: 3300,
            dpi: 300.0,
            color: "bilevel",
            page_width_pt: 612.0,
            page_height_pt: 792.0,
        };
        let all = Conversion {
            pdf: vec![],
            source_pages: 4,
            pages_written: 4,
            pages: vec![page.clone()],
        };
        let s = summary(&all);
        assert!(s.contains("all 4 pages"), "{s}");
        assert!(s.contains("2550x3300 px at 300 DPI (bilevel)"), "{s}");

        let some = Conversion {
            pages_written: 2,
            ..all
        };
        assert!(summary(&some).contains("2 of 4 pages"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "page_size": { "type": "string", "enum": ["fit", "a4", "letter", "legal", "a3", "tabloid"], "default": "fit", "description": "PDF page geometry. 'fit' (default) makes every page exactly the size of its own TIFF page, so a 300-DPI A4 scan comes out A4-sized and nothing is scaled. The fixed sizes (a4, letter, legal, a3, tabloid) put each image, centred and aspect-preserved, on that standard sheet instead." },
                    "orientation": { "type": "string", "enum": ["auto", "portrait", "landscape"], "default": "auto", "description": "Sheet orientation when page_size is a fixed size: 'auto' (default) matches each page to the shape of its own image, 'portrait' and 'landscape' force one for every page. Ignored when page_size=fit, where the page already follows the image." },
                    "color": { "type": "string", "enum": ["auto", "color", "grayscale"], "default": "auto", "description": "How samples are carried into the PDF. 'auto' (default) keeps the source as-is — a 1-bit fax/scan page stays 1-bit (smallest possible), grayscale stays grayscale, colour stays colour. 'grayscale' converts colour pages to 8-bit gray (roughly a third the size). 'color' forces every page to 8-bit RGB." },
                    "margin_pt": { "type": "number", "minimum": 0, "maximum": 144, "default": 0.0, "description": "Blank border around the image on every page, in PDF points (72 pt = 1 inch), 0-144. Default 0 (edge to edge). With page_size=fit the margin grows the page; with a fixed page size it shrinks the area the image is fitted into. Try 36 for a half-inch border." },
                    "pages": { "type": "string", "default": "", "description": "Which TIFF pages to convert, 1-based, e.g. '1-3', '2,5,9' or '4-' for page 4 to the end. Empty (the default) converts every page in file order. Duplicates are dropped and the output follows the order you list." },
                    "rotate": { "type": "string", "enum": ["0", "90", "180", "270"], "default": "0", "description": "Clockwise rotation in degrees applied to every page: 0 (default), 90, 180 or 270. Rotation is done with the PDF placement matrix, so no pixels are resampled and the file stays exactly as sharp. A quarter turn also swaps the page's width and height." },
                    "dpi": { "type": "number", "minimum": 0, "maximum": 2400, "default": 0.0, "description": "Resolution used to turn pixels into physical page size. 0 (default) reads the TIFF's own resolution tags and falls back to 72 DPI when it has none. Set it explicitly (e.g. 200, 300, 600) when a scan carries wrong or missing tags — a 2550x3300 page at 300 DPI is exactly US Letter." }
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
