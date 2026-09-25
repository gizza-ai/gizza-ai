//! gizza-ai/freelance-rate-calc — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    target_income: f64,
    #[serde(default)]
    business_expenses: f64,
    #[serde(default)]
    health_insurance: f64,
    #[serde(default)]
    retirement: f64,
    #[serde(default = "default_tax_rate")]
    tax_rate: f64,
    #[serde(default = "default_tax_basis")]
    tax_basis: String,
    #[serde(default = "default_hours_per_week")]
    hours_per_week: f64,
    #[serde(default = "default_days_per_week")]
    days_per_week: f64,
    #[serde(default = "default_hours_per_day")]
    hours_per_day: f64,
    #[serde(default = "default_weeks_per_year")]
    weeks_per_year: f64,
    #[serde(default = "default_vacation_weeks")]
    vacation_weeks: f64,
    #[serde(default = "default_holidays")]
    holidays: f64,
    #[serde(default = "default_sick_days")]
    sick_days: f64,
    #[serde(default = "default_billable_percent")]
    billable_percent: f64,
    #[serde(default)]
    buffer_percent: f64,
    #[serde(default)]
    current_rate: f64,
    #[serde(default)]
    project_hours: f64,
    #[serde(default = "default_complexity")]
    complexity: String,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default = "default_decimals")]
    decimals: f64,
    #[serde(default = "default_format")]
    format: String,
}

fn default_tax_rate() -> f64 {
    30.0
}
fn default_tax_basis() -> String {
    "income_only".into()
}
fn default_hours_per_week() -> f64 {
    40.0
}
fn default_days_per_week() -> f64 {
    5.0
}
fn default_hours_per_day() -> f64 {
    8.0
}
fn default_weeks_per_year() -> f64 {
    52.0
}
fn default_vacation_weeks() -> f64 {
    4.0
}
fn default_holidays() -> f64 {
    10.0
}
fn default_sick_days() -> f64 {
    5.0
}
fn default_billable_percent() -> f64 {
    70.0
}
fn default_complexity() -> String {
    "standard".into()
}
fn default_currency() -> String {
    "$".into()
}
fn default_decimals() -> f64 {
    2.0
}
fn default_format() -> String {
    "markdown".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::number("target_income").required().min(0.01).describe(
            "Annual take-home income goal after taxes and business costs, in your chosen currency. Example: 80000 means you want 80,000 left for personal income over the year.",
        ))
        .param(Param::number("business_expenses").min(0.0).default(0.0).describe(
            "Annual business overhead before owner pay: software, equipment, bookkeeping, coworking, insurance, marketing and similar costs. Default 0.",
        ))
        .param(Param::number("health_insurance").min(0.0).default(0.0).describe(
            "Annual health-insurance or benefits cost you need the business to cover separately from ordinary overhead. Default 0.",
        ))
        .param(Param::number("retirement").min(0.0).default(0.0).describe(
            "Annual retirement, pension or savings contribution you want the rate to fund before take-home pay. Default 0.",
        ))
        .param(Param::number("tax_rate").min(0.0).max(95.0).default(30.0).describe(
            "Effective income-tax percentage on profit, 0 to 95. Use a blended estimate for federal/state/local tax. Default 30.",
        ))
        .param(Param::enumv("tax_basis", ["income_only", "self_employment"]).default("income_only").describe(
            "Tax model. 'income_only' uses only tax_rate. 'self_employment' adds the US self-employment layer (15.3% assessed on 92.35% of net profit) on top of tax_rate.",
        ))
        .param(Param::number("hours_per_week").min(0.1).max(168.0).default(40.0).describe(
            "Hours you expect to work in an ordinary week before non-billable time is removed. Default 40.",
        ))
        .param(Param::number("days_per_week").min(0.1).max(7.0).default(5.0).describe(
            "Working days in a normal week. Holidays and sick days are converted to weeks using this value. Default 5.",
        ))
        .param(Param::number("hours_per_day").min(0.1).max(24.0).default(8.0).describe(
            "Hours in the day-rate basis. Day rate = hourly rate × this value. Default 8.",
        ))
        .param(Param::number("weeks_per_year").min(1.0).max(53.0).default(52.0).describe(
            "Calendar weeks in the year used for the time budget. Default 52.",
        ))
        .param(Param::number("vacation_weeks").min(0.0).max(52.0).default(4.0).describe(
            "Unpaid vacation or planned time off in weeks. Default 4.",
        ))
        .param(Param::number("holidays").min(0.0).max(366.0).default(10.0).describe(
            "Public holidays or company-closed days per year. Default 10.",
        ))
        .param(Param::number("sick_days").min(0.0).max(366.0).default(5.0).describe(
            "Sick, admin or contingency days per year to remove from the billable calendar. Default 5.",
        ))
        .param(Param::number("billable_percent").min(1.0).max(100.0).default(70.0).describe(
            "Percent of worked hours you can invoice after sales, admin, learning and downtime. Default 70.",
        ))
        .param(Param::number("buffer_percent").min(0.0).max(200.0).default(0.0).describe(
            "Markup added to the final computed rate for slow months, risk or profit buffer. Default 0.",
        ))
        .param(Param::number("current_rate").min(0.0).default(0.0).describe(
            "Optional hourly rate you already charge. When greater than 0, the report compares its annual revenue/take-home against the required rate. Default 0 = off.",
        ))
        .param(Param::number("project_hours").min(0.0).max(100000.0).default(0.0).describe(
            "Optional estimated project hours. When greater than 0, the report adds a fixed-price quote and 30/40/30 milestone split. Default 0 = off.",
        ))
        .param(Param::enumv("complexity", ["simple", "standard", "complex", "rush"]).default("standard").describe(
            "Multiplier for the optional project quote: simple 0.85×, standard 1×, complex 1.5×, rush 2×.",
        ))
        .param(Param::string("currency").default("$").describe(
            "Currency prefix or symbol to display, such as '$', '€', '£' or 'CHF '. It labels amounts only; no exchange rates are fetched.",
        ))
        .param(Param::integer("decimals").min(0.0).max(6.0).default(2).describe(
            "Decimal places for money, hours and rates, 0 to 6. Default 2.",
        ))
        .param(Param::enumv("format", ["markdown", "text", "csv", "json"]).default("markdown").describe(
            "Output format: markdown tables (default), aligned text, CSV rows, or JSON.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/freelance-rate-calc",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute freelance hourly and day rates from income goals, taxes, expenses and billable time",
    skill(
        description = "Work backwards from an annual take-home income goal to the hourly, day, weekly and monthly freelance rates you need to charge. Add business expenses, health insurance, retirement contributions, an effective tax rate, an optional US self-employment tax layer, vacation, holidays, sick days, billable percentage and a slow-month buffer. The report shows the gross-up math, required annual revenue, working weeks/days/hours, billable hours, rate per worked hour, optional comparison to a current rate, and an optional fixed-price project quote with simple/standard/complex/rush multipliers and a 30/40/30 milestone split. Outputs markdown, text, CSV or JSON and runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "freelance-rate-calc", |a: Args| {
            gizza_ai_freelance_rate_calc_core::run(
                a.target_income,
                a.business_expenses,
                a.health_insurance,
                a.retirement,
                a.tax_rate,
                &a.tax_basis,
                a.hours_per_week,
                a.days_per_week,
                a.hours_per_day,
                a.weeks_per_year,
                a.vacation_weeks,
                a.holidays,
                a.sick_days,
                a.billable_percent,
                a.buffer_percent,
                a.current_rate,
                a.project_hours,
                &a.complexity,
                &a.currency,
                a.decimals,
                &a.format,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
