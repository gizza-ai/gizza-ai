//! gizza-ai/release-from-commits — compute the next semantic version and grouped
//! release notes from a pasted Conventional Commits log.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    current_version: String,
    commits: String,
    #[serde(default = "default_minor_types")]
    minor_types: String,
    #[serde(default = "default_patch_types")]
    patch_types: String,
    #[serde(default = "default_zero_version_policy")]
    zero_version_policy: String,
    #[serde(default = "default_prerelease_policy")]
    prerelease_policy: String,
    #[serde(default = "default_prerelease_identifier")]
    prerelease_identifier: String,
    #[serde(default = "default_hidden_types")]
    hidden_types: String,
    #[serde(default)]
    repo_url: String,
    #[serde(default)]
    release_date: String,
    #[serde(default = "default_output_format")]
    output_format: String,
}

fn default_minor_types() -> String {
    "feat,feature".to_string()
}
fn default_patch_types() -> String {
    "fix,perf,revert".to_string()
}
fn default_zero_version_policy() -> String {
    "standard".to_string()
}
fn default_prerelease_policy() -> String {
    "finalize".to_string()
}
fn default_prerelease_identifier() -> String {
    "rc".to_string()
}
fn default_hidden_types() -> String {
    "chore,style,ci,build,test".to_string()
}
fn default_output_format() -> String {
    "markdown".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("current_version").required().describe("Current semantic version or tag prefix plus version, such as 1.4.2, v1.4.2, or web-v0.8.0. The same prefix is preserved on the next version."))
        .param(Param::string("commits").required().multiline().describe("Pasted Conventional Commits log to analyse. Accepts git log --oneline lines, full commit messages, or bullet lists. Supports type(scope)!: subject plus BREAKING CHANGE footers."))
        .param(Param::string("minor_types").default("feat,feature").describe("Comma-separated commit types that trigger a minor bump. Default feat,feature."))
        .param(Param::string("patch_types").default("fix,perf,revert").describe("Comma-separated commit types that trigger a patch bump. Use * to make every non-breaking commit at least a patch. Default fix,perf,revert."))
        .param(Param::enumv("zero_version_policy", ["standard", "cautious"]).default("standard").describe("How 0.x versions handle bumps: standard treats breaking as 1.0.0 and features as 0.(minor+1).0; cautious downgrades breaking to minor and features to patch. Default standard."))
        .param(Param::enumv("prerelease_policy", ["finalize", "increment", "ignore"]).default("finalize").describe("How to handle an existing pre-release version: finalize removes the suffix when the bump matches, increment stays on a prerelease line, ignore drops prerelease metadata before bumping. Default finalize."))
        .param(Param::string("prerelease_identifier").default("rc").describe("Identifier used when prerelease_policy=increment starts a new prerelease, for example rc, beta, or alpha. Default rc."))
        .param(Param::string("hidden_types").default("chore,style,ci,build,test").describe("Comma-separated commit types omitted from markdown release notes (breaking changes still appear). Use an empty string to show every type. Default chore,style,ci,build,test."))
        .param(Param::string("repo_url").describe("Optional repository URL such as https://github.com/acme/widget. When set, commit hashes and #123 issue references become links and a compare link is added."))
        .param(Param::string("release_date").describe("Optional release date shown in the markdown heading, for example 2026-08-29."))
        .param(Param::enumv("output_format", ["markdown", "version", "json"]).default("markdown").describe("Output format: markdown release notes (default), version for CI scripts, or json for structured downstream use."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/release-from-commits",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the next semver and release notes from Conventional Commits",
    skill(
        description = "Given the current semantic version and a pasted Conventional Commits log, compute the next version and grouped release notes. Supports type(scope)! headers, BREAKING CHANGE footers, configurable minor and patch type lists, pre-1.0 cautious mode, prerelease finalize/increment/ignore policies, hidden changelog types, optional repository links, optional release dates, and markdown/version/json output formats. It does not read git itself; paste the relevant log range.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "release-from-commits", |a: Args| {
            gizza_ai_release_from_commits_core::run(
                &a.current_version,
                &a.commits,
                &a.minor_types,
                &a.patch_types,
                &a.zero_version_policy,
                &a.prerelease_policy,
                &a.prerelease_identifier,
                &a.hidden_types,
                &a.repo_url,
                &a.release_date,
                &a.output_format,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
          "type":"object",
          "properties":{
            "current_version":{"type":"string","description":"Current semantic version or tag prefix plus version, such as 1.4.2, v1.4.2, or web-v0.8.0. The same prefix is preserved on the next version."},
            "commits":{"type":"string","description":"Pasted Conventional Commits log to analyse. Accepts git log --oneline lines, full commit messages, or bullet lists. Supports type(scope)!: subject plus BREAKING CHANGE footers."},
            "minor_types":{"type":"string","default":"feat,feature","description":"Comma-separated commit types that trigger a minor bump. Default feat,feature."},
            "patch_types":{"type":"string","default":"fix,perf,revert","description":"Comma-separated commit types that trigger a patch bump. Use * to make every non-breaking commit at least a patch. Default fix,perf,revert."},
            "zero_version_policy":{"type":"string","enum":["standard","cautious"],"default":"standard","description":"How 0.x versions handle bumps: standard treats breaking as 1.0.0 and features as 0.(minor+1).0; cautious downgrades breaking to minor and features to patch. Default standard."},
            "prerelease_policy":{"type":"string","enum":["finalize","increment","ignore"],"default":"finalize","description":"How to handle an existing pre-release version: finalize removes the suffix when the bump matches, increment stays on a prerelease line, ignore drops prerelease metadata before bumping. Default finalize."},
            "prerelease_identifier":{"type":"string","default":"rc","description":"Identifier used when prerelease_policy=increment starts a new prerelease, for example rc, beta, or alpha. Default rc."},
            "hidden_types":{"type":"string","default":"chore,style,ci,build,test","description":"Comma-separated commit types omitted from markdown release notes (breaking changes still appear). Use an empty string to show every type. Default chore,style,ci,build,test."},
            "repo_url":{"type":"string","description":"Optional repository URL such as https://github.com/acme/widget. When set, commit hashes and #123 issue references become links and a compare link is added."},
            "release_date":{"type":"string","description":"Optional release date shown in the markdown heading, for example 2026-08-29."},
            "output_format":{"type":"string","enum":["markdown","version","json"],"default":"markdown","description":"Output format: markdown release notes (default), version for CI scripts, or json for structured downstream use."}
          },
          "required":["current_version","commits"],
          "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
