//! gizza-ai/set-diff-json — union / intersection / difference / symmetric
//! difference over two JSON arrays, comparing elements as JSON values or
//! matching them on a chosen key field. Chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_set_diff_json_core::{set_diff, Operation, Options, Output};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    array_a: String,
    array_b: String,
    #[serde(default = "default_operation")]
    operation: String,
    #[serde(default)]
    key: String,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default = "default_dedupe")]
    dedupe: bool,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_operation() -> String {
    "difference".into()
}
fn default_dedupe() -> bool {
    true
}
fn default_output() -> String {
    "report".into()
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("array_a")
                .required()
                .describe("Array A, as a JSON array like [1, 2, 3] or [{\"id\":1,\"name\":\"Ada\"}]. NDJSON (one JSON value per line, 2+ lines) is also accepted. Max 50000 elements."),
        )
        .param(
            Param::string("array_b")
                .required()
                .describe("Array B, in the same formats as array_a. Difference computes A minus B, so order matters for difference."),
        )
        .param(
            Param::enumv(
                "operation",
                ["union", "intersection", "difference", "symmetric_difference"],
            )
            .default("difference")
            .describe("Set operation: \"union\" everything in either array, \"intersection\" elements in both, \"difference\" (default) elements of A missing from B, \"symmetric_difference\" elements in exactly one array."),
        )
        .param(
            Param::string("key")
                .default("")
                .describe("Field to match elements on instead of comparing whole values, e.g. \"id\" or a dot-path like \"meta.sku\" / \"tags.0\". Every element must carry it. Default blank: compare whole JSON values (object key order and 1 vs 1.0 are ignored)."),
        )
        .param(
            Param::boolean("case_insensitive")
                .default(false)
                .describe("Fold string case when matching, so \"Apple\" and \"APPLE\" are the same element. Default false."),
        )
        .param(
            Param::boolean("dedupe")
                .default(true)
                .describe("Collapse repeats so the result is a true set. Default true; set false to keep every occurrence of a matching element."),
        )
        .param(
            Param::enumv("output", ["report", "array"])
                .default("report")
                .describe("Result shape: \"report\" (default) a JSON object with operation, matched_by, counts and result; \"array\" the bare result array, ready to paste into the next step."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Indentation in spaces for the JSON output (0-8). Use 0 to minify onto one line. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from_args(a: &Args) -> Result<Options, String> {
    Ok(Options {
        operation: Operation::parse(&a.operation)?,
        key: a.key.trim().to_string(),
        case_insensitive: a.case_insensitive,
        dedupe: a.dedupe,
        output: Output::parse(&a.output)?,
        indent: (a.indent as usize).min(8),
    })
}

#[cfg(target_arch = "wasm32")]
struct SetDiffJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/set-diff-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Union, intersection and difference of two JSON arrays",
    skill(
        description = "Compute a set operation over two JSON arrays: union, intersection, difference (A minus B) or symmetric difference. Elements are compared as JSON *values*, so object key order and 1 vs 1.0 never matter, or matched on a chosen field with `key` (a name or dot-path like meta.sku) the way lodash differenceBy works. `case_insensitive` folds string case, `dedupe` (default on) makes the result a true set. Returns a JSON report { operation, matched_by, counts: { a, b, a_unique, b_unique, only_in_a, only_in_b, in_both, union, result }, result: [...] }, or just the result array with output=array. Elements come from array A wherever an operation could draw from either side, and the first occurrence of a repeated key wins. Inputs may be JSON arrays or NDJSON; each is capped at 50000 elements. Runs locally.",
        parameters = schema_json()
    ),
)]
impl SetDiffJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "set-diff-json", |a: Args| {
            let opts = options_from_args(&a).map_err(SkillError::InvalidArgs)?;
            set_diff(&a.array_a, &a.array_b, &opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the chat/CLI schema the descriptor renders must stay exactly
    /// what is authored here, so a param rename or a lost enum can't slip
    /// through (the page picks its controls from this schema via manifest.json).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "array_a": {
                        "type": "string",
                        "description": "Array A, as a JSON array like [1, 2, 3] or [{\"id\":1,\"name\":\"Ada\"}]. NDJSON (one JSON value per line, 2+ lines) is also accepted. Max 50000 elements."
                    },
                    "array_b": {
                        "type": "string",
                        "description": "Array B, in the same formats as array_a. Difference computes A minus B, so order matters for difference."
                    },
                    "operation": {
                        "type": "string",
                        "enum": ["union", "intersection", "difference", "symmetric_difference"],
                        "default": "difference",
                        "description": "Set operation: \"union\" everything in either array, \"intersection\" elements in both, \"difference\" (default) elements of A missing from B, \"symmetric_difference\" elements in exactly one array."
                    },
                    "key": {
                        "type": "string",
                        "default": "",
                        "description": "Field to match elements on instead of comparing whole values, e.g. \"id\" or a dot-path like \"meta.sku\" / \"tags.0\". Every element must carry it. Default blank: compare whole JSON values (object key order and 1 vs 1.0 are ignored)."
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "default": false,
                        "description": "Fold string case when matching, so \"Apple\" and \"APPLE\" are the same element. Default false."
                    },
                    "dedupe": {
                        "type": "boolean",
                        "default": true,
                        "description": "Collapse repeats so the result is a true set. Default true; set false to keep every occurrence of a matching element."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["report", "array"],
                        "default": "report",
                        "description": "Result shape: \"report\" (default) a JSON object with operation, matched_by, counts and result; \"array\" the bare result array, ready to paste into the next step."
                    },
                    "indent": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 8,
                        "default": 2,
                        "description": "Indentation in spaces for the JSON output (0-8). Use 0 to minify onto one line. Default 2."
                    }
                },
                "required": ["array_a", "array_b"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }

    #[test]
    fn args_defaults_match_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"array_a":"[1,2]","array_b":"[2]"}"#).unwrap();
        assert_eq!(a.operation, "difference");
        assert_eq!(a.key, "");
        assert!(!a.case_insensitive);
        assert!(a.dedupe);
        assert_eq!(a.output, "report");
        assert_eq!(a.indent, 2);
        let opts = options_from_args(&a).unwrap();
        assert_eq!(opts.operation, Operation::Difference);
        assert_eq!(opts.output, Output::Report);
        assert_eq!(opts.indent, 2);
        let out = set_diff(&a.array_a, &a.array_b, &opts).unwrap();
        assert!(out.contains("\"result\": ["), "{out}");
    }

    #[test]
    fn every_operation_choice_parses_and_runs() {
        for (choice, expected) in [
            ("union", "[1,2,3]"),
            ("intersection", "[2]"),
            ("difference", "[1]"),
            ("symmetric_difference", "[1,3]"),
        ] {
            let a: Args = serde_json::from_str(&format!(
                r#"{{"array_a":"[1,2]","array_b":"[2,3]","operation":"{choice}","output":"array","indent":0}}"#
            ))
            .unwrap();
            let opts = options_from_args(&a).unwrap();
            assert_eq!(set_diff(&a.array_a, &a.array_b, &opts).unwrap(), expected);
        }
    }

    #[test]
    fn rejects_unknown_choice_values() {
        let a: Args =
            serde_json::from_str(r#"{"array_a":"[1]","array_b":"[1]","operation":"sideways"}"#)
                .unwrap();
        let err = options_from_args(&a).unwrap_err();
        assert!(err.contains("invalid operation"), "got {err}");
    }

    #[test]
    fn indent_is_clamped_to_the_advertised_maximum() {
        let a: Args = serde_json::from_str(
            r#"{"array_a":"[1]","array_b":"[2]","output":"array","indent":99}"#,
        )
        .unwrap();
        assert_eq!(options_from_args(&a).unwrap().indent, 8);
    }
}
