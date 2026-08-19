//! gizza-ai/authorization-header-decode — split an HTTP Authorization header into its
//! auth-scheme and credentials: base64-decode Basic into username/password, describe a
//! Bearer token's structure (JWT vs opaque, segments, JOSE header), parse Digest and
//! AWS4-HMAC-SHA256 auth-params, and identify Negotiate/NTLM blobs. Pure → all backends.
//! Chat schema single-sourced from descriptor(); handle() delegates to run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    header: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    mask_credentials: bool,
    #[serde(default)]
    strict: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("header")
                .required()
                .describe("The Authorization header to decode, with or without the leading 'Authorization:' name — e.g. 'Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==', 'Bearer eyJhbGciOiJIUzI1NiJ9.…', or 'Digest username=\"alice\", realm=\"example.com\", …'. A bare base64 credential with no scheme is accepted and reported as such. Line breaks are treated as spaces; the limit is 8192 characters."),
        )
        .param(
            Param::enumv("format", ["json", "text", "table"]).default("json").describe(
                "Output format: json (default) returns every field plus warnings as an object; text returns aligned key: value lines; table returns an ASCII field/value table.",
            ),
        )
        .param(
            Param::boolean("mask_credentials").default(false).describe(
                "Set true to replace the secret parts — the Basic password, the bearer token, a Digest response/cnonce, a signature or mac, and the raw credentials string — with asterisks while keeping their reported lengths, so the result is safe to paste into a ticket. Default false.",
            ),
        )
        .param(
            Param::boolean("strict").default(false).describe(
                "Set true to fail on anything this tool would otherwise report as a warning: a non-canonical scheme spelling, a missing scheme, a missing ':' in the decoded Basic credentials, URL-safe or unpadded base64, non-UTF-8 credentials, or an unknown scheme. Default false (report them under warnings).",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/authorization-header-decode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split an Authorization header into its scheme and decoded credentials",
    skill(
        description = "Decode an HTTP Authorization header. Splits the RFC 7235 auth-scheme from the credentials that follow, accepting a full 'Authorization: …' line, a bare '<scheme> <credentials>' value, or a naked base64/JWT credential with no scheme. Basic credentials are base64-decoded and split on the FIRST colon into username and password per RFC 7617. A Bearer token is described STRUCTURALLY — JWT, JWE or opaque, its segment count and lengths, its character set, and its decoded JOSE header (alg/typ/kid); payload claims are deliberately NOT decoded, so use a dedicated JWT decoder for claims and expiry. Digest, AWS4-HMAC-SHA256, Hawk, Signature and unknown schemes have their name=value auth-params parsed into a map (quoted values unescaped), and an AWS4 Credential= scope is split into access key, date, region and service. Negotiate/NTLM blobs are base64-decoded and identified from their NTLMSSP or SPNEGO signature. Warnings flag base64 being an encoding rather than encryption, a non-canonical scheme spelling, a missing colon, an empty password, non-UTF-8 credentials, URL-safe or unpadded base64, and line-wrapped input. Set format=json (default), text, or table; mask_credentials=true hides the secrets while keeping their lengths; strict=true turns the warnings into errors. Runs locally — the credentials never leave the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "authorization-header-decode", |a: Args| {
            gizza_ai_authorization_header_decode_core::run_with_options(
                &a.header,
                a.format.as_deref().unwrap_or("json"),
                a.mask_credentials,
                a.strict,
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
                    "header": { "type": "string", "description": "The Authorization header to decode, with or without the leading 'Authorization:' name — e.g. 'Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==', 'Bearer eyJhbGciOiJIUzI1NiJ9.…', or 'Digest username=\"alice\", realm=\"example.com\", …'. A bare base64 credential with no scheme is accepted and reported as such. Line breaks are treated as spaces; the limit is 8192 characters." },
                    "format": { "type": "string", "enum": ["json", "text", "table"], "default": "json", "description": "Output format: json (default) returns every field plus warnings as an object; text returns aligned key: value lines; table returns an ASCII field/value table." },
                    "mask_credentials": { "type": "boolean", "default": false, "description": "Set true to replace the secret parts — the Basic password, the bearer token, a Digest response/cnonce, a signature or mac, and the raw credentials string — with asterisks while keeping their reported lengths, so the result is safe to paste into a ticket. Default false." },
                    "strict": { "type": "boolean", "default": false, "description": "Set true to fail on anything this tool would otherwise report as a warning: a non-canonical scheme spelling, a missing scheme, a missing ':' in the decoded Basic credentials, URL-safe or unpadded base64, non-UTF-8 credentials, or an unknown scheme. Default false (report them under warnings)." }
                },
                "required": ["header"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
