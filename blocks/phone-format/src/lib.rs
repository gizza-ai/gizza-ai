//! gizza-ai/phone-format — chat skill block (thin wrapper around core).
//!
//! Parses, validates, and formats an international phone number. Takes
//! `{ "number": "+1 415 555 2671", "region": "US"? }` and returns
//! `{ "result": "Valid: yes\nE.164: …\n…" }` or `{ "error": "..." }`.
//! No host calls — runs entirely inside the WASM sandbox (the `phonenumber`
//! crate bundles its own metadata).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    number: String,
    #[serde(default)]
    region: String,
}

#[cfg(target_arch = "wasm32")]
struct PhoneFormat;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/phone-format",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Phone number formatter & validator skill",
    skill(
        description = "Parse, validate, and format an international phone number. Reports validity, E.164, national & international formats, the country/region, and the number type when derivable.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "number": { "type": "string", "description": "The phone number to analyze. May include a +country prefix, e.g. +1 415 555 2671." },
                "region": { "type": "string", "description": "Optional ISO-3166 alpha-2 region code (e.g. US, GB, DE) used to interpret a number given without a + prefix." }
            },
            "required": ["number"],
            "additionalProperties": false
        }"#
    )
)]
impl PhoneFormat {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return respond_error(format!("invalid args: {e}")),
        };
        match gizza_ai_phone_format_core::format_number(&args.number, &args.region) {
            Ok(v) => GuestResult::respond(
                serde_json::to_vec(&serde_json::json!({ "result": v })).unwrap_or_default(),
            ),
            Err(e) => respond_error(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn respond_error(msg: String) -> GuestResult {
    GuestResult::respond(
        serde_json::to_vec(&serde_json::json!({ "error": msg })).unwrap_or_default(),
    )
}
