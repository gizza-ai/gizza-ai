//! gizza-ai/nmap-output-parser — parse nmap XML or greppable output into tables.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_nmap_output_parser_core::{parse, InputFormat, Options, OutputFormat, SortBy};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_sort_by")]
    sort_by: String,
    #[serde(default = "default_open_only")]
    open_only: bool,
}

fn default_format() -> String {
    "auto".into()
}
fn default_output() -> String {
    "markdown".into()
}
fn default_sort_by() -> String {
    "host".into()
}
fn default_open_only() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Raw nmap output to parse: XML from `nmap -oX` or greppable text from `nmap -oG`."),
        )
        .param(
            Param::enumv("format", ["auto", "xml", "greppable"])
                .default("auto")
                .describe("Input format: auto-detect (default), xml for nmap `-oX`, or greppable for nmap `-oG`."),
        )
        .param(
            Param::enumv("output", ["markdown", "csv", "json"])
                .default("markdown")
                .describe("Output format for the parsed port table: markdown (default), csv, or json."),
        )
        .param(
            Param::enumv("sort_by", ["host", "port", "service"])
                .default("host")
                .describe("Sort rows by host (default; IPv4 numerically), port, or service."),
        )
        .param(
            Param::boolean("open_only")
                .default(true)
                .describe("Only include open/open|filtered ports. Turn off to include closed or filtered ports too."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, SkillError> {
    let opts = Options {
        input_format: InputFormat::parse(&a.format),
        output_format: OutputFormat::parse(&a.output),
        sort_by: SortBy::parse(&a.sort_by),
        open_only: a.open_only,
    };
    parse(&a.input, opts).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct NmapOutputParser;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/nmap-output-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse nmap XML or greppable output into host/port tables",
    skill(
        description = "Parse nmap scan output into a clean sortable table of hosts, hostnames, ports, protocols, states, services and detected versions. Paste XML from `nmap -oX` or greppable output from `nmap -oG`; format can auto-detect, xml, or greppable. output can be markdown, csv or json. sort_by can be host, port or service. open_only defaults true so closed/filtered ports are hidden unless explicitly requested. Runs locally.",
        parameters = schema_json()
    ),
)]
impl NmapOutputParser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "nmap-output-parser", run_args) {
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
                    "input":     { "type": "string", "description": "Raw nmap output to parse: XML from `nmap -oX` or greppable text from `nmap -oG`." },
                    "format":    { "type": "string", "enum": ["auto", "xml", "greppable"], "default": "auto", "description": "Input format: auto-detect (default), xml for nmap `-oX`, or greppable for nmap `-oG`." },
                    "output":    { "type": "string", "enum": ["markdown", "csv", "json"], "default": "markdown", "description": "Output format for the parsed port table: markdown (default), csv, or json." },
                    "sort_by":   { "type": "string", "enum": ["host", "port", "service"], "default": "host", "description": "Sort rows by host (default; IPv4 numerically), port, or service." },
                    "open_only": { "type": "boolean", "default": true, "description": "Only include open/open|filtered ports. Turn off to include closed or filtered ports too." }
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
