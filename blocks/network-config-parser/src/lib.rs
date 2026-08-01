//! gizza-ai/network-config-parser — chat skill block on the shared tool abstraction.
//!
//! Parses a hierarchical network device configuration (Cisco IOS / IOS-XE /
//! NX-OS, Arista EOS, Juniper Junos) into a foldable, searchable tree of
//! sections. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI + page); `handle()` delegates to `block_utils::run_skill`. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    config: String,
    #[serde(default)]
    syntax: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    filter: String,
    #[serde(default)]
    comments: String,
}

/// Single source for the chat schema (and CLI + page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("config")
                .required()
                .describe("The device configuration text to parse. Indentation style (Cisco IOS / IOS-XE / NX-OS, Arista EOS) uses leading whitespace for nesting and '!' as a section separator; brace style (Juniper Junos) uses '{ … }' blocks with statements ending in ';'."),
        )
        .param(
            Param::enumv("syntax", ["auto", "indent", "brace"])
                .default("auto")
                .describe("Config syntax. 'auto' (default) detects the style from the text; 'indent' forces Cisco/NX-OS/Arista indentation parsing; 'brace' forces Juniper Junos curly-brace parsing."),
        )
        .param(
            Param::enumv("output", ["tree", "paths", "report"])
                .default("tree")
                .describe("Output shape. 'tree' (default) is a nested JSON array of { line, children } nodes; 'paths' is a flat list of full command paths (one per leaf, joined by ' / '); 'report' summarises the config with a top-level section list and stats (sections, lines, depth, comments)."),
        )
        .param(
            Param::string("filter")
                .default("")
                .describe("Optional case-insensitive substring. When set, only sections/lines containing it are kept: matching a section header keeps its whole block; matching a deep value keeps its ancestor chain for context. Empty (default) returns the full tree."),
        )
        .param(
            Param::enumv("comments", ["strip", "keep"])
                .default("strip")
                .describe("How to treat comment/separator lines ('!' or '#' in indent style; '#' and '/* … */' in brace style). 'strip' (default) drops them; 'keep' includes them as nodes. Bare '!' separators are always dropped."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NetworkConfigParser;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/network-config-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a hierarchical network device config (Cisco, Juniper, Arista) into a searchable tree.",
    skill(
        description = "Parse a hierarchical network device configuration into a foldable, searchable tree of sections. Handles indentation style (Cisco IOS / IOS-XE / NX-OS, Arista EOS — whitespace nesting, '!' separators) and brace style (Juniper Junos — '{ … }' blocks, ';' statements); syntax='auto' (default) detects which. output='tree' (default) is a nested JSON array of { line, children }, 'paths' is a flat list of full command paths, 'report' gives a section list plus stats. Set filter to a case-insensitive substring to keep only matching sections/lines with their context, and comments to strip (default) or keep comment lines.",
        parameters = schema_json()
    ),
)]
impl NetworkConfigParser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned string in { "result": … }.
        match run_skill(&body, "network-config-parser", |a: Args| {
            gizza_ai_network_config_parser_core::parse(
                &a.config,
                &a.syntax,
                &a.output,
                &a.filter,
                &a.comments,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "config": { "type": "string", "description": "The device configuration text to parse. Indentation style (Cisco IOS / IOS-XE / NX-OS, Arista EOS) uses leading whitespace for nesting and '!' as a section separator; brace style (Juniper Junos) uses '{ … }' blocks with statements ending in ';'." },
                    "syntax": { "type": "string", "enum": ["auto", "indent", "brace"], "default": "auto", "description": "Config syntax. 'auto' (default) detects the style from the text; 'indent' forces Cisco/NX-OS/Arista indentation parsing; 'brace' forces Juniper Junos curly-brace parsing." },
                    "output": { "type": "string", "enum": ["tree", "paths", "report"], "default": "tree", "description": "Output shape. 'tree' (default) is a nested JSON array of { line, children } nodes; 'paths' is a flat list of full command paths (one per leaf, joined by ' / '); 'report' summarises the config with a top-level section list and stats (sections, lines, depth, comments)." },
                    "filter": { "type": "string", "default": "", "description": "Optional case-insensitive substring. When set, only sections/lines containing it are kept: matching a section header keeps its whole block; matching a deep value keeps its ancestor chain for context. Empty (default) returns the full tree." },
                    "comments": { "type": "string", "enum": ["strip", "keep"], "default": "strip", "description": "How to treat comment/separator lines ('!' or '#' in indent style; '#' and '/* … */' in brace style). 'strip' (default) drops them; 'keep' includes them as nodes. Bare '!' separators are always dropped." }
                },
                "required": ["config"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
