//! gizza-ai/gap-finder — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_auto")]
    id_format: String,
    #[serde(default = "default_auto")]
    separator: String,
    #[serde(default = "default_step")]
    step: f64,
    #[serde(default)]
    start: String,
    #[serde(default)]
    end: String,
    #[serde(default = "default_sorted")]
    order: String,
    #[serde(default = "default_true")]
    duplicates: bool,
    #[serde(default = "default_report")]
    output: String,
    #[serde(default = "default_limit")]
    limit: f64,
}

fn default_auto() -> String {
    "auto".to_string()
}
fn default_sorted() -> String {
    "sorted".to_string()
}
fn default_report() -> String {
    "report".to_string()
}
fn default_true() -> bool {
    true
}
fn default_step() -> f64 {
    1.0
}
fn default_limit() -> f64 {
    1000.0
}

/// Whole-number coercion for the two numeric params (they arrive as JSON numbers).
fn whole(n: f64, field: &str) -> Result<i64, SkillError> {
    if !n.is_finite() || n.fract() != 0.0 {
        return Err(SkillError::InvalidArgs(format!(
            "{field} must be a whole number (got {n})"
        )));
    }
    Ok(n as i64)
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The sequence to audit, one entry per line (commas, semicolons, tabs, pipes, and spaces also work). Plain numbers like 1004 or IDs like INV-1004 and 2026-0007 — the last run of digits in each entry is treated as the counter. Max 20,000 entries."))
        .param(Param::enumv("id_format", ["auto", "number"]).default("auto").describe("How entries are parsed: 'auto' (default) accepts plain numbers and prefixed/suffixed IDs, reproducing the prefix and zero padding in the results; 'number' accepts plain integers only and reports anything else as an error."))
        .param(Param::enumv("separator", ["auto", "newline", "comma", "space", "semicolon", "tab", "pipe"]).default("auto").describe("How the pasted entries are separated. 'auto' (default) splits on any of newline, comma, semicolon, pipe, or whitespace; pick an explicit separator when an ID contains one of the others (e.g. a space inside 'INV 1004')."))
        .param(Param::integer("step").default(1).min(1.0).max(1_000_000.0).describe("Expected spacing between consecutive entries — 1 (default) for 1,2,3, 2 for even-numbered cheques, 10 for 10,20,30. Entries inside the range that do not land on the step are reported as off-step."))
        .param(Param::string("start").default("").describe("Expected first value of the sequence, e.g. 1000 or INV-1000. Leave empty (default) to start at the lowest value found. Use it to catch values missing from the beginning of the run."))
        .param(Param::string("end").default("").describe("Expected last value of the sequence, e.g. 1250 or INV-1250. Leave empty (default) to stop at the highest value found. Use it to catch values missing from the end of the run."))
        .param(Param::enumv("order", ["sorted", "input"]).default("sorted").describe("How the pasted order is treated: 'sorted' (default) audits the entries as an unordered set; 'input' additionally reports entries that appear after a higher entry, which flags a ledger recorded out of sequence."))
        .param(Param::boolean("duplicates").default(true).describe("Report values that appear more than once, with their occurrence counts (default true). Duplicates count once towards the present total either way."))
        .param(Param::enumv("output", ["report", "missing", "table", "json"]).default("report").describe("Output shape: 'report' (default) is a summary with compressed gap ranges; 'missing' lists every missing value one per line for pasting elsewhere; 'table' is a TSV of gap_start/gap_end/count; 'json' is a structured result with every list."))
        .param(Param::integer("limit").default(1000).min(1.0).max(gizza_ai_gap_finder_core::MAX_LIMIT as f64).describe("Maximum number of entries listed in the output — gap ranges in report/table mode, individual values in missing/json mode (default 1000, max 20,000). Counts and totals are always complete; truncation is stated in the output."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/gap-finder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find missing values, gap ranges, and duplicates in a sequence that should be consecutive",
    skill(
        description = "Audit a pasted sequence of numbers or IDs (invoice numbers, order IDs, cheque numbers, serials) for gaps. Reports the missing values as compressed ranges, duplicates, entries that do not land on the expected step, values outside the expected range, and entries pasted out of order. Options: id_format auto|number, separator auto|newline|comma|space|semicolon|tab|pipe, step (default 1), start/end expected bounds (default: the lowest/highest value found), order sorted|input, duplicates true|false, output report|missing|table|json, limit (default 1000). Max 20,000 entries and 5,000,000 expected slots.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "gap-finder", |a: Args| {
            let step = whole(a.step, "step")?;
            let limit = whole(a.limit, "limit")?;
            if limit < 0 {
                return Err(SkillError::InvalidArgs("limit must be positive".to_string()));
            }
            gizza_ai_gap_finder_core::run(
                &a.data,
                &a.id_format,
                &a.separator,
                step,
                &a.start,
                &a.end,
                &a.order,
                a.duplicates,
                &a.output,
                limit as usize,
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
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["properties"]["data"]["type"], "string");
        assert_eq!(v["required"], serde_json::json!(["data"]));
        assert_eq!(
            v["properties"]["id_format"]["enum"],
            serde_json::json!(["auto", "number"])
        );
        assert_eq!(v["properties"]["id_format"]["default"], "auto");
        assert_eq!(
            v["properties"]["separator"]["enum"],
            serde_json::json!(["auto", "newline", "comma", "space", "semicolon", "tab", "pipe"])
        );
        assert_eq!(v["properties"]["separator"]["default"], "auto");
        assert_eq!(v["properties"]["step"]["type"], "integer");
        assert_eq!(v["properties"]["step"]["default"], 1);
        assert_eq!(v["properties"]["step"]["minimum"], 1.0);
        assert_eq!(v["properties"]["start"]["default"], "");
        assert_eq!(v["properties"]["end"]["default"], "");
        assert_eq!(
            v["properties"]["order"]["enum"],
            serde_json::json!(["sorted", "input"])
        );
        assert_eq!(v["properties"]["order"]["default"], "sorted");
        assert_eq!(v["properties"]["duplicates"]["default"], true);
        assert_eq!(
            v["properties"]["output"]["enum"],
            serde_json::json!(["report", "missing", "table", "json"])
        );
        assert_eq!(v["properties"]["output"]["default"], "report");
        assert_eq!(v["properties"]["limit"]["default"], 1000);
        assert_eq!(v["properties"]["limit"]["maximum"], 20000.0);
    }

    #[test]
    fn whole_rejects_fractional_numbers() {
        assert_eq!(whole(2.0, "step").unwrap(), 2);
        assert!(matches!(whole(1.5, "step"), Err(SkillError::InvalidArgs(_))));
        assert!(matches!(
            whole(f64::NAN, "limit"),
            Err(SkillError::InvalidArgs(_))
        ));
    }
}
