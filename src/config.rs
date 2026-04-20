//! Browser-side variable seeding and loading.
//!
//! Mirrors the native `seed_and_load_variables()` / `load_block_settings()` from
//! `solobase/src/main.rs`, but uses the JS bridge (`bridge::db_exec_raw` /
//! `bridge::db_query_raw`) instead of `rusqlite`.

use std::collections::HashMap;

use solobase_browser::bridge;

// ---------------------------------------------------------------------------
// Variable seeding and loading
// ---------------------------------------------------------------------------

/// Ensure the variables table exists, auto-generate secrets for config vars
/// marked `auto_generate: true`, and return all variables from the DB.
///
/// This is the browser equivalent of the native `seed_and_load_variables()`.
/// There are no env vars to seed from in the browser — only auto-generated
/// secrets and previously-stored values.
pub fn seed_and_load_variables() -> HashMap<String, String> {
    // 1. Create variables table if it does not exist
    bridge::db_exec_raw(
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
    );
    bridge::db_exec_raw(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_variables_key ON variables (key)",
        "[]",
    );

    // 2. Seed default admin account for browser build.
    //    Email: admin@solobase.local / Password: admin
    //    This is local-only (OPFS) so a simple default is acceptable.
    bridge::db_exec_raw(
        "INSERT OR IGNORE INTO variables (id, key, name, description, value, sensitive, created_at, updated_at)
         VALUES ('var_admin_email', 'SUPPERS_AI__AUTH__ADMIN_EMAIL', 'Admin Email', 'Admin account email', 'admin@solobase.local', 0, datetime('now'), datetime('now'))",
        "[]",
    );
    bridge::db_exec_raw(
        "INSERT OR IGNORE INTO variables (id, key, name, description, value, sensitive, created_at, updated_at)
         VALUES ('var_admin_pass', 'SUPPERS_AI__AUTH__ADMIN_PASSWORD', 'Admin Password', 'Admin account password', 'admin', 1, datetime('now'), datetime('now'))",
        "[]",
    );

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
        bridge::db_exec_raw(&sql, "[]");
    }

    // 3. Auto-generate secrets for config vars marked with auto_generate
    seed_auto_generated();

    // 3. Load all variables
    let json = bridge::db_query_raw("SELECT key, value FROM variables", "[]");
    let rows: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap_or_default();

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

    vars
}

/// Auto-generate values for config vars marked with `auto_generate: true`.
///
/// Reads all block config var declarations, finds those needing auto-generation,
/// and generates random hex values for any that don't already exist in the
/// variables table.
fn seed_auto_generated() {
    let block_infos = solobase_core::blocks::all_block_infos();
    let all_vars = solobase_core::config_vars::collect_all_config_vars(&block_infos);

    for var in &all_vars {
        if !var.auto_generate {
            continue;
        }

        // Generate a random 32-byte hex secret
        let mut bytes = [0u8; 32];
        if let Err(e) = getrandom::getrandom(&mut bytes) {
            web_sys::console::warn_1(
                &format!("solobase: getrandom failed for {}: {e}", var.key).into(),
            );
            continue;
        }
        let secret: String = bytes.iter().map(|b| format!("{b:02x}")).collect();

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
        bridge::db_exec_raw(
            "INSERT OR IGNORE INTO variables (id, key, name, description, value, warning, sensitive, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))",
            &params.to_string(),
        );
    }
}

// ---------------------------------------------------------------------------
// Block settings
// ---------------------------------------------------------------------------

/// Load block settings from the browser database.
///
/// Reads the `suppers_ai__admin__block_settings` table (creating it if needed)
/// and returns a `BlockSettings` with the enabled/disabled state of each block.
pub fn load_block_settings() -> solobase_core::features::BlockSettings {
    // Ensure table exists
    bridge::db_exec_raw(
        "CREATE TABLE IF NOT EXISTS suppers_ai__admin__block_settings (
            block_name TEXT PRIMARY KEY,
            enabled INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
        "[]",
    );

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
        bridge::db_exec_raw(
            "INSERT OR IGNORE INTO suppers_ai__admin__block_settings (block_name, enabled) VALUES (?, ?)",
            &params.to_string(),
        );
    }

    // Read all settings
    let json = bridge::db_query_raw(
        "SELECT block_name, enabled FROM suppers_ai__admin__block_settings",
        "[]",
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap_or_default();

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

    solobase_core::features::BlockSettings::from_map(map)
}
