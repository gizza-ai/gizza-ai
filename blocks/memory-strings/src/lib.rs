//! gizza-ai/memory-strings — chat skill block on the shared tool abstraction.
//! Runs a `strings`-style extraction over a pasted memory/process dump (raw text
//! or hex-encoded), then categorizes the recovered ASCII/UTF-16LE strings into
//! URLs, IPv4/IPv6, emails, domains, file paths and Windows registry keys. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    dump: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default = "default_min_length")]
    min_length: i64,
    #[serde(default = "default_categories")]
    categories: String,
    #[serde(default)]
    defang: bool,
}

fn default_input_format() -> String {
    "text".to_string()
}
fn default_encoding() -> String {
    "both".to_string()
}
fn default_min_length() -> i64 {
    4
}
fn default_categories() -> String {
    "all".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("dump")
                .required()
                .describe("The memory / process dump to scan. Paste it as raw text (its bytes are read directly) or as a hex-encoded dump when input_format=hex. Non-printable bytes act as string delimiters, exactly like the `strings` utility."),
        )
        .param(
            Param::enumv("input_format", ["text", "hex"])
                .default("text")
                .describe("How the dump is represented. 'text' (default) reads the pasted text's bytes directly. 'hex' first decodes a hex-encoded dump — contiguous hex, or bytes separated by spaces/colons/commas, with optional 0x prefixes (e.g. '48 65 6c', '48:65', '0x48 0x65')."),
        )
        .param(
            Param::enumv("encoding", ["both", "ascii", "utf16le"])
                .default("both")
                .describe("Which printable-string encodings to recover before categorizing. 'ascii' = 7-bit runs; 'utf16le' = wide runs (each character followed by a 0x00 byte, common in Windows memory); 'both' (default) recovers each."),
        )
        .param(
            Param::integer("min_length")
                .default(4)
                .min(1.0)
                .max(1024.0)
                .describe("Minimum printable run length to keep, like `strings -n`. Shorter runs are treated as noise and dropped. Default 4."),
        )
        .param(
            Param::string("categories")
                .default("all")
                .describe("Comma-separated subset of categories to report: any of 'url', 'ipv4', 'ipv6', 'ip' (both IP types), 'email', 'domain', 'path' (Windows/UNC/Unix file paths), 'registry' (Windows registry keys). 'all' (default) reports every category."),
        )
        .param(
            Param::boolean("defang")
                .default(false)
                .describe("When true, defang the URLs, IPs, domains and emails in the output (hxxp[://]…, 1[.]2[.]3[.]4, bad[at]evil[.]com) so the result is safe to paste into a report or ticket. Default false emits the real, clickable indicators."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/memory-strings",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract and categorize strings from a memory dump",
    skill(
        description = "Run a `strings`-style extraction over a pasted memory / process dump and categorize the results. First recovers the printable ASCII and/or UTF-16LE (wide) runs of at least min_length characters (like `strings -n`), treating non-printable bytes as delimiters; the dump can be raw text or a hex-encoded dump (input_format=hex). Then groups the recovered strings — de-duplicated, sorted and counted — into URLs, IPv4 and IPv6 addresses, emails, bare domains, file paths (Windows, UNC and Unix) and Windows registry keys. Use categories to report a subset and defang=true to make indicators safe for reports. Hash extraction is left to the ioc-extract tool. Runs entirely locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "memory-strings", |a: Args| {
            let min = a.min_length.max(1) as usize;
            gizza_ai_memory_strings_core::run(
                &a.dump,
                &a.input_format,
                &a.encoding,
                min,
                &a.categories,
                a.defang,
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
                    "dump": { "type": "string", "description": "The memory / process dump to scan. Paste it as raw text (its bytes are read directly) or as a hex-encoded dump when input_format=hex. Non-printable bytes act as string delimiters, exactly like the `strings` utility." },
                    "input_format": { "type": "string", "enum": ["text", "hex"], "default": "text", "description": "How the dump is represented. 'text' (default) reads the pasted text's bytes directly. 'hex' first decodes a hex-encoded dump — contiguous hex, or bytes separated by spaces/colons/commas, with optional 0x prefixes (e.g. '48 65 6c', '48:65', '0x48 0x65')." },
                    "encoding": { "type": "string", "enum": ["both", "ascii", "utf16le"], "default": "both", "description": "Which printable-string encodings to recover before categorizing. 'ascii' = 7-bit runs; 'utf16le' = wide runs (each character followed by a 0x00 byte, common in Windows memory); 'both' (default) recovers each." },
                    "min_length": { "type": "integer", "minimum": 1, "maximum": 1024, "default": 4, "description": "Minimum printable run length to keep, like `strings -n`. Shorter runs are treated as noise and dropped. Default 4." },
                    "categories": { "type": "string", "default": "all", "description": "Comma-separated subset of categories to report: any of 'url', 'ipv4', 'ipv6', 'ip' (both IP types), 'email', 'domain', 'path' (Windows/UNC/Unix file paths), 'registry' (Windows registry keys). 'all' (default) reports every category." },
                    "defang": { "type": "boolean", "default": false, "description": "When true, defang the URLs, IPs, domains and emails in the output (hxxp[://]…, 1[.]2[.]3[.]4, bad[at]evil[.]com) so the result is safe to paste into a report or ticket. Default false emits the real, clickable indicators." }
                },
                "required": ["dump"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
