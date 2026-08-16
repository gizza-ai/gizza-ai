//! gizza-ai/csv-regex-replace — chat skill block on the shared tool abstraction.
//! Applies one find-and-replace rule (regex or literal) to the cells of selected
//! columns of a parsed CSV table, with capture-group substitution. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI);
//! `handle()` delegates to `block_utils::run_skill`. Pure compute — the table is
//! parsed and rewritten in the sandbox, nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    pattern: String,
    #[serde(default)]
    replacement: String,
    #[serde(default)]
    columns: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    match_scope: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    dotall: bool,
    #[serde(default = "default_true")]
    replace_all: bool,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default)]
    include_header: bool,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default)]
    quote_style: String,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}
fn default_auto() -> String {
    "auto".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The CSV/delimited table to edit, as text. It is parsed before anything is matched, so a pattern only ever sees a decoded cell value — it can never match across a delimiter, eat a quote character, or split a quoted field that contains an embedded newline. Row order, column count and the delimiter are preserved; output quoting is re-derived. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("pattern")
                .required()
                .describe("What to find. By default this is a Rust regular expression — \\d, \\w, \\b, character classes, alternation, groups, quantifiers and named groups (?<name>...) all work; there are no backreferences or lookaround, which is what keeps matching linear-time. Set mode to 'literal' to match the text exactly as typed instead."),
        )
        .param(
            Param::string("replacement")
                .default("")
                .describe("What to put in place of each match. In regex mode $1 / ${1} / ${name} expand capture groups, $0 is the whole match and $$ is a literal dollar sign — write ${1}x rather than $1x when a group reference is followed by a word character, since $1x is read as a group named '1x'. In literal mode the text is inserted verbatim, dollar signs included. Leave it blank to DELETE every match."),
        )
        .param(
            Param::string("columns")
                .default("")
                .describe("Which columns the rule applies to. Blank (the default) or '*' means every column. Otherwise a comma-separated list of header names ('email'), 1-based indices ('2'), and inclusive index ranges ('2-4') — mixed freely, e.g. 'name,3,5-7'. Names are matched exactly first, then case-insensitively. Cells in unselected columns are copied through byte-for-byte."),
        )
        .param(
            Param::enumv("mode", ["regex", "literal"])
                .default("regex")
                .describe("How to read the pattern. 'regex' (default) compiles it as a regular expression and expands capture references in the replacement. 'literal' escapes every metacharacter, so '.' matches a real dot and '$1' in the replacement stays '$1' — the safe choice for pasted values that happen to contain regex punctuation."),
        )
        .param(
            Param::enumv("match_scope", ["substring", "whole_cell"])
                .default("substring")
                .describe("Where the pattern is allowed to match. 'substring' (default) matches anywhere inside a cell and can match several times. 'whole_cell' anchors the pattern to the ENTIRE cell value, so it either rewrites the whole cell or leaves it alone — use it for value remaps such as turning exactly 'NA' into an empty cell without touching 'NAME'."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match without regard to case (the regex 'i' flag), so 'error' also matches 'Error' and 'ERROR'. Applies to literal mode too. The replacement text is inserted exactly as written — the original casing is not restored."),
        )
        .param(
            Param::boolean("multiline")
                .default(false)
                .describe("The regex 'm' flag: ^ and $ also match at line breaks INSIDE a cell instead of only at its start and end. Only meaningful for quoted cells that contain embedded newlines. Whole-cell scope is unaffected — it anchors with \\A and \\z, which this flag cannot loosen."),
        )
        .param(
            Param::boolean("dotall")
                .default(false)
                .describe("The regex 's' flag: '.' also matches a newline, so a pattern can span an embedded line break inside a quoted cell. Off by default, matching the usual regex convention."),
        )
        .param(
            Param::boolean("replace_all")
                .default(true)
                .describe("When true (default) every match in a cell is replaced, like a regex 'g' flag. Turn it off to replace only the FIRST match in each cell and leave later ones intact."),
        )
        .param(
            Param::boolean("has_header")
                .default(true)
                .describe("When true (default) row 1 is a header: its names can be used in 'columns', it is excluded from replacement unless include_header is on, and it is kept at the top of a 'changed' listing. Turn it off for a headerless table — every row is then data and columns must be given as indices or ranges."),
        )
        .param(
            Param::boolean("include_header")
                .default(false)
                .describe("Also apply the rule to the header row itself. Off by default, so a pattern like 'code' rewrites the data without renaming the 'code' column. Ignored when has_header is false (there is no header to protect)."),
        )
        .param(
            Param::string("delimiter")
                .default("auto")
                .describe("Field separator: 'auto' (default) sniffs it from the first line, counting candidates outside quotes with comma winning a tie; or give a name ('comma', 'tab', 'semicolon', 'pipe') or any single character. The output is written with the same separator."),
        )
        .param(
            Param::enumv("quote_style", ["minimal", "always", "non_numeric"])
                .default("minimal")
                .describe("How the rewritten table is quoted. 'minimal' (default) quotes only fields that need it — one whose new value contains the delimiter, a quote or a newline is re-quoted automatically. 'always' quotes every field; 'non_numeric' quotes every field that is not a number."),
        )
        .param(
            Param::enumv("output", ["csv", "changed", "report"])
                .default("csv")
                .describe("What to return: 'csv' (default) is the whole table with the replacements applied; 'changed' is only the rows that actually changed, plus the header, for reviewing a rule before committing to it; 'report' is a per-column audit table 'column,cells_changed,replacements' with a TOTAL row and no data at all."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-regex-replace",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find and replace with a regex inside chosen CSV columns, with capture-group substitution.",
    skill(
        description = "Apply one find-and-replace rule to the cells of selected columns of a CSV table, with capture-group substitution in the replacement. The table is PARSED first, so the pattern only ever sees a decoded cell value: it cannot match across a delimiter, eat a quote character, or split a quoted field containing an embedded newline — the failure mode of running a regex over raw CSV text. Output quoting is re-derived, so a replacement that introduces a comma or a quote is re-quoted automatically. pattern is a Rust regular expression by default (mode = 'literal' matches it verbatim instead); replacement expands $1, ${name}, $0 and $$, and a blank replacement deletes every match. columns is blank for every column, or a mix of header names, 1-based indices and '2-4' ranges. match_scope = 'whole_cell' anchors the pattern to the entire cell for value remaps; ignore_case, multiline and dotall are the i/m/s flags; replace_all off replaces only the first match per cell; has_header and include_header control whether the header row is protected (it is, by default); delimiter is 'auto' or a named/single character; quote_style is minimal, always or non_numeric. output is 'csv' (the rewritten table), 'changed' (header plus only the rows that changed), or 'report' (a per-column column,cells_changed,replacements audit with a TOTAL row). Rust regex has no backreferences or lookaround, which keeps matching linear-time. Max 5,000,000 bytes. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-regex-replace", |a: Args| {
            gizza_ai_csv_regex_replace_core::replace(
                &a.data,
                &a.pattern,
                &a.replacement,
                &a.columns,
                &a.mode,
                &a.match_scope,
                a.ignore_case,
                a.multiline,
                a.dotall,
                a.replace_all,
                a.has_header,
                a.include_header,
                &a.delimiter,
                &a.quote_style,
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
    /// reviewed. Authored 2026-08-16 for the initial csv-regex-replace release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The CSV/delimited table to edit, as text. It is parsed before anything is matched, so a pattern only ever sees a decoded cell value — it can never match across a delimiter, eat a quote character, or split a quoted field that contains an embedded newline. Row order, column count and the delimiter are preserved; output quoting is re-derived. Max 5,000,000 bytes." },
                    "pattern": { "type": "string", "description": "What to find. By default this is a Rust regular expression — \\d, \\w, \\b, character classes, alternation, groups, quantifiers and named groups (?<name>...) all work; there are no backreferences or lookaround, which is what keeps matching linear-time. Set mode to 'literal' to match the text exactly as typed instead." },
                    "replacement": { "type": "string", "default": "", "description": "What to put in place of each match. In regex mode $1 / ${1} / ${name} expand capture groups, $0 is the whole match and $$ is a literal dollar sign — write ${1}x rather than $1x when a group reference is followed by a word character, since $1x is read as a group named '1x'. In literal mode the text is inserted verbatim, dollar signs included. Leave it blank to DELETE every match." },
                    "columns": { "type": "string", "default": "", "description": "Which columns the rule applies to. Blank (the default) or '*' means every column. Otherwise a comma-separated list of header names ('email'), 1-based indices ('2'), and inclusive index ranges ('2-4') — mixed freely, e.g. 'name,3,5-7'. Names are matched exactly first, then case-insensitively. Cells in unselected columns are copied through byte-for-byte." },
                    "mode": { "type": "string", "enum": ["regex", "literal"], "default": "regex", "description": "How to read the pattern. 'regex' (default) compiles it as a regular expression and expands capture references in the replacement. 'literal' escapes every metacharacter, so '.' matches a real dot and '$1' in the replacement stays '$1' — the safe choice for pasted values that happen to contain regex punctuation." },
                    "match_scope": { "type": "string", "enum": ["substring", "whole_cell"], "default": "substring", "description": "Where the pattern is allowed to match. 'substring' (default) matches anywhere inside a cell and can match several times. 'whole_cell' anchors the pattern to the ENTIRE cell value, so it either rewrites the whole cell or leaves it alone — use it for value remaps such as turning exactly 'NA' into an empty cell without touching 'NAME'." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match without regard to case (the regex 'i' flag), so 'error' also matches 'Error' and 'ERROR'. Applies to literal mode too. The replacement text is inserted exactly as written — the original casing is not restored." },
                    "multiline": { "type": "boolean", "default": false, "description": "The regex 'm' flag: ^ and $ also match at line breaks INSIDE a cell instead of only at its start and end. Only meaningful for quoted cells that contain embedded newlines. Whole-cell scope is unaffected — it anchors with \\A and \\z, which this flag cannot loosen." },
                    "dotall": { "type": "boolean", "default": false, "description": "The regex 's' flag: '.' also matches a newline, so a pattern can span an embedded line break inside a quoted cell. Off by default, matching the usual regex convention." },
                    "replace_all": { "type": "boolean", "default": true, "description": "When true (default) every match in a cell is replaced, like a regex 'g' flag. Turn it off to replace only the FIRST match in each cell and leave later ones intact." },
                    "has_header": { "type": "boolean", "default": true, "description": "When true (default) row 1 is a header: its names can be used in 'columns', it is excluded from replacement unless include_header is on, and it is kept at the top of a 'changed' listing. Turn it off for a headerless table — every row is then data and columns must be given as indices or ranges." },
                    "include_header": { "type": "boolean", "default": false, "description": "Also apply the rule to the header row itself. Off by default, so a pattern like 'code' rewrites the data without renaming the 'code' column. Ignored when has_header is false (there is no header to protect)." },
                    "delimiter": { "type": "string", "default": "auto", "description": "Field separator: 'auto' (default) sniffs it from the first line, counting candidates outside quotes with comma winning a tie; or give a name ('comma', 'tab', 'semicolon', 'pipe') or any single character. The output is written with the same separator." },
                    "quote_style": { "type": "string", "enum": ["minimal", "always", "non_numeric"], "default": "minimal", "description": "How the rewritten table is quoted. 'minimal' (default) quotes only fields that need it — one whose new value contains the delimiter, a quote or a newline is re-quoted automatically. 'always' quotes every field; 'non_numeric' quotes every field that is not a number." },
                    "output": { "type": "string", "enum": ["csv", "changed", "report"], "default": "csv", "description": "What to return: 'csv' (default) is the whole table with the replacements applied; 'changed' is only the rows that actually changed, plus the header, for reviewing a rule before committing to it; 'report' is a per-column audit table 'column,cells_changed,replacements' with a TOTAL row and no data at all." }
                },
                "required": ["data", "pattern"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
