//! gizza-ai/payment-qr — build a BIP-21 (or generic text) payment URI and render
//! it as a scannable QR code SVG.
//!
//! Pure-Rust (`qrcode` + `bs58` + a hand-rolled bech32 verifier), so it runs on
//! ALL backends including the chat Service Worker. The SVG is wrapped as an
//! `image/svg+xml` data-URL envelope for chat/CLI; the standalone page renders the
//! same SVG inline.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_payment_qr_core::{Options, MAX_SIZE, MIN_SIZE};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    address: String,
    #[serde(default = "default_scheme")]
    scheme: String,
    #[serde(default)]
    amount: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    message: String,
    #[serde(default = "default_ecc")]
    error_correction: String,
    #[serde(default = "default_size")]
    size: u32,
    #[serde(default = "default_foreground")]
    foreground: String,
    #[serde(default = "default_background")]
    background: String,
    #[serde(default = "default_show_uri")]
    show_uri: bool,
}

fn default_scheme() -> String {
    "bitcoin".to_string()
}
fn default_ecc() -> String {
    "M".to_string()
}
fn default_size() -> u32 {
    512
}
fn default_foreground() -> String {
    "#000000".to_string()
}
fn default_background() -> String {
    "#ffffff".to_string()
}
fn default_show_uri() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("address")
                .required()
                .describe(
                    "The payment destination: a Bitcoin/Litecoin/Dogecoin address, an Ethereum 0x address, a BOLT-11 Lightning invoice, or — with scheme=text — any payload to encode verbatim. Addresses are checksum-verified (Base58Check, Bech32/Bech32m), so a typo is rejected rather than encoded. Example: bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4",
                ),
        )
        .param(
            Param::enumv(
                "scheme",
                ["bitcoin", "litecoin", "dogecoin", "ethereum", "lightning", "text"],
            )
            .default("bitcoin")
            .describe(
                "Which URI to build. bitcoin/litecoin/dogecoin use the BIP-21 grammar (scheme:address?amount=&label=&message=); ethereum uses EIP-681 (ethereum:0x…?value=<wei>); lightning wraps a BOLT-11 invoice as lightning:<invoice>; text encodes the address field verbatim with no scheme prefix. Default bitcoin.",
            ),
        )
        .param(
            Param::string("amount")
                .describe(
                    "Optional amount to request, in whole coin units with a period as the decimal separator (BIP-21 forbids commas and exponent notation). BTC/LTC/DOGE allow up to 8 decimal places; ETH allows 18 and is converted to exact wei for the EIP-681 value. Not accepted for lightning (the invoice carries its own amount) or text. Example: 0.025",
                ),
        )
        .param(
            Param::string("label")
                .describe(
                    "Optional BIP-21 label — who is being paid, shown by the payer's wallet before they confirm. UTF-8 is percent-encoded automatically. Bitcoin, Litecoin and Dogecoin only. Example: Coffee Bar",
                ),
        )
        .param(
            Param::string("message")
                .describe(
                    "Optional BIP-21 message — a note about what the payment is for, shown alongside the label. UTF-8 is percent-encoded automatically. Bitcoin, Litecoin and Dogecoin only. Example: Invoice 2026-114",
                ),
        )
        .param(
            Param::enumv("error_correction", ["L", "M", "Q", "H"])
                .default("M")
                .describe(
                    "QR error-correction level. L is densest and scans fastest on screen; M is the default; Q and H survive smudges, folds and printing but make the code denser. Use Q or H for anything printed.",
                ),
        )
        .param(
            Param::integer("size")
                .default(512)
                .min(MIN_SIZE as f64)
                .max(MAX_SIZE as f64)
                .describe(
                    "Width of the rendered SVG in pixels (128-2048, default 512). The output is vector, so this only sets the default display size — it stays sharp at any print scale.",
                ),
        )
        .param(
            Param::string("foreground")
                .default("#000000")
                .describe(
                    "Colour of the dark QR modules and the caption text, as a hex value (#rgb, #rrggbb, #rrggbbaa) or a CSS colour name. Keep strong contrast against the background or scanners will fail. Default #000000.",
                ),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe(
                    "Colour behind the QR code, as a hex value or a CSS colour name; 'transparent' is accepted for overlaying on artwork. Default #ffffff.",
                ),
        )
        .param(
            Param::boolean("show_uri")
                .default(true)
                .describe(
                    "Print the full payment URI as wrapped monospace text under the code, so a payer can read or retype it if scanning fails. Default true. The URI is always in the SVG <title> for screen readers either way.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PaymentQr;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/payment-qr",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a BIP-21 payment URI and render it as a QR code",
    skill(
        description = "Build a BIP-21 payment URI (or a generic text payload) and render it as a scannable QR code SVG. Set scheme to bitcoin, litecoin or dogecoin for BIP-21 (scheme:address?amount=&label=&message=), ethereum for EIP-681 (value converted to exact wei), lightning to wrap a BOLT-11 invoice, or text to encode the address field verbatim. Addresses are checksum-verified — Base58Check for legacy/P2SH and Bech32/Bech32m for SegWit and Taproot — so a mistyped address is rejected instead of silently producing a QR that pays nowhere. label and message are percent-encoded per RFC 3986. Tune error_correction (L/M/Q/H), size, foreground/background colours, and show_uri to print the URI under the code. Returns an SVG image. Runs locally — the address never leaves the device.",
        parameters = schema_json()
    ),
)]
impl PaymentQr {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("payment-qr")?;
    let opts = Options {
        address: &args.address,
        scheme: &args.scheme,
        amount: &args.amount,
        label: &args.label,
        message: &args.message,
        error_correction: &args.error_correction,
        size: args.size,
        foreground: &args.foreground,
        background: &args.background,
        show_uri: args.show_uri,
    };
    let (uri, svg) = gizza_ai_payment_qr_core::render(&opts).map_err(SkillError::InvalidArgs)?;
    let scheme = args.scheme.trim().to_ascii_lowercase();
    let scheme = if scheme.is_empty() {
        "bitcoin"
    } else {
        &scheme
    };
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("payment-qr-{}.svg", scheme.replace(['/', '\\', ' '], "-")),
        format!("Payment QR code for {uri}"),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "address": { "type": "string", "description": "The payment destination: a Bitcoin/Litecoin/Dogecoin address, an Ethereum 0x address, a BOLT-11 Lightning invoice, or — with scheme=text — any payload to encode verbatim. Addresses are checksum-verified (Base58Check, Bech32/Bech32m), so a typo is rejected rather than encoded. Example: bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4" },
                    "scheme": { "type": "string", "enum": ["bitcoin", "litecoin", "dogecoin", "ethereum", "lightning", "text"], "default": "bitcoin", "description": "Which URI to build. bitcoin/litecoin/dogecoin use the BIP-21 grammar (scheme:address?amount=&label=&message=); ethereum uses EIP-681 (ethereum:0x…?value=<wei>); lightning wraps a BOLT-11 invoice as lightning:<invoice>; text encodes the address field verbatim with no scheme prefix. Default bitcoin." },
                    "amount": { "type": "string", "description": "Optional amount to request, in whole coin units with a period as the decimal separator (BIP-21 forbids commas and exponent notation). BTC/LTC/DOGE allow up to 8 decimal places; ETH allows 18 and is converted to exact wei for the EIP-681 value. Not accepted for lightning (the invoice carries its own amount) or text. Example: 0.025" },
                    "label": { "type": "string", "description": "Optional BIP-21 label — who is being paid, shown by the payer's wallet before they confirm. UTF-8 is percent-encoded automatically. Bitcoin, Litecoin and Dogecoin only. Example: Coffee Bar" },
                    "message": { "type": "string", "description": "Optional BIP-21 message — a note about what the payment is for, shown alongside the label. UTF-8 is percent-encoded automatically. Bitcoin, Litecoin and Dogecoin only. Example: Invoice 2026-114" },
                    "error_correction": { "type": "string", "enum": ["L", "M", "Q", "H"], "default": "M", "description": "QR error-correction level. L is densest and scans fastest on screen; M is the default; Q and H survive smudges, folds and printing but make the code denser. Use Q or H for anything printed." },
                    "size": { "type": "integer", "default": 512, "minimum": 128, "maximum": 2048, "description": "Width of the rendered SVG in pixels (128-2048, default 512). The output is vector, so this only sets the default display size — it stays sharp at any print scale." },
                    "foreground": { "type": "string", "default": "#000000", "description": "Colour of the dark QR modules and the caption text, as a hex value (#rgb, #rrggbb, #rrggbbaa) or a CSS colour name. Keep strong contrast against the background or scanners will fail. Default #000000." },
                    "background": { "type": "string", "default": "#ffffff", "description": "Colour behind the QR code, as a hex value or a CSS colour name; 'transparent' is accepted for overlaying on artwork. Default #ffffff." },
                    "show_uri": { "type": "boolean", "default": true, "description": "Print the full payment URI as wrapped monospace text under the code, so a payer can read or retype it if scanning fails. Default true. The URI is always in the SVG <title> for screen readers either way." }
                },
                "required": ["address"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
