//! gizza-ai/port-process-mapper — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure Rust, no host calls:
//! the socket listing is pasted in as text that was captured elsewhere.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_port_process_mapper_core as core;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default = "default_sort_by")]
    sort_by: String,
    #[serde(default = "default_true")]
    listening_only: bool,
    #[serde(default = "default_protocol")]
    protocol: String,
    #[serde(default)]
    ports: String,
    #[serde(default)]
    process: String,
    #[serde(default)]
    conflicts_only: bool,
    #[serde(default = "default_true")]
    annotate_services: bool,
    #[serde(default)]
    kill_commands: bool,
}

fn default_input_format() -> String { "auto".to_string() }
fn default_output_format() -> String { "markdown".to_string() }
fn default_sort_by() -> String { "port".to_string() }
fn default_protocol() -> String { "any".to_string() }
fn default_true() -> bool { true }

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The captured socket listing to map. Paste the output of `lsof -i -P -n`, `ss -tulpn`, `netstat -tulpn` (Linux/macOS) or `netstat -ano` / `netstat -anb` (Windows), header line included. Up to 20000 lines."),
        )
        .param(
            Param::enumv("input_format", ["auto", "lsof", "ss", "netstat", "netstat-windows"])
                .default("auto")
                .describe("Which command produced the input. 'auto' (default) detects the dialect from the header and row shape; set one explicitly when a trimmed paste is detected wrongly or reported as unrecognised."),
        )
        .param(
            Param::enumv("output_format", ["markdown", "csv", "json", "text"])
                .default("markdown")
                .describe("How to render the mapping. 'markdown' is a pipe table plus a summary and a port-conflict list; 'text' is the same table space-aligned for a terminal; 'csv' is one row per socket for a spreadsheet; 'json' returns rows, conflicts and summary counts as structured data."),
        )
        .param(
            Param::enumv("sort_by", ["port", "pid", "process", "state", "address"])
                .default("port")
                .describe("Row ordering. 'port' sorts numerically by local port (default), 'pid' by process id, 'process' by command name, 'state' by socket state (LISTEN first), 'address' by the bound local address."),
        )
        .param(
            Param::boolean("listening_only")
                .default(true)
                .describe("Keep only listening/bound server sockets (LISTEN, UNCONN and stateless UDP). Turn this off to also include established, time-wait and other client connections present in the paste."),
        )
        .param(
            Param::enumv("protocol", ["any", "tcp", "udp"])
                .default("any")
                .describe("Restrict the table to one transport protocol. 'any' (default) keeps both; 'tcp' and 'udp' match the normalised protocol, so tcp6/udp6 rows are included with their IPv4 counterparts."),
        )
        .param(
            Param::string("ports")
                .default("")
                .describe("Optional port filter: a comma-separated list of numbers and inclusive ranges, e.g. '80,443,8000-8100'. Empty (default) keeps every port. Rows whose port is a service name rather than a number are dropped when this filter is set."),
        )
        .param(
            Param::string("process")
                .default("")
                .describe("Optional case-insensitive substring matched against the process/command name, e.g. 'node' or 'nginx'. Empty (default) keeps every process."),
        )
        .param(
            Param::boolean("conflicts_only")
                .default(false)
                .describe("Show only the rows on ports that more than one distinct process is bound to — the answer to 'why is this port already in use?'. Conflicts are detected after the protocol, port, process and listening filters are applied."),
        )
        .param(
            Param::boolean("annotate_services")
                .default(true)
                .describe("Add a Service column naming the well-known service behind each port number (22 → ssh, 5432 → postgresql, 5173 → dev server (Vite), …). Unregistered ports show a dash."),
        )
        .param(
            Param::boolean("kill_commands")
                .default(false)
                .describe("Append a 'Free a port' section with ready-to-run `kill -9` (Linux/macOS) and `taskkill /PID … /F` (Windows) command lines pre-filled with every PID holding each listed port. Covers at most 20 ports."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/port-process-mapper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Map listening ports to PIDs and processes from lsof, ss or netstat output.",
    skill(
        description = "Turn a pasted socket listing into one normalised port → PID → process table and flag the ports more than one process is bound to. Accepts `lsof -i`, `ss -tulpn`, Linux `netstat -tulpn` and Windows `netstat -ano`/`-anb` output, auto-detecting the dialect. Normalises IPv4/IPv6 addresses, protocol, socket state, PID, command name and user; annotates well-known services (22 → ssh, 5432 → postgresql); filters by protocol, port list/ranges, process-name substring, listening-only or conflicts-only; sorts by port, PID, process, state or address; and can emit ready-to-run kill/taskkill commands for freeing a port. Renders as a markdown table, aligned text, CSV or JSON. Pure text in, text out — nothing is executed or uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "port-process-mapper", |a: Args| {
            core::parse(
                &a.input,
                core::Options {
                    input_format: core::InputFormat::parse(&a.input_format),
                    output_format: core::OutputFormat::parse(&a.output_format),
                    sort_by: core::SortBy::parse(&a.sort_by),
                    listening_only: a.listening_only,
                    protocol: core::Protocol::parse(&a.protocol),
                    ports: a.ports,
                    process: a.process,
                    conflicts_only: a.conflicts_only,
                    annotate_services: a.annotate_services,
                    kill_commands: a.kill_commands,
                },
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
                    "input": { "type": "string", "description": "The captured socket listing to map. Paste the output of `lsof -i -P -n`, `ss -tulpn`, `netstat -tulpn` (Linux/macOS) or `netstat -ano` / `netstat -anb` (Windows), header line included. Up to 20000 lines." },
                    "input_format": { "type": "string", "enum": ["auto", "lsof", "ss", "netstat", "netstat-windows"], "default": "auto", "description": "Which command produced the input. 'auto' (default) detects the dialect from the header and row shape; set one explicitly when a trimmed paste is detected wrongly or reported as unrecognised." },
                    "output_format": { "type": "string", "enum": ["markdown", "csv", "json", "text"], "default": "markdown", "description": "How to render the mapping. 'markdown' is a pipe table plus a summary and a port-conflict list; 'text' is the same table space-aligned for a terminal; 'csv' is one row per socket for a spreadsheet; 'json' returns rows, conflicts and summary counts as structured data." },
                    "sort_by": { "type": "string", "enum": ["port", "pid", "process", "state", "address"], "default": "port", "description": "Row ordering. 'port' sorts numerically by local port (default), 'pid' by process id, 'process' by command name, 'state' by socket state (LISTEN first), 'address' by the bound local address." },
                    "listening_only": { "type": "boolean", "default": true, "description": "Keep only listening/bound server sockets (LISTEN, UNCONN and stateless UDP). Turn this off to also include established, time-wait and other client connections present in the paste." },
                    "protocol": { "type": "string", "enum": ["any", "tcp", "udp"], "default": "any", "description": "Restrict the table to one transport protocol. 'any' (default) keeps both; 'tcp' and 'udp' match the normalised protocol, so tcp6/udp6 rows are included with their IPv4 counterparts." },
                    "ports": { "type": "string", "default": "", "description": "Optional port filter: a comma-separated list of numbers and inclusive ranges, e.g. '80,443,8000-8100'. Empty (default) keeps every port. Rows whose port is a service name rather than a number are dropped when this filter is set." },
                    "process": { "type": "string", "default": "", "description": "Optional case-insensitive substring matched against the process/command name, e.g. 'node' or 'nginx'. Empty (default) keeps every process." },
                    "conflicts_only": { "type": "boolean", "default": false, "description": "Show only the rows on ports that more than one distinct process is bound to — the answer to 'why is this port already in use?'. Conflicts are detected after the protocol, port, process and listening filters are applied." },
                    "annotate_services": { "type": "boolean", "default": true, "description": "Add a Service column naming the well-known service behind each port number (22 → ssh, 5432 → postgresql, 5173 → dev server (Vite), …). Unregistered ports show a dash." },
                    "kill_commands": { "type": "boolean", "default": false, "description": "Append a 'Free a port' section with ready-to-run `kill -9` (Linux/macOS) and `taskkill /PID … /F` (Windows) command lines pre-filled with every PID holding each listed port. Covers at most 20 ports." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
