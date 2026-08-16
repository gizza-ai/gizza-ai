//! gizza-ai/api-response-diff — compare two JSON API responses while ignoring
//! volatile fields (timestamps, ids, request-ids), so only meaningful changes
//! surface. Chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_api_response_diff_core::{
    diff_responses, parse_array_match, parse_output, Options,
};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    #[serde(default)]
    ignore: String,
    #[serde(default)]
    ignore_timestamps: bool,
    #[serde(default)]
    ignore_uuids: bool,
    #[serde(default = "default_array_match")]
    array_match: String,
    #[serde(default = "default_array_key")]
    array_key: String,
    #[serde(default)]
    numeric_tolerance: f64,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    trim_strings: bool,
    #[serde(default)]
    null_equals_missing: bool,
    #[serde(default)]
    coerce_types: bool,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_array_match() -> String {
    "index".into()
}
fn default_array_key() -> String {
    "id".into()
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
            Param::string("left")
                .required()
                .describe("The first (baseline/old) API response, as JSON text."),
        )
        .param(
            Param::string("right")
                .required()
                .describe("The second (candidate/new) API response, as JSON text."),
        )
        .param(Param::string("ignore").describe(
            "Comma- or newline-separated fields to ignore, e.g. \"requestId, updatedAt, $.data.token\". A bare name (updatedAt, *_at) ignores that key at any depth; a dotted path (data.token or $.data.token) is anchored at the root; '*' matches one path segment, '**' any number, and [2]/[*] match array indices. Default: nothing ignored.",
        ))
        .param(
            Param::boolean("ignore_timestamps")
                .default(false)
                .describe("Ignore a value change when BOTH sides look like timestamps (ISO-8601 dates/datetimes, or epoch seconds/milliseconds). Default false."),
        )
        .param(
            Param::boolean("ignore_uuids")
                .default(false)
                .describe("Ignore a value change when BOTH sides look like canonical 8-4-4-4-12 UUIDs. Default false."),
        )
        .param(
            Param::enumv("array_match", ["index", "key", "set"])
                .default("index")
                .describe("How array elements are paired before comparing: \"index\" position by position (default), \"key\" pair objects by the array_key field so reordered lists still diff field-by-field, \"set\" order-insensitive multiset comparison."),
        )
        .param(
            Param::string("array_key")
                .default("id")
                .describe("Field used to pair array elements when array_match=key, e.g. \"id\" or \"sku\". Arrays whose elements are not objects with a unique value for it fall back to index matching (reported under notes). Default \"id\"."),
        )
        .param(
            Param::number("numeric_tolerance")
                .min(0.0)
                .default(0.0)
                .describe("Absolute tolerance for number comparisons: values within this distance count as equal, e.g. 0.01 for money rounding. Default 0 (exact)."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare strings case-insensitively. Default false."),
        )
        .param(
            Param::boolean("trim_strings")
                .default(false)
                .describe("Trim leading/trailing whitespace from strings before comparing. Default false."),
        )
        .param(
            Param::boolean("null_equals_missing")
                .default(false)
                .describe("Treat an explicit null the same as an absent field, so null -> missing is not reported. Default false."),
        )
        .param(
            Param::boolean("coerce_types")
                .default(false)
                .describe("Compare across scalar types, so \"5\" equals 5 and \"true\" equals true. Default false (a type change is reported as type_changed)."),
        )
        .param(
            Param::enumv("output", ["report", "summary", "patch"])
                .default("report")
                .describe("Result shape: \"report\" full JSON with counts, notes, ignored_paths and per-change entries (default); \"summary\" one readable line per change (+ - ~ !); \"patch\" an RFC 6902 JSON Patch array (requires array_match=index)."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Indentation in spaces for the JSON outputs (0-8). Use 0 to minify. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from_args(a: &Args) -> Result<Options, String> {
    Ok(Options {
        ignore: a.ignore.clone(),
        ignore_timestamps: a.ignore_timestamps,
        ignore_uuids: a.ignore_uuids,
        array_match: parse_array_match(&a.array_match)?,
        array_key: a.array_key.clone(),
        numeric_tolerance: a.numeric_tolerance,
        ignore_case: a.ignore_case,
        trim_strings: a.trim_strings,
        null_equals_missing: a.null_equals_missing,
        coerce_types: a.coerce_types,
        output: parse_output(&a.output)?,
        indent: (a.indent as usize).min(8),
    })
}

#[cfg(target_arch = "wasm32")]
struct ApiResponseDiff;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/api-response-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Diff two JSON API responses while ignoring volatile fields",
    skill(
        description = "Compare two JSON API responses and report only the meaningful differences. Volatile fields (request ids, timestamps, generated ids) can be dropped by name/path pattern via `ignore`, or by value shape via `ignore_timestamps`/`ignore_uuids`. Arrays are paired by index, by a key field (array_match=key + array_key), or as unordered sets (array_match=set); numbers accept a tolerance and strings optional case/whitespace/type leniency. Returns a JSON report { equal, counts, truncated, notes, ignored_paths, changes:[{ path, kind: added|removed|changed|type_changed, old?, new? }] } with paths like $.data.items[2].name, or a readable summary (output=summary), or an RFC 6902 JSON Patch (output=patch). Runs locally; each response is capped at 4 MiB.",
        parameters = schema_json()
    ),
)]
impl ApiResponseDiff {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "api-response-diff", |a: Args| {
            let opts = options_from_args(&a).map_err(SkillError::InvalidArgs)?;
            diff_responses(&a.left, &a.right, &opts).map_err(SkillError::InvalidArgs)
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
                    "left": {
                        "type": "string",
                        "description": "The first (baseline/old) API response, as JSON text."
                    },
                    "right": {
                        "type": "string",
                        "description": "The second (candidate/new) API response, as JSON text."
                    },
                    "ignore": {
                        "type": "string",
                        "description": "Comma- or newline-separated fields to ignore, e.g. \"requestId, updatedAt, $.data.token\". A bare name (updatedAt, *_at) ignores that key at any depth; a dotted path (data.token or $.data.token) is anchored at the root; '*' matches one path segment, '**' any number, and [2]/[*] match array indices. Default: nothing ignored."
                    },
                    "ignore_timestamps": {
                        "type": "boolean",
                        "default": false,
                        "description": "Ignore a value change when BOTH sides look like timestamps (ISO-8601 dates/datetimes, or epoch seconds/milliseconds). Default false."
                    },
                    "ignore_uuids": {
                        "type": "boolean",
                        "default": false,
                        "description": "Ignore a value change when BOTH sides look like canonical 8-4-4-4-12 UUIDs. Default false."
                    },
                    "array_match": {
                        "type": "string",
                        "enum": ["index", "key", "set"],
                        "default": "index",
                        "description": "How array elements are paired before comparing: \"index\" position by position (default), \"key\" pair objects by the array_key field so reordered lists still diff field-by-field, \"set\" order-insensitive multiset comparison."
                    },
                    "array_key": {
                        "type": "string",
                        "default": "id",
                        "description": "Field used to pair array elements when array_match=key, e.g. \"id\" or \"sku\". Arrays whose elements are not objects with a unique value for it fall back to index matching (reported under notes). Default \"id\"."
                    },
                    "numeric_tolerance": {
                        "type": "number",
                        "minimum": 0,
                        "default": 0.0,
                        "description": "Absolute tolerance for number comparisons: values within this distance count as equal, e.g. 0.01 for money rounding. Default 0 (exact)."
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "default": false,
                        "description": "Compare strings case-insensitively. Default false."
                    },
                    "trim_strings": {
                        "type": "boolean",
                        "default": false,
                        "description": "Trim leading/trailing whitespace from strings before comparing. Default false."
                    },
                    "null_equals_missing": {
                        "type": "boolean",
                        "default": false,
                        "description": "Treat an explicit null the same as an absent field, so null -> missing is not reported. Default false."
                    },
                    "coerce_types": {
                        "type": "boolean",
                        "default": false,
                        "description": "Compare across scalar types, so \"5\" equals 5 and \"true\" equals true. Default false (a type change is reported as type_changed)."
                    },
                    "output": {
                        "type": "string",
                        "enum": ["report", "summary", "patch"],
                        "default": "report",
                        "description": "Result shape: \"report\" full JSON with counts, notes, ignored_paths and per-change entries (default); \"summary\" one readable line per change (+ - ~ !); \"patch\" an RFC 6902 JSON Patch array (requires array_match=index)."
                    },
                    "indent": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 8,
                        "default": 2,
                        "description": "Indentation in spaces for the JSON outputs (0-8). Use 0 to minify. Default 2."
                    }
                },
                "required": ["left", "right"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }

    #[test]
    fn args_defaults_match_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"left":"{}","right":"{}"}"#).unwrap();
        assert_eq!(a.array_match, "index");
        assert_eq!(a.array_key, "id");
        assert_eq!(a.output, "report");
        assert_eq!(a.indent, 2);
        assert!(!a.ignore_timestamps);
        let opts = options_from_args(&a).unwrap();
        assert_eq!(diff_responses("{}", "{}", &opts).unwrap().contains("\"equal\": true"), true);
    }

    #[test]
    fn rejects_unknown_choice_values() {
        let a: Args =
            serde_json::from_str(r#"{"left":"{}","right":"{}","array_match":"random"}"#).unwrap();
        let err = options_from_args(&a).unwrap_err();
        assert!(err.contains("index, key, set"), "got {err}");
    }
}
