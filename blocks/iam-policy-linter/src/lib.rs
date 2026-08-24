//! gizza-ai/iam-policy-linter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    policy: String,
    #[serde(default = "default_policy_type")]
    policy_type: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default)]
    ignore: String,
}

fn default_policy_type() -> String {
    "identity".into()
}
fn default_format() -> String {
    "text".into()
}
fn default_min_severity() -> String {
    "low".into()
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("policy")
                .required()
                .describe("AWS IAM policy JSON to lint. Paste one identity policy, resource policy, trust policy, or service control policy document. JSON only; maximum 200000 characters."),
        )
        .param(
            Param::enumv("policy_type", ["identity", "resource", "trust", "scp"])
                .default("identity")
                .describe("How AWS will attach this document: identity (user/group/role permissions, no Principal), resource (bucket/key/queue-style policy, Principal required), trust (role AssumeRolePolicyDocument, Principal required and Resource forbidden), or scp (Organizations service control policy rules). Default identity."),
        )
        .param(
            Param::enumv("format", ["text", "json", "csv"])
                .default("text")
                .describe("Output format: text for a readable verdict and findings, json for CI or automation, or csv for spreadsheet/ticket exports. Default text."),
        )
        .param(
            Param::enumv("min_severity", ["low", "medium", "high"])
                .default("low")
                .describe("Display threshold for findings. The verdict still counts all non-ignored findings; this only hides lower-severity rows from the rendered output. Default low."),
        )
        .param(
            Param::string("ignore")
                .default("")
                .describe("Comma-separated rule codes to suppress from the report and verdict after review, e.g. MISSING-VERSION,RESOURCE-STAR. Unknown codes are errors so typos cannot silently hide nothing."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/iam-policy-linter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Lint AWS IAM policy JSON for risky wildcards and policy syntax issues",
    skill(
        description = "Validate an AWS IAM policy JSON document locally and flag grammar errors plus risky permissions such as Action/Resource wildcards, Allow + NotAction/NotResource/NotPrincipal, public principals, iam:PassRole on Resource *, and sensitive action families on unconstrained resources. Choose identity, resource, trust or SCP policy semantics; render text, JSON or CSV; filter display by severity; and suppress reviewed rule codes explicitly.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "iam-policy-linter", |a: Args| {
            gizza_ai_iam_policy_linter_core::render(
                &a.policy,
                &a.policy_type,
                &a.format,
                &a.min_severity,
                &a.ignore,
            )
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
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(schema["required"], serde_json::json!(["policy"]));
        assert_eq!(
            schema["properties"]["policy_type"]["enum"],
            serde_json::json!(["identity", "resource", "trust", "scp"])
        );
        assert_eq!(
            schema["properties"]["format"]["enum"],
            serde_json::json!(["text", "json", "csv"])
        );
        assert_eq!(
            schema["properties"]["min_severity"]["enum"],
            serde_json::json!(["low", "medium", "high"])
        );
        assert_eq!(schema["properties"]["policy_type"]["default"], "identity");
        assert_eq!(schema["properties"]["ignore"]["default"], "");
        for name in ["policy", "policy_type", "format", "min_severity", "ignore"] {
            assert!(
                schema["properties"][name]["description"]
                    .as_str()
                    .unwrap()
                    .len()
                    > 20
            );
        }
    }
}
