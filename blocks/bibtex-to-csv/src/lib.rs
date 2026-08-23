//! gizza-ai/bibtex-to-csv — chat skill block on the shared tool abstraction.
//! Turns a pasted BibTeX bibliography into a CSV table, one row per entry. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill, which calls the shared core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    bibtex: String,
    #[serde(default = "default_columns")]
    columns: String,
    #[serde(default)]
    custom_columns: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "yes")]
    header: bool,
    #[serde(default = "yes")]
    decode_latex: bool,
    #[serde(default = "default_author_format")]
    author_format: String,
    #[serde(default = "default_author_separator")]
    author_separator: String,
    #[serde(default = "yes")]
    expand_strings: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default)]
    bom: bool,
}

fn yes() -> bool {
    true
}
fn default_columns() -> String {
    "standard".into()
}
fn default_delimiter() -> String {
    "comma".into()
}
fn default_author_format() -> String {
    "bibtex".into()
}
fn default_author_separator() -> String {
    "and".into()
}
fn default_sort() -> String {
    "source".into()
}

fn run_tool(a: Args) -> Result<String, String> {
    gizza_ai_bibtex_to_csv_core::convert_str(
        &a.bibtex,
        &a.columns,
        &a.custom_columns,
        &a.delimiter,
        a.header,
        a.decode_latex,
        &a.author_format,
        &a.author_separator,
        a.expand_strings,
        &a.sort,
        a.bom,
    )
}

/// Single source for the chat schema (and the CLI). Every param is described so
/// a model can pick values without reading the page.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("bibtex")
                .required()
                .multiline()
                .describe("The BibTeX source to convert, i.e. the contents of a .bib file. Every @type{key, field = value, ...} entry becomes one CSV row; @string, @comment and @preamble items and any free text between entries are handled but never emitted as rows. Values in {braces}, \"quotes\", bare numbers and # concatenation are all accepted. Max 1,000,000 bytes."),
        )
        .param(
            Param::enumv("columns", ["standard", "all", "custom"])
                .default("standard")
                .describe("Which columns the CSV carries: 'standard' (default) emits the fixed bibliographic set type,key,title,author,year,journal,booktitle,volume,number,pages,publisher,doi,isbn,issn,url with empty cells for fields an entry lacks; 'all' emits type,key then every field name that appears anywhere in the file, alphabetically; 'custom' emits exactly the columns named in custom_columns, in that order."),
        )
        .param(
            Param::string("custom_columns")
                .default("")
                .describe("Comma-separated column names used only when columns=custom, e.g. 'key,title,author,year,doi'. Names are matched case-insensitively against BibTeX field names; 'type' (the entry type) and 'key' (the cite key) are accepted as virtual columns. A name no entry has becomes an empty column. Max 200 columns. Ignored unless columns=custom."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("Field separator for the output: 'comma' (default, plain .csv), 'semicolon' (what Excel expects in comma-decimal locales), 'tab' (.tsv) or 'pipe'. Cells containing the separator, a double quote, CR or LF are quoted and internal quotes doubled, per RFC 4180."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit the column-name header row as the first line. Default true. Turn it off when appending the result to an existing CSV that already has a header."),
        )
        .param(
            Param::boolean("decode_latex")
                .default(true)
                .describe("Decode LaTeX markup to plain UTF-8 text: accent macros (\\\"a becomes ä, \\'e becomes é, \\c{c} becomes ç), ligatures and symbols (\\ss, \\ae, \\o, \\ldots), escaped literals (\\& \\% \\#), en/em dashes (-- and ---) and `` '' quotes, and protective braces are dropped so {DNA} becomes DNA. Default true. Turn it off to keep the source spelling verbatim."),
        )
        .param(
            Param::enumv("author_format", ["bibtex", "last-first", "first-last"])
                .default("bibtex")
                .describe("How each name in the author, editor and translator fields is written: 'bibtex' (default) keeps the source spelling; 'last-first' normalises to 'Curie, Marie'; 'first-last' normalises to 'Marie Curie'. Lowercase particles (van, von, de) stay with the last name and a brace-protected corporate name such as {The MIT Press} is left whole."),
        )
        .param(
            Param::enumv("author_separator", ["and", "semicolon", "comma", "pipe"])
                .default("and")
                .describe("What joins two names inside one author/editor cell: 'and' (default) keeps the BibTeX ' and ' spelling, 'semicolon' uses '; ', 'comma' uses ', ' and 'pipe' uses ' | '. Use semicolon or pipe when the names are written last-first, so the comma inside each name stays unambiguous."),
        )
        .param(
            Param::boolean("expand_strings")
                .default(true)
                .describe("Resolve @string macros and # concatenation before emitting, so @string{jcp = \"J. Chem. Phys.\"} plus journal = jcp # \" (Letters)\" yields 'J. Chem. Phys. (Letters)'. Default true. Turn it off to keep the unresolved macro name in the cell."),
        )
        .param(
            Param::enumv("sort", ["source", "key", "year", "type"])
                .default("source")
                .describe("Row order: 'source' (default) keeps the order of the .bib file; 'key' sorts case-insensitively by cite key; 'year' sorts ascending by year with entries lacking a parseable year last; 'type' groups by entry type then cite key."),
        )
        .param(
            Param::boolean("bom")
                .default(false)
                .describe("Prepend a UTF-8 byte-order mark so Excel opens the file as UTF-8 instead of mangling accented characters. Default false, because a BOM confuses parsers that do not strip it. Turn it on only when the CSV is headed straight for Excel."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bibtex-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a BibTeX bibliography to a CSV table, one row per entry.",
    skill(
        description = "Convert BibTeX (.bib) source into CSV, one row per @type{key, ...} entry. Choose the standard bibliographic column set, the union of every field present, or your own ordered column list; pick the delimiter (comma, semicolon, tab, pipe) and whether to emit a header row. LaTeX accent macros and protective braces are decoded to UTF-8 by default ({DNA} stays DNA, \\'e becomes é), @string macros and # concatenation are resolved, author names can be normalised to 'Last, First' or 'First Last' with a chosen separator, rows can be sorted by key, year or type, and a UTF-8 BOM can be added for Excel. Output is RFC-4180 quoted. Runs locally in the sandbox; nothing is uploaded. Max 1,000,000 bytes of input.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bibtex-to-csv", |a: Args| {
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "bibtex": { "type": "string", "description": "The BibTeX source to convert, i.e. the contents of a .bib file. Every @type{key, field = value, ...} entry becomes one CSV row; @string, @comment and @preamble items and any free text between entries are handled but never emitted as rows. Values in {braces}, \"quotes\", bare numbers and # concatenation are all accepted. Max 1,000,000 bytes." },
                    "columns": { "type": "string", "enum": ["standard", "all", "custom"], "default": "standard", "description": "Which columns the CSV carries: 'standard' (default) emits the fixed bibliographic set type,key,title,author,year,journal,booktitle,volume,number,pages,publisher,doi,isbn,issn,url with empty cells for fields an entry lacks; 'all' emits type,key then every field name that appears anywhere in the file, alphabetically; 'custom' emits exactly the columns named in custom_columns, in that order." },
                    "custom_columns": { "type": "string", "default": "", "description": "Comma-separated column names used only when columns=custom, e.g. 'key,title,author,year,doi'. Names are matched case-insensitively against BibTeX field names; 'type' (the entry type) and 'key' (the cite key) are accepted as virtual columns. A name no entry has becomes an empty column. Max 200 columns. Ignored unless columns=custom." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "Field separator for the output: 'comma' (default, plain .csv), 'semicolon' (what Excel expects in comma-decimal locales), 'tab' (.tsv) or 'pipe'. Cells containing the separator, a double quote, CR or LF are quoted and internal quotes doubled, per RFC 4180." },
                    "header": { "type": "boolean", "default": true, "description": "Emit the column-name header row as the first line. Default true. Turn it off when appending the result to an existing CSV that already has a header." },
                    "decode_latex": { "type": "boolean", "default": true, "description": "Decode LaTeX markup to plain UTF-8 text: accent macros (\\\"a becomes ä, \\'e becomes é, \\c{c} becomes ç), ligatures and symbols (\\ss, \\ae, \\o, \\ldots), escaped literals (\\& \\% \\#), en/em dashes (-- and ---) and `` '' quotes, and protective braces are dropped so {DNA} becomes DNA. Default true. Turn it off to keep the source spelling verbatim." },
                    "author_format": { "type": "string", "enum": ["bibtex", "last-first", "first-last"], "default": "bibtex", "description": "How each name in the author, editor and translator fields is written: 'bibtex' (default) keeps the source spelling; 'last-first' normalises to 'Curie, Marie'; 'first-last' normalises to 'Marie Curie'. Lowercase particles (van, von, de) stay with the last name and a brace-protected corporate name such as {The MIT Press} is left whole." },
                    "author_separator": { "type": "string", "enum": ["and", "semicolon", "comma", "pipe"], "default": "and", "description": "What joins two names inside one author/editor cell: 'and' (default) keeps the BibTeX ' and ' spelling, 'semicolon' uses '; ', 'comma' uses ', ' and 'pipe' uses ' | '. Use semicolon or pipe when the names are written last-first, so the comma inside each name stays unambiguous." },
                    "expand_strings": { "type": "boolean", "default": true, "description": "Resolve @string macros and # concatenation before emitting, so @string{jcp = \"J. Chem. Phys.\"} plus journal = jcp # \" (Letters)\" yields 'J. Chem. Phys. (Letters)'. Default true. Turn it off to keep the unresolved macro name in the cell." },
                    "sort": { "type": "string", "enum": ["source", "key", "year", "type"], "default": "source", "description": "Row order: 'source' (default) keeps the order of the .bib file; 'key' sorts case-insensitively by cite key; 'year' sorts ascending by year with entries lacking a parseable year last; 'type' groups by entry type then cite key." },
                    "bom": { "type": "boolean", "default": false, "description": "Prepend a UTF-8 byte-order mark so Excel opens the file as UTF-8 instead of mangling accented characters. Default false, because a BOM confuses parsers that do not strip it. Turn it on only when the CSV is headed straight for Excel." }
                },
                "required": ["bibtex"],
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
        assert_eq!(props.len(), 11);
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "param '{name}' needs a real .describe()");
        }
    }

    #[test]
    fn run_tool_converts_with_chat_defaults() {
        let got = run_tool(Args {
            bibtex: "@article{curie1898, title = {Sur une substance nouvelle}, author = {Curie, Marie}, year = {1898}}".into(),
            columns: default_columns(),
            custom_columns: String::new(),
            delimiter: default_delimiter(),
            header: true,
            decode_latex: true,
            author_format: default_author_format(),
            author_separator: default_author_separator(),
            expand_strings: true,
            sort: default_sort(),
            bom: false,
        })
        .unwrap();
        assert_eq!(
            got,
            "type,key,title,author,year,journal,booktitle,volume,number,pages,publisher,doi,isbn,issn,url\n\
article,curie1898,Sur une substance nouvelle,\"Curie, Marie\",1898,,,,,,,,,,"
        );
    }

    #[test]
    fn run_tool_surfaces_core_errors() {
        let err = run_tool(Args {
            bibtex: "not a bibliography".into(),
            columns: default_columns(),
            custom_columns: String::new(),
            delimiter: default_delimiter(),
            header: true,
            decode_latex: true,
            author_format: default_author_format(),
            author_separator: default_author_separator(),
            expand_strings: true,
            sort: default_sort(),
            bom: false,
        })
        .unwrap_err();
        assert!(err.contains("no BibTeX entries found"), "{err}");
    }
}
