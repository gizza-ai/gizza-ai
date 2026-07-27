//! gizza-ai/aws-sigv4-signer — chat skill block on the shared tool abstraction.
//!
//! Computes an AWS Signature Version 4 (AWS4-HMAC-SHA256) request signature:
//! the canonical request, the string to sign, the signature, and the final
//! `Authorization` header (plus the full header set and a ready-to-run cURL).
//! Pure Rust HMAC/SHA-256 (RustCrypto) → runs on ALL backends including the chat
//! Service Worker. Surfaces: chat + CLI + standalone page.
//!
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. When the caller leaves
//! `amz_date` blank, this surface fills the current UTC time.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    url: String,
    region: String,
    service: String,
    access_key: String,
    secret_key: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    session_token: String,
    #[serde(default)]
    payload: String,
    #[serde(default)]
    headers: String,
    #[serde(default)]
    amz_date: String,
    #[serde(default)]
    unsigned_payload: bool,
    #[serde(default)]
    sign_content_sha256: bool,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("url")
                .required()
                .describe("Full request URL including path and query string, e.g. https://examplebucket.s3.amazonaws.com/test.txt or https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08. The Host header, canonical URI, and canonical query string are derived from this."),
        )
        .param(
            Param::string("region")
                .required()
                .describe("AWS region code the request targets, e.g. us-east-1, eu-west-1. Used in the credential scope and signing-key derivation."),
        )
        .param(
            Param::string("service")
                .required()
                .describe("AWS service code, e.g. s3, iam, execute-api, dynamodb, ec2. Used in the credential scope. 's3' also switches on S3's raw-path (no dot-segment normalization) rule."),
        )
        .param(
            Param::string("access_key")
                .required()
                .describe("AWS access key ID, e.g. AKIAIOSFODNN7EXAMPLE. Appears in the Credential of the Authorization header."),
        )
        .param(
            Param::string("secret_key")
                .required()
                .describe("AWS secret access key. Used only to derive the signing key locally; it never appears in the output. Runs entirely in your browser / locally — nothing is uploaded."),
        )
        .param(
            Param::enumv("method", ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"])
                .default("GET")
                .describe("HTTP request method. Default GET."),
        )
        .param(
            Param::string("session_token")
                .describe("Optional AWS STS session token for temporary credentials. When set, it is added as the x-amz-security-token header and included in the signature. Leave blank for long-term credentials."),
        )
        .param(
            Param::string("payload")
                .describe("Request body used to compute the SHA-256 payload hash. Leave blank for bodyless requests (GET/HEAD) — the hash of the empty string is used. Ignored when unsigned_payload is true."),
        )
        .param(
            Param::string("headers")
                .describe("Additional request headers to sign, one 'Name: Value' per line (e.g. 'content-type: application/json'). Host and x-amz-date are added automatically; add any other x-amz-* or content-type headers you will send. Values are lowercased-name, whitespace-trimmed, and sorted per the SigV4 rules."),
        )
        .param(
            Param::string("amz_date")
                .describe("Request timestamp in ISO-8601 basic UTC 'YYYYMMDDTHHMMSSZ' (e.g. 20150830T123600Z); the extended form '2015-08-30T12:36:00Z' is also accepted. Leave blank to use the current UTC time. Provide a fixed value to reproduce a known signature."),
        )
        .param(
            Param::boolean("unsigned_payload")
                .default(false)
                .describe("Sign with the literal 'UNSIGNED-PAYLOAD' instead of hashing the body. Common for large S3 uploads. Default false."),
        )
        .param(
            Param::boolean("sign_content_sha256")
                .default(false)
                .describe("Add and sign an x-amz-content-sha256 header (set to the payload hash or UNSIGNED-PAYLOAD). Required by Amazon S3. Default false."),
        )
        .param(
            Param::enumv(
                "output",
                ["all", "authorization", "headers", "canonical-request", "string-to-sign", "signature", "curl"],
            )
            .default("all")
            .describe("Which artifact to return. 'all' (default) is a labelled multi-section report; 'authorization' is just the Authorization header value; 'headers' is every header to send; 'canonical-request' / 'string-to-sign' / 'signature' are the individual signing artifacts; 'curl' is a ready-to-run cURL command."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/aws-sigv4-signer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute an AWS Signature Version 4 (SigV4) request signature — canonical request, string to sign, and Authorization header",
    skill(
        description = "Compute an AWS Signature Version 4 (AWS4-HMAC-SHA256) request signature. Given the request (url, method, headers, payload), the AWS region and service, and your credentials (access_key, secret_key, optional STS session_token), it builds the canonical request, the string to sign, the credential scope, the signature, and the final Authorization header — plus the full set of headers to send and a ready-to-run cURL command (choose with the `output` param). Set amz_date to reproduce a fixed timestamp or leave it blank for the current UTC time; set unsigned_payload for S3 streaming uploads and sign_content_sha256 to add S3's x-amz-content-sha256 header. All HMAC/SHA-256 runs locally — the secret key is never transmitted. This is header-based SigV4 auth; SigV4a (ECDSA) and query-string presigned URLs are not produced.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "aws-sigv4-signer", |a: Args| {
            // Fill the timestamp from the chat/CLI clock when the caller left it blank.
            let amz_date = if a.amz_date.trim().is_empty() {
                let epoch = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                gizza_ai_aws_sigv4_signer_core::format_amz_date(epoch)
            } else {
                a.amz_date.clone()
            };
            gizza_ai_aws_sigv4_signer_core::sign(
                &a.method,
                &a.url,
                &a.region,
                &a.service,
                &a.access_key,
                &a.secret_key,
                &a.session_token,
                &a.payload,
                &a.headers,
                &amz_date,
                a.unsigned_payload,
                a.sign_content_sha256,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Full request URL including path and query string, e.g. https://examplebucket.s3.amazonaws.com/test.txt or https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08. The Host header, canonical URI, and canonical query string are derived from this." },
                    "region": { "type": "string", "description": "AWS region code the request targets, e.g. us-east-1, eu-west-1. Used in the credential scope and signing-key derivation." },
                    "service": { "type": "string", "description": "AWS service code, e.g. s3, iam, execute-api, dynamodb, ec2. Used in the credential scope. 's3' also switches on S3's raw-path (no dot-segment normalization) rule." },
                    "access_key": { "type": "string", "description": "AWS access key ID, e.g. AKIAIOSFODNN7EXAMPLE. Appears in the Credential of the Authorization header." },
                    "secret_key": { "type": "string", "description": "AWS secret access key. Used only to derive the signing key locally; it never appears in the output. Runs entirely in your browser / locally — nothing is uploaded." },
                    "method": { "type": "string", "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"], "default": "GET", "description": "HTTP request method. Default GET." },
                    "session_token": { "type": "string", "description": "Optional AWS STS session token for temporary credentials. When set, it is added as the x-amz-security-token header and included in the signature. Leave blank for long-term credentials." },
                    "payload": { "type": "string", "description": "Request body used to compute the SHA-256 payload hash. Leave blank for bodyless requests (GET/HEAD) — the hash of the empty string is used. Ignored when unsigned_payload is true." },
                    "headers": { "type": "string", "description": "Additional request headers to sign, one 'Name: Value' per line (e.g. 'content-type: application/json'). Host and x-amz-date are added automatically; add any other x-amz-* or content-type headers you will send. Values are lowercased-name, whitespace-trimmed, and sorted per the SigV4 rules." },
                    "amz_date": { "type": "string", "description": "Request timestamp in ISO-8601 basic UTC 'YYYYMMDDTHHMMSSZ' (e.g. 20150830T123600Z); the extended form '2015-08-30T12:36:00Z' is also accepted. Leave blank to use the current UTC time. Provide a fixed value to reproduce a known signature." },
                    "unsigned_payload": { "type": "boolean", "default": false, "description": "Sign with the literal 'UNSIGNED-PAYLOAD' instead of hashing the body. Common for large S3 uploads. Default false." },
                    "sign_content_sha256": { "type": "boolean", "default": false, "description": "Add and sign an x-amz-content-sha256 header (set to the payload hash or UNSIGNED-PAYLOAD). Required by Amazon S3. Default false." },
                    "output": { "type": "string", "enum": ["all", "authorization", "headers", "canonical-request", "string-to-sign", "signature", "curl"], "default": "all", "description": "Which artifact to return. 'all' (default) is a labelled multi-section report; 'authorization' is just the Authorization header value; 'headers' is every header to send; 'canonical-request' / 'string-to-sign' / 'signature' are the individual signing artifacts; 'curl' is a ready-to-run cURL command." }
                },
                "required": ["url", "region", "service", "access_key", "secret_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
