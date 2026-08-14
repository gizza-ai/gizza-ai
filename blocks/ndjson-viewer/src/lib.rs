//! gizza-ai/ndjson-viewer — chat skill block on the shared tool abstraction.
//! Renders, searches and filters a newline-delimited JSON (NDJSON / JSON Lines)
//! stream: pretty / compact / array / table views, path-free key+value search,
//! dotted-path filtering, pagination and per-line invalid-JSON diagnostics.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_view")]
    view: String,
    #[serde(default = "default_indent")]
    indent: f64,
    #[serde(default)]
    search: String,
    #[serde(default = "default_search_in")]
    search_in: String,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default)]
    path: String,
    #[serde(default)]
    value: String,
    #[serde(default = "default_match_mode")]
    match_mode: String,
    #[serde(default)]
    sort_keys: bool,
    #[serde(default)]
    skip: f64,
    #[serde(default)]
    limit: f64,
    #[serde(default = "default_invalid")]
    invalid: String,
    #[serde(default)]
    line_numbers: bool,
    #[serde(default)]
    stats: bool,
}

fn default_view() -> String {
    "pretty".to_string()
}
fn default_indent() -> f64 {
    2.0
}
fn default_search_in() -> String {
    "any".to_string()
}
fn default_match_mode() -> String {
    "contains".to_string()
}
fn default_invalid() -> String {
    "report".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The NDJSON (JSON Lines) stream to view: one complete JSON value per line. Blank lines are skipped, a leading UTF-8 byte-order mark is stripped and CRLF endings are handled, so a paste straight out of a log viewer works. Every line is parsed on its own, so one malformed line never costs you the rest of the stream — what happens to it is set by invalid. Records do not have to share a shape: the table view unions whatever keys it finds and search walks whatever nesting each record has. Up to 50000 non-blank lines per run."),
        )
        .param(
            Param::enumv("view", ["pretty", "compact", "array", "table"])
                .default("pretty")
                .describe("How the kept records are rendered. \"pretty\" (default) prints one indented record per block, separated by a blank line — the readable form. \"compact\" minifies each record onto its own line, so the output is still valid NDJSON you can pipe onward. \"array\" wraps every record in a single indented JSON array, which is the shape tools that refuse NDJSON want. \"table\" draws an aligned text table whose columns are the keys discovered across the shown records in first-seen order, with any record that is not an object landing in a trailing (value) column."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(0.0)
                .max(8.0)
                .describe("Spaces of indentation for the pretty and array views. 2 by default; 0 minifies, which collapses the array view onto one line and prints one record per line in the pretty view. The compact and table views are unaffected — they are minified and aligned by definition. Maximum 8."),
        )
        .param(
            Param::string("search")
                .default("")
                .describe("Keep only records containing this text anywhere in them, at any depth, without your having to know the path. \"timeout\" matches a key named timeout, a value of \"timeout\", and either one nested inside err.code or items[3].reason. Empty (default) keeps every record. Narrow it with search_in (key names or values only), switch substring matching to exact or regular-expression matching with match_mode, and control folding with case_sensitive. Strings match on their contents; numbers, booleans and null match on their JSON text."),
        )
        .param(
            Param::enumv("search_in", ["any", "keys", "values"])
                .default("any")
                .describe("Where search is allowed to match. \"any\" (default) matches both key names and scalar values. \"keys\" matches key names only — that is how you find which records carry an err field rather than the ones whose message text happens to mention an error. \"values\" matches scalar values only, so a needle like \"status\" does not match every record just because they all have a key by that name. Ignored when search is empty."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("Match search and value case-sensitively. Off by default, which is what you want for log text where the same word is written three ways. Turn it on to tell ERROR from error, or to keep a regex like [A-Z]{3} meaning what it says — in regex mode this flag is exactly what sets the pattern's case-insensitivity."),
        )
        .param(
            Param::string("path")
                .default("")
                .describe("Keep only records that have a value at this dotted path. \"user.name\" walks object keys, and a numeric segment indexes an array, so \"items.0.id\" is the first item's id. Empty (default) applies no path filter. On its own it is an existence test — records missing the path are dropped, which is how you find the ones that do carry an err.code. Combined with value it becomes a test on exactly that field, the precise counterpart to search."),
        )
        .param(
            Param::string("value")
                .default("")
                .describe("Require this text at path or, when path is empty, anywhere among the record's values. value=\"error\" with path=\"status\" keeps the records whose status field mentions error; add match_mode=exact to require the field to be exactly \"error\" and nothing else. Empty (default) applies no value test. It is compared using match_mode and case_sensitive, the same way search is."),
        )
        .param(
            Param::enumv("match_mode", ["contains", "exact", "regex"])
                .default("contains")
                .describe("How search and value are compared. \"contains\" (default) is a substring test. \"exact\" requires the whole key name or the whole value to equal the needle, which is what a status or level field wants. \"regex\" reads the needle as a regular expression in Rust regex syntax (^ok$, \\d{3}, error|fatal, [0-9a-f]{8}); an unparseable pattern is reported as an error instead of silently matching nothing."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("Reorder every object's keys alphabetically, at every level of nesting, before rendering. Off by default, so records print in the key order they were written in. Turn it on when you are diffing two streams or reading records from producers that emit keys in different orders — it changes only the order keys print in, never a value."),
        )
        .param(
            Param::integer("skip")
                .default(0)
                .min(0.0)
                .describe("Drop this many matching records before showing any — the offset half of pagination. 0 (default) starts at the first match. It is counted after search, path and value have been applied, so skip=100 alongside a search means the 101st match, not the 101st line. Asking for more records than matched is not an error: the output says how many matched instead."),
        )
        .param(
            Param::integer("limit")
                .default(0)
                .min(0.0)
                .describe("Show at most this many records. 0 (default) shows every match. Pair it with skip to page through a large stream — limit=50 with skip=0, then 50, then 100. The stats header always reports the full match count, so you can see what a limit is holding back."),
        )
        .param(
            Param::enumv("invalid", ["report", "skip", "error"])
                .default("report")
                .describe("What to do with an input line that is not valid JSON. \"report\" (default) leaves a \"# line 7: invalid JSON — …\" marker where the record would have been, naming the column the parser stopped at, so a truncated line in the middle of a 10000-line paste is obvious while the rest still renders. \"skip\" drops those lines silently. \"error\" fails the whole run on the first one, which is what you want when the stream is meant to be machine-clean."),
        )
        .param(
            Param::boolean("line_numbers")
                .default(false)
                .describe("Label each record with where it came from in the input. Off by default. The pretty view gains a \"# record 3 (line 7)\" header per record, the compact view prefixes each line with \"7: \", and the table view gains a leading line column. The array view has nowhere to put them and ignores the flag. Numbering counts every input line, blank and invalid ones included, so it lines up with your editor."),
        )
        .param(
            Param::boolean("stats")
                .default(false)
                .describe("Prepend a three-line \"#\" header: how many non-blank lines were read, how many parsed as records and how many were invalid; how many records matched the filters and how many are shown at the current skip and limit; and an inventory of the top-level keys in first-seen order with the number of records carrying each (the first 20 keys, then a count of the rest). Off by default. Every header line starts with \"#\", so the result still pastes into a JSON-aware editor as comments."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NdjsonViewer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ndjson-viewer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "View, search and filter NDJSON / JSON Lines as pretty, compact, array or table output",
    skill(
        description = "Read a newline-delimited JSON stream (NDJSON / JSON Lines) — one complete JSON value per line — and render, search, filter and paginate it. Paste the stream into `data`. Every line is parsed on its own, so a malformed line never aborts the rest: `invalid` reports it in place with its line and column (default), drops it silently, or fails the run. `view` picks the output shape: `pretty` (one indented record per block, default), `compact` (minified, one record per line, still valid NDJSON), `array` (all records inside one JSON array) or `table` (an aligned text table whose columns are the keys discovered across the records, non-objects in a trailing `(value)` column); `indent` sets 0-8 spaces for the pretty and array views. `search` finds a needle at ANY depth without needing a path — a key name, a scalar value, or either one nested — with `search_in` narrowing to keys or values only, `match_mode` choosing contains (default), exact or regex, and `case_sensitive` controlling folding. `path` filters on an explicit dotted path (`user.name`, `items.0.id`, numeric segments index arrays), on existence alone or together with `value` for a test on that one field; `value` without a path tests every value in the record. `skip` and `limit` paginate the matches, `sort_keys` orders every object's keys alphabetically at every depth for diffing, `line_numbers` labels each record with its input line, and `stats` prepends a `#` header with the line/record/invalid counts, the matched and shown counts, and an inventory of top-level keys with how many records carry each. Pure text in, text out: no I/O and no clock, so the same paste always produces the same output. Up to 50000 non-blank lines per run. For a boolean predicate language or field selection use ndjson-filter, and for CSV output use data-format-converter.",
        parameters = schema_json()
    ),
)]
impl NdjsonViewer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ndjson-viewer", |a: Args| {
            gizza_ai_ndjson_viewer_core::run(
                &a.data,
                &a.view,
                a.indent.round() as i64,
                &a.search,
                &a.search_in,
                a.case_sensitive,
                &a.path,
                &a.value,
                &a.match_mode,
                a.sort_keys,
                a.skip.round() as i64,
                a.limit.round() as i64,
                &a.invalid,
                a.line_numbers,
                a.stats,
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
            r##"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The NDJSON (JSON Lines) stream to view: one complete JSON value per line. Blank lines are skipped, a leading UTF-8 byte-order mark is stripped and CRLF endings are handled, so a paste straight out of a log viewer works. Every line is parsed on its own, so one malformed line never costs you the rest of the stream — what happens to it is set by invalid. Records do not have to share a shape: the table view unions whatever keys it finds and search walks whatever nesting each record has. Up to 50000 non-blank lines per run." },
                    "view": { "type": "string", "enum": ["pretty", "compact", "array", "table"], "default": "pretty", "description": "How the kept records are rendered. \"pretty\" (default) prints one indented record per block, separated by a blank line — the readable form. \"compact\" minifies each record onto its own line, so the output is still valid NDJSON you can pipe onward. \"array\" wraps every record in a single indented JSON array, which is the shape tools that refuse NDJSON want. \"table\" draws an aligned text table whose columns are the keys discovered across the shown records in first-seen order, with any record that is not an object landing in a trailing (value) column." },
                    "indent": { "type": "integer", "default": 2, "minimum": 0, "maximum": 8, "description": "Spaces of indentation for the pretty and array views. 2 by default; 0 minifies, which collapses the array view onto one line and prints one record per line in the pretty view. The compact and table views are unaffected — they are minified and aligned by definition. Maximum 8." },
                    "search": { "type": "string", "default": "", "description": "Keep only records containing this text anywhere in them, at any depth, without your having to know the path. \"timeout\" matches a key named timeout, a value of \"timeout\", and either one nested inside err.code or items[3].reason. Empty (default) keeps every record. Narrow it with search_in (key names or values only), switch substring matching to exact or regular-expression matching with match_mode, and control folding with case_sensitive. Strings match on their contents; numbers, booleans and null match on their JSON text." },
                    "search_in": { "type": "string", "enum": ["any", "keys", "values"], "default": "any", "description": "Where search is allowed to match. \"any\" (default) matches both key names and scalar values. \"keys\" matches key names only — that is how you find which records carry an err field rather than the ones whose message text happens to mention an error. \"values\" matches scalar values only, so a needle like \"status\" does not match every record just because they all have a key by that name. Ignored when search is empty." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "Match search and value case-sensitively. Off by default, which is what you want for log text where the same word is written three ways. Turn it on to tell ERROR from error, or to keep a regex like [A-Z]{3} meaning what it says — in regex mode this flag is exactly what sets the pattern's case-insensitivity." },
                    "path": { "type": "string", "default": "", "description": "Keep only records that have a value at this dotted path. \"user.name\" walks object keys, and a numeric segment indexes an array, so \"items.0.id\" is the first item's id. Empty (default) applies no path filter. On its own it is an existence test — records missing the path are dropped, which is how you find the ones that do carry an err.code. Combined with value it becomes a test on exactly that field, the precise counterpart to search." },
                    "value": { "type": "string", "default": "", "description": "Require this text at path or, when path is empty, anywhere among the record's values. value=\"error\" with path=\"status\" keeps the records whose status field mentions error; add match_mode=exact to require the field to be exactly \"error\" and nothing else. Empty (default) applies no value test. It is compared using match_mode and case_sensitive, the same way search is." },
                    "match_mode": { "type": "string", "enum": ["contains", "exact", "regex"], "default": "contains", "description": "How search and value are compared. \"contains\" (default) is a substring test. \"exact\" requires the whole key name or the whole value to equal the needle, which is what a status or level field wants. \"regex\" reads the needle as a regular expression in Rust regex syntax (^ok$, \\d{3}, error|fatal, [0-9a-f]{8}); an unparseable pattern is reported as an error instead of silently matching nothing." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "Reorder every object's keys alphabetically, at every level of nesting, before rendering. Off by default, so records print in the key order they were written in. Turn it on when you are diffing two streams or reading records from producers that emit keys in different orders — it changes only the order keys print in, never a value." },
                    "skip": { "type": "integer", "default": 0, "minimum": 0, "description": "Drop this many matching records before showing any — the offset half of pagination. 0 (default) starts at the first match. It is counted after search, path and value have been applied, so skip=100 alongside a search means the 101st match, not the 101st line. Asking for more records than matched is not an error: the output says how many matched instead." },
                    "limit": { "type": "integer", "default": 0, "minimum": 0, "description": "Show at most this many records. 0 (default) shows every match. Pair it with skip to page through a large stream — limit=50 with skip=0, then 50, then 100. The stats header always reports the full match count, so you can see what a limit is holding back." },
                    "invalid": { "type": "string", "enum": ["report", "skip", "error"], "default": "report", "description": "What to do with an input line that is not valid JSON. \"report\" (default) leaves a \"# line 7: invalid JSON — …\" marker where the record would have been, naming the column the parser stopped at, so a truncated line in the middle of a 10000-line paste is obvious while the rest still renders. \"skip\" drops those lines silently. \"error\" fails the whole run on the first one, which is what you want when the stream is meant to be machine-clean." },
                    "line_numbers": { "type": "boolean", "default": false, "description": "Label each record with where it came from in the input. Off by default. The pretty view gains a \"# record 3 (line 7)\" header per record, the compact view prefixes each line with \"7: \", and the table view gains a leading line column. The array view has nowhere to put them and ignores the flag. Numbering counts every input line, blank and invalid ones included, so it lines up with your editor." },
                    "stats": { "type": "boolean", "default": false, "description": "Prepend a three-line \"#\" header: how many non-blank lines were read, how many parsed as records and how many were invalid; how many records matched the filters and how many are shown at the current skip and limit; and an inventory of the top-level keys in first-seen order with the number of records carrying each (the first 20 keys, then a count of the rest). Off by default. Every header line starts with \"#\", so the result still pastes into a JSON-aware editor as comments." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
