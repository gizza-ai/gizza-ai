//! gizza-ai/scryptenc-file — chat skill block on the shared tool abstraction.
//!
//! Reads and writes the **scrypt encrypted data format**: the container the
//! Tarsnap `scrypt enc`/`scrypt dec` CLI (and compatible tools such as
//! `rscrypt`) produce — a 96-byte header (`scrypt` magic, version, logN/r/p,
//! 32-byte salt, SHA-256 checksum, header HMAC), AES-256-CTR ciphertext, and a
//! trailing HMAC-SHA256 over the whole file. `operation=info` reports a file's
//! cost parameters without a password. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

use gizza_ai_scryptenc_file_core as core;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_operation")]
    operation: String,
    data: String,
    #[serde(default)]
    password: String,
    #[serde(default = "default_data_encoding")]
    data_encoding: String,
    #[serde(default = "default_output_encoding")]
    output_encoding: String,
    #[serde(default = "default_log_n")]
    log_n: i64,
    #[serde(default = "default_r")]
    r: i64,
    #[serde(default = "default_p")]
    p: i64,
    #[serde(default)]
    salt: String,
    #[serde(default = "default_max_memory")]
    max_memory_mib: i64,
}

fn default_operation() -> String {
    "encrypt".to_string()
}

fn default_data_encoding() -> String {
    "text".to_string()
}

fn default_output_encoding() -> String {
    "base64".to_string()
}

fn default_log_n() -> i64 {
    core::DEFAULT_LOG_N
}

fn default_r() -> i64 {
    core::DEFAULT_R
}

fn default_p() -> i64 {
    core::DEFAULT_P
}

fn default_max_memory() -> i64 {
    core::DEFAULT_MAX_MEMORY_MIB
}

impl Args {
    fn run(&self) -> Result<String, String> {
        core::run(
            &self.operation,
            &self.data,
            &self.password,
            &self.data_encoding,
            &self.output_encoding,
            self.log_n,
            self.r,
            self.p,
            &self.salt,
            self.max_memory_mib,
        )
    }
}

/// Single source for the chat schema (and CLI). `data` and `password` are
/// required so the generated copy-paste CLI example demonstrates the default
/// encrypt operation correctly. `operation=info` still ignores the password;
/// callers can pass an empty string there.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "What to process: the plaintext to seal when operation=encrypt, or the scrypt \
             container to open or inspect when operation=decrypt or info. Read according to \
             data_encoding. Capped at 4 MiB of decoded bytes.",
        ))
        .param(Param::string("password").required().describe(
            "The passphrase the AES-256 and HMAC-SHA256 keys are derived from via scrypt. \
             Required for encrypt and decrypt; ignored by info. Any non-empty string, \
             including non-ASCII; it is UTF-8 encoded before derivation. The same passphrase \
             plus the file's own logN/r/p and salt are all that is needed to decrypt, and \
             there is no recovery path if it is lost.",
        ))
        .param(
            Param::enumv("operation", ["encrypt", "decrypt", "info"])
                .default("encrypt")
                .describe(
                    "encrypt (default) seals data into a new scrypt-format container; decrypt \
                     verifies both HMACs of an existing container and returns the plaintext; \
                     info reads the header's cost parameters and sizes WITHOUT a password, \
                     which also works on files whose parameters are too large to open here.",
                ),
        )
        .param(
            Param::enumv("data_encoding", ["text", "hex", "base64"])
                .default("text")
                .describe(
                    "How to read data. text (default) treats it as UTF-8 characters when \
                     encrypting and auto-detects hex vs base64 when decrypting or inspecting; \
                     hex and base64 decode it to raw bytes first, which is how you seal binary \
                     input. Whitespace and a leading 0x are ignored in hex.",
                ),
        )
        .param(
            Param::enumv("output_encoding", ["base64", "hex"])
                .default("base64")
                .describe(
                    "How binary results are printed. base64 (default) is the compact form to \
                     paste between systems; hex is byte-addressable for comparing against the \
                     format spec. On decrypt this applies only when the plaintext is not valid \
                     UTF-8 — readable text comes back as text. Ignored by info.",
                ),
        )
        .param(
            Param::integer("log_n")
                .default(core::DEFAULT_LOG_N)
                .min(1.0)
                .max(63.0)
                .describe(
                    "log2 of the scrypt CPU/memory cost N, so N = 2^log_n. Default 14 (N=16384, \
                     about 16 MiB with r=8), chosen to fit the sandbox. Each +1 doubles both \
                     work and memory. Used only when operation=encrypt — decrypt and info read \
                     it from the file's own header.",
                ),
        )
        .param(
            Param::integer("r")
                .default(core::DEFAULT_R)
                .min(1.0)
                .max(32.0)
                .describe(
                    "scrypt block size r, default 8 (the value the reference scrypt CLI uses). \
                     Memory scales as 128 * N * r, so raising r raises memory as much as raising \
                     log_n does. Used only when operation=encrypt.",
                ),
        )
        .param(
            Param::integer("p")
                .default(core::DEFAULT_P)
                .min(1.0)
                .max(16.0)
                .describe(
                    "scrypt parallelization p, default 1. Raises CPU work without raising the \
                     large memory buffer, so it hardens against attackers with lots of memory \
                     but little parallel compute. Used only when operation=encrypt.",
                ),
        )
        .param(Param::string("salt").describe(
            "Optional 32-byte salt as 64 hex characters (e.g. \
             000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f). Leave empty for \
             a fresh random salt, which is what you want for real data; set it only to \
             reproduce a container byte for byte in a test. Ignored when operation=decrypt or \
             info (the salt is read from the header).",
        ))
        .param(
            Param::integer("max_memory_mib")
                .default(core::DEFAULT_MAX_MEMORY_MIB)
                .min(1.0)
                .max(core::MAX_MAX_MEMORY_MIB as f64)
                .describe(
                    "Ceiling in MiB on the scrypt working buffer, the analogue of the reference \
                     CLI's -M maxmem. Default 32; the hard maximum is 64 because this runs in a \
                     64 MiB wasm sandbox. Parameters needing more are refused with the required \
                     amount named, rather than trapping. Applies to both encrypt and decrypt.",
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
    name = "gizza-ai/scryptenc-file",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt, decrypt and inspect files in the scrypt encrypted data format.",
    skill(
        description = "Read and write the scrypt encrypted data format used by the scrypt CLI \
                       (scrypt enc / scrypt dec) and compatible tools like rscrypt: a 96-byte \
                       header carrying the magic, version, logN/r/p cost parameters, 32-byte \
                       salt, SHA-256 checksum and header HMAC, followed by AES-256-CTR \
                       ciphertext and a trailing HMAC-SHA256 over the whole file. Set \
                       operation=encrypt to seal text or bytes with a passphrase, \
                       operation=decrypt to verify both HMACs and recover the plaintext, or \
                       operation=info to report a container's cost parameters, salt and sizes \
                       without any passphrase.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "scryptenc-file", |a: Args| {
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

    /// Drift guard: the authored schema below is what chat and the CLI consume.
    /// If a param is added, renamed, or loses its enum, this test fails and the
    /// manifest must be re-synced with scripts/sync-tool-manifest.py.
    #[test]
    fn schema_matches_the_authored_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let expected: serde_json::Value = serde_json::json!({
            "type": "object",
            "required": ["data", "password"],
            "properties": {
                "data": {
                    "type": "string",
                    "description": actual["properties"]["data"]["description"],
                },
                "password": {
                    "type": "string",
                    "description": actual["properties"]["password"]["description"],
                },
                "operation": {
                    "type": "string",
                    "enum": ["encrypt", "decrypt", "info"],
                    "default": "encrypt",
                    "description": actual["properties"]["operation"]["description"],
                },
                "data_encoding": {
                    "type": "string",
                    "enum": ["text", "hex", "base64"],
                    "default": "text",
                    "description": actual["properties"]["data_encoding"]["description"],
                },
                "output_encoding": {
                    "type": "string",
                    "enum": ["base64", "hex"],
                    "default": "base64",
                    "description": actual["properties"]["output_encoding"]["description"],
                },
                "log_n": {
                    "type": "integer",
                    "default": 14,
                    "minimum": 1,
                    "maximum": 63,
                    "description": actual["properties"]["log_n"]["description"],
                },
                "r": {
                    "type": "integer",
                    "default": 8,
                    "minimum": 1,
                    "maximum": 32,
                    "description": actual["properties"]["r"]["description"],
                },
                "p": {
                    "type": "integer",
                    "default": 1,
                    "minimum": 1,
                    "maximum": 16,
                    "description": actual["properties"]["p"]["description"],
                },
                "salt": {
                    "type": "string",
                    "description": actual["properties"]["salt"]["description"],
                },
                "max_memory_mib": {
                    "type": "integer",
                    "default": 32,
                    "minimum": 1,
                    "maximum": 64,
                    "description": actual["properties"]["max_memory_mib"]["description"],
                },
            },
            "additionalProperties": false,
        });
        assert_eq!(actual, expected);
    }

    /// Every param must carry a `.describe()` an LLM can act on.
    #[test]
    fn every_param_is_described() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for (name, prop) in schema["properties"].as_object().unwrap() {
            let desc = prop["description"].as_str().unwrap_or("");
            assert!(desc.len() > 30, "param {name} needs a real description");
        }
    }
}
