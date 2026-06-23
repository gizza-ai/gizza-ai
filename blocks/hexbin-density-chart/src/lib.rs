//! gizza-ai/hexbin-density-chart — bin x/y point data into a hexagonal grid and
//! render the per-hex density as a heatmap, output as SVG or PNG.
//!
//! Pure-Rust (core has no wafer/wasm-bindgen deps; PNG is rasterized with
//! resvg/tiny-skia), so it runs on ALL backends including the chat Service
//! Worker. The image is wrapped as an image/svg+xml | image/png data-URL
//! envelope so the chat UI shows the chart. Surfaces: chat + CLI (no standalone
//! page — a pure-Rust image-bytes output has no page mode, like scatter-chart).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_hexbin_density_chart_core::{render, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    data: String,
    #[serde(default)]
    title: String,
    #[serde(default = "default_w")]
    width: u32,
    #[serde(default = "default_h")]
    height: u32,
    #[serde(default = "default_radius")]
    radius: f64,
    #[serde(default)]
    format: String,
}
fn default_w() -> u32 {
    700
}
fn default_h() -> u32 {
    500
}
fn default_radius() -> f64 {
    18.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "Points as a JSON array. Each point is either [x, y] or an object {\"x\":..,\"y\":..}. The points are dropped into a hexagonal grid and each hexagon is coloured by how many points fall in it. E.g. [[1,2],[3,4],[2,2]] or [{\"x\":1,\"y\":2}].",
        ))
        .param(Param::string("title").default("").describe("Optional chart title."))
        .param(Param::integer("width").default(700).min(200.0).describe("Image width in pixels (default 700)."))
        .param(Param::integer("height").default(500).min(150.0).describe("Image height in pixels (default 500)."))
        .param(
            Param::number("radius")
                .default(18.0)
                .min(4.0)
                .max(120.0)
                .describe("Hexagon radius in pixels (default 18). Larger = coarser bins (fewer, bigger hexagons); smaller = finer bins."),
        )
        .param(
            Param::enumv("format", ["svg", "png"])
                .default("svg")
                .describe("Output image format: svg (default, scalable vector) or png (raster)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HexbinDensityChart;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hexbin-density-chart",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a hexagonal-bin density heatmap from x/y points",
    skill(
        description = "Render a hexagonal-bin density heatmap from x/y point data as an SVG or PNG chart. Points are a JSON array of [x, y] pairs or {x, y} objects; they are aggregated into a flat-top hexagonal grid and each occupied hexagon is filled by its point count using a perceptual (Viridis-like) sequential colour scale, with axes (min/mid/max ticks) and a count legend. Useful for visualising the density of large/overlapping scatter data where individual points would overplot. Optional title, width, height, hexagon radius (bin size), and format (svg or png). Returns an image.",
        parameters = schema_json()
    ),
)]
impl HexbinDensityChart {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("hexbin-density-chart")?;
    let fmt = OutputFormat::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let bytes = render(&args.data, &args.title, args.width, args.height, args.radius, fmt)
        .map_err(SkillError::InvalidArgs)?;
    let title = if args.title.is_empty() {
        "hexbin".to_string()
    } else {
        args.title.clone()
    };
    build_media_envelope(
        &bytes,
        fmt.mime(),
        format!("{}.{}", title.replace(['/', '\\', ' '], "-"), fmt.extension()),
        format!("rendered a hexbin density chart ({} bytes {})", bytes.len(), fmt.extension().to_uppercase()),
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
                    "data":   { "type": "string", "description": "Points as a JSON array. Each point is either [x, y] or an object {\"x\":..,\"y\":..}. The points are dropped into a hexagonal grid and each hexagon is coloured by how many points fall in it. E.g. [[1,2],[3,4],[2,2]] or [{\"x\":1,\"y\":2}]." },
                    "title":  { "type": "string", "default": "", "description": "Optional chart title." },
                    "width":  { "type": "integer", "default": 700, "minimum": 200, "description": "Image width in pixels (default 700)." },
                    "height": { "type": "integer", "default": 500, "minimum": 150, "description": "Image height in pixels (default 500)." },
                    "radius": { "type": "number", "default": 18.0, "minimum": 4, "maximum": 120, "description": "Hexagon radius in pixels (default 18). Larger = coarser bins (fewer, bigger hexagons); smaller = finer bins." },
                    "format": { "type": "string", "enum": ["svg", "png"], "default": "svg", "description": "Output image format: svg (default, scalable vector) or png (raster)." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
