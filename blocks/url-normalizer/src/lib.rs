//! gizza-ai/url-normalizer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + the page query-params); handle() delegates to block_utils::run_skill.
//! No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_url_normalizer_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    urls: String,
    #[serde(default)]
    base: String,
    #[serde(default)]
    scheme: String,
    #[serde(default)]
    www: String,
    #[serde(default = "yes")]
    strip_default_port: bool,
    #[serde(default = "yes")]
    dot_segments: bool,
    #[serde(default)]
    collapse_slashes: bool,
    #[serde(default)]
    lowercase_path: bool,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    drop_index: bool,
    #[serde(default)]
    trailing_slash: String,
    #[serde(default)]
    sort_query: String,
    #[serde(default)]
    dedupe_query: bool,
    #[serde(default)]
    drop_empty_params: bool,
    #[serde(default)]
    drop_tracking: bool,
    #[serde(default)]
    drop_fragment: bool,
    #[serde(default)]
    dedupe_urls: bool,
    #[serde(default)]
    on_invalid: String,
    #[serde(default)]
    output: String,
}

fn yes() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("urls")
                .required()
                .describe("The URLs to normalize, one per line — e.g. 'HTTP://Example.COM:80/a/b/../c?b=2&a=1'. Absolute URLs (any scheme), scheme-relative URLs ('//cdn.example.com/a'), bare hosts ('Example.COM/path', 'localhost:8080/a') and — when a base URL is given — relative references ('../images/logo.png', '/about', '?page=2') are all accepted. Blank lines are ignored. Max 20,000 URLs and 1,000,000 bytes per run."),
        )
        .param(
            Param::string("base")
                .default("")
                .describe("Optional absolute base URL used to turn relative references into absolute ones, per RFC 3986 section 5.2 — e.g. 'https://example.com/docs/guide/index.html' makes '../images/logo.png' resolve to 'https://example.com/docs/images/logo.png'. Lines that already carry their own scheme are left absolute and unaffected. Empty by default, which leaves relative lines relative."),
        )
        .param(
            Param::enumv("scheme", ["preserve", "https", "http"])
                .default("preserve")
                .describe("What to do with the scheme. It is always lowercased ('HTTP:' becomes 'http:'). 'preserve' (default) keeps whichever scheme was written; 'https' and 'http' rewrite it, but ONLY for http/https URLs and for scheme-less lines that name a host — a 'mailto:', 'ftp:' or 'tel:' line is never rewritten into a web URL."),
        )
        .param(
            Param::enumv("www", ["preserve", "strip", "add"])
                .default("preserve")
                .describe("The 'www.' policy for the host. 'preserve' (default) leaves it as written; 'strip' removes a leading 'www.' as long as at least two labels remain, so 'www.example.com' becomes 'example.com' but 'www.com' is left alone; 'add' prefixes 'www.' to a bare apex domain only ('example.com' becomes 'www.example.com'; 'blog.example.com' and IP addresses are untouched). Pick one and use it consistently — the two forms are different hosts to a search engine."),
        )
        .param(
            Param::boolean("strip_default_port")
                .default(true)
                .describe("Remove a port that the scheme already implies — ':80' on http, ':443' on https, plus the defaults for ws, wss, ftp, ftps, sftp, ssh, telnet, smtp, gopher, pop3, nntp, imap, ldap and ldaps. On by default (RFC 3986 section 6.2.3). Leading zeros are trimmed first, so ':0443' counts as 443, and an empty ':' is always dropped. Turn it off to keep the port exactly as written."),
        )
        .param(
            Param::boolean("dot_segments")
                .default(true)
                .describe("Resolve '.' and '..' inside the path using the RFC 3986 section 5.2.4 algorithm, so '/a/b/../c' becomes '/a/c' and '/a/./b' becomes '/a/b'. On by default — this is what makes two spellings of the same file compare equal. Turn it off to leave the path segments exactly as written."),
        )
        .param(
            Param::boolean("collapse_slashes")
                .default(false)
                .describe("Collapse runs of '/' in the path to a single slash, so '/a//b///c' becomes '/a/b/c'. Off by default: most servers serve both spellings identically, but a few treat an empty segment as meaningful, and RFC 3986 does not license the change. Turn it on when a crawl export is full of accidental double slashes."),
        )
        .param(
            Param::boolean("lowercase_path")
                .default(false)
                .describe("Lowercase the letters in the path, leaving percent-escape hex digits uppercase — '/Docs/Caf%c3%a9' becomes '/docs/caf%C3%A9'. Off by default because a path is case-SENSITIVE on most servers, so this can point a URL at a page that does not exist. Turn it on only for a case-insensitive host such as IIS. The scheme and host are always lowercased regardless."),
        )
        .param(
            Param::enumv("encoding", ["normalize", "decode", "preserve"])
                .default("normalize")
                .describe("Percent-encoding policy, applied to the path, query and fragment separately. 'normalize' (default) is RFC 3986 section 6.2.2: escapes of unreserved characters (A-Z a-z 0-9 - . _ ~) are decoded to literals, everything that must stay escaped keeps uppercase hex, and illegal literals such as a space are escaped — so '%2d' becomes '-' and '%c3%a9' becomes '%C3%A9'. 'decode' additionally unescapes characters that are legal literals in that component, including '%2F' in a path, which — combined with dot_segments — reveals a disguised traversal like '%2E%2E%2F'. 'preserve' leaves every escape byte-for-byte and only reorders and filters. A lone '%' is never an error: it is escaped as '%25'."),
        )
        .param(
            Param::boolean("drop_index")
                .default(false)
                .describe("Remove a trailing directory-index file from the path, leaving the directory slash — '/blog/index.html' becomes '/blog/'. Recognizes index.html, index.htm, index.php, index.asp, index.aspx, index.jsp, index.cgi, index.shtml, index.xhtml and the default.* equivalents, matched case-insensitively. Off by default because not every server serves the two forms identically."),
        )
        .param(
            Param::enumv("trailing_slash", ["preserve", "add", "remove"])
                .default("preserve")
                .describe("Trailing-slash policy for the path. 'preserve' (default) leaves it alone; 'add' appends a slash to directory-style paths but never to a last segment that looks like a file ('/sitemap.xml' stays put); 'remove' strips trailing slashes. The site root always stays a single '/'. Applied after drop_index, so '/blog/index.html' with both set becomes '/blog/' or '/blog'."),
        )
        .param(
            Param::enumv("sort_query", ["key", "key-value", "none"])
                .default("key")
                .describe("How to order the query parameters — this is what makes '?b=2&a=1' and '?a=1&b=2' collapse to one canonical string. 'key' (default) sorts alphabetically by parameter name; 'key-value' also orders repeats of the same name by value; 'none' keeps the original order. Sorting is stable, so equally-ranked parameters keep the order you gave them."),
        )
        .param(
            Param::boolean("dedupe_query")
                .default(false)
                .describe("Drop repeated query parameters that are byte-identical after encoding normalization, keeping the first — so '?a=1&a=1' becomes '?a=1' while a genuinely multi-valued 'tag=a&tag=b' survives intact. Off by default because a repeat can be meaningful to some applications."),
        )
        .param(
            Param::boolean("drop_empty_params")
                .default(false)
                .describe("Remove query parameters with no value — both 'a=' and a bare valueless 'flag'. Off by default, because an empty value is meaningful to some applications; turn it on to clear the leftovers an unfilled form appends to a URL. If every parameter goes, the '?' goes with them."),
        )
        .param(
            Param::boolean("drop_tracking")
                .default(false)
                .describe("Remove the usual analytics and click-ID query parameters — the utm_*, pk_*, mtm_*, ga_*, _hs* families plus fbclid, gclid, msclkid, yclid, igshid, mkt_tok and friends. Off by default because canonicalizing and stripping are separate decisions. For allow/deny lists and custom parameter names use the dedicated URL query normalizer instead."),
        )
        .param(
            Param::boolean("drop_fragment")
                .default(false)
                .describe("Remove the '#fragment' entirely. Off by default, since a fragment is part of the address a user shared. An EMPTY fragment (a bare trailing '#') is always dropped, because it addresses the same resource. Turn this on for the SEO canonical form, where the fragment is never part of the indexed URL."),
        )
        .param(
            Param::boolean("dedupe_urls")
                .default(false)
                .describe("Drop lines that normalize to a URL an earlier line already produced, keeping the first occurrence and the original order. This is the point of canonicalizing a crawl export: 'https://Example.com/a?b=1&a=2' and 'https://example.com/a?a=2&b=1' are one page. Default false."),
        )
        .param(
            Param::enumv("on_invalid", ["keep", "drop", "error"])
                .default("keep")
                .describe("What to do with a line that cannot be parsed — a host containing a space or an illegal character, an unterminated IPv6 literal, a non-numeric port, a control character. 'keep' (default) passes it through untouched so an annotated list survives a round trip, 'drop' leaves it out of the result, 'error' fails the run and names the line number and the reason."),
        )
        .param(
            Param::enumv("output", ["urls", "changed", "report", "summary"])
                .default("urls")
                .describe("What to return: 'urls' (default) is every line normalized, one per line; 'changed' is only the URLs that actually differ from the input, which is the canonical/redirect list worth acting on; 'report' is a line,original,normalized,action CSV covering every input line, where action is normalized, unchanged, duplicate or invalid; 'summary' is a metric,value CSV of the run totals."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/url-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Canonicalize URLs: lowercase the host, drop default ports, resolve dot-segments and sort the query.",
    skill(
        description = "Normalize a batch of URLs to one canonical form, per RFC 3986 syntax-based normalization. Lowercases the scheme and host, drops a port the scheme already implies, resolves '.' and '..' path segments, rewrites percent-encoding to a single canonical spelling, and sorts the query parameters so '?b=2&a=1' and '?a=1&b=2' collapse to the same string. Optional SEO passes on top: force http or https, add or strip 'www.', add or remove trailing slashes, collapse duplicate slashes, lowercase the path, remove directory-index files like index.html, drop tracking parameters, empty parameters, repeated parameters and the fragment, and deduplicate the whole list. Takes one URL per line — absolute, scheme-relative, bare host, or relative against an optional base URL — and returns the normalized list, only the URLs that changed, a per-line CSV report, or a CSV summary of the totals.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "url-normalizer", |a: Args| {
            run(
                &a.urls,
                &a.base,
                &a.scheme,
                &a.www,
                a.strip_default_port,
                a.dot_segments,
                a.collapse_slashes,
                a.lowercase_path,
                &a.encoding,
                a.drop_index,
                &a.trailing_slash,
                &a.sort_query,
                a.dedupe_query,
                a.drop_empty_params,
                a.drop_tracking,
                a.drop_fragment,
                a.dedupe_urls,
                &a.on_invalid,
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
                    "urls": { "type": "string", "description": "The URLs to normalize, one per line — e.g. 'HTTP://Example.COM:80/a/b/../c?b=2&a=1'. Absolute URLs (any scheme), scheme-relative URLs ('//cdn.example.com/a'), bare hosts ('Example.COM/path', 'localhost:8080/a') and — when a base URL is given — relative references ('../images/logo.png', '/about', '?page=2') are all accepted. Blank lines are ignored. Max 20,000 URLs and 1,000,000 bytes per run." },
                    "base": { "type": "string", "default": "", "description": "Optional absolute base URL used to turn relative references into absolute ones, per RFC 3986 section 5.2 — e.g. 'https://example.com/docs/guide/index.html' makes '../images/logo.png' resolve to 'https://example.com/docs/images/logo.png'. Lines that already carry their own scheme are left absolute and unaffected. Empty by default, which leaves relative lines relative." },
                    "scheme": { "type": "string", "enum": ["preserve", "https", "http"], "default": "preserve", "description": "What to do with the scheme. It is always lowercased ('HTTP:' becomes 'http:'). 'preserve' (default) keeps whichever scheme was written; 'https' and 'http' rewrite it, but ONLY for http/https URLs and for scheme-less lines that name a host — a 'mailto:', 'ftp:' or 'tel:' line is never rewritten into a web URL." },
                    "www": { "type": "string", "enum": ["preserve", "strip", "add"], "default": "preserve", "description": "The 'www.' policy for the host. 'preserve' (default) leaves it as written; 'strip' removes a leading 'www.' as long as at least two labels remain, so 'www.example.com' becomes 'example.com' but 'www.com' is left alone; 'add' prefixes 'www.' to a bare apex domain only ('example.com' becomes 'www.example.com'; 'blog.example.com' and IP addresses are untouched). Pick one and use it consistently — the two forms are different hosts to a search engine." },
                    "strip_default_port": { "type": "boolean", "default": true, "description": "Remove a port that the scheme already implies — ':80' on http, ':443' on https, plus the defaults for ws, wss, ftp, ftps, sftp, ssh, telnet, smtp, gopher, pop3, nntp, imap, ldap and ldaps. On by default (RFC 3986 section 6.2.3). Leading zeros are trimmed first, so ':0443' counts as 443, and an empty ':' is always dropped. Turn it off to keep the port exactly as written." },
                    "dot_segments": { "type": "boolean", "default": true, "description": "Resolve '.' and '..' inside the path using the RFC 3986 section 5.2.4 algorithm, so '/a/b/../c' becomes '/a/c' and '/a/./b' becomes '/a/b'. On by default — this is what makes two spellings of the same file compare equal. Turn it off to leave the path segments exactly as written." },
                    "collapse_slashes": { "type": "boolean", "default": false, "description": "Collapse runs of '/' in the path to a single slash, so '/a//b///c' becomes '/a/b/c'. Off by default: most servers serve both spellings identically, but a few treat an empty segment as meaningful, and RFC 3986 does not license the change. Turn it on when a crawl export is full of accidental double slashes." },
                    "lowercase_path": { "type": "boolean", "default": false, "description": "Lowercase the letters in the path, leaving percent-escape hex digits uppercase — '/Docs/Caf%c3%a9' becomes '/docs/caf%C3%A9'. Off by default because a path is case-SENSITIVE on most servers, so this can point a URL at a page that does not exist. Turn it on only for a case-insensitive host such as IIS. The scheme and host are always lowercased regardless." },
                    "encoding": { "type": "string", "enum": ["normalize", "decode", "preserve"], "default": "normalize", "description": "Percent-encoding policy, applied to the path, query and fragment separately. 'normalize' (default) is RFC 3986 section 6.2.2: escapes of unreserved characters (A-Z a-z 0-9 - . _ ~) are decoded to literals, everything that must stay escaped keeps uppercase hex, and illegal literals such as a space are escaped — so '%2d' becomes '-' and '%c3%a9' becomes '%C3%A9'. 'decode' additionally unescapes characters that are legal literals in that component, including '%2F' in a path, which — combined with dot_segments — reveals a disguised traversal like '%2E%2E%2F'. 'preserve' leaves every escape byte-for-byte and only reorders and filters. A lone '%' is never an error: it is escaped as '%25'." },
                    "drop_index": { "type": "boolean", "default": false, "description": "Remove a trailing directory-index file from the path, leaving the directory slash — '/blog/index.html' becomes '/blog/'. Recognizes index.html, index.htm, index.php, index.asp, index.aspx, index.jsp, index.cgi, index.shtml, index.xhtml and the default.* equivalents, matched case-insensitively. Off by default because not every server serves the two forms identically." },
                    "trailing_slash": { "type": "string", "enum": ["preserve", "add", "remove"], "default": "preserve", "description": "Trailing-slash policy for the path. 'preserve' (default) leaves it alone; 'add' appends a slash to directory-style paths but never to a last segment that looks like a file ('/sitemap.xml' stays put); 'remove' strips trailing slashes. The site root always stays a single '/'. Applied after drop_index, so '/blog/index.html' with both set becomes '/blog/' or '/blog'." },
                    "sort_query": { "type": "string", "enum": ["key", "key-value", "none"], "default": "key", "description": "How to order the query parameters — this is what makes '?b=2&a=1' and '?a=1&b=2' collapse to one canonical string. 'key' (default) sorts alphabetically by parameter name; 'key-value' also orders repeats of the same name by value; 'none' keeps the original order. Sorting is stable, so equally-ranked parameters keep the order you gave them." },
                    "dedupe_query": { "type": "boolean", "default": false, "description": "Drop repeated query parameters that are byte-identical after encoding normalization, keeping the first — so '?a=1&a=1' becomes '?a=1' while a genuinely multi-valued 'tag=a&tag=b' survives intact. Off by default because a repeat can be meaningful to some applications." },
                    "drop_empty_params": { "type": "boolean", "default": false, "description": "Remove query parameters with no value — both 'a=' and a bare valueless 'flag'. Off by default, because an empty value is meaningful to some applications; turn it on to clear the leftovers an unfilled form appends to a URL. If every parameter goes, the '?' goes with them." },
                    "drop_tracking": { "type": "boolean", "default": false, "description": "Remove the usual analytics and click-ID query parameters — the utm_*, pk_*, mtm_*, ga_*, _hs* families plus fbclid, gclid, msclkid, yclid, igshid, mkt_tok and friends. Off by default because canonicalizing and stripping are separate decisions. For allow/deny lists and custom parameter names use the dedicated URL query normalizer instead." },
                    "drop_fragment": { "type": "boolean", "default": false, "description": "Remove the '#fragment' entirely. Off by default, since a fragment is part of the address a user shared. An EMPTY fragment (a bare trailing '#') is always dropped, because it addresses the same resource. Turn this on for the SEO canonical form, where the fragment is never part of the indexed URL." },
                    "dedupe_urls": { "type": "boolean", "default": false, "description": "Drop lines that normalize to a URL an earlier line already produced, keeping the first occurrence and the original order. This is the point of canonicalizing a crawl export: 'https://Example.com/a?b=1&a=2' and 'https://example.com/a?a=2&b=1' are one page. Default false." },
                    "on_invalid": { "type": "string", "enum": ["keep", "drop", "error"], "default": "keep", "description": "What to do with a line that cannot be parsed — a host containing a space or an illegal character, an unterminated IPv6 literal, a non-numeric port, a control character. 'keep' (default) passes it through untouched so an annotated list survives a round trip, 'drop' leaves it out of the result, 'error' fails the run and names the line number and the reason." },
                    "output": { "type": "string", "enum": ["urls", "changed", "report", "summary"], "default": "urls", "description": "What to return: 'urls' (default) is every line normalized, one per line; 'changed' is only the URLs that actually differ from the input, which is the canonical/redirect list worth acting on; 'report' is a line,original,normalized,action CSV covering every input line, where action is normalized, unchanged, duplicate or invalid; 'summary' is a metric,value CSV of the run totals." }
                },
                "required": ["urls"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
