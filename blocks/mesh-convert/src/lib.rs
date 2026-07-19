//! gizza-ai/mesh-convert — chat skill block on the shared tool abstraction.
//! Converts 3D meshes between Wavefront OBJ and STL (both directions, ASCII or
//! binary STL). The input format (OBJ vs ASCII STL) is auto-detected from the
//! pasted text. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure → runs on
//! all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mesh_convert_core::{convert, Axis, Options, StlEncoding, Target};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    mesh: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default = "default_stl_encoding")]
    stl_encoding: String,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_axis")]
    axis: String,
    #[serde(default = "default_name")]
    name: String,
}
fn default_to() -> String {
    "stl".to_string()
}
fn default_stl_encoding() -> String {
    "ascii".to_string()
}
fn default_scale() -> f64 {
    1.0
}
fn default_axis() -> String {
    "keep".to_string()
}
fn default_name() -> String {
    "mesh".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("mesh")
                .required()
                .describe(
                    "The 3D mesh to convert, as pasted text: Wavefront OBJ or ASCII STL. The \
                     input format is auto-detected (STL if it starts with 'solid' and contains \
                     'facet'; OBJ if it has v/f lines). Binary STL cannot be pasted as text — \
                     re-export it as ASCII STL first.",
                ),
        )
        .param(
            Param::enumv("to", ["stl", "obj"])
                .default("stl")
                .describe(
                    "Target format. 'stl' (default) writes STL from OBJ or STL input; 'obj' \
                     writes Wavefront OBJ (vertices deduplicated, faces as triangles). OBJ \
                     materials/UVs and STL facet normals are not carried over — STL stores raw \
                     triangles and OBJ normals are recomputed per face on STL output.",
                ),
        )
        .param(
            Param::enumv("stl_encoding", ["ascii", "binary"])
                .default("ascii")
                .describe(
                    "STL byte encoding when to=stl (ignored for to=obj). 'ascii' (default) \
                     returns human-readable STL text; 'binary' returns a compact binary STL as a \
                     data:model/stl;base64 URL you can save as a .stl file.",
                ),
        )
        .param(
            Param::number("scale")
                .default(1.0)
                .describe(
                    "Uniform scale factor applied to every vertex (default 1.0 = unchanged). Use \
                     e.g. 0.001 to convert millimetres to metres, or 25.4 for inches to \
                     millimetres.",
                ),
        )
        .param(
            Param::enumv("axis", ["keep", "y-up-to-z-up", "z-up-to-y-up"])
                .default("keep")
                .describe(
                    "Coordinate-frame reorientation. 'keep' (default) leaves axes as-is; \
                     'y-up-to-z-up' rotates a graphics Y-up model to 3D-print/CAD Z-up; \
                     'z-up-to-y-up' does the reverse.",
                ),
        )
        .param(
            Param::string("name")
                .default("mesh")
                .describe(
                    "Name written into the output: the STL 'solid' name / binary header label, or \
                     the OBJ 'o' object line. Default 'mesh'.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_convert(a: Args) -> Result<String, SkillError> {
    let opt = Options {
        to: Target::parse(&a.to).map_err(SkillError::InvalidArgs)?,
        stl_encoding: StlEncoding::parse(&a.stl_encoding).map_err(SkillError::InvalidArgs)?,
        scale: a.scale,
        axis: Axis::parse(&a.axis).map_err(SkillError::InvalidArgs)?,
        name: a.name,
    };
    convert(&a.mesh, &opt).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mesh-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert 3D meshes between Wavefront OBJ and STL (ASCII or binary)",
    skill(
        description = "Convert 3D meshes between Wavefront OBJ and STL in both directions. The input format is auto-detected from the pasted text (OBJ v/f lines, or ASCII STL solid/facet/vertex). Set to=stl (default) or to=obj to choose the output; stl_encoding=ascii (default) writes readable STL text while stl_encoding=binary returns a compact binary STL as a data:model/stl;base64 URL you can save. Optional scale multiplies every vertex (e.g. 0.001 mm->m), and axis reorients the coordinate frame between graphics Y-up and 3D-print/CAD Z-up. OBJ polygonal faces are fan-triangulated, shared vertices are deduplicated on OBJ output, and STL facet normals are recomputed per face from the geometry. Materials, UVs, textures and vertex colors are not carried over — STL stores only raw triangles. Binary STL input cannot be pasted as text; re-export it as ASCII STL first. Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mesh-convert", run_convert) {
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
                    "mesh": { "type": "string", "description": "The 3D mesh to convert, as pasted text: Wavefront OBJ or ASCII STL. The input format is auto-detected (STL if it starts with 'solid' and contains 'facet'; OBJ if it has v/f lines). Binary STL cannot be pasted as text — re-export it as ASCII STL first." },
                    "to": { "type": "string", "enum": ["stl", "obj"], "default": "stl", "description": "Target format. 'stl' (default) writes STL from OBJ or STL input; 'obj' writes Wavefront OBJ (vertices deduplicated, faces as triangles). OBJ materials/UVs and STL facet normals are not carried over — STL stores raw triangles and OBJ normals are recomputed per face on STL output." },
                    "stl_encoding": { "type": "string", "enum": ["ascii", "binary"], "default": "ascii", "description": "STL byte encoding when to=stl (ignored for to=obj). 'ascii' (default) returns human-readable STL text; 'binary' returns a compact binary STL as a data:model/stl;base64 URL you can save as a .stl file." },
                    "scale": { "type": "number", "default": 1.0, "description": "Uniform scale factor applied to every vertex (default 1.0 = unchanged). Use e.g. 0.001 to convert millimetres to metres, or 25.4 for inches to millimetres." },
                    "axis": { "type": "string", "enum": ["keep", "y-up-to-z-up", "z-up-to-y-up"], "default": "keep", "description": "Coordinate-frame reorientation. 'keep' (default) leaves axes as-is; 'y-up-to-z-up' rotates a graphics Y-up model to 3D-print/CAD Z-up; 'z-up-to-y-up' does the reverse." },
                    "name": { "type": "string", "default": "mesh", "description": "Name written into the output: the STL 'solid' name / binary header label, or the OBJ 'o' object line. Default 'mesh'." }
                },
                "required": ["mesh"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
