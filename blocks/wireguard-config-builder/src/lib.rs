//! gizza-ai/wireguard-config-builder — chat skill block on the shared tool abstraction.
//! Builds and validates a complete WireGuard [Interface]/[Peer] config from entered
//! keys, addresses, endpoints and AllowedIPs. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to block_utils::run_skill.
//! Pure compute — no host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_wireguard_config_builder_core::{build, WgInput};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    private_key: String,
    address: String,
    #[serde(default)]
    listen_port: Option<u32>,
    #[serde(default)]
    dns: String,
    #[serde(default)]
    mtu: Option<u32>,
    peer_public_key: String,
    #[serde(default)]
    preshared_key: String,
    allowed_ips: String,
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    persistent_keepalive: Option<u32>,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("private_key").required().describe(
            "Interface PrivateKey: the base64 WireGuard (Curve25519) private key for THIS \
machine, a 44-character string ending in '=' (e.g. from 'wg genkey'). Validated as base64 of \
exactly 32 bytes. This tool never generates keys — paste one you created.",
        ))
        .param(Param::string("address").required().describe(
            "Interface Address: the IP(s) assigned to this WireGuard interface, as a \
comma-separated list of CIDRs, e.g. '10.0.0.2/32' or '10.0.0.2/32, fd00::2/128'. Each entry is \
validated as an IPv4/IPv6 address with a prefix (0-32 for IPv4, 0-128 for IPv6).",
        ))
        .param(Param::integer("listen_port").describe(
            "Interface ListenPort: the UDP port WireGuard listens on, 1-65535 (servers commonly \
use 51820). Optional — omit for a roaming client that uses a random port.",
        ))
        .param(Param::string("dns").default("").describe(
            "Interface DNS: comma-separated DNS server IP addresses pushed to this interface, \
e.g. '1.1.1.1' or '1.1.1.1, 8.8.8.8'. Plain IPs only (no prefix). Optional; default empty (line \
omitted).",
        ))
        .param(Param::integer("mtu").describe(
            "Interface MTU in bytes, 576-9000. WireGuard's default is 1420; lower it (e.g. 1280) \
only if you see fragmentation. Optional (line omitted when unset).",
        ))
        .param(Param::string("peer_public_key").required().describe(
            "Peer PublicKey: the base64 WireGuard public key of the OTHER end (server or peer), a \
44-character string ending in '='. Validated as base64 of exactly 32 bytes.",
        ))
        .param(Param::string("preshared_key").default("").describe(
            "Peer PresharedKey: an optional base64 32-byte pre-shared key added for post-quantum \
defence-in-depth (from 'wg genpsk'). Validated as base64 of 32 bytes when given. Optional; \
default empty (line omitted).",
        ))
        .param(Param::string("allowed_ips").required().describe(
            "Peer AllowedIPs: comma-separated CIDRs this peer is allowed to send/receive, e.g. \
'0.0.0.0/0, ::/0' for a full tunnel (route everything) or '10.0.0.0/24' for a split tunnel (only \
that subnet). Each entry is validated as a CIDR.",
        ))
        .param(Param::string("endpoint").default("").describe(
            "Peer Endpoint: the peer's reachable 'host:port', e.g. 'vpn.example.com:51820' or \
'203.0.113.5:51820'; an IPv6 literal must be bracketed, '[2001:db8::1]:51820'. Optional (a \
server listening for roaming clients may omit it); default empty.",
        ))
        .param(Param::integer("persistent_keepalive").describe(
            "Peer PersistentKeepalive in seconds, 0-65535. Set to 25 when this side is behind NAT \
to keep the tunnel open; 0/omit disables it. Optional (line omitted when unset).",
        ))
        .param(Param::enumv("format", ["conf", "json"]).default("conf").describe(
            "Output format. 'conf' (default) is the ready-to-save WireGuard config file text. \
'json' is a machine-readable object with the parsed interface and peer fields.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/wireguard-config-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build and validate a complete WireGuard interface/peer config from entered keys, endpoints and AllowedIPs.",
    skill(
        description = "Build and validate a complete WireGuard configuration file from values you \
enter — it does NOT generate keys (paste keys from 'wg genkey'/'wg pubkey'). Set 'private_key' \
(interface PrivateKey) and 'address' (interface Address CIDRs), plus the peer's 'peer_public_key' \
and 'allowed_ips'. Optionally add 'listen_port', 'dns', 'mtu' for the [Interface], and \
'preshared_key', 'endpoint', 'persistent_keepalive' for the [Peer]. Every key is validated as \
base64 of exactly 32 bytes, every Address/AllowedIPs/DNS entry is parsed as a valid IP/CIDR, and \
the Endpoint host:port and all port/MTU/keepalive ranges are checked, so an invalid value is \
reported instead of producing a broken file. Set 'format' to 'conf' (default, the ready-to-save \
wg0.conf text) or 'json' (parsed interface + peer object).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "wireguard-config-builder", |a: Args| {
            let input = WgInput {
                private_key: a.private_key,
                address: a.address,
                listen_port: a.listen_port,
                dns: a.dns,
                mtu: a.mtu,
                peer_public_key: a.peer_public_key,
                preshared_key: a.preshared_key,
                allowed_ips: a.allowed_ips,
                endpoint: a.endpoint,
                persistent_keepalive: a.persistent_keepalive,
            };
            build(&input, &a.format).map_err(SkillError::InvalidArgs)
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
                    "private_key": {
                        "type": "string",
                        "description": "Interface PrivateKey: the base64 WireGuard (Curve25519) private key for THIS machine, a 44-character string ending in '=' (e.g. from 'wg genkey'). Validated as base64 of exactly 32 bytes. This tool never generates keys — paste one you created."
                    },
                    "address": {
                        "type": "string",
                        "description": "Interface Address: the IP(s) assigned to this WireGuard interface, as a comma-separated list of CIDRs, e.g. '10.0.0.2/32' or '10.0.0.2/32, fd00::2/128'. Each entry is validated as an IPv4/IPv6 address with a prefix (0-32 for IPv4, 0-128 for IPv6)."
                    },
                    "listen_port": {
                        "type": "integer",
                        "description": "Interface ListenPort: the UDP port WireGuard listens on, 1-65535 (servers commonly use 51820). Optional — omit for a roaming client that uses a random port."
                    },
                    "dns": {
                        "type": "string",
                        "default": "",
                        "description": "Interface DNS: comma-separated DNS server IP addresses pushed to this interface, e.g. '1.1.1.1' or '1.1.1.1, 8.8.8.8'. Plain IPs only (no prefix). Optional; default empty (line omitted)."
                    },
                    "mtu": {
                        "type": "integer",
                        "description": "Interface MTU in bytes, 576-9000. WireGuard's default is 1420; lower it (e.g. 1280) only if you see fragmentation. Optional (line omitted when unset)."
                    },
                    "peer_public_key": {
                        "type": "string",
                        "description": "Peer PublicKey: the base64 WireGuard public key of the OTHER end (server or peer), a 44-character string ending in '='. Validated as base64 of exactly 32 bytes."
                    },
                    "preshared_key": {
                        "type": "string",
                        "default": "",
                        "description": "Peer PresharedKey: an optional base64 32-byte pre-shared key added for post-quantum defence-in-depth (from 'wg genpsk'). Validated as base64 of 32 bytes when given. Optional; default empty (line omitted)."
                    },
                    "allowed_ips": {
                        "type": "string",
                        "description": "Peer AllowedIPs: comma-separated CIDRs this peer is allowed to send/receive, e.g. '0.0.0.0/0, ::/0' for a full tunnel (route everything) or '10.0.0.0/24' for a split tunnel (only that subnet). Each entry is validated as a CIDR."
                    },
                    "endpoint": {
                        "type": "string",
                        "default": "",
                        "description": "Peer Endpoint: the peer's reachable 'host:port', e.g. 'vpn.example.com:51820' or '203.0.113.5:51820'; an IPv6 literal must be bracketed, '[2001:db8::1]:51820'. Optional (a server listening for roaming clients may omit it); default empty."
                    },
                    "persistent_keepalive": {
                        "type": "integer",
                        "description": "Peer PersistentKeepalive in seconds, 0-65535. Set to 25 when this side is behind NAT to keep the tunnel open; 0/omit disables it. Optional (line omitted when unset)."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["conf", "json"],
                        "default": "conf",
                        "description": "Output format. 'conf' (default) is the ready-to-save WireGuard config file text. 'json' is a machine-readable object with the parsed interface and peer fields."
                    }
                },
                "required": ["private_key", "address", "peer_public_key", "allowed_ips"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
