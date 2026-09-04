//! gizza-ai/nps-csat-calculator — chat skill block on the shared tool abstraction.
//!
//! Turns a column of survey ratings into NPS, CSAT or CES: the headline score, the
//! promoter/passive/detractor (or satisfied/neutral/dissatisfied) breakdown, the
//! rating distribution and a confidence band. The chat schema is single-sourced
//! from `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ratings: String,
    #[serde(default)]
    metric: String,
    #[serde(default)]
    input: String,
    #[serde(default)]
    scale: String,
    #[serde(default = "auto_cutoff")]
    threshold: i64,
    #[serde(default)]
    confidence: String,
    #[serde(default = "one")]
    decimals: i64,
    #[serde(default = "yes")]
    distribution: bool,
    #[serde(default)]
    format: String,
}

fn auto_cutoff() -> i64 {
    -1
}
fn one() -> i64 {
    1
}
fn yes() -> bool {
    true
}

/// Single source for the chat schema (and CLI). `ratings` is required; every
/// option falls back to the documented default so a bare 0-10 column returns NPS.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("ratings").required().describe(
                "The survey ratings, one per cell — newline, comma, semicolon, tab or space \
                 separated (paste a spreadsheet column straight in). A leading header line that \
                 holds no numbers is skipped; blank cells and NA/N-A/-/./none/null/missing/? are \
                 counted as skipped. With input=counts each line is instead 'rating,count', e.g. \
                 '10,42'. Up to 100000 responses per run.",
            ),
        )
        .param(
            Param::enumv("metric", ["nps", "csat", "ces"])
                .default("nps")
                .describe(
                    "Which metric to compute: nps = Net Promoter Score, promoters (9-10) minus \
                     detractors (0-6) as a share of all responses, in points from -100 to +100; \
                     csat = Customer Satisfaction Score, the percentage of responses at or above \
                     the satisfied cut-off; ces = Customer Effort Score, the mean rating plus the \
                     share at or above the easy cut-off. Default nps.",
                ),
        )
        .param(
            Param::enumv("input", ["values", "counts"])
                .default("values")
                .describe(
                    "Shape of the ratings: values = one rating per cell (the raw column); counts \
                     = one 'rating,count' row per scale point, for data you have already tallied. \
                     Default values.",
                ),
        )
        .param(
            Param::enumv("scale", ["auto", "0-10", "1-5", "1-7", "1-10"])
                .default("auto")
                .describe(
                    "Rating scale the responses use. auto picks the convention for the metric: \
                     0-10 for nps, 1-5 for csat, 1-7 for ces. NPS accepts 0-10 only. Any rating \
                     outside the scale is an error. Default auto.",
                ),
        )
        .param(
            Param::integer("threshold")
                .min(-1.0)
                .max(100.0)
                .default(-1)
                .describe(
                    "Lowest rating that counts as satisfied (csat) or easy (ces). -1 = automatic \
                     top-2 box: 4+ on 1-5, 9+ on 0-10 and 1-10, and 5+ on 1-7 (the usual top-3 \
                     cut-off there). The rating just below it is the neutral band and everything \
                     under that is the bottom band. Ignored for nps, whose bands are fixed. \
                     Default -1.",
                ),
        )
        .param(
            Param::enumv("confidence", ["95", "90", "99", "none"])
                .default("95")
                .describe(
                    "Confidence level for the band around the score, as a normal approximation: \
                     the standard error of promoters minus detractors for nps, a proportion \
                     interval for csat, and a mean interval for ces. none omits the band. Needs at \
                     least 2 responses. Default 95.",
                ),
        )
        .param(
            Param::integer("decimals")
                .min(0.0)
                .max(6.0)
                .default(1)
                .describe(
                    "Decimal places for the score, mean, standard deviation, confidence band and \
                     percentages. Counts are always whole numbers. Default 1.",
                ),
        )
        .param(Param::boolean("distribution").default(true).describe(
            "Include the per-rating distribution — every point on the scale with its count, \
             percentage and a text bar. Default true.",
        ))
        .param(
            Param::enumv("format", ["report", "json", "csv"])
                .default("report")
                .describe(
                    "Output shape: report = a monospaced summary with the breakdown and \
                     distribution; json = a machine-readable object; csv = a long \
                     section,label,value,percent table. Default report.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

impl Args {
    fn calculate(&self) -> Result<String, String> {
        gizza_ai_nps_csat_calculator_core::calculate(
            &self.ratings,
            &self.metric,
            &self.input,
            &self.scale,
            self.threshold,
            &self.confidence,
            self.decimals,
            self.distribution,
            &self.format,
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/nps-csat-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute NPS, CSAT or CES from a column of survey ratings, with breakdown and confidence band.",
    skill(
        description = "Compute a customer-experience metric from a column of survey ratings. metric=nps (default) returns the Net Promoter Score in points from -100 to +100 on the fixed 0-10 scale; metric=csat returns the percentage of responses at or above the satisfied cut-off; metric=ces returns the mean effort score plus the share at or above the easy cut-off. Every run reports the three-band breakdown (promoters 9-10 / passives 7-8 / detractors 0-6, or satisfied / neutral / dissatisfied), counts and percentages, the mean and standard deviation, a rating tier, the full per-rating distribution, and a normal-approximation confidence band (95% by default). Ratings can be pasted raw (input=values, newline/comma/tab separated, header line and NA markers skipped) or pre-tallied as 'rating,count' rows (input=counts). Options: scale (auto/0-10/1-5/1-7/1-10), threshold for the satisfied/easy cut-off, confidence (95/90/99/none), decimals, distribution, and format (report/json/csv).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "nps-csat-calculator", |a: Args| {
            a.calculate().map_err(SkillError::InvalidArgs)
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
                    "ratings": { "type": "string", "description": "The survey ratings, one per cell — newline, comma, semicolon, tab or space separated (paste a spreadsheet column straight in). A leading header line that holds no numbers is skipped; blank cells and NA/N-A/-/./none/null/missing/? are counted as skipped. With input=counts each line is instead 'rating,count', e.g. '10,42'. Up to 100000 responses per run." },
                    "metric": { "type": "string", "enum": ["nps","csat","ces"], "default": "nps", "description": "Which metric to compute: nps = Net Promoter Score, promoters (9-10) minus detractors (0-6) as a share of all responses, in points from -100 to +100; csat = Customer Satisfaction Score, the percentage of responses at or above the satisfied cut-off; ces = Customer Effort Score, the mean rating plus the share at or above the easy cut-off. Default nps." },
                    "input": { "type": "string", "enum": ["values","counts"], "default": "values", "description": "Shape of the ratings: values = one rating per cell (the raw column); counts = one 'rating,count' row per scale point, for data you have already tallied. Default values." },
                    "scale": { "type": "string", "enum": ["auto","0-10","1-5","1-7","1-10"], "default": "auto", "description": "Rating scale the responses use. auto picks the convention for the metric: 0-10 for nps, 1-5 for csat, 1-7 for ces. NPS accepts 0-10 only. Any rating outside the scale is an error. Default auto." },
                    "threshold": { "type": "integer", "minimum": -1, "maximum": 100, "default": -1, "description": "Lowest rating that counts as satisfied (csat) or easy (ces). -1 = automatic top-2 box: 4+ on 1-5, 9+ on 0-10 and 1-10, and 5+ on 1-7 (the usual top-3 cut-off there). The rating just below it is the neutral band and everything under that is the bottom band. Ignored for nps, whose bands are fixed. Default -1." },
                    "confidence": { "type": "string", "enum": ["95","90","99","none"], "default": "95", "description": "Confidence level for the band around the score, as a normal approximation: the standard error of promoters minus detractors for nps, a proportion interval for csat, and a mean interval for ces. none omits the band. Needs at least 2 responses. Default 95." },
                    "decimals": { "type": "integer", "minimum": 0, "maximum": 6, "default": 1, "description": "Decimal places for the score, mean, standard deviation, confidence band and percentages. Counts are always whole numbers. Default 1." },
                    "distribution": { "type": "boolean", "default": true, "description": "Include the per-rating distribution — every point on the scale with its count, percentage and a text bar. Default true." },
                    "format": { "type": "string", "enum": ["report","json","csv"], "default": "report", "description": "Output shape: report = a monospaced summary with the breakdown and distribution; json = a machine-readable object; csv = a long section,label,value,percent table. Default report." }
                },
                "required": ["ratings"],
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
        let a: Args = serde_json::from_str(r#"{"ratings":"10,9,8,7,0"}"#).unwrap();
        assert_eq!(a.threshold, -1);
        assert_eq!(a.decimals, 1);
        assert!(a.distribution);
        let out = a.calculate().unwrap();
        assert!(out.starts_with("Net Promoter Score (NPS)"), "{out}");
        assert!(out.contains("NPS                       20.0"), "{out}");
    }

    /// A bad argument surfaces as an invalid-args error, not a panic.
    #[test]
    fn out_of_scale_rating_is_an_error() {
        let a: Args = serde_json::from_str(r#"{"ratings":"11"}"#).unwrap();
        let e = a.calculate().unwrap_err();
        assert!(e.contains("outside the 0-10 scale"), "{e}");
    }
}
