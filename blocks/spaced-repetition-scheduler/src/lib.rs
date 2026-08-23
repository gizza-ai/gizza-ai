//! gizza-ai/spaced-repetition-scheduler — next review dates from a batch review log.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn algorithm_default() -> String {
    "sm2".to_string()
}
fn grade_scale_default() -> String {
    "auto".to_string()
}
fn output_default() -> String {
    "table".to_string()
}
fn sort_default() -> String {
    "due".to_string()
}
fn desired_retention_default() -> f64 {
    0.9
}
fn ease_start_default() -> f64 {
    2.5
}
fn min_ease_default() -> f64 {
    1.3
}
fn first_interval_default() -> f64 {
    1.0
}
fn second_interval_default() -> f64 {
    6.0
}
fn easy_bonus_default() -> f64 {
    1.3
}
fn hard_multiplier_default() -> f64 {
    1.2
}
fn interval_modifier_default() -> f64 {
    1.0
}
fn lapse_multiplier_default() -> f64 {
    0.0
}
fn max_interval_default() -> f64 {
    36500.0
}
fn leech_threshold_default() -> i64 {
    8
}
fn forecast_reviews_default() -> i64 {
    6
}
fn forecast_grade_default() -> String {
    "good".to_string()
}

#[derive(Deserialize)]
struct Args {
    reviews: String,
    #[serde(default = "algorithm_default")]
    algorithm: String,
    #[serde(default = "grade_scale_default")]
    grade_scale: String,
    #[serde(default)]
    today: String,
    #[serde(default = "output_default")]
    output: String,
    #[serde(default = "sort_default")]
    sort: String,
    #[serde(default)]
    only_due: bool,
    #[serde(default = "desired_retention_default")]
    desired_retention: f64,
    #[serde(default = "ease_start_default")]
    ease_start: f64,
    #[serde(default = "min_ease_default")]
    min_ease: f64,
    #[serde(default = "first_interval_default")]
    first_interval: f64,
    #[serde(default = "second_interval_default")]
    second_interval: f64,
    #[serde(default = "easy_bonus_default")]
    easy_bonus: f64,
    #[serde(default = "hard_multiplier_default")]
    hard_multiplier: f64,
    #[serde(default = "interval_modifier_default")]
    interval_modifier: f64,
    #[serde(default = "lapse_multiplier_default")]
    lapse_multiplier: f64,
    #[serde(default = "max_interval_default")]
    max_interval: f64,
    #[serde(default = "leech_threshold_default")]
    leech_threshold: i64,
    #[serde(default = "forecast_reviews_default")]
    forecast_reviews: i64,
    #[serde(default = "forecast_grade_default")]
    forecast_grade: String,
    #[serde(default)]
    fsrs_weights: String,
}

/// Single source for the chat schema, the CLI and the page form. Field ORDER here is
/// the page-form order and the `web::run()` argument order — keep the three aligned.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("reviews").required().multiline().describe(
            "Review log, one row per review: card, date (YYYY-MM-DD), grade. Rows may be \
             comma, tab, semicolon, pipe or space separated, may carry optional trailing \
             state fields (reps=, ease=, interval=, lapses=, difficulty=, stability=, \
             last=), and a row holding only a card name declares a new, unreviewed card. \
             A leading header row and lines starting with # are ignored.",
        ))
        .param(
            Param::enumv("algorithm", ["sm2", "fsrs"])
                .default("sm2")
                .describe(
                    "Scheduling engine: sm2 is the classic SuperMemo-2 ease/interval state \
                     machine; fsrs is an FSRS-style difficulty/stability/retrievability memory \
                     model driven by desired_retention.",
                ),
        )
        .param(
            Param::enumv("grade_scale", ["auto", "sm2", "anki"])
                .default("auto")
                .describe(
                    "How numeric grades are read: sm2 is the 0-5 recall-quality scale, anki is \
                     the four-button 1-4 scale, auto picks sm2 when any 0 or 5 appears and anki \
                     otherwise. Words (again/hard/good/easy and aliases) work in every scale.",
                ),
        )
        .param(Param::string("today").placeholder("2026-08-24").describe(
            "Reference date (YYYY-MM-DD) used for due/overdue status and days_until. Blank \
             uses the latest review date in the log, which keeps runs reproducible.",
        ))
        .param(
            Param::enumv("output", ["table", "csv", "json", "explain", "forecast"])
                .default("table")
                .describe(
                    "Result format: table is an aligned schedule, csv the same columns as CSV, \
                     json a structured document, explain a per-review state trace, forecast a \
                     projection of the next forecast_reviews reviews.",
                ),
        )
        .param(
            Param::enumv("sort", ["due", "card", "interval", "lapses", "input"])
                .default("due")
                .describe(
                    "Row order: due date ascending, card name, interval ascending, lapses \
                     descending, or the order cards first appear in the log. Ties break on \
                     card name.",
                ),
        )
        .param(
            Param::boolean("only_due")
                .default(false)
                .describe("Keep only cards whose due date is on or before today."),
        )
        .param(
            Param::number("desired_retention")
                .default(0.9)
                .min(0.7)
                .max(0.99)
                .describe(
                    "FSRS only: probability of recall to schedule for. Higher means shorter \
                     intervals and more reviews.",
                ),
        )
        .param(
            Param::number("ease_start")
                .default(2.5)
                .min(1.0)
                .max(10.0)
                .describe("SM-2 only: starting ease factor for a card with no history."),
        )
        .param(
            Param::number("min_ease")
                .default(1.3)
                .min(1.0)
                .max(10.0)
                .describe("SM-2 only: floor the ease factor is never allowed to fall below."),
        )
        .param(
            Param::number("first_interval")
                .default(1.0)
                .min(1.0)
                .max(365.0)
                .describe("SM-2 only: days scheduled after the first successful review."),
        )
        .param(
            Param::number("second_interval")
                .default(6.0)
                .min(1.0)
                .max(3650.0)
                .describe("SM-2 only: days scheduled after the second successful review."),
        )
        .param(
            Param::number("easy_bonus")
                .default(1.3)
                .min(1.0)
                .max(10.0)
                .describe("SM-2 only: extra multiplier applied when the top grade is used."),
        )
        .param(
            Param::number("hard_multiplier")
                .default(1.2)
                .min(0.1)
                .max(10.0)
                .describe(
                    "SM-2 only: multiplier used instead of the ease factor when the card is \
                     passed with the hard grade.",
                ),
        )
        .param(
            Param::number("interval_modifier")
                .default(1.0)
                .min(0.1)
                .max(10.0)
                .describe(
                    "Global multiplier applied to every computed interval before rounding, in \
                     both engines. Below 1 compresses the schedule, above 1 stretches it.",
                ),
        )
        .param(
            Param::number("lapse_multiplier")
                .default(0.0)
                .min(0.0)
                .max(1.0)
                .describe(
                    "SM-2 only: share of the old interval kept after a lapse. 0 restarts at \
                     first_interval, the classic SM-2 behaviour.",
                ),
        )
        .param(
            Param::number("max_interval")
                .default(36500.0)
                .min(1.0)
                .max(36500.0)
                .describe("Cap, in days, on any scheduled interval in both engines."),
        )
        .param(
            Param::integer("leech_threshold")
                .default(8)
                .min(0.0)
                .max(1000.0)
                .describe(
                    "Lapse count at which a card is reported with status leech. 0 turns leech \
                     flagging off.",
                ),
        )
        .param(
            Param::integer("forecast_reviews")
                .default(6)
                .min(1.0)
                .max(50.0)
                .describe("Number of future reviews to project when output is forecast."),
        )
        .param(
            Param::enumv("forecast_grade", ["again", "hard", "good", "easy"])
                .default("good")
                .describe("Grade assumed on every projected review when output is forecast."),
        )
        .param(Param::string("fsrs_weights").placeholder("0.2172, 1.1771, 3.2602, …").describe(
            "FSRS only: 21 comma or space separated weights from your own optimiser. Blank \
             uses the built-in default vector.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SpacedRepetitionScheduler;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spaced-repetition-scheduler",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute next review dates from a card review log with SM-2 or FSRS",
    skill(
        description = "Replay a batch flashcard review log (one row per review: card, date, grade) and report every card's scheduler state and next review date. Two engines: sm2 (SuperMemo-2 repetitions, ease factor and interval, with configurable starting ease, ease floor, first/second interval, easy bonus, hard multiplier, interval modifier, lapse multiplier and interval cap) and fsrs (an FSRS-style difficulty/stability/retrievability memory model scheduled to a desired retention, with an optional custom 21-weight vector). Grades accept again/hard/good/easy words and aliases as well as the SuperMemo 0-5 and four-button 1-4 numeric scales. Output is an aligned table, CSV, JSON, a per-review explain trace, or a forecast of upcoming reviews; rows can be sorted and filtered to what is due.",
        parameters = schema_json()
    ),
)]
impl SpacedRepetitionScheduler {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "spaced-repetition-scheduler", |a: Args| {
            gizza_ai_spaced_repetition_scheduler_core::schedule(
                &a.reviews,
                &a.algorithm,
                &a.grade_scale,
                &a.today,
                &a.output,
                &a.sort,
                a.only_due,
                a.desired_retention,
                a.ease_start,
                a.min_ease,
                a.first_interval,
                a.second_interval,
                a.easy_bonus,
                a.hard_multiplier,
                a.interval_modifier,
                a.lapse_multiplier,
                a.max_interval,
                a.leech_threshold,
                a.forecast_reviews,
                &a.forecast_grade,
                &a.fsrs_weights,
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
                    "reviews":           { "type": "string", "description": "Review log, one row per review: card, date (YYYY-MM-DD), grade. Rows may be comma, tab, semicolon, pipe or space separated, may carry optional trailing state fields (reps=, ease=, interval=, lapses=, difficulty=, stability=, last=), and a row holding only a card name declares a new, unreviewed card. A leading header row and lines starting with # are ignored." },
                    "algorithm":         { "type": "string", "enum": ["sm2", "fsrs"], "default": "sm2", "description": "Scheduling engine: sm2 is the classic SuperMemo-2 ease/interval state machine; fsrs is an FSRS-style difficulty/stability/retrievability memory model driven by desired_retention." },
                    "grade_scale":       { "type": "string", "enum": ["auto", "sm2", "anki"], "default": "auto", "description": "How numeric grades are read: sm2 is the 0-5 recall-quality scale, anki is the four-button 1-4 scale, auto picks sm2 when any 0 or 5 appears and anki otherwise. Words (again/hard/good/easy and aliases) work in every scale." },
                    "today":             { "type": "string", "description": "Reference date (YYYY-MM-DD) used for due/overdue status and days_until. Blank uses the latest review date in the log, which keeps runs reproducible." },
                    "output":            { "type": "string", "enum": ["table", "csv", "json", "explain", "forecast"], "default": "table", "description": "Result format: table is an aligned schedule, csv the same columns as CSV, json a structured document, explain a per-review state trace, forecast a projection of the next forecast_reviews reviews." },
                    "sort":              { "type": "string", "enum": ["due", "card", "interval", "lapses", "input"], "default": "due", "description": "Row order: due date ascending, card name, interval ascending, lapses descending, or the order cards first appear in the log. Ties break on card name." },
                    "only_due":          { "type": "boolean", "default": false, "description": "Keep only cards whose due date is on or before today." },
                    "desired_retention": { "type": "number", "minimum": 0.7, "maximum": 0.99, "default": 0.9, "description": "FSRS only: probability of recall to schedule for. Higher means shorter intervals and more reviews." },
                    "ease_start":        { "type": "number", "minimum": 1, "maximum": 10, "default": 2.5, "description": "SM-2 only: starting ease factor for a card with no history." },
                    "min_ease":          { "type": "number", "minimum": 1, "maximum": 10, "default": 1.3, "description": "SM-2 only: floor the ease factor is never allowed to fall below." },
                    "first_interval":    { "type": "number", "minimum": 1, "maximum": 365, "default": 1.0, "description": "SM-2 only: days scheduled after the first successful review." },
                    "second_interval":   { "type": "number", "minimum": 1, "maximum": 3650, "default": 6.0, "description": "SM-2 only: days scheduled after the second successful review." },
                    "easy_bonus":        { "type": "number", "minimum": 1, "maximum": 10, "default": 1.3, "description": "SM-2 only: extra multiplier applied when the top grade is used." },
                    "hard_multiplier":   { "type": "number", "minimum": 0.1, "maximum": 10, "default": 1.2, "description": "SM-2 only: multiplier used instead of the ease factor when the card is passed with the hard grade." },
                    "interval_modifier": { "type": "number", "minimum": 0.1, "maximum": 10, "default": 1.0, "description": "Global multiplier applied to every computed interval before rounding, in both engines. Below 1 compresses the schedule, above 1 stretches it." },
                    "lapse_multiplier":  { "type": "number", "minimum": 0, "maximum": 1, "default": 0.0, "description": "SM-2 only: share of the old interval kept after a lapse. 0 restarts at first_interval, the classic SM-2 behaviour." },
                    "max_interval":      { "type": "number", "minimum": 1, "maximum": 36500, "default": 36500.0, "description": "Cap, in days, on any scheduled interval in both engines." },
                    "leech_threshold":   { "type": "integer", "minimum": 0, "maximum": 1000, "default": 8, "description": "Lapse count at which a card is reported with status leech. 0 turns leech flagging off." },
                    "forecast_reviews":  { "type": "integer", "minimum": 1, "maximum": 50, "default": 6, "description": "Number of future reviews to project when output is forecast." },
                    "forecast_grade":    { "type": "string", "enum": ["again", "hard", "good", "easy"], "default": "good", "description": "Grade assumed on every projected review when output is forecast." },
                    "fsrs_weights":      { "type": "string", "description": "FSRS only: 21 comma or space separated weights from your own optimiser. Blank uses the built-in default vector." }
                },
                "required": ["reviews"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn descriptor_order_matches_the_web_export_and_page_form() {
        // page/meta.toml [[input]] order and web::run()'s argument order are BOTH this
        // list — a reorder here without touching those two silently mis-binds the form.
        let names: Vec<String> = descriptor().params.iter().map(|p| p.name.clone()).collect();
        assert_eq!(
            names,
            vec![
                "reviews",
                "algorithm",
                "grade_scale",
                "today",
                "output",
                "sort",
                "only_due",
                "desired_retention",
                "ease_start",
                "min_ease",
                "first_interval",
                "second_interval",
                "easy_bonus",
                "hard_multiplier",
                "interval_modifier",
                "lapse_multiplier",
                "max_interval",
                "leech_threshold",
                "forecast_reviews",
                "forecast_grade",
                "fsrs_weights",
            ]
        );
    }

    #[test]
    fn every_param_is_described() {
        for p in descriptor().params {
            assert!(!p.description.is_empty(), "{} has no .describe()", p.name);
        }
    }
}
