//! gizza-ai/merge-conflict-resolver — rewrite the conflict markers Git leaves in a
//! file. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_merge_conflict_resolver_core::resolve;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    strategy: String,
    #[serde(default)]
    choices: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    strict: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The conflicted file text, pasted with its Git markers intact ('<<<<<<<', an optional '|||||||' common-ancestor section, '=======', '>>>>>>>'). Everything outside a conflict block is preserved byte for byte. Up to 1 MB."),
        )
        .param(
            Param::enumv(
                "strategy",
                ["ours", "theirs", "both", "both-theirs-first", "base", "keep"],
            )
            .default("ours")
            .describe("Which side every conflict collapses to: 'ours' the current branch (default), 'theirs' the incoming branch, 'both' ours then theirs, 'both-theirs-first' theirs then ours, 'base' the '|||||||' common ancestor (diff3/zdiff3 input only), 'keep' leaves the block and its markers untouched."),
        )
        .param(
            Param::string("choices")
                .describe("Per-conflict overrides using the numbers from output=list, comma-separated: '2=theirs, 3-5=both, 4-=keep, all=ours'. A later entry wins over an earlier overlapping one. Leave empty to apply 'strategy' to every conflict."),
        )
        .param(
            Param::enumv("output", ["resolved", "list", "sides", "json"])
                .default("resolved")
                .describe("What to return: 'resolved' the rewritten file text (default), 'list' a numbered inventory with line spans, branch labels and the chosen strategy, 'sides' an aligned ours-vs-theirs text comparison, 'json' the full inventory plus the resolved text."),
        )
        .param(
            Param::boolean("strict")
                .describe("Fail instead of returning text when the input has no conflict markers at all, or when any conflict is still left unresolved by 'keep'. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MergeConflictResolver;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/merge-conflict-resolver",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Resolve Git merge conflict markers by keeping ours, theirs, both or the common ancestor",
    skill(
        description = "Rewrite the merge conflict markers Git leaves in a working-tree file. Paste the whole conflicted file and every '<<<<<<< / ======= / >>>>>>>' block collapses to the side you pick: `ours` (current branch, default), `theirs` (incoming), `both` in either order, `base` (the '|||||||' common-ancestor section that the diff3 and zdiff3 conflict styles add), or `keep` to leave a block untouched. `choices` overrides individual conflicts by number ('2=theirs, 3-5=both'), matching the numbering shown by `output=list`. `output` also offers `sides` (an aligned ours-vs-theirs comparison) and `json` (per-conflict line spans, branch labels, side contents and the resolved text). `strict` turns a marker-free paste or a surviving conflict into an error, which is useful as a pre-commit gate. Text outside conflict blocks is preserved byte for byte, CRLF line endings and a missing final newline survive, and a bare '=======' outside a conflict is treated as ordinary text. Malformed marker sequences report the offending line number instead of producing silently wrong output. Fully local and deterministic — no AI model, no repository access.",
        parameters = schema_json()
    ),
)]
impl MergeConflictResolver {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "merge-conflict-resolver", |a: Args| {
            resolve(&a.text, &a.strategy, &a.choices, &a.output, a.strict)
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
                    "text": { "type": "string", "description": "The conflicted file text, pasted with its Git markers intact ('<<<<<<<', an optional '|||||||' common-ancestor section, '=======', '>>>>>>>'). Everything outside a conflict block is preserved byte for byte. Up to 1 MB." },
                    "strategy": { "type": "string", "enum": ["ours", "theirs", "both", "both-theirs-first", "base", "keep"], "default": "ours", "description": "Which side every conflict collapses to: 'ours' the current branch (default), 'theirs' the incoming branch, 'both' ours then theirs, 'both-theirs-first' theirs then ours, 'base' the '|||||||' common ancestor (diff3/zdiff3 input only), 'keep' leaves the block and its markers untouched." },
                    "choices": { "type": "string", "description": "Per-conflict overrides using the numbers from output=list, comma-separated: '2=theirs, 3-5=both, 4-=keep, all=ours'. A later entry wins over an earlier overlapping one. Leave empty to apply 'strategy' to every conflict." },
                    "output": { "type": "string", "enum": ["resolved", "list", "sides", "json"], "default": "resolved", "description": "What to return: 'resolved' the rewritten file text (default), 'list' a numbered inventory with line spans, branch labels and the chosen strategy, 'sides' an aligned ours-vs-theirs text comparison, 'json' the full inventory plus the resolved text." },
                    "strict": { "type": "boolean", "description": "Fail instead of returning text when the input has no conflict markers at all, or when any conflict is still left unresolved by 'keep'. Default false." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_resolve_to_ours_and_the_resolved_view() {
        let a: Args = serde_json::from_str(r#"{"text":"<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> t\n"}"#)
            .unwrap();
        assert_eq!(a.strategy, "");
        assert!(!a.strict);
        let out = resolve(&a.text, &a.strategy, &a.choices, &a.output, a.strict).unwrap();
        assert_eq!(out, "x\n");
    }

    #[test]
    fn an_unknown_strategy_is_rejected_through_the_skill_path() {
        let err = resolve("<<<<<<<\nx\n=======\ny\n>>>>>>>\n", "mine", "", "", false).unwrap_err();
        assert!(err.contains("unknown strategy 'mine'"), "{err}");
    }
}
