//! gizza-ai/ply-to-obj — chat skill block on the shared tool abstraction.
//! Converts a Stanford PLY mesh or point cloud (ascii, binary_little_endian or
//! binary_big_endian) into a Wavefront OBJ file, carrying over vertex colors,
//! normals and texture coordinates where OBJ can express them. The chat schema
//! is single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ply_to_obj_core::{convert, Axis, InputFormat, Options, Output};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ply: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_true")]
    colors: bool,
    #[serde(default = "default_true")]
    normals: bool,
    #[serde(default = "default_true")]
    uvs: bool,
    #[serde(default)]
    triangulate: bool,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_axis")]
    axis: String,
    #[serde(default = "default_name")]
    name: String,
    #[serde(default = "default_output")]
    output: String,
}
fn default_input_format() -> String {
    "auto".to_string()
}
fn default_true() -> bool {
    true
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
fn default_output() -> String {
    "obj".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("ply")
                .required()
                .describe(
                    "The PLY file to convert. An ASCII PLY is pasted as text (it starts with the \
                     word 'ply' on its own line). A BINARY PLY is not text — paste its bytes as \
                     base64 or hex instead (a data:…;base64,… URL works too). All three PLY \
                     encodings are read: ascii, binary_little_endian and binary_big_endian. Up to \
                     200000 vertices and 200000 faces.",
                ),
        )
        .param(
            Param::enumv("input_format", ["auto", "ascii", "base64", "hex"])
                .default("auto")
                .describe(
                    "How the pasted input is encoded. 'auto' (default) detects it: text starting \
                     with 'ply' is read as ASCII PLY, otherwise the input is decoded as hex, then \
                     as base64, and must yield bytes starting with the PLY magic word. Set \
                     'ascii', 'base64' or 'hex' to force one.",
                ),
        )
        .param(
            Param::boolean("colors")
                .default(true)
                .describe(
                    "Carry per-vertex colors over when the PLY has red/green/blue properties \
                     (default true). They are written as the widely-supported OBJ extension \
                     'v x y z r g b' with r/g/b in 0..1, which MeshLab, CloudCompare and Blender \
                     read. Set false for a strict 'v x y z' OBJ. Integer color channels (uchar \
                     0..255) are divided by 255; float channels are used as-is.",
                ),
        )
        .param(
            Param::boolean("normals")
                .default(true)
                .describe(
                    "Carry per-vertex normals over when the PLY has nx/ny/nz properties (default \
                     true), written as 'vn' lines with faces referencing them as 'f v//vn' (or \
                     'f v/vt/vn' when texture coordinates are kept too). Set false to omit them.",
                ),
        )
        .param(
            Param::boolean("uvs")
                .default(true)
                .describe(
                    "Carry per-vertex texture coordinates over when the PLY has s/t, u/v or \
                     texture_u/texture_v properties (default true), written as 'vt' lines with \
                     faces referencing them as 'f v/vt'. Set false to omit them. No .mtl material \
                     file is written — PLY stores no material definitions.",
                ),
        )
        .param(
            Param::boolean("triangulate")
                .default(false)
                .describe(
                    "Split faces with more than 3 corners into triangles by fan triangulation. \
                     Default false keeps them as OBJ n-gons, which OBJ supports natively and is \
                     the faithful conversion. Set true for 3D printers, game engines or slicers \
                     that only accept triangles.",
                ),
        )
        .param(
            Param::number("scale")
                .default(1.0)
                .describe(
                    "Uniform scale factor applied to every vertex position (default 1.0 = \
                     unchanged). Use e.g. 0.001 to convert millimetres to metres, or 25.4 for \
                     inches to millimetres. Normals are direction vectors and are not scaled.",
                ),
        )
        .param(
            Param::enumv("axis", ["keep", "y-up-to-z-up", "z-up-to-y-up"])
                .default("keep")
                .describe(
                    "Coordinate-frame reorientation. 'keep' (default) leaves axes as-is; \
                     'y-up-to-z-up' rotates a graphics Y-up model to 3D-print/CAD Z-up; \
                     'z-up-to-y-up' does the reverse. Positions and normals rotate together.",
                ),
        )
        .param(
            Param::string("name")
                .default("mesh")
                .describe(
                    "Object name written on the OBJ 'o' line. Default 'mesh'. Use it to label a \
                     scan or part, e.g. 'scan_01'.",
                ),
        )
        .param(
            Param::enumv("output", ["obj", "summary"])
                .default("obj")
                .describe(
                    "What to return. 'obj' (default) is the converted Wavefront OBJ text. \
                     'summary' is a report about the source file instead: its PLY encoding, \
                     whether it is a mesh or a point cloud, vertex/face/polygon counts, which of \
                     colors/normals/texture coordinates it declared and which were written, every \
                     header element, and the vertex properties that were ignored.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_convert(a: Args) -> Result<String, SkillError> {
    let opt = Options {
        input_format: InputFormat::parse(&a.input_format).map_err(SkillError::InvalidArgs)?,
        colors: a.colors,
        normals: a.normals,
        uvs: a.uvs,
        triangulate: a.triangulate,
        scale: a.scale,
        axis: Axis::parse(&a.axis).map_err(SkillError::InvalidArgs)?,
        name: a.name,
        output: Output::parse(&a.output).map_err(SkillError::InvalidArgs)?,
    };
    convert(&a.ply, &opt).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ply-to-obj",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a PLY mesh or point cloud (ascii or binary) into a Wavefront OBJ file",
    skill(
        description = "Convert a Stanford PLY mesh or point cloud into a Wavefront OBJ file. All three PLY encodings are read: ascii, binary_little_endian and binary_big_endian. Paste an ASCII PLY as text; a binary PLY is not text, so paste its bytes as base64 or hex (a data:…;base64,… URL works too) and input_format=auto will detect it. Per-vertex extras are carried over when the file declares them: colors become the OBJ 'v x y z r g b' extension with r/g/b in 0..1 (uchar 0..255 channels are divided by 255), normals become 'vn' lines and s/t, u/v or texture_u/texture_v become 'vt' lines, with faces written as f v/vt/vn accordingly; set colors, normals or uvs to false to drop any of them. A PLY with no face element is a point cloud and emits 'v' lines only. Faces keep their original corner count as OBJ n-gons unless triangulate=true fan-triangulates them for printers and engines that need triangles. Unknown elements (edges, materials, scanner data) and unmapped vertex properties are skipped without desynchronising the file. scale multiplies every vertex position (e.g. 0.001 mm->m) and axis reorients between graphics Y-up and 3D-print/CAD Z-up. output=summary returns a report on the source file — encoding, mesh vs point cloud, counts, which properties were present and written, and what was ignored — instead of the OBJ. No .mtl material file is written (PLY stores no materials) and per-vertex alpha is dropped (OBJ has no alpha channel). Limits: 200000 vertices and 200000 faces. Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ply-to-obj", run_convert) {
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
                    "ply": { "type": "string", "description": "The PLY file to convert. An ASCII PLY is pasted as text (it starts with the word 'ply' on its own line). A BINARY PLY is not text — paste its bytes as base64 or hex instead (a data:…;base64,… URL works too). All three PLY encodings are read: ascii, binary_little_endian and binary_big_endian. Up to 200000 vertices and 200000 faces." },
                    "input_format": { "type": "string", "enum": ["auto", "ascii", "base64", "hex"], "default": "auto", "description": "How the pasted input is encoded. 'auto' (default) detects it: text starting with 'ply' is read as ASCII PLY, otherwise the input is decoded as hex, then as base64, and must yield bytes starting with the PLY magic word. Set 'ascii', 'base64' or 'hex' to force one." },
                    "colors": { "type": "boolean", "default": true, "description": "Carry per-vertex colors over when the PLY has red/green/blue properties (default true). They are written as the widely-supported OBJ extension 'v x y z r g b' with r/g/b in 0..1, which MeshLab, CloudCompare and Blender read. Set false for a strict 'v x y z' OBJ. Integer color channels (uchar 0..255) are divided by 255; float channels are used as-is." },
                    "normals": { "type": "boolean", "default": true, "description": "Carry per-vertex normals over when the PLY has nx/ny/nz properties (default true), written as 'vn' lines with faces referencing them as 'f v//vn' (or 'f v/vt/vn' when texture coordinates are kept too). Set false to omit them." },
                    "uvs": { "type": "boolean", "default": true, "description": "Carry per-vertex texture coordinates over when the PLY has s/t, u/v or texture_u/texture_v properties (default true), written as 'vt' lines with faces referencing them as 'f v/vt'. Set false to omit them. No .mtl material file is written — PLY stores no material definitions." },
                    "triangulate": { "type": "boolean", "default": false, "description": "Split faces with more than 3 corners into triangles by fan triangulation. Default false keeps them as OBJ n-gons, which OBJ supports natively and is the faithful conversion. Set true for 3D printers, game engines or slicers that only accept triangles." },
                    "scale": { "type": "number", "default": 1.0, "description": "Uniform scale factor applied to every vertex position (default 1.0 = unchanged). Use e.g. 0.001 to convert millimetres to metres, or 25.4 for inches to millimetres. Normals are direction vectors and are not scaled." },
                    "axis": { "type": "string", "enum": ["keep", "y-up-to-z-up", "z-up-to-y-up"], "default": "keep", "description": "Coordinate-frame reorientation. 'keep' (default) leaves axes as-is; 'y-up-to-z-up' rotates a graphics Y-up model to 3D-print/CAD Z-up; 'z-up-to-y-up' does the reverse. Positions and normals rotate together." },
                    "name": { "type": "string", "default": "mesh", "description": "Object name written on the OBJ 'o' line. Default 'mesh'. Use it to label a scan or part, e.g. 'scan_01'." },
                    "output": { "type": "string", "enum": ["obj", "summary"], "default": "obj", "description": "What to return. 'obj' (default) is the converted Wavefront OBJ text. 'summary' is a report about the source file instead: its PLY encoding, whether it is a mesh or a point cloud, vertex/face/polygon counts, which of colors/normals/texture coordinates it declared and which were written, every header element, and the vertex properties that were ignored." }
                },
                "required": ["ply"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The block-level wiring: args in → OBJ out, and a bad enum is rejected.
    #[test]
    fn run_convert_wires_args_to_core() {
        let args = Args {
            ply: "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n".to_string(),
            input_format: "auto".to_string(),
            colors: true,
            normals: true,
            uvs: true,
            triangulate: false,
            scale: 1.0,
            axis: "keep".to_string(),
            name: "tri".to_string(),
            output: "obj".to_string(),
        };
        let out = run_convert(args).unwrap();
        assert!(out.contains("o tri\n"), "got: {out}");
        assert!(out.contains("f 1 2 3"), "got: {out}");
    }

    #[test]
    fn run_convert_rejects_an_unknown_axis() {
        let args = Args {
            ply: "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 0 0\n".to_string(),
            input_format: "auto".to_string(),
            colors: true,
            normals: true,
            uvs: true,
            triangulate: false,
            scale: 1.0,
            axis: "sideways".to_string(),
            name: "mesh".to_string(),
            output: "obj".to_string(),
        };
        assert!(run_convert(args).is_err());
    }
}
