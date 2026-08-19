//! Browser-facing wasm-bindgen wrapper for /tools/ts-decompose/.
use wasm_bindgen::prelude::*;

use gizza_ai_ts_decompose_core::Options;

fn boolish(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

/// Checkbox fields that default to ON: an empty value means "not sent", not "off".
fn boolish_default_true(s: &str) -> bool {
    if s.trim().is_empty() {
        true
    } else {
        boolish(s)
    }
}

fn text_or(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.into()
    } else {
        s.into()
    }
}

fn parse_u32(name: &str, s: &str, fallback: u32) -> Result<u32, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<u32>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    method: &str,
    model: &str,
    period: &str,
    seasonal_window: &str,
    trend_window: &str,
    robust: &str,
    two_sided: &str,
    extrapolate_trend: &str,
    trend_overlay: &str,
    show_adjusted: &str,
    residual_style: &str,
    grid: &str,
    title: &str,
    x_label: &str,
    y_label: &str,
    width: &str,
    height: &str,
    color: &str,
    theme: &str,
    precision: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        method: text_or(method, "stl"),
        model: text_or(model, "additive"),
        period: parse_u32("period", period, 0)?,
        seasonal_window: parse_u32("seasonal_window", seasonal_window, 0)?,
        trend_window: parse_u32("trend_window", trend_window, 0)?,
        robust: boolish(robust),
        two_sided: boolish_default_true(two_sided),
        extrapolate_trend: boolish_default_true(extrapolate_trend),
        trend_overlay: boolish_default_true(trend_overlay),
        show_adjusted: boolish(show_adjusted),
        residual_style: text_or(residual_style, "bar"),
        grid: boolish_default_true(grid),
        title: title.into(),
        x_label: x_label.into(),
        y_label: y_label.into(),
        width: parse_u32("width", width, 900)?,
        height: parse_u32("height", height, 720)?,
        color: text_or(color, "#2563eb"),
        theme: text_or(theme, "light"),
        precision: parse_u32("precision", precision, 4)?,
        output: text_or(output, "svg"),
    };
    gizza_ai_ts_decompose_core::render(data, &opts).map_err(|e| JsValue::from_str(&e))
}
