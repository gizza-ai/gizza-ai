//! gizza-ai/eth-vanity-address — chat skill block on the shared tool abstraction.
//! Grinds consecutive secp256k1 keys until one produces an Ethereum address
//! matching a chosen hex prefix and/or suffix, then returns the key material
//! and the search statistics. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(default)]
    match_case: bool,
    #[serde(default = "default_max_attempts")]
    max_attempts: u64,
    #[serde(default)]
    seed: String,
    #[serde(default = "default_output_format")]
    output_format: String,
}

fn default_max_attempts() -> u64 {
    gizza_ai_eth_vanity_address_core::DEFAULT_MAX_ATTEMPTS
}

fn default_output_format() -> String {
    "all".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("prefix")
                .default("")
                .describe(
                    "Hex characters the address must start with, right after the 0x (for example dead or c0ffee). Use 0-9 and a-f only; a leading 0x is ignored. Each extra character multiplies the search cost by 16, so 4-5 characters is the practical limit here. Leave empty to match on the suffix alone.",
                ),
        )
        .param(
            Param::string("suffix")
                .default("")
                .describe(
                    "Hex characters the address must end with (for example 1234). Same 0-9/a-f rules and same 16x cost per character as prefix. Prefix and suffix combined cannot exceed the 40 hex characters of an address; give at least one of the two.",
                ),
        )
        .param(
            Param::boolean("match_case")
                .default(false)
                .describe(
                    "When false (the default) the pattern is matched against the all-lowercase address, so case in the pattern is ignored. When true the pattern must match the EIP-55 checksummed address exactly, which doubles the search cost for every letter (a-f) in the pattern.",
                ),
        )
        .param(
            Param::integer("max_attempts")
                .default(100000)
                .min(1.0)
                .max(5000000.0)
                .describe(
                    "How many consecutive keys to test before giving up, 1 to 5000000 (default 100000). Expect roughly 16^n keys per hit for an n-character case-insensitive pattern: 4096 for 3 characters, 65536 for 4, about 1.05 million for 5. If no key matches, the tool reports the difficulty and the odds instead of returning an address.",
                ),
        )
        .param(
            Param::string("seed")
                .default("")
                .describe(
                    "Optional text that makes the search reproducible: the same seed with the same pattern always returns the same private key. Leave it empty (the default) to start from a key drawn from the platform's cryptographically secure random generator, which is what you want for a wallet you will actually use — a key derived from guessable seed text can be rediscovered by anyone.",
                ),
        )
        .param(
            Param::enumv(
                "output_format",
                ["all", "address", "private-key", "json", "estimate"],
            )
            .default("all")
            .describe(
                "What to return. all (default) prints the address, private key, public key and search statistics; address returns only the EIP-55 address; private-key returns only the 0x-prefixed 32-byte key; json returns every field plus difficulty as JSON; estimate skips the search entirely and just reports how hard the pattern is and the odds within max_attempts.",
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
    name = "gizza-ai/eth-vanity-address",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Grind a vanity Ethereum address matching a hex prefix or suffix, locally.",
    skill(
        description = "Search for an Ethereum vanity address: test consecutive secp256k1 private keys until one derives an address that starts with a chosen hex prefix and/or ends with a chosen hex suffix, matched either case-insensitively or against the EIP-55 checksum. Returns the EIP-55 address, the private key, the uncompressed public key and the search statistics, or the difficulty and odds when nothing matched. An estimate mode reports pattern difficulty without generating keys. Keys never leave the machine; leave the seed empty for CSPRNG randomness or set it for a reproducible search.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "eth-vanity-address", |a: Args| {
            let start = gizza_ai_eth_vanity_address_core::resolve_start_key(&a.seed)
                .map_err(SkillError::InvalidArgs)?;
            gizza_ai_eth_vanity_address_core::run(
                &a.prefix,
                &a.suffix,
                a.match_case,
                a.max_attempts,
                &a.output_format,
                &a.seed,
                &start,
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
    fn schema_matches_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let expected: serde_json::Value = serde_json::json!({
            "type": "object",
            "properties": {
                "prefix": {
                    "type": "string",
                    "default": "",
                    "description": actual["properties"]["prefix"]["description"],
                },
                "suffix": {
                    "type": "string",
                    "default": "",
                    "description": actual["properties"]["suffix"]["description"],
                },
                "match_case": {
                    "type": "boolean",
                    "default": false,
                    "description": actual["properties"]["match_case"]["description"],
                },
                "max_attempts": {
                    "type": "integer",
                    "default": 100000,
                    "minimum": 1,
                    "maximum": 5000000,
                    "description": actual["properties"]["max_attempts"]["description"],
                },
                "seed": {
                    "type": "string",
                    "default": "",
                    "description": actual["properties"]["seed"]["description"],
                },
                "output_format": {
                    "type": "string",
                    "enum": ["all", "address", "private-key", "json", "estimate"],
                    "default": "all",
                    "description": actual["properties"]["output_format"]["description"],
                },
            },
            "additionalProperties": false,
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for (name, prop) in schema["properties"].as_object().unwrap() {
            let desc = prop["description"].as_str().unwrap_or("");
            assert!(desc.len() > 30, "param {name} needs a real description");
        }
    }

    #[test]
    fn descriptor_default_matches_the_core_default() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(
            schema["properties"]["max_attempts"]["default"].as_u64().unwrap(),
            default_max_attempts()
        );
    }
}
