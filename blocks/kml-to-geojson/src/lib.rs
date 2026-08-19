//! gizza-ai/kml-to-geojson — chat skill block on the shared tool abstraction.
//! Converts KML (or a base64-encoded KMZ archive) into GeoJSON, and GeoJSON
//! back into KML. The chat schema is single-sourced from descriptor() (which
//! also drives the CLI); handle() delegates to block_utils::run_skill. Pure →
//! runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_kml_to_geojson_core::{convert, AltitudeMode, Options, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default = "default_true")]
    include_styles: bool,
    #[serde(default = "default_true")]
    include_folders: bool,
    #[serde(default = "default_precision")]
    precision: u32,
    #[serde(default = "default_document_name")]
    document_name: String,
    #[serde(default = "default_altitude_mode")]
    altitude_mode: String,
}
fn default_output_format() -> String {
    "geojson".to_string()
}
fn default_true() -> bool {
    true
}
fn default_precision() -> u32 {
    6
}
fn default_document_name() -> String {
    "GeoJSON Export".to_string()
}
fn default_altitude_mode() -> String {
    "clamp_to_ground".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe(
            "The map data to convert: KML XML text, a base64-encoded KMZ archive (detected by \
             its 'UEsD' prefix — the doc.kml entry, or the first .kml entry, is used), or a \
             GeoJSON document (FeatureCollection, Feature, or bare geometry) when \
             output_format=kml. Up to 2 MB.",
        ))
        .param(
            Param::enumv("output_format", ["geojson", "kml"])
                .default("geojson")
                .describe(
                    "Direction to convert. 'geojson' (default) turns KML/KMZ into a GeoJSON \
                     FeatureCollection: Placemark Point/LineString/Polygon become the matching \
                     geometry, MultiGeometry becomes a GeometryCollection, and name/description/\
                     ExtendedData/TimeSpan/TimeStamp become properties. 'kml' turns GeoJSON back \
                     into a KML document.",
                ),
        )
        .param(Param::boolean("include_styles").default(true).describe(
            "Carry styling across. KML -> GeoJSON: fold each Placemark's inline or shared \
             Style/styleUrl/StyleMap into simplestyle-spec properties (stroke, stroke-width, \
             stroke-opacity, fill, fill-opacity, marker-color). GeoJSON -> KML: turn those same \
             properties back into an inline Style (LineStyle/PolyStyle/IconStyle). Default true.",
        ))
        .param(Param::boolean("include_folders").default(true).describe(
            "Carry the <Folder> hierarchy across. KML -> GeoJSON: each feature gets a 'folder' \
             property holding its slash-separated folder path (e.g. 'Trails/Day 1'). GeoJSON -> \
             KML: features are regrouped into nested <Folder> elements from that property. \
             Default true.",
        ))
        .param(
            Param::integer("precision")
                .default(6)
                .min(0.0)
                .max(15.0)
                .describe(
                    "Decimal places kept on every longitude/latitude/altitude, in both \
                     directions. 6 (the default) is about 0.1 m; 5 is about 1 m and makes a \
                     noticeably smaller file; 0 rounds to whole degrees. Range 0-15.",
                ),
        )
        .param(Param::string("document_name").default("GeoJSON Export").describe(
            "GeoJSON -> KML only: the <name> written on the KML <Document>, which is what \
             Google Earth shows in its Places list. Blank falls back to 'GeoJSON Export'. \
             Ignored when output_format=geojson.",
        ))
        .param(
            Param::enumv(
                "altitude_mode",
                ["clamp_to_ground", "relative_to_ground", "absolute"],
            )
            .default("clamp_to_ground")
            .describe(
                "GeoJSON -> KML only: how KML should read a position's third (altitude) value. \
                 'clamp_to_ground' (default) drapes geometry on the terrain, \
                 'relative_to_ground' treats altitude as height above the terrain, and \
                 'absolute' treats it as height above sea level. Ignored when \
                 output_format=geojson.",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/kml-to-geojson",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert KML or KMZ map data into GeoJSON, and GeoJSON back into KML",
    skill(
        description = "Convert KML or KMZ map data into GeoJSON, or GeoJSON back into KML. KML/KMZ -> GeoJSON: each Placemark's Point/LineString/Polygon becomes the matching GeoJSON geometry and MultiGeometry becomes a GeometryCollection; name/description become properties, ExtendedData/SimpleData become arbitrary properties, TimeSpan/TimeStamp become begin/end/time properties, the <Folder> path becomes a 'folder' property (include_folders=true, the default), and inline or shared Style/styleUrl/StyleMap colors and line widths become simplestyle-spec properties (stroke, stroke-width, stroke-opacity, fill, fill-opacity, marker-color) when include_styles=true (the default). A KMZ is a zip archive, so pass it base64-encoded (it is detected by its 'UEsD' prefix) and its doc.kml — or first .kml — entry is converted. Set output_format=kml to go the other way: Point/LineString/Polygon become the matching KML geometry, MultiPoint/MultiLineString/MultiPolygon/GeometryCollection become a MultiGeometry, name/description become <name>/<description>, every other property becomes ExtendedData, simplestyle properties become an inline Style, the 'folder' property rebuilds the <Folder> tree, and document_name/altitude_mode set the <Document> name and <altitudeMode>. The precision parameter rounds every coordinate in both directions (default 6 decimal places). Both formats are WGS84, so no reprojection happens. Input is capped at 2 MB. Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "kml-to-geojson", |a: Args| {
            let output_format =
                OutputFormat::parse(&a.output_format).map_err(SkillError::InvalidArgs)?;
            let altitude_mode =
                AltitudeMode::parse(&a.altitude_mode).map_err(SkillError::InvalidArgs)?;
            let opt = Options {
                output_format,
                include_styles: a.include_styles,
                include_folders: a.include_folders,
                precision: a.precision,
                document_name: a.document_name,
                altitude_mode,
            };
            convert(&a.input, &opt).map_err(SkillError::InvalidArgs)
        }) {
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
                    "input": { "type": "string", "description": "The map data to convert: KML XML text, a base64-encoded KMZ archive (detected by its 'UEsD' prefix — the doc.kml entry, or the first .kml entry, is used), or a GeoJSON document (FeatureCollection, Feature, or bare geometry) when output_format=kml. Up to 2 MB." },
                    "output_format": { "type": "string", "enum": ["geojson", "kml"], "default": "geojson", "description": "Direction to convert. 'geojson' (default) turns KML/KMZ into a GeoJSON FeatureCollection: Placemark Point/LineString/Polygon become the matching geometry, MultiGeometry becomes a GeometryCollection, and name/description/ExtendedData/TimeSpan/TimeStamp become properties. 'kml' turns GeoJSON back into a KML document." },
                    "include_styles": { "type": "boolean", "default": true, "description": "Carry styling across. KML -> GeoJSON: fold each Placemark's inline or shared Style/styleUrl/StyleMap into simplestyle-spec properties (stroke, stroke-width, stroke-opacity, fill, fill-opacity, marker-color). GeoJSON -> KML: turn those same properties back into an inline Style (LineStyle/PolyStyle/IconStyle). Default true." },
                    "include_folders": { "type": "boolean", "default": true, "description": "Carry the <Folder> hierarchy across. KML -> GeoJSON: each feature gets a 'folder' property holding its slash-separated folder path (e.g. 'Trails/Day 1'). GeoJSON -> KML: features are regrouped into nested <Folder> elements from that property. Default true." },
                    "precision": { "type": "integer", "default": 6, "minimum": 0, "maximum": 15, "description": "Decimal places kept on every longitude/latitude/altitude, in both directions. 6 (the default) is about 0.1 m; 5 is about 1 m and makes a noticeably smaller file; 0 rounds to whole degrees. Range 0-15." },
                    "document_name": { "type": "string", "default": "GeoJSON Export", "description": "GeoJSON -> KML only: the <name> written on the KML <Document>, which is what Google Earth shows in its Places list. Blank falls back to 'GeoJSON Export'. Ignored when output_format=geojson." },
                    "altitude_mode": { "type": "string", "enum": ["clamp_to_ground", "relative_to_ground", "absolute"], "default": "clamp_to_ground", "description": "GeoJSON -> KML only: how KML should read a position's third (altitude) value. 'clamp_to_ground' (default) drapes geometry on the terrain, 'relative_to_ground' treats altitude as height above the terrain, and 'absolute' treats it as height above sea level. Ignored when output_format=geojson." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
