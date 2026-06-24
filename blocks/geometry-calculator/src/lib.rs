//! gizza-ai/geometry-calculator — chat skill block on the shared tool abstraction.
//!
//! Computes area + perimeter (2D) or surface area + volume (3D) for common
//! shapes from their dimensions. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`, which shapes `{ "result": <Geometry> }` so the LLM
//! sees the full structured breakdown (shape, dimensionality, echoed dimensions,
//! measures, and a human-readable summary). Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_geometry_calculator_core::Dimensions;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    shape: String,
    #[serde(default)]
    side: Option<f64>,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    length: Option<f64>,
    #[serde(default)]
    radius: Option<f64>,
    #[serde(default)]
    radius_a: Option<f64>,
    #[serde(default)]
    radius_b: Option<f64>,
    #[serde(default)]
    base: Option<f64>,
    #[serde(default)]
    top: Option<f64>,
    #[serde(default)]
    sides: Option<f64>,
    #[serde(default)]
    side_a: Option<f64>,
    #[serde(default)]
    side_b: Option<f64>,
    #[serde(default)]
    side_c: Option<f64>,
}

impl Args {
    fn dimensions(&self) -> Dimensions {
        Dimensions {
            side: self.side,
            width: self.width,
            height: self.height,
            length: self.length,
            radius: self.radius,
            radius_a: self.radius_a,
            radius_b: self.radius_b,
            base: self.base,
            top: self.top,
            sides: self.sides,
            side_a: self.side_a,
            side_b: self.side_b,
            side_c: self.side_c,
        }
    }
}

const SHAPES: [&str; 14] = [
    "square",
    "rectangle",
    "triangle",
    "circle",
    "ellipse",
    "trapezoid",
    "parallelogram",
    "regular_polygon",
    "cube",
    "rectangular_prism",
    "sphere",
    "cylinder",
    "cone",
    "pyramid",
];

/// Single source for the chat schema (and CLI). Every dimension is an optional
/// number; only the ones relevant to the chosen `shape` are read.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("shape", SHAPES).required().describe(
                "The shape to measure. 2D shapes (square, rectangle, triangle, \
                 circle, ellipse, trapezoid, parallelogram, regular_polygon) \
                 return area and perimeter; 3D shapes (cube, rectangular_prism, \
                 sphere, cylinder, cone, pyramid) return surface area and volume.",
            ),
        )
        .param(Param::number("side").describe(
            "Edge length. Used by square, cube, and regular_polygon.",
        ))
        .param(Param::number("width").describe(
            "Width. Used by rectangle and rectangular_prism.",
        ))
        .param(Param::number("height").describe(
            "Height. Used by rectangle, triangle, trapezoid, parallelogram, \
             rectangular_prism, cylinder, cone, and pyramid.",
        ))
        .param(Param::number("length").describe(
            "Length (depth). Used by rectangular_prism.",
        ))
        .param(Param::number("radius").describe(
            "Radius. Used by circle, sphere, cylinder, and cone.",
        ))
        .param(Param::number("radius_a").describe(
            "Semi-major axis (longer radius). Used by ellipse.",
        ))
        .param(Param::number("radius_b").describe(
            "Semi-minor axis (shorter radius). Used by ellipse.",
        ))
        .param(Param::number("base").describe(
            "Base length. Used by triangle, trapezoid, parallelogram, and the \
             square base of a pyramid.",
        ))
        .param(Param::number("top").describe(
            "Top (shorter parallel side). Used by trapezoid.",
        ))
        .param(Param::number("sides").describe(
            "Number of sides (>= 3). Used by regular_polygon.",
        ))
        .param(Param::number("side_a").describe(
            "Side A length. The slant side of a parallelogram; one triangle/\
             trapezoid edge (optional, enables perimeter).",
        ))
        .param(Param::number("side_b").describe(
            "Side B length. A triangle/trapezoid edge (optional, enables \
             perimeter).",
        ))
        .param(Param::number("side_c").describe(
            "Side C length. The third triangle edge (optional, enables \
             perimeter).",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/geometry-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Area, perimeter, surface area, and volume of common shapes",
    skill(
        description = "Calculate area and perimeter for 2D shapes (square, rectangle, triangle, circle, ellipse, trapezoid, parallelogram, regular_polygon) or surface area and volume for 3D shapes (cube, rectangular_prism, sphere, cylinder, cone, pyramid) from their dimensions. Pass the shape plus only the dimensions that shape needs (e.g. circle → radius; rectangle → width, height; cylinder → radius, height). Returns the shape, its dimensionality, the dimensions echoed back, the computed measures with unit suffixes, and a human-readable summary.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "geometry-calculator", |a: Args| {
            gizza_ai_geometry_calculator_core::compute(&a.shape, &a.dimensions())
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

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "shape": { "type": "string", "enum": ["square","rectangle","triangle","circle","ellipse","trapezoid","parallelogram","regular_polygon","cube","rectangular_prism","sphere","cylinder","cone","pyramid"], "description": "The shape to measure. 2D shapes (square, rectangle, triangle, circle, ellipse, trapezoid, parallelogram, regular_polygon) return area and perimeter; 3D shapes (cube, rectangular_prism, sphere, cylinder, cone, pyramid) return surface area and volume." },
                    "side": { "type": "number", "description": "Edge length. Used by square, cube, and regular_polygon." },
                    "width": { "type": "number", "description": "Width. Used by rectangle and rectangular_prism." },
                    "height": { "type": "number", "description": "Height. Used by rectangle, triangle, trapezoid, parallelogram, rectangular_prism, cylinder, cone, and pyramid." },
                    "length": { "type": "number", "description": "Length (depth). Used by rectangular_prism." },
                    "radius": { "type": "number", "description": "Radius. Used by circle, sphere, cylinder, and cone." },
                    "radius_a": { "type": "number", "description": "Semi-major axis (longer radius). Used by ellipse." },
                    "radius_b": { "type": "number", "description": "Semi-minor axis (shorter radius). Used by ellipse." },
                    "base": { "type": "number", "description": "Base length. Used by triangle, trapezoid, parallelogram, and the square base of a pyramid." },
                    "top": { "type": "number", "description": "Top (shorter parallel side). Used by trapezoid." },
                    "sides": { "type": "number", "description": "Number of sides (>= 3). Used by regular_polygon." },
                    "side_a": { "type": "number", "description": "Side A length. The slant side of a parallelogram; one triangle/trapezoid edge (optional, enables perimeter)." },
                    "side_b": { "type": "number", "description": "Side B length. A triangle/trapezoid edge (optional, enables perimeter)." },
                    "side_c": { "type": "number", "description": "Side C length. The third triangle edge (optional, enables perimeter)." }
                },
                "required": ["shape"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
