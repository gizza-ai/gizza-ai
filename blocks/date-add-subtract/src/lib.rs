//! gizza-ai/date-add-subtract — add or subtract calendar/business durations.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use chrono::{Datelike, Utc};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_date_add_subtract_core::{shift_json, Inputs};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Default)]
struct Args {
    #[serde(default)]
    date: String,
    #[serde(default = "d_operation")]
    operation: String,
    years: Option<f64>,
    months: Option<f64>,
    weeks: Option<f64>,
    days: Option<f64>,
    hours: Option<f64>,
    minutes: Option<f64>,
    seconds: Option<f64>,
    #[serde(default)]
    skip_weekends: bool,
    #[serde(default)]
    weekend_days: String,
    #[serde(default)]
    holidays: String,
}

fn d_operation() -> String {
    "add".into()
}

impl From<Args> for Inputs {
    fn from(a: Args) -> Self {
        Inputs {
            date: a.date,
            operation: a.operation,
            years: a.years,
            months: a.months,
            weeks: a.weeks,
            days: a.days,
            hours: a.hours,
            minutes: a.minutes,
            seconds: a.seconds,
            skip_weekends: Some(a.skip_weekends),
            weekend_days: a.weekend_days,
            holidays: a.holidays,
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("date").required().describe("Required. Start date or datetime. Accepts YYYY-MM-DD, YYYY-MM-DDTHH:MM[:SS], YYYY/MM/DD, MM/DD/YYYY, DD.MM.YYYY, month-name forms, or today/tomorrow/yesterday."))
        .param(Param::enumv("operation", ["add", "subtract"]).default("add").describe("Whether to add the duration to the start date or subtract it."))
        .param(Param::number("years").default(0.0).describe("Whole calendar years to shift. Years and months are applied first and clamp the day-of-month when needed."))
        .param(Param::number("months").default(0.0).describe("Whole calendar months to shift after years. Jan 31 plus one month becomes the last day of February."))
        .param(Param::number("weeks").default(0.0).describe("Whole weeks to shift. In business-day mode, each week means seven working-day steps."))
        .param(Param::number("days").default(0.0).describe("Whole days to shift. With skip_weekends or holidays, these are working-day steps."))
        .param(Param::number("hours").default(0.0).describe("Whole hours to shift after calendar date math."))
        .param(Param::number("minutes").default(0.0).describe("Whole minutes to shift after calendar date math."))
        .param(Param::number("seconds").default(0.0).describe("Whole seconds to shift after calendar date math."))
        .param(Param::boolean("skip_weekends").default(false).describe("When true, day and week steps count only Monday-Friday, and a result landing on a weekend rolls in the direction of travel."))
        .param(Param::enumv("weekend_days", ["sat-sun", "fri-sat", "thu-fri", "sun-only", "fri-only", "none"]).default("sat-sun").describe("Which days count as the weekend when skip_weekends is enabled. Use sat-sun for the common Saturday/Sunday weekend, fri-sat for Gulf schedules, or none to skip only listed holidays."))
        .param(Param::string("holidays").describe("Optional comma, semicolon, or newline separated holiday dates to skip in business-day mode. Use YYYY-MM-DD or another supported date-only format."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn today_utc() -> chrono::NaiveDate {
    let now = Utc::now().date_naive();
    chrono::NaiveDate::from_ymd_opt(now.year(), now.month(), now.day()).unwrap()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/date-add-subtract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add or subtract calendar and business-day durations from a date.",
    skill(
        description = "Add or subtract years, months, weeks, days, hours, minutes and seconds from a date or datetime. Supports calendar math with month-end clamping plus business-day mode that skips configurable weekend days and optional holiday dates. Parameters: date (required), operation add|subtract, years/months/weeks/days/hours/minutes/seconds (whole numbers), skip_weekends, weekend_days, and holidays. Returns JSON with the normalized result date/time, weekday, ISO week, day-of-year, moved-day counts, skipped days and a human summary.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "date-add-subtract", |a: Args| {
            shift_json(&Inputs::from(a), today_utc()).map_err(SkillError::InvalidArgs)
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
    fn args_defaults_match_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"date":"2026-06-19"}"#).unwrap();
        assert_eq!(a.operation, "add");
        assert_eq!(a.years, None);
        assert!(!a.skip_weekends);
    }

    #[test]
    fn args_flow_into_core_inputs() {
        let a: Args = serde_json::from_str(r#"{"date":"2026-06-19","operation":"subtract","months":1,"days":2,"skip_weekends":true,"weekend_days":"fri-sat","holidays":"2026-06-18"}"#).unwrap();
        let i = Inputs::from(a);
        assert_eq!(i.operation, "subtract");
        assert_eq!(i.months, Some(1.0));
        assert_eq!(i.days, Some(2.0));
        assert_eq!(i.skip_weekends, Some(true));
        assert_eq!(i.weekend_days, "fri-sat");
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["required"], serde_json::json!(["date"]));
        assert_eq!(
            v["properties"]["operation"]["enum"],
            serde_json::json!(["add", "subtract"])
        );
        assert_eq!(v["properties"]["skip_weekends"]["default"], false);
        assert_eq!(v["properties"]["weekend_days"]["default"], "sat-sun");
    }
}
