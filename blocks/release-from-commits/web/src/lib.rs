//! Browser-facing wasm-bindgen wrapper for /tools/release-from-commits/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    current_version: &str,
    commits: &str,
    minor_types: &str,
    patch_types: &str,
    zero_version_policy: &str,
    prerelease_policy: &str,
    prerelease_identifier: &str,
    hidden_types: &str,
    repo_url: &str,
    release_date: &str,
    output_format: &str,
) -> Result<String, JsValue> {
    gizza_ai_release_from_commits_core::run(
        current_version,
        commits,
        minor_types,
        patch_types,
        zero_version_policy,
        prerelease_policy,
        prerelease_identifier,
        hidden_types,
        repo_url,
        release_date,
        output_format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
