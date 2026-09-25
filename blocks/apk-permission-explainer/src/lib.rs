//! gizza-ai/apk-permission-explainer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_mode() -> String {
    "report".into()
}
fn default_risk() -> String {
    "all".into()
}
fn default_sort() -> String {
    "risk".into()
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_risk")]
    risk: String,
    #[serde(default = "default_sort")]
    sort: String,
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().multiline().describe("APK bytes as Base64, a Base64 AndroidManifest.xml, or a decoded AndroidManifest.xml pasted as text."))
        .param(Param::enumv("mode", ["report", "list", "csv", "json"]).default("report").describe("Output format. report gives a readable Markdown audit, list is compact one-line-per-permission text, csv is spreadsheet-friendly, and json is structured data."))
        .param(Param::enumv("risk", ["all", "risky", "dangerous", "privacy-sensitive", "signature", "normal", "unknown"]).default("all").describe("Filter permissions by risk bucket. risky keeps dangerous, privacy-sensitive, and signature/system permissions."))
        .param(Param::enumv("sort", ["risk", "name"]).default("risk").describe("Sort permissions by severity then name, or alphabetically by permission name."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/apk-permission-explainer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Explain APK permissions from an AndroidManifest.xml with risk categories and plain-English descriptions.",
    skill(
        description = "Decode an APK or AndroidManifest.xml, extract requested Android permissions, and explain each permission in plain English with a risk category. Paste APK bytes as Base64, a Base64 manifest, or a decoded manifest. Returns a readable report by default, with list, CSV, and JSON output modes plus risk filtering and sorting.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "apk-permission-explainer", |a: Args| {
            gizza_ai_apk_permission_explainer_core::run(&a.input, &a.mode, &a.risk, &a.sort)
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
          "type":"object","properties":{
            "input":{"type":"string","description":"APK bytes as Base64, a Base64 AndroidManifest.xml, or a decoded AndroidManifest.xml pasted as text."},
            "mode":{"type":"string","enum":["report","list","csv","json"],"default":"report","description":"Output format. report gives a readable Markdown audit, list is compact one-line-per-permission text, csv is spreadsheet-friendly, and json is structured data."},
            "risk":{"type":"string","enum":["all","risky","dangerous","privacy-sensitive","signature","normal","unknown"],"default":"all","description":"Filter permissions by risk bucket. risky keeps dangerous, privacy-sensitive, and signature/system permissions."},
            "sort":{"type":"string","enum":["risk","name"],"default":"risk","description":"Sort permissions by severity then name, or alphabetically by permission name."}
          },"required":["input"],"additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
