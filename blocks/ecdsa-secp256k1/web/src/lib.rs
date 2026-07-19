//! Browser-facing wasm-bindgen wrapper for /tools/ecdsa-secp256k1/.
//! Field order MUST match meta.toml: operation, message, message_encoding,
//! hash, key, signature.
use gizza_ai_ecdsa_secp256k1_core::{process, HashAlg, MsgEncoding, Operation};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    operation: &str,
    message: &str,
    message_encoding: &str,
    hash: &str,
    key: &str,
    signature: &str,
) -> Result<String, JsValue> {
    let op = Operation::parse(operation).map_err(|e| JsValue::from_str(&e))?;
    let enc = MsgEncoding::parse(message_encoding).map_err(|e| JsValue::from_str(&e))?;
    let alg = HashAlg::parse(hash).map_err(|e| JsValue::from_str(&e))?;
    let out = process(op, message, enc, alg, key, signature).map_err(|e| JsValue::from_str(&e))?;
    let text = match op {
        Operation::Generate => format!(
            "operation: generate\n\nprivate key (hex): {}\npublic key (compressed hex): {}\npublic key (uncompressed hex): {}\n\nprivate key (PKCS#8 PEM):\n{}\npublic key (SPKI PEM):\n{}",
            out.private_key_hex.unwrap_or_default(),
            out.public_key_compressed_hex.unwrap_or_default(),
            out.public_key_uncompressed_hex.unwrap_or_default(),
            out.private_key_pem.unwrap_or_default(),
            out.public_key_pem.unwrap_or_default(),
        ),
        Operation::Sign => format!(
            "operation: sign\nhash: {}\ndigest (hex): {}\n\nsignature (compact hex): {}\nsignature (compact base64): {}\nsignature (DER hex): {}\nr: {}\ns: {}\nrecovery id: {} (v = {})\n\npublic key (compressed hex): {}\npublic key (uncompressed hex): {}",
            out.hash.unwrap_or_default(),
            out.digest_hex.unwrap_or_default(),
            out.signature_compact_hex.unwrap_or_default(),
            out.signature_compact_base64.unwrap_or_default(),
            out.signature_der_hex.unwrap_or_default(),
            out.r_hex.unwrap_or_default(),
            out.s_hex.unwrap_or_default(),
            out.recovery_id.map(|b| b.to_string()).unwrap_or_default(),
            out.v.map(|b| b.to_string()).unwrap_or_default(),
            out.public_key_compressed_hex.unwrap_or_default(),
            out.public_key_uncompressed_hex.unwrap_or_default(),
        ),
        Operation::Verify => {
            let valid = out.valid.unwrap_or(false);
            format!(
                "operation: verify\nhash: {}\ndigest (hex): {}\nsignature form: {}{}\nvalid: {}\n{}",
                out.hash.unwrap_or_default(),
                out.digest_hex.unwrap_or_default(),
                out.signature_form.unwrap_or_default(),
                if out.normalized_s == Some(true) { " (high-S, normalized before checking)" } else { "" },
                valid,
                if valid {
                    "✓ signature is valid for this message and public key"
                } else {
                    "✗ signature does NOT match this message and public key"
                }
            )
        }
    };
    Ok(text)
}
