//! gizza-ai/http-header-normalizer — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI + the page query-params); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_http_header_normalizer_core::normalize;
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    case: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    duplicates: String,
    #[serde(default = "default_true")]
    unfold: bool,
    #[serde(default)]
    drop_empty: bool,
    #[serde(default)]
    drop_headers: String,
    #[serde(default)]
    keep_headers: String,
    #[serde(default)]
    output: String,
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The HTTP header block to normalize, one 'Name: value' per line — e.g. 'host:  example.com' then 'CONTENT-TYPE: application/json'. A leading request line ('GET /p HTTP/1.1') or status line ('HTTP/1.1 200 OK') is optional and is kept verbatim at the top. Indented continuation lines are joined onto the header above them, HTTP/2 pseudo-headers such as ':method' are understood, and the first blank line ends the block so a pasted body is ignored. Max 20,000 lines and 1,000,000 bytes per run."),
        )
        .param(
            Param::enumv("case", ["canonical", "lower", "upper", "preserve"])
                .default("canonical")
                .describe("How field names are spelled on the way out: 'canonical' (default) is HTTP Title-Case with the exception table plain title-casing gets wrong — ETag, WWW-Authenticate, DNT, TE, Content-MD5, X-XSS-Protection, Sec-WebSocket-*, X-UA-Compatible and friends; 'lower' is the HTTP/2 and proxy-log form; 'upper' suits CGI-style comparisons; 'preserve' keeps the casing of the first time each name appears. HTTP/2 pseudo-headers stay lowercase in every mode, because the protocol requires it."),
        )
        .param(
            Param::enumv("sort", ["name", "none"])
                .default("name")
                .describe("Line order in the result: 'name' (default) sorts alphabetically by lower-cased field name, which is what makes two captures of the same request diff cleanly; 'none' keeps the order you pasted. Pseudo-headers are placed before regular fields when sorting, per RFC 9113. Sorting is stable, so repeats of one name keep their relative order."),
        )
        .param(
            Param::enumv("duplicates", ["combine", "list", "first", "last"])
                .default("combine")
                .describe("How repeated field names (compared case-insensitively) are folded: 'combine' (default) joins their values with ', ' per RFC 7230 §3.2.2; 'list' keeps every occurrence on its own line; 'first' keeps only the first value; 'last' keeps only the last. Set-Cookie is never comma-joined — RFC 6265 forbids it, so it always stays one line per cookie."),
        )
        .param(
            Param::boolean("unfold")
                .default(true)
                .describe("Join obsolete line folds: a continuation line that starts with a space or tab is appended to the header above it with a single space, which is how a modern parser reads it. On by default. Turn it off to keep the fold as an indented second line in the output. A continuation with no header before it is an error either way."),
        )
        .param(
            Param::boolean("drop_empty")
                .default(false)
                .describe("Remove fields whose value is empty after trimming, such as a bare 'X-Debug:' line. Off by default, because an empty value is still a header that was sent and can be meaningful. Turn it on to clear the leftovers a proxy or an unfilled template appends."),
        )
        .param(
            Param::string("drop_headers")
                .default("")
                .describe("Field names to remove, comma-separated and matched case-insensitively — e.g. 'cookie,authorization,x-internal-token'. A trailing '*' makes it a prefix rule, so 'x-*' drops every X- header. Useful for redacting credentials before pasting a capture into a bug report."),
        )
        .param(
            Param::string("keep_headers")
                .default("")
                .describe("An allowlist: when set, ONLY these field names survive and everything else is dropped — comma-separated, case-insensitive, with the same trailing-'*' prefix rule as drop_headers. Empty by default, which keeps everything. Applied before drop_headers, so the two can be combined."),
        )
        .param(
            Param::enumv("output", ["headers", "curl", "summary"])
                .default("headers")
                .describe("What to return: 'headers' (default) is the normalized header block as text; 'curl' is the same headers as copy-pasteable '-H' flags, single-quoted and backslash-continued (pseudo-headers are skipped because curl derives them from the URL and method); 'summary' is a metric,value CSV of the run — lines in, headers in, names in, headers out, names recased, values trimmed, duplicates merged, names dropped, and body lines skipped."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/http-header-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Canonicalize a pasted HTTP header block — fix name casing, trim values, fold duplicates and sort.",
    skill(
        description = "Turn a pasted HTTP header block into one canonical block. Rewrites field names to a chosen casing (canonical Title-Case by default, including the names plain title-casing gets wrong such as ETag, WWW-Authenticate, DNT and Sec-WebSocket-Key; or lower, upper, or preserved), trims every value, joins obsolete indented continuation lines, folds repeated names case-insensitively (comma-joined per RFC 7230 by default, or list/first/last, with Set-Cookie never comma-joined), drops empty values, applies a comma-separated deny list or allowlist with 'x-*' prefix wildcards, and sorts the fields by name so two captures of the same request diff cleanly. An optional leading request or status line is kept verbatim on top and HTTP/2 pseudo-headers are sorted first and left lowercase. Returns the normalized header block, the same headers as copy-pasteable curl -H flags, or a CSV summary of the run.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "http-header-normalizer", |a: Args| {
            normalize(
                &a.input,
                &a.case,
                &a.sort,
                &a.duplicates,
                a.unfold,
                a.drop_empty,
                &a.drop_headers,
                &a.keep_headers,
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
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The HTTP header block to normalize, one 'Name: value' per line — e.g. 'host:  example.com' then 'CONTENT-TYPE: application/json'. A leading request line ('GET /p HTTP/1.1') or status line ('HTTP/1.1 200 OK') is optional and is kept verbatim at the top. Indented continuation lines are joined onto the header above them, HTTP/2 pseudo-headers such as ':method' are understood, and the first blank line ends the block so a pasted body is ignored. Max 20,000 lines and 1,000,000 bytes per run." },
                    "case": { "type": "string", "enum": ["canonical", "lower", "upper", "preserve"], "default": "canonical", "description": "How field names are spelled on the way out: 'canonical' (default) is HTTP Title-Case with the exception table plain title-casing gets wrong — ETag, WWW-Authenticate, DNT, TE, Content-MD5, X-XSS-Protection, Sec-WebSocket-*, X-UA-Compatible and friends; 'lower' is the HTTP/2 and proxy-log form; 'upper' suits CGI-style comparisons; 'preserve' keeps the casing of the first time each name appears. HTTP/2 pseudo-headers stay lowercase in every mode, because the protocol requires it." },
                    "sort": { "type": "string", "enum": ["name", "none"], "default": "name", "description": "Line order in the result: 'name' (default) sorts alphabetically by lower-cased field name, which is what makes two captures of the same request diff cleanly; 'none' keeps the order you pasted. Pseudo-headers are placed before regular fields when sorting, per RFC 9113. Sorting is stable, so repeats of one name keep their relative order." },
                    "duplicates": { "type": "string", "enum": ["combine", "list", "first", "last"], "default": "combine", "description": "How repeated field names (compared case-insensitively) are folded: 'combine' (default) joins their values with ', ' per RFC 7230 §3.2.2; 'list' keeps every occurrence on its own line; 'first' keeps only the first value; 'last' keeps only the last. Set-Cookie is never comma-joined — RFC 6265 forbids it, so it always stays one line per cookie." },
                    "unfold": { "type": "boolean", "default": true, "description": "Join obsolete line folds: a continuation line that starts with a space or tab is appended to the header above it with a single space, which is how a modern parser reads it. On by default. Turn it off to keep the fold as an indented second line in the output. A continuation with no header before it is an error either way." },
                    "drop_empty": { "type": "boolean", "default": false, "description": "Remove fields whose value is empty after trimming, such as a bare 'X-Debug:' line. Off by default, because an empty value is still a header that was sent and can be meaningful. Turn it on to clear the leftovers a proxy or an unfilled template appends." },
                    "drop_headers": { "type": "string", "default": "", "description": "Field names to remove, comma-separated and matched case-insensitively — e.g. 'cookie,authorization,x-internal-token'. A trailing '*' makes it a prefix rule, so 'x-*' drops every X- header. Useful for redacting credentials before pasting a capture into a bug report." },
                    "keep_headers": { "type": "string", "default": "", "description": "An allowlist: when set, ONLY these field names survive and everything else is dropped — comma-separated, case-insensitive, with the same trailing-'*' prefix rule as drop_headers. Empty by default, which keeps everything. Applied before drop_headers, so the two can be combined." },
                    "output": { "type": "string", "enum": ["headers", "curl", "summary"], "default": "headers", "description": "What to return: 'headers' (default) is the normalized header block as text; 'curl' is the same headers as copy-pasteable '-H' flags, single-quoted and backslash-continued (pseudo-headers are skipped because curl derives them from the URL and method); 'summary' is a metric,value CSV of the run — lines in, headers in, names in, headers out, names recased, values trimmed, duplicates merged, names dropped, and body lines skipped." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
