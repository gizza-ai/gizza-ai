//! Browser-facing wasm-bindgen wrapper for /tools/fiscal-quarter-mapper/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    column: &str,
    fiscal_start_month: &str,
    fiscal_year_naming: &str,
    quarter_label: &str,
    fiscal_year_label: &str,
    add_fiscal_year: &str,
    add_quarter_dates: &str,
    add_fiscal_month: &str,
    add_quarter_position: &str,
    date_order: &str,
    on_error: &str,
    header: &str,
    delimiter: &str,
    output: &str,
) -> Result<String, JsValue> {
    let bool_arg = |name: &str, value: &str, default: bool| -> Result<bool, JsValue> {
        let v = value.trim().to_ascii_lowercase();
        Ok(match v.as_str() {
            "" => default,
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" => false,
            _ => return Err(JsValue::from_str(&format!("{name} must be true or false"))),
        })
    };
    gizza_ai_fiscal_quarter_mapper_core::run(
        input,
        column,
        fiscal_start_month,
        fiscal_year_naming,
        quarter_label,
        fiscal_year_label,
        bool_arg("add_fiscal_year", add_fiscal_year, true)?,
        bool_arg("add_quarter_dates", add_quarter_dates, false)?,
        bool_arg("add_fiscal_month", add_fiscal_month, false)?,
        bool_arg("add_quarter_position", add_quarter_position, false)?,
        date_order,
        on_error,
        bool_arg("header", header, true)?,
        delimiter,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
