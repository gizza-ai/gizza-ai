//! gizza-ai/arff-converter — convert Weka ARFF datasets to CSV and back. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_arff_converter_core::{convert, ArffFormat, Direction, Options};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    relation: String,
    #[serde(default = "default_nominal_threshold")]
    nominal_threshold: i64,
    #[serde(default)]
    column_types: String,
    #[serde(default)]
    date_format: String,
    #[serde(default)]
    missing_value: String,
    #[serde(default)]
    arff_format: String,
    #[serde(default)]
    type_row: bool,
}
fn default_true() -> bool {
    true
}
fn default_nominal_threshold() -> i64 {
    10
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The dataset text to convert: either a Weka ARFF document (@relation/@attribute/@data) or a CSV table. Up to 2,000,000 characters."),
        )
        .param(
            Param::enumv("direction", ["auto", "arff-to-csv", "csv-to-arff"])
                .default("auto")
                .describe("Which way to convert. 'auto' (default) reads the input and picks ARFF-to-CSV when it starts with an @relation/@attribute/@data line, otherwise CSV-to-ARFF."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("CSV field separator, used for the CSV side in both directions: a single character or 'comma'/'tab'/'semicolon'/'pipe'/'space'. Default ','."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Whether the CSV has a header row of column names. Reading CSV, false generates att1, att2, … names; writing CSV, false omits the name row. Default true."),
        )
        .param(
            Param::string("relation")
                .describe("The @relation name written when converting CSV to ARFF. Leave empty for 'data'."),
        )
        .param(
            Param::integer("nominal_threshold")
                .default(10)
                .min(0.0)
                .max(1000.0)
                .describe("CSV to ARFF: a non-numeric column with at most this many distinct values becomes a nominal {a,b,c} attribute; more makes it a string attribute. Default 10; 0 never produces nominal attributes."),
        )
        .param(
            Param::string("column_types")
                .describe("CSV to ARFF: force column types, as comma-separated 'column:type' pairs such as 'class:nominal,id:string,3:date'. The column is a header name or a 1-based index; the type is numeric, nominal, string or date. Overrides the inferred type."),
        )
        .param(
            Param::string("date_format")
                .default("yyyy-MM-dd'T'HH:mm:ss")
                .describe("Java SimpleDateFormat pattern written for date attributes (columns typed 'date' in column_types). Default \"yyyy-MM-dd'T'HH:mm:ss\"."),
        )
        .param(
            Param::string("missing_value")
                .describe("The CSV text that stands for ARFF's missing value '?'. Empty (the default) writes an empty cell and reads both empty cells and '?' as missing."),
        )
        .param(
            Param::enumv("arff_format", ["dense", "sparse"])
                .default("dense")
                .describe("Row style when writing ARFF: 'dense' one comma-separated row per instance (default), or 'sparse' {index value, …} rows that omit zeros and first nominal labels. Sparse input is always read correctly regardless of this setting."),
        )
        .param(
            Param::boolean("type_row")
                .describe("Keep attribute types in the CSV: writing CSV, adds a second header row holding each ARFF type (numeric, string, date pattern, {a,b} label set); reading CSV, consumes that row instead of guessing types, so ARFF to CSV to ARFF round-trips exactly. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn to_options(a: &Args) -> Result<Options, String> {
    Ok(Options {
        direction: Direction::parse(&a.direction)?,
        delimiter: if a.delimiter.is_empty() {
            ",".to_string()
        } else {
            a.delimiter.clone()
        },
        header: a.header,
        relation: a.relation.clone(),
        nominal_threshold: a.nominal_threshold,
        column_types: a.column_types.clone(),
        date_format: a.date_format.clone(),
        missing_value: a.missing_value.clone(),
        arff_format: ArffFormat::parse(&a.arff_format)?,
        type_row: a.type_row,
    })
}

#[cfg(target_arch = "wasm32")]
struct ArffConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/arff-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert Weka ARFF datasets to CSV and back",
    skill(
        description = "Convert a Weka ARFF machine-learning dataset to CSV, or a CSV table to ARFF, preserving attribute types. `direction` defaults to auto-detection. Reading ARFF it handles `%` comments, quoted values with \\n/\\t/\\\\ escapes, nominal `{a,b}` label sets, `date` attributes with a SimpleDateFormat pattern, `?` missing values, sparse `{index value, …}` rows (expanded with type-aware defaults: 0 for numeric, the first label for nominal, empty for string) and trailing `{weight}` instance weights (stripped). Writing ARFF it infers each column's type — all-numeric columns become numeric, columns with at most `nominal_threshold` distinct values become nominal label sets, the rest become string — with `column_types` forcing any column to numeric/nominal/string/date by header name or 1-based index, `relation` naming the dataset, `date_format` supplying the date pattern, and `arff_format=sparse` emitting sparse rows. `type_row` writes the ARFF types as a second CSV header line and reads it back, so ARFF→CSV→ARFF keeps its exact types. `delimiter` and `header` control the CSV side; `missing_value` sets the CSV token for `?`. Relational (multi-instance) attributes are rejected with an explicit error. Fully local and deterministic — no AI model.",
        parameters = schema_json()
    ),
)]
impl ArffConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "arff-converter", |a: Args| {
            let opts = to_options(&a).map_err(SkillError::InvalidArgs)?;
            convert(&a.data, &opts).map_err(SkillError::InvalidArgs)
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
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The dataset text to convert: either a Weka ARFF document (@relation/@attribute/@data) or a CSV table. Up to 2,000,000 characters." },
                    "direction": { "type": "string", "enum": ["auto", "arff-to-csv", "csv-to-arff"], "default": "auto", "description": "Which way to convert. 'auto' (default) reads the input and picks ARFF-to-CSV when it starts with an @relation/@attribute/@data line, otherwise CSV-to-ARFF." },
                    "delimiter": { "type": "string", "default": ",", "description": "CSV field separator, used for the CSV side in both directions: a single character or 'comma'/'tab'/'semicolon'/'pipe'/'space'. Default ','." },
                    "header": { "type": "boolean", "default": true, "description": "Whether the CSV has a header row of column names. Reading CSV, false generates att1, att2, … names; writing CSV, false omits the name row. Default true." },
                    "relation": { "type": "string", "description": "The @relation name written when converting CSV to ARFF. Leave empty for 'data'." },
                    "nominal_threshold": { "type": "integer", "default": 10, "minimum": 0, "maximum": 1000, "description": "CSV to ARFF: a non-numeric column with at most this many distinct values becomes a nominal {a,b,c} attribute; more makes it a string attribute. Default 10; 0 never produces nominal attributes." },
                    "column_types": { "type": "string", "description": "CSV to ARFF: force column types, as comma-separated 'column:type' pairs such as 'class:nominal,id:string,3:date'. The column is a header name or a 1-based index; the type is numeric, nominal, string or date. Overrides the inferred type." },
                    "date_format": { "type": "string", "default": "yyyy-MM-dd'T'HH:mm:ss", "description": "Java SimpleDateFormat pattern written for date attributes (columns typed 'date' in column_types). Default \"yyyy-MM-dd'T'HH:mm:ss\"." },
                    "missing_value": { "type": "string", "description": "The CSV text that stands for ARFF's missing value '?'. Empty (the default) writes an empty cell and reads both empty cells and '?' as missing." },
                    "arff_format": { "type": "string", "enum": ["dense", "sparse"], "default": "dense", "description": "Row style when writing ARFF: 'dense' one comma-separated row per instance (default), or 'sparse' {index value, …} rows that omit zeros and first nominal labels. Sparse input is always read correctly regardless of this setting." },
                    "type_row": { "type": "boolean", "description": "Keep attribute types in the CSV: writing CSV, adds a second header row holding each ARFF type (numeric, string, date pattern, {a,b} label set); reading CSV, consumes that row instead of guessing types, so ARFF to CSV to ARFF round-trips exactly. Default false." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    fn args() -> Args {
        Args {
            data: "a,b\n1,2".into(),
            direction: String::new(),
            delimiter: String::new(),
            header: true,
            relation: String::new(),
            nominal_threshold: 10,
            column_types: String::new(),
            date_format: String::new(),
            missing_value: String::new(),
            arff_format: String::new(),
            type_row: false,
        }
    }

    #[test]
    fn options_reject_an_unknown_direction() {
        let a = Args {
            direction: "sideways".into(),
            ..args()
        };
        let err = to_options(&a).unwrap_err();
        assert!(err.contains("auto/arff-to-csv/csv-to-arff"), "{err}");
    }

    #[test]
    fn options_reject_an_unknown_arff_format() {
        let a = Args {
            arff_format: "compact".into(),
            ..args()
        };
        let err = to_options(&a).unwrap_err();
        assert!(err.contains("dense/sparse"), "{err}");
    }

    #[test]
    fn empty_option_strings_fall_back_to_defaults() {
        let o = to_options(&args()).unwrap();
        assert_eq!(o.direction, Direction::Auto);
        assert_eq!(o.arff_format, ArffFormat::Dense);
        assert_eq!(o.delimiter, ",");
    }
}
