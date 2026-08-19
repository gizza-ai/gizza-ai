//! gizza-ai/cartesian-to-polar-csv — chat skill block on the shared tool abstraction.
//!
//! Batch-convert a CSV of 2D points between Cartesian (x, y) and polar (r, θ)
//! coordinates. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI); `handle()` delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    csv: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default)]
    x_column: String,
    #[serde(default)]
    y_column: String,
    #[serde(default = "default_angle_unit")]
    angle_unit: String,
    #[serde(default = "default_angle_range")]
    angle_range: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default = "default_true")]
    keep_columns: bool,
    #[serde(default = "default_output")]
    output: String,
}

fn default_direction() -> String {
    "cartesian_to_polar".to_string()
}
fn default_angle_unit() -> String {
    "degrees".to_string()
}
fn default_angle_range() -> String {
    "signed".to_string()
}
fn default_decimals() -> i64 {
    6
}
fn default_delimiter() -> String {
    "auto".to_string()
}
fn default_true() -> bool {
    true
}
fn default_output() -> String {
    "csv".to_string()
}

impl Args {
    fn run(&self) -> Result<String, String> {
        gizza_ai_cartesian_to_polar_csv_core::convert(
            &self.csv,
            &self.direction,
            &self.x_column,
            &self.y_column,
            &self.angle_unit,
            &self.angle_range,
            self.decimals,
            &self.delimiter,
            self.has_header,
            self.keep_columns,
            &self.output,
        )
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("csv").required().describe(
            "The CSV text of 2D points, one point per row, for example a header line x,y \
             followed by rows like 3,4. Extra columns such as id or label are allowed and are \
             carried through. Input is capped at 5 MB and 200000 data rows.",
        ))
        .param(
            Param::enumv("direction", ["cartesian_to_polar", "polar_to_cartesian"])
                .default("cartesian_to_polar")
                .describe(
                    "Conversion direction. cartesian_to_polar (default) reads x and y and emits \
                     r and theta; polar_to_cartesian reads r and theta and emits x and y.",
                ),
        )
        .param(Param::string("x_column").describe(
            "Column holding x, or holding r when direction is polar_to_cartesian. Give a header \
             name such as x or easting, or a 1-based column number such as 2. Leave empty to \
             auto-detect names like x, easting, r, rho or radius, falling back to the first column.",
        ))
        .param(Param::string("y_column").describe(
            "Column holding y, or holding the angle when direction is polar_to_cartesian. Give a \
             header name such as y or theta, or a 1-based column number such as 3. Leave empty to \
             auto-detect names like y, northing, theta, phi or angle, falling back to the second column.",
        ))
        .param(
            Param::enumv("angle_unit", ["degrees", "radians", "gradians", "turns"])
                .default("degrees")
                .describe(
                    "Unit for the angle: degrees (default, full turn 360), radians (full turn \
                     2*pi), gradians (full turn 400) or turns (full turn 1). It applies to the \
                     theta output when converting to polar and to the theta input when converting back.",
                ),
        )
        .param(
            Param::enumv("angle_range", ["signed", "positive"])
                .default("signed")
                .describe(
                    "Range for the reported angle. signed (default) is the atan2 range, -180 to \
                     180 degrees; positive wraps negatives up into 0 to 360 degrees. Both scale \
                     with angle_unit and only affect cartesian_to_polar output.",
                ),
        )
        .param(
            Param::integer("decimals")
                .default(6.0)
                .min(0.0)
                .max(15.0)
                .describe(
                    "Decimal places for every converted number, from 0 through 15. The default 6 \
                     keeps sub-millimetre precision on metre-scale data; 15 is the practical \
                     limit of 64-bit floating point.",
                ),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "semicolon", "tab", "pipe"])
                .default("auto")
                .describe(
                    "Field delimiter of the input. auto (default) sniffs the first non-empty line \
                     for comma, semicolon, tab or pipe. CSV output is written back with the same \
                     delimiter.",
                ),
        )
        .param(Param::boolean("has_header").default(true).describe(
            "Treat the first row as a header of column names. Turn this off for bare numeric rows \
             like 3,4 — columns are then named column1, column2 and no header row is written.",
        ))
        .param(Param::boolean("keep_columns").default(true).describe(
            "Carry every non-coordinate column (id, label, timestamp, ...) through to the output, \
             in its original order, with the two converted values appended after them. Turn off to \
             emit only the converted pair.",
        ))
        .param(
            Param::enumv("output", ["csv", "tsv", "json", "table"])
                .default("csv")
                .describe(
                    "Output shape. csv (default) reuses the input delimiter; tsv is tab-separated; \
                     json is an array of one object per row; table is a right-aligned plain-text \
                     table for reading.",
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
    name = "gizza-ai/cartesian-to-polar-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a CSV of x,y points to polar r,theta coordinates",
    skill(
        description = "Batch-convert a CSV of 2D points between Cartesian (x, y) and polar (r, theta) coordinates. Paste the CSV in the csv parameter; the tool finds the coordinate columns by header name or 1-based index (x_column / y_column), auto-detecting the usual spellings (x/y, easting/northing, r/rho/radius, theta/phi/angle) when they are left empty. Cartesian to polar computes r = sqrt(x^2 + y^2) and theta = atan2(y, x), so all four quadrants are correct; polar to cartesian computes x = r*cos(theta) and y = r*sin(theta). The angle can be reported in degrees, radians, gradians or turns, either signed (-180 to 180 degrees) or wrapped positive (0 to 360 degrees), rounded to 0-15 decimals. Non-coordinate columns such as id or label are carried through by default, the delimiter (comma, semicolon, tab, pipe) is sniffed automatically, and output can be CSV, TSV, JSON or an aligned text table. Bad cells report their row number. Pure local Rust/WASM; no network, no plotting, no 3D or geodetic coordinate systems.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cartesian-to-polar-csv", |a: Args| {
            a.run().map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> Args {
        Args {
            csv: "id,x,y\np1,3,4\n".into(),
            direction: default_direction(),
            x_column: String::new(),
            y_column: String::new(),
            angle_unit: default_angle_unit(),
            angle_range: default_angle_range(),
            decimals: 2,
            delimiter: default_delimiter(),
            has_header: true,
            keep_columns: true,
            output: default_output(),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"object",
                "properties":{
                    "csv":{"type":"string","description":"The CSV text of 2D points, one point per row, for example a header line x,y followed by rows like 3,4. Extra columns such as id or label are allowed and are carried through. Input is capped at 5 MB and 200000 data rows."},
                    "direction":{"type":"string","enum":["cartesian_to_polar","polar_to_cartesian"],"default":"cartesian_to_polar","description":"Conversion direction. cartesian_to_polar (default) reads x and y and emits r and theta; polar_to_cartesian reads r and theta and emits x and y."},
                    "x_column":{"type":"string","description":"Column holding x, or holding r when direction is polar_to_cartesian. Give a header name such as x or easting, or a 1-based column number such as 2. Leave empty to auto-detect names like x, easting, r, rho or radius, falling back to the first column."},
                    "y_column":{"type":"string","description":"Column holding y, or holding the angle when direction is polar_to_cartesian. Give a header name such as y or theta, or a 1-based column number such as 3. Leave empty to auto-detect names like y, northing, theta, phi or angle, falling back to the second column."},
                    "angle_unit":{"type":"string","enum":["degrees","radians","gradians","turns"],"default":"degrees","description":"Unit for the angle: degrees (default, full turn 360), radians (full turn 2*pi), gradians (full turn 400) or turns (full turn 1). It applies to the theta output when converting to polar and to the theta input when converting back."},
                    "angle_range":{"type":"string","enum":["signed","positive"],"default":"signed","description":"Range for the reported angle. signed (default) is the atan2 range, -180 to 180 degrees; positive wraps negatives up into 0 to 360 degrees. Both scale with angle_unit and only affect cartesian_to_polar output."},
                    "decimals":{"type":"integer","default":6.0,"minimum":0,"maximum":15,"description":"Decimal places for every converted number, from 0 through 15. The default 6 keeps sub-millimetre precision on metre-scale data; 15 is the practical limit of 64-bit floating point."},
                    "delimiter":{"type":"string","enum":["auto","comma","semicolon","tab","pipe"],"default":"auto","description":"Field delimiter of the input. auto (default) sniffs the first non-empty line for comma, semicolon, tab or pipe. CSV output is written back with the same delimiter."},
                    "has_header":{"type":"boolean","default":true,"description":"Treat the first row as a header of column names. Turn this off for bare numeric rows like 3,4 — columns are then named column1, column2 and no header row is written."},
                    "keep_columns":{"type":"boolean","default":true,"description":"Carry every non-coordinate column (id, label, timestamp, ...) through to the output, in its original order, with the two converted values appended after them. Turn off to emit only the converted pair."},
                    "output":{"type":"string","enum":["csv","tsv","json","table"],"default":"csv","description":"Output shape. csv (default) reuses the input delimiter; tsv is tab-separated; json is an array of one object per row; table is a right-aligned plain-text table for reading."}
                },
                "required":["csv"],
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_layer_converts_and_keeps_extra_columns() {
        assert_eq!(args().run().unwrap(), "id,r,theta\np1,5.00,53.13\n");
    }

    #[test]
    fn args_layer_surfaces_core_errors() {
        let mut a = args();
        a.csv = "id,x,y\np1,left,4\n".into();
        let err = a.run().unwrap_err();
        assert!(err.contains("is not a number"), "{err}");
    }
}
