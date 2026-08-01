//! gizza-ai/nmea-to-csv — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page + query-params); handle() delegates to block_utils::run_skill.
//! Pure compute — no host calls, runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_nmea_to_csv_core::convert_str;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    nmea: String,
    #[serde(default)]
    coordinates: String,
    #[serde(default)]
    altitude_unit: String,
    #[serde(default)]
    speed_unit: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    validate_checksum: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("nmea")
                .required()
                .describe("The NMEA 0183 log to convert. Paste the raw serial text — one sentence per line such as $GPGGA,... , $GNRMC,... , $GPGLL,... , $GPVTG,... , $GPZDA,... , $GPGSA,... (any talker prefix: GP/GN/GL/GA/GB)."),
        )
        .param(
            Param::enumv("coordinates", ["decimal", "dms"])
                .default("decimal")
                .describe("Coordinate output format. decimal (default) writes signed decimal degrees (e.g. 48.1173, -11.5167); dms writes degrees-minutes-seconds with a hemisphere letter (e.g. 48°07'02.28\"N)."),
        )
        .param(
            Param::enumv("altitude_unit", ["m", "ft"])
                .default("m")
                .describe("Unit for the altitude column. m (default) keeps GGA metres; ft converts to feet. The column header is altitude_m or altitude_ft accordingly."),
        )
        .param(
            Param::enumv("speed_unit", ["knots", "kmh", "mph"])
                .default("knots")
                .describe("Unit for the speed column. knots (default) matches NMEA's native speed-over-ground; kmh is km/h; mph is miles/h. The column header is speed_knots, speed_kmh, or speed_mph."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("Output CSV field separator. Default comma. Use semicolon for comma-decimal locales, tab for TSV, or pipe."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit the column-name header row. Default true; set false for a headerless CSV."),
        )
        .param(
            Param::boolean("validate_checksum")
                .default(false)
                .describe("Verify each line's NMEA *XX checksum and drop any sentence that fails it. Default false (parse every line). Lines that carry no checksum are always parsed."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NmeaToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/nmea-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a raw NMEA 0183 GPS log into a clean CSV track table.",
    skill(
        description = "Convert a raw NMEA 0183 GPS/GNSS serial log into a CSV track table, one row per positional fix. Sentences of the same cycle (shared UTC time) are merged: GGA supplies altitude / fix quality / satellite count / HDOP, RMC supplies date / speed / course, GLL is a position fallback, VTG fills speed + course, ZDA supplies the date, GSA backfills HDOP. Columns: timestamp (added when a date is known), time, latitude, longitude, altitude_<unit>, fix, satellites, hdop, speed_<unit>, course. coordinates picks decimal-degrees or DMS; altitude_unit is m/ft; speed_unit is knots/kmh/mph; delimiter picks the separator; header toggles the header row; validate_checksum drops lines that fail their *XX checksum. Paste the log in the 'nmea' field. Runs entirely in-browser; nothing is uploaded.",
        parameters = schema_json()
    )
)]
impl NmeaToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "nmea-to-csv", |a: Args| {
            convert_str(
                &a.nmea,
                &a.coordinates,
                &a.altitude_unit,
                &a.speed_unit,
                &a.delimiter,
                a.header,
                a.validate_checksum,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. (Regenerate this literal when the descriptor changes.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "nmea": { "type": "string", "description": "The NMEA 0183 log to convert. Paste the raw serial text — one sentence per line such as $GPGGA,... , $GNRMC,... , $GPGLL,... , $GPVTG,... , $GPZDA,... , $GPGSA,... (any talker prefix: GP/GN/GL/GA/GB)." },
                    "coordinates": { "type": "string", "enum": ["decimal", "dms"], "default": "decimal", "description": "Coordinate output format. decimal (default) writes signed decimal degrees (e.g. 48.1173, -11.5167); dms writes degrees-minutes-seconds with a hemisphere letter (e.g. 48°07'02.28\"N)." },
                    "altitude_unit": { "type": "string", "enum": ["m", "ft"], "default": "m", "description": "Unit for the altitude column. m (default) keeps GGA metres; ft converts to feet. The column header is altitude_m or altitude_ft accordingly." },
                    "speed_unit": { "type": "string", "enum": ["knots", "kmh", "mph"], "default": "knots", "description": "Unit for the speed column. knots (default) matches NMEA's native speed-over-ground; kmh is km/h; mph is miles/h. The column header is speed_knots, speed_kmh, or speed_mph." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "Output CSV field separator. Default comma. Use semicolon for comma-decimal locales, tab for TSV, or pipe." },
                    "header": { "type": "boolean", "default": true, "description": "Emit the column-name header row. Default true; set false for a headerless CSV." },
                    "validate_checksum": { "type": "boolean", "default": false, "description": "Verify each line's NMEA *XX checksum and drop any sentence that fails it. Default false (parse every line). Lines that carry no checksum are always parsed." }
                },
                "required": ["nmea"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
