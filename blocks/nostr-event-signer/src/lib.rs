//! gizza-ai/nostr-event-signer — build a Nostr event from its parts, hash it
//! into a NIP-01 event id and sign that id with a BIP-340 Schnorr signature, so
//! the result can be pasted straight into a relay client. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    nsec: String,
    #[serde(default)]
    content: String,
    #[serde(default = "default_kind")]
    kind: f64,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    created_at: f64,
    #[serde(default)]
    template: String,
    #[serde(default)]
    pow: f64,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn default_kind() -> f64 {
    1.0
}
fn default_output() -> String {
    "event".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("nsec")
                .required()
                .describe("Your Nostr private (secret) key, either as a NIP-19 'nsec1…' bech32 string or as 64 hex characters (a leading 0x and any whitespace are ignored). Everything is computed locally — the key is never sent anywhere. Use a disposable test key unless you fully trust the machine."),
        )
        .param(
            Param::string("content")
                .describe("The event body. For kind 1 this is the note text; for kind 0 it is a JSON profile object; for kind 4 it would be ciphertext. Any UTF-8 is allowed and is escaped per NIP-01 when the id is hashed. Defaults to an empty string."),
        )
        .param(
            Param::integer("kind")
                .default(1)
                .min(0.0)
                .max(65535.0)
                .describe("Nostr event kind, a whole number from 0 to 65535. Common values: 0 profile metadata, 1 text note (default), 3 follow list, 5 deletion request, 6 repost, 7 reaction, 30023 long-form article."),
        )
        .param(
            Param::string("tags")
                .describe("Event tags, in either form. Shorthand: one tag per line (or comma-separated) as 'name=value1;value2', e.g. 'e=<event id>;wss://relay.example.com;root' and 'p=<pubkey hex>'. JSON: a full array of arrays of strings, e.g. [[\"e\",\"<event id>\"],[\"p\",\"<pubkey hex>\"]] — use this form when a value contains a comma or newline. Defaults to no tags. Limit 2000 tags."),
        )
        .param(
            Param::integer("created_at")
                .default(0)
                .min(0.0)
                .describe("Event timestamp as whole seconds since 1970-01-01 UTC, e.g. 1700000000. Use 0 (the default) to stamp the event with the current time; pass an explicit value when you need a reproducible id and signature."),
        )
        .param(
            Param::string("template")
                .describe("Optional unsigned-event JSON object, e.g. {\"kind\":1,\"content\":\"hello\",\"tags\":[],\"created_at\":1700000000}. Any of kind/content/tags/created_at present here OVERRIDES the matching parameter above; id, pubkey and sig are always recomputed from the signing key and ignored if present. Leave empty to build the event from the individual fields."),
        )
        .param(
            Param::integer("pow")
                .default(0)
                .min(0.0)
                .max(20.0)
                .describe("NIP-13 proof-of-work difficulty in leading zero bits of the event id, 0 to 20. 0 (the default) skips mining. Anything above ~16 can take a noticeable while; a mined event carries a [\"nonce\",\"<counter>\",\"<target>\"] tag and any nonce tag you supplied is replaced."),
        )
        .param(
            Param::enumv("output", ["event", "relay-message", "report"])
                .default("event")
                .describe("Output shape. 'event' (default) is the signed event JSON object. 'relay-message' wraps it as [\"EVENT\", {…}], the exact frame a relay websocket expects. 'report' prints a labeled summary — id, note1…, pubkey, npub1…, timestamp, kind, proof-of-work bits, signature and a verification line — followed by the event JSON."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Indent the JSON output. True (the default) is easier to read; false emits the single-line compact form that relays and other tools expect when pasting."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Seconds since the unix epoch. Wafer provides a clock import, so this works
/// in the chat block and natively in the CLI; the page supplies its own value
/// via `Date.now()` (wasm32-unknown-unknown has no std clock).
fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn run_logic(a: Args) -> Result<String, String> {
    gizza_ai_nostr_event_signer_core::sign_event(
        &a.nsec,
        &a.content,
        a.kind,
        &a.tags,
        a.created_at,
        &a.template,
        a.pow,
        &a.output,
        a.pretty,
        now_unix(),
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/nostr-event-signer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build and Schnorr-sign a Nostr event from your nsec",
    skill(
        description = "Build a Nostr event and sign it with a BIP-340 Schnorr signature over secp256k1, producing publish-ready JSON. Give it a private key (nsec1… or 64 hex characters) plus the event's content, kind, tags and timestamp, or paste a partial unsigned event as 'template'. It derives the x-only public key, serializes the event per NIP-01 as [0,pubkey,created_at,kind,tags,content], hashes that with SHA-256 to get the event id, signs the id, and verifies its own signature before returning. Output is the signed event JSON, a [\"EVENT\", {…}] relay frame ready to send over a relay websocket, or a labeled report with the NIP-19 note1…/npub1… forms. Optional NIP-13 proof-of-work mines a nonce tag up to 20 leading zero bits. Signing is deterministic (no auxiliary randomness), so the same inputs always give the same id and signature. Runs locally; the key is never uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "nostr-event-signer", |a: Args| {
            run_logic(a).map_err(SkillError::InvalidArgs)
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
                    "nsec": { "type": "string", "description": "Your Nostr private (secret) key, either as a NIP-19 'nsec1…' bech32 string or as 64 hex characters (a leading 0x and any whitespace are ignored). Everything is computed locally — the key is never sent anywhere. Use a disposable test key unless you fully trust the machine." },
                    "content": { "type": "string", "description": "The event body. For kind 1 this is the note text; for kind 0 it is a JSON profile object; for kind 4 it would be ciphertext. Any UTF-8 is allowed and is escaped per NIP-01 when the id is hashed. Defaults to an empty string." },
                    "kind": { "type": "integer", "minimum": 0, "maximum": 65535, "default": 1, "description": "Nostr event kind, a whole number from 0 to 65535. Common values: 0 profile metadata, 1 text note (default), 3 follow list, 5 deletion request, 6 repost, 7 reaction, 30023 long-form article." },
                    "tags": { "type": "string", "description": "Event tags, in either form. Shorthand: one tag per line (or comma-separated) as 'name=value1;value2', e.g. 'e=<event id>;wss://relay.example.com;root' and 'p=<pubkey hex>'. JSON: a full array of arrays of strings, e.g. [[\"e\",\"<event id>\"],[\"p\",\"<pubkey hex>\"]] — use this form when a value contains a comma or newline. Defaults to no tags. Limit 2000 tags." },
                    "created_at": { "type": "integer", "minimum": 0, "default": 0, "description": "Event timestamp as whole seconds since 1970-01-01 UTC, e.g. 1700000000. Use 0 (the default) to stamp the event with the current time; pass an explicit value when you need a reproducible id and signature." },
                    "template": { "type": "string", "description": "Optional unsigned-event JSON object, e.g. {\"kind\":1,\"content\":\"hello\",\"tags\":[],\"created_at\":1700000000}. Any of kind/content/tags/created_at present here OVERRIDES the matching parameter above; id, pubkey and sig are always recomputed from the signing key and ignored if present. Leave empty to build the event from the individual fields." },
                    "pow": { "type": "integer", "minimum": 0, "maximum": 20, "default": 0, "description": "NIP-13 proof-of-work difficulty in leading zero bits of the event id, 0 to 20. 0 (the default) skips mining. Anything above ~16 can take a noticeable while; a mined event carries a [\"nonce\",\"<counter>\",\"<target>\"] tag and any nonce tag you supplied is replaced." },
                    "output": { "type": "string", "enum": ["event", "relay-message", "report"], "default": "event", "description": "Output shape. 'event' (default) is the signed event JSON object. 'relay-message' wraps it as [\"EVENT\", {…}], the exact frame a relay websocket expects. 'report' prints a labeled summary — id, note1…, pubkey, npub1…, timestamp, kind, proof-of-work bits, signature and a verification line — followed by the event JSON." },
                    "pretty": { "type": "boolean", "default": true, "description": "Indent the JSON output. True (the default) is easier to read; false emits the single-line compact form that relays and other tools expect when pasting." }
                },
                "required": ["nsec"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The page/web wrapper and the CLI must produce identical bytes for the
    /// same inputs — they both funnel through `core::sign_event`.
    #[test]
    fn run_logic_signs_with_an_explicit_timestamp() {
        let args: Args = serde_json::from_str(
            r#"{"nsec":"nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5",
                "content":"hello from gizza","created_at":1700000000,"pretty":false}"#,
        )
        .unwrap();
        let out = run_logic(args).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v["pubkey"],
            "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e"
        );
        assert_eq!(v["kind"], 1);
        assert_eq!(v["created_at"], 1700000000);
        assert_eq!(v["sig"].as_str().unwrap().len(), 128);
    }

    #[test]
    fn missing_nsec_is_an_argument_error() {
        // Args is deliberately NOT Debug — it holds a secret key — so match
        // rather than unwrap_err().
        let err = match serde_json::from_str::<Args>(r#"{"content":"x"}"#) {
            Ok(_) => panic!("expected a missing-nsec error"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("nsec"), "got {err}");
    }
}
