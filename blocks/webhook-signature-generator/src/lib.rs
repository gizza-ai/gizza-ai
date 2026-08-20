//! gizza-ai/webhook-signature-generator — chat skill block on the shared tool abstraction.
//!
//! Computes the signing header(s) a webhook provider would send for a given raw
//! request body — Stripe, GitHub, Slack, Shopify, Standard Webhooks/Svix,
//! Square, Twilio, Paddle, or a custom HMAC scheme — so a delivery can be
//! replayed against your own receiver. Pure Rust HMAC (RustCrypto) → runs on
//! ALL backends including the chat Service Worker. Surfaces: chat + CLI + page.
//!
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. When the caller leaves
//! `timestamp` blank, this surface fills the current Unix time.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    payload: String,
    secret: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    message_id: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    secret_encoding: String,
    #[serde(default)]
    template: String,
    #[serde(default)]
    header_name: String,
    #[serde(default)]
    signature_prefix: String,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("payload")
                .required()
                .describe("The raw request body to sign, byte for byte, e.g. {\"id\":\"evt_1\",\"type\":\"payment_intent.succeeded\"}. Every provider signs the exact bytes it sent, so re-serializing or pretty-printing the JSON changes the signature. Maximum 2 MiB."),
        )
        .param(
            Param::string("secret")
                .required()
                .describe("The endpoint's signing secret, e.g. whsec_… (Stripe/Standard Webhooks), a GitHub webhook secret, a Slack signing secret, or a Twilio auth token. Used only as the local HMAC key — it is never transmitted and never appears in the output."),
        )
        .param(
            Param::enumv(
                "provider",
                [
                    "stripe",
                    "github",
                    "slack",
                    "shopify",
                    "standard-webhooks",
                    "svix",
                    "square",
                    "twilio",
                    "paddle",
                    "custom",
                ],
            )
            .default("stripe")
            .describe("Which provider's signing scheme to reproduce. Each one fixes what gets signed, the hash, and the header layout: stripe (Stripe-Signature: t=…,v1=hex over '<timestamp>.<body>'), github (X-Hub-Signature-256: sha256=hex over the body, plus the legacy sha1 header), slack (X-Slack-Signature: v0=hex over 'v0:<timestamp>:<body>'), shopify (X-Shopify-Hmac-SHA256: base64 over the body), standard-webhooks / svix (v1,base64 over '<id>.<timestamp>.<body>' with a base64-decoded whsec_ secret), square (base64 over '<url><body>'), twilio (X-Twilio-Signature: base64 HMAC-SHA1 over the URL plus alphabetically sorted form fields), paddle (Paddle-Signature: ts=…;h1=hex over '<timestamp>:<body>'), or custom for your own template. Default stripe."),
        )
        .param(
            Param::string("timestamp")
                .describe("Signing timestamp for the schemes that use one (stripe, slack, standard-webhooks, svix, paddle). Accepts Unix SECONDS (e.g. 1700000000) or ISO-8601 (e.g. 2023-11-14T22:13:20Z, with optional offset). Leave blank to use the current time. Receivers reject stale timestamps, so use 'now' to replay live and a fixed value to reproduce a known signature. Ignored by schemes that sign the body alone."),
        )
        .param(
            Param::string("message_id")
                .describe("Message id signed by the standard-webhooks and svix schemes and sent as the webhook-id / svix-id header, e.g. msg_2KWPBgLlAfxdpx2AI54pPJ85f4W. Leave blank to derive a stable id from the payload so repeated runs stay reproducible. Ignored by other providers unless a custom template uses {id}."),
        )
        .param(
            Param::string("url")
                .describe("Destination endpoint URL, e.g. https://example.com/webhook. REQUIRED for square and twilio, which sign the URL together with the body (include the query string exactly as the provider would call it). For every other provider it is optional and is used only as the target of the generated cURL command."),
        )
        .param(
            Param::enumv("algorithm", ["md5", "sha1", "sha256", "sha384", "sha512"])
                .default("sha256")
                .describe("Hash underlying the HMAC. Only applies when provider is 'custom' — every named provider's algorithm is fixed by its spec (a mismatched choice is ignored and reported in the notes). Default sha256."),
        )
        .param(
            Param::enumv("encoding", ["hex", "hex-upper", "base64", "base64url"])
                .default("hex")
                .describe("How the HMAC bytes are rendered in the header. Only applies when provider is 'custom'; named providers use the encoding their spec mandates. Default hex (lowercase)."),
        )
        .param(
            Param::enumv("secret_encoding", ["auto", "text", "hex", "base64"])
                .default("auto")
                .describe("How to turn the secret string into key bytes. 'auto' (default) follows the provider: standard-webhooks and svix secrets are base64 with the whsec_ prefix stripped, everything else is used as literal UTF-8 text (Stripe's whsec_ secret is text, NOT base64 — a common cause of signatures that never match). Use hex or base64 when your secret is stored encoded."),
        )
        .param(
            Param::string("template")
                .describe("Signed-string template for provider=custom. Placeholders: {payload}, {timestamp}, {id}, {url}; \\n and \\t become real newline/tab. Default {payload} (sign the body alone). Example: v0:{timestamp}:{payload}. Ignored by named providers."),
        )
        .param(
            Param::string("header_name")
                .describe("Header name to emit for provider=custom, e.g. X-Signature or X-Webhook-Signature. Default X-Signature. Ignored by named providers, which use their own header names."),
        )
        .param(
            Param::string("signature_prefix")
                .describe("Literal text placed before the encoded signature in the custom header value, e.g. 'sha256=' or 'v1='. Default empty (bare signature). Ignored by named providers."),
        )
        .param(
            Param::enumv(
                "output",
                ["all", "headers", "header", "signature", "signed-payload", "curl"],
            )
            .default("all")
            .describe("Which artifact to return. 'all' (default) is a labelled report with the signed bytes, signature, headers, cURL command and any notes; 'headers' is every header line to send; 'header' is just the primary signature header's value; 'signature' is the bare encoded HMAC; 'signed-payload' is the exact byte string that was HMAC'd (the fastest way to debug a mismatch); 'curl' is a ready-to-run replay command."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/webhook-signature-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute the signing header (HMAC + timestamp) for a webhook payload so you can replay or test it",
    skill(
        description = "Compute the signing header(s) a webhook provider would send for a raw request body, so you can replay or test a delivery against your own receiver. Pick the provider scheme — stripe, github, slack, shopify, standard-webhooks, svix, square, twilio, paddle, or custom — paste the exact body and the endpoint's signing secret, and it returns the signed byte string, the HMAC, the finished header lines, and a ready-to-run cURL command (choose with the `output` param). Timestamped schemes (stripe, slack, standard-webhooks, svix, paddle) take a `timestamp` in Unix seconds or ISO-8601, or use the current time when it is blank; square and twilio also need the destination `url` because they sign it with the body. `custom` exposes the raw knobs: a {payload}/{timestamp}/{id}/{url} template, algorithm (md5/sha1/sha256/sha384/sha512), output encoding (hex/hex-upper/base64/base64url), header name and prefix. All HMAC runs locally — the secret is never transmitted. This GENERATES signatures for testing your own endpoints; it does not verify an incoming signature and does not send the request for you. Asymmetric schemes (SendGrid ECDSA, PayPal certificates, Discord Ed25519) are not covered.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "webhook-signature-generator", |a: Args| {
            // Fill the timestamp from the chat/CLI clock when the caller left it blank.
            let timestamp = if a.timestamp.trim().is_empty() {
                let epoch = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                gizza_ai_webhook_signature_generator_core::format_timestamp(epoch)
            } else {
                a.timestamp.clone()
            };
            gizza_ai_webhook_signature_generator_core::run(
                &a.payload,
                &a.secret,
                &a.provider,
                &timestamp,
                &a.message_id,
                &a.url,
                &a.algorithm,
                &a.encoding,
                &a.secret_encoding,
                &a.template,
                &a.header_name,
                &a.signature_prefix,
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
                    "payload": { "type": "string", "description": "The raw request body to sign, byte for byte, e.g. {\"id\":\"evt_1\",\"type\":\"payment_intent.succeeded\"}. Every provider signs the exact bytes it sent, so re-serializing or pretty-printing the JSON changes the signature. Maximum 2 MiB." },
                    "secret": { "type": "string", "description": "The endpoint's signing secret, e.g. whsec_… (Stripe/Standard Webhooks), a GitHub webhook secret, a Slack signing secret, or a Twilio auth token. Used only as the local HMAC key — it is never transmitted and never appears in the output." },
                    "provider": { "type": "string", "enum": ["stripe", "github", "slack", "shopify", "standard-webhooks", "svix", "square", "twilio", "paddle", "custom"], "default": "stripe", "description": "Which provider's signing scheme to reproduce. Each one fixes what gets signed, the hash, and the header layout: stripe (Stripe-Signature: t=…,v1=hex over '<timestamp>.<body>'), github (X-Hub-Signature-256: sha256=hex over the body, plus the legacy sha1 header), slack (X-Slack-Signature: v0=hex over 'v0:<timestamp>:<body>'), shopify (X-Shopify-Hmac-SHA256: base64 over the body), standard-webhooks / svix (v1,base64 over '<id>.<timestamp>.<body>' with a base64-decoded whsec_ secret), square (base64 over '<url><body>'), twilio (X-Twilio-Signature: base64 HMAC-SHA1 over the URL plus alphabetically sorted form fields), paddle (Paddle-Signature: ts=…;h1=hex over '<timestamp>:<body>'), or custom for your own template. Default stripe." },
                    "timestamp": { "type": "string", "description": "Signing timestamp for the schemes that use one (stripe, slack, standard-webhooks, svix, paddle). Accepts Unix SECONDS (e.g. 1700000000) or ISO-8601 (e.g. 2023-11-14T22:13:20Z, with optional offset). Leave blank to use the current time. Receivers reject stale timestamps, so use 'now' to replay live and a fixed value to reproduce a known signature. Ignored by schemes that sign the body alone." },
                    "message_id": { "type": "string", "description": "Message id signed by the standard-webhooks and svix schemes and sent as the webhook-id / svix-id header, e.g. msg_2KWPBgLlAfxdpx2AI54pPJ85f4W. Leave blank to derive a stable id from the payload so repeated runs stay reproducible. Ignored by other providers unless a custom template uses {id}." },
                    "url": { "type": "string", "description": "Destination endpoint URL, e.g. https://example.com/webhook. REQUIRED for square and twilio, which sign the URL together with the body (include the query string exactly as the provider would call it). For every other provider it is optional and is used only as the target of the generated cURL command." },
                    "algorithm": { "type": "string", "enum": ["md5", "sha1", "sha256", "sha384", "sha512"], "default": "sha256", "description": "Hash underlying the HMAC. Only applies when provider is 'custom' — every named provider's algorithm is fixed by its spec (a mismatched choice is ignored and reported in the notes). Default sha256." },
                    "encoding": { "type": "string", "enum": ["hex", "hex-upper", "base64", "base64url"], "default": "hex", "description": "How the HMAC bytes are rendered in the header. Only applies when provider is 'custom'; named providers use the encoding their spec mandates. Default hex (lowercase)." },
                    "secret_encoding": { "type": "string", "enum": ["auto", "text", "hex", "base64"], "default": "auto", "description": "How to turn the secret string into key bytes. 'auto' (default) follows the provider: standard-webhooks and svix secrets are base64 with the whsec_ prefix stripped, everything else is used as literal UTF-8 text (Stripe's whsec_ secret is text, NOT base64 — a common cause of signatures that never match). Use hex or base64 when your secret is stored encoded." },
                    "template": { "type": "string", "description": "Signed-string template for provider=custom. Placeholders: {payload}, {timestamp}, {id}, {url}; \\n and \\t become real newline/tab. Default {payload} (sign the body alone). Example: v0:{timestamp}:{payload}. Ignored by named providers." },
                    "header_name": { "type": "string", "description": "Header name to emit for provider=custom, e.g. X-Signature or X-Webhook-Signature. Default X-Signature. Ignored by named providers, which use their own header names." },
                    "signature_prefix": { "type": "string", "description": "Literal text placed before the encoded signature in the custom header value, e.g. 'sha256=' or 'v1='. Default empty (bare signature). Ignored by named providers." },
                    "output": { "type": "string", "enum": ["all", "headers", "header", "signature", "signed-payload", "curl"], "default": "all", "description": "Which artifact to return. 'all' (default) is a labelled report with the signed bytes, signature, headers, cURL command and any notes; 'headers' is every header line to send; 'header' is just the primary signature header's value; 'signature' is the bare encoded HMAC; 'signed-payload' is the exact byte string that was HMAC'd (the fastest way to debug a mismatch); 'curl' is a ready-to-run replay command." }
                },
                "required": ["payload", "secret"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Every enum choice advertised to the model must actually be accepted by
    /// the core, and every provider must be reachable from the schema list.
    #[test]
    fn advertised_provider_enum_matches_the_core() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let listed: Vec<String> = schema["properties"]["provider"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            listed,
            gizza_ai_webhook_signature_generator_core::PROVIDERS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }
}
