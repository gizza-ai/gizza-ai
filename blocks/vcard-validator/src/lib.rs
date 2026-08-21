//! gizza-ai/vcard-validator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute, no host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_vcard_validator_core::{Output, Version};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    data: String,
    /// Spec version to check against; blank/"auto" uses each card's VERSION.
    #[serde(default)]
    version: String,
    /// ISO-3166 alpha-2 region hint for national-format TEL values.
    #[serde(default)]
    default_country: String,
    /// Validate EMAIL values (default true).
    #[serde(default = "default_true")]
    check_email: bool,
    /// Validate TEL values (default true).
    #[serde(default = "default_true")]
    check_phone: bool,
    /// Extra comma-separated property names every card must carry.
    #[serde(default)]
    required_properties: String,
    /// Output form: report | json.
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("Raw vCard / .vcf text containing one or more BEGIN:VCARD ... END:VCARD blocks. Folded (continuation) lines are unfolded first, and every issue is reported against its original line number. The text is only read, never rewritten."),
        )
        .param(
            Param::enumv("version", ["auto", "2.1", "3.0", "4.0"])
                .default("auto")
                .describe("Which vCard specification to check against. 'auto' (default) uses each card's own VERSION property (a card with no VERSION is checked as 3.0). Pick 2.1, 3.0 or 4.0 to check every card against that spec and flag any card declaring a different VERSION."),
        )
        .param(
            Param::string("default_country")
                .default("")
                .describe("Optional ISO-3166 alpha-2 country/region hint (for example US, GB, DE) used to check TEL values written without a '+'. Leave blank to report national-format numbers as unverifiable instead of invalid."),
        )
        .param(
            Param::boolean("check_email")
                .default(true)
                .describe("Check EMAIL values for valid address syntax (single '@', non-empty local part, dotted domain with valid labels). Default true; set false to skip the EMAIL rules."),
        )
        .param(
            Param::boolean("check_phone")
                .default(true)
                .describe("Check TEL values as real phone numbers (libphonenumber rules, honouring default_country and stripping a leading 'tel:'). Default true; set false to skip the TEL rules."),
        )
        .param(
            Param::string("required_properties")
                .default("")
                .describe("Extra comma-separated property names that every card must carry, on top of the version's own requirements — for example 'UID,ORG' for a CardDAV-style profile. Matched case-insensitively. Default none."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output form: 'report' (default) a readable per-card list of issues with line numbers, severities and rule names, or 'json' a structured {ok, cards, error_count, warning_count, versions, issues[]} object for CI."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VcardValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vcard-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate vCard/.vcf files and flag bad phones, emails and missing fields.",
    skill(
        description = "Validate a vCard/.vcf address book against the specification and report every problem without rewriting the file. Pass the card text in `data`; the tool unfolds continuation lines, splits the document into cards, and checks: structure (BEGIN/END pairing, content outside a card, stray fold lines, over-long unfolded lines, LF-only line endings), VERSION (present, known, first property in 4.0, matching an explicitly requested version), required properties (FN in 3.0/4.0, N in 2.1/3.0, plus anything named in required_properties), line syntax (missing ':', property-name charset, group prefixes, empty or bare parameters — bare parameters are legal in 2.1 only — unquoted parameter values, a 2.1-only CHARSET parameter), values (EMAIL address syntax, TEL phone-number validity via libphonenumber with an optional default_country hint, BDAY/ANNIVERSARY dates including vCard 4.0 partial dates, REV timestamps, absolute URIs for URL/SOURCE/FBURL/CALURI, exactly 5 components in N and 7 in ADR, KIND and GENDER enums in 4.0) and hygiene (single-instance properties appearing twice, non-standard properties without an X- prefix, empty values). Every issue carries a card number, a 1-indexed source line, a severity (error/warning) and a stable rule name. Turn off check_email or check_phone to silence those rule groups. `output` selects 'report' (default, human-readable) or 'json' (structured, for CI). Returns an error when the input contains no vCard at all.",
        parameters = schema_json()
    ),
)]
impl VcardValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "vcard-validator", |a: Args| {
            let version = Version::parse(&a.version).map_err(SkillError::InvalidArgs)?;
            let output = Output::parse(&a.output).map_err(SkillError::InvalidArgs)?;
            gizza_ai_vcard_validator_core::validate(
                &a.data,
                version,
                &a.default_country,
                a.check_email,
                a.check_phone,
                &a.required_properties,
                output,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "Raw vCard / .vcf text containing one or more BEGIN:VCARD ... END:VCARD blocks. Folded (continuation) lines are unfolded first, and every issue is reported against its original line number. The text is only read, never rewritten." },
                    "version": { "type": "string", "enum": ["auto", "2.1", "3.0", "4.0"], "default": "auto", "description": "Which vCard specification to check against. 'auto' (default) uses each card's own VERSION property (a card with no VERSION is checked as 3.0). Pick 2.1, 3.0 or 4.0 to check every card against that spec and flag any card declaring a different VERSION." },
                    "default_country": { "type": "string", "default": "", "description": "Optional ISO-3166 alpha-2 country/region hint (for example US, GB, DE) used to check TEL values written without a '+'. Leave blank to report national-format numbers as unverifiable instead of invalid." },
                    "check_email": { "type": "boolean", "default": true, "description": "Check EMAIL values for valid address syntax (single '@', non-empty local part, dotted domain with valid labels). Default true; set false to skip the EMAIL rules." },
                    "check_phone": { "type": "boolean", "default": true, "description": "Check TEL values as real phone numbers (libphonenumber rules, honouring default_country and stripping a leading 'tel:'). Default true; set false to skip the TEL rules." },
                    "required_properties": { "type": "string", "default": "", "description": "Extra comma-separated property names that every card must carry, on top of the version's own requirements — for example 'UID,ORG' for a CardDAV-style profile. Matched case-insensitively. Default none." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output form: 'report' (default) a readable per-card list of issues with line numbers, severities and rule names, or 'json' a structured {ok, cards, error_count, warning_count, versions, issues[]} object for CI." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
