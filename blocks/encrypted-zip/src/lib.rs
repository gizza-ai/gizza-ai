//! gizza-ai/encrypted-zip — password-protected AES-encrypted ZIP pack/extract.
//!
//! mode=pack: loads each source (URL/ref, any bytes) via block-utils, encrypts
//! every entry with WinZip AES (AE-2, AES-256 default or AES-128), and returns
//! the ZIP as a base64 envelope (like create-zip, plus encryption).
//! mode=extract: decrypts + extracts a password-protected ZIP — AES-256/192/128
//! and legacy ZipCrypto auto-detected — returning each file inline as flat JSON
//! (like unzip, plus decryption).
//! Surfaces: chat + CLI (no page — array input + binary output, the same
//! no-page family as create-zip/unzip/7z-extract).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Per input file on pack (create-zip parity).
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// The encrypted archive on extract.
const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;
/// The produced ZIP on pack.
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default = "default_mode")]
    mode: String,
    files: Vec<SourceFields>,
    password: String,
    #[serde(default = "default_encryption")]
    encryption: String,
    #[serde(default = "default_level")]
    level: i64,
}

fn default_mode() -> String {
    "pack".to_string()
}
fn default_encryption() -> String {
    "aes256".to_string()
}
fn default_level() -> i64 {
    6
}

#[derive(Serialize)]
struct Resp {
    entries: Vec<gizza_ai_encrypted_zip_core::Entry>,
    count: usize,
    encrypted_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("mode", ["pack", "extract"])
                .default("pack")
                .describe("Whether to create an encrypted ZIP from the given files (pack, the default) or decrypt and extract an existing password-protected ZIP (extract)."),
        )
        .param(
            Param::source_list("files", 1)
                .required()
                .describe("Pack: the files to bundle and encrypt (each item has exactly one of `url` or `ref`; entry names come from each file's name, duplicates are made unique). Extract: exactly one item — the password-protected .zip archive."),
        )
        .param(
            Param::string("password")
                .required()
                .describe("The archive password. Pack: every entry is encrypted with it (pick a strong one — it is the only thing protecting the archive). Extract: the password the archive was created with."),
        )
        .param(
            Param::enumv("encryption", ["aes256", "aes128"])
                .default("aes256")
                .describe("Pack only: AES key strength for the WinZip AE-2 encryption — aes256 (default) or aes128. Ignored on extract, where the method is auto-detected (AES-256/192/128 and legacy ZipCrypto archives are all supported)."),
        )
        .param(
            Param::integer("level")
                .min(1.0)
                .max(9.0)
                .default(6)
                .describe("Pack only: deflate compression level 1-9 (default 6; higher = smaller but slower). Ignored on extract."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EncryptedZip;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/encrypted-zip",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Create or extract password-protected AES-encrypted ZIP archives",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Create a password-protected, AES-encrypted ZIP archive from one or more files, or decrypt and extract an existing password-protected ZIP — entirely in memory. mode='pack' (default) bundles the given files into one encrypted ZIP (WinZip AE-2; encryption='aes256' default or 'aes128'; deflate compression level 1-9, default 6) that opens in 7-Zip, WinZip, WinRAR, or The Unarchiver — note Windows Explorer cannot open AES zips, and entry NAMES stay visible without the password. mode='extract' decrypts a password-protected .zip supplied as the single files item: AES-256/192/128 and legacy ZipCrypto are auto-detected, a wrong password is rejected with a clear error, and each file is returned with its path, size, and content inline (text when UTF-8, otherwise base64; files past an 8 MiB total budget are listed without content). Limits: 8 MiB per input file, 32 MiB output ZIP, 32 MiB input archive. Each files item has exactly one of url (HTTP/HTTPS) or ref (id from a prior tool call). Runs locally — files and passwords never leave the device.",
        parameters = schema_json()
    ),
)]
impl EncryptedZip {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;
    use gizza_ai_encrypted_zip_core as core;

    let args: Args = serde_json::from_slice(&body).invalid_args("encrypted-zip")?;
    match args.mode.as_str() {
        "pack" => {
            let enc = core::Encryption::parse(&args.encryption).map_err(SkillError::InvalidArgs)?;
            if args.files.is_empty() {
                return Err(SkillError::InvalidArgs("pack needs at least 1 file".into()));
            }
            let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(args.files.len());
            for field in args.files {
                let (bytes, _mime, name) =
                    resolve_source(field.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
                files.push((name, bytes));
            }
            let zip = core::pack(&files, &args.password, enc, args.level)
                .map_err(SkillError::InvalidArgs)?;
            if zip.len() > MAX_OUTPUT_BYTES {
                return Err(SkillError::InvalidArgs(format!(
                    "output ZIP is {} bytes, over the {MAX_OUTPUT_BYTES} cap",
                    zip.len()
                )));
            }
            let label = match enc {
                core::Encryption::Aes256 => "AES-256",
                core::Encryption::Aes128 => "AES-128",
            };
            let out_len = zip.len();
            let encoded = B64.encode(&zip);
            let data_url = format!("data:application/zip;base64,{encoded}");
            let env = Envelope {
                for_llm: format!(
                    "packed {} file(s) into a {out_len}-byte {label}-encrypted ZIP (encrypted.zip). It opens in 7-Zip, WinZip, WinRAR, or The Unarchiver with the password — Windows Explorer cannot open AES zips.",
                    files.len()
                ),
                for_ui: ForUi {
                    data_url,
                    mime: "application/zip".to_string(),
                    filename: "encrypted.zip".to_string(),
                },
            };
            serde_json::to_vec(&env)
                .map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
        }
        "extract" => {
            if args.files.len() != 1 {
                return Err(SkillError::InvalidArgs(format!(
                    "extract takes exactly 1 item in files (the password-protected .zip archive), got {}",
                    args.files.len()
                )));
            }
            let field = args.files.into_iter().next().unwrap();
            let (bytes, _mime, filename) =
                resolve_source(field.into_inner(), AssetKind::Any, MAX_ARCHIVE_BYTES)?;
            let ex = core::extract(&bytes, &args.password, core::DEFAULT_CONTENT_BUDGET)
                .map_err(SkillError::InvalidArgs)?;
            let resp = Resp {
                entries: ex.entries,
                count: ex.count,
                encrypted_count: ex.encrypted_count,
                filename: (!filename.is_empty()).then_some(filename),
            };
            serde_json::to_vec(&resp)
                .map_err(|e| SkillError::Serialize(format!("serialize extract response: {e}")))
        }
        other => Err(SkillError::InvalidArgs(format!(
            "invalid mode '{other}' (use pack or extract)"
        ))),
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
                    "mode": {
                        "type": "string",
                        "enum": ["pack", "extract"],
                        "default": "pack",
                        "description": "Whether to create an encrypted ZIP from the given files (pack, the default) or decrypt and extract an existing password-protected ZIP (extract)."
                    },
                    "files": {
                        "type": "array",
                        "minItems": 1,
                        "description": "Pack: the files to bundle and encrypt (each item has exactly one of `url` or `ref`; entry names come from each file's name, duplicates are made unique). Extract: exactly one item — the password-protected .zip archive.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "password": {
                        "type": "string",
                        "description": "The archive password. Pack: every entry is encrypted with it (pick a strong one — it is the only thing protecting the archive). Extract: the password the archive was created with."
                    },
                    "encryption": {
                        "type": "string",
                        "enum": ["aes256", "aes128"],
                        "default": "aes256",
                        "description": "Pack only: AES key strength for the WinZip AE-2 encryption — aes256 (default) or aes128. Ignored on extract, where the method is auto-detected (AES-256/192/128 and legacy ZipCrypto archives are all supported)."
                    },
                    "level": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 9,
                        "default": 6,
                        "description": "Pack only: deflate compression level 1-9 (default 6; higher = smaller but slower). Ignored on extract."
                    }
                },
                "required": ["files", "password"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
