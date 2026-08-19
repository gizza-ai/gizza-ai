//! gizza-ai/amcache-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure Rust, no host calls:
//! the hive bytes are supplied as hex or Base64 text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_input_encoding")]
    input_encoding: String,
    #[serde(default = "default_section")]
    section: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_association")]
    association: String,
    #[serde(default)]
    filter: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_max_entries")]
    max_entries: i64,
}

fn default_input_encoding() -> String { "hex".to_string() }
fn default_section() -> String { "auto".to_string() }
fn default_mode() -> String { "report".to_string() }
fn default_association() -> String { "all".to_string() }
fn default_sort() -> String { "time".to_string() }
fn default_max_entries() -> i64 { 200 }

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The raw Windows Amcache.hve registry hive bytes encoded as hex (default) or Base64. Paste the whole regf hive; the tool parses it locally and never uploads it."),
        )
        .param(
            Param::enumv("input_encoding", ["hex", "base64"])
                .default("hex")
                .describe("How the hive bytes in data are encoded. 'hex' accepts contiguous or separated bytes with an optional leading 0x; 'base64' accepts standard Base64 with optional whitespace."),
        )
        .param(
            Param::enumv("section", ["auto", "files", "programs", "drivers", "shortcuts", "all"])
                .default("auto")
                .describe("Which Amcache containers to report. 'auto' shows programs and executable file records; 'files', 'programs', 'drivers' and 'shortcuts' select one artifact family; 'all' includes every known modern and legacy container."),
        )
        .param(
            Param::enumv("mode", ["report", "list", "csv", "bodyfile", "hashes"])
                .default("report")
                .describe("Output format. 'report' is grouped and human-readable; 'list' prints one dense line per record; 'csv' emits a spreadsheet table; 'bodyfile' emits Sleuth Kit/mactime rows; 'hashes' prints a de-duplicated SHA-1 list."),
        )
        .param(
            Param::enumv("association", ["all", "associated", "unassociated"])
                .default("all")
                .describe("Filter file-like records by whether their ProgramId resolves to an installed-program record. Program records themselves remain visible; use 'unassociated' to focus on orphan executable records."),
        )
        .param(
            Param::string("filter")
                .default("")
                .describe("Optional case-insensitive substring matched against path, name, publisher, SHA-1, program id and extra values before the entry cap is applied."),
        )
        .param(
            Param::enumv("sort", ["time", "path", "none"])
                .default("time")
                .describe("Ordering before the entry cap. 'time' sorts by key last-write newest first; 'path' sorts by display path or name; 'none' keeps hive traversal order."),
        )
        .param(
            Param::integer("max_entries")
                .default(200)
                .min(1.0)
                .max(5000.0)
                .describe("Maximum records emitted after filtering. Values above 5000 are clamped, and truncation is reported in the output. Default 200."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/amcache-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse Amcache.hve program and executable evidence from a registry hive.",
    skill(
        description = "Parse a Windows Amcache.hve registry hive supplied as hex or Base64 and report installed applications, executable file records, driver binaries and shortcuts from both modern Root\\Inventory* containers and legacy Root\\File/Root\\Programs schemas. Extracts paths, names, publishers, versions, SHA-1 hashes, ProgramId associations, file size, key last-write times, PE link dates and install dates. Output a grouped report, one-line list, CSV, Sleuth Kit bodyfile or de-duplicated hash list. Runs locally; no upload.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "amcache-parser", |a: Args| {
            gizza_ai_amcache_parser_core::run(
                &a.data,
                &a.input_encoding,
                &a.section,
                &a.mode,
                &a.association,
                &a.filter,
                &a.sort,
                a.max_entries.max(0) as usize,
            ).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The raw Windows Amcache.hve registry hive bytes encoded as hex (default) or Base64. Paste the whole regf hive; the tool parses it locally and never uploads it." },
                    "input_encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How the hive bytes in data are encoded. 'hex' accepts contiguous or separated bytes with an optional leading 0x; 'base64' accepts standard Base64 with optional whitespace." },
                    "section": { "type": "string", "enum": ["auto", "files", "programs", "drivers", "shortcuts", "all"], "default": "auto", "description": "Which Amcache containers to report. 'auto' shows programs and executable file records; 'files', 'programs', 'drivers' and 'shortcuts' select one artifact family; 'all' includes every known modern and legacy container." },
                    "mode": { "type": "string", "enum": ["report", "list", "csv", "bodyfile", "hashes"], "default": "report", "description": "Output format. 'report' is grouped and human-readable; 'list' prints one dense line per record; 'csv' emits a spreadsheet table; 'bodyfile' emits Sleuth Kit/mactime rows; 'hashes' prints a de-duplicated SHA-1 list." },
                    "association": { "type": "string", "enum": ["all", "associated", "unassociated"], "default": "all", "description": "Filter file-like records by whether their ProgramId resolves to an installed-program record. Program records themselves remain visible; use 'unassociated' to focus on orphan executable records." },
                    "filter": { "type": "string", "default": "", "description": "Optional case-insensitive substring matched against path, name, publisher, SHA-1, program id and extra values before the entry cap is applied." },
                    "sort": { "type": "string", "enum": ["time", "path", "none"], "default": "time", "description": "Ordering before the entry cap. 'time' sorts by key last-write newest first; 'path' sorts by display path or name; 'none' keeps hive traversal order." },
                    "max_entries": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 200, "description": "Maximum records emitted after filtering. Values above 5000 are clamped, and truncation is reported in the output. Default 200." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
