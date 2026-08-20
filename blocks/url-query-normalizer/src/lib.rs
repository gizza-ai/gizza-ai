//! gizza-ai/url-query-normalizer — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI + the page query-params); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_url_query_normalizer_core::normalize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    dedupe: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    space: String,
    #[serde(default)]
    drop_tracking: bool,
    #[serde(default)]
    drop_params: String,
    #[serde(default)]
    keep_params: String,
    #[serde(default)]
    drop_empty: bool,
    #[serde(default)]
    output: String,
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The URLs to normalize, one per line — e.g. 'https://example.com/p?utm_source=news&b=2&a=1'. A bare query string with no scheme or host ('b=2&a=1') is accepted too and comes back without a leading '?'. Lines with no query string are passed through untouched, blank lines are ignored, and everything outside the query — scheme, host, port, path and fragment — is copied byte-for-byte. Max 20,000 lines and 1,000,000 bytes per run."),
        )
        .param(
            Param::enumv("sort", ["key", "key-value", "none"])
                .default("key")
                .describe("How to order the surviving parameters: 'key' (default) sorts alphabetically by parameter name and is what makes two spellings of the same URL converge; 'key-value' also orders repeats of the same name by their value; 'none' keeps the original order. Sorting is stable, so equally-ranked parameters keep the order you gave them."),
        )
        .param(
            Param::enumv("dedupe", ["exact", "first", "last", "none"])
                .default("exact")
                .describe("How to collapse repeated parameters: 'exact' (default) drops only byte-identical name=value repeats, so a genuinely multi-valued parameter like 'tag=a&tag=b' survives intact; 'first' keeps the first value seen for each name and drops the rest; 'last' keeps the last; 'none' keeps every repeat. Comparison happens after encoding normalization, so 'q=a+b' and 'q=a%20b' count as the same pair."),
        )
        .param(
            Param::enumv("encoding", ["normalize", "preserve"])
                .default("normalize")
                .describe("Percent-encoding policy. 'normalize' (default) rewrites every name and value to one canonical spelling per RFC 3986: unreserved characters (A-Z a-z 0-9 - . _ ~) are decoded to literals, everything that must be escaped is escaped, and hex digits are uppercased, so '%2d' becomes '-' and '%c3%a9' becomes '%C3%A9'. 'preserve' leaves the text of each name and value exactly as written and only reorders, filters and deduplicates. Malformed escapes such as a trailing '%' are never an error — the '%' is escaped as '%25'."),
        )
        .param(
            Param::enumv("space", ["percent", "plus"])
                .default("percent")
                .describe("How a space inside a name or value is spelled on the way out: 'percent' (default) writes '%20', 'plus' writes '+'. A literal '+' in the input is read as a space, per the form-urlencoded convention every browser applies to query strings; a real plus sign written as '%2B' stays '%2B'. Only consulted when encoding is 'normalize'."),
        )
        .param(
            Param::boolean("drop_tracking")
                .default(false)
                .describe("Remove the usual analytics and click-ID parameters — the utm_*, pk_*, mtm_*, ga_*, _hs* families plus fbclid, gclid, msclkid, yclid, igshid, mkt_tok and friends. Off by default because normalizing and stripping are separate decisions; turn it on to get a shareable canonical link in one pass. If every parameter is removed the '?' goes with them."),
        )
        .param(
            Param::string("drop_params")
                .default("")
                .describe("Extra parameter names to remove, comma-separated and matched case-insensitively — e.g. 'sid,ref,session_id'. A trailing '*' makes it a prefix rule, so 'x_*' drops x_foo and x_bar. Applied on top of drop_tracking."),
        )
        .param(
            Param::string("keep_params")
                .default("")
                .describe("An allowlist: when set, ONLY these parameter names survive and everything else is dropped — comma-separated, case-insensitive, with the same trailing-'*' prefix rule as drop_params. This is the fastest way to build a cache key from the two or three parameters that actually change the response, e.g. 'page,sort'. Empty by default, which keeps everything."),
        )
        .param(
            Param::boolean("drop_empty")
                .default(false)
                .describe("Also remove parameters with no value — both 'a=' and a bare valueless 'flag'. Off by default, because an empty value is meaningful to some applications. Turn it on to clear the leftovers an unfilled form appends to a URL."),
        )
        .param(
            Param::enumv("output", ["urls", "changed", "report", "summary"])
                .default("urls")
                .describe("What to return: 'urls' (default) is every line normalized, one per line; 'changed' is only the lines that actually differ from the input, which is the canonical/redirect list worth acting on; 'report' is a line,original,normalized,params_in,params_out,changed CSV covering every line; 'summary' is a metric,value CSV of the run totals."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/url-query-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Canonicalize URL query strings — sort, deduplicate and normalize percent-encoding, with optional tracking-parameter removal.",
    skill(
        description = "Canonicalize the query string of one URL or a whole list, so two spellings of the same address become one string. Sorts parameters by name, collapses duplicates (byte-identical pairs by default, or first/last wins per name), and rewrites percent-encoding to the RFC 3986 canonical form — unreserved characters decoded, required escapes uppercased, '+' and '%20' unified on one spelling for spaces. Optionally strips tracking parameters (utm_*, fbclid, gclid, …), applies a custom drop list or an allowlist with prefix wildcards, and removes empty values. Takes one URL per line or a bare query string with no scheme; scheme, host, port, path and fragment are copied byte-for-byte and nothing outside the query is touched. Returns the normalized list, only the lines that changed, a per-line CSV report, or a CSV summary of the totals.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "url-query-normalizer", |a: Args| {
            normalize(
                &a.input,
                &a.sort,
                &a.dedupe,
                &a.encoding,
                &a.space,
                a.drop_tracking,
                &a.drop_params,
                &a.keep_params,
                a.drop_empty,
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
                    "input": { "type": "string", "description": "The URLs to normalize, one per line — e.g. 'https://example.com/p?utm_source=news&b=2&a=1'. A bare query string with no scheme or host ('b=2&a=1') is accepted too and comes back without a leading '?'. Lines with no query string are passed through untouched, blank lines are ignored, and everything outside the query — scheme, host, port, path and fragment — is copied byte-for-byte. Max 20,000 lines and 1,000,000 bytes per run." },
                    "sort": { "type": "string", "enum": ["key", "key-value", "none"], "default": "key", "description": "How to order the surviving parameters: 'key' (default) sorts alphabetically by parameter name and is what makes two spellings of the same URL converge; 'key-value' also orders repeats of the same name by their value; 'none' keeps the original order. Sorting is stable, so equally-ranked parameters keep the order you gave them." },
                    "dedupe": { "type": "string", "enum": ["exact", "first", "last", "none"], "default": "exact", "description": "How to collapse repeated parameters: 'exact' (default) drops only byte-identical name=value repeats, so a genuinely multi-valued parameter like 'tag=a&tag=b' survives intact; 'first' keeps the first value seen for each name and drops the rest; 'last' keeps the last; 'none' keeps every repeat. Comparison happens after encoding normalization, so 'q=a+b' and 'q=a%20b' count as the same pair." },
                    "encoding": { "type": "string", "enum": ["normalize", "preserve"], "default": "normalize", "description": "Percent-encoding policy. 'normalize' (default) rewrites every name and value to one canonical spelling per RFC 3986: unreserved characters (A-Z a-z 0-9 - . _ ~) are decoded to literals, everything that must be escaped is escaped, and hex digits are uppercased, so '%2d' becomes '-' and '%c3%a9' becomes '%C3%A9'. 'preserve' leaves the text of each name and value exactly as written and only reorders, filters and deduplicates. Malformed escapes such as a trailing '%' are never an error — the '%' is escaped as '%25'." },
                    "space": { "type": "string", "enum": ["percent", "plus"], "default": "percent", "description": "How a space inside a name or value is spelled on the way out: 'percent' (default) writes '%20', 'plus' writes '+'. A literal '+' in the input is read as a space, per the form-urlencoded convention every browser applies to query strings; a real plus sign written as '%2B' stays '%2B'. Only consulted when encoding is 'normalize'." },
                    "drop_tracking": { "type": "boolean", "default": false, "description": "Remove the usual analytics and click-ID parameters — the utm_*, pk_*, mtm_*, ga_*, _hs* families plus fbclid, gclid, msclkid, yclid, igshid, mkt_tok and friends. Off by default because normalizing and stripping are separate decisions; turn it on to get a shareable canonical link in one pass. If every parameter is removed the '?' goes with them." },
                    "drop_params": { "type": "string", "default": "", "description": "Extra parameter names to remove, comma-separated and matched case-insensitively — e.g. 'sid,ref,session_id'. A trailing '*' makes it a prefix rule, so 'x_*' drops x_foo and x_bar. Applied on top of drop_tracking." },
                    "keep_params": { "type": "string", "default": "", "description": "An allowlist: when set, ONLY these parameter names survive and everything else is dropped — comma-separated, case-insensitive, with the same trailing-'*' prefix rule as drop_params. This is the fastest way to build a cache key from the two or three parameters that actually change the response, e.g. 'page,sort'. Empty by default, which keeps everything." },
                    "drop_empty": { "type": "boolean", "default": false, "description": "Also remove parameters with no value — both 'a=' and a bare valueless 'flag'. Off by default, because an empty value is meaningful to some applications. Turn it on to clear the leftovers an unfilled form appends to a URL." },
                    "output": { "type": "string", "enum": ["urls", "changed", "report", "summary"], "default": "urls", "description": "What to return: 'urls' (default) is every line normalized, one per line; 'changed' is only the lines that actually differ from the input, which is the canonical/redirect list worth acting on; 'report' is a line,original,normalized,params_in,params_out,changed CSV covering every line; 'summary' is a metric,value CSV of the run totals." }
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
