//! gizza-ai/ris-bibtex-converter — chat skill block on the shared tool abstraction.
//! Converts bibliographic records between RIS and BibTeX in both directions. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill, which calls the shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_key_style")]
    key_style: String,
    #[serde(default = "yes")]
    include_abstract: bool,
    #[serde(default = "yes")]
    include_keywords: bool,
    #[serde(default = "yes")]
    translate_latex: bool,
    #[serde(default = "default_indent")]
    indent: u32,
    #[serde(default = "default_sort")]
    sort: String,
}

fn yes() -> bool {
    true
}
fn default_direction() -> String {
    "auto".into()
}
fn default_key_style() -> String {
    "author-year-word".into()
}
fn default_indent() -> u32 {
    2
}
fn default_sort() -> String {
    "source".into()
}

fn run_tool(a: Args) -> Result<String, String> {
    gizza_ai_ris_bibtex_converter_core::convert_str(
        &a.input,
        &a.direction,
        &a.key_style,
        a.include_abstract,
        a.include_keywords,
        a.translate_latex,
        &a.indent.to_string(),
        &a.sort,
    )
}

/// Single source for the chat schema (and the CLI). Every param is described so
/// a model can pick values without reading the page.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .multiline()
                .describe("The bibliography to convert: either RIS records (each one a 'TY  - JOUR' line, tag lines such as 'AU  - Shannon, C. E.', and a closing 'ER  - ') or BibTeX entries (@article{key, field = {value}, ...}). Wrapped RIS continuation lines, @string macros and # concatenation are all handled. Max 1,000,000 bytes."),
        )
        .param(
            Param::enumv("direction", ["auto", "ris-to-bibtex", "bibtex-to-ris"])
                .default("auto")
                .describe("Which way to convert: 'auto' (default) sniffs the input — a 'TY  - ' line means RIS, an '@article{' line means BibTeX — and emits the other format; 'ris-to-bibtex' and 'bibtex-to-ris' force the direction, which is useful when the input is unusual or you want a parse error instead of a guess."),
        )
        .param(
            Param::enumv("key_style", ["author-year-word", "author-year", "ris-id", "numeric"])
                .default("author-year-word")
                .describe("How the BibTeX cite key is invented when converting RIS to BibTeX (RIS carries no cite key): 'author-year-word' (default) gives shannon1948mathematical — first author's family name, year, first significant title word; 'author-year' gives shannon1948; 'ris-id' reuses the record's 'ID' tag when it has one and falls back to author-year-word; 'numeric' gives ref1, ref2, ... A repeated key gets a trailing a, b, c. Ignored when converting BibTeX to RIS, where the existing cite key becomes the 'ID' tag."),
        )
        .param(
            Param::boolean("include_abstract")
                .default(true)
                .describe("Carry the abstract across ('AB'/'N2' becomes 'abstract' and back). Default true. Turn it off for a compact bibliography — abstracts are long and most citation styles never print them."),
        )
        .param(
            Param::boolean("include_keywords")
                .default(true)
                .describe("Carry keywords across: every RIS 'KW' tag joins into one comma-separated BibTeX 'keywords' field, and a BibTeX 'keywords' field splits back into one 'KW' tag per term. Default true."),
        )
        .param(
            Param::boolean("translate_latex")
                .default(true)
                .describe("Translate markup between the two conventions. Converting BibTeX to RIS, decode LaTeX accents and protective braces to plain UTF-8 (\\\"a becomes ä, {DNA} becomes DNA). Converting RIS to BibTeX, escape the characters that are special to LaTeX (& % $ # _ { } ~ ^ \\\\) so the .bib compiles; 'url' and 'doi' values are left alone so they stay clickable. Default true. Turn it off to pass every value through verbatim."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(0.0)
                .max(16.0)
                .describe("Spaces before each field line in the generated BibTeX, 0-16. Default 2. Ignored when converting BibTeX to RIS, which has a fixed 'TAG  - value' line format."),
        )
        .param(
            Param::enumv("sort", ["source", "key", "year", "type"])
                .default("source")
                .describe("Order of the emitted records: 'source' (default) keeps the input order; 'key' sorts case-insensitively by cite key; 'year' sorts ascending by publication year with undated records last; 'type' groups by entry type then key."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ris-bibtex-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert bibliographic records between RIS and BibTeX in both directions.",
    skill(
        description = "Convert a bibliography between RIS (the EndNote/Zotero/Mendeley/PubMed export format: 'TY  - JOUR' … 'ER  - ') and BibTeX (@article{key, ...}), in either direction. Leave direction='auto' (default) to sniff the input and emit the other format, or force 'ris-to-bibtex'/'bibtex-to-ris'. Reference types map both ways (JOUR↔article, BOOK↔book, CHAP↔incollection, CONF↔inproceedings, THES↔phdthesis/mastersthesis, RPRT↔techreport, UNPB↔unpublished, ELEC/GEN↔misc), as do the common fields: authors and editors, title, journal or booktitle, series, volume, issue, SP/EP↔pages, publisher (school for a thesis, institution for a report), place, edition, DOI, URL, access date, ISBN/ISSN, language, keywords, abstract and notes. Because RIS has no cite key, key_style picks how one is invented ('author-year-word' default, 'author-year', 'ris-id', 'numeric'; duplicates get a trailing letter). translate_latex decodes LaTeX accents and braces on the way into RIS and escapes LaTeX-special characters on the way into BibTeX. include_abstract and include_keywords drop those fields, indent sets BibTeX field padding (0-16), and sort reorders records by key, year or type. Runs locally in the sandbox; nothing is uploaded. Max 1,000,000 bytes of input.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ris-bibtex-converter", |a: Args| {
            run_tool(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &str) -> Args {
        Args {
            input: input.into(),
            direction: default_direction(),
            key_style: default_key_style(),
            include_abstract: true,
            include_keywords: true,
            translate_latex: true,
            indent: default_indent(),
            sort: default_sort(),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The bibliography to convert: either RIS records (each one a 'TY  - JOUR' line, tag lines such as 'AU  - Shannon, C. E.', and a closing 'ER  - ') or BibTeX entries (@article{key, field = {value}, ...}). Wrapped RIS continuation lines, @string macros and # concatenation are all handled. Max 1,000,000 bytes." },
                    "direction": { "type": "string", "enum": ["auto", "ris-to-bibtex", "bibtex-to-ris"], "default": "auto", "description": "Which way to convert: 'auto' (default) sniffs the input — a 'TY  - ' line means RIS, an '@article{' line means BibTeX — and emits the other format; 'ris-to-bibtex' and 'bibtex-to-ris' force the direction, which is useful when the input is unusual or you want a parse error instead of a guess." },
                    "key_style": { "type": "string", "enum": ["author-year-word", "author-year", "ris-id", "numeric"], "default": "author-year-word", "description": "How the BibTeX cite key is invented when converting RIS to BibTeX (RIS carries no cite key): 'author-year-word' (default) gives shannon1948mathematical — first author's family name, year, first significant title word; 'author-year' gives shannon1948; 'ris-id' reuses the record's 'ID' tag when it has one and falls back to author-year-word; 'numeric' gives ref1, ref2, ... A repeated key gets a trailing a, b, c. Ignored when converting BibTeX to RIS, where the existing cite key becomes the 'ID' tag." },
                    "include_abstract": { "type": "boolean", "default": true, "description": "Carry the abstract across ('AB'/'N2' becomes 'abstract' and back). Default true. Turn it off for a compact bibliography — abstracts are long and most citation styles never print them." },
                    "include_keywords": { "type": "boolean", "default": true, "description": "Carry keywords across: every RIS 'KW' tag joins into one comma-separated BibTeX 'keywords' field, and a BibTeX 'keywords' field splits back into one 'KW' tag per term. Default true." },
                    "translate_latex": { "type": "boolean", "default": true, "description": "Translate markup between the two conventions. Converting BibTeX to RIS, decode LaTeX accents and protective braces to plain UTF-8 (\\\"a becomes ä, {DNA} becomes DNA). Converting RIS to BibTeX, escape the characters that are special to LaTeX (& % $ # _ { } ~ ^ \\\\) so the .bib compiles; 'url' and 'doi' values are left alone so they stay clickable. Default true. Turn it off to pass every value through verbatim." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 16, "default": 2, "description": "Spaces before each field line in the generated BibTeX, 0-16. Default 2. Ignored when converting BibTeX to RIS, which has a fixed 'TAG  - value' line format." },
                    "sort": { "type": "string", "enum": ["source", "key", "year", "type"], "default": "source", "description": "Order of the emitted records: 'source' (default) keeps the input order; 'key' sorts case-insensitively by cite key; 'year' sorts ascending by publication year with undated records last; 'type' groups by entry type then key." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }

    #[test]
    fn descriptor_describes_every_param() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived["properties"].as_object().unwrap();
        assert_eq!(props.len(), 8);
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "param '{name}' needs a real .describe()");
        }
    }

    #[test]
    fn run_tool_converts_ris_with_chat_defaults() {
        let got = run_tool(args(
            "TY  - JOUR\nAU  - Shannon, C. E.\nTI  - A Mathematical Theory of Communication\nJO  - Bell System Technical Journal\nPY  - 1948\nSP  - 379\nEP  - 423\nER  - \n",
        ))
        .unwrap();
        assert_eq!(
            got,
            "@article{shannon1948mathematical,\n  author = {Shannon, C. E.},\n  title = {A Mathematical Theory of Communication},\n  journal = {Bell System Technical Journal},\n  pages = {379--423},\n  year = {1948}\n}"
        );
    }

    #[test]
    fn run_tool_converts_bibtex_with_chat_defaults() {
        let got = run_tool(args(
            "@book{knuth1984, title = {The {TeX}book}, author = {Knuth, Donald E.}, publisher = {Addison-Wesley}, year = {1984}}",
        ))
        .unwrap();
        assert_eq!(
            got,
            "TY  - BOOK\nID  - knuth1984\nAU  - Knuth, Donald E.\nTI  - The TeXbook\nPY  - 1984\nPB  - Addison-Wesley\nER  - "
        );
    }

    #[test]
    fn run_tool_surfaces_core_errors() {
        let err = run_tool(args("a plain sentence, not a bibliography")).unwrap_err();
        assert!(
            err.contains("could not tell whether the input is RIS or BibTeX"),
            "{err}"
        );
    }
}
