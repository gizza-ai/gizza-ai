//! gizza-ai/docker-cli-output-parser — chat skill block on the shared tool abstraction.
//!
//! Turns the human-readable tables printed by `docker ps`, `docker images` and
//! `docker stats` into JSON / CSV / TSV / Markdown / aligned text. The header
//! line is used as a fixed-width ruler, so column titles that contain spaces
//! (`CONTAINER ID`, `MEM USAGE / LIMIT`, `NET I/O`) and values that contain
//! spaces (`COMMAND`, `STATUS`, `CREATED`) are split correctly.
//!
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI + page); `handle()` delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    keys: String,
    #[serde(default = "default_true")]
    parse_values: bool,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    strict: bool,
    /// 0 → the core default (500); the core clamps to 1..=MAX_LIMIT.
    #[serde(default)]
    limit: u32,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The raw text printed by docker, INCLUDING the header line — the header is used as the fixed-width ruler that locates each column. Paste `docker ps`, `docker ps -a --size`, `docker images`, `docker images --digests` or `docker stats --no-stream` output as-is; a custom `--format 'table {{.ID}}\\t{{.Names}}'` header works too."),
        )
        .param(
            Param::enumv("kind", ["auto", "ps", "images", "stats"])
                .default("auto")
                .describe("Which docker command produced the text. 'auto' (default) reads the header line: CPU %/MEM USAGE / LIMIT means stats, REPOSITORY/IMAGE ID means images, CONTAINER ID/NAMES means ps. Set it explicitly to document intent; with strict=true a mismatch between this and the header is an error."),
        )
        .param(
            Param::enumv("output", ["json", "csv", "tsv", "markdown", "table"])
                .default("json")
                .describe("Output shape. 'json' (default) is an array of one object per row with typed values; 'csv' and 'tsv' are RFC-4180-quoted delimited text for spreadsheets; 'markdown' is a pipe table for docs and tickets; 'table' is aligned plain text like docker's own output."),
        )
        .param(
            Param::enumv("keys", ["snake", "header", "docker"])
                .default("snake")
                .describe("How columns are named. 'snake' (default) is script-friendly snake_case (container_id, cpu_percent, mem_usage_limit); 'header' keeps the printed titles verbatim ('CONTAINER ID', 'MEM USAGE / LIMIT'); 'docker' uses the `--format` template names so output matches `docker ... --format '{{json .}}'` (ID, Image, CPUPerc, MemUsage, NetIO, PIDs)."),
        )
        .param(
            Param::boolean("parse_values")
                .default(true)
                .describe("Type and split the values instead of returning raw strings. On (default): CPU %/MEM % become numbers, PIDS an integer, PORTS and NAMES become arrays, COMMAND is unquoted, SIZE gains size_bytes (and virtual_size when `docker ps --size` printed one), and the composite MEM USAGE / LIMIT, NET I/O and BLOCK I/O columns split into mem_usage/mem_limit, net_input/net_output, block_input/block_output plus a *_bytes count each. Docker's '--' placeholders become null. Off: every cell stays the exact text docker printed."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Comma-separated list of columns to keep, in the order you list them. Blank (default) keeps every column. Names are matched case- and punctuation-blind, so 'CONTAINER ID', 'container_id' and 'containerid' all select the same column; derived columns such as size_bytes or mem_usage are selectable too. Example: names,cpu_percent,mem_usage_bytes."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit a header/title row for csv, tsv, markdown and table output. On by default; turn it off to append rows to an existing file. JSON output is unaffected — its keys are always present."),
        )
        .param(
            Param::boolean("strict")
                .default(false)
                .describe("Fail instead of guessing. On: a row that is truncated (no value for the last column), a tab-separated row whose field count differs from the header, or a header that disagrees with an explicit 'kind' is an error. Off (default): missing cells are filled with empty values and a mismatched kind is parsed anyway."),
        )
        .param(
            Param::integer("limit")
                .min(1.0)
                .max(5000.0)
                .default(500)
                .describe("Maximum number of rows to emit. Default 500, maximum 5000; extra rows are dropped from the end."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/docker-cli-output-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse docker ps, docker images and docker stats output into JSON, CSV or a table",
    skill(
        description = "Convert the human-readable table printed by `docker ps`, `docker images` or `docker stats` into structured data — JSON, CSV, TSV, a Markdown table, or aligned text. The header line is used as a fixed-width ruler, so column titles containing spaces (CONTAINER ID, IMAGE ID, CREATED AT, CPU %, MEM USAGE / LIMIT, NET I/O, BLOCK I/O) and cell values containing spaces (COMMAND, CREATED, STATUS, PORTS) are split at the right boundaries — unlike whitespace splitting, which mangles them. The command kind is detected from the header, or you can state it. With value parsing on (default) percentages become numbers, PIDS an integer, PORTS and NAMES arrays, and the composite MEM USAGE / LIMIT, NET I/O and BLOCK I/O columns split into separate fields with byte counts (SI kB/MB/GB and binary KiB/MiB/GiB are both understood); docker's '--' placeholders become null. Column keys can be snake_case, the printed titles, or docker's own --format template names. Pick or reorder columns, cap rows, and turn on strict mode to fail on truncated rows instead of guessing. Use it when you have pasted output or a saved log and cannot re-run the command with --format '{{json .}}'. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "docker-cli-output-parser", |a: Args| {
            gizza_ai_docker_cli_output_parser_core::parse(
                &a.input,
                &a.kind,
                &a.output,
                &a.keys,
                a.parse_values,
                &a.columns,
                a.header,
                a.strict,
                a.limit,
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

    /// Drift guard: the chat/CLI schema must stay exactly what the page,
    /// manifest and docs were written against. Regenerate deliberately.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(AUTHORED).unwrap();
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(live, authored);
    }

    const AUTHORED: &str = r#"
        {
            "additionalProperties": false,
            "properties": {
                "columns": {
                    "default": "",
                    "description": "Comma-separated list of columns to keep, in the order you list them. Blank (default) keeps every column. Names are matched case- and punctuation-blind, so 'CONTAINER ID', 'container_id' and 'containerid' all select the same column; derived columns such as size_bytes or mem_usage are selectable too. Example: names,cpu_percent,mem_usage_bytes.",
                    "type": "string"
                },
                "header": {
                    "default": true,
                    "description": "Emit a header/title row for csv, tsv, markdown and table output. On by default; turn it off to append rows to an existing file. JSON output is unaffected — its keys are always present.",
                    "type": "boolean"
                },
                "input": {
                    "description": "The raw text printed by docker, INCLUDING the header line — the header is used as the fixed-width ruler that locates each column. Paste `docker ps`, `docker ps -a --size`, `docker images`, `docker images --digests` or `docker stats --no-stream` output as-is; a custom `--format 'table {{.ID}}\\t{{.Names}}'` header works too.",
                    "type": "string"
                },
                "keys": {
                    "default": "snake",
                    "description": "How columns are named. 'snake' (default) is script-friendly snake_case (container_id, cpu_percent, mem_usage_limit); 'header' keeps the printed titles verbatim ('CONTAINER ID', 'MEM USAGE / LIMIT'); 'docker' uses the `--format` template names so output matches `docker ... --format '{{json .}}'` (ID, Image, CPUPerc, MemUsage, NetIO, PIDs).",
                    "enum": [
                        "snake",
                        "header",
                        "docker"
                    ],
                    "type": "string"
                },
                "kind": {
                    "default": "auto",
                    "description": "Which docker command produced the text. 'auto' (default) reads the header line: CPU %/MEM USAGE / LIMIT means stats, REPOSITORY/IMAGE ID means images, CONTAINER ID/NAMES means ps. Set it explicitly to document intent; with strict=true a mismatch between this and the header is an error.",
                    "enum": [
                        "auto",
                        "ps",
                        "images",
                        "stats"
                    ],
                    "type": "string"
                },
                "limit": {
                    "default": 500,
                    "description": "Maximum number of rows to emit. Default 500, maximum 5000; extra rows are dropped from the end.",
                    "maximum": 5000,
                    "minimum": 1,
                    "type": "integer"
                },
                "output": {
                    "default": "json",
                    "description": "Output shape. 'json' (default) is an array of one object per row with typed values; 'csv' and 'tsv' are RFC-4180-quoted delimited text for spreadsheets; 'markdown' is a pipe table for docs and tickets; 'table' is aligned plain text like docker's own output.",
                    "enum": [
                        "json",
                        "csv",
                        "tsv",
                        "markdown",
                        "table"
                    ],
                    "type": "string"
                },
                "parse_values": {
                    "default": true,
                    "description": "Type and split the values instead of returning raw strings. On (default): CPU %/MEM % become numbers, PIDS an integer, PORTS and NAMES become arrays, COMMAND is unquoted, SIZE gains size_bytes (and virtual_size when `docker ps --size` printed one), and the composite MEM USAGE / LIMIT, NET I/O and BLOCK I/O columns split into mem_usage/mem_limit, net_input/net_output, block_input/block_output plus a *_bytes count each. Docker's '--' placeholders become null. Off: every cell stays the exact text docker printed.",
                    "type": "boolean"
                },
                "strict": {
                    "default": false,
                    "description": "Fail instead of guessing. On: a row that is truncated (no value for the last column), a tab-separated row whose field count differs from the header, or a header that disagrees with an explicit 'kind' is an error. Off (default): missing cells are filled with empty values and a mismatched kind is parsed anyway.",
                    "type": "boolean"
                }
            },
            "required": [
                "input"
            ],
            "type": "object"
        }
    "#;
}
