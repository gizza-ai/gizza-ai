//! gizza-ai/super-timeline-builder — chat skill block on the shared tool abstraction.
//! Merges the CSV exports of several already-parsed forensic artifacts into one
//! chronologically sorted super-timeline. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    artifacts: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default = "default_true")]
    expand: bool,
    #[serde(default = "default_true")]
    dedupe: bool,
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
    #[serde(default)]
    tz_offset: f64,
    #[serde(default)]
    drop_epoch_zero: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_limit")]
    limit: f64,
}

fn default_format() -> String {
    "csv".to_string()
}
fn default_order() -> String {
    "asc".to_string()
}
fn default_delimiter() -> String {
    "auto".to_string()
}
fn default_true() -> bool {
    true
}
fn default_limit() -> f64 {
    10_000.0
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("artifacts")
                .required()
                .describe("The parsed-artifact CSVs to merge, pasted one after another. Introduce each one with a header line naming the source — `--- mft.csv ---`, `=== evtx ===`, GNU tail `==> prefetch <==`, or `# mft` — and give each its own header row (e.g. `Path,Created,LastModified`). A blob with no header line is read as a single artifact named artifact1."),
        )
        .param(
            Param::enumv("format", ["csv", "l2tcsv", "tln"])
                .default("csv")
                .describe("Output layout: csv (compact datetime,timestamp_desc,source,message), l2tcsv (the 17-field legacy log2timeline CSV that Timeline Explorer and the SANS template read), or tln (pipe-delimited Time|Source|Host|User|Description with epoch-second times)."),
        )
        .param(
            Param::enumv("order", ["asc", "desc"])
                .default("asc")
                .describe("Sort direction of the merged timeline: asc (oldest first, the default) or desc (newest first)."),
        )
        .param(
            Param::boolean("expand")
                .default(true)
                .describe("When true (default), emit one row per timestamp COLUMN — an MFT row with Created/LastModified/LastAccess becomes three timeline rows, each labelled with its column name. When false, only the first timestamp column of each artifact is used."),
        )
        .param(
            Param::boolean("dedupe")
                .default(true)
                .describe("When true (default), drop repeats of an identical (time, source, timestamp type, message) row — useful when two exports overlap."),
        )
        .param(
            Param::string("from")
                .default("")
                .describe("Earliest event to keep, inclusive — e.g. 2024-06-01 or 2024-06-01T10:00:00Z. Empty means no lower bound."),
        )
        .param(
            Param::string("to")
                .default("")
                .describe("Latest event to keep, inclusive — e.g. 2024-06-02 or 2024-06-02T23:59:59Z. Empty means no upper bound."),
        )
        .param(
            Param::number("tz_offset")
                .default(0.0)
                .min(-14.0)
                .max(14.0)
                .describe("Hours that timezone-less input timestamps are offset from UTC, e.g. -5 for US Eastern standard time or 5.5 for India. Values that already carry a Z or ±hh:mm offset are unaffected. Output is always UTC. Default 0."),
        )
        .param(
            Param::boolean("drop_epoch_zero")
                .default(false)
                .describe("When true, drop rows that land exactly on 1970-01-01T00:00:00Z — the usual placeholder for a null/zeroed timestamp. Off by default so nothing disappears silently."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe("Field separator of the pasted CSVs. auto (default) detects comma, tab, semicolon or pipe per section from its header row; set it explicitly when a section's data confuses the detector."),
        )
        .param(
            Param::integer("limit")
                .default(10_000)
                .min(1.0)
                .max(100_000.0)
                .describe("Maximum rows in the merged timeline (1-100000, default 10000). Producing more is an error naming the actual count, never a silent trim — raise the limit or narrow from/to."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/super-timeline-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge parsed forensic artifact CSVs into one sorted super-timeline",
    skill(
        description = "Merge the CSV exports of several already-parsed forensic artifacts (MFT listing, event-log export, prefetch, registry, browser history) into ONE chronologically sorted super-timeline. Paste the CSVs one after another, each under a header line naming its source (`--- mft.csv ---`, `=== evtx ===`, `==> prefetch <==`, `# mft`); every section keeps its own columns and delimiter. Timestamp columns are auto-detected by header name (Created, LastWriteTime, TimeCreated, datetime, epoch, …) or by ISO 8601 values, and split date + time columns are recombined; with expand=true (default) every timestamp column becomes its own row, labelled with that column's name, the way a super-timeline expands one MFT record into Created/Modified/Accessed lines. Times normalize to UTC (use tz_offset for timezone-less input); from/to filter an inclusive range, dedupe drops identical repeats, drop_epoch_zero removes null 1970 placeholders, and order is asc or desc. Output is csv (datetime,timestamp_desc,source,message), l2tcsv (the 17-field legacy log2timeline layout) or tln (pipe-delimited). Up to 100000 rows. Runs locally — evidence never leaves the machine.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "super-timeline-builder", |a: Args| {
            gizza_ai_super_timeline_builder_core::build(
                &a.artifacts,
                &a.format,
                &a.order,
                a.expand,
                a.dedupe,
                &a.from,
                &a.to,
                a.tz_offset,
                a.drop_epoch_zero,
                &a.delimiter,
                a.limit.round().max(0.0) as u32,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "artifacts": { "type": "string", "description": "The parsed-artifact CSVs to merge, pasted one after another. Introduce each one with a header line naming the source — `--- mft.csv ---`, `=== evtx ===`, GNU tail `==> prefetch <==`, or `# mft` — and give each its own header row (e.g. `Path,Created,LastModified`). A blob with no header line is read as a single artifact named artifact1." },
                    "format": { "type": "string", "enum": ["csv", "l2tcsv", "tln"], "default": "csv", "description": "Output layout: csv (compact datetime,timestamp_desc,source,message), l2tcsv (the 17-field legacy log2timeline CSV that Timeline Explorer and the SANS template read), or tln (pipe-delimited Time|Source|Host|User|Description with epoch-second times)." },
                    "order": { "type": "string", "enum": ["asc", "desc"], "default": "asc", "description": "Sort direction of the merged timeline: asc (oldest first, the default) or desc (newest first)." },
                    "expand": { "type": "boolean", "default": true, "description": "When true (default), emit one row per timestamp COLUMN — an MFT row with Created/LastModified/LastAccess becomes three timeline rows, each labelled with its column name. When false, only the first timestamp column of each artifact is used." },
                    "dedupe": { "type": "boolean", "default": true, "description": "When true (default), drop repeats of an identical (time, source, timestamp type, message) row — useful when two exports overlap." },
                    "from": { "type": "string", "default": "", "description": "Earliest event to keep, inclusive — e.g. 2024-06-01 or 2024-06-01T10:00:00Z. Empty means no lower bound." },
                    "to": { "type": "string", "default": "", "description": "Latest event to keep, inclusive — e.g. 2024-06-02 or 2024-06-02T23:59:59Z. Empty means no upper bound." },
                    "tz_offset": { "type": "number", "default": 0.0, "minimum": -14, "maximum": 14, "description": "Hours that timezone-less input timestamps are offset from UTC, e.g. -5 for US Eastern standard time or 5.5 for India. Values that already carry a Z or ±hh:mm offset are unaffected. Output is always UTC. Default 0." },
                    "drop_epoch_zero": { "type": "boolean", "default": false, "description": "When true, drop rows that land exactly on 1970-01-01T00:00:00Z — the usual placeholder for a null/zeroed timestamp. Off by default so nothing disappears silently." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe"], "default": "auto", "description": "Field separator of the pasted CSVs. auto (default) detects comma, tab, semicolon or pipe per section from its header row; set it explicitly when a section's data confuses the detector." },
                    "limit": { "type": "integer", "default": 10000, "minimum": 1, "maximum": 100000, "description": "Maximum rows in the merged timeline (1-100000, default 10000). Producing more is an error naming the actual count, never a silent trim — raise the limit or narrow from/to." }
                },
                "required": ["artifacts"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
