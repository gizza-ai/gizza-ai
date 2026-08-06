//! gizza-ai/shellbags-parser — chat skill block on the shared tool abstraction.
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
    #[serde(default = "default_bag_root")]
    bag_root: String,
    #[serde(default)]
    custom_path: String,
    #[serde(default = "default_max_entries")]
    max_entries: i64,
    #[serde(default = "default_max_depth")]
    max_depth: i64,
    #[serde(default = "default_resolve_guids")]
    resolve_guids: bool,
}

fn default_input_encoding() -> String {
    "hex".to_string()
}
fn default_mode() -> String {
    "tree".to_string()
}
fn default_bag_root() -> String {
    "auto".to_string()
}
fn default_max_entries() -> i64 {
    200
}
fn default_max_depth() -> i64 {
    32
}
fn default_resolve_guids() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The raw Windows registry hive bytes encoded as hex (default) or Base64. Paste UsrClass.dat for Windows Vista and later, or NTUSER.DAT for Windows XP; the hive is parsed locally and never uploaded."),
        )
        .param(
            Param::enumv("input_encoding", ["hex", "base64"])
                .default("hex")
                .describe("How the hive bytes in data are encoded. 'hex' accepts contiguous or whitespace/colon/dash-separated bytes with an optional leading 0x. 'base64' accepts standard Base64 with optional whitespace and padding."),
        )
        .param(
            Param::enumv("mode", ["tree", "list", "csv", "bodyfile", "raw"])
                .default("tree")
                .describe("Output format. 'tree' indents the reconstructed folder hierarchy in MRU order; 'list' prints one absolute path per line with slot, MRU position and timestamps; 'csv' emits a spreadsheet-ready table with a header row; 'bodyfile' emits Sleuth Kit bodyfile lines for mactime; 'raw' dumps each shell item's class byte, decoded fields and a hex preview for diagnostics."),
        )
        .param(
            Param::enumv("bag_root", ["auto", "usrclass", "ntuser", "shellnoroam"])
                .default("auto")
                .describe("Which BagMRU root to walk. 'auto' (default) tries every known location and reports the ones present. 'usrclass' is Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU in UsrClass.dat; 'ntuser' is Software\\Microsoft\\Windows\\Shell\\BagMRU in NTUSER.DAT; 'shellnoroam' is the Windows XP ShellNoRoam tree. Ignored when custom_path is set."),
        )
        .param(
            Param::string("custom_path")
                .default("")
                .describe("Optional BagMRU key path relative to the hive root, which overrides bag_root when non-empty. Use it to start from a subtree, for example Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU\\0. Do not include an HKCU or HKLM prefix."),
        )
        .param(
            Param::integer("max_entries")
                .default(200)
                .min(1.0)
                .max(5000.0)
                .describe("Maximum shellbag entries emitted per root before the walk stops. Use a small number such as 50 for chat-friendly output; values above 5000 are clamped. Truncation is always reported, never silent. Default 200."),
        )
        .param(
            Param::integer("max_depth")
                .default(32)
                .min(1.0)
                .max(64.0)
                .describe("Maximum folder depth to descend in the BagMRU tree. Real shellbag trees rarely exceed 20 levels; values above 64 are clamped. Hitting the cap is reported in the output. Default 32."),
        )
        .param(
            Param::boolean("resolve_guids")
                .default(true)
                .describe("When true (default), well-known shell-namespace GUIDs are shown as friendly names such as 'This PC', 'Desktop' or 'Recycle Bin'. Set false to print every root folder as a raw {guid} instead, which is what you want when cross-checking against another parser."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/shellbags-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reconstruct browsed folder paths from Windows shellbags in a registry hive.",
    skill(
        description = "Extract shellbag entries from an offline Windows registry hive (UsrClass.dat, or NTUSER.DAT on Windows XP) supplied as hex or Base64, and reconstruct the folders a user browsed in Explorer — including folders that were later deleted and paths on removable or network media. Walks the BagMRU tree, decodes each shell item (root/GUID folders, volumes, file entries, network locations, control-panel and delegate items) and reports the reconstructed absolute path, MRU position, NodeSlot bag number, the shell item's DOS/FAT created/modified/accessed timestamps, the NTFS MFT reference from a 0xBEEF0004 extension block, and the registry key's last-write time. Output as a tree, a flat path list, CSV, a Sleuth Kit bodyfile, or a raw per-item diagnostic dump. Runs locally; no upload.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "shellbags-parser", |a: Args| {
            gizza_ai_shellbags_parser_core::run(
                &a.data,
                &a.input_encoding,
                &a.mode,
                &a.bag_root,
                &a.custom_path,
                a.max_entries.max(0) as usize,
                a.max_depth.max(0) as usize,
                a.resolve_guids,
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
                    "data": { "type": "string", "description": "The raw Windows registry hive bytes encoded as hex (default) or Base64. Paste UsrClass.dat for Windows Vista and later, or NTUSER.DAT for Windows XP; the hive is parsed locally and never uploaded." },
                    "input_encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How the hive bytes in data are encoded. 'hex' accepts contiguous or whitespace/colon/dash-separated bytes with an optional leading 0x. 'base64' accepts standard Base64 with optional whitespace and padding." },
                    "mode": { "type": "string", "enum": ["tree", "list", "csv", "bodyfile", "raw"], "default": "tree", "description": "Output format. 'tree' indents the reconstructed folder hierarchy in MRU order; 'list' prints one absolute path per line with slot, MRU position and timestamps; 'csv' emits a spreadsheet-ready table with a header row; 'bodyfile' emits Sleuth Kit bodyfile lines for mactime; 'raw' dumps each shell item's class byte, decoded fields and a hex preview for diagnostics." },
                    "bag_root": { "type": "string", "enum": ["auto", "usrclass", "ntuser", "shellnoroam"], "default": "auto", "description": "Which BagMRU root to walk. 'auto' (default) tries every known location and reports the ones present. 'usrclass' is Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU in UsrClass.dat; 'ntuser' is Software\\Microsoft\\Windows\\Shell\\BagMRU in NTUSER.DAT; 'shellnoroam' is the Windows XP ShellNoRoam tree. Ignored when custom_path is set." },
                    "custom_path": { "type": "string", "default": "", "description": "Optional BagMRU key path relative to the hive root, which overrides bag_root when non-empty. Use it to start from a subtree, for example Local Settings\\Software\\Microsoft\\Windows\\Shell\\BagMRU\\0. Do not include an HKCU or HKLM prefix." },
                    "max_entries": { "type": "integer", "minimum": 1, "maximum": 5000, "default": 200, "description": "Maximum shellbag entries emitted per root before the walk stops. Use a small number such as 50 for chat-friendly output; values above 5000 are clamped. Truncation is always reported, never silent. Default 200." },
                    "max_depth": { "type": "integer", "minimum": 1, "maximum": 64, "default": 32, "description": "Maximum folder depth to descend in the BagMRU tree. Real shellbag trees rarely exceed 20 levels; values above 64 are clamped. Hitting the cap is reported in the output. Default 32." },
                    "resolve_guids": { "type": "boolean", "default": true, "description": "When true (default), well-known shell-namespace GUIDs are shown as friendly names such as 'This PC', 'Desktop' or 'Recycle Bin'. Set false to print every root folder as a raw {guid} instead, which is what you want when cross-checking against another parser." }
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
