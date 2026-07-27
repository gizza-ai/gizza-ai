//! gizza-ai/cookie-string-to-json — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    cookie: String,
    #[serde(default = "default_true")]
    decode: bool,
    #[serde(default = "default_output")]
    output: String,
}

fn default_true() -> bool {
    true
}
fn default_output() -> String {
    "object".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("cookie")
                .required()
                .describe("The raw HTTP request `Cookie:` header string, e.g. `sessionid=abc123; theme=dark; path=%2Fhome`. Cookies are separated by `;` (newlines also work). A pasted `Cookie:`/`Set-Cookie:` header name and surrounding whitespace are stripped; a value wrapped in \"double quotes\" is unwrapped."),
        )
        .param(
            Param::boolean("decode")
                .default(true)
                .describe("When true (default), percent-decode names and values (`%2F`→`/`, `%20`→space). A `+` is kept literal (cookies are not form-urlencoded). Set false to return raw, still-encoded values."),
        )
        .param(
            Param::enumv("output", ["object", "pairs"])
                .default("object")
                .describe("Output shape. `object` (default): a `{ \"name\": \"value\" }` JSON object in source order, where a repeated cookie name collapses into an array of its values. `pairs`: an ordered array of `{ \"name\", \"value\" }` objects, the shape Selenium/Puppeteer/Playwright use, keeping every duplicate separately."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cookie-string-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a raw Cookie header string into JSON name/value pairs",
    skill(
        description = "Parse a raw HTTP request `Cookie:` header string (`name1=value1; name2=value2; …`) into JSON. Splits on `;` (and newlines), trims whitespace, strips a pasted `Cookie:`/`Set-Cookie:` header name, unwraps RFC 6265 double-quoted values, and by default percent-decodes names and values (a `+` stays literal — cookies are not form-urlencoded). Choose the output shape with `output`: `object` (default) returns a `{name: value}` JSON object in source order where a repeated name collapses into an array; `pairs` returns an ordered `[{name, value}]` array (the browser-automation shape) keeping every duplicate. Set `decode` false to keep values raw. This parses the request Cookie header's name/value list only — `Set-Cookie` attributes (expires, path, domain, Secure, HttpOnly) are not extracted. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cookie-string-to-json", |a: Args| {
            gizza_ai_cookie_string_to_json_core::run(&a.cookie, a.decode, &a.output)
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
                    "cookie": { "type": "string", "description": "The raw HTTP request `Cookie:` header string, e.g. `sessionid=abc123; theme=dark; path=%2Fhome`. Cookies are separated by `;` (newlines also work). A pasted `Cookie:`/`Set-Cookie:` header name and surrounding whitespace are stripped; a value wrapped in \"double quotes\" is unwrapped." },
                    "decode": { "type": "boolean", "default": true, "description": "When true (default), percent-decode names and values (`%2F`→`/`, `%20`→space). A `+` is kept literal (cookies are not form-urlencoded). Set false to return raw, still-encoded values." },
                    "output": { "type": "string", "enum": ["object", "pairs"], "default": "object", "description": "Output shape. `object` (default): a `{ \"name\": \"value\" }` JSON object in source order, where a repeated cookie name collapses into an array of its values. `pairs`: an ordered array of `{ \"name\", \"value\" }` objects, the shape Selenium/Puppeteer/Playwright use, keeping every duplicate separately." }
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
