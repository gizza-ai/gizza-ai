//! gizza-ai/obj-vertices-to-csv — chat skill block on the shared tool abstraction.
//! Extract the vertex (`v`) coordinates of a Wavefront OBJ document into a flat
//! x,y,z CSV, one row per vertex. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI and, via manifest.json, the page
//! form); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_obj_vertices_to_csv_core::convert_str;
use serde::Deserialize;
use wafer_sdk::*;

fn default_columns() -> String {
    "xyz".into()
}
fn default_color() -> String {
    "auto".into()
}
fn default_up_axis() -> String {
    "keep".into()
}
fn default_scale() -> f64 {
    1.0
}
fn default_precision() -> f64 {
    -1.0
}
fn default_dedupe() -> String {
    "none".into()
}
fn default_delimiter() -> String {
    "comma".into()
}
fn default_header() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    obj: String,
    #[serde(default = "default_columns")]
    columns: String,
    #[serde(default = "default_color")]
    color: String,
    #[serde(default = "default_up_axis")]
    up_axis: String,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_precision")]
    precision: f64,
    #[serde(default = "default_dedupe")]
    dedupe: String,
    #[serde(default)]
    objects: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_header")]
    header: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("obj").required().describe(
            "Wavefront OBJ text to extract vertices from. Every `v x y z` line becomes ONE CSV \
             row, in file order — a 5000-vertex mesh gives 5000 rows. The extended `v x y z r g \
             b` form (per-vertex colours) is understood and an optional 4th `w` value is ignored. \
             Faces (`f`), texture coordinates (`vt`), normals (`vn`), parameter-space vertices \
             (`vp`) and material libraries are skipped — nothing is resampled. Up to 16 MiB.",
        ))
        .param(
            Param::enumv("columns", ["xyz", "indexed", "object", "full"])
                .default("xyz")
                .describe(
                    "Context columns written before the coordinates: 'xyz' (default) just the \
                     coordinate columns; 'indexed' adds `index`, the vertex's 1-based OBJ index \
                     (the number `f` lines reference, kept even when rows are filtered or \
                     deduped); 'object' adds `index`, `object` (the enclosing `o` name) and \
                     `group` (the enclosing `g` names); 'full' additionally adds `material` (the \
                     active `usemtl`).",
                ),
        )
        .param(
            Param::enumv("color", ["auto", "always", "never"])
                .default("auto")
                .describe(
                    "When the per-vertex `red,green,blue` columns are written, for OBJ files \
                     using the extended `v x y z r g b` form: 'auto' (default) only if at least \
                     one vertex carries colour; 'always' writes the columns with blanks for \
                     colourless vertices; 'never' drops them. Colour values are copied verbatim \
                     and never rounded.",
                ),
        )
        .param(
            Param::enumv("up_axis", ["keep", "y-to-z", "z-to-y"])
                .default("keep")
                .describe(
                    "Up-axis conversion, applied before scaling. 'keep' (default) leaves the \
                     coordinates as stored (OBJ is Y-up); 'y-to-z' rotates +90° about X so Y-up \
                     data lands in a Z-up convention (CAD/GIS/printing), mapping (x,y,z) to \
                     (x,-z,y); 'z-to-y' is the inverse, mapping (x,y,z) to (x,z,-y).",
                ),
        )
        .param(
            Param::number("scale")
                .default(1.0)
                .min(-1000000.0)
                .max(1000000.0)
                .describe(
                    "Multiplier applied to every coordinate after the up-axis conversion — use it \
                     for unit changes (1000 = metres to millimetres, 0.001 = millimetres to \
                     metres, 25.4 = inches to millimetres). Default 1 leaves the size unchanged.",
                ),
        )
        .param(
            Param::integer("precision")
                .default(-1)
                .min(-1.0)
                .max(15.0)
                .describe(
                    "Decimal places for the coordinate columns. -1 (default) keeps each number \
                     exactly as written in the file (so `1.500` stays `1.500`); 0-15 rounds and \
                     pads to that many places. Coordinates are always reformatted when `up_axis` \
                     or `scale` changes them.",
                ),
        )
        .param(
            Param::enumv("dedupe", ["none", "adjacent", "all"])
                .default("none")
                .describe(
                    "Drop repeated positions, compared after up-axis, scale and rounding: 'none' \
                     (default) emits every `v` line; 'adjacent' drops a vertex identical to the \
                     one emitted just before it; 'all' emits each distinct position once. \
                     Welded/seam-split exports often repeat positions.",
                ),
        )
        .param(Param::string("objects").describe(
            "Comma-separated `o` object or `g` group names to keep — for example \
             'wheels,chassis'. Names are matched exactly (a `g` line's individual names are each \
             matched) and a vertex belongs to the section declared most recently before it, which \
             is how exporters segment multi-part OBJ files. Empty (default) keeps every vertex.",
        ))
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe", "space"])
                .default("comma")
                .describe(
                    "Output field separator: 'comma' (default), 'semicolon', 'tab', 'pipe' or \
                     'space' (space plus header=false gives a plain XYZ/ASC point-cloud file). \
                     Fields are RFC-4180 escaped (quoted when they contain the delimiter, a quote \
                     or a newline).",
                ),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit a header row of column names. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/obj-vertices-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract Wavefront OBJ vertex coordinates into an x,y,z CSV",
    skill(
        description = "Extract the vertex (`v`) coordinates of a Wavefront OBJ document into a flat x,y,z CSV, ONE ROW PER VERTEX, in file order. Pass the OBJ text as `obj`. Faces, texture coordinates, normals and materials are skipped and nothing is resampled — the rows are the stored positions. The extended `v x y z r g b` colour form is understood (`color` controls the red/green/blue columns) and a 4th `w` value is ignored. `columns` adds context: 'indexed' (the 1-based OBJ vertex index that `f` lines reference), 'object' (`object`, `group`) or 'full' (also `material`). `up_axis` converts between Y-up and Z-up ('y-to-z' maps (x,y,z) to (x,-z,y)); `scale` multiplies coordinates for unit changes; `precision` rounds to 0-15 decimals (-1 keeps the source text); `dedupe` drops 'adjacent' or 'all' repeated positions; `objects` keeps only named `o`/`g` sections; `delimiter` picks comma/semicolon/tab/pipe/space and `header`=false drops the header row (space + no header = a plain XYZ point-cloud file). Up to 16 MiB of OBJ and 500000 rows. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "obj-vertices-to-csv", |a: Args| {
            convert_str(
                &a.obj,
                &a.columns,
                &a.color,
                &a.up_axis,
                a.scale,
                a.precision as i32,
                &a.dedupe,
                &a.objects,
                &a.delimiter,
                a.header,
            )
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "obj": { "type": "string", "description": "Wavefront OBJ text to extract vertices from. Every `v x y z` line becomes ONE CSV row, in file order — a 5000-vertex mesh gives 5000 rows. The extended `v x y z r g b` form (per-vertex colours) is understood and an optional 4th `w` value is ignored. Faces (`f`), texture coordinates (`vt`), normals (`vn`), parameter-space vertices (`vp`) and material libraries are skipped — nothing is resampled. Up to 16 MiB." },
                    "columns": { "type": "string", "enum": ["xyz", "indexed", "object", "full"], "default": "xyz", "description": "Context columns written before the coordinates: 'xyz' (default) just the coordinate columns; 'indexed' adds `index`, the vertex's 1-based OBJ index (the number `f` lines reference, kept even when rows are filtered or deduped); 'object' adds `index`, `object` (the enclosing `o` name) and `group` (the enclosing `g` names); 'full' additionally adds `material` (the active `usemtl`)." },
                    "color": { "type": "string", "enum": ["auto", "always", "never"], "default": "auto", "description": "When the per-vertex `red,green,blue` columns are written, for OBJ files using the extended `v x y z r g b` form: 'auto' (default) only if at least one vertex carries colour; 'always' writes the columns with blanks for colourless vertices; 'never' drops them. Colour values are copied verbatim and never rounded." },
                    "up_axis": { "type": "string", "enum": ["keep", "y-to-z", "z-to-y"], "default": "keep", "description": "Up-axis conversion, applied before scaling. 'keep' (default) leaves the coordinates as stored (OBJ is Y-up); 'y-to-z' rotates +90° about X so Y-up data lands in a Z-up convention (CAD/GIS/printing), mapping (x,y,z) to (x,-z,y); 'z-to-y' is the inverse, mapping (x,y,z) to (x,z,-y)." },
                    "scale": { "type": "number", "default": 1.0, "minimum": -1000000, "maximum": 1000000, "description": "Multiplier applied to every coordinate after the up-axis conversion — use it for unit changes (1000 = metres to millimetres, 0.001 = millimetres to metres, 25.4 = inches to millimetres). Default 1 leaves the size unchanged." },
                    "precision": { "type": "integer", "default": -1, "minimum": -1, "maximum": 15, "description": "Decimal places for the coordinate columns. -1 (default) keeps each number exactly as written in the file (so `1.500` stays `1.500`); 0-15 rounds and pads to that many places. Coordinates are always reformatted when `up_axis` or `scale` changes them." },
                    "dedupe": { "type": "string", "enum": ["none", "adjacent", "all"], "default": "none", "description": "Drop repeated positions, compared after up-axis, scale and rounding: 'none' (default) emits every `v` line; 'adjacent' drops a vertex identical to the one emitted just before it; 'all' emits each distinct position once. Welded/seam-split exports often repeat positions." },
                    "objects": { "type": "string", "description": "Comma-separated `o` object or `g` group names to keep — for example 'wheels,chassis'. Names are matched exactly (a `g` line's individual names are each matched) and a vertex belongs to the section declared most recently before it, which is how exporters segment multi-part OBJ files. Empty (default) keeps every vertex." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe", "space"], "default": "comma", "description": "Output field separator: 'comma' (default), 'semicolon', 'tab', 'pipe' or 'space' (space plus header=false gives a plain XYZ/ASC point-cloud file). Fields are RFC-4180 escaped (quoted when they contain the delimiter, a quote or a newline)." },
                    "header": { "type": "boolean", "default": true, "description": "Emit a header row of column names. Default true." }
                },
                "required": ["obj"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
