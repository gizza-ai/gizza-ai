//! gizza-ai/k8s-manifest-scaffold — build a Kubernetes Deployment + Service
//! YAML manifest from a handful of app settings. Chat schema single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_k8s_manifest_scaffold_core::scaffold;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    name: String,
    image: String,
    #[serde(default)]
    namespace: String,
    #[serde(default = "default_replicas")]
    replicas: i64,
    #[serde(default = "default_container_port")]
    container_port: i64,
    #[serde(default = "default_service_port")]
    service_port: i64,
    #[serde(default)]
    service_type: String,
    #[serde(default)]
    node_port: Option<i64>,
    #[serde(default)]
    image_pull_policy: String,
    #[serde(default)]
    cpu_request: String,
    #[serde(default)]
    cpu_limit: String,
    #[serde(default)]
    memory_request: String,
    #[serde(default)]
    memory_limit: String,
    #[serde(default)]
    env: String,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    probe_path: String,
}

fn default_replicas() -> i64 {
    1
}
fn default_container_port() -> i64 {
    8080
}
fn default_service_port() -> i64 {
    80
}

fn build(a: &Args) -> Result<String, String> {
    scaffold(
        &a.name,
        &a.image,
        &a.namespace,
        a.replicas,
        a.container_port,
        a.service_port,
        &a.service_type,
        a.node_port,
        &a.image_pull_policy,
        &a.cpu_request,
        &a.cpu_limit,
        &a.memory_request,
        &a.memory_limit,
        &a.env,
        &a.labels,
        &a.probe_path,
    )
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("name")
                .required()
                .describe("Name for both the Deployment and the Service, also used as the 'app' selector label, e.g. \"web\". Lowercase letters, digits, '-' and '.', max 63 characters (RFC 1123)."),
        )
        .param(
            Param::string("image")
                .required()
                .describe("Container image reference, e.g. \"nginx:1.27\" or \"ghcr.io/acme/api:v1.2.3\"."),
        )
        .param(
            Param::string("namespace")
                .describe("metadata.namespace for both resources, e.g. \"prod\". Omitted from the manifest when blank."),
        )
        .param(
            Param::integer("replicas")
                .min(0.0)
                .max(100.0)
                .default(1)
                .describe("Deployment replica count, 0-100 (0 scales down to no pods). Default 1."),
        )
        .param(
            Param::integer("container_port")
                .min(1.0)
                .max(65535.0)
                .default(8080)
                .describe("Port the container listens on (containerPort), 1-65535. Also the Service targetPort and the probe port. Default 8080."),
        )
        .param(
            Param::integer("service_port")
                .min(1.0)
                .max(65535.0)
                .default(80)
                .describe("Port the Service exposes, 1-65535. Default 80."),
        )
        .param(
            Param::enumv("service_type", ["ClusterIP", "NodePort", "LoadBalancer"])
                .default("ClusterIP")
                .describe("How the Service is exposed: 'ClusterIP' (in-cluster only, default), 'NodePort' (a port on every node) or 'LoadBalancer' (a cloud load balancer)."),
        )
        .param(
            Param::integer("node_port")
                .min(30000.0)
                .max(32767.0)
                .describe("Fixed nodePort, 30000-32767. Only valid when service_type is 'NodePort'; leave unset to let Kubernetes pick one."),
        )
        .param(
            Param::enumv("image_pull_policy", ["IfNotPresent", "Always", "Never"])
                .default("IfNotPresent")
                .describe("When the kubelet pulls the image: 'IfNotPresent' (default), 'Always' or 'Never'."),
        )
        .param(
            Param::string("cpu_request")
                .describe("CPU request as a Kubernetes quantity, e.g. \"100m\", \"0.5\" or \"2\". Omitted when blank."),
        )
        .param(
            Param::string("cpu_limit")
                .describe("CPU limit as a Kubernetes quantity, e.g. \"500m\". Omitted when blank."),
        )
        .param(
            Param::string("memory_request")
                .describe("Memory request as a Kubernetes quantity, e.g. \"128Mi\", \"512M\" or \"1Gi\". Omitted when blank."),
        )
        .param(
            Param::string("memory_limit")
                .describe("Memory limit as a Kubernetes quantity, e.g. \"256Mi\". Omitted when blank. The whole resources: block is dropped when all four resource fields are blank."),
        )
        .param(
            Param::string("env")
                .describe("Container environment as KEY=value lines, one per line, e.g. \"LOG_LEVEL=info\\nPORT=8000\". Blank and '#' comment lines are skipped, a leading 'export ' is dropped, quoted values are unquoted, and every value is emitted as a quoted string."),
        )
        .param(
            Param::string("labels")
                .describe("Extra key=value labels, newline- or comma-separated, e.g. \"tier=backend,team=payments\". Merged after the standard 'app' label on both resources and the pod template; the key 'app' is reserved."),
        )
        .param(
            Param::string("probe_path")
                .describe("HTTP path for a liveness and readiness probe on the container port, e.g. \"/healthz\". Blank emits neither probe."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/k8s-manifest-scaffold",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a Kubernetes Deployment + Service YAML manifest",
    skill(
        description = "Generate a ready-to-apply Kubernetes manifest — a Deployment and a matching Service as two YAML documents separated by '---' — from a name and an image. replicas (0-100, default 1), container_port (default 8080) and service_port (default 80) set the pod and Service ports, with the Service targetPort wired to the container port; service_type picks ClusterIP (default), NodePort or LoadBalancer, and node_port (30000-32767) pins a nodePort for NodePort Services. Optional namespace, image_pull_policy (IfNotPresent/Always/Never), cpu_request/cpu_limit/memory_request/memory_limit quantities, env as KEY=value lines, extra key=value labels, and probe_path to add matching liveness and readiness HTTP probes. Names, namespaces, ports, quantities, env names, labels and the probe path are all validated, ambiguous values are quoted so YAML keeps them as strings, and the same inputs always produce the same bytes. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "k8s-manifest-scaffold", |a: Args| {
            build(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name for both the Deployment and the Service, also used as the 'app' selector label, e.g. \"web\". Lowercase letters, digits, '-' and '.', max 63 characters (RFC 1123)." },
                    "image": { "type": "string", "description": "Container image reference, e.g. \"nginx:1.27\" or \"ghcr.io/acme/api:v1.2.3\"." },
                    "namespace": { "type": "string", "description": "metadata.namespace for both resources, e.g. \"prod\". Omitted from the manifest when blank." },
                    "replicas": { "type": "integer", "minimum": 0, "maximum": 100, "default": 1, "description": "Deployment replica count, 0-100 (0 scales down to no pods). Default 1." },
                    "container_port": { "type": "integer", "minimum": 1, "maximum": 65535, "default": 8080, "description": "Port the container listens on (containerPort), 1-65535. Also the Service targetPort and the probe port. Default 8080." },
                    "service_port": { "type": "integer", "minimum": 1, "maximum": 65535, "default": 80, "description": "Port the Service exposes, 1-65535. Default 80." },
                    "service_type": { "type": "string", "enum": ["ClusterIP", "NodePort", "LoadBalancer"], "default": "ClusterIP", "description": "How the Service is exposed: 'ClusterIP' (in-cluster only, default), 'NodePort' (a port on every node) or 'LoadBalancer' (a cloud load balancer)." },
                    "node_port": { "type": "integer", "minimum": 30000, "maximum": 32767, "description": "Fixed nodePort, 30000-32767. Only valid when service_type is 'NodePort'; leave unset to let Kubernetes pick one." },
                    "image_pull_policy": { "type": "string", "enum": ["IfNotPresent", "Always", "Never"], "default": "IfNotPresent", "description": "When the kubelet pulls the image: 'IfNotPresent' (default), 'Always' or 'Never'." },
                    "cpu_request": { "type": "string", "description": "CPU request as a Kubernetes quantity, e.g. \"100m\", \"0.5\" or \"2\". Omitted when blank." },
                    "cpu_limit": { "type": "string", "description": "CPU limit as a Kubernetes quantity, e.g. \"500m\". Omitted when blank." },
                    "memory_request": { "type": "string", "description": "Memory request as a Kubernetes quantity, e.g. \"128Mi\", \"512M\" or \"1Gi\". Omitted when blank." },
                    "memory_limit": { "type": "string", "description": "Memory limit as a Kubernetes quantity, e.g. \"256Mi\". Omitted when blank. The whole resources: block is dropped when all four resource fields are blank." },
                    "env": { "type": "string", "description": "Container environment as KEY=value lines, one per line, e.g. \"LOG_LEVEL=info\\nPORT=8000\". Blank and '#' comment lines are skipped, a leading 'export ' is dropped, quoted values are unquoted, and every value is emitted as a quoted string." },
                    "labels": { "type": "string", "description": "Extra key=value labels, newline- or comma-separated, e.g. \"tier=backend,team=payments\". Merged after the standard 'app' label on both resources and the pod template; the key 'app' is reserved." },
                    "probe_path": { "type": "string", "description": "HTTP path for a liveness and readiness probe on the container port, e.g. \"/healthz\". Blank emits neither probe." }
                },
                "required": ["name", "image"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
