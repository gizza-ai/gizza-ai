//! gizza-ai/dkim-generate — chat skill block on the shared tool abstraction.
//!
//! Generate a DKIM signing key pair and the `selector._domainkey.<domain>` TXT
//! record that publishes its public half, or rebuild that record from a key that
//! is already installed on a mail server. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    domain: String,
    #[serde(default = "default_selector")]
    selector: String,
    #[serde(default = "default_key_type")]
    key_type: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_include_hash")]
    include_hash: bool,
    #[serde(default)]
    flags: String,
    #[serde(default)]
    existing_key: String,
}

fn default_selector() -> String {
    "mail".to_string()
}
fn default_key_type() -> String {
    "rsa-2048".to_string()
}
fn default_output() -> String {
    "text".to_string()
}
fn default_include_hash() -> bool {
    true
}

impl Args {
    fn run(&self) -> Result<String, String> {
        gizza_ai_dkim_generate_core::run(
            &self.domain,
            &self.selector,
            &self.key_type,
            &self.output,
            self.include_hash,
            &self.flags,
            &self.existing_key,
        )
    }
}

/// Single source for the chat schema (and the CLI). Param order is also the
/// page field order in `page/meta.toml` and the `web/src/lib.rs` argument order.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("domain").required().describe(
            "The domain your mail is sent from, e.g. example.com. This is the domain the record \
             is published under and the one that appears as d= in the signature. A pasted URL, \
             email address or full record host (mail._domainkey.example.com) is reduced to the \
             domain itself. Internationalized domains must already be in punycode (xn--) form.",
        ))
        .param(Param::string("selector").default("mail").describe(
            "The DKIM selector — the label published to the left of ._domainkey, e.g. mail, s1 or \
             2026a. It becomes the DNS host <selector>._domainkey.<domain> and appears as s= in \
             the signature. ASCII letters, digits and hyphens, 63 characters or fewer. Use a new \
             selector for each key so you can rotate without downtime. Defaults to mail.",
        ))
        .param(
            Param::enumv("key_type", ["rsa-1024", "rsa-2048", "rsa-4096", "ed25519"])
                .default("rsa-2048")
                .describe(
                    "Key pair to generate. rsa-2048 is the interoperable default that every \
                     receiver verifies. rsa-1024 is legacy-only and now treated as weak. rsa-4096 \
                     produces a TXT value over 255 characters, which some DNS panels and older \
                     resolvers reject. ed25519 (RFC 8463) is short but not verified everywhere \
                     yet, so publish an RSA selector alongside it. Ignored when existing_key is \
                     given — the pasted key's own type is used.",
                ),
        )
        .param(
            Param::enumv(
                "output",
                [
                    "text",
                    "dns_value",
                    "zone_file",
                    "json",
                    "public_key",
                    "private_key",
                ],
            )
            .default("text")
            .describe(
                "Which part of the result to return. text is the full report: host, TXT value, \
                 zone line, private key, public key and warnings. dns_value is just the TXT value \
                 starting v=DKIM1, ready to paste into a DNS panel. zone_file is the BIND line, \
                 split into quoted 255-character strings when needed. json is the machine-readable \
                 object. public_key and private_key return only that PEM.",
            ),
        )
        .param(Param::boolean("include_hash").default(true).describe(
            "Include the optional h=sha256 tag, which tells receivers to accept only SHA-256 \
             signatures for this selector. On by default and recommended; turn it off for the \
             shortest possible record, or if a legacy signer still emits rsa-sha1 signatures.",
        ))
        .param(
            Param::enumv("flags", ["none", "y", "s", "y:s"])
                .default("none")
                .describe(
                    "The DKIM t= flag tag. none omits the tag, which is what production selectors \
                     use. y marks the selector as being in test mode so receivers must not treat a \
                     signature failure as a policy failure. s forbids using this key for \
                     subdomains of the domain. y:s sets both.",
                ),
        )
        .param(Param::string("existing_key").multiline().describe(
            "Leave empty to generate a brand-new key pair. Paste a private key to rebuild the DNS \
             record for a key already installed on your mail server: PKCS#8 (-----BEGIN PRIVATE \
             KEY-----), PKCS#1 (-----BEGIN RSA PRIVATE KEY-----) or a base64 Ed25519 seed. A \
             public key PEM or a bare p= value also works when you only need the record. \
             Passphrase-encrypted and OpenSSH-format keys are rejected with an explanation.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dkim-generate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a DKIM key pair and its DNS TXT record",
    skill(
        description = "Generate a DKIM signing key pair locally and the DNS TXT record that publishes its public half at <selector>._domainkey.<domain>. Supports RSA 1024/2048/4096 (RFC 6376 k=rsa, private key returned as both PKCS#8 and PKCS#1 PEM) and Ed25519 (RFC 8463 k=ed25519, private key returned as a PKCS#8 PEM and the base64 32-byte seed OpenDKIM and rspamd store). The default text output shows the record host, TXT value, BIND zone line split into 255-character strings when required, both key halves and warnings about weak or oversized keys; dns_value, zone_file, public_key, private_key and json return one piece each. Paste an existing private key, public key or p= value into existing_key to rebuild the record for a key already installed on a mail server instead of generating a new one. Pure local Rust/WASM: no upload, no network, no DNS lookup, no publishing to a DNS provider, and no verification that an existing record resolves.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dkim-generate", |a: Args| {
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
    use gizza_ai_dkim_generate_core::{
        TEST_ED25519_P_TAG, TEST_ED25519_SEED, TEST_RSA_PKCS8_PEM, TEST_RSA_P_TAG,
    };

    fn args(output: &str, existing_key: &str) -> Args {
        Args {
            domain: "example.com".into(),
            selector: default_selector(),
            key_type: default_key_type(),
            output: output.into(),
            include_hash: default_include_hash(),
            flags: String::new(),
            existing_key: existing_key.into(),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"object",
                "properties":{
                    "domain":{"type":"string","description":"The domain your mail is sent from, e.g. example.com. This is the domain the record is published under and the one that appears as d= in the signature. A pasted URL, email address or full record host (mail._domainkey.example.com) is reduced to the domain itself. Internationalized domains must already be in punycode (xn--) form."},
                    "selector":{"type":"string","default":"mail","description":"The DKIM selector — the label published to the left of ._domainkey, e.g. mail, s1 or 2026a. It becomes the DNS host <selector>._domainkey.<domain> and appears as s= in the signature. ASCII letters, digits and hyphens, 63 characters or fewer. Use a new selector for each key so you can rotate without downtime. Defaults to mail."},
                    "key_type":{"type":"string","enum":["rsa-1024","rsa-2048","rsa-4096","ed25519"],"default":"rsa-2048","description":"Key pair to generate. rsa-2048 is the interoperable default that every receiver verifies. rsa-1024 is legacy-only and now treated as weak. rsa-4096 produces a TXT value over 255 characters, which some DNS panels and older resolvers reject. ed25519 (RFC 8463) is short but not verified everywhere yet, so publish an RSA selector alongside it. Ignored when existing_key is given — the pasted key's own type is used."},
                    "output":{"type":"string","enum":["text","dns_value","zone_file","json","public_key","private_key"],"default":"text","description":"Which part of the result to return. text is the full report: host, TXT value, zone line, private key, public key and warnings. dns_value is just the TXT value starting v=DKIM1, ready to paste into a DNS panel. zone_file is the BIND line, split into quoted 255-character strings when needed. json is the machine-readable object. public_key and private_key return only that PEM."},
                    "include_hash":{"type":"boolean","default":true,"description":"Include the optional h=sha256 tag, which tells receivers to accept only SHA-256 signatures for this selector. On by default and recommended; turn it off for the shortest possible record, or if a legacy signer still emits rsa-sha1 signatures."},
                    "flags":{"type":"string","enum":["none","y","s","y:s"],"default":"none","description":"The DKIM t= flag tag. none omits the tag, which is what production selectors use. y marks the selector as being in test mode so receivers must not treat a signature failure as a policy failure. s forbids using this key for subdomains of the domain. y:s sets both."},
                    "existing_key":{"type":"string","description":"Leave empty to generate a brand-new key pair. Paste a private key to rebuild the DNS record for a key already installed on your mail server: PKCS#8 (-----BEGIN PRIVATE KEY-----), PKCS#1 (-----BEGIN RSA PRIVATE KEY-----) or a base64 Ed25519 seed. A public key PEM or a bare p= value also works when you only need the record. Passphrase-encrypted and OpenSSH-format keys are rejected with an explanation."}
                },
                "additionalProperties":false,
                "required":["domain"]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn descriptor_param_order_matches_the_page_and_web_wrapper() {
        // page/meta.toml `[[input]]` order and web/src/lib.rs `run(...)` argument
        // order are both this list; a reorder here silently mis-binds the page.
        let d = descriptor();
        let names: Vec<&str> = d.params.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "domain",
                "selector",
                "key_type",
                "output",
                "include_hash",
                "flags",
                "existing_key"
            ]
        );
    }

    #[test]
    fn args_defaults_fill_in_from_a_domain_only_call() {
        let a: Args = serde_json::from_str(r#"{"domain":"example.com"}"#).unwrap();
        assert_eq!(a.selector, "mail");
        assert_eq!(a.key_type, "rsa-2048");
        assert_eq!(a.output, "text");
        assert!(a.include_hash);
        assert_eq!(a.flags, "");
        assert_eq!(a.existing_key, "");
    }

    #[test]
    fn args_layer_builds_the_record_for_a_supplied_key() {
        let a = args("dns_value", TEST_RSA_PKCS8_PEM);
        assert_eq!(
            a.run().unwrap(),
            format!("v=DKIM1; h=sha256; k=rsa; p={TEST_RSA_P_TAG}")
        );

        let a = args("zone_file", TEST_ED25519_SEED);
        assert_eq!(
            a.run().unwrap(),
            format!(
                "mail._domainkey.example.com. 3600 IN TXT \
                 \"v=DKIM1; h=sha256; k=ed25519; p={TEST_ED25519_P_TAG}\""
            )
        );
    }

    #[test]
    fn args_layer_reports_bad_input() {
        let mut a = args("text", TEST_RSA_PKCS8_PEM);
        a.domain = "localhost".into();
        assert!(a.run().unwrap_err().contains("full domain name"));

        let mut a = args("yaml", TEST_RSA_PKCS8_PEM);
        a.flags = "none".into();
        assert!(a.run().unwrap_err().contains("unknown format"));
    }
}
