//! gizza-ai — browser-local AI chat site.
//!
//! Compiles to wasm32 via wasm-bindgen; loaded by a Service Worker that
//! forwards requests through the WAFER runtime. `initialize()` builds the
//! runtime via `SolobaseBuilder` using browser platform services from
//! `solobase-browser`, registers gizza's curated feature blocks plus the
//! native agent/ui blocks and every embedded skill WASM, and wires `/`,
//! `/b/ui/`, `/b/ui`, and `/b/agent/` to gizza blocks as Public tier.
//! `handle_request()` dispatches through the `site-main` flow.

use std::sync::Arc;

use solobase::builder::{self, SolobaseBuilder};
use solobase_core::RouteAccess;
use wafer_core::interfaces::config::service::ConfigService;
use wasm_bindgen::prelude::*;

pub mod blocks;
pub mod config;
pub mod skills;

// ---------------------------------------------------------------------------
// module_start()
// ---------------------------------------------------------------------------

/// Module init — runs automatically before any other wasm-bindgen export is
/// first called. Install the panic hook here so ANY panic (including ones in
/// code paths that don't go through initialize()) surfaces in the console.
#[wasm_bindgen(start)]
pub fn module_start() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"gizza-ai: panic hook installed".into());
}

// ---------------------------------------------------------------------------
// initialize()
// ---------------------------------------------------------------------------

/// Initialize the gizza-ai WAFER runtime.
///
/// Must be called exactly once when the Service Worker starts, before any
/// `handle_request()` call. Async because it awaits `solobase_browser::db_init()`
/// and `wafer.start_without_bind()`.
#[wasm_bindgen]
pub async fn initialize() -> Result<(), JsValue> {
    // Guard against double initialization.
    if solobase_browser::runtime::is_initialized() {
        return Ok(());
    }

    // 1. Load sql.js WASM + open/create the OPFS database.
    solobase_browser::db_init().await;

    // 2. Seed variables and load config.
    let vars = config::seed_and_load_variables();
    web_sys::console::log_1(
        &format!("gizza-ai: {} variables loaded from database", vars.len()).into(),
    );

    // 3. Load feature flag settings (curated for gizza — only auth/llm/
    //    local-llm/messages are enabled).
    let features = config::load_block_settings();

    // 4. Extract JWT secret.
    let jwt_secret = vars
        .get("SUPPERS_AI__AUTH__JWT_SECRET")
        .cloned()
        .unwrap_or_default();

    // 5. Build config service.
    let config_svc = wafer_block_config::service::EnvConfigService::new();
    for (key, value) in &vars {
        config_svc.set(key, value);
    }

    // 5b. Browser WebLLM service — provided by solobase-browser in Phase D.
    //     Registered on the MultiBackendLlmService router under the label
    //     "browser"; BrowserLlmService::claims_backend matches the
    //     `"webllm"` backend_id that ChatRequest will carry.
    let browser_llm: Arc<dyn wafer_core::interfaces::llm::service::LlmService> =
        Arc::new(solobase_browser::llm::BrowserLlmService::new());

    // 6. Build WAFER runtime via SolobaseBuilder. We reuse solobase's
    //    builder for the service blocks, middleware, router, and
    //    site-main flow, and inject gizza-specific routes via
    //    `add_route` so /, /b/ui/, and /b/agent/ reach gizza's native
    //    blocks as Public tier (no auth required — gizza runs anonymous).
    let (mut wafer, storage_block) = SolobaseBuilder::new()
        .database(solobase_browser::make_database_service())
        .storage(solobase_browser::make_storage_service())
        .config(Arc::new(config_svc))
        .crypto(solobase_browser::make_crypto_service(jwt_secret))
        .network(solobase_browser::make_network_service())
        .logger(solobase_browser::make_console_logger())
        .llm_service("browser", browser_llm)
        .block_settings(features)
        .block_config(
            "wafer-run/security-headers",
            serde_json::json!({
                "csp": concat!(
                    "default-src 'self'; ",
                    "script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' https://cdn.jsdelivr.net; ",
                    "style-src 'self' 'unsafe-inline'; ",
                    "img-src 'self' data: blob: https:; ",
                    "font-src 'self' https:; ",
                    "connect-src 'self' https://cdn.jsdelivr.net https://esm.run https://huggingface.co ",
                        "https://raw.githubusercontent.com https://*.huggingface.co https://*.hf.co https://*.xethub.hf.co; ",
                    "frame-ancestors 'none'; ",
                    "base-uri 'self'; ",
                    "form-action 'self'"
                )
            }),
        )
        // Placeholder config for the auth block. gizza-ai runs anonymously;
        // OAuth sign-in is never exercised. The required-config validator
        // rejects empty strings though, so feed non-empty placeholders.
        // Plan C tightens this (make the auth block accept empty OAuth
        // fields as "provider disabled").
        .block_config(
            "suppers-ai/auth",
            serde_json::json!({
                "SUPPERS_AI__AUTH__JWT_SECRET": "gizza-mvp-dev-jwt-secret-not-for-production",
                "SUPPERS_AI__AUTH__ALLOWED_EMAIL_DOMAINS": "*",
                "SUPPERS_AI__AUTH__ADMIN_EMAIL": "admin@gizza.local",
                "SUPPERS_AI__AUTH__ADMIN_PASSWORD": "admin",
                "SUPPERS_AI__AUTH__INTERNAL_SECRET": "gizza-mvp-dev-internal-secret",
                "SUPPERS_AI__AUTH__OAUTH_REDIRECT_URI": "http://localhost:8000/b/auth/oauth/callback",
                "SUPPERS_AI__AUTH__OAUTH_GOOGLE_CLIENT_ID": "disabled",
                "SUPPERS_AI__AUTH__OAUTH_GOOGLE_CLIENT_SECRET": "disabled",
                "SUPPERS_AI__AUTH__OAUTH_GITHUB_CLIENT_ID": "disabled",
                "SUPPERS_AI__AUTH__OAUTH_GITHUB_CLIENT_SECRET": "disabled",
                "SUPPERS_AI__AUTH__OAUTH_MICROSOFT_CLIENT_ID": "disabled",
                "SUPPERS_AI__AUTH__OAUTH_MICROSOFT_CLIENT_SECRET": "disabled",
            }),
        )
        .block_config(
            "suppers-ai/email",
            serde_json::json!({
                "SUPPERS_AI__EMAIL__MAILGUN_API_KEY": "disabled",
                "SUPPERS_AI__EMAIL__MAILGUN_DOMAIN": "disabled",
                "SUPPERS_AI__EMAIL__MAILGUN_FROM": "noreply@gizza.local",
                "SUPPERS_AI__EMAIL__MAILGUN_REPLY_TO": "noreply@gizza.local",
            }),
        )
        .block_config(
            "suppers-ai/llm",
            serde_json::json!({
                "SUPPERS_AI__LLM__DEFAULT_PROVIDER": "suppers-ai/local-llm",
                "SUPPERS_AI__LLM__DEFAULT_MODEL": "Qwen2.5-1.5B-Instruct-q4f32_1-MLC",
            }),
        )
        .block_config(
            "suppers-ai/provider-llm",
            serde_json::json!({
                "SUPPERS_AI__PROVIDER_LLM__OPENAI_KEY": "disabled",
                "SUPPERS_AI__PROVIDER_LLM__ANTHROPIC_KEY": "disabled",
            }),
        )
        .add_route("/", "gizza-ai/ui", RouteAccess::Public)
        .add_route("/b/ui", "gizza-ai/ui", RouteAccess::Public)
        .add_route("/b/ui/", "gizza-ai/ui", RouteAccess::Public)
        .add_route("/b/agent/", "gizza-ai/agent", RouteAccess::Public)
        .build()
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // 6a. Override wafer-run/router's default routes so `/` dispatches to
    // gizza-ai/ui (not wafer-run/web). SolobaseBuilder::add_route only
    // affects the inner suppers-ai/router (reached via /b/**); the OUTER
    // site-main flow dispatcher (wafer-run/router) has its own route
    // table set by flows::register_site_main. Without this override, `/`
    // falls through to wafer-run/web which tries OPFS path
    // "wafer-run/web/site" — illegal (contains /) — and panics.
    wafer.add_block_config(
        "wafer-run/router",
        serde_json::json!({
            "routes": [
                { "path": "/b/**",                   "block": "suppers-ai/router" },
                { "path": "/health",                 "block": "suppers-ai/router" },
                { "path": "/openapi.json",           "block": "suppers-ai/router" },
                { "path": "/.well-known/agent.json", "block": "suppers-ai/router" },
                { "path": "/",                       "block": "gizza-ai/ui" },
            ],
        }),
    );

    // 6a-bis. Override the site-main flow with inline step config for
    // wafer-run/security-headers. That block reads its `csp` from the
    // flow-step config, not from block_configs — the default CSP has
    // only `'self' 'unsafe-inline'` which blocks WebLLM's jsdelivr import.
    //
    // SolobaseBuilder's `.block_config("wafer-run/security-headers", ...)`
    // was silently ineffective for this reason. Plan C follow-up: make
    // security-headers also consult block_configs, or expose a clean
    // SolobaseBuilder::csp(...) helper.
    wafer.add_flow_json(r##"{
        "id": "site-main",
        "name": "Site Main (gizza-ai)",
        "version": "0.1.0",
        "description": "Top-level HTTP dispatch with gizza-ai CSP.",
        "steps": [
            { "id": "security-headers", "block": "wafer-run/security-headers", "config": {
                "csp": "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' https://cdn.jsdelivr.net; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; font-src 'self' https:; connect-src 'self' https://cdn.jsdelivr.net https://esm.run https://huggingface.co https://raw.githubusercontent.com https://*.huggingface.co https://*.hf.co https://*.xethub.hf.co; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
            }},
            { "id": "cors", "block": "wafer-run/cors" },
            { "id": "readonly-guard", "block": "wafer-run/readonly-guard" },
            { "id": "router", "block": "wafer-run/router" }
        ],
        "config": { "on_error": "stop" },
        "config_map": {
            "routes": { "target": "wafer-run/router", "key": "routes" }
        }
    }"##)
    .map_err(|e| JsValue::from_str(&format!("register gizza site-main: {e}")))?;

    // 6b. Register the SW-side external-asset loader before start so any
    // block init that triggers an asset load sees the real loader (not
    // the NoopAssetLoader default).
    wafer.set_asset_loader(solobase_browser::make_sw_asset_loader());

    // 6c. Register gizza-ai's native blocks (agent + ui) and every
    // embedded skill WASM from skills::SKILLS (produced by build.rs).
    wafer
        .register_block("gizza-ai/agent", Arc::new(blocks::agent::AgentBlock))
        .map_err(|e| JsValue::from_str(&format!("register gizza-ai/agent: {e}")))?;
    wafer
        .register_block("gizza-ai/ui", Arc::new(blocks::ui::UiBlock))
        .map_err(|e| JsValue::from_str(&format!("register gizza-ai/ui: {e}")))?;

    for (name, bytes) in skills::SKILLS {
        let wasmi = wafer_run::wasm::WasmiBlock::load_from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&format!("loading skill {name}: {e}")))?;
        wafer
            .register_block(*name, Arc::new(wasmi))
            .map_err(|e| JsValue::from_str(&format!("registering skill {name}: {e}")))?;
        web_sys::console::log_1(&format!("gizza-ai: skill '{name}' registered").into());
    }

    // 7. Start runtime.
    wafer
        .start_without_bind()
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // 8. Inject WRAP grants.
    builder::post_start(&wafer, &storage_block);

    web_sys::console::log_1(&"gizza-ai: WAFER runtime started".into());

    // 9. Store in framework's thread_local.
    solobase_browser::runtime::store_wafer(wafer);

    Ok(())
}

// ---------------------------------------------------------------------------
// handle_request()
// ---------------------------------------------------------------------------

/// Handle an incoming fetch request from the Service Worker.
///
/// Converts the browser `Request` into a WAFER `Message`, dispatches it
/// through the `site-main` flow, and returns a browser `Response`.
#[wasm_bindgen]
pub async fn handle_request(request: web_sys::Request) -> Result<web_sys::Response, JsValue> {
    solobase_browser::runtime::dispatch_request(request).await
}
