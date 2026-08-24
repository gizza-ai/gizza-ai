//! Browser-facing wasm-bindgen wrapper for /tools/k8s-manifest-scaffold/.
//! Field order MUST match meta.toml: name, image, namespace, replicas,
//! container_port, service_port, service_type, node_port, image_pull_policy,
//! cpu_request, cpu_limit, memory_request, memory_limit, env, labels,
//! probe_path. Every field arrives as a string; a blank one means the field was
//! left empty → fall back to the schema default (or, for node_port, to "let
//! Kubernetes pick").
use gizza_ai_k8s_manifest_scaffold_core::scaffold;
use wasm_bindgen::prelude::*;

/// A blank field keeps the descriptor default; anything unparseable is an error
/// the page shows verbatim rather than silently using a different number.
fn parse_int(s: &str, field: &str, default: i64) -> Result<i64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("invalid {field} `{t}`: expected a whole number")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    name: &str,
    image: &str,
    namespace: &str,
    replicas: &str,
    container_port: &str,
    service_port: &str,
    service_type: &str,
    node_port: &str,
    image_pull_policy: &str,
    cpu_request: &str,
    cpu_limit: &str,
    memory_request: &str,
    memory_limit: &str,
    env: &str,
    labels: &str,
    probe_path: &str,
) -> Result<String, JsValue> {
    let node_port = if node_port.trim().is_empty() {
        None
    } else {
        Some(parse_int(node_port, "node_port", 0)?)
    };
    scaffold(
        name,
        image,
        namespace,
        parse_int(replicas, "replicas", 1)?,
        parse_int(container_port, "container_port", 8080)?,
        parse_int(service_port, "service_port", 80)?,
        service_type,
        node_port,
        image_pull_policy,
        cpu_request,
        cpu_limit,
        memory_request,
        memory_limit,
        env,
        labels,
        probe_path,
    )
    .map_err(|e| JsValue::from_str(&e))
}
