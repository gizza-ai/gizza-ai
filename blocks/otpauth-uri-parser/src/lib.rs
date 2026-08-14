//! gizza-ai/otpauth-uri-parser — parse a single `otpauth://` provisioning URI (the Key
//! Uri Format) into its fields: type, issuer, account, base32 secret, algorithm, digits,
//! and period/counter. Pure → all backends. Chat schema single-sourced from descriptor();
//! handle() delegates to run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    uri: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    mask_secret: bool,
    #[serde(default)]
    strict: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("uri")
                .required()
                .describe("The otpauth:// provisioning URI to parse, e.g. otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&digits=6&period=30. Line breaks in a pasted URI are ignored; the limit is 4096 characters."),
        )
        .param(
            Param::enumv("format", ["json", "text", "table"]).default("json").describe(
                "Output format: json (default) returns every field plus warnings as an object; text returns aligned key: value lines; table returns an ASCII field/value table.",
            ),
        )
        .param(
            Param::boolean("mask_secret").default(false).describe(
                "Set true to replace the secret with asterisks so the result is safe to paste into a ticket or screenshot. The reported secret_chars and secret_bytes stay accurate. Default false.",
            ),
        )
        .param(
            Param::boolean("strict").default(false).describe(
                "Set true to enforce the Key Uri Format's recommendations as errors instead of warnings: a missing issuer, an issuer mismatch between label and query, a digit count other than 6 or 8, a duplicate parameter, or any parameter outside the spec will fail. Default false (report them as warnings).",
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
    name = "gizza-ai/otpauth-uri-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an otpauth:// 2FA URI into its fields",
    skill(
        description = "Parse a single otpauth:// provisioning URI (the Key Uri Format authenticator apps read from a 2FA QR code) into its fields: type (totp or hotp), issuer, account name, base32 secret, algorithm, digits, and period or counter. The label is percent-decoded and split on its first colon, the secret is normalized to unpadded upper-case base32 and validated against RFC 4648, and the spec defaults (SHA1, 6 digits, 30-second period) are applied and reported under defaults_applied. Warnings flag an issuer that differs between the label and the issuer parameter, a missing issuer, non-standard digit counts, duplicate or unknown query parameters, and a counter on a totp URI. Set format=json (default), text, or table; mask_secret=true hides the secret while keeping its length; strict=true turns those warnings into errors. Runs locally — the secret never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "otpauth-uri-parser", |a: Args| {
            gizza_ai_otpauth_uri_parser_core::run_with_options(
                &a.uri,
                a.format.as_deref().unwrap_or("json"),
                a.mask_secret,
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
                    "uri": { "type": "string", "description": "The otpauth:// provisioning URI to parse, e.g. otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&digits=6&period=30. Line breaks in a pasted URI are ignored; the limit is 4096 characters." },
                    "format": { "type": "string", "enum": ["json", "text", "table"], "default": "json", "description": "Output format: json (default) returns every field plus warnings as an object; text returns aligned key: value lines; table returns an ASCII field/value table." },
                    "mask_secret": { "type": "boolean", "default": false, "description": "Set true to replace the secret with asterisks so the result is safe to paste into a ticket or screenshot. The reported secret_chars and secret_bytes stay accurate. Default false." },
                    "strict": { "type": "boolean", "default": false, "description": "Set true to enforce the Key Uri Format's recommendations as errors instead of warnings: a missing issuer, an issuer mismatch between label and query, a digit count other than 6 or 8, a duplicate parameter, or any parameter outside the spec will fail. Default false (report them as warnings)." }
                },
                "required": ["uri"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
