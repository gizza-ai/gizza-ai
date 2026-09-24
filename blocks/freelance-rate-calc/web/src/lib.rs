//! Browser-facing wasm-bindgen wrapper for /tools/freelance-rate-calc/.
use wasm_bindgen::prelude::*;

fn number(value: &str, default: f64, label: &str) -> Result<f64, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    v.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{label} must be a number")))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    target_income: &str,
    business_expenses: &str,
    health_insurance: &str,
    retirement: &str,
    tax_rate: &str,
    tax_basis: &str,
    hours_per_week: &str,
    days_per_week: &str,
    hours_per_day: &str,
    weeks_per_year: &str,
    vacation_weeks: &str,
    holidays: &str,
    sick_days: &str,
    billable_percent: &str,
    buffer_percent: &str,
    current_rate: &str,
    project_hours: &str,
    complexity: &str,
    currency: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_freelance_rate_calc_core::run(
        number(target_income, f64::NAN, "target income")?,
        number(business_expenses, 0.0, "business expenses")?,
        number(health_insurance, 0.0, "health insurance")?,
        number(retirement, 0.0, "retirement")?,
        number(tax_rate, 30.0, "tax rate")?,
        tax_basis,
        number(hours_per_week, 40.0, "hours per week")?,
        number(days_per_week, 5.0, "days per week")?,
        number(hours_per_day, 8.0, "hours per day")?,
        number(weeks_per_year, 52.0, "weeks per year")?,
        number(vacation_weeks, 4.0, "vacation weeks")?,
        number(holidays, 10.0, "holidays")?,
        number(sick_days, 5.0, "sick days")?,
        number(billable_percent, 70.0, "billable percent")?,
        number(buffer_percent, 0.0, "buffer percent")?,
        number(current_rate, 0.0, "current rate")?,
        number(project_hours, 0.0, "project hours")?,
        complexity,
        currency,
        number(decimals, 2.0, "decimals")?,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
