//! Browser-facing wasm-bindgen wrapper for /tools/key-derive/.
//! Field order MUST match meta.toml: algorithm, secret, input_encoding, salt,
//! salt_encoding, length, encoding, hash, iterations, n, r, p, memory_kib,
//! time_cost, parallelism, argon2_variant, info, info_encoding.
use gizza_ai_key_derive_core::{derive, DeriveParams};
use wasm_bindgen::prelude::*;

fn num(s: &str, dflt: u32) -> u32 {
    let t = s.trim();
    if t.is_empty() {
        dflt
    } else {
        t.parse().unwrap_or(dflt)
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    algorithm: &str,
    secret: &str,
    input_encoding: &str,
    salt: &str,
    salt_encoding: &str,
    length: &str,
    encoding: &str,
    hash: &str,
    iterations: &str,
    n: &str,
    r: &str,
    p: &str,
    memory_kib: &str,
    time_cost: &str,
    parallelism: &str,
    argon2_variant: &str,
    info: &str,
    info_encoding: &str,
) -> Result<String, JsValue> {
    let alg = if algorithm.trim().is_empty() { "pbkdf2" } else { algorithm };
    let in_enc = if input_encoding.trim().is_empty() { "utf8" } else { input_encoding };
    let salt_enc = if salt_encoding.trim().is_empty() { "utf8" } else { salt_encoding };
    let out_enc = if encoding.trim().is_empty() { "hex" } else { encoding };
    let len: usize = {
        let t = length.trim();
        if t.is_empty() { 32 } else { t.parse().unwrap_or(32) }
    };
    let params = DeriveParams {
        hash: if hash.trim().is_empty() { "sha256".into() } else { hash.into() },
        iterations: num(iterations, 100_000),
        n: num(n, 16384),
        r: num(r, 8),
        p: num(p, 1),
        memory_kib: num(memory_kib, 19456),
        time_cost: num(time_cost, 2),
        parallelism: num(parallelism, 1),
        argon2_variant: if argon2_variant.trim().is_empty() {
            "argon2id".into()
        } else {
            argon2_variant.into()
        },
        info: info.to_string(),
        info_encoding: if info_encoding.trim().is_empty() {
            "utf8".into()
        } else {
            info_encoding.into()
        },
    };
    derive(alg, secret, in_enc, salt, salt_enc, len, out_enc, params)
        .map_err(|e| JsValue::from_str(&e))
}
