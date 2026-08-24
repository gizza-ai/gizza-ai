//! gizza-ai/markdown-link-check — chat skill block on the shared tool abstraction.
//! Offline structural link checker for Markdown: malformed link syntax, broken
//! in-document anchors, and reference-definition problems. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_link_check_core::run as check_run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default = "default_link_kind")]
    link_kind: String,
    #[serde(default = "default_report_format")]
    report_format: String,
    #[serde(default)]
    show_ok: bool,
    #[serde(default = "default_true")]
    check_anchors: bool,
    #[serde(default)]
    flag_insecure: bool,
}

fn default_link_kind() -> String {
    "all".into()
}

fn default_report_format() -> String {
    "text".into()
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown document to check, e.g. the contents of README.md. Max 1 MB."),
        )
        .param(
            Param::enumv(
                "link_kind",
                [
                    "all",
                    "anchor",
                    "external",
                    "relative",
                    "mailto",
                    "image",
                    "empty",
                    "reference",
                ],
            )
            .default("all")
            .describe(
                "Only report links of this kind. all (default) = every link; anchor = #in-document \
                 targets; external = http/https and other schemes; relative = paths like ./docs/a.md; \
                 mailto = mail addresses; image = ![alt](src); empty = links with no target; \
                 reference = [text][label] links whose definition is missing.",
            ),
        )
        .param(
            Param::enumv("report_format", ["text", "markdown", "json"])
                .default("text")
                .describe(
                    "How to render the report. text (default) = one 'line:col severity rule message' \
                     per finding; markdown = a table you can paste into an issue or PR; json = a \
                     machine-readable object with checked/errors/warnings/issues/links for CI.",
                ),
        )
        .param(
            Param::boolean("show_ok").default(false).describe(
                "Also list every link that passed, with its kind and target, not just the problems. \
                 Default false (problems only).",
            ),
        )
        .param(
            Param::boolean("check_anchors").default(true).describe(
                "Verify that each #anchor target matches a heading id this document actually \
                 produces (GitHub-style slugs, duplicate headings numbered -1/-2, plus {#custom-id} \
                 and HTML id=\"…\" anchors). Default true; set false to ignore in-document anchors.",
            ),
        )
        .param(
            Param::boolean("flag_insecure").default(false).describe(
                "Also warn about http:// links that could be https:// (rule ML011). Default false, \
                 since docs often cite http:// URLs deliberately.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownLinkCheck;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-link-check",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check Markdown links offline for bad syntax, broken anchors and reference problems",
    skill(
        description = "Check a Markdown document's links and images without touching the network. Reports: empty link target (ML001), empty link text (ML002), image with no alt text (ML003), undefined reference [text][label] (ML004), duplicate reference definition (ML005), unused reference definition (ML006), broken in-document anchor (ML007), reversed link syntax (text)[url] (ML008), unencoded space in a URL (ML009), malformed mailto address (ML010), insecure http:// link (ML011, opt-in), unclosed link syntax (ML012), and a space between the link text and the URL (ML013). Anchors are matched against GitHub-style heading slugs, including duplicate-heading -1/-2 numbering, {#custom-id} attributes and HTML id=\"…\" anchors. Fenced code blocks and inline code spans are skipped. Filter with link_kind, choose text/markdown/json output with report_format, list passing links with show_ok. It does NOT fetch external URLs or check that relative paths exist on disk — every check is local and deterministic. Max 1 MB of Markdown.",
        parameters = schema_json()
    ),
)]
impl MarkdownLinkCheck {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-link-check", |a: Args| {
            check_run(
                &a.markdown,
                &a.link_kind,
                &a.report_format,
                a.show_ok,
                a.check_anchors,
                a.flag_insecure,
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
                    "markdown": {
                        "type": "string",
                        "description": "The Markdown document to check, e.g. the contents of README.md. Max 1 MB."
                    },
                    "link_kind": {
                        "type": "string",
                        "enum": ["all", "anchor", "external", "relative", "mailto", "image", "empty", "reference"],
                        "default": "all",
                        "description": "Only report links of this kind. all (default) = every link; anchor = #in-document targets; external = http/https and other schemes; relative = paths like ./docs/a.md; mailto = mail addresses; image = ![alt](src); empty = links with no target; reference = [text][label] links whose definition is missing."
                    },
                    "report_format": {
                        "type": "string",
                        "enum": ["text", "markdown", "json"],
                        "default": "text",
                        "description": "How to render the report. text (default) = one 'line:col severity rule message' per finding; markdown = a table you can paste into an issue or PR; json = a machine-readable object with checked/errors/warnings/issues/links for CI."
                    },
                    "show_ok": {
                        "type": "boolean",
                        "default": false,
                        "description": "Also list every link that passed, with its kind and target, not just the problems. Default false (problems only)."
                    },
                    "check_anchors": {
                        "type": "boolean",
                        "default": true,
                        "description": "Verify that each #anchor target matches a heading id this document actually produces (GitHub-style slugs, duplicate headings numbered -1/-2, plus {#custom-id} and HTML id=\"…\" anchors). Default true; set false to ignore in-document anchors."
                    },
                    "flag_insecure": {
                        "type": "boolean",
                        "default": false,
                        "description": "Also warn about http:// links that could be https:// (rule ML011). Default false, since docs often cite http:// URLs deliberately."
                    }
                },
                "required": ["markdown"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
