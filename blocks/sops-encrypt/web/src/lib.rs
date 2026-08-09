//! Browser-facing wasm-bindgen wrapper for /tools/sops-encrypt/.
//! The argument order mirrors `page/meta.toml`'s `[[input]]` order.
use wasm_bindgen::prelude::*;

use gizza_ai_sops_encrypt_core as core;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    document: &str,
    passphrase: &str,
    mode: &str,
    format: &str,
    encrypted_suffix: &str,
    unencrypted_suffix: &str,
    encrypted_regex: &str,
    unencrypted_regex: &str,
) -> Result<String, JsValue> {
    rewrite(
        document,
        passphrase,
        mode,
        format,
        encrypted_suffix,
        unencrypted_suffix,
        encrypted_regex,
        unencrypted_regex,
    )
    .map_err(|e| JsValue::from_str(&e))
}

#[allow(clippy::too_many_arguments)]
fn rewrite(
    document: &str,
    passphrase: &str,
    mode: &str,
    format: &str,
    encrypted_suffix: &str,
    unencrypted_suffix: &str,
    encrypted_regex: &str,
    unencrypted_regex: &str,
) -> Result<String, String> {
    let opts = core::Options {
        format: Some(core::Format::parse(format)?),
        encrypted_suffix: encrypted_suffix.trim().to_string(),
        unencrypted_suffix: unencrypted_suffix.trim().to_string(),
        encrypted_regex: encrypted_regex.trim().to_string(),
        unencrypted_regex: unencrypted_regex.trim().to_string(),
    };
    let outcome = match mode.trim().to_ascii_lowercase().as_str() {
        "" | "encrypt" => core::encrypt(document, passphrase, &opts)?,
        "decrypt" => core::decrypt(document, passphrase, &opts)?,
        other => return Err(format!("unknown mode '{other}' (use 'encrypt' or 'decrypt')")),
    };
    Ok(outcome.document)
}
