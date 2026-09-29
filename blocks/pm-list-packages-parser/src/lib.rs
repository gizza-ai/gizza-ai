//! gizza-ai/pm-list-packages-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_filter() -> String {
    "all".into()
}
fn default_format() -> String {
    "table".into()
}
fn default_sort() -> String {
    "package".into()
}
fn default_group() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_filter")]
    filter: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_group")]
    group: bool,
    #[serde(default)]
    disabled_list: String,
    #[serde(default)]
    system_list: String,
}

/// Single source for the chat schema (and the CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().multiline().describe(
            "Raw output of `adb shell pm list packages -f`, one `package:` line per app. Plain `package:com.example.app` lines also work, as do the `-i` (installer=), `-U` (uid:) and `--show-versioncode` (versionCode:) decorations. Blank lines, `#` comments and a copied shell prompt are ignored. Max 5000 lines.",
        ))
        .param(
            Param::enumv(
                "filter",
                ["all", "user", "system", "system-updated", "enabled", "disabled"],
            )
            .default("all")
            .describe(
                "Which packages to keep. user = installed under /data, system = ships with the ROM (including updated ones), system-updated = preinstalled but now running an update from /data/app, enabled/disabled = needs the disabled_list paste.",
            ),
        )
        .param(
            Param::enumv("format", ["table", "list", "csv", "json", "markdown"])
                .default("table")
                .describe(
                    "Output shape. table is an aligned text table with counts, list is bare package names one per line, csv and json carry the full fixed column schema for scripts, markdown is a pipe table for tickets and wikis.",
                ),
        )
        .param(
            Param::enumv("sort", ["package", "type", "path"])
                .default("package")
                .describe(
                    "Row order: package = alphabetical by package name, type = user apps first then updated system then system, path = by APK path.",
                ),
        )
        .param(Param::boolean("group").default(true).describe(
            "Split the table into `User apps` / `Updated system apps` / `System apps` sections with per-section counts. Applies to the table and markdown formats only; list, csv and json are always flat.",
        ))
        .param(Param::string("disabled_list").multiline().describe(
            "Optional. Output of `adb shell pm list packages -d` (disabled packages). A single package list carries no enabled/disabled bit, so without this the status column reads `unknown` and the enabled/disabled filters match nothing.",
        ))
        .param(Param::string("system_list").multiline().describe(
            "Optional. Output of `adb shell pm list packages -s` (system packages). Use it to mark preinstalled apps that now run an update from /data/app as `system-updated` instead of `user`.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pm-list-packages-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse `adb shell pm list packages` output into a clean table split into system, user and disabled apps.",
    skill(
        description = "Turn raw `adb shell pm list packages -f` output into a clean, filterable table. Classifies every package as user, system or updated-system from its APK partition, splits out the installer, uid and versionCode decorations, and filters the list. Optional pastes of `pm list packages -d` and `-s` add enabled/disabled status and accurate updated-system detection. Outputs an aligned table, bare package names, CSV, JSON or a Markdown table.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pm-list-packages-parser", |a: Args| {
            gizza_ai_pm_list_packages_parser_core::run(
                &a.input,
                &a.filter,
                &a.format,
                &a.sort,
                a.group,
                &a.disabled_list,
                &a.system_list,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
          "type":"object","properties":{
            "input":{"type":"string","description":"Raw output of `adb shell pm list packages -f`, one `package:` line per app. Plain `package:com.example.app` lines also work, as do the `-i` (installer=), `-U` (uid:) and `--show-versioncode` (versionCode:) decorations. Blank lines, `#` comments and a copied shell prompt are ignored. Max 5000 lines."},
            "filter":{"type":"string","enum":["all","user","system","system-updated","enabled","disabled"],"default":"all","description":"Which packages to keep. user = installed under /data, system = ships with the ROM (including updated ones), system-updated = preinstalled but now running an update from /data/app, enabled/disabled = needs the disabled_list paste."},
            "format":{"type":"string","enum":["table","list","csv","json","markdown"],"default":"table","description":"Output shape. table is an aligned text table with counts, list is bare package names one per line, csv and json carry the full fixed column schema for scripts, markdown is a pipe table for tickets and wikis."},
            "sort":{"type":"string","enum":["package","type","path"],"default":"package","description":"Row order: package = alphabetical by package name, type = user apps first then updated system then system, path = by APK path."},
            "group":{"type":"boolean","default":true,"description":"Split the table into `User apps` / `Updated system apps` / `System apps` sections with per-section counts. Applies to the table and markdown formats only; list, csv and json are always flat."},
            "disabled_list":{"type":"string","description":"Optional. Output of `adb shell pm list packages -d` (disabled packages). A single package list carries no enabled/disabled bit, so without this the status column reads `unknown` and the enabled/disabled filters match nothing."},
            "system_list":{"type":"string","description":"Optional. Output of `adb shell pm list packages -s` (system packages). Use it to mark preinstalled apps that now run an update from /data/app as `system-updated` instead of `user`."}
          },"required":["input"],"additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
