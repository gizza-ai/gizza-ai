//! gizza-ai/sorted-order-checker — chat skill block on the shared tool abstraction.
//! Verifies a numeric list is sorted ascending or descending and pinpoints the
//! first out-of-order element. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_max_issues() -> u64 {
    20
}

#[derive(Deserialize)]
struct Args {
    numbers: String,
    #[serde(default)]
    order: String,
    #[serde(default)]
    strict: bool,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    strip_thousands: bool,
    #[serde(default)]
    non_numeric: String,
    #[serde(default = "default_max_issues")]
    max_issues: u64,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("numbers")
                .required()
                .describe(
                    "The numeric list to verify, as pasted text. Values may be separated by commas, \
                     spaces, tabs, newlines, semicolons or pipes (see separator). Integers, decimals, \
                     negatives, leading `+`, and scientific notation (`1e6`) are all accepted — for \
                     example `1, 4, 9, 2, 7`. Up to 20000 values per run. The list is never re-ordered; \
                     this reports on it.",
                ),
        )
        .param(
            Param::enumv("order", ["auto", "ascending", "descending"])
                .default("auto")
                .describe(
                    "Which order to verify. `auto` (default) reads the direction off the first two \
                     differing values and then checks the whole list against it — use it when you just \
                     want to know whether the list is in ANY order. `ascending` (smallest first) and \
                     `descending` (largest first) assert a specific direction and report every value \
                     that breaks it.",
                ),
        )
        .param(
            Param::boolean("strict")
                .default(false)
                .describe(
                    "Require a change at every step. Default false: equal neighbours count as sorted, \
                     so `1 2 2 5` is ascending. Set true for a strictly increasing/decreasing list — \
                     repeated values are then reported as out-of-order (useful for checking unique, \
                     gap-free ids or timestamps).",
                ),
        )
        .param(
            Param::enumv(
                "separator",
                ["auto", "comma", "newline", "space", "semicolon", "tab", "pipe"],
            )
            .default("auto")
            .describe(
                "How the values are separated. `auto` (default) splits on any whitespace, comma, \
                 semicolon or pipe. Pick an explicit separator for ambiguous data — for example \
                 `newline` for one value per line, or `space` together with strip_thousands for \
                 `1,024 2,048`. Blank entries are skipped.",
            ),
        )
        .param(
            Param::boolean("strip_thousands")
                .default(false)
                .describe(
                    "Treat `,` and `_` INSIDE a value as digit-group separators, so `1,234,567` and \
                     `1_000` parse as single numbers. Default false. When true, commas stop acting as \
                     delimiters under separator=auto; combining it with separator=comma is rejected, \
                     because a comma cannot be both.",
                ),
        )
        .param(
            Param::enumv("non_numeric", ["error", "ignore"])
                .default("error")
                .describe(
                    "What to do with a token that is not a number (a header word, `n/a`, `NaN` — which \
                     has no order at all). `error` (default) stops and names the offending token and its \
                     position; `ignore` skips it and checks the remaining values, reporting how many were \
                     skipped.",
                ),
        )
        .param(
            Param::integer("max_issues")
                .default(20)
                .min(1.0)
                .max(1000.0)
                .describe(
                    "How many out-of-order elements to LIST, 1-1000 (default 20). The first one is always \
                     shown; the totals count every break regardless, and a `… and N more` line says how \
                     many were withheld. Raise it to see every break in a badly-shuffled list.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe(
                    "Output format. `text` (default) is the readable report: verdict, first out-of-order \
                     element with its reason, every break, and the longest sorted run. `json` returns the \
                     same report as an object (`sorted`, `direction`, `first_break`, `breaks`, \
                     `longest_run`, `min`/`max`) for scripting.",
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
    name = "gizza-ai/sorted-order-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check whether a numeric list is sorted ascending or descending and pinpoint the first out-of-order value.",
    skill(
        description = "Verify that a pasted list of numbers is in order and, when it is not, pinpoint the first out-of-order element. Checks ascending, descending, or auto-detects the direction from the values; `strict` decides whether equal neighbours count as sorted. Reports the first break (position, value, the neighbour it violates and why), every further break up to max_issues, the longest already-sorted run, and min/max/first/last. Accepts comma, space, tab, newline, semicolon or pipe separated values, decimals, negatives and scientific notation; optional thousands-separator stripping. Text or JSON output. Report-only — it never re-orders the list.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sorted-order-checker", |a: Args| {
            gizza_ai_sorted_order_checker_core::run_with_options(
                &a.numbers,
                &a.order,
                a.strict,
                &a.separator,
                a.strip_thousands,
                &a.non_numeric,
                a.max_issues,
                &a.format,
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

    /// Drift guard: the chat/CLI schema is the contract other surfaces consume.
    #[test]
    fn schema_matches_expected() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let expected: serde_json::Value = serde_json::from_str(EXPECTED_SCHEMA).unwrap();
        assert_eq!(
            actual["required"], expected["required"],
            "required params drifted"
        );
        let props = actual["properties"].as_object().unwrap();
        let exp_props = expected["properties"].as_object().unwrap();
        assert_eq!(
            props.keys().collect::<Vec<_>>(),
            exp_props.keys().collect::<Vec<_>>(),
            "param set drifted"
        );
        for (name, exp) in exp_props {
            let got = &props[name];
            assert_eq!(got["type"], exp["type"], "{name}: type drifted");
            assert_eq!(got["enum"], exp["enum"], "{name}: enum drifted");
            assert_eq!(got["default"], exp["default"], "{name}: default drifted");
            assert!(
                got["description"].as_str().is_some_and(|d| d.len() > 40),
                "{name}: needs a real .describe()"
            );
        }
    }

    const EXPECTED_SCHEMA: &str = r#"{
      "required": ["numbers"],
      "properties": {
        "numbers":         { "type": "string" },
        "order":           { "type": "string", "enum": ["auto", "ascending", "descending"], "default": "auto" },
        "strict":          { "type": "boolean", "default": false },
        "separator":       { "type": "string", "enum": ["auto", "comma", "newline", "space", "semicolon", "tab", "pipe"], "default": "auto" },
        "strip_thousands": { "type": "boolean", "default": false },
        "non_numeric":     { "type": "string", "enum": ["error", "ignore"], "default": "error" },
        "max_issues":      { "type": "integer", "default": 20 },
        "format":          { "type": "string", "enum": ["text", "json"], "default": "text" }
      }
    }"#;
}
