//! gizza-ai/cert-chain-validate — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    chain_pem: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("chain_pem")
            .required()
            .describe("Leaf-to-root PEM certificate chain. Paste one or more -----BEGIN CERTIFICATE----- blocks in order: leaf, intermediates, then optional self-signed root."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cert-chain-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate a PEM certificate chain offline.",
    skill(
        description = "Checks a pasted PEM certificate chain offline for leaf-to-root ordering, issuer/subject linkage, certificate signatures, CA flags and current validity windows. It does not consult browser or OS trust stores.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cert-chain-validate", |a: Args| {
            gizza_ai_cert_chain_validate_core::run(&a.chain_pem).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
