//! gizza-ai/likert-summary — chat skill block on the shared tool abstraction.
//!
//! Summarizes Likert-scale survey answers: per-item mean, SD, median, mode,
//! full response distribution, bottom-box / neutral / top-box percentages,
//! floor/ceiling flags, optional Cronbach's alpha, and text stacked bars
//! (optionally neutral-centred/diverging). The chat schema is single-sourced
//! from `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    input: String,
    #[serde(default)]
    items: String,
    #[serde(default = "five")]
    points: i64,
    #[serde(default)]
    scale: String,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    reverse: String,
    #[serde(default = "two")]
    box_size: i64,
    #[serde(default)]
    missing: String,
    #[serde(default)]
    sort: String,
    #[serde(default = "two")]
    decimals: i64,
    #[serde(default = "yes")]
    chart: bool,
    #[serde(default)]
    diverging: bool,
    #[serde(default)]
    alpha: bool,
    #[serde(default)]
    delimiter: String,
}

fn five() -> i64 {
    5
}
fn two() -> i64 {
    2
}
fn yes() -> bool {
    true
}

/// Single source for the chat schema (and CLI). `data` is required; every option
/// falls back to the documented default so a bare 5-point CSV summarizes cleanly.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data").required().describe(
                "The survey answers. With input=responses (default): a CSV whose first row is the \
                 item/question headers and each later row is one respondent, with answers as scale \
                 codes (1-points) or the scale labels themselves. With input=counts: one row per \
                 item — the item name, then how many respondents chose each category, lowest first.",
            ),
        )
        .param(
            Param::enumv("input", ["responses", "counts"])
                .default("responses")
                .describe(
                    "Shape of the data: responses = one row per respondent, one column per item; \
                     counts = one row per item holding a tally per scale category. Default \
                     responses.",
                ),
        )
        .param(Param::string("items").default("").describe(
            "Which columns are Likert items, as a comma-separated list of header names or 1-based \
             indices (e.g. 'Q1,Q3' or '2,3'). Blank = every column. Use this to skip respondent \
             IDs, timestamps, or free-text comments.",
        ))
        .param(
            Param::integer("points")
                .min(2.0)
                .max(11.0)
                .default(5)
                .describe(
                    "How many categories the scale has (4, 5 and 7 are the usual ones). Answers \
                     must be codes 1-points, where 1 is the most negative. Default 5.",
                ),
        )
        .param(
            Param::enumv(
                "scale",
                ["agreement", "satisfaction", "frequency", "quality", "numeric", "custom"],
            )
            .default("agreement")
            .describe(
                "Which label set to print next to each category: agreement (Strongly disagree … \
                 Strongly agree), satisfaction, frequency (Never … Always), quality (Poor … \
                 Excellent), numeric (1 … N), or custom with labels=. Named sets cover 2-7 points. \
                 Default agreement.",
            ),
        )
        .param(Param::string("labels").default("").describe(
            "Custom category labels, comma-separated, lowest first, one per scale point (e.g. \
             'Never,Rarely,Sometimes,Often,Always'). Required for scale=custom; overrides the \
             named set if given.",
        ))
        .param(Param::string("reverse").default("").describe(
            "Items to reverse-score before summarizing, as a comma-separated list of header names \
             or 1-based indices. Each answer becomes points + 1 - answer, so negatively worded \
             items point the same way as the rest.",
        ))
        .param(
            Param::integer("box_size")
                .min(1.0)
                .default(2)
                .describe(
                    "How many categories at each end make up the top box and bottom box: 2 gives \
                     the usual top-2-box / bottom-2-box percentages, 1 gives top-box only. Must be \
                     at most points / 2. Default 2.",
                ),
        )
        .param(
            Param::enumv("missing", ["exclude", "listwise"])
                .default("exclude")
                .describe(
                    "How to treat blank answers and NA/N-A/-/./none/null/missing/? markers: \
                     exclude drops them item by item (each item keeps every answer it has); \
                     listwise drops any respondent who skipped at least one item. Ignored for \
                     input=counts. Default exclude.",
                ),
        )
        .param(
            Param::enumv("sort", ["input", "mean-desc", "mean-asc", "top-desc"])
                .default("input")
                .describe(
                    "Row order of the item table: input (as given), mean-desc / mean-asc (by item \
                     mean), or top-desc (by top-box percentage). Default input.",
                ),
        )
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(6.0)
                .default(2)
                .describe(
                    "Decimal places for means, SDs, medians and Cronbach's alpha. Percentages are \
                     always shown to 1 decimal place. Default 2.",
                ),
        )
        .param(Param::boolean("chart").default(true).describe(
            "Draw a text stacked bar per item, 40 characters wide = 100% of that item's valid \
             answers, with a key mapping each character to its category. Default true.",
        ))
        .param(Param::boolean("diverging").default(false).describe(
            "Centre the stacked bars between the negative and positive halves of the scale (the \
             neutral category is split across the centre line) instead of stacking from the left. \
             Needs chart=true. Default false.",
        ))
        .param(Param::boolean("alpha").default(false).describe(
            "Add Cronbach's alpha, the internal-consistency reliability of the items taken as one \
             scale. Needs at least 2 items and 2 respondents who answered every item; not \
             available for input=counts. Default false.",
        ))
        .param(Param::string("delimiter").default(",").describe(
            "Field delimiter of the input: a single character or one of comma, tab, semicolon, \
             pipe. Default comma.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

impl Args {
    fn summarize(&self) -> Result<String, String> {
        gizza_ai_likert_summary_core::summarize(
            &self.data,
            &self.input,
            &self.items,
            self.points,
            &self.scale,
            &self.labels,
            &self.reverse,
            self.box_size,
            &self.missing,
            &self.sort,
            self.decimals,
            self.chart,
            self.diverging,
            self.alpha,
            &self.delimiter,
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/likert-summary",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize Likert survey answers into per-item means, distributions and stacked bars.",
    skill(
        description = "Summarize Likert-scale survey answers. Pass data as either a response CSV (input=responses, the default: first row = item headers, each later row = one respondent, answers as codes 1-points or as the scale labels) or a per-item tally (input=counts: item name then one count per category). Returns a monospaced report: a per-item table with n, missing, mean, SD, median, mode and bottom-box / neutral / top-box percentages; the full response distribution per item; floor and ceiling flags; the overall mean of item means; text stacked bars (optionally diverging around the neutral midpoint); and, with alpha=true, Cronbach's alpha. Options: points (2-11, default 5), scale (agreement/satisfaction/frequency/quality/numeric/custom) with labels for custom label sets, items to pick just the Likert columns, reverse to flip negatively worded items, box_size, missing (exclude/listwise), sort, decimals, chart, delimiter.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "likert-summary", |a: Args| {
            a.summarize().map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The survey answers. With input=responses (default): a CSV whose first row is the item/question headers and each later row is one respondent, with answers as scale codes (1-points) or the scale labels themselves. With input=counts: one row per item — the item name, then how many respondents chose each category, lowest first." },
                    "input": { "type": "string", "enum": ["responses","counts"], "default": "responses", "description": "Shape of the data: responses = one row per respondent, one column per item; counts = one row per item holding a tally per scale category. Default responses." },
                    "items": { "type": "string", "default": "", "description": "Which columns are Likert items, as a comma-separated list of header names or 1-based indices (e.g. 'Q1,Q3' or '2,3'). Blank = every column. Use this to skip respondent IDs, timestamps, or free-text comments." },
                    "points": { "type": "integer", "minimum": 2, "maximum": 11, "default": 5, "description": "How many categories the scale has (4, 5 and 7 are the usual ones). Answers must be codes 1-points, where 1 is the most negative. Default 5." },
                    "scale": { "type": "string", "enum": ["agreement","satisfaction","frequency","quality","numeric","custom"], "default": "agreement", "description": "Which label set to print next to each category: agreement (Strongly disagree … Strongly agree), satisfaction, frequency (Never … Always), quality (Poor … Excellent), numeric (1 … N), or custom with labels=. Named sets cover 2-7 points. Default agreement." },
                    "labels": { "type": "string", "default": "", "description": "Custom category labels, comma-separated, lowest first, one per scale point (e.g. 'Never,Rarely,Sometimes,Often,Always'). Required for scale=custom; overrides the named set if given." },
                    "reverse": { "type": "string", "default": "", "description": "Items to reverse-score before summarizing, as a comma-separated list of header names or 1-based indices. Each answer becomes points + 1 - answer, so negatively worded items point the same way as the rest." },
                    "box_size": { "type": "integer", "minimum": 1, "default": 2, "description": "How many categories at each end make up the top box and bottom box: 2 gives the usual top-2-box / bottom-2-box percentages, 1 gives top-box only. Must be at most points / 2. Default 2." },
                    "missing": { "type": "string", "enum": ["exclude","listwise"], "default": "exclude", "description": "How to treat blank answers and NA/N-A/-/./none/null/missing/? markers: exclude drops them item by item (each item keeps every answer it has); listwise drops any respondent who skipped at least one item. Ignored for input=counts. Default exclude." },
                    "sort": { "type": "string", "enum": ["input","mean-desc","mean-asc","top-desc"], "default": "input", "description": "Row order of the item table: input (as given), mean-desc / mean-asc (by item mean), or top-desc (by top-box percentage). Default input." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 6, "default": 2, "description": "Decimal places for means, SDs, medians and Cronbach's alpha. Percentages are always shown to 1 decimal place. Default 2." },
                    "chart": { "type": "boolean", "default": true, "description": "Draw a text stacked bar per item, 40 characters wide = 100% of that item's valid answers, with a key mapping each character to its category. Default true." },
                    "diverging": { "type": "boolean", "default": false, "description": "Centre the stacked bars between the negative and positive halves of the scale (the neutral category is split across the centre line) instead of stacking from the left. Needs chart=true. Default false." },
                    "alpha": { "type": "boolean", "default": false, "description": "Add Cronbach's alpha, the internal-consistency reliability of the items taken as one scale. Needs at least 2 items and 2 respondents who answered every item; not available for input=counts. Default false." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field delimiter of the input: a single character or one of comma, tab, semicolon, pipe. Default comma." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The defaults serde applies to an args-only payload must equal the
    /// defaults the schema advertises.
    #[test]
    fn serde_defaults_match_the_schema_defaults() {
        let a: Args = serde_json::from_str(r#"{"data":"Q1\n5\n"}"#).unwrap();
        assert_eq!(a.points, 5);
        assert_eq!(a.box_size, 2);
        assert_eq!(a.decimals, 2);
        assert!(a.chart);
        assert!(!a.diverging);
        assert!(!a.alpha);
        assert!(a.summarize().is_ok());
    }
}
