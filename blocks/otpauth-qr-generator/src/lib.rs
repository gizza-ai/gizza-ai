//! gizza-ai/otpauth-qr-generator — render a scannable 2FA QR code from an
//! `otpauth://` provisioning URI (or its individual fields: issuer, account,
//! base32 secret, algorithm, digits, period/counter) so an authenticator app
//! can import the account by scanning it.
//!
//! Pure-Rust (`qrcode` + `image`), so it runs on ALL backends incl. the chat
//! Service Worker. The QR is wrapped as an `image/svg+xml` or `image/png`
//! data-URL envelope. Surfaces: chat + CLI (image-bytes output → no page, like
//! wifi-qr-code-generator and the chart tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_otpauth_qr_generator_core::{generate, Ecc, Fields, Format};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default)]
    uri: String,
    #[serde(default = "default_otp_type")]
    otp_type: String,
    #[serde(default)]
    issuer: String,
    #[serde(default)]
    account: String,
    #[serde(default)]
    secret: String,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_digits")]
    digits: u32,
    #[serde(default = "default_period")]
    period: u64,
    #[serde(default)]
    counter: u64,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_ecc")]
    error_correction: String,
    #[serde(default = "default_size")]
    size: u32,
    #[serde(default = "default_dark")]
    dark: String,
    #[serde(default = "default_light")]
    light: String,
}
fn default_otp_type() -> String {
    "totp".to_string()
}
fn default_algorithm() -> String {
    "sha1".to_string()
}
fn default_digits() -> u32 {
    6
}
fn default_period() -> u64 {
    30
}
fn default_format() -> String {
    "svg".to_string()
}
fn default_ecc() -> String {
    "M".to_string()
}
fn default_size() -> u32 {
    300
}
fn default_dark() -> String {
    "#000000".to_string()
}
fn default_light() -> String {
    "#ffffff".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("uri").describe(
            "A full otpauth:// provisioning URI to encode verbatim (e.g. otpauth://totp/Example:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example). If given, the individual fields below are ignored.",
        ))
        .param(
            Param::enumv("otp_type", ["totp", "hotp"]).default("totp").describe(
                "OTP type when building from fields: totp (time-based, default) or hotp (counter-based).",
            ),
        )
        .param(
            Param::string("issuer")
                .describe("The service/company name shown in the authenticator app (e.g. GitHub)."),
        )
        .param(Param::string("account").describe(
            "The account label, usually the username or email (e.g. alice@example.com). Required when building from fields.",
        ))
        .param(Param::string("secret").describe(
            "The base32-encoded shared secret (spaces and case are ignored). Required when building from fields.",
        ))
        .param(
            Param::enumv("algorithm", ["sha1", "sha256", "sha512"]).default("sha1").describe(
                "HMAC hash algorithm (default sha1, the standard for authenticator apps).",
            ),
        )
        .param(
            Param::integer("digits")
                .min(6.0)
                .max(8.0)
                .describe("Number of digits in the generated code (6, 7, or 8; default 6)."),
        )
        .param(
            Param::integer("period")
                .min(1.0)
                .max(600.0)
                .describe("TOTP time step in seconds (default 30; ignored for hotp)."),
        )
        .param(
            Param::integer("counter")
                .min(0.0)
                .describe("HOTP initial counter value (default 0; ignored for totp)."),
        )
        .param(
            Param::enumv("format", ["svg", "png"])
                .default("svg")
                .describe("Output image format: svg (default, crisp at any size) or png."),
        )
        .param(
            Param::enumv("error_correction", ["L", "M", "Q", "H"]).default("M").describe(
                "QR error-correction level: L (~7%), M (~15%, default), Q (~25%), or H (~30%). Higher survives more damage but makes a denser code.",
            ),
        )
        .param(
            Param::integer("size")
                .min(64.0)
                .max(2048.0)
                .describe("PNG image edge length in pixels (default 300; ignored for svg)."),
        )
        .param(
            Param::string("dark")
                .describe("Foreground (module) colour as #rgb or #rrggbb hex (default #000000)."),
        )
        .param(
            Param::string("light")
                .describe("Background colour as #rgb or #rrggbb hex (default #ffffff)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct OtpauthQrGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/otpauth-qr-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a scannable 2FA (otpauth) QR code",
    skill(
        description = "Generate a scannable 2FA QR code from an otpauth:// provisioning URI, or from the individual fields (issuer, account, base32 secret, algorithm, digits, period/counter). Scanning it with an authenticator app (Google Authenticator, Authy, 1Password…) imports the account — no manual typing. Pass a full uri to encode it verbatim, or leave uri empty and supply account + secret. otp_type is totp (default) or hotp. Output is an SVG (default) or PNG image; error_correction (L/M/Q/H), size (PNG px), and dark/light colours are configurable. Returns an image. Runs locally — the secret never leaves the device.",
        parameters = schema_json()
    ),
)]
impl OtpauthQrGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("otpauth-qr-generator")?;
    let format = Format::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let ecc = Ecc::parse(&args.error_correction).map_err(SkillError::InvalidArgs)?;
    let fields = Fields {
        otp_type: args.otp_type,
        issuer: args.issuer,
        account: args.account,
        secret: args.secret,
        algorithm: args.algorithm,
        digits: args.digits,
        period: args.period,
        counter: args.counter,
    };
    let g = generate(&args.uri, &fields, format, ecc, args.size, &args.dark, &args.light)
        .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &g.bytes,
        format.mime(),
        format!("otpauth-qr.{}", format.ext()),
        format!("2FA otpauth QR code ({} image, {} bytes)", format.ext().to_uppercase(), g.bytes.len()),
        MAX_OUTPUT_BYTES,
    )
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
                    "uri": { "type": "string", "description": "A full otpauth:// provisioning URI to encode verbatim (e.g. otpauth://totp/Example:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example). If given, the individual fields below are ignored." },
                    "otp_type": { "type": "string", "enum": ["totp", "hotp"], "default": "totp", "description": "OTP type when building from fields: totp (time-based, default) or hotp (counter-based)." },
                    "issuer": { "type": "string", "description": "The service/company name shown in the authenticator app (e.g. GitHub)." },
                    "account": { "type": "string", "description": "The account label, usually the username or email (e.g. alice@example.com). Required when building from fields." },
                    "secret": { "type": "string", "description": "The base32-encoded shared secret (spaces and case are ignored). Required when building from fields." },
                    "algorithm": { "type": "string", "enum": ["sha1", "sha256", "sha512"], "default": "sha1", "description": "HMAC hash algorithm (default sha1, the standard for authenticator apps)." },
                    "digits": { "type": "integer", "minimum": 6, "maximum": 8, "description": "Number of digits in the generated code (6, 7, or 8; default 6)." },
                    "period": { "type": "integer", "minimum": 1, "maximum": 600, "description": "TOTP time step in seconds (default 30; ignored for hotp)." },
                    "counter": { "type": "integer", "minimum": 0, "description": "HOTP initial counter value (default 0; ignored for totp)." },
                    "format": { "type": "string", "enum": ["svg", "png"], "default": "svg", "description": "Output image format: svg (default, crisp at any size) or png." },
                    "error_correction": { "type": "string", "enum": ["L", "M", "Q", "H"], "default": "M", "description": "QR error-correction level: L (~7%), M (~15%, default), Q (~25%), or H (~30%). Higher survives more damage but makes a denser code." },
                    "size": { "type": "integer", "minimum": 64, "maximum": 2048, "description": "PNG image edge length in pixels (default 300; ignored for svg)." },
                    "dark": { "type": "string", "description": "Foreground (module) colour as #rgb or #rrggbb hex (default #000000)." },
                    "light": { "type": "string", "description": "Background colour as #rgb or #rrggbb hex (default #ffffff)." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
