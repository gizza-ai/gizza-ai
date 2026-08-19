//! gizza-ai/stl-inspector — chat skill block on the shared tool abstraction.
//! Read-only inspection of a triangle mesh given as ASCII STL text or as binary
//! STL bytes pasted in base64/hex: counts, bounding box, surface area, signed and
//! absolute volume, watertight/manifold status, edge topology, and how the stored
//! facet normals compare with the geometry. Repairs live in `blocks/stl-repair`.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_stl_inspector_core::{inspect, InputFormat, Options, Output, Units};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    stl: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_units")]
    units: String,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default)]
    density: f64,
    #[serde(default = "default_weld_tolerance")]
    weld_tolerance: f64,
}
fn default_input_format() -> String {
    "auto".to_string()
}
fn default_output() -> String {
    "report".to_string()
}
fn default_units() -> String {
    "mm".to_string()
}
fn default_scale() -> f64 {
    1.0
}
fn default_weld_tolerance() -> f64 {
    0.000001
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("stl")
                .required()
                .describe(
                    "The mesh to inspect. Paste either an ASCII STL as text (solid / facet \
                     normal / vertex lines), or a BINARY STL's bytes encoded as base64 or hex \
                     (hex may include spaces, colons or dashes). The encoding is auto-detected \
                     by default. Up to 100000 triangles.",
                ),
        )
        .param(
            Param::enumv("input_format", ["auto", "ascii", "base64", "hex"])
                .default("auto")
                .describe(
                    "How the pasted value is encoded. 'auto' (default) detects ASCII STL text, \
                     hex bytes or base64 bytes; binary STL is recognised by the 84 + 50 x \
                     triangle-count byte length rather than the leading 'solid' keyword, which \
                     many exporters also write into a binary header. Set 'ascii', 'base64' or \
                     'hex' to force one.",
                ),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe(
                    "What to return. 'report' (default) is a readable inspection: input \
                     encoding, geometry (triangles, distinct vertices, bounding box, bounds, \
                     centre, surface area, signed and absolute volume), mesh integrity \
                     (watertight, manifold, boundary/non-manifold/inconsistent edges, shells, \
                     degenerate and duplicate triangles, normal mismatches) and a print-ready \
                     verdict. 'json' returns the same numbers as a machine-readable object.",
                ),
        )
        .param(
            Param::enumv("units", ["mm", "cm", "in"])
                .default("mm")
                .describe(
                    "The unit the mesh's raw coordinates are taken to be in: 'mm' (default, the \
                     3D-printing convention), 'cm' or 'in'. STL files carry no units, so this \
                     only labels the output — lengths use this unit, areas its square, volumes \
                     its cube — and converts the volume to cm³ for the weight estimate.",
                ),
        )
        .param(
            Param::number("scale")
                .default(1.0)
                .min(0.000001)
                .max(1000000.0)
                .describe(
                    "Multiplier applied to every coordinate before measuring (default 1, i.e. \
                     no change). Use it to see the numbers for a resized print — 2 doubles each \
                     dimension (8x the volume), and 25.4 converts a model authored in inches to \
                     millimetres.",
                ),
        )
        .param(
            Param::number("density")
                .default(0.0)
                .min(0.0)
                .max(100000.0)
                .describe(
                    "Material density in g/cm³ used to estimate the solid part's mass. Default \
                     0, which omits the weight line. Common values: PLA 1.24, PETG 1.27, ABS \
                     1.04, nylon 1.14, standard resin 1.10. The estimate is for a 100%-solid \
                     part — infill, walls and supports are not modelled.",
                ),
        )
        .param(
            Param::number("weld_tolerance")
                .default(0.000001)
                .min(0.0)
                .max(1000.0)
                .describe(
                    "Distance below which two corners are counted as the same vertex, in the \
                     mesh's own units (default 0.000001). STL stores each triangle's corners \
                     separately, so welding is what turns loose facets into a connected surface \
                     — it drives the vertex count, edge topology, shells and the watertight \
                     verdict. Raise it (e.g. 0.001) to see whether hairline export cracks are \
                     the only thing keeping a mesh open; set 0 to merge only bit-identical \
                     positions.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_inspect(a: Args) -> Result<String, SkillError> {
    let opt = Options {
        input_format: InputFormat::parse(&a.input_format).map_err(SkillError::InvalidArgs)?,
        output: Output::parse(&a.output).map_err(SkillError::InvalidArgs)?,
        units: Units::parse(&a.units).map_err(SkillError::InvalidArgs)?,
        scale: a.scale,
        density: a.density,
        weld_tolerance: a.weld_tolerance,
    };
    inspect(&a.stl, &opt).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/stl-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect an ASCII or binary STL: triangle count, bounding box, area, volume and whether it is watertight",
    skill(
        description = "Inspect a 3D triangle mesh given as an ASCII STL pasted as text, or a BINARY STL whose bytes are pasted as base64 or hex (the encoding is auto-detected; binary is identified by its 84 + 50 x triangle-count length, not by the leading 'solid' keyword many binary headers also carry). Reports, without modifying the mesh: triangle count, distinct welded vertices (weld_tolerance, default 0.000001), bounding box, min/max bounds and centre, surface area, signed volume (its sign shows whether normals face outward) and absolute volume, watertight and manifold status, open boundary edges, non-manifold edges, edges whose two faces disagree on winding, disconnected shells, degenerate and duplicate triangles, facet normals that disagree with the geometry by more than 1 degree, unset (0,0,0) normals, non-zero attribute bytes, and a print-ready verdict listing the specific problems. Set units=mm (default), cm or in to label the measurements, scale to measure a resized copy, and density (g/cm³, e.g. PLA 1.24) to add a solid-part weight estimate. output=report (default) is readable; output=json returns the same numbers as an object. This tool only reports — use the stl-repair tool to weld, re-wind, fill holes or clean a mesh. OBJ, PLY, 3MF, STEP and glTF input are not accepted. Limit 100000 triangles. Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "stl-inspector", run_inspect) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "stl": { "type": "string", "description": "The mesh to inspect. Paste either an ASCII STL as text (solid / facet normal / vertex lines), or a BINARY STL's bytes encoded as base64 or hex (hex may include spaces, colons or dashes). The encoding is auto-detected by default. Up to 100000 triangles." },
                    "input_format": { "type": "string", "enum": ["auto", "ascii", "base64", "hex"], "default": "auto", "description": "How the pasted value is encoded. 'auto' (default) detects ASCII STL text, hex bytes or base64 bytes; binary STL is recognised by the 84 + 50 x triangle-count byte length rather than the leading 'solid' keyword, which many exporters also write into a binary header. Set 'ascii', 'base64' or 'hex' to force one." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "What to return. 'report' (default) is a readable inspection: input encoding, geometry (triangles, distinct vertices, bounding box, bounds, centre, surface area, signed and absolute volume), mesh integrity (watertight, manifold, boundary/non-manifold/inconsistent edges, shells, degenerate and duplicate triangles, normal mismatches) and a print-ready verdict. 'json' returns the same numbers as a machine-readable object." },
                    "units": { "type": "string", "enum": ["mm", "cm", "in"], "default": "mm", "description": "The unit the mesh's raw coordinates are taken to be in: 'mm' (default, the 3D-printing convention), 'cm' or 'in'. STL files carry no units, so this only labels the output — lengths use this unit, areas its square, volumes its cube — and converts the volume to cm³ for the weight estimate." },
                    "scale": { "type": "number", "default": 1.0, "minimum": 0.000001, "maximum": 1000000, "description": "Multiplier applied to every coordinate before measuring (default 1, i.e. no change). Use it to see the numbers for a resized print — 2 doubles each dimension (8x the volume), and 25.4 converts a model authored in inches to millimetres." },
                    "density": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 100000, "description": "Material density in g/cm³ used to estimate the solid part's mass. Default 0, which omits the weight line. Common values: PLA 1.24, PETG 1.27, ABS 1.04, nylon 1.14, standard resin 1.10. The estimate is for a 100%-solid part — infill, walls and supports are not modelled." },
                    "weld_tolerance": { "type": "number", "default": 0.000001, "minimum": 0, "maximum": 1000, "description": "Distance below which two corners are counted as the same vertex, in the mesh's own units (default 0.000001). STL stores each triangle's corners separately, so welding is what turns loose facets into a connected surface — it drives the vertex count, edge topology, shells and the watertight verdict. Raise it (e.g. 0.001) to see whether hairline export cracks are the only thing keeping a mesh open; set 0 to merge only bit-identical positions." }
                },
                "required": ["stl"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(live, authored);
    }

    #[test]
    fn every_param_is_described() {
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = live["properties"].as_object().unwrap();
        assert_eq!(props.len(), 7);
        for (name, p) in props {
            let d = p["description"].as_str().unwrap_or("");
            assert!(d.len() > 40, "param '{name}' needs a real description");
        }
    }

    #[test]
    fn run_inspect_reports_a_cube() {
        let stl = "solid c\n facet normal 0 0 -1\n outer loop\n vertex 0 0 0\n vertex 0 1 0\n \
                   vertex 1 0 0\n endloop\n endfacet\nendsolid c\n";
        let out = run_inspect(Args {
            stl: stl.to_string(),
            input_format: default_input_format(),
            output: default_output(),
            units: default_units(),
            scale: default_scale(),
            density: 0.0,
            weld_tolerance: default_weld_tolerance(),
        })
        .unwrap();
        assert!(out.contains("Triangles"));
        assert!(out.contains("Watertight"));
    }

    #[test]
    fn run_inspect_surfaces_parse_errors() {
        let err = run_inspect(Args {
            stl: "not a mesh".to_string(),
            input_format: "ascii".to_string(),
            output: default_output(),
            units: default_units(),
            scale: default_scale(),
            density: 0.0,
            weld_tolerance: default_weld_tolerance(),
        })
        .unwrap_err();
        assert!(format!("{err:?}").contains("no facets found"));
    }
}
