//! gizza-ai/js-linter — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_js_linter_core::run_with_options;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_preset")]
    preset: String,
    #[serde(default = "default_ecma")]
    ecma: String,
    #[serde(default = "default_env")]
    env: String,
    #[serde(default = "default_source_type")]
    source_type: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default)]
    ignore: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_preset() -> String {
    "recommended".into()
}
fn default_ecma() -> String {
    "latest".into()
}
fn default_env() -> String {
    "browser".into()
}
fn default_source_type() -> String {
    "auto".into()
}
fn default_min_severity() -> String {
    "all".into()
}
fn default_format() -> String {
    "text".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("code").required().describe("JavaScript source code to lint. Paste plain .js; JSX and TypeScript are intentionally out of scope."))
        .param(Param::enumv("preset", ["recommended", "minimal", "strict"]).default("recommended").describe("Rule bundle to run. minimal keeps only high-signal bug checks; recommended is balanced; strict adds style-oriented warnings."))
        .param(Param::enumv("ecma", ["latest", "es2020", "es2015", "es5"]).default("latest").describe("JavaScript target version. es5 suppresses modern no-var guidance; newer targets recommend let/const."))
        .param(Param::enumv("env", ["browser", "node", "both", "none"]).default("browser").describe("Known global environment used when checking implicit assignments and undeclared names."))
        .param(Param::enumv("source_type", ["auto", "script", "module"]).default("auto").describe("Treat the input as a classic script, an ES module, or infer automatically. script flags import/export syntax."))
        .param(Param::enumv("min_severity", ["all", "warning", "error"]).default("all").describe("Filter the report to all findings, warning-or-error findings, or only errors."))
        .param(Param::string("ignore").describe("Optional comma- or space-separated rule IDs to suppress, such as EQEQ, UNUSED-VAR, SEMICOLON, or NO-CONSOLE."))
        .param(Param::enumv("format", ["text", "json"]).default("text").describe("Output format: human-readable text report or JSON diagnostics for scripts and CI."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/js-linter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Lint JavaScript for common bugs and style hazards",
    skill(
        description = "Lint JavaScript locally for common issues: syntax imbalance, loose equality, unreachable code, unused variables, implicit globals, var in modern targets, missing semicolons, console/debugger/alert calls, module/script mismatches, and single-line control flow without braces. Supports presets, environment globals, severity filtering, ignored rule IDs, and text or JSON output. JSX, TypeScript, auto-fixing, and deep AST metrics are deliberately out of scope.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "js-linter", |a: Args| {
            run_with_options(
                &a.code,
                &a.preset,
                &a.ecma,
                &a.env,
                &a.source_type,
                &a.min_severity,
                &a.ignore,
                &a.format,
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
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived["required"], serde_json::json!(["code"]));
        assert_eq!(
            derived["properties"]["preset"]["enum"],
            serde_json::json!(["recommended", "minimal", "strict"])
        );
        assert_eq!(
            derived["properties"]["format"]["enum"],
            serde_json::json!(["text", "json"])
        );
        for key in [
            "code",
            "preset",
            "ecma",
            "env",
            "source_type",
            "min_severity",
            "ignore",
            "format",
        ] {
            assert!(
                derived["properties"][key]["description"]
                    .as_str()
                    .unwrap()
                    .len()
                    > 10
            );
        }
    }
}
