//! gizza-ai/dependency-risk-auditor — chat skill block on the shared tool abstraction.
//!
//! Audits a pasted npm `package.json` or lockfile (`package-lock.json`,
//! `yarn.lock`, `pnpm-lock.yaml`) for risky supply-chain patterns: wildcard and
//! dist-tag version specs, git/URL/local dependencies, install and lifecycle
//! scripts, missing or weak integrity hashes, non-registry resolved URLs, and
//! disagreement between the manifest and the lockfile. Pure compute — every
//! finding is derived from the pasted bytes, with no registry lookups and no
//! network. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI); `handle()` delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    manifest: String,
    #[serde(default)]
    lockfile: String,
    #[serde(default)]
    manifest_format: String,
    #[serde(default)]
    strictness: String,
    #[serde(default = "default_true")]
    include_dev: bool,
    #[serde(default)]
    ignore: String,
    #[serde(default)]
    fail_on: String,
    #[serde(default)]
    output: String,
}

impl Args {
    fn build(&self) -> Result<String, String> {
        gizza_ai_dependency_risk_auditor_core::audit(
            &self.manifest,
            &self.lockfile,
            &self.manifest_format,
            &self.strictness,
            self.include_dev,
            &self.ignore,
            &self.fail_on,
            &self.output,
        )
    }
}

/// Single source for the chat schema (and CLI). `manifest` is the only required
/// param; every option falls back to its documented default, so pasting a bare
/// package.json already yields a graded risk report.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("manifest").required().describe(
            "The file to audit, pasted as text: an npm package.json, a package-lock.json, a \
             yarn.lock (classic or Berry), or a pnpm-lock.yaml. The format is auto-detected \
             unless manifest_format is set. Maximum 2097152 bytes.",
        ))
        .param(Param::string("lockfile").describe(
            "Optional second file: the lockfile that goes with the package.json in manifest. \
             Supplying both also runs the cross-checks — dependencies declared but missing from \
             the lockfile (unlocked-dependency) and exact pins that disagree with the locked \
             version (pin-mismatch). Leave empty to audit a single file. Maximum 2097152 bytes.",
        ))
        .param(
            Param::enumv(
                "manifest_format",
                [
                    "auto",
                    "package-json",
                    "package-lock",
                    "yarn-lock",
                    "pnpm-lock",
                ],
            )
            .default("auto")
            .describe(
                "How to parse manifest: auto (detect from the content, default), package-json, \
                 package-lock (npm lockfileVersion 1/2/3), yarn-lock (classic v1 or Berry), or \
                 pnpm-lock (pnpm-lock.yaml).",
            ),
        )
        .param(
            Param::enumv("strictness", ["lenient", "standard", "strict"])
                .default("standard")
                .describe(
                    "Which findings to report, by severity: lenient (high only — the \
                     supply-chain red flags), standard (high and medium, the default), or \
                     strict (also low and info — loose caret/tilde ranges, missing engines, \
                     Node built-in name shadowing, overrides, legacy lockfile versions).",
                ),
        )
        .param(Param::boolean("include_dev").default(true).describe(
            "Audit devDependencies and dev-only lockfile entries too. Set false to review only \
             what ships to production. Default true.",
        ))
        .param(Param::string("ignore").describe(
            "Rule IDs to suppress, comma- or newline-separated. Example: \"range-prefix, \
             third-party-registry\". Rule IDs appear in square brackets in the text report and \
             in the rule field of the JSON report — wildcard-version, dist-tag-version, \
             prerelease-version, range-prefix, git-dependency, url-dependency, http-dependency, \
             file-dependency, alias-dependency, duplicate-dependency, builtin-shadow, \
             install-script, lifecycle-script, forced-override, missing-engines, \
             missing-integrity, weak-integrity, insecure-resolved-url, git-resolved, \
             third-party-registry, resolved-version-mismatch, has-install-script, \
             legacy-lockfile-version, unlocked-dependency, pin-mismatch, no-lockfile-supplied. \
             Leave empty to report everything.",
        ))
        .param(
            Param::enumv("fail_on", ["high", "medium", "low", "info", "never"])
                .default("high")
                .describe(
                    "Lowest severity that makes the verdict FAIL, for CI gating: high \
                     (default), medium, low, info (any finding fails), or never (always PASS, \
                     report only). Findings already filtered out by strictness or ignore never \
                     affect the verdict.",
                ),
        )
        .param(
            Param::enumv("output", ["text", "markdown", "json"])
                .default("text")
                .describe(
                    "Report format: text (verdict, grade and findings grouped by severity — \
                     the default), markdown (a PR-ready findings table), or json (verdict, \
                     score, grade, per-severity summary and a findings array).",
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
    name = "gizza-ai/dependency-risk-auditor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Audit a package.json or lockfile for risky dependency patterns and get a graded report.",
    skill(
        description = "Audit a pasted npm package.json or lockfile for risky supply-chain patterns and return a graded PASS/FAIL report. manifest takes the file contents: a package.json, package-lock.json (lockfileVersion 1/2/3), yarn.lock (classic or Berry) or pnpm-lock.yaml; the format is auto-detected unless manifest_format is set. Manifest rules flag wildcard and `latest` specs (wildcard-version), dist-tag specs such as next/beta (dist-tag-version), pre-release specs (prerelease-version), loose caret/tilde ranges (range-prefix), git and GitHub-shorthand dependencies (git-dependency), remote tarball URLs (url-dependency) and plain-http ones (http-dependency), file:/link:/portal: paths (file-dependency), npm: aliases (alias-dependency), a package declared in both dependencies and devDependencies (duplicate-dependency), package names that shadow Node built-ins (builtin-shadow), preinstall/install/postinstall scripts (install-script), other lifecycle scripts (lifecycle-script), overrides/resolutions (forced-override) and a missing engines field (missing-engines). Lockfile rules flag missing (missing-integrity) or SHA-1 (weak-integrity) hashes, plain-http resolved URLs (insecure-resolved-url), git-resolved entries (git-resolved), non-npm registry hosts (third-party-registry), a resolved URL whose version disagrees with the entry (resolved-version-mismatch), packages that run install scripts (has-install-script) and lockfileVersion 1 (legacy-lockfile-version). Pass a package.json in manifest AND its lockfile in lockfile to also get unlocked-dependency and pin-mismatch cross-checks. strictness (lenient/standard/strict, default standard) selects the severity floor, include_dev (default true) covers devDependencies, ignore suppresses rule IDs, fail_on (high/medium/low/info/never, default high) sets the FAIL threshold, and output picks text (default), markdown or json. Each report carries a 0-100 risk score and an A-F grade. Everything is local pure compute: no registry lookups, so known-vulnerability (CVE) matching, package age, maintainer counts and package-contents scanning are out of scope. Returns the audit report as text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dependency-risk-auditor", |a: Args| {
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
                    "manifest": { "type": "string", "description": "The file to audit, pasted as text: an npm package.json, a package-lock.json, a yarn.lock (classic or Berry), or a pnpm-lock.yaml. The format is auto-detected unless manifest_format is set. Maximum 2097152 bytes." },
                    "lockfile": { "type": "string", "description": "Optional second file: the lockfile that goes with the package.json in manifest. Supplying both also runs the cross-checks — dependencies declared but missing from the lockfile (unlocked-dependency) and exact pins that disagree with the locked version (pin-mismatch). Leave empty to audit a single file. Maximum 2097152 bytes." },
                    "manifest_format": { "type": "string", "enum": ["auto","package-json","package-lock","yarn-lock","pnpm-lock"], "default": "auto", "description": "How to parse manifest: auto (detect from the content, default), package-json, package-lock (npm lockfileVersion 1/2/3), yarn-lock (classic v1 or Berry), or pnpm-lock (pnpm-lock.yaml)." },
                    "strictness": { "type": "string", "enum": ["lenient","standard","strict"], "default": "standard", "description": "Which findings to report, by severity: lenient (high only — the supply-chain red flags), standard (high and medium, the default), or strict (also low and info — loose caret/tilde ranges, missing engines, Node built-in name shadowing, overrides, legacy lockfile versions)." },
                    "include_dev": { "type": "boolean", "default": true, "description": "Audit devDependencies and dev-only lockfile entries too. Set false to review only what ships to production. Default true." },
                    "ignore": { "type": "string", "description": "Rule IDs to suppress, comma- or newline-separated. Example: \"range-prefix, third-party-registry\". Rule IDs appear in square brackets in the text report and in the rule field of the JSON report — wildcard-version, dist-tag-version, prerelease-version, range-prefix, git-dependency, url-dependency, http-dependency, file-dependency, alias-dependency, duplicate-dependency, builtin-shadow, install-script, lifecycle-script, forced-override, missing-engines, missing-integrity, weak-integrity, insecure-resolved-url, git-resolved, third-party-registry, resolved-version-mismatch, has-install-script, legacy-lockfile-version, unlocked-dependency, pin-mismatch, no-lockfile-supplied. Leave empty to report everything." },
                    "fail_on": { "type": "string", "enum": ["high","medium","low","info","never"], "default": "high", "description": "Lowest severity that makes the verdict FAIL, for CI gating: high (default), medium, low, info (any finding fails), or never (always PASS, report only). Findings already filtered out by strictness or ignore never affect the verdict." },
                    "output": { "type": "string", "enum": ["text","markdown","json"], "default": "text", "description": "Report format: text (verdict, grade and findings grouped by severity — the default), markdown (a PR-ready findings table), or json (verdict, score, grade, per-severity summary and a findings array)." }
                },
                "required": ["manifest"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
