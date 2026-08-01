//! gizza-ai/csv-column-mapping-suggest — suggest source→target CSV column mappings.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_column_mapping_suggest_core::{run, Delimiter, Options, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    source: String,
    target: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_header")]
    header: bool,
    #[serde(default = "default_sample_rows")]
    sample_rows: u64,
    #[serde(default = "default_header_weight")]
    header_weight: f64,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_format")]
    format: String,
}

fn default_delimiter() -> String {
    "comma".into()
}
fn default_header() -> bool {
    true
}
fn default_sample_rows() -> u64 {
    50
}
fn default_header_weight() -> f64 {
    0.6
}
fn default_threshold() -> f64 {
    0.3
}
fn default_format() -> String {
    "table".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("source").required().describe("Source CSV text whose columns need to be mapped. Include the header row unless header=false."))
        .param(Param::string("target").required().describe("Target CSV text or schema CSV to map into. Include the desired target header row unless header=false."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("CSV delimiter used by both inputs: comma (default), tab, semicolon, or pipe."))
        .param(Param::boolean("header").default(true).describe("Whether the first row contains column names. When false, columns are named column_1, column_2, and so on."))
        .param(Param::integer("sample_rows").min(0.0).max(500.0).default(50).describe("How many data rows to sample for value-overlap scoring. Use 0 for header-only matching. Default 50, max 500."))
        .param(Param::number("header_weight").min(0.0).max(1.0).default(0.6).describe("Weight given to header-name similarity from 0.0 to 1.0; the remaining weight goes to value overlap. Default 0.6."))
        .param(Param::number("threshold").min(0.0).max(1.0).default(0.3).describe("Minimum combined score required to suggest a mapping. Raise it for stricter suggestions or lower it to surface weak matches. Default 0.3."))
        .param(Param::enumv("format", ["table", "json", "csv"]).default("table").describe("Output format: table (Markdown-style report), json (machine-readable suggestions), or csv (flat mapping rows). Default table."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Options {
    Options {
        delimiter: Delimiter::parse(&a.delimiter),
        header: a.header,
        sample_rows: a.sample_rows.min(500) as usize,
        header_weight: a.header_weight,
        threshold: a.threshold,
        format: OutputFormat::parse(&a.format),
    }
}

#[cfg(target_arch = "wasm32")]
struct CsvColumnMappingSuggest;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-column-mapping-suggest",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Suggest source-to-target CSV column mappings from headers and sample values",
    skill(
        description = "Suggest a one-to-one mapping between two CSV files with differently named columns. The tool parses source and target CSV text, compares normalized header tokens and sampled value overlap, then ranks source→target suggestions with confidence scores and reasons. delimiter is comma/tab/semicolon/pipe; header toggles whether the first row is a header; sample_rows controls value-overlap rows (0 = header-only); header_weight balances header similarity vs value overlap; threshold filters weak suggestions; format is table, json, or csv. Runs locally and does not upload the CSV data.",
        parameters = schema_json()
    ),
)]
impl CsvColumnMappingSuggest {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-column-mapping-suggest", |a: Args| {
            let opts = build_options(&a);
            run(&a.source, &a.target, &opts).map_err(SkillError::InvalidArgs)
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "source":{"type":"string","description":"Source CSV text whose columns need to be mapped. Include the header row unless header=false."},
                "target":{"type":"string","description":"Target CSV text or schema CSV to map into. Include the desired target header row unless header=false."},
                "delimiter":{"type":"string","enum":["comma","tab","semicolon","pipe"],"default":"comma","description":"CSV delimiter used by both inputs: comma (default), tab, semicolon, or pipe."},
                "header":{"type":"boolean","default":true,"description":"Whether the first row contains column names. When false, columns are named column_1, column_2, and so on."},
                "sample_rows":{"type":"integer","minimum":0,"maximum":500,"default":50,"description":"How many data rows to sample for value-overlap scoring. Use 0 for header-only matching. Default 50, max 500."},
                "header_weight":{"type":"number","minimum":0,"maximum":1,"default":0.6,"description":"Weight given to header-name similarity from 0.0 to 1.0; the remaining weight goes to value overlap. Default 0.6."},
                "threshold":{"type":"number","minimum":0,"maximum":1,"default":0.3,"description":"Minimum combined score required to suggest a mapping. Raise it for stricter suggestions or lower it to surface weak matches. Default 0.3."},
                "format":{"type":"string","enum":["table","json","csv"],"default":"table","description":"Output format: table (Markdown-style report), json (machine-readable suggestions), or csv (flat mapping rows). Default table."}
            },
            "required":["source","target"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
