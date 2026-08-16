//! gizza-ai/html-comment-stripper — chat skill block on the shared tool abstraction.
//! Removes `<!-- … -->` comments from markup with a raw-text- and quote-aware
//! scanner, keeping conditional / SSI / banner comments by default. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI);
//! `handle()` delegates to `block_utils::run_skill`. Pure compute — the markup
//! is scanned in the sandbox, nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default = "default_true")]
    keep_conditional: bool,
    #[serde(default = "default_true")]
    keep_ssi: bool,
    #[serde(default = "default_true")]
    keep_bang: bool,
    #[serde(default)]
    pattern: String,
    #[serde(default)]
    pattern_mode: String,
    #[serde(default)]
    remove_css_comments: bool,
    #[serde(default)]
    blank_lines: String,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The markup to clean, as text. The output is the input MINUS the comment bytes — nothing else is touched: no whitespace collapsing, no tag or attribute rewriting, no re-indentation. The scanner is raw-text- and quote-aware, so a '<!--' inside <script>, <style>, <textarea>, <title> or inside a quoted attribute value is not a comment; comments do not nest, so '<!-- a <!-- b --> c -->' ends at the FIRST '-->'; and an unterminated comment is an error rather than a silent truncation of the rest of the document. Max 5,000,000 bytes."),
        )
        .param(
            Param::boolean("keep_conditional")
                .default(true)
                .describe("Keep Internet Explorer conditional comments — '<!--[if lt IE 9]> … <![endif]-->' and the downlevel-revealed split forms. On by default, because deleting them changes which stylesheets and scripts legacy browsers load; turn it off to strip IE fallbacks along with everything else."),
        )
        .param(
            Param::boolean("keep_ssi")
                .default(true)
                .describe("Keep server-side include directives — any comment whose text starts with '#', such as '<!--#include virtual=… -->' or '<!--#echo var=… -->'. On by default: these are instructions to the web server rather than notes, so removing them silently drops part of the rendered page."),
        )
        .param(
            Param::boolean("keep_bang")
                .default(true)
                .describe("Keep bang (banner) comments — any comment whose text starts with '!', such as '<!--! (c) 2026 Example Ltd, MIT licence -->'. That marker is the industry convention for licence headers and other must-keep notices, so it is on by default; turn it off only when you know the banners are disposable."),
        )
        .param(
            Param::string("pattern")
                .default("")
                .describe("Optional Rust regular expression matched against each comment's INNER text — what sits between '<!--' and '-->', delimiters excluded. Blank (the default) disables it. Under pattern_mode 'keep' a match PROTECTS the comment from removal; under 'only' a match is the only thing removed. Character classes, alternation, groups, quantifiers and anchors all work; there are no backreferences or lookaround, which is what keeps matching linear-time."),
        )
        .param(
            Param::enumv("pattern_mode", ["keep", "only"])
                .default("keep")
                .describe("How 'pattern' is applied. 'keep' (default) treats it as a keep-list: a matching comment survives even when its kind would otherwise be removed. 'only' inverts the tool — ONLY matching comments are removed and every other comment is left alone, which is how you delete CMS block markers such as wp: or analytics placeholders without touching real notes. 'only' with a blank pattern is an error, since nothing would be removed."),
        )
        .param(
            Param::boolean("remove_css_comments")
                .default(false)
                .describe("Also strip '/* … */' comments from inside <style> blocks. Off by default, so a run is purely an HTML-comment operation. The CSS pass is string-aware: a '/*' inside a quoted CSS string such as content: … is left alone. Comments inside <script> are never touched — correct JavaScript comment removal needs a real lexer, because '//' can appear inside a string or a regex literal."),
        )
        .param(
            Param::enumv("blank_lines", ["keep", "trim", "collapse"])
                .default("keep")
                .describe("What to do with lines a removal left empty. 'keep' (default) changes no whitespace at all, so the output is byte-for-byte the input minus the comments. 'trim' drops lines that became blank because a comment was removed, while lines that were already blank in the input are preserved. 'collapse' does that and also folds runs of consecutive blank lines into a single one."),
        )
        .param(
            Param::enumv("output", ["html", "report", "comments"])
                .default("html")
                .describe("What to return: 'html' (default) is the cleaned markup; 'report' is a metric,value CSV with comments_found/removed/kept, a per-kind breakdown of what was removed, css_comments_removed, and bytes_before/after/saved plus percent_smaller; 'comments' is a line,kind,action,comment CSV listing every comment found with its 1-based line, its kind (plain, conditional, ssi or bang) and whether it was removed or kept — the dry run for checking a rule before trusting it on a real file."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-comment-stripper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip HTML comments while keeping conditional, SSI and banner comments, byte-identical otherwise.",
    skill(
        description = "Remove <!-- … --> comments from HTML while leaving every other byte exactly where it was — no whitespace collapsing, no tag or attribute rewriting, no re-indentation, unlike a minifier. The scanner is raw-text- and quote-aware, which is what a '<!--.*?-->' regex gets wrong: a '<!--' inside <script>, <style>, <textarea>, <title> or inside a quoted attribute value is not a comment, comments do not nest (the FIRST '-->' closes one), and an unterminated comment is reported as an error instead of truncating the document. Conditional ('<!--[if lt IE 9]> … <![endif]-->'), SSI ('<!--#include … -->') and bang/banner ('<!--! … -->') comments are recognized as kinds and KEPT by default via keep_conditional / keep_ssi / keep_bang. pattern is a regular expression over each comment's inner text: with pattern_mode 'keep' it protects matching comments, with 'only' it inverts the tool so ONLY matching comments are removed (for deleting CMS block markers such as wp: while keeping real notes). remove_css_comments additionally strips string-aware '/* … */' comments inside <style>; JavaScript comments are deliberately never touched. blank_lines is 'keep' (default, byte-exact), 'trim' (drop lines a removal emptied) or 'collapse' (also fold blank runs). output is 'html' (the cleaned markup), 'report' (a metric,value CSV of counts, per-kind breakdown and bytes saved) or 'comments' (a line,kind,action,comment CSV dry-run listing). Max 5,000,000 bytes. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-comment-stripper", |a: Args| {
            gizza_ai_html_comment_stripper_core::strip(
                &a.html,
                a.keep_conditional,
                a.keep_ssi,
                a.keep_bang,
                &a.pattern,
                &a.pattern_mode,
                a.remove_css_comments,
                &a.blank_lines,
                &a.output,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-08-16 for the initial html-comment-stripper release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "html": { "type": "string", "description": "The markup to clean, as text. The output is the input MINUS the comment bytes — nothing else is touched: no whitespace collapsing, no tag or attribute rewriting, no re-indentation. The scanner is raw-text- and quote-aware, so a '<!--' inside <script>, <style>, <textarea>, <title> or inside a quoted attribute value is not a comment; comments do not nest, so '<!-- a <!-- b --> c -->' ends at the FIRST '-->'; and an unterminated comment is an error rather than a silent truncation of the rest of the document. Max 5,000,000 bytes." },
                    "keep_conditional": { "type": "boolean", "default": true, "description": "Keep Internet Explorer conditional comments — '<!--[if lt IE 9]> … <![endif]-->' and the downlevel-revealed split forms. On by default, because deleting them changes which stylesheets and scripts legacy browsers load; turn it off to strip IE fallbacks along with everything else." },
                    "keep_ssi": { "type": "boolean", "default": true, "description": "Keep server-side include directives — any comment whose text starts with '#', such as '<!--#include virtual=… -->' or '<!--#echo var=… -->'. On by default: these are instructions to the web server rather than notes, so removing them silently drops part of the rendered page." },
                    "keep_bang": { "type": "boolean", "default": true, "description": "Keep bang (banner) comments — any comment whose text starts with '!', such as '<!--! (c) 2026 Example Ltd, MIT licence -->'. That marker is the industry convention for licence headers and other must-keep notices, so it is on by default; turn it off only when you know the banners are disposable." },
                    "pattern": { "type": "string", "default": "", "description": "Optional Rust regular expression matched against each comment's INNER text — what sits between '<!--' and '-->', delimiters excluded. Blank (the default) disables it. Under pattern_mode 'keep' a match PROTECTS the comment from removal; under 'only' a match is the only thing removed. Character classes, alternation, groups, quantifiers and anchors all work; there are no backreferences or lookaround, which is what keeps matching linear-time." },
                    "pattern_mode": { "type": "string", "enum": ["keep", "only"], "default": "keep", "description": "How 'pattern' is applied. 'keep' (default) treats it as a keep-list: a matching comment survives even when its kind would otherwise be removed. 'only' inverts the tool — ONLY matching comments are removed and every other comment is left alone, which is how you delete CMS block markers such as wp: or analytics placeholders without touching real notes. 'only' with a blank pattern is an error, since nothing would be removed." },
                    "remove_css_comments": { "type": "boolean", "default": false, "description": "Also strip '/* … */' comments from inside <style> blocks. Off by default, so a run is purely an HTML-comment operation. The CSS pass is string-aware: a '/*' inside a quoted CSS string such as content: … is left alone. Comments inside <script> are never touched — correct JavaScript comment removal needs a real lexer, because '//' can appear inside a string or a regex literal." },
                    "blank_lines": { "type": "string", "enum": ["keep", "trim", "collapse"], "default": "keep", "description": "What to do with lines a removal left empty. 'keep' (default) changes no whitespace at all, so the output is byte-for-byte the input minus the comments. 'trim' drops lines that became blank because a comment was removed, while lines that were already blank in the input are preserved. 'collapse' does that and also folds runs of consecutive blank lines into a single one." },
                    "output": { "type": "string", "enum": ["html", "report", "comments"], "default": "html", "description": "What to return: 'html' (default) is the cleaned markup; 'report' is a metric,value CSV with comments_found/removed/kept, a per-kind breakdown of what was removed, css_comments_removed, and bytes_before/after/saved plus percent_smaller; 'comments' is a line,kind,action,comment CSV listing every comment found with its 1-based line, its kind (plain, conditional, ssi or bang) and whether it was removed or kept — the dry run for checking a rule before trusting it on a real file." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
