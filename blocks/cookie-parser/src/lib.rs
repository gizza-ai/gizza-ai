//! gizza-ai/cookie-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    cookie: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_true")]
    decode: bool,
    #[serde(default)]
    raw_attributes: bool,
    #[serde(default = "default_true")]
    warnings: bool,
}

fn default_true() -> bool {
    true
}
fn default_mode() -> String {
    "auto".to_string()
}
fn default_format() -> String {
    "json".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("cookie")
                .required()
                .describe("The raw cookie header text. A request `Cookie:` header is one line of `name=value` pairs separated by `;` (e.g. `sessionid=abc123; theme=dark`); a response `Set-Cookie:` header is ONE cookie plus its attributes (e.g. `sid=abc; Domain=example.com; Path=/; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Secure; HttpOnly; SameSite=Lax`) — paste several, one per line. A pasted `Cookie:`/`Set-Cookie:` header name is stripped and a \"double-quoted\" value is unwrapped."),
        )
        .param(
            Param::enumv("mode", ["auto", "cookie", "set-cookie"])
                .default("auto")
                .describe("Which header direction to parse. `auto` (default) picks `set-cookie` when a line carries a `Set-Cookie:` name or any attribute (Path, Domain, Expires, Max-Age, Secure, HttpOnly, SameSite, Priority, Partitioned), otherwise `cookie`. `cookie`: force the request-header reading — every `;`-separated segment is a name/value pair, attributes included. `set-cookie`: force the response-header reading — one cookie per line, everything after the first `;` is attributes."),
        )
        .param(
            Param::enumv("format", ["json", "table", "csv", "markdown"])
                .default("json")
                .describe("Output format. `json` (default): `{ mode, count, cookies: [{ name, value, size, attributes: {…}, session, host_only }] }`, with `Expires` also normalized to ISO-8601 UTC as `expires_iso`. `table`: an aligned plain-text table. `csv`: a comma-separated table with a header row (values quoted when needed). `markdown`: a pipe table for pasting into docs or an issue."),
        )
        .param(
            Param::boolean("decode")
                .default(true)
                .describe("When true (default), percent-decode cookie names and values (`%2F`→`/`, `%20`→space). A `+` is kept literal — cookies are not form-urlencoded. Set false to keep values exactly as sent. Attribute values are never decoded, and `size` always counts the raw, undecoded bytes."),
        )
        .param(
            Param::boolean("raw_attributes")
                .default(false)
                .describe("When true, also echo each cookie verbatim: `raw` (the whole line as written) and, in `set-cookie` mode, `attributes_raw` (each attribute segment as written, before normalization). In `table`/`csv`/`markdown` this adds one extra column. Default false. Unrecognized attributes are always kept under `attributes.other` regardless of this flag."),
        )
        .param(
            Param::boolean("warnings")
                .default(true)
                .describe("When true (default), flag structural problems per cookie: `SameSite=None` without `Secure`, missing `Secure`/`HttpOnly`/`SameSite`, a cookie over 4096 bytes, both `Expires` and `Max-Age`, a `Max-Age` of 0 or less (a delete), an unparseable `Expires`, a leading dot in `Domain`, `Partitioned` without `Secure`, a duplicate name, and `__Host-`/`__Secure-` name-prefix violations. Set false for a clean machine-readable result."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cookie-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split a Cookie or Set-Cookie header into individual cookies with their attributes",
    skill(
        description = "Split a raw HTTP `Cookie:` or `Set-Cookie:` header into individual cookies with their attributes. Auto-detects the header direction (`mode`): a request `Cookie:` header is a flat `name=value; …` list, while each response `Set-Cookie:` line is one cookie plus attributes — Domain, Path, Expires, Max-Age, Secure, HttpOnly, SameSite, Priority, and Partitioned, with unrecognized attributes preserved. Paste many `Set-Cookie:` lines at once. `Expires` is also normalized to an ISO-8601 UTC timestamp, each cookie's byte size is reported against the common 4096-byte browser limit, and `session`/`host_only` are derived. Values are percent-decoded by default (`decode`) and RFC 6265 double-quoted values are unwrapped. `warnings` (on by default) flags `SameSite=None` without `Secure`, missing `Secure`/`HttpOnly`/`SameSite`, a deleting `Max-Age`, an unparseable date, `__Host-`/`__Secure-` prefix violations, and oversized cookies. Choose `format`: `json` (default), `table`, `csv`, or `markdown`. Deterministic — no clock is read, so no relative countdowns. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cookie-parser", |a: Args| {
            gizza_ai_cookie_parser_core::run(
                &a.cookie,
                &a.mode,
                &a.format,
                a.decode,
                a.raw_attributes,
                a.warnings,
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
                    "cookie": { "type": "string", "description": "The raw cookie header text. A request `Cookie:` header is one line of `name=value` pairs separated by `;` (e.g. `sessionid=abc123; theme=dark`); a response `Set-Cookie:` header is ONE cookie plus its attributes (e.g. `sid=abc; Domain=example.com; Path=/; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Secure; HttpOnly; SameSite=Lax`) — paste several, one per line. A pasted `Cookie:`/`Set-Cookie:` header name is stripped and a \"double-quoted\" value is unwrapped." },
                    "mode": { "type": "string", "enum": ["auto", "cookie", "set-cookie"], "default": "auto", "description": "Which header direction to parse. `auto` (default) picks `set-cookie` when a line carries a `Set-Cookie:` name or any attribute (Path, Domain, Expires, Max-Age, Secure, HttpOnly, SameSite, Priority, Partitioned), otherwise `cookie`. `cookie`: force the request-header reading — every `;`-separated segment is a name/value pair, attributes included. `set-cookie`: force the response-header reading — one cookie per line, everything after the first `;` is attributes." },
                    "format": { "type": "string", "enum": ["json", "table", "csv", "markdown"], "default": "json", "description": "Output format. `json` (default): `{ mode, count, cookies: [{ name, value, size, attributes: {…}, session, host_only }] }`, with `Expires` also normalized to ISO-8601 UTC as `expires_iso`. `table`: an aligned plain-text table. `csv`: a comma-separated table with a header row (values quoted when needed). `markdown`: a pipe table for pasting into docs or an issue." },
                    "decode": { "type": "boolean", "default": true, "description": "When true (default), percent-decode cookie names and values (`%2F`→`/`, `%20`→space). A `+` is kept literal — cookies are not form-urlencoded. Set false to keep values exactly as sent. Attribute values are never decoded, and `size` always counts the raw, undecoded bytes." },
                    "raw_attributes": { "type": "boolean", "default": false, "description": "When true, also echo each cookie verbatim: `raw` (the whole line as written) and, in `set-cookie` mode, `attributes_raw` (each attribute segment as written, before normalization). In `table`/`csv`/`markdown` this adds one extra column. Default false. Unrecognized attributes are always kept under `attributes.other` regardless of this flag." },
                    "warnings": { "type": "boolean", "default": true, "description": "When true (default), flag structural problems per cookie: `SameSite=None` without `Secure`, missing `Secure`/`HttpOnly`/`SameSite`, a cookie over 4096 bytes, both `Expires` and `Max-Age`, a `Max-Age` of 0 or less (a delete), an unparseable `Expires`, a leading dot in `Domain`, `Partitioned` without `Secure`, a duplicate name, and `__Host-`/`__Secure-` name-prefix violations. Set false for a clean machine-readable result." }
                },
                "required": ["cookie"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
