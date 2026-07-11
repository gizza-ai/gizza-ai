//! gizza-ai/ip-log-anonymizer — mask or salted-hash IP addresses inside a log so
//! analytics stay GDPR-compliant. Chat schema single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill. Pure
//! → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ip_log_anonymizer_core::anonymize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_ipv4_octets")]
    ipv4_octets: u32,
    #[serde(default = "default_ipv6_groups")]
    ipv6_groups: u32,
    #[serde(default)]
    salt: String,
    #[serde(default = "default_hash_length")]
    hash_length: u32,
    #[serde(default = "default_replacement")]
    replacement: String,
    #[serde(default)]
    skip_private: bool,
}
fn default_mode() -> String {
    "mask".into()
}
fn default_ipv4_octets() -> u32 {
    1
}
fn default_ipv6_groups() -> u32 {
    5
}
fn default_hash_length() -> u32 {
    12
}
fn default_replacement() -> String {
    "[IP]".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The log or text to anonymize. Every valid IPv4 and IPv6 address is replaced in place; all other characters (paths, ports, timestamps) are left untouched."),
        )
        .param(
            Param::enumv("mode", ["mask", "hash", "redact"])
                .default("mask")
                .describe("How to anonymize each address: 'mask' zeros the trailing octets/hextets (GA/Matomo-style truncation, keeps geolocation); 'hash' replaces it with a short salted SHA-256 token (stable pseudonym, not reversible); 'redact' swaps in a fixed placeholder token."),
        )
        .param(
            Param::integer("ipv4_octets")
                .default(1)
                .min(0.0)
                .max(4.0)
                .describe("mask mode: how many trailing IPv4 octets to zero (1 = 203.0.113.45→203.0.113.0, the Google Analytics default; 2 matches Matomo's stricter default; 4 = 0.0.0.0). 0-4."),
        )
        .param(
            Param::integer("ipv6_groups")
                .default(5)
                .min(0.0)
                .max(8.0)
                .describe("mask mode: how many trailing IPv6 16-bit hextets to zero (5 = zero the last 80 bits, keeping the /48 prefix — the common GA/Cloudflare default; 8 zeros the whole address). 0-8."),
        )
        .param(
            Param::string("salt")
                .default("")
                .describe("hash mode: a secret string mixed in before hashing. Strongly recommended — a per-project random salt makes the pseudonyms un-guessable; an empty salt still hashes but is vulnerable to reverse lookup."),
        )
        .param(
            Param::integer("hash_length")
                .default(12)
                .min(4.0)
                .max(64.0)
                .describe("hash mode: how many hex characters of the SHA-256 digest to keep for each token (4-64; default 12). Longer = fewer collisions."),
        )
        .param(
            Param::string("replacement")
                .default("[IP]")
                .describe("redact mode: the placeholder token that replaces each address (e.g. '[IP]', '[REDACTED]', 'x.x.x.x')."),
        )
        .param(
            Param::boolean("skip_private")
                .default(false)
                .describe("When true, private/loopback/link-local addresses (10.x, 192.168.x, 127.x, fc00::/7, fe80::/10, …) are left untouched — they are not personal data. Default false anonymizes every address."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct IpLogAnonymizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ip-log-anonymizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Mask or salted-hash IP addresses in a log",
    skill(
        description = "Anonymize every IPv4 and IPv6 address inside a log or text for GDPR-compliant analytics, in place. Three modes: 'mask' zeros trailing octets/hextets like Google Analytics / Matomo (ipv4_octets default 1, ipv6_groups default 5 = keep the /48); 'hash' replaces each address with a short salted SHA-256 token (set `salt` to a secret; `hash_length` chars kept); 'redact' swaps a fixed `replacement` token. Set skip_private=true to leave internal 10.x/192.168.x/127.x/fc00::/fe80:: addresses untouched. Addresses are validated before replacement; ports, paths, and timestamps are preserved. Runs locally.",
        parameters = schema_json()
    ),
)]
impl IpLogAnonymizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ip-log-anonymizer", |a: Args| {
            anonymize(
                &a.text,
                &a.mode,
                a.ipv4_octets,
                a.ipv6_groups,
                &a.salt,
                a.hash_length,
                &a.replacement,
                a.skip_private,
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
                    "text": { "type": "string", "description": "The log or text to anonymize. Every valid IPv4 and IPv6 address is replaced in place; all other characters (paths, ports, timestamps) are left untouched." },
                    "mode": { "type": "string", "enum": ["mask", "hash", "redact"], "default": "mask", "description": "How to anonymize each address: 'mask' zeros the trailing octets/hextets (GA/Matomo-style truncation, keeps geolocation); 'hash' replaces it with a short salted SHA-256 token (stable pseudonym, not reversible); 'redact' swaps in a fixed placeholder token." },
                    "ipv4_octets": { "type": "integer", "default": 1, "minimum": 0, "maximum": 4, "description": "mask mode: how many trailing IPv4 octets to zero (1 = 203.0.113.45→203.0.113.0, the Google Analytics default; 2 matches Matomo's stricter default; 4 = 0.0.0.0). 0-4." },
                    "ipv6_groups": { "type": "integer", "default": 5, "minimum": 0, "maximum": 8, "description": "mask mode: how many trailing IPv6 16-bit hextets to zero (5 = zero the last 80 bits, keeping the /48 prefix — the common GA/Cloudflare default; 8 zeros the whole address). 0-8." },
                    "salt": { "type": "string", "default": "", "description": "hash mode: a secret string mixed in before hashing. Strongly recommended — a per-project random salt makes the pseudonyms un-guessable; an empty salt still hashes but is vulnerable to reverse lookup." },
                    "hash_length": { "type": "integer", "default": 12, "minimum": 4, "maximum": 64, "description": "hash mode: how many hex characters of the SHA-256 digest to keep for each token (4-64; default 12). Longer = fewer collisions." },
                    "replacement": { "type": "string", "default": "[IP]", "description": "redact mode: the placeholder token that replaces each address (e.g. '[IP]', '[REDACTED]', 'x.x.x.x')." },
                    "skip_private": { "type": "boolean", "default": false, "description": "When true, private/loopback/link-local addresses (10.x, 192.168.x, 127.x, fc00::/7, fe80::/10, …) are left untouched — they are not personal data. Default false anonymizes every address." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
