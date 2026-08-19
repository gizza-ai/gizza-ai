//! gizza-ai/shapefile-to-geojson — convert an ESRI shapefile set (`.shp` +
//! `.dbf` + `.prj`, normally zipped) into GeoJSON.
//!
//! No-page block (chat + CLI surface only, like `blocks/dbf-table-parser` /
//! `blocks/xlsx-to-csv`): it ingests binary archive/`.shp` bytes, which is neither
//! a pure-text page input nor an ffmpeg media transform, so there is no standalone
//! page. The chat schema is derived from `descriptor()` (single source shared
//! across chat + CLI).
//!
//! Pipeline: parse `{url|ref}` + options → resolve bytes via
//! `block_utils::resolve_source` (URL fetch or attachment lookup; `AssetKind::Any`,
//! since shapefile zips are usually served as `application/octet-stream`) →
//! `core::convert` → emit a text `Envelope`. The LLM sees a short conversion
//! summary plus the GeoJSON (head-truncated if large); the UI gets a downloadable
//! `data:` URL + filename.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_shapefile_to_geojson_core::{convert, Conversion, Encoding, Options, Output};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the uploaded archive/`.shp`. Boundary files are large and the GeoJSON
/// they expand into is several times bigger again, so stay inside the sandbox.
const MAX_BYTES: usize = 24 * 1024 * 1024; // 24 MiB

/// Cap on the text fed back to the LLM (`_for_llm`). Larger results are
/// head-truncated with a note; the full output is always available via `_for_ui`.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB

fn default_output() -> String {
    "geojson".to_string()
}
fn default_precision() -> i64 {
    6
}
fn default_properties() -> bool {
    true
}
fn default_encoding() -> String {
    "auto".to_string()
}
fn default_bbox() -> bool {
    true
}
fn default_include_z() -> bool {
    true
}
fn default_rewind() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// `geojson` (one FeatureCollection) or `ndjson` (one Feature per line).
    #[serde(default = "default_output")]
    output: String,
    /// Indent the GeoJSON.
    #[serde(default)]
    pretty: bool,
    /// Coordinate decimal places; -1 keeps full precision.
    #[serde(default = "default_precision")]
    precision: i64,
    /// Max features to emit; 0 = all.
    #[serde(default)]
    limit: u64,
    /// Attach the `.dbf` attribute row to each feature.
    #[serde(default = "default_properties")]
    properties: bool,
    /// Comma-separated attribute columns to keep/reorder.
    #[serde(default)]
    columns: String,
    /// `.dbf` text encoding: auto, utf-8, latin1, cp1252.
    #[serde(default = "default_encoding")]
    encoding: String,
    /// Which `.shp` inside a multi-layer zip (base name, no extension).
    #[serde(default)]
    layer: String,
    /// Emit a top-level `bbox`.
    #[serde(default = "default_bbox")]
    bbox: bool,
    /// Keep Z as a third coordinate.
    #[serde(default = "default_include_z")]
    include_z: bool,
    /// Rewind rings to RFC 7946 winding.
    #[serde(default = "default_rewind")]
    rewind: bool,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::File` emits
/// the `url`⊕`ref` `oneOf`; the options tune the parse + output.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("output", ["geojson", "ndjson"])
                .default("geojson")
                .describe(
                    "Output shape. \"geojson\" writes one RFC 7946 FeatureCollection. \"ndjson\" writes newline-delimited GeoJSON (GeoJSONL): one Feature object per line, no wrapper, which streams line by line into tools like tippecanoe or DuckDB. Defaults to geojson.",
                ),
        )
        .param(
            Param::boolean("pretty").default(false).describe(
                "Indent the GeoJSON for reading. Leave false for the smallest file. Ignored for ndjson, where each feature must stay on one line. Defaults to false.",
            ),
        )
        .param(
            Param::integer("precision")
                .default(6)
                .min(-1.0)
                .max(17.0)
                .describe(
                    "Decimal places to keep on every coordinate. 6 (the default) is about 11 cm at the equator and often shrinks a boundary file several-fold; use -1 to keep the source's full precision.",
                ),
        )
        .param(
            Param::integer("limit").default(0).min(0.0).describe(
                "Maximum number of features to emit; 0 (the default) converts every record. Use a small value to preview a large boundary file.",
            ),
        )
        .param(
            Param::boolean("properties").default(true).describe(
                "Attach each record's .dbf attribute row to the feature's \"properties\". Set false for geometry only. Defaults to true.",
            ),
        )
        .param(
            Param::string("columns").default("").describe(
                "Comma-separated attribute columns to keep and reorder, by name (case-insensitive) or 0-based index, e.g. \"GEOID,NAME,ALAND\". Leave empty to keep every column in file order.",
            ),
        )
        .param(
            Param::enumv("encoding", ["auto", "utf-8", "latin1", "cp1252"])
                .default("auto")
                .describe(
                    "Text decoding for .dbf character fields. \"auto\" honours a .cpg sidecar when present, else UTF-8 if valid, else Latin-1; \"latin1\" (ISO-8859-1) and \"cp1252\" (Windows-1252) cover most legacy tables. Defaults to auto.",
                ),
        )
        .param(
            Param::string("layer").default("").describe(
                "Which layer to convert when the .zip holds several .shp files: the base name without extension, e.g. \"tl_2023_us_county\". Leave empty to take the first in name order; a wrong name lists the ones available.",
            ),
        )
        .param(
            Param::boolean("bbox").default(true).describe(
                "Add a top-level \"bbox\" ([minLon, minLat, maxLon, maxLat]) to the FeatureCollection, computed from the features actually emitted. Ignored for ndjson. Defaults to true.",
            ),
        )
        .param(
            Param::boolean("include_z").default(true).describe(
                "Keep the Z value of PointZ/PolyLineZ/PolygonZ shapes as a third coordinate. Set false for flat [lon, lat] positions. M (measure) values are always dropped — RFC 7946 has no slot for them. Defaults to true.",
            ),
        )
        .param(
            Param::boolean("rewind").default(true).describe(
                "Rewind polygon rings to RFC 7946 winding (exterior counter-clockwise, holes clockwise), the opposite of the shapefile convention. Set false to keep the source's winding byte-for-byte. Defaults to true.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Resolve the descriptor string args into a `core::Options`.
fn build_options(args: &Args) -> Result<Options, SkillError> {
    let output = match args.output.trim().to_ascii_lowercase().as_str() {
        "geojson" | "json" => Output::Geojson,
        "ndjson" | "geojsonl" | "geojsonseq" => Output::Ndjson,
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "output must be \"geojson\" or \"ndjson\", got {other:?}"
            )))
        }
    };
    let encoding = match args.encoding.trim().to_ascii_lowercase().as_str() {
        "auto" => Encoding::Auto,
        "utf-8" | "utf8" => Encoding::Utf8,
        "latin1" | "latin-1" | "iso-8859-1" => Encoding::Latin1,
        "cp1252" | "windows-1252" => Encoding::Cp1252,
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "encoding must be auto, utf-8, latin1, or cp1252, got {other:?}"
            )))
        }
    };
    if args.precision < -1 || args.precision > 17 {
        return Err(SkillError::InvalidArgs(format!(
            "precision must be between -1 (full precision) and 17, got {}",
            args.precision
        )));
    }
    Ok(Options {
        output,
        pretty: args.pretty,
        precision: args.precision,
        limit: args.limit as usize,
        properties: args.properties,
        columns: args.columns.clone(),
        encoding,
        layer: args.layer.clone(),
        bbox: args.bbox,
        include_z: args.include_z,
        rewind: args.rewind,
    })
}

/// One-line-per-fact conversion summary, so the LLM (and the CLI user) can see
/// what was read without scanning the GeoJSON itself.
fn summary_lines(c: &Conversion, filename: &str) -> String {
    let mut s = format!(
        "Converted {filename} (layer \"{}\", {} shapes) to GeoJSON: {} feature{}",
        c.layer,
        c.shape_type,
        c.feature_count,
        if c.feature_count == 1 { "" } else { "s" }
    );
    if c.feature_count < c.total_records {
        s.push_str(&format!(" of {} (limit applied)", c.total_records));
    }
    s.push('.');
    if c.layers.len() > 1 {
        s.push_str(&format!(
            "\nLayers in the archive: {}.",
            c.layers.join(", ")
        ));
    }
    if let Some(crs) = &c.crs {
        s.push_str(&format!("\nSource CRS (.prj): {crs}."));
    }
    for w in &c.warnings {
        s.push_str(&format!("\nNote: {w}"));
    }
    s
}

/// Build the `_for_llm` text: the summary plus the full output when small, else a
/// head plus a note.
fn summarize_for_llm(c: &Conversion, filename: &str) -> String {
    let head = summary_lines(c, filename);
    if c.geojson.chars().count() <= MAX_LLM_CHARS {
        format!("{head}\n{}", c.geojson)
    } else {
        let clipped: String = c.geojson.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "{head}\n(first {MAX_LLM_CHARS} of {} chars; full file in the download)\n{clipped}",
            c.geojson.chars().count()
        )
    }
}

/// Name the download after the layer, not the uploaded archive: a zip called
/// `tl_2023_06_tract.zip` holding `tl_2023_06_tract.shp` should download as
/// `tl_2023_06_tract.geojson`.
fn output_filename(layer: &str, upload: &str, ext: &str) -> String {
    let stem = if layer.is_empty() || layer == "shapefile" {
        let base = upload.rsplit('/').next().unwrap_or(upload);
        match base.rsplit_once('.') {
            Some((s, _)) if !s.is_empty() => s.to_string(),
            _ => "shapefile".to_string(),
        }
    } else {
        layer.to_string()
    };
    format!("{stem}.{ext}")
}

#[cfg(target_arch = "wasm32")]
struct ShapefileToGeojson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/shapefile-to-geojson",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an ESRI shapefile set (.shp/.dbf/.prj, zipped) to GeoJSON",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert an ESRI shapefile set into GeoJSON. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). A shapefile is a SET of files, so upload the .zip that holds .shp + .dbf (+ .prj/.cpg) together — that is how Census/TIGER, Natural Earth and most government portals ship them; a bare .shp also works but yields geometry with empty properties. The .shx index is not needed. Geometry: Point, MultiPoint, PolyLine and Polygon, including their Z and M variants; rings are regrouped into Polygon/MultiPolygon by orientation and rewound to RFC 7946 winding, and M (measure) values are dropped because GeoJSON has no slot for them. MultiPatch (type 31) is rejected. Attributes come from the .dbf; the .prj is reported and flagged when it declares a PROJECTED CRS (whose coordinates are metres, not WGS 84 lon/lat, so they need reprojecting before they will line up on a web map). Options: `output` (geojson default, or ndjson/GeoJSONL one feature per line), `pretty`, `precision` (coordinate decimals, default 6, -1 = full), `limit` (feature cap for previews), `properties` (default true), `columns` (keep/reorder attributes), `encoding` (auto/utf-8/latin1/cp1252, auto honours a .cpg), `layer` (pick one .shp from a multi-layer zip), `bbox` (default true), `include_z` (default true), and `rewind` (default true).",
        parameters = schema_json()
    ),
)]
impl ShapefileToGeojson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("shapefile-to-geojson")?;
    let opts = build_options(&args)?;

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let name = if filename.is_empty() {
        "shapefile.zip".to_string()
    } else {
        filename.clone()
    };

    let conv = convert(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let for_llm = summarize_for_llm(&conv, &name);
    let (ext, mime) = match opts.output {
        Output::Geojson => ("geojson", "application/geo+json"),
        Output::Ndjson => ("geojsonl", "application/geo+json-seq"),
    };
    let out_filename = output_filename(&conv.layer, &name, ext);
    let data_url = format!("data:{mime};base64,{}", B64.encode(conv.geojson.as_bytes()));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: mime.to_string(),
            filename: out_filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored one, so the LLM sees no drift. `url`/`ref` wording is centralized
    /// in `to_schema_json` (shared by every File/media tool).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "output":     { "type": "string", "enum": ["geojson", "ndjson"], "default": "geojson", "description": "Output shape. \"geojson\" writes one RFC 7946 FeatureCollection. \"ndjson\" writes newline-delimited GeoJSON (GeoJSONL): one Feature object per line, no wrapper, which streams line by line into tools like tippecanoe or DuckDB. Defaults to geojson." },
                    "pretty":     { "type": "boolean", "default": false, "description": "Indent the GeoJSON for reading. Leave false for the smallest file. Ignored for ndjson, where each feature must stay on one line. Defaults to false." },
                    "precision":  { "type": "integer", "default": 6, "minimum": -1, "maximum": 17, "description": "Decimal places to keep on every coordinate. 6 (the default) is about 11 cm at the equator and often shrinks a boundary file several-fold; use -1 to keep the source's full precision." },
                    "limit":      { "type": "integer", "default": 0, "minimum": 0, "description": "Maximum number of features to emit; 0 (the default) converts every record. Use a small value to preview a large boundary file." },
                    "properties": { "type": "boolean", "default": true, "description": "Attach each record's .dbf attribute row to the feature's \"properties\". Set false for geometry only. Defaults to true." },
                    "columns":    { "type": "string", "default": "", "description": "Comma-separated attribute columns to keep and reorder, by name (case-insensitive) or 0-based index, e.g. \"GEOID,NAME,ALAND\". Leave empty to keep every column in file order." },
                    "encoding":   { "type": "string", "enum": ["auto", "utf-8", "latin1", "cp1252"], "default": "auto", "description": "Text decoding for .dbf character fields. \"auto\" honours a .cpg sidecar when present, else UTF-8 if valid, else Latin-1; \"latin1\" (ISO-8859-1) and \"cp1252\" (Windows-1252) cover most legacy tables. Defaults to auto." },
                    "layer":      { "type": "string", "default": "", "description": "Which layer to convert when the .zip holds several .shp files: the base name without extension, e.g. \"tl_2023_us_county\". Leave empty to take the first in name order; a wrong name lists the ones available." },
                    "bbox":       { "type": "boolean", "default": true, "description": "Add a top-level \"bbox\" ([minLon, minLat, maxLon, maxLat]) to the FeatureCollection, computed from the features actually emitted. Ignored for ndjson. Defaults to true." },
                    "include_z":  { "type": "boolean", "default": true, "description": "Keep the Z value of PointZ/PolyLineZ/PolygonZ shapes as a third coordinate. Set false for flat [lon, lat] positions. M (measure) values are always dropped — RFC 7946 has no slot for them. Defaults to true." },
                    "rewind":     { "type": "boolean", "default": true, "description": "Rewind polygon rings to RFC 7946 winding (exterior counter-clockwise, holes clockwise), the opposite of the shapefile convention. Set false to keep the source's winding byte-for-byte. Defaults to true." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_apply() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.zip"}"#).unwrap();
        assert_eq!(a.output, "geojson");
        assert!(!a.pretty);
        assert_eq!(a.precision, 6);
        assert_eq!(a.limit, 0);
        assert!(a.properties);
        assert_eq!(a.columns, "");
        assert_eq!(a.encoding, "auto");
        assert_eq!(a.layer, "");
        assert!(a.bbox);
        assert!(a.include_z);
        assert!(a.rewind);
    }

    #[test]
    fn args_parse_overrides() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"call_1","output":"ndjson","pretty":true,"precision":-1,"limit":5,"properties":false,"columns":"GEOID,NAME","encoding":"cp1252","layer":"tracts","bbox":false,"include_z":false,"rewind":false}"#,
        )
        .unwrap();
        assert_eq!(a.output, "ndjson");
        assert!(a.pretty);
        assert_eq!(a.precision, -1);
        assert_eq!(a.limit, 5);
        assert!(!a.properties);
        assert_eq!(a.columns, "GEOID,NAME");
        assert_eq!(a.encoding, "cp1252");
        assert_eq!(a.layer, "tracts");
        assert!(!a.bbox);
        assert!(!a.include_z);
        assert!(!a.rewind);
    }

    #[test]
    fn build_options_maps_aliases() {
        let a: Args =
            serde_json::from_str(r#"{"url":"u","output":"geojsonl","encoding":"windows-1252"}"#)
                .unwrap();
        let o = build_options(&a).unwrap();
        assert_eq!(o.output, Output::Ndjson);
        assert_eq!(o.encoding, Encoding::Cp1252);
    }

    #[test]
    fn build_options_rejects_bad_output() {
        let a: Args = serde_json::from_str(r#"{"url":"u","output":"kml"}"#).unwrap();
        let err = build_options(&a).unwrap_err();
        assert!(err.to_string().contains("output must be"), "got: {err}");
    }

    #[test]
    fn build_options_rejects_bad_encoding() {
        let a: Args = serde_json::from_str(r#"{"url":"u","encoding":"ebcdic"}"#).unwrap();
        let err = build_options(&a).unwrap_err();
        assert!(err.to_string().contains("encoding must be"), "got: {err}");
    }

    #[test]
    fn build_options_rejects_out_of_range_precision() {
        let a: Args = serde_json::from_str(r#"{"url":"u","precision":25}"#).unwrap();
        let err = build_options(&a).unwrap_err();
        assert!(err.to_string().contains("precision must be"), "got: {err}");
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn args_reject_neither_url_nor_ref() {
        let err = serde_json::from_str::<Args>(r#"{"output":"geojson"}"#).unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn output_filename_prefers_the_layer_name() {
        assert_eq!(
            output_filename("tl_2023_06_tract", "download.zip", "geojson"),
            "tl_2023_06_tract.geojson"
        );
        // A bare .shp has no layer name, so fall back to the upload's stem.
        assert_eq!(
            output_filename("shapefile", "coastline.shp", "geojson"),
            "coastline.geojson"
        );
        assert_eq!(
            output_filename("", "a/b/places.zip", "geojsonl"),
            "places.geojsonl"
        );
    }

    #[test]
    fn summary_reports_layer_shape_type_and_counts() {
        let c = Conversion {
            geojson: "{}".to_string(),
            feature_count: 2,
            total_records: 7,
            shape_type: "PolygonZ".to_string(),
            layer: "tracts".to_string(),
            layers: vec!["tracts".to_string(), "roads".to_string()],
            crs: Some("GCS_WGS_1984".to_string()),
            warnings: vec!["something to know".to_string()],
        };
        let s = summary_lines(&c, "ca.zip");
        assert!(s.contains("layer \"tracts\""), "got: {s}");
        assert!(s.contains("PolygonZ shapes"), "got: {s}");
        assert!(s.contains("2 features of 7 (limit applied)"), "got: {s}");
        assert!(
            s.contains("Layers in the archive: tracts, roads."),
            "got: {s}"
        );
        assert!(s.contains("Source CRS (.prj): GCS_WGS_1984."), "got: {s}");
        assert!(s.contains("Note: something to know"), "got: {s}");
    }

    fn conv(geojson: &str) -> Conversion {
        Conversion {
            geojson: geojson.to_string(),
            feature_count: 1,
            total_records: 1,
            shape_type: "Point".to_string(),
            layer: "places".to_string(),
            layers: vec!["places".to_string()],
            crs: None,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn summarize_short_includes_the_full_geojson() {
        let s = summarize_for_llm(&conv(r#"{"type":"FeatureCollection"}"#), "places.zip");
        assert!(s.contains("Converted places.zip"));
        assert!(s.contains(r#"{"type":"FeatureCollection"}"#));
    }

    #[test]
    fn summarize_long_truncates_with_a_note() {
        let big = "x".repeat(MAX_LLM_CHARS + 100);
        let s = summarize_for_llm(&conv(&big), "big.zip");
        assert!(s.contains("full file in the download"));
        assert!(s.len() < big.len() + 400);
    }
}
