//! gizza-ai/generate-uuid — generate RFC 4122 / RFC 9562 UUIDs (v4 random, v7
//! time-ordered, v1 time+node, v5/v3 namespace), singly or in bulk. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure (getrandom CSPRNG + a SystemTime clock for v1/v7) → all
//! backends incl. the chat SW. Surfaces: chat + CLI + page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_generate_uuid_core::generate;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_version")]
    version: String,
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default)]
    namespace: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    uppercase: bool,
    #[serde(default = "default_true")]
    hyphens: bool,
    #[serde(default)]
    braces: bool,
    #[serde(default)]
    urn: bool,
}
fn default_version() -> String {
    "4".to_string()
}
fn default_count() -> u32 {
    1
}
fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct Resp {
    uuids: Vec<String>,
    version: String,
    count: usize,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("version", ["4", "7", "1", "5", "3", "nil", "max"])
                .default("4")
                .describe("UUID version: '4' (random, default), '7' (unix-time-ordered, sortable), '1' (gregorian-time + random node), '5' (namespace, SHA-1), '3' (namespace, MD5), 'nil' (all-zero), or 'max' (all-one)."),
        )
        .param(
            Param::integer("count")
                .min(1.0)
                .max(1000.0)
                .describe("How many UUIDs to generate (default 1). Ignored for nil/max (always 1) and for namespace versions when multiple names are given."),
        )
        .param(
            Param::string("namespace")
                .default("")
                .describe("For v5/v3 only: the namespace — 'dns', 'url', 'oid', 'x500', or a UUID string."),
        )
        .param(
            Param::string("name")
                .default("")
                .describe("For v5/v3 only: the name to hash within the namespace. Multiple names (comma- or newline-separated) yield one deterministic UUID each."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("Output hex in uppercase (default lowercase)."),
        )
        .param(
            Param::boolean("hyphens")
                .default(true)
                .describe("Include the 8-4-4-4-12 hyphens (default true). When false, output is 32 hex chars."),
        )
        .param(
            Param::boolean("braces")
                .default(false)
                .describe("Wrap each UUID in {curly braces} (Microsoft GUID registry form)."),
        )
        .param(
            Param::boolean("urn")
                .default(false)
                .describe("Prefix each UUID with 'urn:uuid:' (RFC 4122 URN form)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Unix epoch in milliseconds from the runtime clock (wafer provides the WASI
/// clock import; native on the CLI). Used only by v1/v7.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
struct GenerateUuid;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/generate-uuid",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate UUIDs (v4/v7/v1/v5/v3), singly or in bulk",
    skill(
        description = "Generate RFC 4122 / RFC 9562 UUIDs locally. version: '4' (random, default), '7' (unix-time-ordered and lexicographically sortable), '1' (gregorian-time + random node), '5' (namespace, SHA-1), '3' (namespace, MD5), 'nil', or 'max'. count returns a batch (1..1000). For v5/v3, set namespace ('dns', 'url', 'oid', 'x500', or a UUID) and name (comma/newline-separated names each yield one deterministic UUID). Format flags: uppercase, hyphens (default on), braces, urn. Returns the uuids array, the version, and the count.",
        parameters = schema_json()
    )
)]
impl GenerateUuid {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "generate-uuid", |a: Args| {
            let r = generate(
                &a.version,
                a.count as usize,
                now_ms(),
                &a.namespace,
                &a.name,
                a.uppercase,
                a.hyphens,
                a.braces,
                a.urn,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp {
                count: r.uuids.len(),
                uuids: r.uuids,
                version: r.version,
            })
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
                    "version":   { "type": "string", "enum": ["4", "7", "1", "5", "3", "nil", "max"], "default": "4", "description": "UUID version: '4' (random, default), '7' (unix-time-ordered, sortable), '1' (gregorian-time + random node), '5' (namespace, SHA-1), '3' (namespace, MD5), 'nil' (all-zero), or 'max' (all-one)." },
                    "count":     { "type": "integer", "minimum": 1, "maximum": 1000, "description": "How many UUIDs to generate (default 1). Ignored for nil/max (always 1) and for namespace versions when multiple names are given." },
                    "namespace": { "type": "string", "default": "", "description": "For v5/v3 only: the namespace — 'dns', 'url', 'oid', 'x500', or a UUID string." },
                    "name":      { "type": "string", "default": "", "description": "For v5/v3 only: the name to hash within the namespace. Multiple names (comma- or newline-separated) yield one deterministic UUID each." },
                    "uppercase": { "type": "boolean", "default": false, "description": "Output hex in uppercase (default lowercase)." },
                    "hyphens":   { "type": "boolean", "default": true, "description": "Include the 8-4-4-4-12 hyphens (default true). When false, output is 32 hex chars." },
                    "braces":    { "type": "boolean", "default": false, "description": "Wrap each UUID in {curly braces} (Microsoft GUID registry form)." },
                    "urn":       { "type": "boolean", "default": false, "description": "Prefix each UUID with 'urn:uuid:' (RFC 4122 URN form)." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
