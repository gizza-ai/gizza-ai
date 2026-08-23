//! Browser-facing wasm-bindgen wrapper for /tools/docker-compose-generator/.
//! Field order MUST match meta.toml: services, project_name, compose_version,
//! network, network_driver, restart, env, env_file. Every field arrives as a
//! string; a blank one means the field was left empty → fall back to the
//! schema default.
use gizza_ai_docker_compose_generator_core::generate;
use wasm_bindgen::prelude::*;

/// A blank enum field keeps the descriptor default rather than erroring.
fn or_default<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.trim().is_empty() {
        default
    } else {
        value
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    services: &str,
    project_name: &str,
    compose_version: &str,
    network: &str,
    network_driver: &str,
    restart: &str,
    env: &str,
    env_file: &str,
) -> Result<String, JsValue> {
    generate(
        services,
        project_name,
        or_default(compose_version, "none"),
        network,
        or_default(network_driver, "bridge"),
        or_default(restart, "none"),
        env,
        env_file,
    )
    .map_err(|e| JsValue::from_str(&e))
}
