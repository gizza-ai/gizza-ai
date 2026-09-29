//! gizza-ai/xml-namespace-stripper — chat skill block on the shared tool
//! abstraction. Removes XML namespace declarations (`xmlns`, `xmlns:prefix`)
//! and element/attribute name prefixes with a streaming quick-xml rewrite, so
//! comments, processing instructions, CDATA, the DOCTYPE, the prolog, text and
//! attribute values all round-trip untouched. The chat schema is single-sourced
//! from `descriptor()` (which also drives the CLI and the page's query params);
//! `handle()` delegates to `block_utils::run_skill`. Pure compute — the XML is
//! parsed in the sandbox, nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xml: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    keep: String,
    #[serde(default)]
    conflicts: String,
    #[serde(default = "default_true")]
    remove_schema_hints: bool,
    #[serde(default)]
    format: String,
    #[serde(default = "default_indent")]
    indent: u32,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}
fn default_indent() -> u32 {
    2
}

/// Single source for the chat schema, the CLI and the page's query params.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("xml")
                .required()
                .describe("The XML document to strip, as text. Everything that is not namespace plumbing is preserved: comments, processing instructions, CDATA sections, the DOCTYPE, the '<?xml …?>' prolog, text nodes, and attribute values with their entities and character references intact. Input must be well-formed XML — a mismatched or unclosed tag is reported with its byte position rather than silently half-processed. Max 5,000,000 bytes."),
        )
        .param(
            Param::enumv("mode", ["all", "prefixes", "declarations"])
                .default("all")
                .describe("What to remove. 'all' (default) removes both the xmlns declarations and the element/attribute name prefixes — the usual goal, leaving plain prefix-free XML that XPath and XML-to-JSON converters can address directly. 'prefixes' rewrites the names only and leaves the xmlns attributes in place (the declarations become unused but harmless). 'declarations' deletes only the xmlns attributes and keeps the prefixed names, which is what you want when re-parenting a fragment under a document that declares the same prefixes."),
        )
        .param(
            Param::string("keep")
                .default("")
                .describe("Comma-separated prefixes to leave alone, e.g. 'soap' or 'soap,wsse' — useful for flattening a payload while the envelope it travels in stays addressable. A kept prefix keeps its own xmlns declaration too, so the result is still namespace-well-formed. Use the literal token 'xmlns' to keep the default (unprefixed) declaration, which has no prefix of its own to name. Blank (the default) keeps nothing. Prefixes are case-sensitive, as in XML."),
        )
        .param(
            Param::enumv("conflicts", ["rename", "first", "error"])
                .default("rename")
                .describe("What to do when two attributes on one element collapse onto the same name after their prefixes are dropped — 'a:id' beside 'b:id', or 'xsi:type' beside a plain 'type'. 'rename' (default) keeps both: the first takes the bare local name and each later one becomes 'prefix_name' (so 'b:id' becomes 'b_id'), with a '_2', '_3' suffix if even that is taken. 'first' keeps the first and drops the later ones. 'error' refuses and names the clashing pair. Plain local-name() stylesheets emit duplicate attribute names here, which is not well-formed XML."),
        )
        .param(
            Param::boolean("remove_schema_hints")
                .default(true)
                .describe("Also drop 'schemaLocation' and 'noNamespaceSchemaLocation' attributes whose prefix actually resolves to the XML-Schema-instance namespace (http://www.w3.org/2001/XMLSchema-instance). On by default, because those attributes map namespace URIs to .xsd locations and reference namespaces a stripped document no longer declares. Matching is by resolved namespace, not by spelling, so an attribute on a prefix that merely happens to be called 'xsi' but is bound elsewhere is treated as data and kept. Turn it off to keep the hints (with their prefixes stripped like any other attribute)."),
        )
        .param(
            Param::enumv("format", ["preserve", "pretty", "minify"])
                .default("preserve")
                .describe("How to lay out the result. 'preserve' (default) keeps the document's own whitespace, so the output is the input minus the namespace plumbing and nothing else — the safe choice for diffing against the original. 'pretty' re-indents with 'indent' spaces per level; 'minify' collapses everything onto one line. Both reflowing modes drop whitespace-only text nodes, so avoid them for mixed content such as '<p>hello <b>there</b></p>' where the spacing around inline elements is part of the text."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(16.0)
                .default(2)
                .describe("Spaces per indent level when format=pretty (0-16, default 2). Ignored by 'preserve' and 'minify'."),
        )
        .param(
            Param::enumv("output", ["xml", "report"])
                .default("xml")
                .describe("What to return: 'xml' (default) is the stripped document; 'report' is a metric,value CSV — the mode used, declarations_removed, element_prefixes_stripped, attribute_prefixes_stripped, schema_references_removed, attribute_clashes_resolved and input/output bytes — followed by a prefix,namespace_uri table naming every declaration that was removed and the namespace it pointed at. Run 'report' first to see what a strip would cost before trusting it on a document you cannot re-fetch."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xml-namespace-stripper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip XML namespace declarations and prefixes",
    skill(
        description = "Remove XML namespace declarations (xmlns, xmlns:prefix) and element/attribute name prefixes, so namespaced XML from SOAP responses, Office documents and Atom/RSS feeds becomes plain prefix-free XML that XPath expressions and XML-to-JSON converters can address directly. mode=all (default) strips declarations and prefixes; mode=prefixes rewrites names only; mode=declarations deletes the xmlns attributes only. Comments, processing instructions, CDATA, the DOCTYPE, the prolog, text and attribute entities are preserved, and with format=preserve the output is byte-for-byte the input minus the namespace plumbing. keep=soap,wsse leaves chosen prefixes (and their declarations) alone; conflicts=rename|first|error decides what happens when two attributes collapse onto one name; remove_schema_hints drops dangling xsi:schemaLocation references; output=report returns a CSV of exactly what was removed. The reserved xml: prefix (xml:lang, xml:space, xml:id) is always preserved. Runs locally — the XML never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xml-namespace-stripper", |a: Args| {
            gizza_ai_xml_namespace_stripper_core::strip(
                &a.xml,
                &a.mode,
                &a.keep,
                &a.conflicts,
                a.remove_schema_hints,
                &a.format,
                a.indent as usize,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "xml": { "type": "string", "description": "The XML document to strip, as text. Everything that is not namespace plumbing is preserved: comments, processing instructions, CDATA sections, the DOCTYPE, the '<?xml …?>' prolog, text nodes, and attribute values with their entities and character references intact. Input must be well-formed XML — a mismatched or unclosed tag is reported with its byte position rather than silently half-processed. Max 5,000,000 bytes." },
                    "mode": { "type": "string", "enum": ["all", "prefixes", "declarations"], "default": "all", "description": "What to remove. 'all' (default) removes both the xmlns declarations and the element/attribute name prefixes — the usual goal, leaving plain prefix-free XML that XPath and XML-to-JSON converters can address directly. 'prefixes' rewrites the names only and leaves the xmlns attributes in place (the declarations become unused but harmless). 'declarations' deletes only the xmlns attributes and keeps the prefixed names, which is what you want when re-parenting a fragment under a document that declares the same prefixes." },
                    "keep": { "type": "string", "default": "", "description": "Comma-separated prefixes to leave alone, e.g. 'soap' or 'soap,wsse' — useful for flattening a payload while the envelope it travels in stays addressable. A kept prefix keeps its own xmlns declaration too, so the result is still namespace-well-formed. Use the literal token 'xmlns' to keep the default (unprefixed) declaration, which has no prefix of its own to name. Blank (the default) keeps nothing. Prefixes are case-sensitive, as in XML." },
                    "conflicts": { "type": "string", "enum": ["rename", "first", "error"], "default": "rename", "description": "What to do when two attributes on one element collapse onto the same name after their prefixes are dropped — 'a:id' beside 'b:id', or 'xsi:type' beside a plain 'type'. 'rename' (default) keeps both: the first takes the bare local name and each later one becomes 'prefix_name' (so 'b:id' becomes 'b_id'), with a '_2', '_3' suffix if even that is taken. 'first' keeps the first and drops the later ones. 'error' refuses and names the clashing pair. Plain local-name() stylesheets emit duplicate attribute names here, which is not well-formed XML." },
                    "remove_schema_hints": { "type": "boolean", "default": true, "description": "Also drop 'schemaLocation' and 'noNamespaceSchemaLocation' attributes whose prefix actually resolves to the XML-Schema-instance namespace (http://www.w3.org/2001/XMLSchema-instance). On by default, because those attributes map namespace URIs to .xsd locations and reference namespaces a stripped document no longer declares. Matching is by resolved namespace, not by spelling, so an attribute on a prefix that merely happens to be called 'xsi' but is bound elsewhere is treated as data and kept. Turn it off to keep the hints (with their prefixes stripped like any other attribute)." },
                    "format": { "type": "string", "enum": ["preserve", "pretty", "minify"], "default": "preserve", "description": "How to lay out the result. 'preserve' (default) keeps the document's own whitespace, so the output is the input minus the namespace plumbing and nothing else — the safe choice for diffing against the original. 'pretty' re-indents with 'indent' spaces per level; 'minify' collapses everything onto one line. Both reflowing modes drop whitespace-only text nodes, so avoid them for mixed content such as '<p>hello <b>there</b></p>' where the spacing around inline elements is part of the text." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 16, "default": 2, "description": "Spaces per indent level when format=pretty (0-16, default 2). Ignored by 'preserve' and 'minify'." },
                    "output": { "type": "string", "enum": ["xml", "report"], "default": "xml", "description": "What to return: 'xml' (default) is the stripped document; 'report' is a metric,value CSV — the mode used, declarations_removed, element_prefixes_stripped, attribute_prefixes_stripped, schema_references_removed, attribute_clashes_resolved and input/output bytes — followed by a prefix,namespace_uri table naming every declaration that was removed and the namespace it pointed at. Run 'report' first to see what a strip would cost before trusting it on a document you cannot re-fetch." }
                },
                "required": ["xml"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
