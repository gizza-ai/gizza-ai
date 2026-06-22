//! gizza-ai/find-replace — find & replace text (literal or regex), with case and
//! global options. Thin wrapper around the core; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_find_replace_core::find_replace;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    find: String,
    #[serde(default)]
    replace: String,
    #[serde(default)]
    regex: bool,
    #[serde(default = "default_true")]
    case_sensitive: bool,
    #[serde(default = "default_true")]
    global: bool,
}
fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct Resp {
    /// The text after replacement.
    text: String,
    /// Number of matches replaced.
    count: usize,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The text to search within."))
        .param(Param::string("find").required().describe("The text or regular expression to find."))
        .param(Param::string("replace").default("").describe("Replacement text. In regex mode, $1/${name} insert capture groups; in literal mode it is inserted verbatim. Default empty (deletes matches)."))
        .param(Param::boolean("regex").default(false).describe("Treat `find` as a regular expression. Default false (literal match)."))
        .param(Param::boolean("case_sensitive").default(true).describe("Match case-sensitively. Default true."))
        .param(Param::boolean("global").default(true).describe("Replace every match. When false, only the first match is replaced. Default true."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FindReplace;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/find-replace",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find and replace text (literal or regex)",
    skill(
        description = "Find and replace text. By default `find` is matched literally; set regex=true to use a regular expression (with $1/${name} capture references in `replace`). case_sensitive (default true) and global (default true; false = first match only) control matching. Returns the new text and the number of replacements.",
        parameters = schema_json()
    )
)]
impl FindReplace {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "find-replace", |a: Args| {
            let (text, count) =
                find_replace(&a.text, &a.find, &a.replace, a.regex, a.case_sensitive, a.global)
                    .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { text, count })
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
                    "text":           { "type": "string", "description": "The text to search within." },
                    "find":           { "type": "string", "description": "The text or regular expression to find." },
                    "replace":        { "type": "string", "default": "", "description": "Replacement text. In regex mode, $1/${name} insert capture groups; in literal mode it is inserted verbatim. Default empty (deletes matches)." },
                    "regex":          { "type": "boolean", "default": false, "description": "Treat `find` as a regular expression. Default false (literal match)." },
                    "case_sensitive": { "type": "boolean", "default": true, "description": "Match case-sensitively. Default true." },
                    "global":         { "type": "boolean", "default": true, "description": "Replace every match. When false, only the first match is replaced. Default true." }
                },
                "required": ["text", "find"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
