//! gizza-ai/c-vuln-pattern-scanner — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure → all
//! backends. Lexical heuristic: the snippet is never compiled, preprocessed,
//! linked or executed.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_profile")]
    profile: String,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default)]
    ignore: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_include_context")]
    include_context: bool,
}

fn default_language() -> String {
    "auto".into()
}
fn default_profile() -> String {
    "all".into()
}
fn default_min_severity() -> String {
    "all".into()
}
fn default_format() -> String {
    "text".into()
}
fn default_include_context() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe(
                    "The C or C++ source to scan — a function, a whole file, or a reduced \
                     snippet (up to 200,000 bytes). It is scanned as text: never compiled, \
                     preprocessed, linked or executed.",
                ),
        )
        .param(
            Param::enumv("language", ["auto", "c", "cpp"])
                .default("auto")
                .describe(
                    "Language of the snippet. auto (default) reads it as C++ when it sees \
                     markers such as std::, class, namespace, template or #include <iostream>, \
                     otherwise C. cpp adds the CPP-STREAM rule (unbounded cin >> into a char \
                     buffer); c never fires it.",
                ),
        )
        .param(
            Param::enumv("profile", ["all", "memory", "injection", "crypto", "banned"])
                .default("all")
                .describe(
                    "Rule family to run. all (default) runs every rule; memory keeps buffer, \
                     bounds, allocation and lifetime rules; injection keeps format strings, \
                     shell execution, temp-file and TOCTOU races; crypto keeps weak random and \
                     broken algorithms; banned keeps the dangerous-libc-function list.",
                ),
        )
        .param(
            Param::enumv("min_severity", ["all", "low", "medium", "high", "critical"])
                .default("all")
                .describe(
                    "Lowest severity to report. all (default) and low are equivalent and keep \
                     every finding; medium, high and critical drop everything below that level. \
                     Use high or critical to triage a large paste down to the likely-exploitable \
                     patterns.",
                ),
        )
        .param(
            Param::string("ignore")
                .default("")
                .describe(
                    "Rule codes to suppress, comma- or space-separated and case-insensitive — \
                     e.g. \"BOUNDED-COPY, MEM-LEAK\". Unknown codes are ignored. Use it to mute a \
                     rule that is noisy for your codebase without lowering min_severity.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json", "csv"])
                .default("text")
                .describe(
                    "Output format. text (default) is a readable report with a severity roll-up \
                     header; json returns language, profile, per-severity counts and a findings \
                     array; csv returns line,severity,code,cwe,message,source with RFC-4180 \
                     quoting for spreadsheets and CI.",
                ),
        )
        .param(
            Param::boolean("include_context")
                .default(true)
                .describe(
                    "Echo the offending source line under each finding (text), or fill the \
                     source field (json/csv). Turn it off for a compact one-line-per-finding \
                     report that is easier to diff or grep.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/c-vuln-pattern-scanner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan C/C++ source for vulnerability patterns with CWE ids and severities",
    skill(
        description = "Lexically scan pasted C or C++ source for common vulnerability patterns and report each hit with a line number, a severity (low/medium/high/critical), a rule code, a CWE identifier and a fix hint. Rules cover banned copies (strcpy/strcat/sprintf), gets, scanf %s without a field width, non-literal format strings, provable buffer overruns and off-by-one indexes, integer-overflow and signed-length size arithmetic, alloca/VLA stack growth, unchecked malloc results, leaks, use-after-free and double free, sizeof on a pointer, unbounded C++ cin >> extraction, shell execution, insecure temp files, TOCTOU races, weak randomness and broken crypto. Params: code (source, max 200,000 bytes), language (auto|c|cpp), profile (all|memory|injection|crypto|banned rule family), min_severity (all|low|medium|high|critical), ignore (comma-separated rule codes to mute), format (text|json|csv), include_context (echo the matching source line). Comments and string/char literal bodies are masked first, so a flagged name inside a comment or a string does not fire; a `// vuln-scan: ignore` comment suppresses its own line and the next. The code is never compiled, preprocessed, linked or executed, and there is no control-flow, data-flow, macro or type information — findings mean \"worth a human look\", never \"proven vulnerability\", and a clean report is not a proof of safety. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "c-vuln-pattern-scanner", |a: Args| {
            gizza_ai_c_vuln_pattern_scanner_core::scan_source(
                &a.code,
                &a.language,
                &a.profile,
                &a.min_severity,
                &a.ignore,
                &a.format,
                a.include_context,
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
                    "code": {
                        "type": "string",
                        "description": "The C or C++ source to scan — a function, a whole file, or a reduced snippet (up to 200,000 bytes). It is scanned as text: never compiled, preprocessed, linked or executed."
                    },
                    "language": {
                        "type": "string",
                        "enum": ["auto", "c", "cpp"],
                        "default": "auto",
                        "description": "Language of the snippet. auto (default) reads it as C++ when it sees markers such as std::, class, namespace, template or #include <iostream>, otherwise C. cpp adds the CPP-STREAM rule (unbounded cin >> into a char buffer); c never fires it."
                    },
                    "profile": {
                        "type": "string",
                        "enum": ["all", "memory", "injection", "crypto", "banned"],
                        "default": "all",
                        "description": "Rule family to run. all (default) runs every rule; memory keeps buffer, bounds, allocation and lifetime rules; injection keeps format strings, shell execution, temp-file and TOCTOU races; crypto keeps weak random and broken algorithms; banned keeps the dangerous-libc-function list."
                    },
                    "min_severity": {
                        "type": "string",
                        "enum": ["all", "low", "medium", "high", "critical"],
                        "default": "all",
                        "description": "Lowest severity to report. all (default) and low are equivalent and keep every finding; medium, high and critical drop everything below that level. Use high or critical to triage a large paste down to the likely-exploitable patterns."
                    },
                    "ignore": {
                        "type": "string",
                        "default": "",
                        "description": "Rule codes to suppress, comma- or space-separated and case-insensitive — e.g. \"BOUNDED-COPY, MEM-LEAK\". Unknown codes are ignored. Use it to mute a rule that is noisy for your codebase without lowering min_severity."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json", "csv"],
                        "default": "text",
                        "description": "Output format. text (default) is a readable report with a severity roll-up header; json returns language, profile, per-severity counts and a findings array; csv returns line,severity,code,cwe,message,source with RFC-4180 quoting for spreadsheets and CI."
                    },
                    "include_context": {
                        "type": "boolean",
                        "default": true,
                        "description": "Echo the offending source line under each finding (text), or fill the source field (json/csv). Turn it off for a compact one-line-per-finding report that is easier to diff or grep."
                    }
                },
                "required": ["code"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_match_the_descriptor_defaults() {
        // A chat call that sends only `code` must land on the same defaults the
        // schema advertises, or the LLM-visible contract lies.
        let a: Args = serde_json::from_str(r#"{"code":"int x;"}"#).unwrap();
        assert_eq!(a.language, "auto");
        assert_eq!(a.profile, "all");
        assert_eq!(a.min_severity, "all");
        assert_eq!(a.ignore, "");
        assert_eq!(a.format, "text");
        assert!(a.include_context);
    }
}
