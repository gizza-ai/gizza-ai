//! gizza-ai/http-header-analyzer — parse a block of pasted HTTP response headers
//! and explain what each one does (caching, compression, content negotiation,
//! server hints, cookies, CORS) plus flag which recommended security headers are
//! missing. Chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_http_header_analyzer_core::run as analyze_headers;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    headers: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("headers")
            .required()
            .describe("The HTTP response headers to analyze, one 'Name: value' per line (optionally preceded by a status line such as 'HTTP/2 200'). A blank line ends the header block. Obsolete line-folded headers are joined."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HttpHeaderAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/http-header-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Analyze pasted HTTP response headers and explain caching, compression, server hints, and missing security headers",
    skill(
        description = "Parse a block of pasted HTTP response headers and explain them. For each header it returns a plain-English explanation and a category (caching, compression, content, security, cookie, cors, server, redirect, or other): it decodes Cache-Control/Expires/ETag caching policy, Content-Encoding compression, Content-Type and other content headers, Set-Cookie hardening (flags missing Secure/HttpOnly/SameSite), CORS Access-Control-* headers, and server-fingerprinting headers. It lists which recommended SECURITY headers are MISSING (Strict-Transport-Security, Content-Security-Policy, X-Content-Type-Options, X-Frame-Options, Referrer-Policy, Permissions-Policy) with a fix for each, and grades the VALUES of present ones (CSP unsafe-inline/unsafe-eval, short HSTS max-age that won't preload, weak Referrer-Policy, obsolete X-Frame-Options, deprecated X-XSS-Protection, and information-disclosure headers like Server/X-Powered-By/X-AspNet-Version). It returns an overall A+-to-F security_grade plus notes (no caching, no compression, charset missing). Accepts an optional leading status line and CRLF or bare-LF endings. Returns JSON. Runs locally.",
        parameters = schema_json()
    ),
)]
impl HttpHeaderAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "http-header-analyzer", |a: Args| {
            analyze_headers(&a.headers).map_err(SkillError::InvalidArgs)
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
                    "headers": { "type": "string", "description": "The HTTP response headers to analyze, one 'Name: value' per line (optionally preceded by a status line such as 'HTTP/2 200'). A blank line ends the header block. Obsolete line-folded headers are joined." }
                },
                "required": ["headers"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
