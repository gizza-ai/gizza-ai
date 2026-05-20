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

/// Serialize builder-emitted parameter values for the JS bridge.
///
/// `wafer_sql_utils` builders return a `Statement` whose `values` field holds
/// `Vec<SeaValue>`. The JS bridge expects a JSON-encoded array.  This helper
/// converts and serializes in one place, factoring the pattern that would
/// otherwise repeat at every callsite.
///
/// Note: `exec_or_err` / `query_or_err` are kept as-is — they wrap
/// `bridge::db_exec_raw` / `bridge::db_query_raw` (the only execute primitive
/// available at bootstrap, before the wafer runtime exists).
#[cfg(target_arch = "wasm32")]
fn json_params(values: Vec<wafer_sql_utils::SeaValue>) -> Result<String, JsValue> {
    let json_vals = wafer_sql_utils::value::sea_values_to_json(values);
    serde_json::to_string(&json_vals)
        .map_err(|e| JsValue::from_str(&format!("config: serialize params failed: {e:?}")))
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
    // 1. Create variables table if it does not exist.
    //
    // gizza-ai bootstrap: variables table is wafer-run-shared; schema is
    // defined here as a hand-written string because `ddl::build_create_table`
    // requires a `Table` struct — coupling gizza-ai to wafer-run's schema
    // definition machinery for a one-off bootstrap DDL trades more code for
    // less clarity.  The equivalent index is similarly hand-written.
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

    // Pre-compute a timestamp for all seeds in this call.  The original SQL
    // used `datetime('now')` (a SQLite function evaluated server-side), but
    // `wafer_sql_utils` builders bind values as parameters and cannot carry
    // raw SQL expressions.  A pre-computed RFC-3339 string is functionally
    // equivalent for bootstrap seeding.
    let now = chrono::Utc::now().to_rfc3339();

    // 2. Seed default admin account for browser build.
    //    Email: admin@solobase.local / Password: admin
    //    This is local-only (OPFS) so a simple default is acceptable.
    {
        use wafer_sql_utils::{upsert, Backend};
        let stmt = upsert::build_upsert(
            "variables",
            &[
                ("id".into(), serde_json::json!("var_admin_email")),
                (
                    "key".into(),
                    serde_json::json!("SUPPERS_AI__AUTH__ADMIN_EMAIL"),
                ),
                ("name".into(), serde_json::json!("Admin Email")),
                (
                    "description".into(),
                    serde_json::json!("Admin account email"),
                ),
                ("value".into(), serde_json::json!("admin@solobase.local")),
                ("sensitive".into(), serde_json::json!(0)),
                ("created_at".into(), serde_json::json!(now.clone())),
                ("updated_at".into(), serde_json::json!(now.clone())),
            ],
            &["key"],
            &[], // insert-or-ignore: empty update columns
            Backend::Sqlite,
        );
        exec_or_err(&stmt.sql, &json_params(stmt.values)?)?;
    }
    {
        use wafer_sql_utils::{upsert, Backend};
        let stmt = upsert::build_upsert(
            "variables",
            &[
                ("id".into(), serde_json::json!("var_admin_pass")),
                (
                    "key".into(),
                    serde_json::json!("SUPPERS_AI__AUTH__ADMIN_PASSWORD"),
                ),
                ("name".into(), serde_json::json!("Admin Password")),
                (
                    "description".into(),
                    serde_json::json!("Admin account password"),
                ),
                ("value".into(), serde_json::json!("admin")),
                ("sensitive".into(), serde_json::json!(1)),
                ("created_at".into(), serde_json::json!(now.clone())),
                ("updated_at".into(), serde_json::json!(now.clone())),
            ],
            &["key"],
            &[], // insert-or-ignore: empty update columns
            Backend::Sqlite,
        );
        exec_or_err(&stmt.sql, &json_params(stmt.values)?)?;
    }

    // Inject the framework's page-side WebLLM engine into every SSR-rendered
    // page served via solobase-core's layout (admin UI, etc.). gizza's own
    // /b/ui page has its own maud template that loads /webllm-engine.js
    // directly, so this seed is strictly for framework-rendered surfaces.
    {
        use wafer_sql_utils::{upsert, Backend};
        let stmt = upsert::build_upsert(
            "variables",
            &[
                ("id".into(), serde_json::json!("var_embedded_scripts")),
                (
                    "key".into(),
                    serde_json::json!("SOLOBASE_SHARED__EMBEDDED_SCRIPTS"),
                ),
                ("name".into(), serde_json::json!("Embedded Scripts")),
                (
                    "description".into(),
                    serde_json::json!(
                        "Module-script URLs injected into every SSR page"
                    ),
                ),
                ("value".into(), serde_json::json!("/webllm-engine.js")),
                ("sensitive".into(), serde_json::json!(0)),
                ("created_at".into(), serde_json::json!(now.clone())),
                ("updated_at".into(), serde_json::json!(now.clone())),
            ],
            &["key"],
            &[], // insert-or-ignore: empty update columns
            Backend::Sqlite,
        );
        exec_or_err(&stmt.sql, &json_params(stmt.values)?)?;
    }

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
        use wafer_sql_utils::{upsert, Backend};
        let id = format!("var_{}", key.to_lowercase().replace("__", "_"));
        let stmt = upsert::build_upsert(
            "variables",
            &[
                ("id".into(), serde_json::json!(id)),
                ("key".into(), serde_json::json!(key)),
                ("name".into(), serde_json::json!(key)),
                (
                    "description".into(),
                    serde_json::json!("Placeholder — gizza-ai runs anonymously"),
                ),
                ("value".into(), serde_json::json!(val)),
                ("sensitive".into(), serde_json::json!(0)),
                ("created_at".into(), serde_json::json!(now.clone())),
                ("updated_at".into(), serde_json::json!(now.clone())),
            ],
            &["key"],
            &[], // insert-or-ignore: empty update columns
            Backend::Sqlite,
        );
        exec_or_err(&stmt.sql, &json_params(stmt.values)?)?;
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
        &now,
    )?;
    seed_random_secret(
        "var_auth_internal_secret",
        "SUPPERS_AI__AUTH__INTERNAL_SECRET",
        "Internal-endpoint shared secret (gizza-local)",
        &now,
    )?;

    // 3. Auto-generate secrets for config vars marked with auto_generate
    seed_auto_generated(&now)?;

    // 4. Load all variables
    {
        use wafer_block::db::ListOptions;
        use wafer_sql_utils::{query, Backend};
        let stmt = query::build_select_columns(
            "variables",
            &["key", "value"],
            &ListOptions::default(),
            None,
            Backend::Sqlite,
        );
        let json = query_or_err(&stmt.sql, &json_params(stmt.values)?)?;
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
///
/// `now` is the RFC-3339 timestamp pre-computed by the caller — passed in
/// so all seeds within a boot share the same `created_at` / `updated_at`.
#[cfg(target_arch = "wasm32")]
fn seed_random_secret(
    id: &str,
    key: &str,
    description: &str,
    now: &str,
) -> Result<(), JsValue> {
    use wafer_sql_utils::{upsert, Backend};
    let secret = random_secret_hex(key)?;
    let stmt = upsert::build_upsert(
        "variables",
        &[
            ("id".into(), serde_json::json!(id)),
            ("key".into(), serde_json::json!(key)),
            ("name".into(), serde_json::json!(key)),
            ("description".into(), serde_json::json!(description)),
            ("value".into(), serde_json::json!(secret)),
            ("sensitive".into(), serde_json::json!(1)),
            ("created_at".into(), serde_json::json!(now)),
            ("updated_at".into(), serde_json::json!(now)),
        ],
        &["key"],
        &[], // insert-or-ignore: empty update columns
        Backend::Sqlite,
    );
    exec_or_err(&stmt.sql, &json_params(stmt.values)?)
}

#[cfg(not(target_arch = "wasm32"))]
fn seed_random_secret(
    _id: &str,
    _key: &str,
    _description: &str,
    _now: &str,
) -> Result<(), JsValue> {
    Ok(())
}

/// Auto-generate values for config vars marked with `auto_generate: true`.
///
/// Reads all block config var declarations, finds those needing auto-generation,
/// and generates random hex values for any that don't already exist in the
/// variables table.
fn seed_auto_generated(now: &str) -> Result<(), JsValue> {
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
        {
            use wafer_sql_utils::{upsert, Backend};
            let stmt = upsert::build_upsert(
                "variables",
                &[
                    ("id".into(), serde_json::json!(id)),
                    ("key".into(), serde_json::json!(var.key)),
                    ("name".into(), serde_json::json!(var.name)),
                    ("description".into(), serde_json::json!(var.description)),
                    ("value".into(), serde_json::json!(secret)),
                    ("warning".into(), serde_json::json!(var.warning)),
                    ("sensitive".into(), serde_json::json!(sensitive)),
                    ("created_at".into(), serde_json::json!(now)),
                    ("updated_at".into(), serde_json::json!(now)),
                ],
                &["key"],
                &[], // insert-or-ignore: empty update columns
                Backend::Sqlite,
            );
            exec_or_err(&stmt.sql, &json_params(stmt.values)?)?;
        }
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
    // PRECONDITION: `wafer.init_block(suppers-ai/admin)` must have already
    // run before this function is called — admin's migration is the single
    // source of schema truth for `suppers_ai__admin__block_settings`. See
    // `initialize()` in `lib.rs` for the ordering. Removing the pre-create
    // eliminates the schema-drift class of bug that took down the solobase
    // demo in solobase #210/#211 (mirrored in #212 for solobase-web; this
    // is the matching gizza-ai change).
    //
    // One-shot migration for legacy OPFS state: users who visited a build
    // before this restructure have a stale 4-column `block_settings` table
    // that admin's `CREATE TABLE IF NOT EXISTS` no-op'd against, leaving
    // the `id`/`current_hash`/`blessed_hash` columns missing. Detect that
    // case via `pragma_table_info` and recreate the table with the
    // canonical schema. block_settings rows are runtime config (enabled
    // flags + per-block migration hashes), not user data — losing them on
    // this one-shot migration is acceptable; defaults below re-seed.
    //
    // gizza-ai bootstrap DDL: the PRAGMA introspection query, DROP TABLE,
    // CREATE TABLE, and CREATE INDEX below are kept as hand-written strings.
    // These are one-shot migration/schema statements; using the DDL builders
    // (`ddl::build_create_table` etc.) would require defining Table structs
    // for a schema owned by solobase-core, coupling gizza-ai to that
    // definition for no practical benefit at bootstrap time.
    let table_info = query_or_err(
        "SELECT name FROM pragma_table_info('suppers_ai__admin__block_settings')",
        "[]",
    )
    .unwrap_or_else(|_| "[]".to_string());
    let has_id_column = table_info.contains("\"id\"");
    let table_exists = table_info != "[]" && !table_info.is_empty();
    if table_exists && !has_id_column {
        exec_or_err(
            "DROP TABLE IF EXISTS suppers_ai__admin__block_settings",
            "[]",
        )?;
        exec_or_err(
            "CREATE TABLE suppers_ai__admin__block_settings (
                id            TEXT PRIMARY KEY,
                block_name    TEXT NOT NULL UNIQUE,
                enabled       INTEGER NOT NULL DEFAULT 1,
                current_hash  TEXT NOT NULL DEFAULT '',
                blessed_hash  TEXT NOT NULL DEFAULT '',
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL
            )",
            "[]",
        )?;
        exec_or_err(
            "CREATE UNIQUE INDEX IF NOT EXISTS suppers_ai__admin__block_settings_block_name_uniq \
             ON suppers_ai__admin__block_settings (block_name)",
            "[]",
        )?;
    }

    // Seed defaults for known blocks — gizza-ai only uses auth, llm,
    // local-llm, and messages. Everything else is explicitly disabled
    // so the feature block factory skips creation/registration. The strict
    // schema demands `id`, `created_at`, `updated_at`; generate them
    // inline via SQLite builtins so the seed remains a one-shot SQL
    // statement per block.
    //
    // This INSERT keeps `lower(hex(randomblob(16)))` as a hand-written
    // SQL expression because that SQLite function generates the row id
    // server-side; it cannot be represented as a parameter-bound value
    // in the wafer_sql_utils builder API.
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
            "INSERT OR IGNORE INTO suppers_ai__admin__block_settings \
             (id, block_name, enabled, created_at, updated_at) \
             VALUES (lower(hex(randomblob(16))), ?, ?, datetime('now'), datetime('now'))",
            &params.to_string(),
        )?;
    }

    // Read all settings
    {
        use wafer_block::db::ListOptions;
        use wafer_sql_utils::{query, Backend};
        let stmt = query::build_select_columns(
            "suppers_ai__admin__block_settings",
            &["block_name", "enabled"],
            &ListOptions::default(),
            None,
            Backend::Sqlite,
        );
        let json = query_or_err(&stmt.sql, &json_params(stmt.values)?)?;
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
}
