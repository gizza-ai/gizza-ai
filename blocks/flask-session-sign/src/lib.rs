//! gizza-ai/flask-session-sign — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_flask_session_sign_core::{
    CompressMode, DigestMethod, KeyDerivation, SecretEncoding, SignOptions,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    payload: String,
    secret: String,
    #[serde(default = "default_salt")]
    salt: String,
    #[serde(default = "default_secret_encoding")]
    secret_encoding: String,
    #[serde(default = "default_digest")]
    digest: String,
    #[serde(default = "default_key_derivation")]
    key_derivation: String,
    #[serde(default)]
    timestamp: i64,
    #[serde(default)]
    legacy_epoch: bool,
    #[serde(default = "default_compress")]
    compress: String,
    #[serde(default = "default_cookie_name")]
    cookie_name: String,
}

fn default_salt() -> String {
    "cookie-session".into()
}
fn default_secret_encoding() -> String {
    "utf8".into()
}
fn default_digest() -> String {
    "sha1".into()
}
fn default_key_derivation() -> String {
    "hmac".into()
}
fn default_compress() -> String {
    "auto".into()
}
fn default_cookie_name() -> String {
    "session".into()
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("payload").required().describe("Session data as a JSON object string, for example {\"user\":1,\"admin\":true}. Use JSON true/false/null, not Python literals."))
        .param(Param::string("secret").required().describe("The Flask SECRET_KEY used to sign the session cookie."))
        .param(Param::string("salt").default("cookie-session").describe("itsdangerous salt. Flask's SecureCookieSessionInterface uses cookie-session by default."))
        .param(Param::enumv("secret_encoding", ["utf8", "hex", "base64"]).default("utf8").describe("How to decode the secret text before signing: utf8 text, hex bytes, or base64 bytes. Default utf8."))
        .param(Param::enumv("digest", ["sha1", "sha256", "sha512"]).default("sha1").describe("Digest algorithm for key derivation and HMAC signature. Flask defaults to sha1."))
        .param(Param::enumv("key_derivation", ["hmac", "django-concat", "concat", "none"]).default("hmac").describe("itsdangerous key derivation method. Flask configures hmac; itsdangerous' own default is django-concat."))
        .param(Param::integer("timestamp").default(0.0).min(0.0).describe("Unix timestamp seconds to embed. Use 0 to sign with the current clock; set a fixed value for reproducible cookies."))
        .param(Param::boolean("legacy_epoch").default(false).describe("Encode timestamps as itsdangerous < 1.0 seconds since 2011-01-01 instead of full Unix seconds. Default false."))
        .param(Param::enumv("compress", ["auto", "always", "never"]).default("auto").describe("Payload compression mode. auto matches itsdangerous: zlib only when it saves more than one byte."))
        .param(Param::string("cookie_name").default("session").describe("Cookie name used in the Set-Cookie header and byte-limit warning. Default session."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/flask-session-sign",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sign Flask session cookies from JSON and a SECRET_KEY.",
    skill(
        description = "Build a Flask-compatible signed session cookie from a JSON object payload and SECRET_KEY. Defaults match Flask's SecureCookieSessionInterface: salt cookie-session, hmac key derivation, sha1 digest, automatic zlib compression, and cookie name session. Set timestamp for reproducible output, choose sha256/sha512 or alternate itsdangerous derivations for custom apps, and use hex/base64 secret encodings for byte keys. Returns the cookie, Set-Cookie header, serialized payload, signature segments, derived key, timestamp, size, and warnings.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "flask-session-sign", |a: Args| {
            let opts = SignOptions {
                secret: a.secret,
                salt: a.salt,
                secret_encoding: SecretEncoding::parse(&a.secret_encoding)
                    .map_err(SkillError::InvalidArgs)?,
                digest: DigestMethod::parse(&a.digest).map_err(SkillError::InvalidArgs)?,
                key_derivation: KeyDerivation::parse(&a.key_derivation)
                    .map_err(SkillError::InvalidArgs)?,
                timestamp: a.timestamp,
                legacy_epoch: a.legacy_epoch,
                compress: CompressMode::parse(&a.compress).map_err(SkillError::InvalidArgs)?,
                cookie_name: a.cookie_name,
                ..Default::default()
            };
            gizza_ai_flask_session_sign_core::sign_to_json(&a.payload, &opts)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
