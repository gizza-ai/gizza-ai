//! Browser-facing wasm-bindgen wrapper for /tools/worklog-summarizer/.
//! The page passes every field as a string in meta.toml order; this parses the
//! options and delegates to the deterministic core.
use gizza_ai_worklog_summarizer_core::{summarize, GroupBy, Options, OutputFormat, SortBy, Units};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    log: &str,
    group_by: &str,
    output: &str,
    units: &str,
    round: &str,
    max_entry: &str,
    end_time: &str,
    from: &str,
    to: &str,
    filter: &str,
    default_project: &str,
    sort: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        group_by: GroupBy::parse(if group_by.trim().is_empty() { "all" } else { group_by })
            .map_err(|e| JsValue::from_str(&e))?,
        output: OutputFormat::parse(if output.trim().is_empty() { "summary" } else { output })
            .map_err(|e| JsValue::from_str(&e))?,
        units: Units::parse(if units.trim().is_empty() { "hm" } else { units })
            .map_err(|e| JsValue::from_str(&e))?,
        round: round.trim().parse::<i64>().unwrap_or(0),
        max_entry: max_entry.trim().parse::<i64>().unwrap_or(0),
        end_time: end_time.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        filter: filter.to_string(),
        default_project: if default_project.trim().is_empty() {
            "(untagged)".into()
        } else {
            default_project.to_string()
        },
        sort: SortBy::parse(if sort.trim().is_empty() { "duration" } else { sort })
            .map_err(|e| JsValue::from_str(&e))?,
    };
    summarize(log, &opts).map_err(|e| JsValue::from_str(&e))
}
