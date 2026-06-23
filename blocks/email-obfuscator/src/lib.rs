//! gizza-ai/email-obfuscator — chat skill block on the shared tool abstraction.
//! Encodes an email address into scraper-resistant HTML (numeric entities, a
//! char-code JavaScript writer, a CSS bidi reversal, or a ROT13 mailto). The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    email: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    entity_style: String,
    #[serde(default = "default_link")]
    link: bool,
    #[serde(default)]
    link_text: String,
}

fn default_link() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("email")
                .required()
                .describe("The email address to obfuscate, e.g. you@example.com."),
        )
        .param(
            Param::enumv("mode", ["entities", "js", "css", "rot13"])
                .default("entities")
                .describe(
                    "How to encode the address. 'entities' (default) replaces every character with an HTML numeric character reference (radix from entity_style) — the browser renders the address but a regex scraper sees only '&#106;…'. 'js' emits a <script> that assembles the address from character codes at runtime via document.write (with a <noscript> entity fallback), so the address is never a literal in the served HTML. 'css' prints the address reversed and flips it back with unicode-bidi:bidi-override;direction:rtl. 'rot13' is a mailto: link whose href is ROT13-scrambled and decoded by an inline onclick handler.",
                ),
        )
        .param(
            Param::enumv("entity_style", ["decimal", "hex"])
                .default("decimal")
                .describe("Radix for the HTML numeric entities used by 'entities'/'css' modes and the JS/ROT13 <noscript> fallbacks. 'decimal' → &#106;, 'hex' → &#x6a;. Default decimal."),
        )
        .param(
            Param::boolean("link")
                .default(true)
                .describe("Wrap the address in a clickable mailto: anchor (entities/js/rot13 modes). When false, only the obfuscated address text is emitted. The css mode always emits display text (a reversed href can't be clicked). Default true."),
        )
        .param(
            Param::string("link_text")
                .default("")
                .describe("Optional visible link text for the anchor (e.g. 'Email us'). When blank, the obfuscated address itself is shown. Ignored by css mode."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-obfuscator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode an email address into scraper-resistant HTML",
    skill(
        description = "Encode an email address into scraper-resistant HTML you can paste into a page, defeating naive address-harvesting bots while keeping the address readable and clickable in a real browser. mode='entities' (default) replaces every character with an HTML numeric character reference (entity_style='decimal' → &#106;…, 'hex' → &#x6a;…), optionally wrapped in an entity-encoded mailto: anchor. mode='js' emits a <script> that builds the address from character codes via document.write, plus a <noscript> entity fallback, so the address never appears as a literal in the served HTML. mode='css' prints the address reversed in the source and flips it back with unicode-bidi:bidi-override;direction:rtl. mode='rot13' is a mailto: link whose href is ROT13-scrambled and decoded by an inline onclick handler. link (default true) wraps the address in a clickable anchor; link_text sets optional visible link text. Validates the address (local@domain with a dotted domain). Runs locally — the address never leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "email-obfuscator", |a: Args| {
            gizza_ai_email_obfuscator_core::obfuscate(
                &a.email,
                &gizza_ai_email_obfuscator_core::Options {
                    mode: &a.mode,
                    entity_style: &a.entity_style,
                    link: a.link,
                    link_text: &a.link_text,
                },
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "email": { "type": "string", "description": "The email address to obfuscate, e.g. you@example.com." },
                    "mode": { "type": "string", "enum": ["entities", "js", "css", "rot13"], "default": "entities", "description": "How to encode the address. 'entities' (default) replaces every character with an HTML numeric character reference (radix from entity_style) — the browser renders the address but a regex scraper sees only '&#106;…'. 'js' emits a <script> that assembles the address from character codes at runtime via document.write (with a <noscript> entity fallback), so the address is never a literal in the served HTML. 'css' prints the address reversed and flips it back with unicode-bidi:bidi-override;direction:rtl. 'rot13' is a mailto: link whose href is ROT13-scrambled and decoded by an inline onclick handler." },
                    "entity_style": { "type": "string", "enum": ["decimal", "hex"], "default": "decimal", "description": "Radix for the HTML numeric entities used by 'entities'/'css' modes and the JS/ROT13 <noscript> fallbacks. 'decimal' → &#106;, 'hex' → &#x6a;. Default decimal." },
                    "link": { "type": "boolean", "default": true, "description": "Wrap the address in a clickable mailto: anchor (entities/js/rot13 modes). When false, only the obfuscated address text is emitted. The css mode always emits display text (a reversed href can't be clicked). Default true." },
                    "link_text": { "type": "string", "default": "", "description": "Optional visible link text for the anchor (e.g. 'Email us'). When blank, the obfuscated address itself is shown. Ignored by css mode." }
                },
                "required": ["email"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
