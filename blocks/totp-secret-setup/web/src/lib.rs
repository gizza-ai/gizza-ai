//! Browser-facing wasm-bindgen wrapper for /tools/totp-secret-setup/.
//! Field order MUST match page/meta.toml: issuer, account, secret_bytes, digits,
//! period, algorithm.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    issuer: &str,
    account: &str,
    secret_bytes: &str,
    digits: &str,
    period: &str,
    algorithm: &str,
) -> Result<String, JsValue> {
    let secret_bytes = parse_or_default(secret_bytes, 20)?;
    let digits = parse_or_default(digits, 6)?;
    let period = parse_or_default(period, 30)?;
    let algorithm = if algorithm.trim().is_empty() { "sha1" } else { algorithm };
    let setup = gizza_ai_totp_secret_setup_core::generate(
        issuer,
        account,
        secret_bytes as usize,
        digits,
        period as u64,
        algorithm,
    )
    .map_err(|e| JsValue::from_str(&e))?;
    Ok(format!(
        "Secret: {secret}\nBits: {bits}\nAlgorithm: {algorithm}\nDigits: {digits}\nPeriod: {period} seconds\nURI: {uri}",
        secret = setup.secret,
        bits = setup.bits,
        algorithm = setup.algorithm,
        digits = setup.digits,
        period = setup.period,
        uri = setup.uri,
    ))
}

fn parse_or_default(raw: &str, default: u32) -> Result<u32, JsValue> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(default);
    }
    s.parse::<u32>()
        .map_err(|_| JsValue::from_str(&format!("expected a number, got {raw:?}")))
}
