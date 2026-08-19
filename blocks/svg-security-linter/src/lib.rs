//! gizza-ai/svg-security-linter — scan SVG markup for XSS-risk constructs on the
//! shared tool abstraction. Thin chat-skill wrapper around
//! `gizza-ai-svg-security-linter-core`; the chat schema is single-sourced from
//! `descriptor()` (shared with the CLI) and the handler delegates to
//! `block_utils::run_skill`. Pure compute — nothing is rendered, fetched or executed.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    svg: String,
    #[serde(default)]
    min_severity: String,
    #[serde(default)]
    allow_external: bool,
    #[serde(default)]
    ignore: String,
    #[serde(default)]
    format: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("svg")
                .required()
                .describe("The SVG markup to lint, e.g. the full contents of an .svg file including the <svg ...> root element. Max 1,000,000 bytes. It is scanned as text — never rendered, parsed by a DOM, fetched or executed."),
        )
        .param(
            Param::enumv("min_severity", ["all", "medium", "high"])
                .default("all")
                .describe("Lowest severity to list. 'all' (default) shows high, medium and low findings; 'medium' hides low; 'high' lists only the high-severity ones. This is a display filter only — the verdict is computed before it is applied, so raising it can never turn an unsafe file into a clean-looking report."),
        )
        .param(
            Param::boolean("allow_external")
                .default(false)
                .describe("Treat http(s) and protocol-relative references to remote resources as accepted policy and drop the EXTERNAL-REF findings (default false). Unlike min_severity this changes the verdict, because the reference is no longer counted as a problem."),
        )
        .param(
            Param::string("ignore")
                .default("")
                .describe("Comma- or space-separated rule codes to suppress after review, e.g. \"UNKNOWN-NS, ANCHOR-TARGET\". Valid codes: SCRIPT, EVENT-HANDLER, JS-URL, FOREIGN-OBJECT, EMBEDDED-HTML, ANIMATE-HREF, HANDLER, DOCTYPE-ENTITY, DATA-URI, EXTERNAL-REF, CSS-IMPORT, XML-STYLESHEET, UNKNOWN-NS, ANCHOR-TARGET. An unknown code is an error rather than a silent no-op. Suppressed findings are dropped before the verdict is computed."),
        )
        .param(
            Param::enumv("format", ["text", "json", "csv"])
                .default("text")
                .describe("Output format. 'text' (default) is a ranked report with a verdict line, per-finding line:column, rule code, element/attribute and a source snippet. 'json' returns { verdict, summary, findings[] } for programmatic use. 'csv' returns just the findings rows with a header, for a spreadsheet or a ticket."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SvgSecurityLinter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svg-security-linter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan SVG markup for XSS-risk constructs and rank them by severity",
    skill(
        description = "Lint SVG markup for the constructs that turn an uploaded or inlined SVG into an XSS vector, and rank them high/medium/low. Pass the file's markup as 'svg'. Detects <script> elements, on* event-handler attributes, javascript:/vbscript: URLs including XML character-reference obfuscation, <foreignObject>, embedded HTML/active elements (iframe/object/embed/base/link/meta/audio/video/form), SMIL <animate>/<set> that retarget href/style/on* handlers, the SVG 1.2 <handler> element and ev:* attributes, DOCTYPE/ENTITY declarations (XXE), data: URLs (high for text/html and image/svg+xml), external http(s) and protocol-relative references, CSS @import / remote url(...) / expression(...), <?xml-stylesheet?> processing instructions, unknown namespaces, and <a target=\\\"_blank\\\"> without rel=noopener. Every finding carries a line, column, rule code, element, attribute and snippet, and the report opens with a verdict of unsafe, review or clean. min_severity='all'|'medium'|'high' filters the listed rows only — the verdict always counts every finding. allow_external=true accepts remote references as policy and drops EXTERNAL-REF. ignore takes rule codes to suppress after review. format='text'|'json'|'csv'. This is a reporter, not a sanitizer: it never emits a cleaned file, because a blocklist scrub of untrusted SVG gives false assurance. Max 1,000,000 bytes. Runs locally — nothing is rendered, fetched or executed.",
        parameters = schema_json()
    ),
)]
impl SvgSecurityLinter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "svg-security-linter", |a: Args| {
            gizza_ai_svg_security_linter_core::lint(
                &a.svg,
                &a.min_severity,
                a.allow_external,
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "svg": { "type": "string", "description": "The SVG markup to lint, e.g. the full contents of an .svg file including the <svg ...> root element. Max 1,000,000 bytes. It is scanned as text — never rendered, parsed by a DOM, fetched or executed." },
                    "min_severity": { "type": "string", "enum": ["all", "medium", "high"], "default": "all", "description": "Lowest severity to list. 'all' (default) shows high, medium and low findings; 'medium' hides low; 'high' lists only the high-severity ones. This is a display filter only — the verdict is computed before it is applied, so raising it can never turn an unsafe file into a clean-looking report." },
                    "allow_external": { "type": "boolean", "default": false, "description": "Treat http(s) and protocol-relative references to remote resources as accepted policy and drop the EXTERNAL-REF findings (default false). Unlike min_severity this changes the verdict, because the reference is no longer counted as a problem." },
                    "ignore": { "type": "string", "default": "", "description": "Comma- or space-separated rule codes to suppress after review, e.g. \"UNKNOWN-NS, ANCHOR-TARGET\". Valid codes: SCRIPT, EVENT-HANDLER, JS-URL, FOREIGN-OBJECT, EMBEDDED-HTML, ANIMATE-HREF, HANDLER, DOCTYPE-ENTITY, DATA-URI, EXTERNAL-REF, CSS-IMPORT, XML-STYLESHEET, UNKNOWN-NS, ANCHOR-TARGET. An unknown code is an error rather than a silent no-op. Suppressed findings are dropped before the verdict is computed." },
                    "format": { "type": "string", "enum": ["text", "json", "csv"], "default": "text", "description": "Output format. 'text' (default) is a ranked report with a verdict line, per-finding line:column, rule code, element/attribute and a source snippet. 'json' returns { verdict, summary, findings[] } for programmatic use. 'csv' returns just the findings rows with a header, for a spreadsheet or a ticket." }
                },
                "required": ["svg"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
