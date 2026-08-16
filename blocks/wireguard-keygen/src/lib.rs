//! gizza-ai/wireguard-keygen — chat skill block on the shared tool abstraction.
//!
//! Generate WireGuard Curve25519 key pairs (and optional preshared keys) with a
//! ready-to-paste `wg0.conf` snippet. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`. Pure (`x25519-dalek` + the platform CSPRNG) → runs
//! on every backend including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_pairs")]
    pairs: u32,
    #[serde(default = "default_true")]
    preshared_key: bool,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_address")]
    address: String,
    #[serde(default = "default_endpoint")]
    endpoint: String,
}

fn default_pairs() -> u32 {
    1
}
fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "text".to_string()
}
fn default_address() -> String {
    "10.0.0.2/32".to_string()
}
fn default_endpoint() -> String {
    "vpn.example.com:51820".to_string()
}

impl Args {
    fn run(&self) -> Result<String, String> {
        gizza_ai_wireguard_keygen_core::run(
            self.pairs as f64,
            self.preshared_key,
            &self.format,
            &self.address,
            &self.endpoint,
        )
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::integer("pairs")
                .default(1)
                .min(1.0)
                .max(25.0)
                .describe(
                    "How many independent key pairs to generate, 1-25 (default 1). Use one pair \
                     per device or peer — never reuse a key pair across machines.",
                ),
        )
        .param(
            Param::boolean("preshared_key").default(true).describe(
                "Also generate a 32-byte preshared key per pair, like wg genpsk (default true). \
                 A preshared key adds a symmetric layer on top of Curve25519 and must be set \
                 identically on BOTH peers. Turn it off for a plain key pair.",
            ),
        )
        .param(
            Param::enumv("format", ["text", "json", "conf"])
                .default("text")
                .describe(
                    "Output shape. text lists PrivateKey/PublicKey/PresharedKey followed by an \
                     annotated wg0.conf snippet (default); json returns one object per pair with \
                     index, private_key, public_key, preshared_key and config; conf returns the \
                     wg0.conf snippet only.",
                ),
        )
        .param(
            Param::string("address")
                .default("10.0.0.2/32")
                .describe(
                    "Tunnel address for this device, written as CIDR and used for the snippet's \
                     [Interface] Address line (default 10.0.0.2/32). Accepts a comma-separated \
                     list and IPv6, e.g. '10.0.0.2/32, fd00::2/128'.",
                ),
        )
        .param(
            Param::string("endpoint")
                .default("vpn.example.com:51820")
                .describe(
                    "Server host:port for the snippet's [Peer] Endpoint line (default \
                     vpn.example.com:51820). IPv6 is written [fd00::1]:51820. Leave empty to omit \
                     the Endpoint line, which is what a listening server's own config wants.",
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
    name = "gizza-ai/wireguard-keygen",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate WireGuard key pairs and preshared keys with a wg0.conf snippet",
    skill(
        description = "Generate WireGuard Curve25519 key pairs locally and return the base64 PrivateKey and PublicKey exactly as wg genkey and wg pubkey print them, plus an optional preshared key like wg genpsk and a ready-to-paste wg0.conf snippet. The private key is clamped the same way wg genkey clamps it, so it is byte-indistinguishable from the real tool. Set pairs to generate up to 25 independent key pairs in one call (one per device). Choose format text for the key listing plus an annotated config snippet, json for one machine-readable object per pair, or conf for the snippet only. address sets the snippet's [Interface] Address and endpoint sets the [Peer] Endpoint (leave endpoint empty for a listening server). Keys are generated with the platform CSPRNG and never leave the device: this is pure local Rust/WASM with no network and no filesystem access. It does not derive a public key from a private key you already have, and it does not validate or assemble a full multi-peer config — use the wireguard-config-builder tool for that.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "wireguard-keygen", |a: Args| {
            a.run().map_err(SkillError::InvalidArgs)
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
                "type":"object",
                "properties":{
                    "pairs":{"type":"integer","minimum":1,"maximum":25,"default":1,"description":"How many independent key pairs to generate, 1-25 (default 1). Use one pair per device or peer — never reuse a key pair across machines."},
                    "preshared_key":{"type":"boolean","default":true,"description":"Also generate a 32-byte preshared key per pair, like wg genpsk (default true). A preshared key adds a symmetric layer on top of Curve25519 and must be set identically on BOTH peers. Turn it off for a plain key pair."},
                    "format":{"type":"string","enum":["text","json","conf"],"default":"text","description":"Output shape. text lists PrivateKey/PublicKey/PresharedKey followed by an annotated wg0.conf snippet (default); json returns one object per pair with index, private_key, public_key, preshared_key and config; conf returns the wg0.conf snippet only."},
                    "address":{"type":"string","default":"10.0.0.2/32","description":"Tunnel address for this device, written as CIDR and used for the snippet's [Interface] Address line (default 10.0.0.2/32). Accepts a comma-separated list and IPv6, e.g. '10.0.0.2/32, fd00::2/128'."},
                    "endpoint":{"type":"string","default":"vpn.example.com:51820","description":"Server host:port for the snippet's [Peer] Endpoint line (default vpn.example.com:51820). IPv6 is written [fd00::1]:51820. Leave empty to omit the Endpoint line, which is what a listening server's own config wants."}
                },
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_layer_generates_a_wg_shaped_pair() {
        let a = Args {
            pairs: default_pairs(),
            preshared_key: default_true(),
            format: default_format(),
            address: default_address(),
            endpoint: default_endpoint(),
        };
        let out = a.run().unwrap();
        assert!(out.contains("PrivateKey   = "));
        assert!(out.contains("PublicKey    = "));
        assert!(out.contains("PresharedKey = "));
        assert!(out.contains("Endpoint = vpn.example.com:51820"));
    }

    #[test]
    fn args_layer_reports_a_bad_address() {
        let a = Args {
            pairs: 1,
            preshared_key: false,
            format: "text".into(),
            address: "10.0.0.2".into(),
            endpoint: String::new(),
        };
        assert!(a.run().unwrap_err().contains("no prefix length"));
    }
}
