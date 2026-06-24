//! Browser-facing wasm-bindgen wrapper for /tools/otpauth-uri/.
//! Field order MUST match meta.toml: type, issuer, account, secret, digits, period,
//! algorithm, counter.
use gizza_ai_otpauth_uri_core::{build, Algo, OtpAuth, OtpType};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    r#type: &str,
    issuer: &str,
    account: &str,
    secret: &str,
    digits: &str,
    period: &str,
    algorithm: &str,
    counter: &str,
) -> Result<String, JsValue> {
    let otp_type = OtpType::parse(r#type).map_err(|e| JsValue::from_str(&e))?;
    let algo = Algo::parse(algorithm).map_err(|e| JsValue::from_str(&e))?;
    let digits: u32 = digits.trim().parse().unwrap_or(6);
    let period: u64 = period.trim().parse().unwrap_or(30);
    let counter: u64 = counter.trim().parse().unwrap_or(0);
    build(&OtpAuth {
        otp_type,
        issuer: issuer.to_string(),
        account: account.to_string(),
        secret: secret.to_string(),
        algorithm: algo,
        digits,
        period,
        counter,
    })
    .map_err(|e| JsValue::from_str(&e))
}
