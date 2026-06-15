//! gizza-ai — browser-local AI chat site.
//!
//! Compiles to wasm32 via wasm-bindgen; loaded by a Service Worker that
//! forwards requests through the WAFER runtime. `initialize()` builds the
//! runtime via `SolobaseBuilder` using browser platform services from
//! `solobase-browser`, registers gizza's curated feature blocks plus the
//! native agent/ui blocks and every embedded skill WASM, and wires `/`,
//! `/b/ui/`, `/b/ui`, and `/b/agent/` to gizza blocks as Public tier.
//! `handle_request()` dispatches through the `site-main` flow.

#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use solobase_core::builder::{self, SolobaseBuilder};
#[cfg(target_arch = "wasm32")]
use solobase_core::RouteAccess;
#[cfg(target_arch = "wasm32")]
use wafer_core::interfaces::config::service::ConfigService;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

pub mod blocks;
#[cfg(target_arch = "wasm32")]
pub mod config;
pub mod ffmpeg;
pub mod skills;

// ---------------------------------------------------------------------------
// module_start()
// ---------------------------------------------------------------------------

/// Module init — runs automatically before any other wasm-bindgen export is
/// first called. Install the panic hook here so ANY panic (including ones in
/// code paths that don't go through initialize()) surfaces in the console.
#[cfg(target_arch = "wasm32")]
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
/// and `wafer.seal()`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn initialize() -> Result<(), JsValue> {
    // Guard against double initialization.
    if solobase_browser::runtime::is_initialized() {
        return Ok(());
    }

    // 1. Load sql.js WASM + open/create the OPFS database.
    solobase_browser::db_init().await;

    // ── Phase 1 ─────────────────────────────────────────────────────────────
    // Build the runtime with EMPTY config + EMPTY BlockSettings. We can't
    // pre-fill them from OPFS yet because the `suppers_ai__admin__block_settings`
    // table only exists after admin's `lifecycle(Init)` has run its
    // migrations — and admin can't run until the wafer is built and sealed.
    // gizza-ai's own `variables` table is independent of the admin tables
    // and CAN be seeded earlier, but for symmetry with the solobase-web
    // restructure (and so any future schema additions to the gizza vars
    // table can also rely on a known runtime baseline) we defer all seeding
    // to Phase 3. See solobase #212 for the equivalent rewrite there.
    let config_svc: Arc<dyn ConfigService> =
        Arc::new(wafer_core::service_blocks::config::EnvConfigService::new());
    let initial_block_settings =
        solobase_core::features::BlockSettings::from_map(std::collections::HashMap::new());

    // Browser WebLLM service — provided by solobase-browser in Phase D.
    //     Registered on the MultiBackendLlmService router under the label
    //     "browser"; BrowserLlmService::claims_backend matches the
    //     `"webllm"` backend_id that ChatRequest will carry.
    let browser_llm: Arc<dyn wafer_core::interfaces::llm::service::LlmService> =
        Arc::new(solobase_browser::llm::BrowserLlmService::new());

    // Browser image service — Janus-Pro-1B via transformers.js on WebGPU.
    //     Registered on the MultiBackendImageService router under the label
    //     "browser"; BrowserImageService::claims_backend matches the
    //     `"transformers-image"` backend_id that ImageRequest will carry.
    let browser_image: Arc<dyn wafer_core::interfaces::image::service::ImageService> =
        Arc::new(solobase_browser::image::BrowserImageService::new());

    // Crypto starts with an empty JWT secret; the persisted secret is loaded
    // in Phase 3 and installed via `BrowserCryptoService::set_jwt_secret`.
    let crypto_concrete = Arc::new(solobase_browser::crypto::BrowserCryptoService::new(
        String::new(),
    ));
    let crypto_svc: Arc<dyn wafer_core::interfaces::crypto::service::CryptoService> =
        crypto_concrete.clone();

    let builder = SolobaseBuilder::new()
        .database(solobase_browser::make_database_service())
        .storage(solobase_browser::make_storage_service())
        .config(config_svc.clone())
        .crypto(crypto_svc)
        .network(solobase_browser::make_network_service())
        .logger(solobase_browser::make_console_logger())
        .llm_service("browser", browser_llm)
        .image_service("browser", browser_image)
        .block_settings(initial_block_settings)
        .block_config(
            "wafer-run/security-headers",
            serde_json::json!({
                "csp": concat!(
                    "default-src 'self'; ",
                    "script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval' ",
                        "https://cdn.jsdelivr.net https://site-kit.suppers.ai; ",
                    "style-src 'self' 'unsafe-inline' https://site-kit.suppers.ai https://cdn.jsdelivr.net; ",
                    "img-src 'self' data: blob: https:; ",
                    "media-src 'self' data: blob:; ",
                    "font-src 'self' https:; ",
                    "connect-src 'self' https://cdn.jsdelivr.net https://esm.run https://huggingface.co ",
                        "https://raw.githubusercontent.com https://*.huggingface.co https://*.hf.co https://*.xethub.hf.co; ",
                    "frame-ancestors 'none'; ",
                    "base-uri 'self'; ",
                    "form-action 'self'"
                )
            }),
        )
        // The auth block reads its config (JWT_SECRET, ADMIN_EMAIL,
        // ADMIN_PASSWORD, INTERNAL_SECRET) from the config service we just
        // populated from `vars`. The DB seed in `config.rs` is the single
        // source of truth: admin email/password from the `INSERT OR IGNORE`
        // seed, JWT/INTERNAL secrets from the per-browser random `seed_random_secret`
        // call. No `.block_config("suppers-ai/auth", ...)` override here —
        // historically that override silently shipped the same hardcoded MVP
        // values to every user, defeating the auto-generate path.
        .block_config(
            "suppers-ai/llm",
            serde_json::json!({
                "SUPPERS_AI__LLM__DEFAULT_PROVIDER": "suppers-ai/local-llm",
                "SUPPERS_AI__LLM__DEFAULT_MODEL": "Qwen2.5-1.5B-Instruct-q4f32_1-MLC",
            }),
        )
        // Note: `/` is routed at the OUTER wafer-run/router level (see step
        // 6a below), so it never reaches suppers-ai/router. Adding `/` to
        // suppers-ai/router's extra routes here would be dead code AND
        // actively harmful — its match uses `starts_with`, so a `/` prefix
        // would catch every `/b/**` request that has no built-in solobase
        // route (e.g. `/b/agent/chat`) before more specific extras.
        .add_route("/b/ui", "gizza-ai/ui", RouteAccess::Public)
        .add_route("/b/ui/", "gizza-ai/ui", RouteAccess::Public)
        .add_route("/b/agent/", "gizza-ai/agent", RouteAccess::Public);
    let block_settings_handle = builder.block_settings_handle();

    let (mut wafer, storage_block) = builder
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

    // 6b. Register the SW-side external-asset loader before start so any
    // block init that triggers an asset load sees the real loader (not
    // the NoopAssetLoader default).
    wafer.set_asset_loader(&solobase_browser::make_sw_asset_loader());

    // 6c. Register gizza-ai's native blocks (agent + ui) and every
    // embedded skill WASM from skills::SKILLS (produced by build.rs).
    wafer
        .register_block("gizza-ai/agent", Arc::new(blocks::agent::AgentBlock))
        .map_err(|e| JsValue::from_str(&format!("register gizza-ai/agent: {e}")))?;
    wafer
        .register_block("gizza-ai/ui", Arc::new(blocks::ui::UiBlock))
        .map_err(|e| JsValue::from_str(&format!("register gizza-ai/ui: {e}")))?;

    let ffmpeg_svc: Arc<dyn ffmpeg::FfmpegService> = Arc::new(ffmpeg::BrowserFfmpegService);
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(blocks::ffmpeg::FfmpegBlock::new(ffmpeg_svc)),
        )
        .map_err(|e| JsValue::from_str(&format!("register gizza-ai/ffmpeg-runtime: {e}")))?;

    for (name, bytes) in skills::SKILLS {
        let wasmi = wafer_run::wasm::WasmiBlock::load_from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&format!("loading skill {name}: {e}")))?;
        wafer
            .register_block(*name, Arc::new(wasmi))
            .map_err(|e| JsValue::from_str(&format!("registering skill {name}: {e}")))?;
        web_sys::console::log_1(&format!("gizza-ai: skill '{name}' registered").into());
    }

    // 7. Seal the runtime.
    wafer
        .seal()
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // ── Phase 2 ─────────────────────────────────────────────────────────────
    // Run admin's `lifecycle(Init)` so its migrations create the canonical
    // `suppers_ai__admin__block_settings` (+ `suppers_ai__admin__variables`)
    // tables. gizza-ai disables admin in its feature flags so it never
    // routes there, but other enabled blocks' migrations call
    // `migration_helper::write_state` against `block_settings` and need
    // the strict schema in place. Forcing the admin Init unconditionally
    // here mirrors the solobase-web restructure (#212) and avoids the
    // schema-drift pre-create pattern (`load_block_settings` no longer
    // pre-creates the admin table).
    if let Err(e) = wafer
        .init_block(solobase_core::blocks::admin::ADMIN_BLOCK_ID)
        .await
    {
        web_sys::console::error_1(&format!("gizza-ai: admin block Init failed: {e}").into());
        return Err(JsValue::from_str(&format!("admin init failed: {e}")));
    }

    // ── Phase 3 ─────────────────────────────────────────────────────────────
    // Now the admin tables exist. Seed/load against them, then publish into
    // the services the wafer already holds:
    //  - `config_svc` (mutated via Arc<dyn ConfigService>::set)
    //  - `block_settings_handle` (the shared RwLock the router's FeatureConfig
    //    also reads from)
    //  - `crypto_concrete` (rotated via set_jwt_secret)
    let vars = config::seed_and_load_variables()?;
    web_sys::console::log_1(
        &format!("gizza-ai: {} variables loaded from database", vars.len()).into(),
    );
    let features = config::load_block_settings()?;
    for (key, value) in &vars {
        config_svc.set(key, value);
    }
    config_svc.set(
        solobase_core::features::BLOCK_SETTINGS_CONFIG_KEY,
        &features.to_config_json(),
    );
    *block_settings_handle
        .write()
        .expect("BlockSettings RwLock poisoned") = features;
    if let Some(secret) = vars.get("SUPPERS_AI__AUTH__JWT_SECRET") {
        crypto_concrete.set_jwt_secret(secret.clone());
    }

    // ── Phase 4 ─────────────────────────────────────────────────────────────
    // Eager Init the remaining blocks. Slot caching makes admin's second
    // init a no-op. Auth's bootstrap reads BOOTSTRAP_ADMIN_{EMAIL,PASSWORD}
    // via config_client::get_default (which now sees the values we just
    // pushed into `config_svc`).
    wafer.init_all_blocks().await;

    // 8. Inject WRAP grants.
    builder::post_start(&wafer, &storage_block);

    web_sys::console::log_1(&"gizza-ai: WAFER runtime started".into());

    // 9. Store in framework's thread_local. Errors if a runtime is already
    //    stored (double-init) — surface that as a boot failure.
    solobase_browser::runtime::store_wafer(wafer).map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// handle_request()
// ---------------------------------------------------------------------------

/// Handle an incoming fetch request from the Service Worker.
///
/// Converts the browser `Request` into a WAFER `Message`, dispatches it
/// through the `site-main` flow, and returns a browser `Response`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn handle_request(request: web_sys::Request) -> Result<web_sys::Response, JsValue> {
    solobase_browser::runtime::dispatch_request(request).await
}
