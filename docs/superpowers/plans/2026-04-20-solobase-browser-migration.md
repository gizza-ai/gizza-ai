# gizza-ai → solobase-browser Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate gizza-ai from its copy-pasted browser platform layer to the `solobase-browser` framework crate, deleting 8 Rust modules + 3 JS files + the `sql-assets` cross-repo fetch and replacing them with framework factories + parameterized templates.

**Architecture:** Swap 8 copy-pasted service modules for `solobase_browser::make_*` factory calls. Delete gizza's own `bridge.rs`, `database.rs`, `storage.rs`, `network.rs`, `crypto.rs`, `logger.rs`, `asset_loader.rs`, `convert.rs`. Delete `site/sw.js`, `site/loader.js`, `site/bridge.js`. Replace the `justfile`'s `sql-assets` fetch and hand-rolled `__BUILD_ID__` `sed` with one `cargo run -p solobase-browser --release --bin export-assets` invocation. Preserve every gizza-specific step in `initialize()` (block-config injection for solobase blocks, router override, flow override with custom CSP, block + skill registration, WRAP grants).

**Tech Stack:** Rust cdylib for wasm32-unknown-unknown; `wafer-run`/`wafer-core`/`wafer-block-config`; `solobase` + `solobase-core` for app composition; new path dep on `solobase-browser` for platform services. Shell + `just` for the build driver.

**Spec:** `docs/superpowers/specs/2026-04-20-solobase-browser-migration-design.md`

---

## File Structure

### Modified files

- `Cargo.toml` — add `solobase-browser`; drop 7 now-indirect deps.
- `src/lib.rs` — rewrite to use framework factories + `solobase_browser::runtime::*`. Keep every gizza-specific step.
- `justfile` — replace `sql-assets` + `cp site/* dist/` + `sed __BUILD_ID__` with `export-assets` + explicit `cp site/{index.html,gizza-app.js,gizza.css,ai-bridge.js} dist/`.

### Deleted files

- `src/bridge.rs`
- `src/database.rs`
- `src/storage.rs`
- `src/network.rs`
- `src/crypto.rs`
- `src/logger.rs`
- `src/asset_loader.rs`
- `src/convert.rs`
- `site/sw.js`
- `site/loader.js`
- `site/bridge.js`

### Preserved files

- `src/lib.rs` (rewritten in Task 2, not deleted)
- `src/config.rs` — gizza-specific config loader
- `src/skills.rs` — embedded skill-WASM loader
- `src/blocks/{mod,agent,ui}.rs` — gizza-specific block implementations
- `site/index.html` — gizza-branded markup
- `site/ai-bridge.js` — main-thread WebLLM bridge
- `site/gizza-app.js`, `site/gizza.css` — gizza UI
- `blocks/clock/` — gizza's WASM skill block source
- `tests/` — existing Playwright E2E
- `build.rs` — embeds skills at compile time

---

## Pre-implementation

Before Task 1, verify the local `solobase` sibling checkout has the solobase-browser framework merged to main and the path `../solobase/crates/solobase-browser/` resolves. Framework PRs #9 and #10 must be in the sibling's main. Run:

```bash
ls ../solobase/crates/solobase-browser/Cargo.toml
ls ../solobase/crates/solobase-browser/bin/export-assets.rs
ls ../solobase/crates/solobase-browser/assets/sw.js.tmpl
```

All three must exist. If any are missing, stop and resolve before proceeding.

---

## Task 1: `Cargo.toml` cleanup

**Files:**
- Modify: `Cargo.toml`

The crate will not compile after this step alone — `lib.rs` still imports from the about-to-be-deleted modules. That's expected; Task 2 finishes the atomic change.

- [ ] **Step 1: Add `solobase-browser` to `[dependencies]`**

After the existing `solobase-core = { path = "../solobase/crates/solobase-core", default-features = false }` line, add:

```toml
solobase-browser = { path = "../solobase/crates/solobase-browser" }
```

- [ ] **Step 2: Drop now-indirect deps**

Delete these lines from `[dependencies]` (all are transitive via `solobase-browser`):

- `serde-wasm-bindgen = "0.6"`
- `hex = "0.4"`
- `pbkdf2 = "0.12"`
- `hkdf = "0.12"`
- `sha2 = "0.10"`
- `hmac = "0.12"`
- `base64ct = { version = "1", features = ["alloc"] }`
- `wafer-block-crypto = { git = "https://github.com/wafer-run/wafer-run", branch = "main" }`

Keep everything else:
- `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys`, `js-sys` — used by `lib.rs` entrypoints and by `ai-bridge.js`'s wasm-bindgen bindings if any
- `console_error_panic_hook` — used by `module_start`
- `serde`, `serde_json`, `chrono` — used by `config.rs`, `skills.rs`, `blocks/`
- `async-trait` — used by gizza-specific blocks
- `maud` — used by `blocks/ui.rs` for HTML templating
- `wafer-run`, `wafer-block`, `wafer-core`, `wafer-block-config` — used by `lib.rs`'s `SolobaseBuilder` chain + flow JSON
- `solobase`, `solobase-core` — used for `SolobaseBuilder`, `RouteAccess`, `builder::post_start`
- `getrandom`, `uuid` (under `[target.'cfg(target_arch = "wasm32")']`) — kept

- [ ] **Step 3: Verify manifest parses**

Run: `cargo metadata --format-version 1 -p gizza-ai --no-deps > /dev/null`
Expected: no errors. This does not build the crate — it just validates the Cargo.toml manifest shape.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "refactor: depend on solobase-browser; drop now-indirect deps"
```

---

## Task 2: Rewrite `src/lib.rs` + delete moved modules

**Files:**
- Modify: `src/lib.rs` (complete rewrite)
- Delete: `src/bridge.rs`, `src/database.rs`, `src/storage.rs`, `src/network.rs`, `src/crypto.rs`, `src/logger.rs`, `src/asset_loader.rs`, `src/convert.rs`

### Step 1: Replace `src/lib.rs` with the following content

Exact file (paste verbatim):

```rust
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
```

Verify: no reference remains to `crate::{bridge,database,storage,network,crypto,logger,asset_loader,convert}`, no reference to the local `RUNTIME` thread_local, and no `pub mod convert/crypto/database/...` declarations.

### Step 2: Delete the moved Rust modules

```bash
git rm src/bridge.rs
git rm src/database.rs
git rm src/storage.rs
git rm src/network.rs
git rm src/crypto.rs
git rm src/logger.rs
git rm src/asset_loader.rs
git rm src/convert.rs
```

Verify the `src/` tree now contains only: `lib.rs`, `config.rs`, `skills.rs`, `blocks/` (directory).

### Step 3: Verify wasm32 compile

Run: `cargo check -p gizza-ai --target wasm32-unknown-unknown 2>&1 | tail -10`

Expected: clean compile. If you get "cannot find method `add_block_config`" or `add_flow_json`, check whether `wafer_run::Wafer`'s API has these exact names on the pinned git rev — if not, preserve whatever the pre-rewrite `lib.rs` used (those calls existed in the original code; don't invent signatures).

If the compile fails citing the old modules (e.g., `pub mod bridge;` still referenced from somewhere), double-check Step 1 removed all of them from the `pub mod …;` declarations at the top of `lib.rs`.

### Step 4: Verify `config.rs` / `skills.rs` / `blocks/` still build

These modules may import from the deleted modules (e.g., `skills.rs` might `use crate::bridge::…`). Grep for any such references:

```bash
grep -rn "crate::\(bridge\|database\|storage\|network\|crypto\|logger\|asset_loader\|convert\)" src/
```

Expected: no matches. If matches appear, fix them — replace `crate::bridge::…` with `solobase_browser::bridge::…` (etc.), or replace with the factory call, whichever matches the original semantics. Preserve behavior.

### Step 5: Commit

```bash
git add -A
git commit -m "refactor: migrate src/ to solobase-browser factories

Delete 8 copy-pasted service modules (bridge, database, storage,
network, crypto, logger, asset_loader, convert) now provided by
solobase-browser. Rewrite lib.rs to use solobase_browser::make_*
factories, solobase_browser::db_init, and
solobase_browser::runtime::{is_initialized, store_wafer,
dispatch_request}. All app-specific steps (block config injection,
router override, site-main flow override with gizza CSP, native
block + skill-WASM registration, WRAP grant injection) preserved."
```

---

## Task 3: `justfile` + `site/` cleanup

**Files:**
- Modify: `justfile`
- Delete: `site/sw.js`, `site/loader.js`, `site/bridge.js`

### Step 1: Replace `justfile`

Full new content:

```makefile
default:
    @just --list

# Build WASM skill blocks.
build-skills:
    #!/usr/bin/env bash
    set -euo pipefail
    for dir in blocks/*/; do
        if [ -f "$dir/Cargo.toml" ]; then
            echo "Building $dir"
            (cd "$dir" && /home/joris/Programs/suppers-ai/workspace/wafer-run/target/release/wafer build)
        fi
    done

# Build the main gizza-ai wasm.
build-wasm: build-skills
    wasm-pack build --target web --out-dir pkg

# Assemble dist/ from site/ + pkg/ + solobase-browser framework assets.
#
# solobase-browser's `export-assets` bin vendors sql.js, writes the
# parameterized sw.js/loader.js/index.html templates, content-hashes
# the wasm-pack output, and renders the templates. We then overwrite
# the default index.html with gizza's branded one and add gizza's
# UI scripts.
build: build-wasm
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf dist
    mkdir -p dist
    cp pkg/gizza_ai.js pkg/gizza_ai_bg.wasm dist/
    # wasm-pack emits `snippets/` referenced from the wasm-bindgen output.
    cp -r pkg/snippets dist/
    cargo run -p solobase-browser --release --bin export-assets -- dist/ \
        --repo-dir "$(pwd)" \
        --app-name gizza-ai \
        --app-title "Gizza AI" \
        --boot-redirect /
    # Gizza-branded index.html + app JS overwrite the framework defaults.
    cp site/index.html site/gizza-app.js site/gizza.css site/ai-bridge.js dist/

# Serve dist/ on localhost:8000.
serve: build
    python3 -m http.server --directory dist 8000

# Run the e2e smoke test.
test:
    cd tests && npx playwright test
```

Key differences from before:
- `sql-assets` target deleted (framework vendors sql.js).
- `cp site/* dist/` wildcard removed (would stomp framework assets).
- Hand-rolled `sed "s|__BUILD_ID__|…|"` deleted (framework's export-assets stamps build-id via sw.js.tmpl).
- `cp ../solobase/crates/solobase-web/pkg/sql-wasm-*` lines deleted (no more cross-repo fetch).
- New `cargo run -p solobase-browser --release --bin export-assets` invocation.
- Explicit `cp site/{index.html,gizza-app.js,gizza.css,ai-bridge.js} dist/` AFTER export-assets so gizza's index.html wins.

### Step 2: Delete the moved JS files

```bash
git rm site/sw.js
git rm site/loader.js
git rm site/bridge.js
```

Verify the `site/` tree now contains only: `index.html`, `ai-bridge.js`, `gizza-app.js`, `gizza.css`.

### Step 3: Run `just build`

```bash
just build 2>&1 | tail -15
```

Expected: `build-skills` builds the clock skill, `build-wasm` runs `wasm-pack`, `export-assets` runs successfully, file copies succeed, no errors.

After the run, verify the `dist/` tree contains:

```bash
ls dist/
```

Expected entries (exact names may vary by hash):

- `asset-manifest.json` — from framework
- `gizza_ai-<hash>.js` — content-hashed wasm-pack glue
- `gizza_ai_bg-<hash>.wasm` — content-hashed wasm
- `gizza_ai.d.ts`, `gizza_ai_bg.wasm.d.ts` — wasm-pack outputs, untouched
- `index.html` — gizza's branded HTML (not framework's)
- `ai-bridge.js`, `gizza-app.js`, `gizza.css` — gizza's UI scripts
- `loader.js` — rendered from framework's `loader.js.tmpl`, contains `/` as `__BOOT_REDIRECT__` and `[gizza-ai]` log prefixes
- `sw.js` — rendered from framework's `sw.js.tmpl`, references `/gizza_ai-<hash>.js` and has `[gizza-ai]` prefixes
- `snippets/` — wasm-pack snippet dir (contains framework's `bridge.js` + the embedded skill bridge files)
- `vendor/sql-wasm-esm.js`, `vendor/sql-wasm.wasm` — framework-vendored sql.js

### Step 4: Verify template substitution

```bash
grep -E '__[A-Z_]+__' dist/sw.js dist/loader.js dist/index.html && echo "FAIL: unresolved placeholder" || echo "OK"
```
Expected: `OK`.

```bash
head -3 dist/sw.js
```
Expected: first line is a build-id comment, second line is an import referencing `/gizza_ai-<hash>.js`.

```bash
grep "boot-redirect\|/b/system/" dist/loader.js
```
Expected: no match (gizza's boot-redirect is `/`, not `/b/system/`).

```bash
grep "gizza-ai" dist/index.html
```
Expected: matches, confirming gizza's HTML was copied over framework's default.

### Step 5: Commit

```bash
git add -A
git commit -m "refactor: wire justfile to solobase-browser export-assets

Replace sql-assets target (cross-repo fetch) and hand-rolled
__BUILD_ID__ sed with a single cargo run -p solobase-browser
--bin export-assets --app-name gizza-ai --app-title 'Gizza AI'
--boot-redirect /.  Delete site/sw.js, site/loader.js,
site/bridge.js — all three come from the framework's
parameterized templates now. Gizza's branded index.html and UI
scripts are cp'd AFTER export-assets to win over framework defaults."
```

---

## Task 4: Manual browser smoke test (verification gate — no commit)

**Goal:** verify the SW activates, loads the hashed wasm, sql.js opens the OPFS DB, and a chat round-trip via local-LLM streams tokens through the SW bridge.

### Step 1: Serve

```bash
just serve &
```

Wait for Python's HTTP server to print `Serving HTTP on :: port 8000`.

### Step 2: Open the site in Chrome

Navigate to `http://localhost:8000/`. Open DevTools → Application → Service Workers. Confirm:

- `/sw.js` is **activated**.
- Source of `sw.js` contains `[gizza-ai]` log prefixes (not `[solobase-web]`).
- Source of `sw.js` references `/gizza_ai-<hash>.js` in the import at top.

Network panel should show:

- `/sw.js` fetched (sw: 1).
- `/gizza_ai-<hash>.js` fetched.
- `/gizza_ai_bg-<hash>.wasm` fetched.
- `/vendor/sql-wasm-esm.js` and `/vendor/sql-wasm.wasm` fetched when the DB initializes.
- `/snippets/<wasm-pack-hash>/bridge.js` and `/snippets/<wasm-pack-hash>/js/bridge.js` (depending on wasm-pack's snippet layout) fetched.

Console panel should show the `gizza-ai: panic hook installed` line, variables-loaded count, at least one `gizza-ai: skill '…' registered` line, and `gizza-ai: WAFER runtime started`.

### Step 3: Exercise the UI

Navigate to `/b/ui/` (or click through from `/`). The gizza UI should render.

Navigate to `/b/agent/` or trigger a chat from the UI. Select a small model (e.g., SmolLM2 1.7B). The LLM should load (expect a ~1-minute download on first load). Send a chat message. Verify tokens stream back.

### Step 4: What to check if it fails

If the SW fails to install: check `grep -E '__[A-Z_]+__' dist/sw.js` — unresolved placeholder means export-assets ran with missing args.

If `/gizza_ai_bg-*.wasm` 404s: check `asset-manifest.json` and the SW's import URL for mismatch.

If the DB fails to open: check `/vendor/sql-wasm.wasm` resolves (200 OK) and that the sql.js ESM import path is reachable.

If the chat fails: check `/b/local-llm/api/chat_stream` returns an SSE stream in Network. If the framework's sw.js.tmpl doesn't route that path, the framework needs a fix — report DONE_WITH_CONCERNS on the plan, revert the Task 3 commit locally, and file a small solobase PR to update sw.js.tmpl.

### Step 5: Document the outcome

This is a verification gate, not a code change. No commit.

If the smoke test passes, proceed to Task 5.
If it fails in a way that requires a framework fix, revert Task 3's commit (`git reset --hard HEAD~1`) and open a solobase PR; return here when it lands.

---

## Task 5: Run Playwright E2E

**Files:** no code changes; verify only.

### Step 1: Confirm Playwright is set up

```bash
ls tests
```

Expected: `tests/` directory with at least one `.spec.ts` file and a `package.json` or `playwright.config.ts` (whatever the existing test infrastructure uses).

### Step 2: Run the tests

With `just serve &` already running from Task 4 (or start it again):

```bash
just test 2>&1 | tail -20
```

Expected: tests pass. If they fail due to URL changes that only the migration introduced (hashed asset URLs, for instance), update the tests — but be conservative: changes should only cover what the migration genuinely changed, not bugfixes to the tests themselves.

### Step 3: Commit test adjustments if any

```bash
git add tests/
git commit -m "test: adjust Playwright selectors for hashed asset URLs"
```

Skip this step if no test changes were needed.

---

## Task 6: Final sanity

**Files:** no changes; just verifications.

- [ ] **Step 1: No references to deleted modules remain**

```bash
grep -rn "crate::\(bridge\|database\|storage\|network\|crypto\|logger\|asset_loader\|convert\)\b" src/ build.rs tests/ 2>&1
grep -rn "solobase-web/pkg/sql-wasm\|solobase-web/pkg/sql-wasm" . --include=justfile --include="*.rs" --include="*.md" 2>&1 | grep -v "^\./docs" | grep -v target | grep -v "\.worktrees"
```

Expected: first grep empty; second grep may only show references in `docs/`. Anything else is a missed migration step.

- [ ] **Step 2: Manifest is tidy**

```bash
grep -E "pbkdf2|hkdf|hmac|base64ct|wafer-block-crypto|serde-wasm-bindgen|^hex = " Cargo.toml
```

Expected: no match.

```bash
grep "solobase-browser" Cargo.toml
```

Expected: one line, `solobase-browser = { path = "../solobase/crates/solobase-browser" }`.

- [ ] **Step 3: Clean build works from scratch**

```bash
cargo clean 2>&1 | tail -2
just build 2>&1 | tail -5
```

Expected: all phases succeed. `dist/` ends up populated with the expected files.

- [ ] **Step 4: No uncommitted changes**

```bash
git status --short
```

Expected: empty.

---

## Self-Review Checklist

- [ ] **Spec coverage**:
  - "`Cargo.toml` gains `solobase-browser`" → Task 1.
  - "Drops now-indirect deps" → Task 1.
  - "Delete 8 Rust modules" → Task 2.
  - "Rewrite `lib.rs` to use framework factories" → Task 2.
  - "Delete `site/sw.js`, `site/loader.js`, `site/bridge.js`" → Task 3.
  - "`justfile` `export-assets` invocation with flags" → Task 3.
  - "Gizza's `index.html` / JS cp'd AFTER export-assets" → Task 3.
  - "Manual browser smoke test" → Task 4.
  - "Playwright E2E re-run" → Task 5.
- [ ] **Placeholder scan**: every step has concrete commands or full code blocks. No "TBD" / "similar to". The `lib.rs` rewrite in Task 2 reproduces the current 263-step initialize() function in full rather than referring to "preserve app-specific steps".
- [ ] **Type consistency**: `solobase_browser::make_*` factory names, `solobase_browser::runtime::{is_initialized,store_wafer,dispatch_request}`, and `solobase_browser::db_init` used consistently across Task 2 (rewrite) and Task 6 (verification grep).
- [ ] **Commit hygiene**: 3 commit-producing tasks (1, 2, 3) + 1 verification gate (4) + 1 optional test-adjust (5) + 0-commit sanity (6). Each commit is atomic and independently revertable.
