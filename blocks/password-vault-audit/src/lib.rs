//! gizza-ai/password-vault-audit — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute apart from
//! the clock the stale-password check needs — no host calls, no network.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_password_vault_audit_core::{audit, Options, OutputForm, SourceFormat};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}
fn default_min_length() -> u32 {
    12
}
fn default_min_score() -> u32 {
    40
}
fn default_max_age_days() -> u32 {
    365
}

#[derive(Deserialize)]
struct Args {
    data: String,
    /// Reader to use; blank/"auto" sniffs the shape.
    #[serde(default)]
    format: String,
    #[serde(default = "default_min_length")]
    min_length: u32,
    #[serde(default = "default_min_score")]
    min_score: u32,
    #[serde(default = "default_max_age_days")]
    max_age_days: u32,
    #[serde(default = "default_true")]
    check_common: bool,
    #[serde(default = "default_true")]
    check_reuse: bool,
    #[serde(default = "default_true")]
    check_similar: bool,
    #[serde(default = "default_true")]
    check_insecure_urls: bool,
    #[serde(default)]
    check_missing_2fa: bool,
    #[serde(default = "default_true")]
    mask_passwords: bool,
    /// Output form: report | json | csv.
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The vault to audit: either a plain list with one password per line, or a password-manager export. Supported exports are Bitwarden JSON and any CSV whose header names a password column (Bitwarden, LastPass, KeePass/KeePassXC, Chrome, 1Password, Dashlane, generic). Up to 5000 entries per run. The text is only read — nothing is uploaded, stored or rewritten."),
        )
        .param(
            Param::enumv("format", ["auto", "list", "csv", "bitwarden-json"])
                .default("auto")
                .describe("How to read the input. 'auto' (default) sniffs it: a leading '{' or '[' is Bitwarden JSON, a multi-column header row naming a password column is CSV, anything else is a one-per-line list. Force 'list', 'csv' or 'bitwarden-json' when the sniff guesses wrong."),
        )
        .param(
            Param::integer("min_length")
                .default(12)
                .min(1.0)
                .max(256.0)
                .describe("Flag any password shorter than this many characters. Default 12."),
        )
        .param(
            Param::integer("min_score")
                .default(40)
                .min(0.0)
                .max(100.0)
                .describe("Flag any password whose strength score falls below this. Scores run 0-100 with the usual bands: under 40 weak, 40-59 fair, 60-79 medium, 80+ strong. Default 40 (flag everything weak); set 60 to also flag fair passwords, or 0 to switch the strength check off."),
        )
        .param(
            Param::integer("max_age_days")
                .default(365)
                .min(0.0)
                .max(3650.0)
                .describe("Flag passwords last changed more than this many days ago. Only applies to entries whose export carries a modified/revision date (Bitwarden JSON always does; most CSVs do not). Default 365; set 0 to switch the age check off."),
        )
        .param(
            Param::boolean("check_common")
                .default(true)
                .describe("Match every password against a bundled list of the best-known common and breached passwords, including capitalisation and leetspeak variants (P@ssw0rd matches password). Runs offline against a fixed list — it is not a live breach-database lookup. Default true."),
        )
        .param(
            Param::boolean("check_reuse")
                .default(true)
                .describe("Report groups of entries that share one identical password, the single highest-value finding in a vault audit. Default true."),
        )
        .param(
            Param::boolean("check_similar")
                .default(true)
                .describe("Report entries whose passwords are variants of the same base once a trailing counter or symbol is stripped (Summer2024! / Summer2025?). Default true."),
        )
        .param(
            Param::boolean("check_insecure_urls")
                .default(true)
                .describe("Report saved logins whose URL uses unencrypted http:// rather than https://. Needs a URL/URI column, so it only applies to exports. Default true."),
        )
        .param(
            Param::boolean("check_missing_2fa")
                .default(false)
                .describe("Report saved logins that have a URL but no stored authenticator (TOTP) secret. Default false, because most people keep their second factor in a separate app and would otherwise see one finding per login."),
        )
        .param(
            Param::boolean("mask_passwords")
                .default(true)
                .describe("Keep plaintext passwords out of the output. Each password is shown as a short non-reversible fingerprint plus its length (#a3f1 (14 chars)), which is enough to correlate a reuse group without putting secrets into a report you might paste elsewhere. Set false to show the passwords themselves. Default true."),
        )
        .param(
            Param::enumv("output", ["report", "json", "csv"])
                .default("report")
                .describe("Output form: 'report' (default) a readable summary with a vault score, strength breakdown and findings grouped by severity; 'json' a structured {ok, entries, with_password, unique_passwords, vault_score, vault_band, error_count, warning_count, strength, findings[]} object; 'csv' one severity,rule,entry,detail row per finding for a spreadsheet."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Seconds since the Unix epoch. The chat/CLI runtimes both provide a real clock;
/// the browser build gets its own from `js_sys::Date` in the web crate.
fn now_unix() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(target_arch = "wasm32")]
struct PasswordVaultAudit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/password-vault-audit",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Audit a password list or vault export for reused, duplicate and weak passwords.",
    skill(
        description = "Audit a whole password vault at once and report every reused, duplicated, empty, common, short, weak, stale and insecurely-stored credential. Pass the vault in `data` as either a plain list with one password per line or a password-manager export — Bitwarden JSON, or any CSV whose header names a password column (Bitwarden, LastPass, KeePass/KeePassXC, Chrome, 1Password, Dashlane, generic); `format` forces a reader when the auto-sniff guesses wrong. Cross-entry checks: `reused-password` (entries sharing one identical password, grouped), `duplicate-entry` (the same item saved twice) and `similar-password` (variants of one base such as Summer2024!/Summer2025?). Per-entry checks: `common-password` (matched against a bundled offline list of well-known breached passwords, including capitalisation and leetspeak variants — not a live breach lookup), `weak-password` (a 0-100 strength score from length, character-class pool and pattern penalties, banded weak/fair/medium/strong, flagged below `min_score`), `short-password` (below `min_length`), `password-contains-username`, `low-character-variety`, `stale-password` (older than `max_age_days`, where the export carries a date), `insecure-url` (an http:// login) and `missing-2fa` (a login with no stored TOTP secret, off by default). Every check has its own on/off switch. The result carries a 0-100 vault score discounted by how much of the vault is reused, a strength breakdown, and findings sorted by severity then by how actionable they are. Passwords are masked by default — shown as a short non-reversible fingerprint plus a length so reuse groups stay correlatable without exposing secrets; set `mask_passwords` false to show them. `output` selects 'report' (readable), 'json' (structured) or 'csv' (one row per finding). Up to 5000 entries per run. Everything runs locally in the sandbox: no network, no storage, nothing uploaded.",
        parameters = schema_json()
    ),
)]
impl PasswordVaultAudit {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "password-vault-audit", |a: Args| {
            let opts = Options {
                format: SourceFormat::parse(&a.format).map_err(SkillError::InvalidArgs)?,
                min_length: a.min_length.clamp(1, 256) as usize,
                min_score: a.min_score.min(100),
                max_age_days: a.max_age_days.min(3650),
                check_common: a.check_common,
                check_reuse: a.check_reuse,
                check_similar: a.check_similar,
                check_insecure_urls: a.check_insecure_urls,
                check_missing_2fa: a.check_missing_2fa,
                mask_passwords: a.mask_passwords,
                output: OutputForm::parse(&a.output).map_err(SkillError::InvalidArgs)?,
            };
            audit(&a.data, &opts, now_unix()).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The vault to audit: either a plain list with one password per line, or a password-manager export. Supported exports are Bitwarden JSON and any CSV whose header names a password column (Bitwarden, LastPass, KeePass/KeePassXC, Chrome, 1Password, Dashlane, generic). Up to 5000 entries per run. The text is only read — nothing is uploaded, stored or rewritten." },
                    "format": { "type": "string", "enum": ["auto", "list", "csv", "bitwarden-json"], "default": "auto", "description": "How to read the input. 'auto' (default) sniffs it: a leading '{' or '[' is Bitwarden JSON, a multi-column header row naming a password column is CSV, anything else is a one-per-line list. Force 'list', 'csv' or 'bitwarden-json' when the sniff guesses wrong." },
                    "min_length": { "type": "integer", "default": 12, "minimum": 1, "maximum": 256, "description": "Flag any password shorter than this many characters. Default 12." },
                    "min_score": { "type": "integer", "default": 40, "minimum": 0, "maximum": 100, "description": "Flag any password whose strength score falls below this. Scores run 0-100 with the usual bands: under 40 weak, 40-59 fair, 60-79 medium, 80+ strong. Default 40 (flag everything weak); set 60 to also flag fair passwords, or 0 to switch the strength check off." },
                    "max_age_days": { "type": "integer", "default": 365, "minimum": 0, "maximum": 3650, "description": "Flag passwords last changed more than this many days ago. Only applies to entries whose export carries a modified/revision date (Bitwarden JSON always does; most CSVs do not). Default 365; set 0 to switch the age check off." },
                    "check_common": { "type": "boolean", "default": true, "description": "Match every password against a bundled list of the best-known common and breached passwords, including capitalisation and leetspeak variants (P@ssw0rd matches password). Runs offline against a fixed list — it is not a live breach-database lookup. Default true." },
                    "check_reuse": { "type": "boolean", "default": true, "description": "Report groups of entries that share one identical password, the single highest-value finding in a vault audit. Default true." },
                    "check_similar": { "type": "boolean", "default": true, "description": "Report entries whose passwords are variants of the same base once a trailing counter or symbol is stripped (Summer2024! / Summer2025?). Default true." },
                    "check_insecure_urls": { "type": "boolean", "default": true, "description": "Report saved logins whose URL uses unencrypted http:// rather than https://. Needs a URL/URI column, so it only applies to exports. Default true." },
                    "check_missing_2fa": { "type": "boolean", "default": false, "description": "Report saved logins that have a URL but no stored authenticator (TOTP) secret. Default false, because most people keep their second factor in a separate app and would otherwise see one finding per login." },
                    "mask_passwords": { "type": "boolean", "default": true, "description": "Keep plaintext passwords out of the output. Each password is shown as a short non-reversible fingerprint plus its length (#a3f1 (14 chars)), which is enough to correlate a reuse group without putting secrets into a report you might paste elsewhere. Set false to show the passwords themselves. Default true." },
                    "output": { "type": "string", "enum": ["report", "json", "csv"], "default": "report", "description": "Output form: 'report' (default) a readable summary with a vault score, strength breakdown and findings grouped by severity; 'json' a structured {ok, entries, with_password, unique_passwords, vault_score, vault_band, error_count, warning_count, strength, findings[]} object; 'csv' one severity,rule,entry,detail row per finding for a spreadsheet." }
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
