//! gizza-ai/html-accessibility-checker — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default = "default_level")]
    level: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    show_passed: bool,
    #[serde(default = "default_max_issues")]
    max_issues: usize,
}

fn default_level() -> String {
    "aa".into()
}
fn default_min_severity() -> String {
    "suggestion".into()
}
fn default_format() -> String {
    "text".into()
}
fn default_max_issues() -> usize {
    200
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("html").required().describe("HTML document or fragment to audit for automatable accessibility problems. Paste markup up to 5,000,000 bytes; it is parsed locally as text and never fetched, uploaded, executed, or rendered."))
        .param(Param::enumv("level", ["a", "aa", "aaa"]).default("aa").describe("Highest WCAG level to include. a runs Level A checks, aa (default) adds Level AA checks such as zoom/focus-outline issues, and aaa adds advisory Level AAA checks such as generic link text."))
        .param(Param::enumv("min_severity", ["suggestion", "warning", "error"]).default("suggestion").describe("Lowest finding severity to show. suggestion reports every issue, warning hides suggestions, and error shows only failures that usually block assistive-technology use."))
        .param(Param::enumv("format", ["text", "markdown", "json", "csv"]).default("text").describe("Report format. text is a readable grouped report, markdown is suitable for issue trackers, json returns score/counts/issues, and csv returns severity/code/WCAG/line/column/element/message rows."))
        .param(Param::boolean("show_passed").default(false).describe("Also include rules that had candidates and passed. Off by default so reports focus on problems; turn on for audits that need evidence of checked rules."))
        .param(Param::integer("max_issues").default(200).min(1.0).max(5000.0).describe("Maximum number of findings to include in the report, from 1 to 5000. Extra findings are counted as omitted so very large pages stay readable."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-accessibility-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan pasted HTML for common WCAG accessibility issues",
    skill(
        description = "Scan pasted HTML documents or fragments for automatable accessibility issues: missing lang/title/main landmarks, images without useful alt text, unlabeled controls, empty or generic links/buttons, heading-order problems, duplicate ids, iframe titles, invalid ARIA roles, aria-hidden focus traps, tables without headers, zoom-blocking viewports, autoplay media, videos without captions, removed focus outlines, and positive tabindex. Parameters: html, level (a|aa|aaa), min_severity (suggestion|warning|error), format (text|markdown|json|csv), show_passed, and max_issues. Reports include line/column, severity, stable rule code, WCAG reference, element, and fix guidance. This is a local lexical checker, not a browser, screen reader, contrast analyzer, axe-core clone, or proof of compliance.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-accessibility-checker", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

fn run(a: Args) -> Result<String, String> {
    let format = gizza_ai_html_accessibility_checker_core::parse_format(&a.format)?;
    let opts = gizza_ai_html_accessibility_checker_core::Options {
        level: gizza_ai_html_accessibility_checker_core::parse_level(&a.level)?,
        min_severity: gizza_ai_html_accessibility_checker_core::parse_severity(&a.min_severity)?,
        show_passed: a.show_passed,
        max_issues: a.max_issues,
    };
    gizza_ai_html_accessibility_checker_core::check_to_string(&a.html, format, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_expected_controls_and_defaults() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["required"], serde_json::json!(["html"]));
        assert_eq!(
            v["properties"]["level"]["enum"],
            serde_json::json!(["a", "aa", "aaa"])
        );
        assert_eq!(v["properties"]["level"]["default"], "aa");
        assert_eq!(
            v["properties"]["min_severity"]["enum"],
            serde_json::json!(["suggestion", "warning", "error"])
        );
        assert_eq!(
            v["properties"]["format"]["enum"],
            serde_json::json!(["text", "markdown", "json", "csv"])
        );
        assert_eq!(v["properties"]["show_passed"]["default"], false);
        assert_eq!(v["properties"]["max_issues"]["default"], 200);
        for (_name, prop) in v["properties"].as_object().unwrap() {
            assert!(
                prop["description"].as_str().unwrap_or("").len() > 40,
                "missing useful description: {prop}"
            );
        }
    }

    #[test]
    fn args_defaults_match_descriptor() {
        let a: Args = serde_json::from_str(r#"{"html":"<img src=x>"}"#).unwrap();
        assert_eq!(a.level, "aa");
        assert_eq!(a.min_severity, "suggestion");
        assert_eq!(a.format, "text");
        assert!(!a.show_passed);
        assert_eq!(a.max_issues, 200);
    }
}
