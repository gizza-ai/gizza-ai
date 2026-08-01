//! Browser-facing wasm-bindgen wrapper for /tools/wireguard-config-builder/.
//! Arg order MUST match page/meta.toml [[input]] order: private_key, address,
//! listen_port, dns, mtu, peer_public_key, preshared_key, allowed_ips, endpoint,
//! persistent_keepalive, format. All fields arrive as strings; the three numeric
//! fields (blank => omitted) are parsed here, then core `build` validates + renders.
use gizza_ai_wireguard_config_builder_core::{build, WgInput};
use wasm_bindgen::prelude::*;

/// Parse an optional non-negative integer field: blank => None, otherwise a u32.
fn opt_u32(raw: &str, label: &str) -> Result<Option<u32>, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    t.parse::<u32>()
        .map(Some)
        .map_err(|_| format!("{label} must be a whole number (got {t:?})"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    private_key: &str,
    address: &str,
    listen_port: &str,
    dns: &str,
    mtu: &str,
    peer_public_key: &str,
    preshared_key: &str,
    allowed_ips: &str,
    endpoint: &str,
    persistent_keepalive: &str,
    format: &str,
) -> Result<String, JsValue> {
    let err = |e: String| JsValue::from_str(&e);
    let input = WgInput {
        private_key: private_key.to_string(),
        address: address.to_string(),
        listen_port: opt_u32(listen_port, "ListenPort").map_err(err)?,
        dns: dns.to_string(),
        mtu: opt_u32(mtu, "MTU").map_err(err)?,
        peer_public_key: peer_public_key.to_string(),
        preshared_key: preshared_key.to_string(),
        allowed_ips: allowed_ips.to_string(),
        endpoint: endpoint.to_string(),
        persistent_keepalive: opt_u32(persistent_keepalive, "PersistentKeepalive").map_err(err)?,
    };
    let fmt = if format.trim().is_empty() { "conf" } else { format };
    build(&input, fmt).map_err(err)
}
