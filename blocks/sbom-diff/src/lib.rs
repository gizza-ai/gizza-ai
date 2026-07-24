//! gizza-ai/sbom-diff — chat skill block on the shared tool abstraction.
//!
//! Compares two resolved dependency **lockfiles or SBOMs** and reports which
//! dependencies were **added**, **removed**, or **version-bumped** between the
//! old and new file, with counts. Pure compute — both inputs are already fully
//! resolved, so no package-manager resolver and no network are needed, and the
//! output is deterministic. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    old: String,
    new: String,
    #[serde(default)]
    old_format: String,
    #[serde(default)]
    new_format: String,
    #[serde(default = "default_true")]
    include_dev: bool,
    #[serde(default)]
    output: String,
}

impl Args {
    fn build(&self) -> Result<String, String> {
        let old_format = if self.old_format.trim().is_empty() {
            "auto"
        } else {
            &self.old_format
        };
        let new_format = if self.new_format.trim().is_empty() {
            "auto"
        } else {
            &self.new_format
        };
        let output = if self.output.trim().is_empty() {
            "text"
        } else {
            &self.output
        };
        gizza_ai_sbom_diff_core::diff(
            &self.old,
            &self.new,
            old_format,
            new_format,
            self.include_dev,
            output,
        )
    }
}

/// Single source for the chat schema (and CLI). `old` and `new` are required;
/// every option falls back to the documented default so a bare pair of lockfiles
/// yields a deterministic text diff.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("old").required().describe(
                "The OLD (before) file contents to diff against. Paste a resolved dependency \
                 lockfile — an npm package-lock.json (v1 or v2/v3), a Cargo.lock, or a pip \
                 requirements.txt — or an existing CycloneDX/SPDX SBOM. The format is \
                 auto-detected unless old_format is set.",
            ),
        )
        .param(
            Param::string("new").required().describe(
                "The NEW (after) file contents to compare with the old side. Same accepted \
                 inputs as old; the two sides may even use different formats. The format is \
                 auto-detected unless new_format is set.",
            ),
        )
        .param(
            Param::enumv(
                "old_format",
                [
                    "auto",
                    "npm",
                    "cargo",
                    "pip",
                    "cyclonedx-json",
                    "spdx-json",
                    "spdx-tag",
                ],
            )
            .default("auto")
            .describe(
                "Format of the old side: auto (detect from the content), npm \
                 (package-lock.json), cargo (Cargo.lock), pip (requirements.txt), cyclonedx-json \
                 (CycloneDX JSON SBOM), spdx-json (SPDX JSON SBOM), or spdx-tag (SPDX tag-value \
                 SBOM). Default auto.",
            ),
        )
        .param(
            Param::enumv(
                "new_format",
                [
                    "auto",
                    "npm",
                    "cargo",
                    "pip",
                    "cyclonedx-json",
                    "spdx-json",
                    "spdx-tag",
                ],
            )
            .default("auto")
            .describe(
                "Format of the new side: auto (detect from the content), npm, cargo, pip, \
                 cyclonedx-json, spdx-json, or spdx-tag. Default auto.",
            ),
        )
        .param(
            Param::boolean("include_dev").default(true).describe(
                "Include npm dev/optional dependencies on both sides (npm lockfile input only — \
                 cargo/pip and SBOM inputs do not distinguish them here). Default true.",
            ),
        )
        .param(
            Param::enumv("output", ["text", "markdown", "json"])
                .default("text")
                .describe(
                    "Report format: text (grouped human-readable report, default), markdown (a \
                     PR/CI-friendly table), or json (a machine-readable change report with a \
                     summary and added/removed/changed lists).",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sbom-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Diff two dependency lockfiles or SBOMs and list added, removed, and version-bumped packages.",
    skill(
        description = "Compare two resolved dependency lockfiles or SBOMs and report which dependencies were added, removed, or version-bumped between them. Pass old (the before file) and new (the after file) as file contents: an npm package-lock.json (v1 or v2/v3), a Cargo.lock, a pip requirements.txt, or an existing CycloneDX/SPDX SBOM (cyclonedx-json, spdx-json, spdx-tag). Each side's format is auto-detected unless old_format / new_format (auto/npm/cargo/pip/cyclonedx-json/spdx-json/spdx-tag) is set, and the two sides may use different formats. The diff is a pure set comparison of the resolved dependency inventories — no resolver and no network — so output is deterministic; version bumps are classified as upgraded, downgraded, or changed. output selects the report: text (grouped human report, default), markdown (a PR/CI table), or json (a summary plus added/removed/changed lists). include_dev keeps npm dev/optional deps on both sides. Returns the diff report as text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sbom-diff", |a: Args| {
            a.build().map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "old": { "type": "string", "description": "The OLD (before) file contents to diff against. Paste a resolved dependency lockfile — an npm package-lock.json (v1 or v2/v3), a Cargo.lock, or a pip requirements.txt — or an existing CycloneDX/SPDX SBOM. The format is auto-detected unless old_format is set." },
                    "new": { "type": "string", "description": "The NEW (after) file contents to compare with the old side. Same accepted inputs as old; the two sides may even use different formats. The format is auto-detected unless new_format is set." },
                    "old_format": { "type": "string", "enum": ["auto","npm","cargo","pip","cyclonedx-json","spdx-json","spdx-tag"], "default": "auto", "description": "Format of the old side: auto (detect from the content), npm (package-lock.json), cargo (Cargo.lock), pip (requirements.txt), cyclonedx-json (CycloneDX JSON SBOM), spdx-json (SPDX JSON SBOM), or spdx-tag (SPDX tag-value SBOM). Default auto." },
                    "new_format": { "type": "string", "enum": ["auto","npm","cargo","pip","cyclonedx-json","spdx-json","spdx-tag"], "default": "auto", "description": "Format of the new side: auto (detect from the content), npm, cargo, pip, cyclonedx-json, spdx-json, or spdx-tag. Default auto." },
                    "include_dev": { "type": "boolean", "default": true, "description": "Include npm dev/optional dependencies on both sides (npm lockfile input only — cargo/pip and SBOM inputs do not distinguish them here). Default true." },
                    "output": { "type": "string", "enum": ["text","markdown","json"], "default": "text", "description": "Report format: text (grouped human-readable report, default), markdown (a PR/CI-friendly table), or json (a machine-readable change report with a summary and added/removed/changed lists)." }
                },
                "required": ["old","new"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
