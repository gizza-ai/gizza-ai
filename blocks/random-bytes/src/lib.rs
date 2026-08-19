//! gizza-ai/random-bytes — draw N cryptographically random BYTES and render
//! those exact bytes as hex, Base64, Base64URL, binary, decimal, a C array
//! literal or a Python bytes literal. Byte-count driven: `bytes = 32` is always
//! 256 bits, whatever the encoding. (The sibling `random-token-generator` block
//! is character-count driven instead — 32 hex CHARACTERS carry 128 bits.)
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handler delegates to run_skill. Pure (getrandom CSPRNG) → all backends incl.
//! the chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_random_bytes_core::{run, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_bytes")]
    bytes: u64,
    #[serde(default = "default_count")]
    count: u64,
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default)]
    uppercase: bool,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    seed_hex: String,
}

fn default_bytes() -> u64 {
    32
}
fn default_count() -> u64 {
    1
}
fn default_encoding() -> String {
    "hex".into()
}
fn default_separator() -> String {
    "auto".into()
}
fn default_output() -> String {
    "text".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::integer("bytes")
                .min(1.0)
                .max(4096.0)
                .default(32)
                .describe("How many random bytes each value contains (1-4096). Default 32, i.e. 256 bits — the size of an AES-256 key, a JWT/HMAC secret or a session token. Use 16 for 128 bits, 64 for 512 bits. The byte count fixes the entropy; the encoding only changes how many characters are printed (32 bytes = 64 hex characters = 44 base64 characters)."),
        )
        .param(
            Param::integer("count")
                .min(1.0)
                .max(100.0)
                .default(1)
                .describe("How many independent values to draw (1-100). Default 1. Each value is a fresh draw and is printed on its own line. bytes x count may not exceed 8192 random bytes per run."),
        )
        .param(
            Param::enumv(
                "encoding",
                ["hex", "base64", "base64url", "binary", "decimal", "c-array", "python-bytes"],
            )
            .default("hex")
            .describe("How the bytes are written out: 'hex' (default, 2 lowercase hex digits per byte, same as openssl rand -hex), 'base64' (RFC 4648 standard alphabet with = padding, same as openssl rand -base64), 'base64url' (URL-safe alphabet, no padding — for tokens and JWT segments), 'binary' (8 bits per byte), 'decimal' (0-255 per byte), 'c-array' (a C initializer list like { 0x1a, 0x2b }) or 'python-bytes' (a literal like b'\\x1a\\x2b')."),
        )
        .param(
            Param::enumv("separator", ["auto", "none", "space", "colon", "dash", "comma"])
                .default("auto")
                .describe("Text placed between bytes, for the one-unit-per-byte encodings only (hex, binary, decimal). 'auto' (default) uses each encoding's convention: nothing for hex, a space for binary, a comma for decimal. 'none' joins them with nothing; 'space', 'colon' (MAC/fingerprint style), 'dash' and 'comma' insert that character. base64, base64url, c-array and python-bytes have no per-byte boundary to split, so they ignore this."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("Uppercase the hex digits. Default false (lowercase, matching openssl). Applies to the 'hex' and 'c-array' encodings; base64, binary, decimal and python-bytes are unaffected."),
        )
        .param(
            Param::enumv("output", ["text", "json"])
                .default("text")
                .describe("How the result is rendered: 'text' (default, one value per line plus a one-line summary of byte count, bit count, encoding and the equivalent openssl command) or 'json' (an object with count, bytes, bits, encoding, uppercase, deterministic and the values array)."),
        )
        .param(
            Param::string("seed_hex")
                .default("")
                .describe("Optional 8-128 hex digits (4-64 bytes). Blank (the default) draws from the platform's cryptographic RNG, so every run differs — this is what you want for real keys. When set, the bytes are derived deterministically from the seed so the same seed always reproduces the same values: useful for tests, examples and shareable links, but such output is only as secret as the seed."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RandomBytes;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/random-bytes",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate secure random bytes as hex, Base64 or a byte array",
    skill(
        description = "Generate cryptographically secure random bytes locally and render those exact bytes in a chosen encoding. `bytes` (default 32 = 256 bits) sets the entropy; `encoding` sets the presentation: 'hex' (like openssl rand -hex), 'base64', 'base64url' (no padding), 'binary', 'decimal', 'c-array' ({ 0x1a, 0x2b }) or 'python-bytes' (b'\\x1a\\x2b'). `count` draws up to 100 independent values at once, `separator` inserts a byte separator for the per-byte encodings, and `uppercase` switches hex to capitals. Output as text with an entropy summary, or as json. Set `seed_hex` to derive the bytes deterministically from a seed instead of the RNG. Use this when you need a keyed number of BYTES (an AES key, HMAC secret, IV, salt or seed); for a target number of CHARACTERS from a chosen alphabet use the random-token-generator tool, and for human-typed passwords use password-generator.",
        parameters = schema_json()
    )
)]
impl RandomBytes {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "random-bytes", |a: Args| {
            run(&Options {
                bytes: a.bytes as usize,
                count: a.count as usize,
                encoding: a.encoding,
                separator: a.separator,
                uppercase: a.uppercase,
                output: a.output,
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
                    "bytes": { "type": "integer", "minimum": 1, "maximum": 4096, "default": 32, "description": "How many random bytes each value contains (1-4096). Default 32, i.e. 256 bits — the size of an AES-256 key, a JWT/HMAC secret or a session token. Use 16 for 128 bits, 64 for 512 bits. The byte count fixes the entropy; the encoding only changes how many characters are printed (32 bytes = 64 hex characters = 44 base64 characters)." },
                    "count": { "type": "integer", "minimum": 1, "maximum": 100, "default": 1, "description": "How many independent values to draw (1-100). Default 1. Each value is a fresh draw and is printed on its own line. bytes x count may not exceed 8192 random bytes per run." },
                    "encoding": { "type": "string", "enum": ["hex", "base64", "base64url", "binary", "decimal", "c-array", "python-bytes"], "default": "hex", "description": "How the bytes are written out: 'hex' (default, 2 lowercase hex digits per byte, same as openssl rand -hex), 'base64' (RFC 4648 standard alphabet with = padding, same as openssl rand -base64), 'base64url' (URL-safe alphabet, no padding — for tokens and JWT segments), 'binary' (8 bits per byte), 'decimal' (0-255 per byte), 'c-array' (a C initializer list like { 0x1a, 0x2b }) or 'python-bytes' (a literal like b'\\x1a\\x2b')." },
                    "separator": { "type": "string", "enum": ["auto", "none", "space", "colon", "dash", "comma"], "default": "auto", "description": "Text placed between bytes, for the one-unit-per-byte encodings only (hex, binary, decimal). 'auto' (default) uses each encoding's convention: nothing for hex, a space for binary, a comma for decimal. 'none' joins them with nothing; 'space', 'colon' (MAC/fingerprint style), 'dash' and 'comma' insert that character. base64, base64url, c-array and python-bytes have no per-byte boundary to split, so they ignore this." },
                    "uppercase": { "type": "boolean", "default": false, "description": "Uppercase the hex digits. Default false (lowercase, matching openssl). Applies to the 'hex' and 'c-array' encodings; base64, binary, decimal and python-bytes are unaffected." },
                    "output": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "How the result is rendered: 'text' (default, one value per line plus a one-line summary of byte count, bit count, encoding and the equivalent openssl command) or 'json' (an object with count, bytes, bits, encoding, uppercase, deterministic and the values array)." },
                    "seed_hex": { "type": "string", "default": "", "description": "Optional 8-128 hex digits (4-64 bytes). Blank (the default) draws from the platform's cryptographic RNG, so every run differs — this is what you want for real keys. When set, the bytes are derived deterministically from the seed so the same seed always reproduces the same values: useful for tests, examples and shareable links, but such output is only as secret as the seed." }
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
        assert_eq!(a.bytes as usize, d.bytes);
        assert_eq!(a.count as usize, d.count);
        assert_eq!(a.encoding, d.encoding);
        assert_eq!(a.separator, d.separator);
        assert_eq!(a.uppercase, d.uppercase);
        assert_eq!(a.output, d.output);
        assert_eq!(a.seed_hex, d.seed_hex);
    }
}
