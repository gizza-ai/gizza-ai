//! gizza-ai/ssh-public-key-parser — chat skill block on the shared tool abstraction.
//! Decodes OpenSSH public keys (id_*.pub lines, authorized_keys entries with an options
//! prefix, known_hosts entries, OpenSSH certificates and RFC 4716 blocks) into algorithm,
//! key size, comment and SHA256/MD5/SHA1 fingerprints — entirely offline, pure Rust/wasm.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    expected_fingerprint: String,
    #[serde(default)]
    include_sha1: bool,
    #[serde(default)]
    uppercase_md5: bool,
    #[serde(default)]
    now: i64,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .multiline()
                .describe(
                    "The SSH PUBLIC key(s) to parse, pasted verbatim. Accepts a plain OpenSSH \
line ('ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA… user@host'), an authorized_keys line with an \
options prefix ('command=\"…\",no-pty ssh-rsa AAAA… user@host'), a known_hosts entry (host \
patterns or a |1| hashed host, optionally prefixed with @cert-authority or @revoked), an \
OpenSSH certificate (*-cert-v01@openssh.com), or an RFC 4716 '---- BEGIN SSH2 PUBLIC KEY ----' \
block. Multiple keys may be pasted at once — one per line, blank lines and # comments are \
ignored — up to 200 keys and 256 KiB. Supported algorithms: ssh-ed25519, ssh-rsa, ssh-dss, \
ecdsa-sha2-nistp256/384/521 and the sk-* FIDO variants. PRIVATE keys and PEM public keys are \
rejected with guidance instead of being parsed.",
                ),
        )
        .param(
            Param::string("expected_fingerprint")
                .describe(
                    "Optional fingerprint to verify each key against, e.g. when checking a \
host key against one published out of band. Accepts any printed form: \
'SHA256:<base64>', the bare base64, 'MD5:aa:bb:…', bare colon-hex, or plain hex; prefixes, \
colons, spaces and hex case are ignored. Adds 'fingerprint_match' to every key and \
'expected_fingerprint_matched' to the report. Default: no comparison.",
                ),
        )
        .param(
            Param::boolean("include_sha1")
                .default(false)
                .describe(
                    "Also report the legacy SHA-1 fingerprint (the same value 'ssh-keygen -l -E \
sha1' prints, base64 with a SHA1: prefix). Only needed for old tooling — SHA-256 is the modern \
default. Default: false.",
                ),
        )
        .param(
            Param::boolean("uppercase_md5")
                .default(false)
                .describe(
                    "Print the MD5 fingerprint's hex digits in UPPERCASE (AA:BB:…) to match \
older consoles and inventory systems that display them that way. The digest is identical \
either way. Default: false (lowercase, as ssh-keygen prints it).",
                ),
        )
        .param(
            Param::integer("now")
                .min(0.0)
                .describe(
                    "Reference time as seconds since the Unix epoch, used to report each OpenSSH \
CERTIFICATE's validity status and days-until-expiry. When omitted (0), the current clock time is \
used. Has no effect on plain (non-certificate) keys.",
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
    name = "gizza-ai/ssh-public-key-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse OpenSSH public keys into type, size, comment and fingerprints",
    skill(
        description = "Parse OpenSSH PUBLIC keys entirely offline and report what is inside \
them. Paste one or more keys as 'input': a plain id_*.pub line, an authorized_keys line with an \
options prefix, a known_hosts entry (host patterns or |1| hashed, with @cert-authority/@revoked \
markers), an OpenSSH certificate (*-cert-v01@openssh.com), or an RFC 4716 '---- BEGIN SSH2 \
PUBLIC KEY ----' block. For each key it returns the source format, the algorithm, the key family \
and size in bits (RSA modulus bits, ECDSA curve, Ed25519), the curve name, the comment, the \
SHA256 and MD5 fingerprints exactly as 'ssh-keygen -l' prints them (SHA-1 optional via \
'include_sha1'), the authorized_keys options or known_hosts host patterns, FIDO application for \
sk-* keys, and a strength rating with warnings (short RSA, obsolete DSA, declared-vs-embedded \
algorithm mismatch). Certificates additionally report user/host type, serial, key ID, \
principals, validity window and status, critical options, extensions and the signing CA's \
fingerprint. Set 'expected_fingerprint' to verify keys against a known fingerprint in any \
printed form. Multiple keys are summarised with a key count and a unique-fingerprint count. \
Private keys are never parsed. Returns one JSON report.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ssh-public-key-parser", |a: Args| {
            let now = if a.now == 0 {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            } else {
                a.now
            };
            let opts = gizza_ai_ssh_public_key_parser_core::Options {
                include_sha1: a.include_sha1,
                uppercase_md5: a.uppercase_md5,
                expected_fingerprint: a.expected_fingerprint,
                now,
            };
            let report = gizza_ai_ssh_public_key_parser_core::parse(&a.input, &opts)
                .map_err(SkillError::InvalidArgs)?;
            serde_json::to_value(report).map_err(|e| SkillError::Serialize(e.to_string()))
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
    /// schema exactly, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The SSH PUBLIC key(s) to parse, pasted verbatim. Accepts a plain OpenSSH line ('ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA… user@host'), an authorized_keys line with an options prefix ('command=\"…\",no-pty ssh-rsa AAAA… user@host'), a known_hosts entry (host patterns or a |1| hashed host, optionally prefixed with @cert-authority or @revoked), an OpenSSH certificate (*-cert-v01@openssh.com), or an RFC 4716 '---- BEGIN SSH2 PUBLIC KEY ----' block. Multiple keys may be pasted at once — one per line, blank lines and # comments are ignored — up to 200 keys and 256 KiB. Supported algorithms: ssh-ed25519, ssh-rsa, ssh-dss, ecdsa-sha2-nistp256/384/521 and the sk-* FIDO variants. PRIVATE keys and PEM public keys are rejected with guidance instead of being parsed."
                    },
                    "expected_fingerprint": {
                        "type": "string",
                        "description": "Optional fingerprint to verify each key against, e.g. when checking a host key against one published out of band. Accepts any printed form: 'SHA256:<base64>', the bare base64, 'MD5:aa:bb:…', bare colon-hex, or plain hex; prefixes, colons, spaces and hex case are ignored. Adds 'fingerprint_match' to every key and 'expected_fingerprint_matched' to the report. Default: no comparison."
                    },
                    "include_sha1": {
                        "type": "boolean",
                        "default": false,
                        "description": "Also report the legacy SHA-1 fingerprint (the same value 'ssh-keygen -l -E sha1' prints, base64 with a SHA1: prefix). Only needed for old tooling — SHA-256 is the modern default. Default: false."
                    },
                    "uppercase_md5": {
                        "type": "boolean",
                        "default": false,
                        "description": "Print the MD5 fingerprint's hex digits in UPPERCASE (AA:BB:…) to match older consoles and inventory systems that display them that way. The digest is identical either way. Default: false (lowercase, as ssh-keygen prints it)."
                    },
                    "now": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Reference time as seconds since the Unix epoch, used to report each OpenSSH CERTIFICATE's validity status and days-until-expiry. When omitted (0), the current clock time is used. Has no effect on plain (non-certificate) keys."
                    }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
