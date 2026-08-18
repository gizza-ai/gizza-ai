//! gizza-ai/recovery-code-generator — generate a sheet of one-time
//! account-recovery ("2FA backup") codes, optionally with the SHA-256 digests a
//! service stores instead of the codes themselves. BIP39 recovery PHRASES are a
//! different deliverable and live in `bip39-mnemonic-generator`; this block is
//! codes only. Chat schema single-sourced from descriptor() (which also drives
//! the CLI); handler delegates to run_skill. Pure (getrandom CSPRNG) → all
//! backends incl. the chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_recovery_code_generator_core::{run, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_count")]
    count: u64,
    #[serde(default = "default_blocks")]
    blocks: u64,
    #[serde(default = "default_chars_per_block")]
    chars_per_block: u64,
    #[serde(default = "default_charset")]
    charset: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_hash")]
    hash: String,
    #[serde(default)]
    seed_hex: String,
}

fn default_count() -> u64 {
    10
}
fn default_blocks() -> u64 {
    2
}
fn default_chars_per_block() -> u64 {
    5
}
fn default_charset() -> String {
    "lowercase".into()
}
fn default_separator() -> String {
    "-".into()
}
fn default_output() -> String {
    "numbered".into()
}
fn default_hash() -> String {
    "none".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::integer("count")
                .min(1.0)
                .max(50.0)
                .default(10)
                .describe("How many recovery codes to generate (1-50). Default 10, the number most services issue per sheet."),
        )
        .param(
            Param::integer("blocks")
                .min(1.0)
                .max(6.0)
                .default(2)
                .describe("How many character groups make up each code (1-6). Default 2, e.g. \"a1b2c-3d4e5\". Use 1 for an ungrouped code with no separator."),
        )
        .param(
            Param::integer("chars_per_block")
                .min(2.0)
                .max(16.0)
                .default(5)
                .describe("Characters in each group (2-16). Default 5. Code length is blocks x chars_per_block, so 2-96 characters; 10 characters over the default alphabet is about 52 bits."),
        )
        .param(
            Param::enumv(
                "charset",
                ["lowercase", "alphanumeric", "uppercase", "numeric", "unambiguous", "hex"],
            )
            .default("lowercase")
            .describe("Alphabet the characters are drawn from: 'lowercase' (default, a-z0-9), 'alphanumeric' (A-Za-z0-9), 'uppercase' (A-Z0-9), 'numeric' (0-9), 'unambiguous' (A-Z0-9 with the look-alikes 0/O, 1/I/L and U removed, best for printing and reading aloud), or 'hex' (0-9a-f). No punctuation in any of them — recovery codes get typed by hand."),
        )
        .param(
            Param::string("separator")
                .default("-")
                .describe("Text placed between the groups, at most 3 characters, e.g. \"-\" (default), \"_\" or \".\". Pass an empty string for no separator. A separator that is also a character of the chosen alphabet is rejected (it would make a printed code ambiguous), as are whitespace, commas and quotes (they would break the printed and csv output)."),
        )
        .param(
            Param::enumv("output", ["numbered", "plain", "csv", "json"])
                .default("numbered")
                .describe("How the sheet is rendered: 'numbered' (default, a 1..N printable list), 'plain' (one code per line, no numbering), 'csv' (index,code[,hash] with a header row) or 'json' (codes plus code_length, alphabet_size and entropy figures). 'numbered' and 'plain' also print a one-line entropy summary."),
        )
        .param(
            Param::enumv("hash", ["none", "sha256", "sha256-salted"])
                .default("none")
                .describe("Digest to publish next to each code, for the SERVICE side that must store codes hashed rather than in the clear: 'none' (default), 'sha256' (hex digest of the code exactly as displayed) or 'sha256-salted' (a fresh random 16-byte salt per code, printed as salt_hex:digest_hex)."),
        )
        .param(
            Param::string("seed_hex")
                .default("")
                .describe("Optional 8-128 hex digits (4-64 bytes). Blank (the default) draws from the platform's cryptographic RNG, so every run differs. When set, the whole sheet is derived deterministically from the seed so the same seed always reproduces the same codes — useful for tests and shareable links, but such a sheet is only as secret as the seed."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RecoveryCodeGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/recovery-code-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate one-time account-recovery backup codes",
    skill(
        description = "Generate a sheet of one-time account-recovery (2FA backup) codes locally with a cryptographic RNG. Each code is `blocks` groups of `chars_per_block` characters joined by `separator`, drawn uniformly (no modulo bias) from a `charset` alphabet — 'lowercase' (default), 'alphanumeric', 'uppercase', 'numeric', 'unambiguous' (look-alikes removed) or 'hex'. `count` sets how many. Render as a numbered sheet, a plain list, csv or json, with the entropy per code and for the whole set. `hash` optionally emits the SHA-256 (or salted SHA-256) digest a service should store instead of the code. Set `seed_hex` to derive the sheet deterministically from a seed instead of the RNG. For BIP39 recovery PHRASES use the bip39-mnemonic-generator tool instead.",
        parameters = schema_json()
    )
)]
impl RecoveryCodeGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "recovery-code-generator", |a: Args| {
            run(&Options {
                count: a.count as usize,
                blocks: a.blocks as usize,
                chars_per_block: a.chars_per_block as usize,
                charset: a.charset,
                separator: a.separator,
                output: a.output,
                hash: a.hash,
                seed_hex: a.seed_hex,
            })
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
                    "count": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10, "description": "How many recovery codes to generate (1-50). Default 10, the number most services issue per sheet." },
                    "blocks": { "type": "integer", "minimum": 1, "maximum": 6, "default": 2, "description": "How many character groups make up each code (1-6). Default 2, e.g. \"a1b2c-3d4e5\". Use 1 for an ungrouped code with no separator." },
                    "chars_per_block": { "type": "integer", "minimum": 2, "maximum": 16, "default": 5, "description": "Characters in each group (2-16). Default 5. Code length is blocks x chars_per_block, so 2-96 characters; 10 characters over the default alphabet is about 52 bits." },
                    "charset": { "type": "string", "enum": ["lowercase", "alphanumeric", "uppercase", "numeric", "unambiguous", "hex"], "default": "lowercase", "description": "Alphabet the characters are drawn from: 'lowercase' (default, a-z0-9), 'alphanumeric' (A-Za-z0-9), 'uppercase' (A-Z0-9), 'numeric' (0-9), 'unambiguous' (A-Z0-9 with the look-alikes 0/O, 1/I/L and U removed, best for printing and reading aloud), or 'hex' (0-9a-f). No punctuation in any of them — recovery codes get typed by hand." },
                    "separator": { "type": "string", "default": "-", "description": "Text placed between the groups, at most 3 characters, e.g. \"-\" (default), \"_\" or \".\". Pass an empty string for no separator. A separator that is also a character of the chosen alphabet is rejected (it would make a printed code ambiguous), as are whitespace, commas and quotes (they would break the printed and csv output)." },
                    "output": { "type": "string", "enum": ["numbered", "plain", "csv", "json"], "default": "numbered", "description": "How the sheet is rendered: 'numbered' (default, a 1..N printable list), 'plain' (one code per line, no numbering), 'csv' (index,code[,hash] with a header row) or 'json' (codes plus code_length, alphabet_size and entropy figures). 'numbered' and 'plain' also print a one-line entropy summary." },
                    "hash": { "type": "string", "enum": ["none", "sha256", "sha256-salted"], "default": "none", "description": "Digest to publish next to each code, for the SERVICE side that must store codes hashed rather than in the clear: 'none' (default), 'sha256' (hex digest of the code exactly as displayed) or 'sha256-salted' (a fresh random 16-byte salt per code, printed as salt_hex:digest_hex)." },
                    "seed_hex": { "type": "string", "default": "", "description": "Optional 8-128 hex digits (4-64 bytes). Blank (the default) draws from the platform's cryptographic RNG, so every run differs. When set, the whole sheet is derived deterministically from the seed so the same seed always reproduces the same codes — useful for tests and shareable links, but such a sheet is only as secret as the seed." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_reproduce_the_core_defaults() {
        let a: Args = serde_json::from_str("{}").unwrap();
        let d = Options::default();
        assert_eq!(a.count as usize, d.count);
        assert_eq!(a.blocks as usize, d.blocks);
        assert_eq!(a.chars_per_block as usize, d.chars_per_block);
        assert_eq!(a.charset, d.charset);
        assert_eq!(a.separator, d.separator);
        assert_eq!(a.output, d.output);
        assert_eq!(a.hash, d.hash);
        assert_eq!(a.seed_hex, d.seed_hex);
    }
}
