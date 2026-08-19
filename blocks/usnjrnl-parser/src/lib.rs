//! gizza-ai/usnjrnl-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure Rust, no host calls:
//! the `$UsnJrnl:$J` bytes are supplied as hex or Base64 text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_input_encoding")]
    input_encoding: String,
    #[serde(default = "default_event")]
    event: String,
    #[serde(default = "default_include")]
    include: String,
    #[serde(default)]
    filter: String,
    #[serde(default = "default_pair_renames")]
    pair_renames: bool,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    host: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_max_entries")]
    max_entries: i64,
}

fn default_input_encoding() -> String {
    "hex".to_string()
}
fn default_event() -> String {
    "all".to_string()
}
fn default_include() -> String {
    "all".to_string()
}
fn default_pair_renames() -> bool {
    true
}
fn default_mode() -> String {
    "report".to_string()
}
fn default_sort() -> String {
    "usn".to_string()
}
fn default_max_entries() -> i64 {
    200
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The raw NTFS $Extend\\$UsnJrnl:$J change-journal bytes, encoded as hex (default) or Base64, e.g. the output of `xxd -p -c 256 J`. The stream need not start on a record boundary — the scanner resynchronises — so a carved fragment or a `dd` chunk works. Parsed locally; never uploaded."),
        )
        .param(
            Param::enumv("input_encoding", ["hex", "base64"])
                .default("hex")
                .describe("How the journal bytes in data are encoded. 'hex' accepts contiguous or separated bytes with an optional leading 0x; 'base64' accepts standard Base64 with optional whitespace."),
        )
        .param(
            Param::enumv("event", ["all", "create", "delete", "rename", "write", "metadata", "close"])
                .default("all")
                .describe("Keep only one class of change. 'create' = USN_REASON_FILE_CREATE, 'delete' = FILE_DELETE, 'rename' = either rename half, 'write' = the data and named-stream overwrite/extend/truncate reasons, 'metadata' = security, EA, attribute, reparse, object-id and similar changes, 'close' = records carrying the CLOSE bit (the final record of a change burst). Default 'all'."),
        )
        .param(
            Param::enumv("include", ["all", "files", "dirs"])
                .default("all")
                .describe("Filter by FILE_ATTRIBUTE_DIRECTORY: 'all' (default), 'files' for non-directory records only, or 'dirs' for directory records only."),
        )
        .param(
            Param::string("filter")
                .default("")
                .describe("Optional case-insensitive substring matched against the file name, and against the new name of a paired rename, e.g. \".exe\", \"invoice\" or \"AppData\". Applied before the max_entries cap."),
        )
        .param(
            Param::boolean("pair_renames")
                .default(true)
                .describe("Merge each USN_REASON_RENAME_OLD_NAME record with its following RENAME_NEW_NAME record for the same file so a rename reads as one row (\"old.txt -> new.txt\") instead of two halves. Set false to see the raw journal records exactly as NTFS wrote them. Default true."),
        )
        .param(
            Param::enumv("mode", ["summary", "report", "list", "csv", "bodyfile", "tln", "json"])
                .default("report")
                .describe("Output format. 'summary' gives triage counts, the USN and UTC time span and the most-active names over everything that matched; 'report' is a detailed block per record; 'list' is one dense line per record; 'csv' is a spreadsheet table; 'bodyfile' emits Sleuth Kit/mactime rows; 'tln' emits epoch|source|host|user|description timeline rows; 'json' returns every decoded field plus the scan accounting."),
        )
        .param(
            Param::string("host")
                .default("")
                .describe("Host or system name written into the host column of the TLN timeline (mode=tln), e.g. \"WKSTN-04\". Empty renders \"-\". Ignored by every other mode."),
        )
        .param(
            Param::enumv("sort", ["usn", "time", "name"])
                .default("usn")
                .describe("Ordering applied before the entry cap. 'usn' is journal order (ascending update sequence number, chronological by construction) and is the default; 'time' is newest first; 'name' is file name A-Z."),
        )
        .param(
            Param::integer("max_entries")
                .default(200)
                .min(1.0)
                .max(5000.0)
                .describe("Maximum records emitted after filtering and sorting. Values above 5000 are clamped, and the output states when the list was capped. Summary mode always counts every matched record regardless. Default 200."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/usnjrnl-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an NTFS $UsnJrnl:$J change journal into a file-activity timeline.",
    skill(
        description = "Parse an NTFS $Extend\\$UsnJrnl:$J change journal supplied as hex or Base64 and report the file creation, rename, write, metadata-change, close and deletion events NTFS recorded over time. Decodes USN_RECORD V2 and V3 (128-bit file references) layouts, counts V4 range-tracking records, and skips the sparse (zeroed) regions and unparseable runs a $J commonly contains, resynchronising on the 8-byte record alignment so carved fragments still parse. Each row carries the UTC timestamp, update sequence number, file name, decoded USN_REASON flags, decoded FILE_ATTRIBUTE flags, the MFT entry and sequence numbers of both the file and its parent directory, the USN_SOURCE flags and the security id. RENAME_OLD_NAME and RENAME_NEW_NAME records are merged into one rename row by default. Filter by change class, files vs directories or a name substring; sort by journal order, time or name; and choose a summary, detailed report, dense list, CSV, Sleuth Kit bodyfile, TLN timeline or full JSON output. Note that $J stores parent reference numbers but no parent names, so full paths need a $MFT listing (see the mft-parser tool) — the parent MFT entry is emitted on every row for that join. Runs locally; no upload.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "usnjrnl-parser", |a: Args| {
            gizza_ai_usnjrnl_parser_core::run(
                &a.data,
                &a.input_encoding,
                &a.event,
                &a.include,
                &a.filter,
                a.pair_renames,
                &a.mode,
                &a.host,
                &a.sort,
                a.max_entries.max(0) as usize,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The raw NTFS $Extend\\$UsnJrnl:$J change-journal bytes, encoded as hex (default) or Base64, e.g. the output of `xxd -p -c 256 J`. The stream need not start on a record boundary — the scanner resynchronises — so a carved fragment or a `dd` chunk works. Parsed locally; never uploaded." },
                    "input_encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How the journal bytes in data are encoded. 'hex' accepts contiguous or separated bytes with an optional leading 0x; 'base64' accepts standard Base64 with optional whitespace." },
                    "event": { "type": "string", "enum": ["all", "create", "delete", "rename", "write", "metadata", "close"], "default": "all", "description": "Keep only one class of change. 'create' = USN_REASON_FILE_CREATE, 'delete' = FILE_DELETE, 'rename' = either rename half, 'write' = the data and named-stream overwrite/extend/truncate reasons, 'metadata' = security, EA, attribute, reparse, object-id and similar changes, 'close' = records carrying the CLOSE bit (the final record of a change burst). Default 'all'." },
                    "include": { "type": "string", "enum": ["all", "files", "dirs"], "default": "all", "description": "Filter by FILE_ATTRIBUTE_DIRECTORY: 'all' (default), 'files' for non-directory records only, or 'dirs' for directory records only." },
                    "filter": { "type": "string", "default": "", "description": "Optional case-insensitive substring matched against the file name, and against the new name of a paired rename, e.g. \".exe\", \"invoice\" or \"AppData\". Applied before the max_entries cap." },
                    "pair_renames": { "type": "boolean", "default": true, "description": "Merge each USN_REASON_RENAME_OLD_NAME record with its following RENAME_NEW_NAME record for the same file so a rename reads as one row (\"old.txt -> new.txt\") instead of two halves. Set false to see the raw journal records exactly as NTFS wrote them. Default true." },
                    "mode": { "type": "string", "enum": ["summary", "report", "list", "csv", "bodyfile", "tln", "json"], "default": "report", "description": "Output format. 'summary' gives triage counts, the USN and UTC time span and the most-active names over everything that matched; 'report' is a detailed block per record; 'list' is one dense line per record; 'csv' is a spreadsheet table; 'bodyfile' emits Sleuth Kit/mactime rows; 'tln' emits epoch|source|host|user|description timeline rows; 'json' returns every decoded field plus the scan accounting." },
                    "host": { "type": "string", "default": "", "description": "Host or system name written into the host column of the TLN timeline (mode=tln), e.g. \"WKSTN-04\". Empty renders \"-\". Ignored by every other mode." },
                    "sort": { "type": "string", "enum": ["usn", "time", "name"], "default": "usn", "description": "Ordering applied before the entry cap. 'usn' is journal order (ascending update sequence number, chronological by construction) and is the default; 'time' is newest first; 'name' is file name A-Z." },
                    "max_entries": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 200, "description": "Maximum records emitted after filtering and sorting. Values above 5000 are clamped, and the output states when the list was capped. Summary mode always counts every matched record regardless. Default 200." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
