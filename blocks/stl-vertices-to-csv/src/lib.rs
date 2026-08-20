//! gizza-ai/stl-vertices-to-csv — chat skill block on the shared tool abstraction.
//! Flattens an ASCII or binary STL mesh into a CSV of its triangle vertex
//! coordinates: three rows per facet by default, or one nine-coordinate row per
//! facet. Mesh metrics live in `blocks/stl-inspector` and repairs in
//! `blocks/stl-repair`; this block only tabulates coordinates. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI and, via
//! manifest.json, the page form); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_stl_vertices_to_csv_core::convert_str;
use serde::Deserialize;
use wafer_sdk::*;

fn default_input_format() -> String {
    "auto".into()
}
fn default_rows() -> String {
    "vertex".into()
}
fn default_columns() -> String {
    "xyz".into()
}
fn default_normal_source() -> String {
    "stored".into()
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
fn default_every_nth() -> f64 {
    1.0
}
fn default_delimiter() -> String {
    "comma".into()
}
fn default_header() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    stl: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_rows")]
    rows: String,
    #[serde(default = "default_columns")]
    columns: String,
    #[serde(default = "default_normal_source")]
    normal_source: String,
    #[serde(default = "default_up_axis")]
    up_axis: String,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default = "default_precision")]
    precision: f64,
    #[serde(default = "default_dedupe")]
    dedupe: String,
    #[serde(default = "default_every_nth")]
    every_nth: f64,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_header")]
    header: bool,
}

/// Single source for the chat schema (and the CLI + page form).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("stl").required().describe(
            "The mesh to flatten. Paste either an ASCII STL as text (solid / facet normal / vertex \
             lines), or a BINARY STL's bytes encoded as base64 or hex (hex may include spaces, \
             colons or dashes). Every facet stores its three corners explicitly, so a 5000-facet \
             mesh gives 15000 vertex rows in file order — nothing is welded or resampled. Up to \
             100000 triangles and 32 MiB of pasted input.",
        ))
        .param(
            Param::enumv("input_format", ["auto", "ascii", "base64", "hex"])
                .default("auto")
                .describe(
                    "How the pasted value is encoded. 'auto' (default) detects ASCII STL text, hex \
                     bytes or base64 bytes; binary STL is recognised by its 84 + 50 x \
                     triangle-count byte layout rather than the leading 'solid' keyword, which \
                     many exporters also write into a binary header. Set 'ascii', 'base64' or \
                     'hex' to force one.",
                ),
        )
        .param(
            Param::enumv("rows", ["vertex", "triangle"])
                .default("vertex")
                .describe(
                    "What one CSV row holds. 'vertex' (default) writes one row per triangle \
                     corner — three rows per facet, columns `x,y,z` — which is the point-cloud / \
                     XYZ shape. 'triangle' writes one row per facet with all nine coordinates \
                     (`v1x,v1y,v1z,v2x,v2y,v2z,v3x,v3y,v3z`), which keeps facets intact for CAD \
                     and spreadsheet imports.",
                ),
        )
        .param(
            Param::enumv("columns", ["xyz", "indexed", "normals", "full"])
                .default("xyz")
                .describe(
                    "Extra columns around the coordinates: 'xyz' (default) coordinates only; \
                     'indexed' prepends `triangle` (1-based facet number) and, for `rows=vertex`, \
                     `corner` (1, 2 or 3) so flattened rows can be regrouped into facets; \
                     'normals' appends the facet normal as `nx,ny,nz`; 'full' adds both.",
                ),
        )
        .param(
            Param::enumv("normal_source", ["stored", "computed"])
                .default("stored")
                .describe(
                    "Which normal the `nx,ny,nz` columns carry when `columns` includes them. \
                     'stored' (default) copies the normal recorded in the file — many exporters \
                     write 0 0 0 there. 'computed' derives a unit right-hand-rule normal from the \
                     corner order instead, which is 0 0 0 only for degenerate facets.",
                ),
        )
        .param(
            Param::enumv("up_axis", ["keep", "z-to-y", "y-to-z"])
                .default("keep")
                .describe(
                    "Up-axis conversion, applied before scaling and to normals as well as \
                     positions. 'keep' (default) leaves the coordinates as stored (STL is \
                     conventionally Z-up); 'z-to-y' rotates -90° about X so Z-up data lands in the \
                     Y-up convention used by glTF and most web 3D viewers, mapping (x,y,z) to \
                     (x,z,-y); 'y-to-z' is the inverse, mapping (x,y,z) to (x,-z,y).",
                ),
        )
        .param(
            Param::number("scale")
                .default(1.0)
                .min(-1000000.0)
                .max(1000000.0)
                .describe(
                    "Multiplier applied to every coordinate after the up-axis conversion — use it \
                     for unit changes (0.1 = millimetres to centimetres, 25.4 = inches to \
                     millimetres, 0.0393701 = millimetres to inches). Normals are directions and \
                     are never scaled. Default 1 leaves the size unchanged.",
                ),
        )
        .param(
            Param::integer("precision")
                .default(-1)
                .min(-1.0)
                .max(15.0)
                .describe(
                    "Decimal places for the coordinate and normal columns. -1 (default) prints the \
                     shortest text that round-trips the stored value — binary STL stores 32-bit \
                     floats, so those print at 32-bit width instead of a 17-digit tail; 0-15 \
                     rounds and pads to that many places, which is usually what a CAD or \
                     spreadsheet import wants.",
                ),
        )
        .param(
            Param::enumv("dedupe", ["none", "adjacent", "all"])
                .default("none")
                .describe(
                    "Drop repeated rows, compared on the coordinate columns after up-axis, scale \
                     and rounding: 'none' (default) keeps every row; 'adjacent' drops a row \
                     identical to the one just before it; 'all' keeps each distinct position (or \
                     facet, for `rows=triangle`) once. STL repeats every shared corner in each \
                     facet that touches it, so 'all' turns a mesh into a welded point cloud.",
                ),
        )
        .param(
            Param::integer("every_nth")
                .default(1)
                .min(1.0)
                .max(1000000.0)
                .describe(
                    "Keep one row out of every N that survives dedupe — 1 (default) keeps them \
                     all, 10 keeps every tenth. Thins a dense scan or mesh down to a manageable \
                     point sample without changing any coordinate.",
                ),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe", "space"])
                .default("comma")
                .describe(
                    "Output field separator: 'comma' (default), 'semicolon', 'tab', 'pipe' or \
                     'space' ('space' plus header=false gives a plain XYZ/ASC point-cloud file). \
                     Every field is a number, so nothing is ever quoted or escaped.",
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
    name = "gizza-ai/stl-vertices-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten an ASCII or binary STL mesh into a CSV of triangle vertex coordinates",
    skill(
        description = "Parse an ASCII or binary STL mesh into a CSV of its triangle vertex coordinates. Pass the mesh as `stl`: ASCII STL as text, or a binary STL's bytes as base64 or hex (`input_format` = auto/ascii/base64/hex). `rows`='vertex' (default) writes ONE ROW PER TRIANGLE CORNER — three rows per facet, `x,y,z`, in file order, nothing welded or resampled; `rows`='triangle' writes one row per facet with all nine coordinates (`v1x`…`v3z`). `columns` adds context: 'indexed' prepends `triangle` and `corner` numbers, 'normals' appends `nx,ny,nz`, 'full' adds both; `normal_source`='computed' replaces STL's often-zero stored normal with a right-hand-rule unit normal. `up_axis` converts Z-up STL to Y-up glTF ('z-to-y' maps (x,y,z) to (x,z,-y)) and back; `scale` multiplies coordinates for unit changes; `precision` rounds to 0-15 decimals (-1 prints the shortest round-tripping text); `dedupe`='all' welds repeated corners into a point cloud; `every_nth` thins the surviving rows; `delimiter` picks comma/semicolon/tab/pipe/space and `header`=false drops the header row (space + no header = a plain XYZ point-cloud file). Up to 100000 triangles and 32 MiB of input. For mesh metrics use stl-inspector, for repairs stl-repair. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "stl-vertices-to-csv", |a: Args| {
            convert_str(
                &a.stl,
                &a.input_format,
                &a.rows,
                &a.columns,
                &a.normal_source,
                &a.up_axis,
                a.scale,
                a.precision as i32,
                &a.dedupe,
                a.every_nth as i64,
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
                    "stl": { "type": "string", "description": "The mesh to flatten. Paste either an ASCII STL as text (solid / facet normal / vertex lines), or a BINARY STL's bytes encoded as base64 or hex (hex may include spaces, colons or dashes). Every facet stores its three corners explicitly, so a 5000-facet mesh gives 15000 vertex rows in file order — nothing is welded or resampled. Up to 100000 triangles and 32 MiB of pasted input." },
                    "input_format": { "type": "string", "enum": ["auto", "ascii", "base64", "hex"], "default": "auto", "description": "How the pasted value is encoded. 'auto' (default) detects ASCII STL text, hex bytes or base64 bytes; binary STL is recognised by its 84 + 50 x triangle-count byte layout rather than the leading 'solid' keyword, which many exporters also write into a binary header. Set 'ascii', 'base64' or 'hex' to force one." },
                    "rows": { "type": "string", "enum": ["vertex", "triangle"], "default": "vertex", "description": "What one CSV row holds. 'vertex' (default) writes one row per triangle corner — three rows per facet, columns `x,y,z` — which is the point-cloud / XYZ shape. 'triangle' writes one row per facet with all nine coordinates (`v1x,v1y,v1z,v2x,v2y,v2z,v3x,v3y,v3z`), which keeps facets intact for CAD and spreadsheet imports." },
                    "columns": { "type": "string", "enum": ["xyz", "indexed", "normals", "full"], "default": "xyz", "description": "Extra columns around the coordinates: 'xyz' (default) coordinates only; 'indexed' prepends `triangle` (1-based facet number) and, for `rows=vertex`, `corner` (1, 2 or 3) so flattened rows can be regrouped into facets; 'normals' appends the facet normal as `nx,ny,nz`; 'full' adds both." },
                    "normal_source": { "type": "string", "enum": ["stored", "computed"], "default": "stored", "description": "Which normal the `nx,ny,nz` columns carry when `columns` includes them. 'stored' (default) copies the normal recorded in the file — many exporters write 0 0 0 there. 'computed' derives a unit right-hand-rule normal from the corner order instead, which is 0 0 0 only for degenerate facets." },
                    "up_axis": { "type": "string", "enum": ["keep", "z-to-y", "y-to-z"], "default": "keep", "description": "Up-axis conversion, applied before scaling and to normals as well as positions. 'keep' (default) leaves the coordinates as stored (STL is conventionally Z-up); 'z-to-y' rotates -90° about X so Z-up data lands in the Y-up convention used by glTF and most web 3D viewers, mapping (x,y,z) to (x,z,-y); 'y-to-z' is the inverse, mapping (x,y,z) to (x,-z,y)." },
                    "scale": { "type": "number", "default": 1.0, "minimum": -1000000, "maximum": 1000000, "description": "Multiplier applied to every coordinate after the up-axis conversion — use it for unit changes (0.1 = millimetres to centimetres, 25.4 = inches to millimetres, 0.0393701 = millimetres to inches). Normals are directions and are never scaled. Default 1 leaves the size unchanged." },
                    "precision": { "type": "integer", "default": -1, "minimum": -1, "maximum": 15, "description": "Decimal places for the coordinate and normal columns. -1 (default) prints the shortest text that round-trips the stored value — binary STL stores 32-bit floats, so those print at 32-bit width instead of a 17-digit tail; 0-15 rounds and pads to that many places, which is usually what a CAD or spreadsheet import wants." },
                    "dedupe": { "type": "string", "enum": ["none", "adjacent", "all"], "default": "none", "description": "Drop repeated rows, compared on the coordinate columns after up-axis, scale and rounding: 'none' (default) keeps every row; 'adjacent' drops a row identical to the one just before it; 'all' keeps each distinct position (or facet, for `rows=triangle`) once. STL repeats every shared corner in each facet that touches it, so 'all' turns a mesh into a welded point cloud." },
                    "every_nth": { "type": "integer", "default": 1, "minimum": 1, "maximum": 1000000, "description": "Keep one row out of every N that survives dedupe — 1 (default) keeps them all, 10 keeps every tenth. Thins a dense scan or mesh down to a manageable point sample without changing any coordinate." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe", "space"], "default": "comma", "description": "Output field separator: 'comma' (default), 'semicolon', 'tab', 'pipe' or 'space' ('space' plus header=false gives a plain XYZ/ASC point-cloud file). Every field is a number, so nothing is ever quoted or escaped." },
                    "header": { "type": "boolean", "default": true, "description": "Emit a header row of column names. Default true." }
                },
                "required": ["stl"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
