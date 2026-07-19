//! gizza-ai/env-to-configmap — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_env_to_configmap_core::to_manifest;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    env: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    namespace: String,
    #[serde(default)]
    secret_encoding: String,
    #[serde(default)]
    labels: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("env")
                .required()
                .describe("The .env file contents: KEY=value lines. '#' comments, blank lines and a leading 'export ' are ignored; quoted values are unquoted."),
        )
        .param(
            Param::enumv("kind", ["configmap", "secret"])
                .default("configmap")
                .describe("Output manifest: 'configmap' (default) for non-sensitive config, or 'secret' for confidential values."),
        )
        .param(
            Param::string("name")
                .default("app-config")
                .describe("metadata.name of the manifest (RFC 1123: lowercase letters, digits, '-' and '.'). Default 'app-config'."),
        )
        .param(
            Param::string("namespace")
                .default("")
                .describe("metadata.namespace. Blank (default) omits the field so it applies to the current namespace."),
        )
        .param(
            Param::enumv("secret_encoding", ["data", "stringData"])
                .default("data")
                .describe("For a Secret: 'data' (default) base64-encodes each value; 'stringData' keeps plaintext and lets Kubernetes encode on apply. Ignored for a ConfigMap."),
        )
        .param(
            Param::string("labels")
                .default("")
                .describe("Optional metadata.labels as comma-separated key=value pairs, e.g. 'app=web,tier=backend'. Blank (default) adds no labels."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/env-to-configmap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a .env file into a Kubernetes ConfigMap or Secret manifest",
    skill(
        description = "Convert a .env file (KEY=value lines; '#' comments, blank lines and a leading 'export ' are ignored, quoted values are unquoted) into a Kubernetes ConfigMap (default) or Secret YAML manifest. Set name/namespace/labels for metadata. Secret values are base64-encoded under 'data' by default, or kept plaintext under 'stringData'. Values that look like numbers/booleans are quoted so they stay strings. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "env-to-configmap", |a: Args| {
            to_manifest(&a.env, &a.kind, &a.name, &a.namespace, &a.secret_encoding, &a.labels)
                .map_err(SkillError::InvalidArgs)
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
                    "env": { "type": "string", "description": "The .env file contents: KEY=value lines. '#' comments, blank lines and a leading 'export ' are ignored; quoted values are unquoted." },
                    "kind": { "type": "string", "enum": ["configmap", "secret"], "default": "configmap", "description": "Output manifest: 'configmap' (default) for non-sensitive config, or 'secret' for confidential values." },
                    "name": { "type": "string", "default": "app-config", "description": "metadata.name of the manifest (RFC 1123: lowercase letters, digits, '-' and '.'). Default 'app-config'." },
                    "namespace": { "type": "string", "default": "", "description": "metadata.namespace. Blank (default) omits the field so it applies to the current namespace." },
                    "secret_encoding": { "type": "string", "enum": ["data", "stringData"], "default": "data", "description": "For a Secret: 'data' (default) base64-encodes each value; 'stringData' keeps plaintext and lets Kubernetes encode on apply. Ignored for a ConfigMap." },
                    "labels": { "type": "string", "default": "", "description": "Optional metadata.labels as comma-separated key=value pairs, e.g. 'app=web,tier=backend'. Blank (default) adds no labels." }
                },
                "required": ["env"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
