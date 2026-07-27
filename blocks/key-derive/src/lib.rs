//! gizza-ai/key-derive — one unified key-derivation-function selector. Derives a
//! key of a chosen length from a passphrase/seed via PBKDF2, scrypt, Argon2id/i/d,
//! or HKDF, with each algorithm's own parameters. Chat schema single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure Rust → all backends. Unlike the focused argon2-hash block (PHC hash for
//! password storage), the Argon2 path here returns RAW chosen-length key material.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_key_derive_core::{derive, DeriveParams};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "d_algorithm")]
    algorithm: String,
    secret: String,
    #[serde(default = "d_enc_in")]
    input_encoding: String,
    #[serde(default)]
    salt: String,
    #[serde(default = "d_enc_in")]
    salt_encoding: String,
    #[serde(default = "d_length")]
    length: usize,
    #[serde(default = "d_encoding")]
    encoding: String,
    #[serde(default = "d_hash")]
    hash: String,
    #[serde(default = "d_iterations")]
    iterations: u32,
    #[serde(default = "d_n")]
    n: u32,
    #[serde(default = "d_r")]
    r: u32,
    #[serde(default = "d_p")]
    p: u32,
    #[serde(default = "d_memory_kib")]
    memory_kib: u32,
    #[serde(default = "d_time_cost")]
    time_cost: u32,
    #[serde(default = "d_parallelism")]
    parallelism: u32,
    #[serde(default = "d_variant")]
    argon2_variant: String,
    #[serde(default)]
    info: String,
    #[serde(default = "d_enc_in")]
    info_encoding: String,
}

fn d_algorithm() -> String {
    "pbkdf2".to_string()
}
fn d_enc_in() -> String {
    "utf8".to_string()
}
fn d_encoding() -> String {
    "hex".to_string()
}
fn d_hash() -> String {
    "sha256".to_string()
}
fn d_variant() -> String {
    "argon2id".to_string()
}
fn d_length() -> usize {
    32
}
fn d_iterations() -> u32 {
    100_000
}
fn d_n() -> u32 {
    16384
}
fn d_r() -> u32 {
    8
}
fn d_p() -> u32 {
    1
}
fn d_memory_kib() -> u32 {
    19456
}
fn d_time_cost() -> u32 {
    2
}
fn d_parallelism() -> u32 {
    1
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("algorithm", ["pbkdf2", "scrypt", "argon2", "hkdf"])
                .default("pbkdf2")
                .describe("Key-derivation function: pbkdf2 (default), scrypt, argon2 (raw Argon2id/i/d key material), or hkdf (RFC 5869, for high-entropy seeds/keys — not passwords)."),
        )
        .param(
            Param::string("secret")
                .required()
                .describe("The passphrase or input key material to derive from."),
        )
        .param(
            Param::enumv("input_encoding", ["utf8", "hex", "base64"])
                .default("utf8")
                .describe("How `secret` is encoded (default utf8)."),
        )
        .param(
            Param::string("salt")
                .describe("Salt (recommended; required for argon2, min 8 bytes). HKDF treats an empty salt as all-zeros per RFC 5869."),
        )
        .param(
            Param::enumv("salt_encoding", ["utf8", "hex", "base64"])
                .default("utf8")
                .describe("How `salt` is encoded (default utf8)."),
        )
        .param(
            Param::integer("length")
                .min(1.0)
                .max(1024.0)
                .default(32)
                .describe("Output key length in bytes (default 32)."),
        )
        .param(
            Param::enumv("encoding", ["hex", "base64"])
                .default("hex")
                .describe("Output encoding of the derived key (default hex)."),
        )
        .param(
            Param::enumv("hash", ["sha1", "sha256", "sha384", "sha512"])
                .default("sha256")
                .describe("Underlying hash for pbkdf2 (sha1/256/384/512) and hkdf (default sha256)."),
        )
        .param(
            Param::integer("iterations")
                .min(1.0)
                .default(100_000)
                .describe("PBKDF2 iteration count (pbkdf2 only; default 100000)."),
        )
        .param(
            Param::integer("n")
                .min(2.0)
                .default(16384)
                .describe("scrypt CPU/memory cost N, a power of two > 1 (scrypt only; default 16384)."),
        )
        .param(
            Param::integer("r")
                .min(1.0)
                .default(8)
                .describe("scrypt block size r (scrypt only; default 8)."),
        )
        .param(
            Param::integer("p")
                .min(1.0)
                .default(1)
                .describe("scrypt parallelization p (scrypt only; default 1)."),
        )
        .param(
            Param::integer("memory_kib")
                .min(8.0)
                .default(19456)
                .describe("Argon2 memory cost in KiB (argon2 only; default 19456 = 19 MiB)."),
        )
        .param(
            Param::integer("time_cost")
                .min(1.0)
                .default(2)
                .describe("Argon2 time (iteration) cost (argon2 only; default 2)."),
        )
        .param(
            Param::integer("parallelism")
                .min(1.0)
                .default(1)
                .describe("Argon2 parallelism / lanes (argon2 only; default 1)."),
        )
        .param(
            Param::enumv("argon2_variant", ["argon2id", "argon2i", "argon2d"])
                .default("argon2id")
                .describe("Argon2 variant (argon2 only; default argon2id)."),
        )
        .param(
            Param::string("info")
                .describe("HKDF context/application info string (hkdf only; optional)."),
        )
        .param(
            Param::enumv("info_encoding", ["utf8", "hex", "base64"])
                .default("utf8")
                .describe("How `info` is encoded (hkdf only; default utf8)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/key-derive",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Derive a key of any length via PBKDF2, scrypt, Argon2, or HKDF",
    skill(
        description = "One unified key-derivation-function selector: derive a raw key of a chosen byte length from a passphrase or seed via PBKDF2, scrypt, Argon2id/i/d, or HKDF (RFC 5869). Pick `algorithm` (default pbkdf2) and its own parameters — pbkdf2: hash + iterations; scrypt: n/r/p; argon2: memory_kib/time_cost/parallelism + argon2_variant (returns RAW chosen-length key material, not a PHC hash); hkdf: hash + info (for high-entropy seeds/keys, not passwords). `secret` and `salt` accept utf8/hex/base64 encodings; output is hex (default) or base64. Deterministic and runs locally — the secret never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "key-derive", |a: Args| {
            let params = DeriveParams {
                hash: a.hash,
                iterations: a.iterations,
                n: a.n,
                r: a.r,
                p: a.p,
                memory_kib: a.memory_kib,
                time_cost: a.time_cost,
                parallelism: a.parallelism,
                argon2_variant: a.argon2_variant,
                info: a.info,
                info_encoding: a.info_encoding,
            };
            derive(
                &a.algorithm,
                &a.secret,
                &a.input_encoding,
                &a.salt,
                &a.salt_encoding,
                a.length,
                &a.encoding,
                params,
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
                    "algorithm": { "type": "string", "enum": ["pbkdf2", "scrypt", "argon2", "hkdf"], "default": "pbkdf2", "description": "Key-derivation function: pbkdf2 (default), scrypt, argon2 (raw Argon2id/i/d key material), or hkdf (RFC 5869, for high-entropy seeds/keys — not passwords)." },
                    "secret": { "type": "string", "description": "The passphrase or input key material to derive from." },
                    "input_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How `secret` is encoded (default utf8)." },
                    "salt": { "type": "string", "description": "Salt (recommended; required for argon2, min 8 bytes). HKDF treats an empty salt as all-zeros per RFC 5869." },
                    "salt_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How `salt` is encoded (default utf8)." },
                    "length": { "type": "integer", "minimum": 1, "maximum": 1024, "default": 32, "description": "Output key length in bytes (default 32)." },
                    "encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "Output encoding of the derived key (default hex)." },
                    "hash": { "type": "string", "enum": ["sha1", "sha256", "sha384", "sha512"], "default": "sha256", "description": "Underlying hash for pbkdf2 (sha1/256/384/512) and hkdf (default sha256)." },
                    "iterations": { "type": "integer", "minimum": 1, "default": 100000, "description": "PBKDF2 iteration count (pbkdf2 only; default 100000)." },
                    "n": { "type": "integer", "minimum": 2, "default": 16384, "description": "scrypt CPU/memory cost N, a power of two > 1 (scrypt only; default 16384)." },
                    "r": { "type": "integer", "minimum": 1, "default": 8, "description": "scrypt block size r (scrypt only; default 8)." },
                    "p": { "type": "integer", "minimum": 1, "default": 1, "description": "scrypt parallelization p (scrypt only; default 1)." },
                    "memory_kib": { "type": "integer", "minimum": 8, "default": 19456, "description": "Argon2 memory cost in KiB (argon2 only; default 19456 = 19 MiB)." },
                    "time_cost": { "type": "integer", "minimum": 1, "default": 2, "description": "Argon2 time (iteration) cost (argon2 only; default 2)." },
                    "parallelism": { "type": "integer", "minimum": 1, "default": 1, "description": "Argon2 parallelism / lanes (argon2 only; default 1)." },
                    "argon2_variant": { "type": "string", "enum": ["argon2id", "argon2i", "argon2d"], "default": "argon2id", "description": "Argon2 variant (argon2 only; default argon2id)." },
                    "info": { "type": "string", "description": "HKDF context/application info string (hkdf only; optional)." },
                    "info_encoding": { "type": "string", "enum": ["utf8", "hex", "base64"], "default": "utf8", "description": "How `info` is encoded (hkdf only; default utf8)." }
                },
                "required": ["secret"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
