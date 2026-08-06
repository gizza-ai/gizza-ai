//! gizza-ai/registry-hive-parser — chat skill block on the shared tool abstraction.
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
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    path: String,
    #[serde(default = "default_max_entries")]
    max_entries: i64,
}

fn default_input_encoding() -> String {
    "hex".to_string()
}
fn default_mode() -> String {
    "summary".to_string()
}
fn default_max_entries() -> i64 {
    50
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The raw Windows registry hive bytes encoded as hex (default) or Base64. Paste NTUSER.DAT, SYSTEM, SOFTWARE, SAM, SECURITY, USRCLASS.DAT, or Amcache.hve bytes after encoding them; the tool never uploads the hive."),
        )
        .param(
            Param::enumv("input_encoding", ["hex", "base64"])
                .default("hex")
                .describe("How the hive bytes in data are encoded. 'hex' accepts contiguous or whitespace-separated bytes with an optional leading 0x. 'base64' accepts standard Base64 with optional whitespace."),
        )
        .param(
            Param::enumv("mode", ["summary", "path", "runkeys"])
                .default("summary")
                .describe("What to inspect: 'summary' reports the regf header, integrity flags, root subkeys, and root values; 'path' browses one backslash-separated key path relative to the hive root; 'runkeys' probes common Run/RunOnce/autostart locations for NTUSER.DAT, SOFTWARE, and SYSTEM hives."),
        )
        .param(
            Param::string("path")
                .default("")
                .describe("For mode='path', a registry key path relative to the loaded hive root, such as Software\\Microsoft\\Windows\\CurrentVersion\\Run. Do not include HKCU or HKLM prefixes; leave blank for the root key."),
        )
        .param(
            Param::integer("max_entries")
                .default(50)
                .min(1.0)
                .max(1000.0)
                .describe("Maximum subkeys or values shown in each section. Use a small number for chat-friendly output; the tool clamps values above 1000. Default 50."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/registry-hive-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Browse Windows registry hive headers, keys, values, and RunKeys.",
    skill(
        description = "Parse an offline Windows registry hive (NTUSER.DAT, SYSTEM, SOFTWARE, SAM, SECURITY, USRCLASS.DAT, Amcache.hve, and other regf files) from hex or Base64 text. Report the base-block header, checksum and dirty/truncated flags, browse a key path relative to the hive root, or sweep common Run/RunOnce/autostart locations used during DFIR triage. Structured traversal uses a pure Rust registry parser; when a damaged hive cannot be walked, the tool still reports the header and carves key names from nk cells with honest limits. Runs locally; no upload.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "registry-hive-parser", |a: Args| {
            gizza_ai_registry_hive_parser_core::run(
                &a.data,
                &a.input_encoding,
                &a.mode,
                &a.path,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The raw Windows registry hive bytes encoded as hex (default) or Base64. Paste NTUSER.DAT, SYSTEM, SOFTWARE, SAM, SECURITY, USRCLASS.DAT, or Amcache.hve bytes after encoding them; the tool never uploads the hive." },
                    "input_encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How the hive bytes in data are encoded. 'hex' accepts contiguous or whitespace-separated bytes with an optional leading 0x. 'base64' accepts standard Base64 with optional whitespace." },
                    "mode": { "type": "string", "enum": ["summary", "path", "runkeys"], "default": "summary", "description": "What to inspect: 'summary' reports the regf header, integrity flags, root subkeys, and root values; 'path' browses one backslash-separated key path relative to the hive root; 'runkeys' probes common Run/RunOnce/autostart locations for NTUSER.DAT, SOFTWARE, and SYSTEM hives." },
                    "path": { "type": "string", "default": "", "description": "For mode='path', a registry key path relative to the loaded hive root, such as Software\\Microsoft\\Windows\\CurrentVersion\\Run. Do not include HKCU or HKLM prefixes; leave blank for the root key." },
                    "max_entries": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 50, "description": "Maximum subkeys or values shown in each section. Use a small number for chat-friendly output; the tool clamps values above 1000. Default 50." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
