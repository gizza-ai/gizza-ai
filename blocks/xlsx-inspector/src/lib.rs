//! gizza-ai/xlsx-inspector — report the structure of a spreadsheet workbook.
//!
//! No-page block (chat + CLI only, like `xlsx-to-csv`, `xlsx-sheet-diff` and
//! `spreadsheet-formula-audit`): it ingests binary workbook bytes
//! (.xlsx/.xlsm/.xlsb/.xls/.ods), which the tool-page generator's pure runtime
//! cannot take as an input. It resolves `url`/`ref` sources, parses the workbook
//! with calamine, and emits a text report in table/json/csv formats.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_xlsx_inspector_core::{inspect, Options, NAMED_RANGE_CEILING};
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
    include_hidden: bool,
    #[serde(default = "default_true")]
    include_named_ranges: bool,
    #[serde(default = "default_true")]
    include_object_counts: bool,
    #[serde(default = "default_max_named_ranges")]
    max_named_ranges: f64,
    #[serde(default)]
    format: String,
}

fn default_true() -> bool {
    true
}
fn default_max_named_ranges() -> f64 {
    200.0
}

/// Whole-number guard for `max_named_ranges` — the CLI and chat surfaces both
/// hand numbers over as f64, so "10000" arrives as `10000.0` and a fractional
/// count has to be caught here rather than by the field type.
fn whole(name: &str, v: f64, lo: usize, hi: usize) -> Result<usize, String> {
    if !v.is_finite() || v.fract() != 0.0 {
        return Err(format!(
            "{name} must be a whole number between {lo} and {hi} (got {v})"
        ));
    }
    if v < lo as f64 || v > hi as f64 {
        return Err(format!("{name} must be between {lo} and {hi} (got {v})"));
    }
    Ok(v as usize)
}

fn to_options(args: &Args) -> Result<Options, String> {
    Ok(Options {
        sheet: args.sheet.clone().filter(|s| !s.trim().is_empty()),
        include_hidden: args.include_hidden,
        include_named_ranges: args.include_named_ranges,
        include_object_counts: args.include_object_counts,
        max_named_ranges: whole(
            "max_named_ranges",
            args.max_named_ranges,
            1,
            NAMED_RANGE_CEILING,
        )?,
    })
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("sheet").describe("Report on a single worksheet: a sheet name, or a 0-based index as a string (e.g. \"0\"). Blank/default reports every sheet in the workbook."))
        .param(Param::boolean("include_hidden").default(true).describe("Include hidden and very-hidden worksheets in the sheet table (default true). Set false to list only the tabs a reader can see."))
        .param(Param::boolean("include_named_ranges").default(true).describe("List the workbook's defined names (named ranges) with the reference each points at and whether it is broken (default true)."))
        .param(Param::boolean("include_object_counts").default(true).describe("Include the workbook object inventory — tables, pivot tables, charts, images, comments, drawings, external links, data connections, VBA macros (default true). Available for .xlsx/.xlsm/.xlsb only."))
        .param(Param::number("max_named_ranges").min(1.0).max(10000.0).default(200.0).describe("Maximum defined names listed before the section is clipped with a note (default 200, max 10000)."))
        .param(Param::enumv("format", ["table", "json", "csv"]).default("table").describe("Output format: readable fixed-width report (default), structured JSON with every count, or CSV with one row per sheet (named ranges and object counts appear only in table/json)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct XlsxInspector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xlsx-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Report a workbook's sheets, used ranges, named ranges, and formula vs value cell counts",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Inspect the structure of a spreadsheet workbook (.xlsx, .xlsm, .xlsb, .xls, or .ods). Provide the file via url or ref. The report lists every sheet with its type (worksheet/chart/macro/dialog), visibility (visible/hidden/very hidden), used range in A1 notation, row and column extent, cells with data, the formula vs value cell split, and a text/number/date/boolean/error type breakdown with per-error-literal counts. It also lists defined names (named ranges) with the reference each points at and flags broken #REF! names, and counts workbook objects (tables, pivot tables, charts, images, comments, drawings, external links, data connections, VBA macros) from the package parts. Values are whatever the writing application cached; nothing is recalculated. Named-range scope, name comments, merged regions, and per-sheet object attribution are not available from the stored file and are not reported.",
        parameters = schema_json()
    ),
)]
impl XlsxInspector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("xlsx-inspector")?;
    let opts = to_options(&args).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;
    let format = if args.format.trim().is_empty() {
        "table"
    } else {
        args.format.trim()
    };
    let mut out = inspect(&bytes, &opts, format).map_err(SkillError::InvalidArgs)?;
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
                "sheet": { "type": "string", "description": "Report on a single worksheet: a sheet name, or a 0-based index as a string (e.g. \"0\"). Blank/default reports every sheet in the workbook." },
                "include_hidden": { "type": "boolean", "default": true, "description": "Include hidden and very-hidden worksheets in the sheet table (default true). Set false to list only the tabs a reader can see." },
                "include_named_ranges": { "type": "boolean", "default": true, "description": "List the workbook's defined names (named ranges) with the reference each points at and whether it is broken (default true)." },
                "include_object_counts": { "type": "boolean", "default": true, "description": "Include the workbook object inventory — tables, pivot tables, charts, images, comments, drawings, external links, data connections, VBA macros (default true). Available for .xlsx/.xlsm/.xlsb only." },
                "max_named_ranges": { "type": "number", "minimum": 1, "maximum": 10000, "default": 200.0, "description": "Maximum defined names listed before the section is clipped with a note (default 200, max 10000)." },
                "format": { "type": "string", "enum": ["table", "json", "csv"], "default": "table", "description": "Output format: readable fixed-width report (default), structured JSON with every count, or CSV with one row per sheet (named ranges and object counts appear only in table/json)." }
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
        assert!(a.sheet.is_none());
        assert!(a.include_hidden);
        assert!(a.include_named_ranges);
        assert!(a.include_object_counts);
        assert_eq!(a.max_named_ranges, 200.0);
        assert_eq!(a.format, "");
    }

    #[test]
    fn args_accept_every_documented_override() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"abc","sheet":"2","include_hidden":false,"include_named_ranges":false,"include_object_counts":false,"max_named_ranges":5,"format":"json"}"#,
        )
        .unwrap();
        assert_eq!(a.sheet.as_deref(), Some("2"));
        assert!(!a.include_hidden);
        assert!(!a.include_named_ranges);
        assert!(!a.include_object_counts);
        assert_eq!(a.max_named_ranges, 5.0);
        assert_eq!(a.format, "json");
    }

    #[test]
    fn max_named_ranges_accepts_cli_numbers_but_rejects_fractional_or_out_of_range() {
        assert_eq!(
            whole("max_named_ranges", 10_000.0, 1, NAMED_RANGE_CEILING).unwrap(),
            10_000
        );
        assert_eq!(
            whole("max_named_ranges", 1.0, 1, NAMED_RANGE_CEILING).unwrap(),
            1
        );
        assert!(whole("max_named_ranges", 0.0, 1, NAMED_RANGE_CEILING)
            .unwrap_err()
            .contains("between 1 and 10000"));
        assert!(whole("max_named_ranges", 10_000.5, 1, NAMED_RANGE_CEILING)
            .unwrap_err()
            .contains("whole number"));
        assert!(whole("max_named_ranges", f64::NAN, 1, NAMED_RANGE_CEILING)
            .unwrap_err()
            .contains("whole number"));
    }
}
