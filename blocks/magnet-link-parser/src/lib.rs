//! gizza-ai/magnet-link-parser — parse a BitTorrent `magnet:` URI into its
//! structured parts (info-hash, display name, trackers, web seeds, …) and build
//! a magnet link back from those parts. Chat schema single-sourced from
//! descriptor() (which also drives the CLI); handler delegates to run_skill.
//! Pure → all backends (no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    mode: String,
    /// parse mode: the magnet link to decode.
    #[serde(default)]
    magnet: String,
    /// build mode: the v1 info-hash (required to build).
    #[serde(default)]
    info_hash: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    trackers: String,
    #[serde(default)]
    web_seeds: String,
    #[serde(default)]
    exact_length: Option<u64>,
}

/// Single source for the chat schema (and CLI). `parse` reads `magnet`; `build`
/// reads `info_hash` + the optional fields.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("mode", ["parse", "build"])
                .default("parse")
                .describe("Direction: 'parse' (default) decodes a magnet: link into its parts; 'build' assembles a magnet: link from an info-hash and optional fields."),
        )
        .param(
            Param::string("magnet")
                .describe("Parse mode: the magnet: URI to decode, e.g. 'magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=...'. A bare query string without the 'magnet:?' prefix is also accepted."),
        )
        .param(
            Param::string("info_hash")
                .describe("Build mode: the BitTorrent v1 info-hash — 40 hex characters, 32 base32 characters, or a full 'urn:btih:<hash>'. Required to build a link."),
        )
        .param(
            Param::string("display_name")
                .describe("Build mode: the display name (dn) for the file or torrent. Optional."),
        )
        .param(
            Param::string("trackers")
                .describe("Build mode: tracker announce URLs (tr), separated by newlines or commas. Optional."),
        )
        .param(
            Param::string("web_seeds")
                .describe("Build mode: web-seed URLs (ws, BEP 19), separated by newlines or commas. Optional."),
        )
        .param(
            Param::integer("exact_length")
                .describe("Build mode: the exact total length in bytes (xl). Optional."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/magnet-link-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse and build BitTorrent magnet links",
    skill(
        description = "Parse a BitTorrent magnet: link into its structured parts, or build a magnet: link from parts. In parse mode (default) set magnet='magnet:?xt=urn:btih:...' and get back JSON with the v1 info-hash (normalised to lower-case hex, from 40-hex or 32-base32), v2 info-hash (urn:btmh), display name (dn), trackers (tr), web seeds (ws), acceptable/exact sources (as/xs), keywords (kt), exact length (xl) and any other parameters; percent- and '+'-encoding are decoded and indexed keys like tr.1/tr.2 are collapsed. In build mode set mode='build' and info_hash (40 hex chars, 32 base32 chars, or a urn:btih:… value) plus optional display_name, trackers and web_seeds (newline- or comma-separated), and exact_length (bytes), and get back the assembled magnet:?… URI. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "magnet-link-parser", |a: Args| {
            gizza_ai_magnet_link_parser_core::dispatch(
                &a.mode,
                &a.magnet,
                &a.info_hash,
                &a.display_name,
                &a.trackers,
                &a.web_seeds,
                a.exact_length,
                false,
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
                    "mode": { "type": "string", "enum": ["parse", "build"], "default": "parse", "description": "Direction: 'parse' (default) decodes a magnet: link into its parts; 'build' assembles a magnet: link from an info-hash and optional fields." },
                    "magnet": { "type": "string", "description": "Parse mode: the magnet: URI to decode, e.g. 'magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1367a88a&dn=...'. A bare query string without the 'magnet:?' prefix is also accepted." },
                    "info_hash": { "type": "string", "description": "Build mode: the BitTorrent v1 info-hash — 40 hex characters, 32 base32 characters, or a full 'urn:btih:<hash>'. Required to build a link." },
                    "display_name": { "type": "string", "description": "Build mode: the display name (dn) for the file or torrent. Optional." },
                    "trackers": { "type": "string", "description": "Build mode: tracker announce URLs (tr), separated by newlines or commas. Optional." },
                    "web_seeds": { "type": "string", "description": "Build mode: web-seed URLs (ws, BEP 19), separated by newlines or commas. Optional." },
                    "exact_length": { "type": "integer", "description": "Build mode: the exact total length in bytes (xl). Optional." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
