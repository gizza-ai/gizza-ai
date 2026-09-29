//! gizza-ai/hough-line-detection — find the straight line segments in an image
//! with the Hough transform and either report their geometry (endpoints, angle,
//! length, votes) or draw them back over the picture as an overlay.
//!
//! Pure Rust (`image` crate + a hand-rolled Canny/accumulator/segment walk in
//! `core`) → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page: the generator's pure-tool page cannot hand
//! uploaded bytes to a wasm decoder, the same "no-page file-input" pattern as
//! image-blank-detector / background-color-detector / document-skew-detector.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_hough_line_detection_core as core;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// A high-resolution scan or phone photo, before the header-first budget check.
const MAX_INPUT_BYTES: usize = 24 * 1024 * 1024;
/// Overlay payload cap (it travels as a base64 data URL).
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    canny_low: f64,
    #[serde(default)]
    canny_high: f64,
    #[serde(default = "d_blur")]
    blur: f64,
    #[serde(default)]
    threshold: u32,
    #[serde(default)]
    min_line_length: f64,
    #[serde(default)]
    max_line_gap: f64,
    #[serde(default = "d_angle_resolution")]
    angle_resolution: f64,
    #[serde(default = "d_rho_resolution")]
    rho_resolution: f64,
    #[serde(default = "d_max_lines")]
    max_lines: u32,
    #[serde(default)]
    orientation: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    line_width: u32,
    #[serde(default)]
    overlay_background: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

fn d_blur() -> f64 {
    core::DEFAULT_BLUR
}
fn d_angle_resolution() -> f64 {
    core::DEFAULT_ANGLE_RESOLUTION
}
fn d_rho_resolution() -> f64 {
    core::DEFAULT_RHO_RESOLUTION
}
fn d_max_lines() -> u32 {
    core::DEFAULT_MAX_LINES as u32
}

impl Args {
    fn to_options(&self) -> Result<core::Options, String> {
        Ok(core::Options {
            mode: core::parse_mode(self.mode.as_deref())?,
            canny_low: self.canny_low,
            canny_high: self.canny_high,
            blur: self.blur,
            threshold: self.threshold,
            min_line_length: self.min_line_length,
            max_line_gap: self.max_line_gap,
            angle_resolution: self.angle_resolution,
            rho_resolution: self.rho_resolution,
            max_lines: self.max_lines,
            orientation: core::parse_orientation(self.orientation.as_deref())?,
            output: core::parse_output(self.output.as_deref())?,
            color: self
                .color
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| core::DEFAULT_COLOR.to_string()),
            line_width: self.line_width,
            overlay_background: core::parse_background(self.overlay_background.as_deref())?,
            format: core::parse_format(self.format.as_deref())?,
        })
    }
}

// ---------------------------------------------------------------------------
// Report shape
// ---------------------------------------------------------------------------

#[derive(Serialize, Debug, PartialEq)]
struct LineOut {
    x1: i64,
    y1: i64,
    x2: i64,
    y2: i64,
    length: f64,
    angle_degrees: f64,
    rho: f64,
    theta_degrees: f64,
    votes: u32,
    orientation: &'static str,
}

#[derive(Serialize, Debug, PartialEq)]
struct Resp {
    width: u32,
    height: u32,
    analysis_width: u32,
    analysis_height: u32,
    downscale_factor: u32,
    edge_pixels: u64,
    canny_low_used: f64,
    canny_high_used: f64,
    threshold_used: u32,
    min_line_length_used: f64,
    max_line_gap_used: f64,
    line_count: usize,
    horizontal_count: usize,
    vertical_count: usize,
    diagonal_count: usize,
    dominant_angle_degrees: Option<f64>,
    lines: Vec<LineOut>,
    warnings: Vec<String>,
    note: String,
}

fn report(d: &core::Detection) -> Resp {
    Resp {
        width: d.width,
        height: d.height,
        analysis_width: d.analysis_width,
        analysis_height: d.analysis_height,
        downscale_factor: d.downscale_factor,
        edge_pixels: d.edge_pixels,
        canny_low_used: d.canny_low_used,
        canny_high_used: d.canny_high_used,
        threshold_used: d.threshold_used,
        min_line_length_used: d.min_line_length_used,
        max_line_gap_used: d.max_line_gap_used,
        line_count: d.line_count,
        horizontal_count: d.horizontal_count,
        vertical_count: d.vertical_count,
        diagonal_count: d.diagonal_count,
        dominant_angle_degrees: d.dominant_angle_degrees,
        lines: d
            .lines
            .iter()
            .map(|l| LineOut {
                x1: l.x1,
                y1: l.y1,
                x2: l.x2,
                y2: l.y2,
                length: l.length,
                angle_degrees: l.angle_degrees,
                rho: l.rho,
                theta_degrees: l.theta_degrees,
                votes: l.votes,
                orientation: l.orientation,
            })
            .collect(),
        warnings: d.warnings.clone(),
        note: note(d),
    }
}

/// One plain-English line telling the caller what to do next.
fn note(d: &core::Detection) -> String {
    if d.line_count == 0 {
        return "No lines passed the thresholds. Lower min_line_length or threshold, lower \
                canny_high to keep fainter edges, or set output=overlay with \
                overlay_background=edges to see what the edge detector actually found."
            .to_string();
    }
    format!(
        "Found {} line(s) ({} horizontal, {} vertical, {} diagonal) at threshold {} votes and \
         min_line_length {} px. Re-run with output=overlay to see them drawn on the image.",
        d.line_count,
        d.horizontal_count,
        d.vertical_count,
        d.diagonal_count,
        d.threshold_used,
        d.min_line_length_used
    )
}

/// The exact one-liner the LLM sees for an overlay render.
fn overlay_summary(in_filename: &str, out_filename: &str, bytes: usize, d: &core::Detection) -> String {
    format!(
        "Drew {} detected line(s) ({} horizontal, {} vertical, {} diagonal) from {} over a {}x{} overlay → {} ({} bytes).",
        d.line_count,
        d.horizontal_count,
        d.vertical_count,
        d.diagonal_count,
        in_filename,
        d.analysis_width,
        d.analysis_height,
        out_filename,
        bytes
    )
}

// ---------------------------------------------------------------------------
// Descriptor — single source for the chat schema and the CLI
// ---------------------------------------------------------------------------

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("mode", core::MODES)
                .default(core::DEFAULT_MODE)
                .describe("segments (default) returns real line segments with endpoints, the probabilistic-Hough answer. lines returns each accumulator peak as an infinite line clipped to the image rectangle, the classic-Hough answer."),
        )
        .param(
            Param::number("canny_low")
                .min(0.0)
                .max(1.0)
                .default(0)
                .describe("Canny lower hysteresis threshold as a 0-1 fraction of the maximum possible gradient. 0 (default) is automatic (0.4 x the automatic high threshold). Lower it to keep fainter, broken edges."),
        )
        .param(
            Param::number("canny_high")
                .min(0.0)
                .max(1.0)
                .default(0)
                .describe("Canny upper hysteresis threshold as a 0-1 fraction of the maximum possible gradient. 0 (default) is automatic (Otsu over the gradient histogram). Raise it on noisy or textured photos to keep only strong edges."),
        )
        .param(
            Param::number("blur")
                .min(0.0)
                .max(core::MAX_BLUR)
                .default(core::DEFAULT_BLUR)
                .describe("Gaussian pre-blur sigma applied before edge detection (0-5, default 1). 0 turns smoothing off for crisp synthetic images; raise to 2-3 to suppress grain in photos."),
        )
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(100000.0)
                .default(0)
                .describe("Minimum accumulator votes a line must collect. 0 (default) is automatic (0.6 x min_line_length in analysis pixels, floor 16). Raise it to keep only the most strongly supported lines."),
        )
        .param(
            Param::number("min_line_length")
                .min(0.0)
                .max(20000.0)
                .default(0)
                .describe("Shortest segment to report, in ORIGINAL image pixels. 0 (default) is automatic (8% of the image diagonal, floor 20 px). Only used in segments mode."),
        )
        .param(
            Param::number("max_line_gap")
                .min(0.0)
                .max(2000.0)
                .default(0)
                .describe("Largest break, in ORIGINAL image pixels, tolerated inside one segment before it is split in two. 0 (default) is automatic (1.5% of the diagonal, floor 3 px). Only used in segments mode."),
        )
        .param(
            Param::number("angle_resolution")
                .min(0.1)
                .max(5.0)
                .default(core::DEFAULT_ANGLE_RESOLUTION)
                .describe("Accumulator angle step in degrees (0.1-5, default 1). Smaller is more precise but slower; 0.25 helps when two lines differ by a fraction of a degree."),
        )
        .param(
            Param::number("rho_resolution")
                .min(0.5)
                .max(20.0)
                .default(core::DEFAULT_RHO_RESOLUTION)
                .describe("Accumulator distance step in analysis pixels (0.5-20, default 1). Larger merges nearly-coincident parallel lines into one."),
        )
        .param(
            Param::integer("max_lines")
                .min(1.0)
                .max(core::MAX_LINES_CAP as f64)
                .default(core::DEFAULT_MAX_LINES)
                .describe("Maximum number of lines to return, strongest first (1-500, default 50)."),
        )
        .param(
            Param::enumv("orientation", core::ORIENTATIONS)
                .default(core::DEFAULT_ORIENTATION)
                .describe("Keep only lines of this class: any (default), horizontal or vertical. A line counts as horizontal/vertical when it sits within 10 degrees of that axis."),
        )
        .param(
            Param::enumv("output", core::OUTPUTS)
                .default(core::DEFAULT_OUTPUT)
                .describe("report (default) returns the line geometry as JSON. overlay returns an image with the detected lines drawn on it."),
        )
        .param(
            Param::string("color")
                .default(core::DEFAULT_COLOR)
                .describe("Overlay line colour as #rgb, #rrggbb or a name (red, green, lime, blue, yellow, cyan, magenta, orange, white, black, gray). Default #ff0000."),
        )
        .param(
            Param::integer("line_width")
                .min(0.0)
                .max(core::MAX_LINE_WIDTH as f64)
                .default(0)
                .describe("Overlay line thickness in pixels (1-20). 0 (default) scales it from the image size."),
        )
        .param(
            Param::enumv("overlay_background", core::BACKGROUNDS)
                .default(core::DEFAULT_BACKGROUND)
                .describe("What the overlay draws on: original (default) the source photo, edges the Canny edge map (use this to debug what the transform saw), black or white a flat canvas."),
        )
        .param(
            Param::enumv("format", core::FORMATS)
                .default(core::DEFAULT_FORMAT)
                .describe("Overlay image format: png (default, lossless) or jpg (smaller)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HoughLineDetection;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hough-line-detection",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect straight lines in an image with the Hough transform and report their endpoints, angles and lengths or draw them as an overlay",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Detect straight lines in an image using the Hough transform (a hand-rolled Canny edge pass, a rho/theta accumulator, peak suppression and a segment walk). mode=segments (default) returns real segments with endpoints, the HoughLinesP answer shape; mode=lines returns each accumulator peak as an infinite line clipped to the image. Tuning: canny_low/canny_high (0-1 gradient fractions, 0 = automatic Otsu), blur (Gaussian sigma 0-5, default 1), threshold (minimum votes, 0 = automatic), min_line_length and max_line_gap (ORIGINAL image pixels, 0 = automatic from the image diagonal), angle_resolution (0.1-5 degrees, default 1), rho_resolution (0.5-20 px, default 1), max_lines (1-500, default 50) and orientation (any|horizontal|vertical). output=report (default) returns JSON: per-line x1,y1,x2,y2,length,angle_degrees,rho,theta_degrees,votes,orientation plus counts, the dominant angle, and the effective value of every automatic threshold so the next call can be tuned. output=overlay returns the image with the lines drawn, controlled by color, line_width, overlay_background (original|edges|black|white) and format (png|jpg). angle_degrees is the tilt from horizontal in (-90,90], positive when the right-hand end sits lower. Large images are analyzed downscaled to 1024 px on the long side and coordinates are mapped back to original pixels. Accepts PNG, JPEG, WebP, GIF and BMP as either url (HTTP/HTTPS) or ref from a prior tool call. This finds straight edges: for one page-skew angle use document-skew-detector, for a photo horizon use image-horizon-tilt-checker, for a page quadrilateral use document-scan, and for a plain edge-map image use edge-detection. Circle and generalized Hough detection are not supported.",
        parameters = schema_json()
    ),
)]
impl HoughLineDetection {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::replace_extension;

    let args: Args = serde_json::from_slice(&body).invalid_args("hough-line-detection")?;
    let opts = args.to_options().map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;

    let outcome = core::detect(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    match outcome.overlay {
        Some((out, _w, _h)) => {
            let filename = replace_extension(&in_filename, opts.format.ext());
            let for_llm = overlay_summary(&in_filename, &filename, out.len(), &outcome.detection);
            build_media_envelope(
                &out,
                opts.format.mime(),
                filename,
                for_llm,
                MAX_OUTPUT_BYTES,
            )
        }
        None => {
            let resp = report(&outcome.detection);
            serde_json::to_vec(&resp).map_err(|e| {
                SkillError::Serialize(format!("serialize hough-line-detection response: {e}"))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_from(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn defaults_applied() {
        let a = args_from(r#"{"url":"https://example.com/plan.png"}"#);
        assert_eq!(a.blur, core::DEFAULT_BLUR);
        assert_eq!(a.angle_resolution, core::DEFAULT_ANGLE_RESOLUTION);
        assert_eq!(a.rho_resolution, core::DEFAULT_RHO_RESOLUTION);
        assert_eq!(a.max_lines, core::DEFAULT_MAX_LINES as u32);
        assert_eq!(a.threshold, 0);
        assert_eq!(a.line_width, 0);
        let o = a.to_options().unwrap();
        assert_eq!(o.mode, core::Mode::Segments);
        assert_eq!(o.orientation, core::Orientation::Any);
        assert_eq!(o.output, core::Output::Report);
        assert_eq!(o.overlay_background, core::Background::Original);
        assert_eq!(o.format, core::Format::Png);
        assert_eq!(o.color, core::DEFAULT_COLOR);
    }

    #[test]
    fn overrides_map_onto_core_options() {
        let a = args_from(
            r##"{"url":"https://example.com/plan.png","mode":"lines","orientation":"vertical",
                "output":"overlay","overlay_background":"edges","format":"jpg","color":"#0f0",
                "line_width":4,"max_lines":7,"threshold":120,"blur":0,"canny_low":0.1,
                "canny_high":0.3,"min_line_length":150,"max_line_gap":12,
                "angle_resolution":0.5,"rho_resolution":2}"##,
        );
        let o = a.to_options().unwrap();
        assert_eq!(o.mode, core::Mode::Lines);
        assert_eq!(o.orientation, core::Orientation::Vertical);
        assert_eq!(o.output, core::Output::Overlay);
        assert_eq!(o.overlay_background, core::Background::Edges);
        assert_eq!(o.format, core::Format::Jpg);
        assert_eq!(o.color, "#0f0");
        assert_eq!(o.line_width, 4);
        assert_eq!(o.max_lines, 7);
        assert_eq!(o.threshold, 120);
        assert_eq!(o.blur, 0.0);
        assert_eq!(o.min_line_length, 150.0);
        assert_eq!(o.max_line_gap, 12.0);
        assert_eq!(o.angle_resolution, 0.5);
        assert_eq!(o.rho_resolution, 2.0);
    }

    #[test]
    fn bad_enum_is_rejected_with_a_guiding_message() {
        let a = args_from(r#"{"url":"https://example.com/p.png","mode":"circles"}"#);
        let err = a.to_options().unwrap_err();
        assert!(err.contains("mode must be one of segments, lines"), "{err}");
    }

    #[test]
    fn empty_color_falls_back_to_the_default() {
        let a = args_from(r#"{"url":"https://example.com/p.png","color":"  "}"#);
        assert_eq!(a.to_options().unwrap().color, core::DEFAULT_COLOR);
    }

    #[test]
    fn note_guides_tuning_when_nothing_was_found() {
        let d = core::Detection {
            width: 100,
            height: 100,
            analysis_width: 100,
            analysis_height: 100,
            downscale_factor: 1,
            edge_pixels: 0,
            canny_low_used: 0.0,
            canny_high_used: 0.0,
            threshold_used: 16,
            min_line_length_used: 20.0,
            max_line_gap_used: 3.0,
            line_count: 0,
            lines: vec![],
            dominant_angle_degrees: None,
            horizontal_count: 0,
            vertical_count: 0,
            diagonal_count: 0,
            warnings: vec![],
        };
        let r = report(&d);
        assert_eq!(r.line_count, 0);
        assert!(r.note.contains("min_line_length"), "{}", r.note);
        assert!(r.note.contains("overlay_background=edges"), "{}", r.note);
    }

    #[test]
    fn overlay_summary_is_exact_and_useful() {
        let d = core::Detection {
            width: 800,
            height: 600,
            analysis_width: 800,
            analysis_height: 600,
            downscale_factor: 1,
            edge_pixels: 4210,
            canny_low_used: 0.04,
            canny_high_used: 0.1,
            threshold_used: 24,
            min_line_length_used: 80.0,
            max_line_gap_used: 15.0,
            line_count: 3,
            lines: vec![],
            dominant_angle_degrees: Some(0.0),
            horizontal_count: 2,
            vertical_count: 1,
            diagonal_count: 0,
            warnings: vec![],
        };
        assert_eq!(
            overlay_summary("plan.png", "plan.png", 5120, &d),
            "Drew 3 detected line(s) (2 horizontal, 1 vertical, 0 diagonal) from plan.png over a 800x600 overlay → plan.png (5120 bytes)."
        );
    }

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Image url⊕ref oneOf + every Hough knob).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["segments", "lines"], "default": "segments", "description": "segments (default) returns real line segments with endpoints, the probabilistic-Hough answer. lines returns each accumulator peak as an infinite line clipped to the image rectangle, the classic-Hough answer." },
                    "canny_low": { "type": "number", "minimum": 0, "maximum": 1, "default": 0, "description": "Canny lower hysteresis threshold as a 0-1 fraction of the maximum possible gradient. 0 (default) is automatic (0.4 x the automatic high threshold). Lower it to keep fainter, broken edges." },
                    "canny_high": { "type": "number", "minimum": 0, "maximum": 1, "default": 0, "description": "Canny upper hysteresis threshold as a 0-1 fraction of the maximum possible gradient. 0 (default) is automatic (Otsu over the gradient histogram). Raise it on noisy or textured photos to keep only strong edges." },
                    "blur": { "type": "number", "minimum": 0, "maximum": 5, "default": 1.0, "description": "Gaussian pre-blur sigma applied before edge detection (0-5, default 1). 0 turns smoothing off for crisp synthetic images; raise to 2-3 to suppress grain in photos." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 100000, "default": 0, "description": "Minimum accumulator votes a line must collect. 0 (default) is automatic (0.6 x min_line_length in analysis pixels, floor 16). Raise it to keep only the most strongly supported lines." },
                    "min_line_length": { "type": "number", "minimum": 0, "maximum": 20000, "default": 0, "description": "Shortest segment to report, in ORIGINAL image pixels. 0 (default) is automatic (8% of the image diagonal, floor 20 px). Only used in segments mode." },
                    "max_line_gap": { "type": "number", "minimum": 0, "maximum": 2000, "default": 0, "description": "Largest break, in ORIGINAL image pixels, tolerated inside one segment before it is split in two. 0 (default) is automatic (1.5% of the diagonal, floor 3 px). Only used in segments mode." },
                    "angle_resolution": { "type": "number", "minimum": 0.1, "maximum": 5, "default": 1.0, "description": "Accumulator angle step in degrees (0.1-5, default 1). Smaller is more precise but slower; 0.25 helps when two lines differ by a fraction of a degree." },
                    "rho_resolution": { "type": "number", "minimum": 0.5, "maximum": 20, "default": 1.0, "description": "Accumulator distance step in analysis pixels (0.5-20, default 1). Larger merges nearly-coincident parallel lines into one." },
                    "max_lines": { "type": "integer", "minimum": 1, "maximum": 500, "default": 50, "description": "Maximum number of lines to return, strongest first (1-500, default 50)." },
                    "orientation": { "type": "string", "enum": ["any", "horizontal", "vertical"], "default": "any", "description": "Keep only lines of this class: any (default), horizontal or vertical. A line counts as horizontal/vertical when it sits within 10 degrees of that axis." },
                    "output": { "type": "string", "enum": ["report", "overlay"], "default": "report", "description": "report (default) returns the line geometry as JSON. overlay returns an image with the detected lines drawn on it." },
                    "color": { "type": "string", "default": "#ff0000", "description": "Overlay line colour as #rgb, #rrggbb or a name (red, green, lime, blue, yellow, cyan, magenta, orange, white, black, gray). Default #ff0000." },
                    "line_width": { "type": "integer", "minimum": 0, "maximum": 20, "default": 0, "description": "Overlay line thickness in pixels (1-20). 0 (default) scales it from the image size." },
                    "overlay_background": { "type": "string", "enum": ["original", "edges", "black", "white"], "default": "original", "description": "What the overlay draws on: original (default) the source photo, edges the Canny edge map (use this to debug what the transform saw), black or white a flat canvas." },
                    "format": { "type": "string", "enum": ["png", "jpg"], "default": "png", "description": "Overlay image format: png (default, lossless) or jpg (smaller)." }
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
