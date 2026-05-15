//! Browser-side variable seeding and loading.
//!
//! Mirrors the native `seed_and_load_variables()` / `load_block_settings()` from
//! `solobase/src/main.rs`, but uses the JS bridge (`bridge::db_exec_raw` /
//! `bridge::db_query_raw`) instead of `rusqlite`.
//!
//! All bootstrap fns are fallible (`Result<_, JsValue>`) so that a real SQL
//! error at boot — schema create failure, OPFS quota, etc. — surfaces to
//! the Service Worker instead of letting the runtime start with empty config.

use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
use solobase_browser::bridge;
use wasm_bindgen::JsValue;

/// Run a write SQL statement, propagating any failure as a JsValue with the
/// SQL snippet attached for debuggability. The bridge already raises on bad
/// SQL; this just labels the error.
#[cfg(target_arch = "wasm32")]
fn exec_or_err(sql: &str, params: &str) -> Result<(), JsValue> {
    bridge::db_exec_raw(sql, params).map(|_| ()).map_err(|e| {
        JsValue::from_str(&format!(
            "config: db_exec_raw failed: {e:?} sql={}",
            short(sql)
        ))
    })
}

#[cfg(target_arch = "wasm32")]
fn query_or_err(sql: &str, params: &str) -> Result<String, JsValue> {
    bridge::db_query_raw(sql, params).map_err(|e| {
        JsValue::from_str(&format!(
            "config: db_query_raw failed: {e:?} sql={}",
            short(sql)
        ))
    })
}

/// Trim a SQL string to its first 80 chars for error messages — full
/// schema-create statements are too long to be useful in the console.
fn short(sql: &str) -> String {
    let trimmed = sql.trim();
    if trimmed.len() <= 80 {
        trimmed.to_string()
    } else {
        format!("{}…", &trimmed[..80])
    }
}

// ---------------------------------------------------------------------------
// Variable seeding and loading
// ---------------------------------------------------------------------------

/// Ensure the variables table exists, auto-generate secrets for config vars
/// marked `auto_generate: true`, and return all variables from the DB.
///
/// This is the browser equivalent of the native `seed_and_load_variables()`.
/// There are no env vars to seed from in the browser — only auto-generated
/// secrets and previously-stored values.
pub fn seed_and_load_variables() -> Result<HashMap<String, String>, JsValue> {
    // 1. Create variables table if it does not exist
    exec_or_err(
        "CREATE TABLE IF NOT EXISTS variables (
            id TEXT PRIMARY KEY,
            key TEXT NOT NULL UNIQUE,
            name TEXT DEFAULT '',
            description TEXT DEFAULT '',
            value TEXT DEFAULT '',
            warning TEXT DEFAULT '',
            sensitive INTEGER DEFAULT 0,
            updated_by TEXT DEFAULT '',
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
        "[]",
    )?;
    exec_or_err(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_variables_key ON variables (key)",
        "[]",
    )?;

    // 2. Seed default admin account for browser build.
    //    Email: admin@solobase.local / Password: admin
    //    This is local-only (OPFS) so a simple default is acceptable.
    exec_or_err(
        "INSERT OR IGNORE INTO variables (id, key, name, description, value, sensitive, created_at, updated_at)
         VALUES ('var_admin_email', 'SUPPERS_AI__AUTH__ADMIN_EMAIL', 'Admin Email', 'Admin account email', 'admin@solobase.local', 0, datetime('now'), datetime('now'))",
        "[]",
    )?;
    exec_or_err(
        "INSERT OR IGNORE INTO variables (id, key, name, description, value, sensitive, created_at, updated_at)
         VALUES ('var_admin_pass', 'SUPPERS_AI__AUTH__ADMIN_PASSWORD', 'Admin Password', 'Admin account password', 'admin', 1, datetime('now'), datetime('now'))",
        "[]",
    )?;

    // Inject the framework's page-side WebLLM engine into every SSR-rendered
    // page served via solobase-core's layout (admin UI, etc.). gizza's own
    // /b/ui page has its own maud template that loads /webllm-engine.js
    // directly, so this seed is strictly for framework-rendered surfaces.
    exec_or_err(
        "INSERT OR IGNORE INTO variables (id, key, name, description, value, sensitive, created_at, updated_at)
         VALUES ('var_embedded_scripts', 'SOLOBASE_SHARED__EMBEDDED_SCRIPTS', 'Embedded Scripts', 'Module-script URLs injected into every SSR page', '/webllm-engine.js', 0, datetime('now'), datetime('now'))",
        "[]",
    )?;

    // 2b. Seed placeholders for auth block's required OAuth + domain config.
    //     gizza-ai runs anonymously — it doesn't use OAuth sign-in — but the
    //     auth block validator still requires these keys to be present. Empty
    //     strings are accepted as "not configured" by the auth block, which
    //     disables the corresponding flow.
    for (key, val) in [
        ("SUPPERS_AI__AUTH__ALLOWED_EMAIL_DOMAINS", ""),
        ("SUPPERS_AI__AUTH__OAUTH_GOOGLE_CLIENT_ID", ""),
        ("SUPPERS_AI__AUTH__OAUTH_GOOGLE_CLIENT_SECRET", ""),
        ("SUPPERS_AI__AUTH__OAUTH_GITHUB_CLIENT_ID", ""),
        ("SUPPERS_AI__AUTH__OAUTH_GITHUB_CLIENT_SECRET", ""),
        ("SUPPERS_AI__AUTH__OAUTH_FACEBOOK_CLIENT_ID", ""),
        ("SUPPERS_AI__AUTH__OAUTH_FACEBOOK_CLIENT_SECRET", ""),
    ] {
        let sql = format!(
            "INSERT OR IGNORE INTO variables (id, key, name, description, value, sensitive, created_at, updated_at)
             VALUES ('var_{}', '{}', '{}', 'Placeholder — gizza-ai runs anonymously', '{}', 0, datetime('now'), datetime('now'))",
            key.to_lowercase().replace("__", "_"),
            key,
            key,
            val
        );
        exec_or_err(&sql, "[]")?;
    }

    // 2c. Seed per-browser random secrets for auth-runtime keys.
    //     JWT_SECRET and INTERNAL_SECRET aren't declared as ConfigVars
    //     upstream (see auth_ui/mod.rs:124-130: "auth identity infra used by
    //     AuthServiceImpl, not UI"), so the generic `seed_auto_generated()`
    //     pass below would skip them. We seed them explicitly here with the
    //     same 32-byte-hex pattern. INSERT OR IGNORE means user-changed
    //     values stick.
    seed_random_secret(
        "var_auth_jwt_secret",
        "SUPPERS_AI__AUTH__JWT_SECRET",
        "JWT signing secret (gizza-local)",
    )?;
    seed_random_secret(
        "var_auth_internal_secret",
        "SUPPERS_AI__AUTH__INTERNAL_SECRET",
        "Internal-endpoint shared secret (gizza-local)",
    )?;

    // 3. Auto-generate secrets for config vars marked with auto_generate
    seed_auto_generated()?;

    // 4. Load all variables
    let json = query_or_err("SELECT key, value FROM variables", "[]")?;
    let rows: Vec<serde_json::Value> = serde_json::from_str(&json).map_err(|e| {
        JsValue::from_str(&format!("config: parse variables result failed: {e}"))
    })?;

    let mut vars = HashMap::new();
    for row in rows {
        let key = row.get("key").and_then(|v| v.as_str()).unwrap_or_default();
        let value = row
            .get("value")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if !key.is_empty() {
            vars.insert(key.to_string(), value.to_string());
        }
    }

    Ok(vars)
}

/// Sample 32 random bytes and return them hex-encoded. Used to seed
/// auth-runtime secrets that solobase's upstream config-var machinery
/// doesn't yet auto-generate. The `key` argument is for error labelling only.
#[cfg(target_arch = "wasm32")]
fn random_secret_hex(key: &str) -> Result<String, JsValue> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|e| JsValue::from_str(&format!("config: getrandom for {key}: {e}")))?;
    Ok(hex::encode(bytes))
}

/// Generate a per-browser random secret and `INSERT OR IGNORE` it into the
/// variables table. Sensitive bit is forced on.
#[cfg(target_arch = "wasm32")]
fn seed_random_secret(id: &str, key: &str, description: &str) -> Result<(), JsValue> {
    let secret = random_secret_hex(key)?;
    let params = serde_json::json!([id, key, key, description, secret]);
    exec_or_err(
        "INSERT OR IGNORE INTO variables (id, key, name, description, value, sensitive, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 1, datetime('now'), datetime('now'))",
        &params.to_string(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn seed_random_secret(_id: &str, _key: &str, _description: &str) -> Result<(), JsValue> {
    Ok(())
}

/// Auto-generate values for config vars marked with `auto_generate: true`.
///
/// Reads all block config var declarations, finds those needing auto-generation,
/// and generates random hex values for any that don't already exist in the
/// variables table.
fn seed_auto_generated() -> Result<(), JsValue> {
    let block_infos = solobase_core::blocks::all_block_infos();
    let all_vars = solobase_core::config_vars::collect_all_config_vars(&block_infos);

    for var in &all_vars {
        if !var.auto_generate {
            continue;
        }

        let secret = random_secret_hex(&var.key)?;

        let id = format!("var_{}", uuid::Uuid::new_v4());
        let sensitive: i32 = if var.is_sensitive() { 1 } else { 0 };

        // INSERT OR IGNORE — existing DB values take priority
        let params = serde_json::json!([
            id,
            var.key,
            var.name,
            var.description,
            secret,
            var.warning,
            sensitive
        ]);
        exec_or_err(
            "INSERT OR IGNORE INTO variables (id, key, name, description, value, warning, sensitive, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))",
            &params.to_string(),
        )?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Block settings
// ---------------------------------------------------------------------------

/// Load block settings from the browser database.
///
/// Reads the `suppers_ai__admin__block_settings` table (creating it if needed)
/// and returns a `BlockSettings` with the enabled/disabled state of each block.
pub fn load_block_settings() -> Result<solobase_core::features::BlockSettings, JsValue> {
    // Ensure table exists
    exec_or_err(
        "CREATE TABLE IF NOT EXISTS suppers_ai__admin__block_settings (
            block_name TEXT PRIMARY KEY,
            enabled INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
        "[]",
    )?;

    // Seed defaults for known blocks — gizza-ai only uses auth, llm,
    // local-llm, and messages. Everything else is explicitly disabled
    // so the feature block factory skips creation/registration.
    let defaults: &[(&str, bool)] = &[
        ("suppers-ai/auth", true),
        ("suppers-ai/llm", true),
        ("suppers-ai/local-llm", true),
        ("suppers-ai/messages", true),
        ("suppers-ai/admin", false),
        ("suppers-ai/files", false),
        ("suppers-ai/products", false),
        ("suppers-ai/projects", false),
        ("suppers-ai/legalpages", false),
        ("suppers-ai/userportal", false),
        ("suppers-ai/system", false),
        ("suppers-ai/vector", false),
    ];

    for &(name, default) in defaults {
        let params = serde_json::json!([name, default as i32]);
        exec_or_err(
            "INSERT OR IGNORE INTO suppers_ai__admin__block_settings (block_name, enabled) VALUES (?, ?)",
            &params.to_string(),
        )?;
    }

    // Read all settings
    let json = query_or_err(
        "SELECT block_name, enabled FROM suppers_ai__admin__block_settings",
        "[]",
    )?;
    let rows: Vec<serde_json::Value> = serde_json::from_str(&json).map_err(|e| {
        JsValue::from_str(&format!("config: parse block_settings result failed: {e}"))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let name = row
            .get("block_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let enabled = row.get("enabled").and_then(|v| v.as_i64()).unwrap_or(1) != 0;
        if !name.is_empty() {
            map.insert(name, enabled);
        }
    }

    Ok(solobase_core::features::BlockSettings::from_map(map))
}
