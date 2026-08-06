//! gizza-ai/spreadsheet-formula-audit — scan spreadsheet formulas for breakage.
//!
//! No-page block (chat + CLI only, like `xlsx-to-csv`): it ingests binary
//! workbook bytes (.xlsx/.xlsm/.xls/.ods), which is not a pure text page input.
//! It resolves `url`/`ref` sources, parses the workbook with calamine, and emits
//! a text audit report in table/json/csv formats.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_spreadsheet_formula_audit_core::{audit, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    sheet: Option<String>,
    #[serde(default = "default_true")]
    check_cycles: bool,
    #[serde(default = "default_true")]
    check_error_values: bool,
    #[serde(default = "default_true")]
    include_hidden: bool,
    #[serde(default = "default_max_findings")]
    max_findings: usize,
    #[serde(default)]
    format: String,
}

fn default_true() -> bool {
    true
}
fn default_max_findings() -> usize {
    200
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("sheet").describe("Worksheet to report on: a sheet name, or a 0-based index as a string (e.g. \"0\"). Blank/default scans all worksheets."))
        .param(Param::boolean("check_cycles").default(true).describe("Build a formula dependency graph and report circular references (default true). Literal A1 references are followed; INDIRECT/OFFSET/table names are documented limits."))
        .param(Param::boolean("check_error_values").default(true).describe("Report cells whose cached workbook value is an Excel error such as #REF!, #DIV/0!, #VALUE!, #NAME?, #N/A, #NUM!, or #NULL! (default true)."))
        .param(Param::boolean("include_hidden").default(true).describe("Include hidden and very-hidden worksheets when scanning the whole workbook (default true)."))
        .param(Param::number("max_findings").default(200.0).describe("Maximum findings to include in the report before clipping with a note (default 200)."))
        .param(Param::enumv("format", ["table", "json", "csv"]).default("table").describe("Output format: readable fixed-width table (default), structured JSON, or CSV rows."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SpreadsheetFormulaAudit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spreadsheet-formula-audit",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan a workbook for circular formulas, broken references, and formula error cells",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Scan a spreadsheet workbook (.xlsx, .xlsm, .xls, or .ods) for formula problems. Provide the file via url or ref. The audit reports formulas containing #REF!, cells with cached Excel error values, formulas that point at missing sheets, external workbook links, and circular references in the formula dependency graph. It reads only the stored workbook bytes and does not recalculate formulas; INDIRECT/OFFSET and structured table references are documented limits.",
        parameters = schema_json()
    ),
)]
impl SpreadsheetFormulaAudit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("spreadsheet-formula-audit")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;
    let max_findings = args.max_findings.clamp(1, 10_000);
    let opts = Options {
        sheet: args.sheet.filter(|s| !s.trim().is_empty()),
        check_cycles: args.check_cycles,
        check_error_values: args.check_error_values,
        include_hidden: args.include_hidden,
        max_findings,
    };
    let format = if args.format.trim().is_empty() {
        "table"
    } else {
        args.format.trim()
    };
    let mut out = audit(&bytes, &opts, format).map_err(SkillError::InvalidArgs)?;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("\nSource workbook: {filename}\n"));
    Ok(out.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                "sheet": { "type": "string", "description": "Worksheet to report on: a sheet name, or a 0-based index as a string (e.g. \"0\"). Blank/default scans all worksheets." },
                "check_cycles": { "type": "boolean", "default": true, "description": "Build a formula dependency graph and report circular references (default true). Literal A1 references are followed; INDIRECT/OFFSET/table names are documented limits." },
                "check_error_values": { "type": "boolean", "default": true, "description": "Report cells whose cached workbook value is an Excel error such as #REF!, #DIV/0!, #VALUE!, #NAME?, #N/A, #NUM!, or #NULL! (default true)." },
                "include_hidden": { "type": "boolean", "default": true, "description": "Include hidden and very-hidden worksheets when scanning the whole workbook (default true)." },
                "max_findings": { "type": "number", "default": 200.0, "description": "Maximum findings to include in the report before clipping with a note (default 200)." },
                "format": { "type": "string", "enum": ["table", "json", "csv"], "default": "table", "description": "Output format: readable fixed-width table (default), structured JSON, or CSV rows." }
            },
            "additionalProperties": false,
            "oneOf": [ { "required": ["url"] }, { "required": ["ref"] } ]
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_deserialize_url_defaults() {
        use gizza_ai_block_utils::Source;
        let a: Args = serde_json::from_str(r#"{"url":"https://x/book.xlsx"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(ref u) if u == "https://x/book.xlsx"));
        assert!(a.check_cycles);
        assert!(a.check_error_values);
        assert!(a.include_hidden);
        assert_eq!(a.max_findings, 200);
        assert_eq!(a.format, "");
    }
}
