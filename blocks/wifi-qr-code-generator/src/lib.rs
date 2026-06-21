//! gizza-ai/wifi-qr-code-generator — generate a Wi-Fi join QR code (SVG) from
//! SSID, password and security type.
//!
//! Pure-Rust (qrcode), so it runs on ALL backends incl. the chat Service Worker.
//! The SVG is wrapped as an `image/svg+xml` data-URL envelope. Surfaces: chat +
//! CLI (image-bytes output → no page, like the chart tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_wifi_qr_code_generator_core::{generate_svg, Security};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    ssid: String,
    #[serde(default)]
    password: String,
    #[serde(default = "default_security")]
    security: String,
    #[serde(default)]
    hidden: bool,
}
fn default_security() -> String {
    "WPA".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("ssid").required().describe("The Wi-Fi network name (SSID)."))
        .param(
            Param::string("password")
                .describe("The Wi-Fi password (required for WPA/WEP; omit for an open network)."),
        )
        .param(
            Param::enumv("security", ["WPA", "WEP", "nopass"]).default("WPA").describe(
                "Security type: WPA (default, covers WPA/WPA2/WPA3), WEP, or nopass (open network).",
            ),
        )
        .param(
            Param::boolean("hidden")
                .default(false)
                .describe("Set true if the network's SSID is hidden (not broadcast)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct WifiQr;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/wifi-qr-code-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a Wi-Fi join QR code",
    skill(
        description = "Generate a Wi-Fi join QR code from an SSID, password and security type, as an SVG. Scanning it with a phone camera offers to connect to the network — no typing the password. security is WPA (default; covers WPA/WPA2/WPA3), WEP, or nopass (open); set hidden=true for a non-broadcast SSID. Returns an SVG image. Runs locally — the credentials never leave the device.",
        parameters = schema_json()
    ),
)]
impl WifiQr {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("wifi-qr-code-generator")?;
    let security = Security::parse(&args.security).map_err(SkillError::InvalidArgs)?;
    let svg = generate_svg(&args.ssid, &args.password, security, args.hidden)
        .map_err(SkillError::InvalidArgs)?;
    let safe = args.ssid.replace(['/', '\\', ' '], "-");
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("wifi-{safe}.svg"),
        format!("Wi-Fi QR code for '{}' ({} bytes SVG)", args.ssid, svg.len()),
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
                    "ssid": { "type": "string", "description": "The Wi-Fi network name (SSID)." },
                    "password": { "type": "string", "description": "The Wi-Fi password (required for WPA/WEP; omit for an open network)." },
                    "security": { "type": "string", "enum": ["WPA", "WEP", "nopass"], "default": "WPA", "description": "Security type: WPA (default, covers WPA/WPA2/WPA3), WEP, or nopass (open network)." },
                    "hidden": { "type": "boolean", "default": false, "description": "Set true if the network's SSID is hidden (not broadcast)." }
                },
                "required": ["ssid"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
