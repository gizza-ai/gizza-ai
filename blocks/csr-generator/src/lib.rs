//! gizza-ai/csr-generator — generate a fresh private key and PKCS#10 CSR.
//!
//! Thin wrapper around the pure core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure Rust (RustCrypto/getrandom) →
//! runs on native and wasm32-wasip1 backends. Surfaces: chat + CLI. No
//! standalone page: CSR/key generation is non-deterministic, matching the other
//! key-generation tools in this repo.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csr_generator_core::{generate, Algorithm, CsrRequest};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_algorithm")]
    algorithm: String,
    common_name: String,
    #[serde(default)]
    organization: String,
    #[serde(default)]
    organizational_unit: String,
    #[serde(default)]
    country: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    locality: String,
    #[serde(default)]
    san_dns: String,
    #[serde(default)]
    san_ips: String,
    #[serde(default)]
    san_emails: String,
    #[serde(default)]
    san_uris: String,
}

fn default_algorithm() -> String {
    "p256".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("algorithm", ["p256", "p384"])
                .default("p256")
                .describe("ECDSA signing key for the CSR: p256 (secp256r1, broad TLS support, default) or p384 (larger curve, SHA-384 signature). RSA CSR generation is intentionally not exposed in this wasm-safe RustCrypto implementation."),
        )
        .param(
            Param::string("common_name")
                .required()
                .describe("Subject common name (CN), usually the primary DNS name such as example.com. If no SANs are supplied and this looks like a DNS name, it is also added as a DNS SAN."),
        )
        .param(
            Param::string("organization")
                .describe("Optional subject organization (O), for example Example Ltd."),
        )
        .param(
            Param::string("organizational_unit")
                .describe("Optional subject organizational unit (OU), for example Platform or IT."),
        )
        .param(
            Param::string("country")
                .describe("Optional two-letter subject country code (C), for example US or GB."),
        )
        .param(
            Param::string("state")
                .describe("Optional subject state or province (ST), for example California."),
        )
        .param(
            Param::string("locality")
                .describe("Optional subject locality/city (L), for example San Francisco."),
        )
        .param(
            Param::string("san_dns")
                .multiline()
                .describe("Optional DNS Subject Alternative Names, separated by commas, semicolons, or new lines. Prefixes like DNS:api.example.com are accepted."),
        )
        .param(
            Param::string("san_ips")
                .multiline()
                .describe("Optional IP address Subject Alternative Names, separated by commas, semicolons, or new lines. IPv4 and IPv6 are accepted; IP: prefixes are optional."),
        )
        .param(
            Param::string("san_emails")
                .multiline()
                .describe("Optional email/rfc822Name Subject Alternative Names, separated by commas, semicolons, or new lines. email: prefixes are optional."),
        )
        .param(
            Param::string("san_uris")
                .multiline()
                .describe("Optional URI Subject Alternative Names, separated by commas, semicolons, or new lines. URI: prefixes are optional."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csr-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate an ECDSA private key and PKCS#10 CSR",
    skill(
        description = "Generate a fresh ECDSA private key and PKCS#10 Certificate Signing Request (CSR) locally. Choose p256 (default) or p384, fill subject fields (CN plus optional O/OU/C/ST/L), and add DNS, IP, email, or URI Subject Alternative Names. The output includes PKCS#8 private key PEM, public key PEM, CSR PEM, and a subject/SAN summary. RSA is listed as out-of-model for this wasm-safe RustCrypto implementation; use an external OpenSSL flow if a CA specifically requires RSA.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csr-generator", |a: Args| {
            let algorithm = Algorithm::parse(&a.algorithm).map_err(SkillError::InvalidArgs)?;
            generate(CsrRequest {
                algorithm,
                common_name: a.common_name,
                organization: a.organization,
                organizational_unit: a.organizational_unit,
                country: a.country,
                state: a.state,
                locality: a.locality,
                san_dns: a.san_dns,
                san_ips: a.san_ips,
                san_emails: a.san_emails,
                san_uris: a.san_uris,
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
                    "algorithm": { "type": "string", "enum": ["p256", "p384"], "default": "p256", "description": "ECDSA signing key for the CSR: p256 (secp256r1, broad TLS support, default) or p384 (larger curve, SHA-384 signature). RSA CSR generation is intentionally not exposed in this wasm-safe RustCrypto implementation." },
                    "common_name": { "type": "string", "description": "Subject common name (CN), usually the primary DNS name such as example.com. If no SANs are supplied and this looks like a DNS name, it is also added as a DNS SAN." },
                    "organization": { "type": "string", "description": "Optional subject organization (O), for example Example Ltd." },
                    "organizational_unit": { "type": "string", "description": "Optional subject organizational unit (OU), for example Platform or IT." },
                    "country": { "type": "string", "description": "Optional two-letter subject country code (C), for example US or GB." },
                    "state": { "type": "string", "description": "Optional subject state or province (ST), for example California." },
                    "locality": { "type": "string", "description": "Optional subject locality/city (L), for example San Francisco." },
                    "san_dns": { "type": "string", "description": "Optional DNS Subject Alternative Names, separated by commas, semicolons, or new lines. Prefixes like DNS:api.example.com are accepted." },
                    "san_ips": { "type": "string", "description": "Optional IP address Subject Alternative Names, separated by commas, semicolons, or new lines. IPv4 and IPv6 are accepted; IP: prefixes are optional." },
                    "san_emails": { "type": "string", "description": "Optional email/rfc822Name Subject Alternative Names, separated by commas, semicolons, or new lines. email: prefixes are optional." },
                    "san_uris": { "type": "string", "description": "Optional URI Subject Alternative Names, separated by commas, semicolons, or new lines. URI: prefixes are optional." }
                },
                "required": ["common_name"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
