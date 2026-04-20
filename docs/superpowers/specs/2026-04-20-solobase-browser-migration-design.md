# gizza-ai → solobase-browser Migration Design

**Date:** 2026-04-20
**Status:** Design approved; pending implementation plan
**Repo:** gizza-ai (this repo)
**Sub-project:** 4 of 4 in the solobase framework refactor

## Problem

Gizza-ai today duplicates the browser platform layer. Eight Rust modules (`bridge`, `database`, `storage`, `network`, `crypto`, `logger`, `asset_loader`, `convert`) and three JS files (`sw.js`, `loader.js`, `bridge.js`) are copy-pasted from solobase-web — `src/bridge.rs:5` literally says *"Copied from solobase-web"*. The `justfile` also reaches across the filesystem with `cp ../solobase/crates/solobase-web/pkg/sql-wasm-*.{js,wasm} dist/` for sql.js assets.

Consequences:

- Bug fixes in the solobase-web platform layer don't propagate to gizza-ai automatically.
- `cp ../solobase/...` breaks builds outside a local sibling checkout (CI, other consumers).
- Changes to the browser layer are riskier because the same code now lives in two places.

The solobase framework refactor's sub-projects 1–3 produced a `solobase-browser` crate that exposes exactly these platform services as factory functions plus a parameterized asset bundler. This sub-project migrates gizza-ai onto that crate.

## Non-Goals

- Any framework changes. This PR only consumes `solobase-browser` as-is.
- Removing gizza-ai's dependency on `solobase` / `solobase-core`. Gizza uses `SolobaseBuilder` to wire up solobase's service blocks + routing + site-main flow, and that stays.
- LLM service refactor. Gizza's `site/ai-bridge.js` (WebLLM driver) and the SW-side local-LLM bridge protocol are both out of scope. Either the framework's `sw.js.tmpl` handles gizza's protocol unchanged (expected) or a follow-up PR on solobase updates it.
- Build-id cache-busting migration polish — the framework's content-hashing replaces gizza's `?v=__BUILD_ID__` query-string trick; old cache-busting logic disappears naturally with the `sw.js` deletion.

## Chosen Approach

**Swap copy-pasted Rust modules for `solobase_browser::make_*` factory calls. Delete the three copy-pasted JS files and let the framework's `export-assets` bin write its parameterized replacements into `dist/`. Keep gizza-specific Rust (blocks, skills loader, config) and gizza-specific JS (branded index.html, WebLLM main-thread bridge, UI) untouched.**

Gizza-ai's existing structure stays. Its `initialize()` keeps all nine app-specific steps (config seeding, JWT extraction, block registration, route injection, etc.) — only the service-construction expressions change from `Arc::new(database::BrowserDatabaseService)` to `solobase_browser::make_database_service()`, and the `thread_local! RUNTIME` moves to `solobase_browser::runtime`.

## Architecture

### Dependency change

`Cargo.toml` gains:
```toml
solobase-browser = { path = "../solobase/crates/solobase-browser" }
```

It drops the now-indirect deps: `hex`, `pbkdf2`, `hkdf`, `sha2`, `hmac`, `base64ct`, `serde-wasm-bindgen`, `wafer-block-crypto`.

The existing deps on `solobase`, `solobase-core`, `wafer-run`, `wafer-core`, `wafer-block`, `wafer-block-config` stay — gizza still uses `SolobaseBuilder` for app composition.

### `src/` changes

**Delete** (replaced by framework factories):
- `src/bridge.rs`
- `src/database.rs`
- `src/storage.rs`
- `src/network.rs`
- `src/crypto.rs`
- `src/logger.rs`
- `src/asset_loader.rs`
- `src/convert.rs`

**Keep** (gizza-specific):
- `src/lib.rs` — rewritten to use framework factories; shape is parallel to solobase-web's post-migration lib.rs
- `src/config.rs` — gizza reads its own config vars
- `src/skills.rs` — embedded skill-WASM loader (gizza's skill-block embedding)
- `src/blocks/` — gizza's `agent` and `ui` block implementations

### `src/lib.rs` after migration

The rewritten file keeps every app-specific step, replacing only the service construction, thread_local, and convert-based dispatch:

```rust
//! gizza-ai — browser-local AI chat site.
//!
//! Thin wasm-bindgen wrapper around the `solobase-browser` framework. Uses
//! `SolobaseBuilder` to wire up solobase's service blocks + router + site-main
//! flow, then registers gizza-specific feature blocks (agent, ui) and embedded
//! skill WASM blocks, plus routes `/`, `/b/ui/`, `/b/agent/` as Public tier.

use std::sync::Arc;

use solobase::builder::{self, SolobaseBuilder};
use solobase_core::RouteAccess;
use wafer_core::interfaces::config::service::ConfigService;
use wasm_bindgen::prelude::*;

pub mod blocks;
pub mod config;
pub mod skills;

#[wasm_bindgen(start)]
pub fn module_start() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"gizza-ai: panic hook installed".into());
}

#[wasm_bindgen]
pub async fn initialize() -> Result<(), JsValue> {
    if solobase_browser::runtime::is_initialized() {
        return Ok(());
    }

    solobase_browser::db_init().await;

    let vars = config::seed_and_load_variables();
    web_sys::console::log_1(
        &format!("gizza-ai: {} variables loaded from database", vars.len()).into(),
    );

    let features = config::load_block_settings();
    let jwt_secret = vars
        .get("SUPPERS_AI__AUTH__JWT_SECRET")
        .cloned()
        .unwrap_or_default();

    let config_svc = wafer_block_config::service::EnvConfigService::new();
    for (key, value) in &vars {
        config_svc.set(key, value);
    }

    let (mut wafer, storage_block) = SolobaseBuilder::new()
        .database(solobase_browser::make_database_service())
        .storage(solobase_browser::make_storage_service())
        .config(Arc::new(config_svc))
        .crypto(solobase_browser::make_crypto_service(jwt_secret))
        .network(solobase_browser::make_network_service())
        .logger(solobase_browser::make_console_logger())
        .block_settings(features)
        // Gizza-specific: inject gizza blocks + routes as Public tier.
        // Match the exact constructors + route list from the current lib.rs;
        // the actual file may register blocks via Arc::new(UiBlock) (unit
        // struct) and may include both trailing-slash and no-trailing-slash
        // route variants (e.g., /b/ui and /b/ui/).
        .register_block("gizza-ai/ui", Arc::new(blocks::ui::UiBlock))
        .register_block("gizza-ai/agent", Arc::new(blocks::agent::AgentBlock))
        .add_route("/", "gizza-ai/ui", RouteAccess::Public)
        .add_route("/b/ui", "gizza-ai/ui", RouteAccess::Public)
        .add_route("/b/ui/", "gizza-ai/ui", RouteAccess::Public)
        .add_route("/b/agent/", "gizza-ai/agent", RouteAccess::Public)
        .build()
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Register embedded skill WASM blocks (gizza-specific).
    skills::register_embedded_skills(&mut wafer)
        .map_err(|e| JsValue::from_str(&e))?;

    wafer.set_asset_loader(solobase_browser::make_sw_asset_loader());

    wafer
        .start_without_bind()
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    builder::post_start(&wafer, &storage_block);

    web_sys::console::log_1(&"gizza-ai: WAFER runtime started".into());

    solobase_browser::runtime::store_wafer(wafer);

    Ok(())
}

#[wasm_bindgen]
pub async fn handle_request(request: web_sys::Request) -> Result<web_sys::Response, JsValue> {
    solobase_browser::runtime::dispatch_request(request).await
}
```

**Spot-check before writing during implementation:** open the current `src/lib.rs` and preserve any gizza-specific block registrations, route injections, or post-start steps that aren't shown here. The template reflects what the spec captured; verify parity by reading the actual file rather than assuming it matches.

### `site/` changes

**Delete** (replaced by `solobase-browser`'s parameterized templates):
- `site/sw.js` — framework ships `sw.js.tmpl` with the full local-LLM bridge protocol
- `site/loader.js` — framework ships `loader.js.tmpl` with `__BOOT_REDIRECT__`
- `site/bridge.js` — tied to the moved `bridge.rs`, now co-located in `solobase-browser/js/bridge.js` and pulled in by wasm-pack's snippets mechanism

**Keep** (app-specific):
- `site/index.html` — gizza-branded markup; loads `gizza.css`, `gizza-app.js`, `ai-bridge.js`
- `site/ai-bridge.js` — gizza's main-thread WebLLM bridge (CDN import, gizza-specific model catalog)
- `site/gizza-app.js`, `site/gizza.css` — gizza's UI

### `justfile` changes

Before:
```makefile
sql-assets:
    # ... fetches sql.js from ../solobase/crates/solobase-web/pkg/ ...

build: build-wasm sql-assets
    rm -rf dist && mkdir -p dist
    cp site/* dist/
    cp pkg/gizza_ai.js dist/
    cp pkg/gizza_ai_bg.wasm dist/
    cp -r pkg/snippets dist/
    cp ../solobase/crates/solobase-web/pkg/sql-wasm-esm.js dist/
    cp ../solobase/crates/solobase-web/pkg/sql-wasm.wasm dist/
    BUILD_ID=$(git rev-parse --short HEAD 2>/dev/null || date +%s)
    sed -i "s|__BUILD_ID__|${BUILD_ID}|g" dist/sw.js
```

After:
```makefile
# `sql-assets` target deleted — sql.js is vendored inside solobase-browser.

build: build-wasm
    rm -rf dist && mkdir -p dist
    cp pkg/gizza_ai.js pkg/gizza_ai_bg.wasm dist/
    cp -r pkg/snippets dist/
    cargo run -p solobase-browser --release --bin export-assets -- dist/ \
        --repo-dir $(shell pwd) \
        --app-name gizza-ai \
        --app-title "Gizza AI" \
        --boot-redirect /
    # Gizza-branded index.html + app JS overwrite the framework defaults.
    cp site/index.html site/gizza-app.js site/gizza.css site/ai-bridge.js dist/
```

Key changes:
- `sql-assets` target deleted; the cross-repo `cp` disappears.
- Hand-rolled `sed "__BUILD_ID__"` stamp deleted — the framework's `export-assets` handles build-id stamping through the `sw.js.tmpl` flow.
- `cp site/* dist/` wildcard removed — would clobber framework assets.
- Explicit per-file `cp site/{index.html,gizza-app.js,gizza.css,ai-bridge.js} dist/` runs AFTER `export-assets` so gizza's branded `index.html` wins over the framework's default.

## Rollout

Three atomic phases + one verification gate:

1. **Cargo.toml cleanup.** Add `solobase-browser` dep; drop now-indirect deps. Crate won't compile yet — `lib.rs` still imports from the about-to-be-deleted modules. One commit.
2. **Rewrite `src/lib.rs` + delete moved modules.** Eight `src/*.rs` files deleted; `lib.rs` rewritten to use framework factories. `cargo check -p gizza-ai --target wasm32-unknown-unknown` passes. One commit.
3. **`justfile` + `site/` cleanup.** Update `build` target; delete `site/sw.js`, `site/loader.js`, `site/bridge.js`. `just build` produces a working `dist/`. One commit.
4. **Manual browser smoke test.** Open `just serve`, verify in DevTools: SW activates, `dist/gizza_ai-<hash>.js` and `dist/gizza_ai_bg-<hash>.wasm` are fetched (confirms content hashing works with gizza's wasm-pack output), sql.js loads via `/vendor/sql-wasm-esm.js`, gizza-app.js renders the UI, clicking through to `/b/ui/` reaches the UI block, opening `/b/agent/` and sending a chat drives WebLLM via ai-bridge.js. **No commit — gate only.** If smoke test fails, the phase-3 commit is revertable.

Each phase is independently revertable. If phase 3 surfaces a framework-shape issue (e.g., `sw.js.tmpl` doesn't cover gizza's local-LLM protocol), revert phase 3 and open a small follow-up PR on solobase.

## Testing

- **Unit tests:** none to add. Gizza-ai has no existing unit tests for the browser platform modules (it just copy-pasted solobase-web's implementation without tests). After migration, those tests live in `solobase-browser`'s unit+integration suite and don't need to be duplicated here.
- **`cargo check`:** after each rollout phase, `cargo check -p gizza-ai --target wasm32-unknown-unknown` must pass clean.
- **E2E (Playwright):** gizza-ai has an existing Playwright smoke test in its MVP work (`tests/`). Re-run after phase 3 — this is a real end-to-end validation from an independent consumer's perspective and catches any framework-contract drift.
- **Bundle size sanity:** after `just build`, check `ls -lSr dist/` — confirm the hashed `gizza_ai_bg-<hash>.wasm` is present and roughly the same size as the previous unhashed version (no accidental content inflation).

## Risks

- **Local-LLM protocol divergence.** Framework's `sw.js.tmpl` inherits solobase-web's Phase C/D local-LLM bridge (`/b/local-llm/api/chat_stream` routes + SSE stream plumbing). Gizza's 285-line `site/sw.js` has the same protocol (both were derived from the same source). Expected to work unchanged, but phase-4 smoke test is the verification point. If protocol drift is real, the fix is a small PR on solobase updating `sw.js.tmpl` — not a gizza-ai concern.
- **`asset_loader.rs` protocol.** Framework's `asset_loader` uses a SW→main postMessage bridge for external asset loads (skill WASM files). Gizza's `skills.rs` uses this bridge. Framework must preserve the exact message types (`load-asset-request` / `load-asset-response` with the same shape). Verified by reading the framework's current `asset_loader.rs` — unchanged from solobase-web's, so gizza's usage should work identically.
- **wasm-pack crate name.** Gizza-ai's cdylib produces `gizza_ai.js` + `gizza_ai_bg.wasm`. The framework's bundler auto-discovers this pair from `dist/`. `--app-name gizza-ai` feeds the log prefix and `__WASM_JS_PREFIX__` resolves to `/gizza_ai` for the SW fetch bypass rule. All derived from the discovered filename; no separate config.
- **`add_route` method on `SolobaseBuilder`.** The rewritten `lib.rs` calls `.add_route("/", "gizza-ai/ui", RouteAccess::Public)`. Verify during implementation that this is the current method name on `SolobaseBuilder` (the existing `src/lib.rs` uses this exact method, but Phase C/D may have renamed it). Preserve whatever the current code uses.
- **`register_block` signature on `SolobaseBuilder`.** Similar check — the rewritten code calls `.register_block("gizza-ai/ui", Arc::new(UiBlock::new()))`. Confirm signature matches current `solobase`.

## Sub-project trail note

After this PR merges, add a 5-line note to `solobase/docs/superpowers/specs/` (cross-link back to this spec and the implementation PR on gizza-ai). This keeps the four-sub-project audit trail complete in solobase's spec directory, where sub-projects 1-3 already live, without polluting solobase's git history with gizza-specific code. Written as a separate commit in solobase after gizza-ai's PR lands.

## Summary

Add `solobase-browser` as a path dep. Swap eight copy-pasted Rust modules for factory calls. Delete three copy-pasted JS files and lean on the framework's parameterized templates via `export-assets --app-name gizza-ai --app-title "Gizza AI" --boot-redirect /`. Preserve gizza-specific Rust (blocks, skills, config) and JS (index.html, ai-bridge.js, gizza-app.js, gizza.css). Three atomic rollout phases + a browser smoke test gate. Validates the solobase-browser framework against a real second consumer.
