//! gizza-ai/stl-repair — chat skill block on the shared tool abstraction.
//! Diagnoses and repairs a triangle mesh pasted as ASCII STL (or Wavefront OBJ)
//! text: welds coincident vertices, drops zero-area and duplicate triangles,
//! harmonises winding so normals point outward, optionally fan-fills open
//! boundary loops and drops stray fragments, then reports watertightness.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_stl_repair_core::{repair, Options, Output, StlEncoding};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    stl: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_weld_tolerance")]
    weld_tolerance: f64,
    #[serde(default = "default_true")]
    remove_degenerate: bool,
    #[serde(default = "default_true")]
    remove_duplicates: bool,
    #[serde(default = "default_true")]
    fix_winding: bool,
    #[serde(default)]
    fill_holes: bool,
    #[serde(default)]
    keep_largest_shell: bool,
    #[serde(default = "default_stl_encoding")]
    stl_encoding: String,
}
fn default_output() -> String {
    "report".to_string()
}
fn default_weld_tolerance() -> f64 {
    0.000001
}
fn default_true() -> bool {
    true
}
fn default_stl_encoding() -> String {
    "ascii".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("stl")
                .required()
                .describe(
                    "The mesh to check and repair, as pasted text: ASCII STL (solid / facet / \
                     vertex lines) or Wavefront OBJ (v / f lines). The format is auto-detected. \
                     Binary STL cannot be pasted as text — re-export it as ASCII STL first. Up to \
                     100000 triangles.",
                ),
        )
        .param(
            Param::enumv("output", ["report", "stl", "json"])
                .default("report")
                .describe(
                    "What to return. 'report' (default) is a readable diagnosis: problems found, \
                     repairs applied, and the resulting triangle/vertex counts, watertightness, \
                     area, volume and bounding box. 'stl' returns the repaired mesh itself \
                     (see stl_encoding). 'json' returns the same numbers as the report in a \
                     machine-readable object.",
                ),
        )
        .param(
            Param::number("weld_tolerance")
                .default(0.000001)
                .min(0.0)
                .max(1000.0)
                .describe(
                    "Distance below which two corners are treated as the same vertex, in the \
                     mesh's own units (default 0.000001). STL stores every triangle's corners \
                     separately, so welding is what turns loose facets into a connected surface. \
                     Raise it (e.g. 0.001) to close hairline cracks from a sloppy export; set 0 \
                     to merge only bit-identical positions.",
                ),
        )
        .param(
            Param::boolean("remove_degenerate")
                .default(true)
                .describe(
                    "Drop zero-area triangles — ones whose corners weld to the same vertex or lie \
                     exactly on a straight line. Default true. These carry no surface and make \
                     slicers and boolean operations misbehave.",
                ),
        )
        .param(
            Param::boolean("remove_duplicates")
                .default(true)
                .describe(
                    "Drop repeated triangles — faces built from the same three vertices in any \
                     rotation or winding, keeping the first. Default true. Duplicates show up as \
                     non-manifold edges and doubled walls.",
                ),
        )
        .param(
            Param::boolean("fix_winding")
                .default(true)
                .describe(
                    "Make every face in a shell agree on winding, then turn each closed shell so \
                     its recomputed normals point outward. Default true. This is what fixes \
                     flipped/inverted normals; facet normals are always recomputed from the \
                     geometry on export, so the normals in the input file are ignored.",
                ),
        )
        .param(
            Param::boolean("fill_holes")
                .default(false)
                .describe(
                    "Close open boundary loops by fanning triangles from each loop's centroid. \
                     Default false. This closes simple, roughly flat holes; it is not a \
                     curvature-aware patch, so a large or twisted hole gets a visibly flat cap. \
                     Boundary chains that do not close are left alone rather than patched wrongly.",
                ),
        )
        .param(
            Param::boolean("keep_largest_shell")
                .default(false)
                .describe(
                    "Keep only the connected shell with the most triangles and drop the rest. \
                     Default false. Use it to strip stray fragments and scanner debris — but note \
                     it also discards legitimately multi-part models.",
                ),
        )
        .param(
            Param::enumv("stl_encoding", ["ascii", "binary"])
                .default("ascii")
                .describe(
                    "Byte encoding when output=stl (ignored otherwise). 'ascii' (default) returns \
                     readable STL text; 'binary' returns a compact binary STL as a \
                     data:model/stl;base64 URL you can save as a .stl file.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_repair(a: Args) -> Result<String, SkillError> {
    let opt = Options {
        output: Output::parse(&a.output).map_err(SkillError::InvalidArgs)?,
        weld_tolerance: a.weld_tolerance,
        remove_degenerate: a.remove_degenerate,
        remove_duplicates: a.remove_duplicates,
        fix_winding: a.fix_winding,
        fill_holes: a.fill_holes,
        keep_largest_shell: a.keep_largest_shell,
        stl_encoding: StlEncoding::parse(&a.stl_encoding).map_err(SkillError::InvalidArgs)?,
    };
    repair(&a.stl, &opt).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/stl-repair",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check and repair an STL mesh: weld vertices, drop bad triangles, fix normals, report watertightness",
    skill(
        description = "Check and repair a 3D triangle mesh pasted as ASCII STL or Wavefront OBJ text. It welds coincident vertices (weld_tolerance, default 0.000001), removes zero-area degenerate triangles and duplicate faces, harmonises triangle winding and turns each closed shell so its normals point outward, and can optionally fan-fill open boundary loops (fill_holes) and drop stray fragments (keep_largest_shell). Set output=report (default) for a readable diagnosis of what was wrong and what was fixed — degenerate/duplicate/coincident counts, non-manifold and open edges, flipped faces, shell count, watertight yes/no before and after, plus surface area, volume (only when the result is closed) and bounding box; output=json for the same numbers as an object; output=stl for the repaired mesh, as ASCII text or, with stl_encoding=binary, a data:model/stl;base64 URL you can save. Facet normals are always recomputed from the geometry, so the normals stored in the input file are ignored. Self-intersection repair, remeshing and curvature-aware hole patching are out of scope, and binary STL cannot be pasted as text — re-export it as ASCII STL first. Limit 100000 triangles. Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "stl-repair", run_repair) {
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
                    "stl": { "type": "string", "description": "The mesh to check and repair, as pasted text: ASCII STL (solid / facet / vertex lines) or Wavefront OBJ (v / f lines). The format is auto-detected. Binary STL cannot be pasted as text — re-export it as ASCII STL first. Up to 100000 triangles." },
                    "output": { "type": "string", "enum": ["report", "stl", "json"], "default": "report", "description": "What to return. 'report' (default) is a readable diagnosis: problems found, repairs applied, and the resulting triangle/vertex counts, watertightness, area, volume and bounding box. 'stl' returns the repaired mesh itself (see stl_encoding). 'json' returns the same numbers as the report in a machine-readable object." },
                    "weld_tolerance": { "type": "number", "default": 0.000001, "minimum": 0, "maximum": 1000, "description": "Distance below which two corners are treated as the same vertex, in the mesh's own units (default 0.000001). STL stores every triangle's corners separately, so welding is what turns loose facets into a connected surface. Raise it (e.g. 0.001) to close hairline cracks from a sloppy export; set 0 to merge only bit-identical positions." },
                    "remove_degenerate": { "type": "boolean", "default": true, "description": "Drop zero-area triangles — ones whose corners weld to the same vertex or lie exactly on a straight line. Default true. These carry no surface and make slicers and boolean operations misbehave." },
                    "remove_duplicates": { "type": "boolean", "default": true, "description": "Drop repeated triangles — faces built from the same three vertices in any rotation or winding, keeping the first. Default true. Duplicates show up as non-manifold edges and doubled walls." },
                    "fix_winding": { "type": "boolean", "default": true, "description": "Make every face in a shell agree on winding, then turn each closed shell so its recomputed normals point outward. Default true. This is what fixes flipped/inverted normals; facet normals are always recomputed from the geometry on export, so the normals in the input file are ignored." },
                    "fill_holes": { "type": "boolean", "default": false, "description": "Close open boundary loops by fanning triangles from each loop's centroid. Default false. This closes simple, roughly flat holes; it is not a curvature-aware patch, so a large or twisted hole gets a visibly flat cap. Boundary chains that do not close are left alone rather than patched wrongly." },
                    "keep_largest_shell": { "type": "boolean", "default": false, "description": "Keep only the connected shell with the most triangles and drop the rest. Default false. Use it to strip stray fragments and scanner debris — but note it also discards legitimately multi-part models." },
                    "stl_encoding": { "type": "string", "enum": ["ascii", "binary"], "default": "ascii", "description": "Byte encoding when output=stl (ignored otherwise). 'ascii' (default) returns readable STL text; 'binary' returns a compact binary STL as a data:model/stl;base64 URL you can save as a .stl file." }
                },
                "required": ["stl"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The block-level wiring (Args defaults → core Options) must produce the
    /// same repair the page and CLI get from the defaults.
    #[test]
    fn defaults_run_a_full_repair() {
        let stl = "solid t\n facet normal 0 0 0\n  outer loop\n   vertex 0 0 0\n   vertex 1 0 0\n   vertex 0 1 0\n  endloop\n endfacet\nendsolid t\n";
        let out = run_repair(Args {
            stl: stl.to_string(),
            output: default_output(),
            weld_tolerance: default_weld_tolerance(),
            remove_degenerate: true,
            remove_duplicates: true,
            fix_winding: true,
            fill_holes: false,
            keep_largest_shell: false,
            stl_encoding: default_stl_encoding(),
        })
        .unwrap();
        assert!(out.contains("STL repair report"));
        assert!(out.contains("ASCII STL"));
    }

    #[test]
    fn a_bad_enum_is_an_invalid_args_error() {
        let err = run_repair(Args {
            stl: "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n".to_string(),
            output: "pdf".to_string(),
            weld_tolerance: default_weld_tolerance(),
            remove_degenerate: true,
            remove_duplicates: true,
            fix_winding: true,
            fill_holes: false,
            keep_largest_shell: false,
            stl_encoding: default_stl_encoding(),
        })
        .unwrap_err();
        assert!(matches!(err, SkillError::InvalidArgs(_)));
    }
}
