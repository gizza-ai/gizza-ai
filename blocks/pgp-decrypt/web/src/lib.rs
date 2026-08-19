//! Browser-facing wasm-bindgen wrapper for /tools/pgp-decrypt/.
//! Argument order MUST match meta.toml: message, private_key, passphrase,
//! public_key, output_format.
use gizza_ai_pgp_decrypt_core::{run as decrypt, OutputFormat};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    message: &str,
    private_key: &str,
    passphrase: &str,
    public_key: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    let fmt = OutputFormat::parse(output_format).map_err(|e| JsValue::from_str(&e))?;
    let res = decrypt(message, private_key, passphrase, public_key, fmt)
        .map_err(|e| JsValue::from_str(&e))?;
    serde_json::to_string_pretty(&res.to_json()).map_err(|e| JsValue::from_str(&e.to_string()))
}
