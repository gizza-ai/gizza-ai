//! gizza-ai/license-checker — chat skill block on the shared tool abstraction.
//!
//! Evaluates the **SPDX license identifiers** carried by an SBOM (CycloneDX or
//! SPDX) or a plain dependency list against user-supplied **allow / deny**
//! rules, and returns a pass/fail compliance report. Pure compute — the input
//! already records the resolved dependency set and its license metadata, so no
//! package manager, no filesystem walk and no network are needed, and the
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
    dependencies: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    allow: String,
    #[serde(default)]
    deny: String,
    #[serde(default)]
    exceptions: String,
    #[serde(default)]
    unlisted: String,
    #[serde(default)]
    unknown: String,
    #[serde(default = "default_true")]
    validate_ids: bool,
    #[serde(default)]
    include_allowed: bool,
    #[serde(default)]
    output: String,
}

impl Args {
    fn build(&self) -> Result<String, String> {
        gizza_ai_license_checker_core::check(
            &self.dependencies,
            &self.input_format,
            &self.allow,
            &self.deny,
            &self.exceptions,
            &self.unlisted,
            &self.unknown,
            self.validate_ids,
            self.include_allowed,
            &self.output,
        )
    }
}

/// Single source for the chat schema (and CLI). `dependencies` is required;
/// every option falls back to the documented default, so a bare SBOM paste
/// still yields an inventory report with a PASS/FAIL verdict.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("dependencies").required().describe(
            "The SBOM or dependency list to check. Paste the file contents: a CycloneDX JSON \
                 SBOM, an SPDX JSON or tag-value SBOM, a dependency-inventory JSON object mapping \
                 \"name@version\" to a record with a licenses field, or a plain list with one \
                 package per line (\"chalk@4.1.2: MIT\", \"left-pad,1.3.0,WTFPL\", \"requests \
                 Apache-2.0\"). The format is auto-detected unless input_format is set.",
        ))
        .param(
            Param::enumv(
                "input_format",
                [
                    "auto",
                    "cyclonedx-json",
                    "spdx-json",
                    "spdx-tag",
                    "npm-json",
                    "list",
                ],
            )
            .default("auto")
            .describe(
                "How to parse the input: auto (detect from the content), cyclonedx-json (a \
                 CycloneDX JSON SBOM), spdx-json (an SPDX JSON SBOM), spdx-tag (an SPDX \
                 tag-value SBOM), npm-json (a \"name@version\" -> {licenses} inventory object), \
                 or list (one package per line). Default auto.",
            ),
        )
        .param(Param::string("allow").describe(
            "Licenses that are permitted, as a comma- or newline-separated list. Each entry \
                 is an SPDX identifier (\"MIT\", \"Apache-2.0\", \"Apache-2.0 WITH \
                 LLVM-exception\") or a category token: category:public-domain, \
                 category:permissive, category:weak-copyleft, category:strong-copyleft, \
                 category:network-copyleft, category:proprietary, category:unknown. Example: \
                 \"MIT, Apache-2.0, category:public-domain\". Leave empty to accept anything \
                 that is not denied. When this list is non-empty, a license matching nothing in \
                 it falls to the unlisted policy.",
        ))
        .param(Param::string("deny").describe(
            "Licenses that are forbidden, in the same format as allow (SPDX identifiers \
                 and/or category: tokens, comma- or newline-separated). Example: \
                 \"category:strong-copyleft, category:network-copyleft, SSPL-1.0\". A deny match \
                 always wins over an allow match. Leave empty for no deny list.",
        ))
        .param(Param::string("exceptions").describe(
            "Packages that are always accepted regardless of their license, as a comma- or \
                 newline-separated list of \"name\" or \"name@version\" entries. Example: \
                 \"legacy-widget, my-gpl-tool@2.1.0\". A bare name matches every version. Leave \
                 empty for none.",
        ))
        .param(
            Param::enumv("unlisted", ["allow", "warn", "deny"])
                .default("deny")
                .describe(
                    "What to do with a license that matches no rule while an allow list is \
                     configured: allow (accept it), warn (report it without failing), or deny \
                     (fail the check). Ignored when allow is empty. Default deny, so an allow \
                     list is exhaustive.",
                ),
        )
        .param(
            Param::enumv("unknown", ["allow", "warn", "deny"])
                .default("warn")
                .describe(
                    "What to do with a package that has no license metadata at all (missing, \
                     NOASSERTION, NONE, UNLICENSED): allow, warn (report it without failing), or \
                     deny. Default warn.",
                ),
        )
        .param(Param::boolean("validate_ids").default(true).describe(
            "Also check that each identifier is a recognized SPDX license ID. Unrecognized \
                 IDs are counted as invalid and reported as warnings (LicenseRef-… custom IDs \
                 are valid SPDX and are not flagged). Default true.",
        ))
        .param(Param::boolean("include_allowed").default(false).describe(
            "List the compliant packages too, not just the violations — useful for a full \
                 inventory export. Default false, so the report leads with what needs \
                 attention.",
        ))
        .param(
            Param::enumv("output", ["text", "markdown", "json", "csv"])
                .default("text")
                .describe(
                    "Report format: text (grouped human-readable report, default), markdown (a \
                     PR/CI-friendly table), json (verdict, summary, findings and a per-license \
                     roll-up), or csv (one row per reported package).",
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
    name = "gizza-ai/license-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check the SPDX licenses in an SBOM or dependency list against allow/deny rules.",
    skill(
        description = "Check the SPDX license identifiers in an SBOM or dependency list against allow/deny rules and return a PASS/FAIL compliance report. Pass dependencies as file contents: a CycloneDX JSON SBOM, an SPDX JSON or tag-value SBOM, a dependency-inventory JSON object mapping \"name@version\" to a record with a licenses field, or a plain list with one package per line (\"chalk@4.1.2: MIT\", \"left-pad,1.3.0,WTFPL\"); the format is auto-detected unless input_format (auto/cyclonedx-json/spdx-json/spdx-tag/npm-json/list) is set. allow and deny are comma- or newline-separated lists of SPDX identifiers (including \"X WITH Y\" exception forms) and/or category tokens — category:public-domain, category:permissive, category:weak-copyleft, category:strong-copyleft, category:network-copyleft, category:proprietary, category:unknown — and a deny match always beats an allow match. Full SPDX expressions are evaluated: OR is a choice (acceptable when any branch is), AND requires every branch, WITH attaches an exception, deprecated IDs and a trailing + are normalized (GPL-2.0+ becomes GPL-2.0-or-later). exceptions lists \"name\" or \"name@version\" packages that are always accepted. unlisted (allow/warn/deny, default deny) governs licenses matching no rule when an allow list is set; unknown (allow/warn/deny, default warn) governs packages with no license metadata; validate_ids flags identifiers that are not on the SPDX list. output selects text (default), markdown, json, or csv, and include_allowed adds the compliant packages to the report. Everything is local pure compute — no registry lookups. Returns the compliance report as text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "license-checker", |a: Args| {
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
                    "dependencies": { "type": "string", "description": "The SBOM or dependency list to check. Paste the file contents: a CycloneDX JSON SBOM, an SPDX JSON or tag-value SBOM, a dependency-inventory JSON object mapping \"name@version\" to a record with a licenses field, or a plain list with one package per line (\"chalk@4.1.2: MIT\", \"left-pad,1.3.0,WTFPL\", \"requests Apache-2.0\"). The format is auto-detected unless input_format is set." },
                    "input_format": { "type": "string", "enum": ["auto","cyclonedx-json","spdx-json","spdx-tag","npm-json","list"], "default": "auto", "description": "How to parse the input: auto (detect from the content), cyclonedx-json (a CycloneDX JSON SBOM), spdx-json (an SPDX JSON SBOM), spdx-tag (an SPDX tag-value SBOM), npm-json (a \"name@version\" -> {licenses} inventory object), or list (one package per line). Default auto." },
                    "allow": { "type": "string", "description": "Licenses that are permitted, as a comma- or newline-separated list. Each entry is an SPDX identifier (\"MIT\", \"Apache-2.0\", \"Apache-2.0 WITH LLVM-exception\") or a category token: category:public-domain, category:permissive, category:weak-copyleft, category:strong-copyleft, category:network-copyleft, category:proprietary, category:unknown. Example: \"MIT, Apache-2.0, category:public-domain\". Leave empty to accept anything that is not denied. When this list is non-empty, a license matching nothing in it falls to the unlisted policy." },
                    "deny": { "type": "string", "description": "Licenses that are forbidden, in the same format as allow (SPDX identifiers and/or category: tokens, comma- or newline-separated). Example: \"category:strong-copyleft, category:network-copyleft, SSPL-1.0\". A deny match always wins over an allow match. Leave empty for no deny list." },
                    "exceptions": { "type": "string", "description": "Packages that are always accepted regardless of their license, as a comma- or newline-separated list of \"name\" or \"name@version\" entries. Example: \"legacy-widget, my-gpl-tool@2.1.0\". A bare name matches every version. Leave empty for none." },
                    "unlisted": { "type": "string", "enum": ["allow","warn","deny"], "default": "deny", "description": "What to do with a license that matches no rule while an allow list is configured: allow (accept it), warn (report it without failing), or deny (fail the check). Ignored when allow is empty. Default deny, so an allow list is exhaustive." },
                    "unknown": { "type": "string", "enum": ["allow","warn","deny"], "default": "warn", "description": "What to do with a package that has no license metadata at all (missing, NOASSERTION, NONE, UNLICENSED): allow, warn (report it without failing), or deny. Default warn." },
                    "validate_ids": { "type": "boolean", "default": true, "description": "Also check that each identifier is a recognized SPDX license ID. Unrecognized IDs are counted as invalid and reported as warnings (LicenseRef-… custom IDs are valid SPDX and are not flagged). Default true." },
                    "include_allowed": { "type": "boolean", "default": false, "description": "List the compliant packages too, not just the violations — useful for a full inventory export. Default false, so the report leads with what needs attention." },
                    "output": { "type": "string", "enum": ["text","markdown","json","csv"], "default": "text", "description": "Report format: text (grouped human-readable report, default), markdown (a PR/CI-friendly table), json (verdict, summary, findings and a per-license roll-up), or csv (one row per reported package)." }
                },
                "required": ["dependencies"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
