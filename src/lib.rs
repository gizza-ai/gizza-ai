//! gizza-ai — browser-local AI chat site.
//!
//! Compiles to wasm32 via wasm-bindgen; loaded by a Service Worker that
//! forwards requests through the WAFER runtime. Agent loop + UI + skill
//! blocks are wired up in subsequent Plan B tasks.

use wasm_bindgen::prelude::*;

pub mod blocks;
pub mod skills;

/// JS bridge — only meaningful on wasm32; the extern "C" block won't
/// compile on native targets.
#[cfg(target_arch = "wasm32")]
pub mod bridge;

#[wasm_bindgen]
pub async fn initialize() -> Result<(), JsValue> {
    web_sys::console::log_1(&"gizza-ai: initialize() called (Plan B Task 1 stub)".into());
    Ok(())
}

#[wasm_bindgen]
pub async fn handle_request(request: web_sys::Request) -> Result<web_sys::Response, JsValue> {
    let _ = request;
    Err(JsValue::from_str("gizza-ai: handle_request not yet implemented"))
}
