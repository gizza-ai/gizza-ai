//! gizza-ai/image-low-poly — turn a photo into low-poly triangle art: warp a
//! seeded triangle mesh onto the image so its corners follow contours, then
//! flat-fill every triangle with the colour of the source region beneath it.
//! Returns a PNG. Pure Rust (`image`) — runs on all backends incl. the chat SW.
//! Surfaces: chat + CLI (image input + image bytes output → no page, like
//! image-to-pixel-art / image-color-quantize).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_image_low_poly_core::{low_poly, ColorMode, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_triangles")]
    triangles: u64,
    #[serde(default = "default_edge_focus")]
    edge_focus: u64,
    #[serde(default = "default_color_mode")]
    color_mode: String,
    #[serde(default = "default_stroke")]
    stroke: String,
    #[serde(default)]
    stroke_width: f64,
    #[serde(default = "default_seed")]
    seed: i64,
}
fn default_triangles() -> u64 {
    800
}
fn default_edge_focus() -> u64 {
    60
}
fn default_color_mode() -> String {
    "average".to_string()
}
fn default_stroke() -> String {
    "#1f2937".to_string()
}
fn default_seed() -> i64 {
    1
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("triangles")
                .min(50.0)
                .max(4000.0)
                .default(800)
                .describe("Approximate number of triangles, 50-4000 (default 800). Fewer = bolder and more abstract; more = closer to the original photo."),
        )
        .param(
            Param::integer("edge_focus")
                .min(0.0)
                .max(100.0)
                .default(60)
                .describe("How strongly triangle corners snap to high-contrast edges, 0-100 (default 60). Higher follows contours and calms the random scatter; lower gives a looser, more evenly scattered mesh."),
        )
        .param(
            Param::enumv("color_mode", ["average", "centroid"])
                .default("average")
                .describe("How each triangle picks its flat colour: 'average' (default) averages the source pixels it covers for a smoother result; 'centroid' takes the single pixel at its centre for punchier, higher-contrast facets."),
        )
        .param(
            Param::string("stroke")
                .default("#1f2937")
                .describe("Wireframe colour as #rgb or #rrggbb (default #1f2937). Only drawn when stroke_width is above 0."),
        )
        .param(
            Param::number("stroke_width")
                .min(0.0)
                .max(6.0)
                .default(0)
                .describe("Wireframe line width in pixels, 0-6 (default 0 = no wireframe). Try 1-2 for an outlined poly-art look."),
        )
        .param(
            Param::integer("seed")
                .default(1)
                .describe("Seed for the mesh scatter (default 1). Change it to reshuffle the triangles into a different variation of the same settings."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageLowPoly;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-low-poly",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn an image into low-poly triangle art",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Turn a photo into low-poly triangle art: a seeded triangle mesh is warped onto the image so its corners follow contours, then every triangle is flat-filled with the colour of the source region beneath it. triangles is the approximate triangle count 50-4000 (default 800; fewer = bolder and more abstract, more = closer to the original); edge_focus 0-100 (default 60) pulls mesh corners onto high-contrast edges and damps the random scatter; color_mode is average (default, smooth) or centroid (punchier); stroke and stroke_width draw an optional wireframe outline (width 0-6, default 0 = none); seed reshuffles the mesh into a different variation. Returns a PNG at the original dimensions. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageLowPoly {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-low-poly")?;
    let opts = Options {
        triangles: args.triangles.clamp(50, 4000) as u32,
        edge_focus: args.edge_focus.min(100) as u32,
        color_mode: ColorMode::parse(&args.color_mode).map_err(SkillError::InvalidArgs)?,
        stroke: args.stroke,
        stroke_width: args.stroke_width.clamp(0.0, 6.0) as f32,
        seed: args.seed as u64,
    };
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let png = low_poly(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &png,
        "image/png",
        "low-poly.png".to_string(),
        format!(
            "low-poly art (~{} triangles, edge focus {}, {} colour, {} bytes PNG)",
            opts.triangles,
            opts.edge_focus,
            args.color_mode,
            png.len()
        ),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url":          { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":          { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "triangles":    { "type": "integer", "minimum": 50, "maximum": 4000, "default": 800, "description": "Approximate number of triangles, 50-4000 (default 800). Fewer = bolder and more abstract; more = closer to the original photo." },
                    "edge_focus":   { "type": "integer", "minimum": 0, "maximum": 100, "default": 60, "description": "How strongly triangle corners snap to high-contrast edges, 0-100 (default 60). Higher follows contours and calms the random scatter; lower gives a looser, more evenly scattered mesh." },
                    "color_mode":   { "type": "string", "enum": ["average", "centroid"], "default": "average", "description": "How each triangle picks its flat colour: 'average' (default) averages the source pixels it covers for a smoother result; 'centroid' takes the single pixel at its centre for punchier, higher-contrast facets." },
                    "stroke":       { "type": "string", "default": "#1f2937", "description": "Wireframe colour as #rgb or #rrggbb (default #1f2937). Only drawn when stroke_width is above 0." },
                    "stroke_width": { "type": "number", "minimum": 0, "maximum": 6, "default": 0, "description": "Wireframe line width in pixels, 0-6 (default 0 = no wireframe). Try 1-2 for an outlined poly-art look." },
                    "seed":         { "type": "integer", "default": 1, "description": "Seed for the mesh scatter (default 1). Change it to reshuffle the triangles into a different variation of the same settings." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The serde defaults must match what the schema advertises — a mismatch
    /// means chat sees one default and the handler applies another.
    #[test]
    fn serde_defaults_match_the_schema_defaults() {
        let args: Args = serde_json::from_str(r#"{"url":"https://example.com/a.png"}"#).unwrap();
        assert_eq!(args.triangles, 800);
        assert_eq!(args.edge_focus, 60);
        assert_eq!(args.color_mode, "average");
        assert_eq!(args.stroke, "#1f2937");
        assert_eq!(args.stroke_width, 0.0);
        assert_eq!(args.seed, 1);
    }
}
