//! gizza-ai/grade-to-gpa-parser — parse a pasted list of letter grades, map each
//! one to GPA points on a configurable scale and average them, credit-weighted.
//! Thin wrapper; chat schema single-sourced from descriptor(); handler delegates
//! to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_grade_to_gpa_parser_core::{summary, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    grades: String,
    #[serde(default = "default_scale")]
    scale: String,
    #[serde(default)]
    custom_scale: String,
    #[serde(default = "default_grade_format")]
    grade_format: String,
    #[serde(default = "default_credits")]
    default_credits: f64,
    #[serde(default = "default_honors_bonus")]
    honors_bonus: f64,
    #[serde(default = "default_ap_bonus")]
    ap_bonus: f64,
    #[serde(default)]
    prior_gpa: f64,
    #[serde(default)]
    prior_credits: f64,
    #[serde(default = "default_true")]
    skip_nongraded: bool,
    #[serde(default = "default_decimals")]
    decimals: i64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_scale() -> String {
    "4.0".to_string()
}
fn default_grade_format() -> String {
    "auto".to_string()
}
fn default_credits() -> f64 {
    1.0
}
fn default_honors_bonus() -> f64 {
    0.5
}
fn default_ap_bonus() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_decimals() -> i64 {
    2
}
fn default_output() -> String {
    "report".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("grades").required().describe(
                "The grades to convert, one course per entry, separated by newlines, semicolons or commas. An entry is an optional course name, the grade, and optional credit hours: 'A', 'A- 4', 'Biology: A- 4', 'AP History: B+ (3)'. A grade is a letter (A+ through F, with E treated as F), a percentage, or raw grade points. Credits default to default_credits when an entry omits them, so a bare list like 'A, B+, C-' is a plain unweighted average. Honours/AP weighting is triggered by an honors, hon, ap, ib, de, dual or college token anywhere in the entry. Maximum 2000 entries.",
            ),
        )
        .param(
            Param::enumv("scale", ["4.0", "4.3", "5.0"])
                .default("4.0")
                .describe(
                    "The base letter-to-points table. 4.0 (default, the standard US unweighted scale): A+ and A = 4.0, A- 3.7, B+ 3.3, B 3.0, B- 2.7, C+ 2.3, C 2.0, C- 1.7, D+ 1.3, D 1.0, D- 0.7, F 0. 4.3: the same but A+ = 4.3. 5.0: the honours/AP table where A = 5.0, A- 4.7, B+ 4.3, B 4.0 and so on down to F = 0. Use custom_scale to change individual grades or add letters your school uses.",
                ),
        )
        .param(
            Param::string("custom_scale").default("").describe(
                "Optional LETTER=POINTS overrides merged on top of scale, separated by commas or newlines, e.g. 'A+=4.5, HD=4.0, P=2.0'. A letter already on the scale is replaced; anything else is added, which is how you support non-US grades (HD/D/CR, 1.0-5.0 systems, and so on). Empty by default.",
            ),
        )
        .param(
            Param::enumv("grade_format", ["auto", "letter", "percent", "points"])
                .default("auto")
                .describe(
                    "How to read a grade written as a number. auto (default): a number larger than the top of the scale is a percentage, anything at or below it is already grade points, so '92' is a percentage on a 4.0 scale and '3.7' is not. letter: reject numbers and require letter grades. percent: always convert through the percentage bands (97+ A+, 93 A, 90 A-, 87 B+, 83 B, 80 B-, 77 C+, 73 C, 70 C-, 67 D+, 63 D, 60 D-, below 60 F). points: always take the number as grade points as-is.",
                ),
        )
        .param(
            Param::number("default_credits")
                .default(1.0)
                .min(0.0)
                .describe(
                    "Credit hours used for an entry that does not state its own, e.g. 'A, B+, C-'. Must be greater than 0. Default 1, which makes a credit-less list an ordinary unweighted average.",
                ),
        )
        .param(
            Param::number("honors_bonus")
                .default(0.5)
                .min(0.0)
                .describe(
                    "Extra grade points added to a course tagged honors, honor, hon or hnrs, the usual weighted-GPA bump. Default 0.5. Set 0 to ignore honours tags and compute an unweighted GPA. Never applied to a failing grade.",
                ),
        )
        .param(
            Param::number("ap_bonus").default(1.0).min(0.0).describe(
                "Extra grade points added to a course tagged ap, ib, de, dual, college or adv. Default 1.0. Set 0 to ignore those tags. Never applied to a failing grade. A course carrying both an honours and an AP tag gets this one.",
            ),
        )
        .param(
            Param::number("prior_gpa").default(0.0).min(0.0).describe(
                "GPA already on the transcript before this list, used with prior_credits to also report a cumulative GPA. Default 0 (no prior record).",
            ),
        )
        .param(
            Param::number("prior_credits")
                .default(0.0)
                .min(0.0)
                .describe(
                    "Credit hours behind prior_gpa. The cumulative GPA line only appears when this is above 0. Default 0.",
                ),
        )
        .param(
            Param::boolean("skip_nongraded").default(true).describe(
                "Leave non-graded transcript marks (P, NP, S, U, CR, NC, W, WD, I, INC, IP, AU, NG, TR) out of the average and list them separately, which is what schools do. Default true. Set false to error on such a row instead of dropping it silently.",
            ),
        )
        .param(
            Param::integer("decimals")
                .default(2)
                .min(0.0)
                .max(6.0)
                .describe(
                    "Decimal places for the GPA and every other number in the output, 0 to 6. Default 2, the precision transcripts use.",
                ),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe(
                    "report (default): a readable summary with the GPA, the totals, a per-course breakdown, anything not counted and the scale used. json: the same data as a JSON object with gpa, grade_points, credits, courses_counted, scale, courses[], not_counted[] and an optional cumulative block.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Shared argument handling so chat, CLI and the page reject the same things
/// with the same wording.
fn run_args(a: Args) -> Result<String, String> {
    if !(0..=6).contains(&a.decimals) {
        return Err(format!(
            "decimals must be between 0 and 6 (got {})",
            a.decimals
        ));
    }
    summary(
        &a.grades,
        &Options {
            scale: a.scale,
            custom_scale: a.custom_scale,
            grade_format: a.grade_format,
            default_credits: a.default_credits,
            honors_bonus: a.honors_bonus,
            ap_bonus: a.ap_bonus,
            prior_gpa: a.prior_gpa,
            prior_credits: a.prior_credits,
            skip_nongraded: a.skip_nongraded,
            decimals: a.decimals as u32,
            output: a.output,
        },
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/grade-to-gpa-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert letter grades to GPA points on a 4.0, 4.3 or 5.0 scale and average them by credit",
    skill(
        description = "Parse a pasted list of letter grades and turn it into a GPA. Each entry is an optional course name, a grade and optional credit hours ('A', 'A- 4', 'Biology: A- 4', 'AP History: B+ (3)'), separated by newlines, semicolons or commas. Grades map through the 4.0, 4.3 or 5.0 scale, or through LETTER=POINTS overrides in custom_scale, and percentages or raw grade points are accepted too. Credit hours weight the average, honours/AP/IB tags add their weighted bonus, non-graded marks such as P and W are listed but excluded, and prior_gpa with prior_credits adds a cumulative GPA. Returns the GPA, total grade points, total credits and a per-course breakdown as a report or as JSON. Runs locally, no data leaves the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "grade-to-gpa-parser", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
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
                    "grades": { "type": "string", "description": "The grades to convert, one course per entry, separated by newlines, semicolons or commas. An entry is an optional course name, the grade, and optional credit hours: 'A', 'A- 4', 'Biology: A- 4', 'AP History: B+ (3)'. A grade is a letter (A+ through F, with E treated as F), a percentage, or raw grade points. Credits default to default_credits when an entry omits them, so a bare list like 'A, B+, C-' is a plain unweighted average. Honours/AP weighting is triggered by an honors, hon, ap, ib, de, dual or college token anywhere in the entry. Maximum 2000 entries." },
                    "scale": { "type": "string", "enum": ["4.0", "4.3", "5.0"], "default": "4.0", "description": "The base letter-to-points table. 4.0 (default, the standard US unweighted scale): A+ and A = 4.0, A- 3.7, B+ 3.3, B 3.0, B- 2.7, C+ 2.3, C 2.0, C- 1.7, D+ 1.3, D 1.0, D- 0.7, F 0. 4.3: the same but A+ = 4.3. 5.0: the honours/AP table where A = 5.0, A- 4.7, B+ 4.3, B 4.0 and so on down to F = 0. Use custom_scale to change individual grades or add letters your school uses." },
                    "custom_scale": { "type": "string", "default": "", "description": "Optional LETTER=POINTS overrides merged on top of scale, separated by commas or newlines, e.g. 'A+=4.5, HD=4.0, P=2.0'. A letter already on the scale is replaced; anything else is added, which is how you support non-US grades (HD/D/CR, 1.0-5.0 systems, and so on). Empty by default." },
                    "grade_format": { "type": "string", "enum": ["auto", "letter", "percent", "points"], "default": "auto", "description": "How to read a grade written as a number. auto (default): a number larger than the top of the scale is a percentage, anything at or below it is already grade points, so '92' is a percentage on a 4.0 scale and '3.7' is not. letter: reject numbers and require letter grades. percent: always convert through the percentage bands (97+ A+, 93 A, 90 A-, 87 B+, 83 B, 80 B-, 77 C+, 73 C, 70 C-, 67 D+, 63 D, 60 D-, below 60 F). points: always take the number as grade points as-is." },
                    "default_credits": { "type": "number", "default": 1.0, "minimum": 0, "description": "Credit hours used for an entry that does not state its own, e.g. 'A, B+, C-'. Must be greater than 0. Default 1, which makes a credit-less list an ordinary unweighted average." },
                    "honors_bonus": { "type": "number", "default": 0.5, "minimum": 0, "description": "Extra grade points added to a course tagged honors, honor, hon or hnrs, the usual weighted-GPA bump. Default 0.5. Set 0 to ignore honours tags and compute an unweighted GPA. Never applied to a failing grade." },
                    "ap_bonus": { "type": "number", "default": 1.0, "minimum": 0, "description": "Extra grade points added to a course tagged ap, ib, de, dual, college or adv. Default 1.0. Set 0 to ignore those tags. Never applied to a failing grade. A course carrying both an honours and an AP tag gets this one." },
                    "prior_gpa": { "type": "number", "default": 0.0, "minimum": 0, "description": "GPA already on the transcript before this list, used with prior_credits to also report a cumulative GPA. Default 0 (no prior record)." },
                    "prior_credits": { "type": "number", "default": 0.0, "minimum": 0, "description": "Credit hours behind prior_gpa. The cumulative GPA line only appears when this is above 0. Default 0." },
                    "skip_nongraded": { "type": "boolean", "default": true, "description": "Leave non-graded transcript marks (P, NP, S, U, CR, NC, W, WD, I, INC, IP, AU, NG, TR) out of the average and list them separately, which is what schools do. Default true. Set false to error on such a row instead of dropping it silently." },
                    "decimals": { "type": "integer", "default": 2, "minimum": 0, "maximum": 6, "description": "Decimal places for the GPA and every other number in the output, 0 to 6. Default 2, the precision transcripts use." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "report (default): a readable summary with the GPA, the totals, a per-course breakdown, anything not counted and the scale used. json: the same data as a JSON object with gpa, grade_points, credits, courses_counted, scale, courses[], not_counted[] and an optional cumulative block." }
                },
                "required": ["grades"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn defaults_give_an_unweighted_credit_less_average() {
        let a: Args = serde_json::from_str(r#"{"grades":"A, B+, C-"}"#).unwrap();
        let out = run_args(a).unwrap();
        assert!(out.starts_with("GPA: 3.00\n"), "{out}");
        assert!(out.contains("Scale: 4.0 — A+ 4.00"), "{out}");
    }

    #[test]
    fn credits_and_ap_tags_come_through_the_json_args() {
        let a: Args =
            serde_json::from_str(r#"{"grades":"AP Biology: A 4\nMath: B 3","decimals":3}"#).unwrap();
        let out = run_args(a).unwrap();
        // (5.0*4 + 3.0*3) / 7 = 29 / 7 = 4.142857
        assert!(out.starts_with("GPA: 4.143\n"), "{out}");
    }

    #[test]
    fn out_of_range_decimals_is_rejected() {
        let a: Args = serde_json::from_str(r#"{"grades":"A","decimals":9}"#).unwrap();
        let e = run_args(a).unwrap_err();
        assert!(e.contains("decimals must be between 0 and 6"), "{e}");
    }

    #[test]
    fn an_unknown_grade_is_rejected() {
        let a: Args = serde_json::from_str(r#"{"grades":"Biology: Z 4"}"#).unwrap();
        let e = run_args(a).unwrap_err();
        assert!(e.contains("is not a grade on this scale"), "{e}");
    }
}
