//! gizza-ai/mac-vendor-lookup — resolve the manufacturer of a MAC address from
//! its OUI prefix (chat skill block).
//!
//! Thin chat-skill wrapper around `gizza-ai-mac-vendor-lookup-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. Fully offline — the
//! IEEE OUI registry is bundled into the wasm; no host/network calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    mac: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("mac")
            .required()
            .describe("The MAC / EUI address (or just its first 3 octets, the OUI) to look up. Colon, hyphen, dot (Cisco), or separator-less hex are all accepted, case-insensitive — e.g. '28:6F:B9:01:23:45', '286F-B901-2345', '286fb9012345', or just '28:6F:B9'. Pass several addresses one per line for a batch lookup (one result line each)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MacVendorLookup;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mac-vendor-lookup",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Resolve the manufacturer of a MAC address from its OUI prefix.",
    skill(
        description = "Resolve the manufacturer (vendor) of a network MAC / EUI address from its OUI (Organizationally Unique Identifier) — the first 3 octets — using a bundled IEEE registry. Fully offline: no DNS, no network, no sign-up. Accepts colon, hyphen, dot (Cisco 286f.b900.1a2b), or separator-less hex, case-insensitive, and either a full address or just the 3-octet OUI. For a single address it returns the normalized MAC, the OUI prefix, the registered organization name (or a note that the OUI is unassigned), and whether the address is globally unique vs locally administered and unicast vs multicast (decoded from the first octet's U/L and I/G bits). Pass several addresses one per line to batch-resolve them (one 'MAC — Vendor' line each).",
        parameters = schema_json()
    ),
)]
impl MacVendorLookup {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "mac-vendor-lookup", |a: Args| {
            Ok::<String, SkillError>(gizza_ai_mac_vendor_lookup_core::report(&a.mac))
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
                    "mac": { "type": "string", "description": "The MAC / EUI address (or just its first 3 octets, the OUI) to look up. Colon, hyphen, dot (Cisco), or separator-less hex are all accepted, case-insensitive — e.g. '28:6F:B9:01:23:45', '286F-B901-2345', '286fb9012345', or just '28:6F:B9'. Pass several addresses one per line for a batch lookup (one result line each)." }
                },
                "required": ["mac"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
