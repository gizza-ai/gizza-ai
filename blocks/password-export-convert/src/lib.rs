//! gizza-ai/password-export-convert — chat skill block on the shared tool abstraction.
//! Converts a password-manager export between Bitwarden JSON, Bitwarden CSV,
//! KeePass/KeePassXC CSV, LastPass CSV, Chrome CSV and a neutral generic CSV.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_from")]
    from: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default = "yes")]
    include_totp: bool,
    #[serde(default = "yes")]
    include_extra_fields: bool,
    #[serde(default)]
    skip_empty_passwords: bool,
    #[serde(default)]
    default_folder: String,
}

fn default_from() -> String {
    "auto".to_string()
}
fn default_to() -> String {
    "keepass-csv".to_string()
}
fn yes() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The password-manager export to convert, pasted as text: either a Bitwarden JSON export (starts with {) or a CSV export with its header row intact. Export unencrypted — a password-protected Bitwarden JSON is rejected, because decrypting it is a separate job. Up to 5000 entries per run."),
        )
        .param(
            Param::enumv(
                "from",
                [
                    "auto",
                    "bitwarden-json",
                    "bitwarden-csv",
                    "keepass-csv",
                    "lastpass-csv",
                    "chrome-csv",
                    "generic-csv",
                ],
            )
            .default("auto")
            .describe("The format the pasted export is already in. \"auto\" (default) sniffs it: a leading { or [ means Bitwarden JSON, otherwise the CSV header row decides (login_password → bitwarden-csv, grouping+extra → lastpass-csv, Title+Group → keepass-csv, name+url+note → chrome-csv, anything else with a password or username column → generic-csv). Name the format by hand when the header row has been edited and auto-detection guesses wrong."),
        )
        .param(
            Param::enumv(
                "to",
                [
                    "bitwarden-json",
                    "bitwarden-csv",
                    "keepass-csv",
                    "lastpass-csv",
                    "chrome-csv",
                    "generic-csv",
                ],
            )
            .default("keepass-csv")
            .describe("The format to write. \"keepass-csv\" (default) is the Group,Title,Username,Password,URL,Notes,TOTP,Icon,Last Modified,Created layout KeePassXC 2.6.2+ imports; \"bitwarden-json\" and \"bitwarden-csv\" are Bitwarden's two documented import shapes; \"lastpass-csv\" is url,username,password,totp,extra,name,grouping,fav; \"chrome-csv\" is the name,url,username,password,note layout Chrome, Edge and Safari accept; \"generic-csv\" is a neutral folder,name,url,username,password,notes,totp,favorite,type sheet for spreadsheets or an unlisted manager."),
        )
        .param(
            Param::boolean("include_totp")
                .default(true)
                .describe("Carry two-factor (TOTP) secrets across. On by default — this is the thing most converters silently drop. Written to the TOTP column of every target that has one (KeePass, Bitwarden, LastPass, generic); Chrome CSV has no such column, so the secret is folded into that entry's note instead of being lost. Turn it off to strip 2FA secrets from the output entirely."),
        )
        .param(
            Param::boolean("include_extra_fields")
                .default(true)
                .describe("Keep the data the target format has no column for by appending it to each entry's notes: Bitwarden custom fields, card and identity details, secondary URIs, and any CSV column the reader didn't claim. On by default. Turn it off for the smallest possible notes field, accepting that those details are dropped."),
        )
        .param(
            Param::boolean("skip_empty_passwords")
                .default(false)
                .describe("Drop entries that have no password before writing. Off by default, so secure notes, cards and identities survive the conversion. Turn it on when the target importer rejects a row with an empty password column, or to export only real logins."),
        )
        .param(
            Param::string("default_folder")
                .describe("Folder/group name to file entries that have none under — e.g. \"Imported\". Empty by default, which leaves them at the top level. Entries that already have a folder keep it; the name maps to the folder, group or grouping column of whichever target format is written."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PasswordExportConvert;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/password-export-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a password-manager export between Bitwarden, KeePass, LastPass, Chrome and generic CSV",
    skill(
        description = "Convert a password-manager export from one manager's format to another so it can be imported. Paste the export into `data`; the supported formats are Bitwarden JSON, Bitwarden CSV, KeePass/KeePassXC CSV, LastPass CSV, Chrome CSV and a neutral generic CSV, and any of them converts to any other. `from` defaults to \"auto\", which sniffs the format from the leading brace or the CSV header row; set it explicitly when the header has been edited. `to` picks the target layout and defaults to \"keepass-csv\". Every entry is projected onto folder, name, username, password, URL, notes, TOTP, favourite and kind, so folder/group structure survives in both directions. `include_totp` (on by default) carries 2FA secrets across — into the TOTP column where the target has one, folded into the note for Chrome CSV which does not. `include_extra_fields` (on by default) rescues what the target has no column for — Bitwarden custom fields, card and identity details, extra URIs, unclaimed CSV columns — into the notes. `skip_empty_passwords` drops password-less rows for importers that reject them, and `default_folder` files unfiled entries under one group. Reading and writing both go through a real RFC-4180 CSV parser, so a password containing a comma, a quote or a newline round-trips intact. Output is the converted file and nothing else — no header comment or summary line — and ids are hashed from names rather than random, so the same export always converts to the same bytes. Up to 5000 entries per run. Encrypted (password-protected) Bitwarden JSON is rejected with an explanation: re-export it unencrypted.",
        parameters = schema_json()
    ),
)]
impl PasswordExportConvert {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "password-export-convert", |a: Args| {
            gizza_ai_password_export_convert_core::run(
                &a.data,
                &a.from,
                &a.to,
                a.include_totp,
                a.include_extra_fields,
                a.skip_empty_passwords,
                &a.default_folder,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The password-manager export to convert, pasted as text: either a Bitwarden JSON export (starts with {) or a CSV export with its header row intact. Export unencrypted — a password-protected Bitwarden JSON is rejected, because decrypting it is a separate job. Up to 5000 entries per run." },
                    "from": { "type": "string", "enum": ["auto", "bitwarden-json", "bitwarden-csv", "keepass-csv", "lastpass-csv", "chrome-csv", "generic-csv"], "default": "auto", "description": "The format the pasted export is already in. \"auto\" (default) sniffs it: a leading { or [ means Bitwarden JSON, otherwise the CSV header row decides (login_password → bitwarden-csv, grouping+extra → lastpass-csv, Title+Group → keepass-csv, name+url+note → chrome-csv, anything else with a password or username column → generic-csv). Name the format by hand when the header row has been edited and auto-detection guesses wrong." },
                    "to": { "type": "string", "enum": ["bitwarden-json", "bitwarden-csv", "keepass-csv", "lastpass-csv", "chrome-csv", "generic-csv"], "default": "keepass-csv", "description": "The format to write. \"keepass-csv\" (default) is the Group,Title,Username,Password,URL,Notes,TOTP,Icon,Last Modified,Created layout KeePassXC 2.6.2+ imports; \"bitwarden-json\" and \"bitwarden-csv\" are Bitwarden's two documented import shapes; \"lastpass-csv\" is url,username,password,totp,extra,name,grouping,fav; \"chrome-csv\" is the name,url,username,password,note layout Chrome, Edge and Safari accept; \"generic-csv\" is a neutral folder,name,url,username,password,notes,totp,favorite,type sheet for spreadsheets or an unlisted manager." },
                    "include_totp": { "type": "boolean", "default": true, "description": "Carry two-factor (TOTP) secrets across. On by default — this is the thing most converters silently drop. Written to the TOTP column of every target that has one (KeePass, Bitwarden, LastPass, generic); Chrome CSV has no such column, so the secret is folded into that entry's note instead of being lost. Turn it off to strip 2FA secrets from the output entirely." },
                    "include_extra_fields": { "type": "boolean", "default": true, "description": "Keep the data the target format has no column for by appending it to each entry's notes: Bitwarden custom fields, card and identity details, secondary URIs, and any CSV column the reader didn't claim. On by default. Turn it off for the smallest possible notes field, accepting that those details are dropped." },
                    "skip_empty_passwords": { "type": "boolean", "default": false, "description": "Drop entries that have no password before writing. Off by default, so secure notes, cards and identities survive the conversion. Turn it on when the target importer rejects a row with an empty password column, or to export only real logins." },
                    "default_folder": { "type": "string", "description": "Folder/group name to file entries that have none under — e.g. \"Imported\". Empty by default, which leaves them at the top level. Entries that already have a folder keep it; the name maps to the folder, group or grouping column of whichever target format is written." }
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
