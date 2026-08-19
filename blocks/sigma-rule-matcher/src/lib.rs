//! gizza-ai/sigma-rule-matcher — evaluate pasted Sigma YAML detection rules
//! against already-parsed Windows event JSON. The descriptor is the single
//! source for chat, CLI, and page parameters; the pure core owns all parsing and
//! matching logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    rules: String,
    events: String,
    #[serde(default = "default_min_level")]
    min_level: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_max_matches")]
    max_matches: u64,
    #[serde(default)]
    show_event: bool,
}

fn default_min_level() -> String {
    "any".to_string()
}
fn default_status() -> String {
    "any".to_string()
}
fn default_output() -> String {
    "report".to_string()
}
fn default_max_matches() -> u64 {
    gizza_ai_sigma_rule_matcher_core::DEFAULT_MAX_MATCHES as u64
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("rules").required().describe("Sigma detection rules as YAML. Paste one rule or multiple YAML documents separated by ---. Supports common detection selections, field modifiers, and conditions."))
        .param(Param::string("events").required().describe("Windows event records as JSON: a JSON array, one JSON object, or newline-delimited JSON. Use parsed EVTX output, not raw .evtx bytes."))
        .param(Param::enumv("min_level", ["any", "informational", "low", "medium", "high", "critical"]).default("any").describe("Lowest Sigma severity to evaluate. Use any (default) to keep all levels, or informational/low/medium/high/critical."))
        .param(Param::enumv("status", ["any", "stable", "test", "experimental", "deprecated", "unsupported"]).default("any").describe("Only evaluate rules with this Sigma status. Default any keeps stable, test, experimental, deprecated, unsupported, and blank statuses."))
        .param(Param::enumv("output", ["report", "table", "json"]).default("report").describe("Output shape: report (human-readable summary), table (Markdown rows), or json (structured detections and counts). Default report."))
        .param(Param::integer("max_matches").min(0.0).max(gizza_ai_sigma_rule_matcher_core::MAX_MATCHES_CAP as f64).default(gizza_ai_sigma_rule_matcher_core::DEFAULT_MAX_MATCHES).describe("Maximum detections to print (0 uses the default 500; hard cap 10000). The summary still reports the true detection count."))
        .param(Param::boolean("show_event").default(false).describe("Include the full matching event record in json/table/report output. Default false keeps results compact."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_from_args(a: Args) -> Result<String, SkillError> {
    gizza_ai_sigma_rule_matcher_core::match_rules(
        &a.rules,
        &a.events,
        &a.min_level,
        &a.status,
        &a.output,
        (a.max_matches as u32).min(gizza_ai_sigma_rule_matcher_core::MAX_MATCHES_CAP),
        a.show_event,
    )
    .map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sigma-rule-matcher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Run Sigma detection rules against parsed Windows event JSON",
    skill(
        description = "Run Sigma detection rules against already-parsed Windows event records. Paste Sigma YAML in rules and JSON events in events (a JSON array, one object, or newline-delimited JSON). The matcher supports common Sigma detection maps/lists, keyword lists, list OR/ALL semantics, null/exists checks, wildcards, contains/startswith/endswith, regex, CIDR, numeric comparisons, base64/base64offset, UTF-16/wide, windash, fieldref, cased, and condition expressions with and/or/not, parentheses, 1 of/all of/N of/prefix*/them. Use min_level and status to filter rules, output=report/table/json for result shape, max_matches to cap displayed detections, and show_event=true when you need the full matching record. EVTX binary parsing, bundled rule sets, backend query conversion, and correlation/aggregation rules are intentionally out of scope; feed JSON from an EVTX parser instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sigma-rule-matcher", run_from_args) {
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
            "type":"object",
            "properties":{
                "rules":{"type":"string","description":"Sigma detection rules as YAML. Paste one rule or multiple YAML documents separated by ---. Supports common detection selections, field modifiers, and conditions."},
                "events":{"type":"string","description":"Windows event records as JSON: a JSON array, one JSON object, or newline-delimited JSON. Use parsed EVTX output, not raw .evtx bytes."},
                "min_level":{"type":"string","enum":["any","informational","low","medium","high","critical"],"default":"any","description":"Lowest Sigma severity to evaluate. Use any (default) to keep all levels, or informational/low/medium/high/critical."},
                "status":{"type":"string","enum":["any","stable","test","experimental","deprecated","unsupported"],"default":"any","description":"Only evaluate rules with this Sigma status. Default any keeps stable, test, experimental, deprecated, unsupported, and blank statuses."},
                "output":{"type":"string","enum":["report","table","json"],"default":"report","description":"Output shape: report (human-readable summary), table (Markdown rows), or json (structured detections and counts). Default report."},
                "max_matches":{"type":"integer","minimum":0,"maximum":10000,"default":500,"description":"Maximum detections to print (0 uses the default 500; hard cap 10000). The summary still reports the true detection count."},
                "show_event":{"type":"boolean","default":false,"description":"Include the full matching event record in json/table/report output. Default false keeps results compact."}
            },
            "required":["rules","events"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no chat/CLI schema drift");
    }

    #[test]
    fn args_defaults_run_a_basic_match() {
        let a: Args = serde_json::from_str(r#"{"rules":"title: Encoded PowerShell\nlevel: high\ndetection:\n  sel:\n    EventID: 4104\n    ScriptBlockText|contains: '-enc'\n  condition: sel\n","events":"[{\"EventID\":4104,\"ScriptBlockText\":\"powershell -enc SQ\"}]"}"#).unwrap();
        assert_eq!(a.min_level, "any");
        assert_eq!(a.status, "any");
        assert_eq!(a.output, "report");
        assert_eq!(a.max_matches, 500);
        assert!(!a.show_event);
        let out = run_from_args(a).unwrap();
        assert!(out.contains("Detections:     1"), "{out}");
        assert!(out.contains("Encoded PowerShell"), "{out}");
    }
}
