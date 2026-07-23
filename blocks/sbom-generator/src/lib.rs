//! gizza-ai/sbom-generator — chat skill block on the shared tool abstraction.
//!
//! Turns a resolved dependency **lockfile** (npm `package-lock.json`, Cargo
//! `Cargo.lock`, or pip `requirements.txt`) into a Software Bill of Materials
//! in CycloneDX 1.6 JSON, SPDX 2.3 JSON, or SPDX 2.3 tag-value. Pure compute —
//! a lockfile is already fully resolved, so no resolver and no network are
//! needed, and output is deterministic. The chat schema is single-sourced from
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
    lockfile: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    component_name: String,
    #[serde(default)]
    component_version: String,
    #[serde(default = "default_true")]
    include_dev: bool,
    #[serde(default)]
    timestamp: String,
    #[serde(default = "default_true")]
    pretty: bool,
}

impl Args {
    fn build(&self) -> Result<String, String> {
        let input_format = if self.input_format.trim().is_empty() {
            "auto"
        } else {
            &self.input_format
        };
        let output = if self.output.trim().is_empty() {
            "cyclonedx-json"
        } else {
            &self.output
        };
        gizza_ai_sbom_generator_core::generate(
            &self.lockfile,
            input_format,
            output,
            &self.component_name,
            &self.component_version,
            self.include_dev,
            &self.timestamp,
            self.pretty,
        )
    }
}

/// Single source for the chat schema (and CLI). `lockfile` is required; every
/// option falls back to the documented default so a bare lockfile yields a
/// deterministic CycloneDX SBOM.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("lockfile").required().describe(
                "The resolved dependency lockfile to convert. Paste the file contents: an npm \
                 package-lock.json (v1 or v2/v3), a Cargo.lock, or a pip requirements.txt. The \
                 ecosystem is auto-detected unless input_format is set.",
            ),
        )
        .param(
            Param::enumv("input_format", ["auto", "npm", "cargo", "pip"])
                .default("auto")
                .describe(
                    "Which lockfile format to parse: auto (detect from the content), npm \
                     (package-lock.json), cargo (Cargo.lock), or pip (requirements.txt). Default \
                     auto.",
                ),
        )
        .param(
            Param::enumv("output", ["cyclonedx-json", "spdx-json", "spdx-tag"])
                .default("cyclonedx-json")
                .describe(
                    "SBOM format to emit: cyclonedx-json (CycloneDX 1.6 JSON), spdx-json (SPDX \
                     2.3 JSON), or spdx-tag (SPDX 2.3 tag-value text). Default cyclonedx-json.",
                ),
        )
        .param(
            Param::string("component_name").default("").describe(
                "Name of the root/primary component (the project itself). Blank = use the name \
                 recorded in the lockfile, if any (npm/cargo record it; pip does not).",
            ),
        )
        .param(
            Param::string("component_version").default("").describe(
                "Version of the root/primary component. Blank = use the version recorded in the \
                 lockfile, if any.",
            ),
        )
        .param(
            Param::boolean("include_dev").default(true).describe(
                "Include dev/optional dependencies (npm only — cargo/pip lockfiles do not \
                 distinguish them here). Default true.",
            ),
        )
        .param(
            Param::string("timestamp").default("").describe(
                "RFC-3339 creation time to stamp in the SBOM metadata, e.g. \
                 2026-07-24T12:00:00Z. Blank = a fixed placeholder so identical input yields \
                 byte-identical output.",
            ),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print JSON output (ignored for spdx-tag). Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sbom-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an npm, Cargo, or pip lockfile into a CycloneDX or SPDX software bill of materials.",
    skill(
        description = "Convert a resolved dependency lockfile into a Software Bill of Materials (SBOM). Pass lockfile (the file contents): an npm package-lock.json (v1 dependencies tree or v2/v3 packages map), a Cargo.lock, or a pip requirements.txt — the ecosystem is auto-detected unless input_format (auto/npm/cargo/pip) is set. output selects the SBOM format: cyclonedx-json (CycloneDX 1.6 JSON, default), spdx-json (SPDX 2.3 JSON), or spdx-tag (SPDX 2.3 tag-value text). Every dependency becomes a component with a package URL (pkg:npm/…, pkg:cargo/…, pkg:pypi/…); components are de-duplicated and sorted so output is deterministic. Options: component_name / component_version override the root project's identity, include_dev keeps npm dev/optional deps, timestamp stamps an RFC-3339 creation time (blank = fixed placeholder for reproducible output), pretty pretty-prints JSON. Returns the SBOM as text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sbom-generator", |a: Args| {
            a.build().map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
