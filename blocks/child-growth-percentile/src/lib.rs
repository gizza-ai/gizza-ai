//! gizza-ai/child-growth-percentile — chat skill block on the shared tool abstraction.
//! Computes CDC growth-chart LMS percentiles from age, sex and measurements.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_child_growth_percentile_core::{
    decimals_from, parse_age, report, Chart, Options, Sex, Units,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sex: String,
    age: String,
    #[serde(default)]
    height: f64,
    #[serde(default)]
    weight: f64,
    #[serde(default)]
    head_circumference: f64,
    #[serde(default = "default_units")]
    units: String,
    #[serde(default = "default_chart")]
    chart: String,
    #[serde(default = "default_decimals")]
    decimals: i64,
}

fn default_units() -> String {
    "metric".to_string()
}
fn default_chart() -> String {
    "auto".to_string()
}
fn default_decimals() -> i64 {
    2
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::enumv("sex", ["boy", "girl"]).required().describe("Child sex for the sex-specific CDC reference curve. Use 'boy' or 'girl' (male/female aliases are accepted by the engine, but the schema advertises the two canonical values)."))
        .param(Param::string("age").required().describe("Child age. A bare number means months (36), or write units such as '3y 4m', '18 months', '6 weeks', '95 days', or a date range like '2023-04-15 to 2026-08-23'. Valid range is birth through 240 months."))
        .param(Param::number("height").required().min(0.0).describe("Length/height measurement. In metric units this is centimetres; in US units this is inches. Use 0 only if height was not measured and you provide weight or head_circumference. Infant charts treat this as recumbent length; child charts treat it as standing stature."))
        .param(Param::number("weight").default(0.0).min(0.0).describe("Weight measurement. In metric units this is kilograms; in US units this is pounds. Use 0 when weight was not measured."))
        .param(Param::number("head_circumference").default(0.0).min(0.0).describe("Head circumference. In metric units this is centimetres; in US units this is inches. Use 0 when not measured. CDC head-circumference charts cover birth through 36 months."))
        .param(Param::enumv("units", ["metric", "us"]).default("metric").describe("Measurement units for height, weight and head_circumference. 'metric' means centimetres and kilograms; 'us' means inches and pounds."))
        .param(Param::enumv("chart", ["auto", "infant", "child"]).default("auto").describe("CDC reference set. 'auto' uses infant charts before 24 months and 2-20 year charts from 24 months; 'infant' forces birth-to-36-month length/head references; 'child' forces 2-20-year stature/BMI references."))
        .param(Param::integer("decimals").default(2).min(0.0).max(4.0).describe("Decimal places for reported percentiles and z-scores, from 0 to 4. Default 2."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_convert(a: Args) -> Result<String, SkillError> {
    let opts = Options {
        sex: Sex::parse(&a.sex).map_err(SkillError::InvalidArgs)?,
        age_months: parse_age(&a.age).map_err(SkillError::InvalidArgs)?,
        height: a.height,
        weight: a.weight,
        head_circumference: a.head_circumference,
        units: Units::parse(&a.units).map_err(SkillError::InvalidArgs)?,
        chart: Chart::parse(&a.chart).map_err(SkillError::InvalidArgs)?,
        decimals: decimals_from(a.decimals).map_err(SkillError::InvalidArgs)?,
    };
    report(&opts).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/child-growth-percentile",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute CDC child growth-chart percentiles from age, sex, height, weight and head circumference",
    skill(
        description = "Compute child growth percentiles and z-scores from the bundled CDC LMS growth-chart coefficients. Provide sex, age (months, years/months, days/weeks or a date range), and at least one measurement: height/length, weight or head_circumference. The tool reports height/length-for-age, weight-for-age, BMI-for-age, head-circumference-for-age and weight-for-length/stature when the selected CDC reference covers the child's age and measurements. It supports metric (cm/kg) and US (in/lb) units, auto-selects infant charts before 24 months and 2-20-year charts from 24 months, and can force either chart set. Percentiles are screening references, not diagnoses; growth should be interpreted over time with clinical context.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "child-growth-percentile", run_convert) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_convert_reports_a_metric_child() {
        let out = run_convert(Args {
            sex: "girl".into(),
            age: "3y".into(),
            height: 95.0,
            weight: 14.0,
            head_circumference: 0.0,
            units: "metric".into(),
            chart: "auto".into(),
            decimals: 2,
        })
        .unwrap();
        assert!(
            out.contains("Child: girl, age 3 y 0 mo (36 months)"),
            "{out}"
        );
        assert!(out.contains("BMI category: Healthy weight"), "{out}");
    }

    #[test]
    fn run_convert_rejects_bad_decimals() {
        let err = run_convert(Args {
            sex: "boy".into(),
            age: "12".into(),
            height: 75.0,
            weight: 0.0,
            head_circumference: 0.0,
            units: "metric".into(),
            chart: "auto".into(),
            decimals: 9,
        })
        .unwrap_err();
        assert!(format!("{err}").contains("decimals"));
    }
}
