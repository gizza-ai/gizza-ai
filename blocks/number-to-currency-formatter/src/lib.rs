//! gizza-ai/number-to-currency-formatter — format a pasted number as a currency
//! string. Presentation only: no exchange rates, no network.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_number_to_currency_formatter_core::format_currency;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
struct Args {
    value: String,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default = "default_symbol_style")]
    symbol_style: String,
    #[serde(default = "default_position")]
    position: String,
    #[serde(default)]
    symbol_space: bool,
    #[serde(default = "default_decimals")]
    decimals: u64,
    #[serde(default = "default_rounding")]
    rounding: String,
    #[serde(default = "default_true")]
    grouping: bool,
    #[serde(default = "default_digit_grouping")]
    digit_grouping: String,
    #[serde(default = "default_group_separator")]
    group_separator: String,
    #[serde(default = "default_decimal_separator")]
    decimal_separator: String,
    #[serde(default = "default_sign_style")]
    sign_style: String,
    #[serde(default)]
    accounting: bool,
    #[serde(default)]
    trim_zeros: bool,
}

fn default_currency() -> String {
    "USD".to_string()
}
fn default_symbol_style() -> String {
    "symbol".to_string()
}
fn default_position() -> String {
    "before".to_string()
}
fn default_decimals() -> u64 {
    2
}
fn default_rounding() -> String {
    "half_up".to_string()
}
fn default_true() -> bool {
    true
}
fn default_digit_grouping() -> String {
    "western".to_string()
}
fn default_group_separator() -> String {
    "comma".to_string()
}
fn default_decimal_separator() -> String {
    "period".to_string()
}
fn default_sign_style() -> String {
    "auto".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("value").required().describe("Number to format as currency. Accepts plain numbers, copied separators, currency affixes, negatives, accounting parentheses, trailing minus, and scientific notation, e.g. '1234.5', '$1,234.50', '1 234,50', '(1234.5)', or '1.5e3'."))
        .param(Param::string("currency").default("USD").describe("ISO code or literal currency text. Known codes such as USD, EUR, GBP, JPY and INR can render as symbols; unknown text is passed through as typed. Default USD."))
        .param(Param::enumv("symbol_style", ["symbol", "code", "none"]).default("symbol").describe("How to render the currency marker: symbol (default, USD -> $ when known), code (USD), or none (digits only)."))
        .param(Param::enumv("position", ["before", "after"]).default("before").describe("Place the currency marker before the digits (default, $1.00) or after them (1.00 USD)."))
        .param(Param::boolean("symbol_space").default(false).describe("Insert a space between the currency marker and the number. Useful for ISO codes and European styles, e.g. '1.234,50 EUR'."))
        .param(Param::integer("decimals").min(0.0).max(8.0).default(2).describe("Number of fraction digits to keep, from 0 to 8. Default 2."))
        .param(Param::enumv("rounding", ["half_up", "half_down", "half_even", "ceil", "floor", "truncate"]).default("half_up").describe("Rounding rule when extra fraction digits are dropped: half_up (default), half_down, half_even (banker's), ceil, floor, or truncate."))
        .param(Param::boolean("grouping").default(true).describe("Group the integer digits with thousands separators. Default true."))
        .param(Param::enumv("digit_grouping", ["western", "indian"]).default("western").describe("Digit grouping style: western (1,234,567, default) or indian lakh/crore grouping (12,34,567)."))
        .param(Param::enumv("group_separator", ["comma", "period", "space", "narrow_space", "apostrophe", "underscore", "none"]).default("comma").describe("Separator between digit groups: comma (default), period, space, narrow_space, apostrophe, underscore, or none."))
        .param(Param::enumv("decimal_separator", ["period", "comma"]).default("period").describe("Decimal separator in the output: period (default) or comma. It must differ from the active group separator."))
        .param(Param::enumv("sign_style", ["auto", "always", "except_zero", "never", "space"]).default("auto").describe("Sign display: auto (minus only, default), always (+/-), except_zero (+ for positives only), never (absolute value), or space (space for positives, minus for negatives)."))
        .param(Param::boolean("accounting").default(false).describe("Wrap negative amounts in parentheses instead of using a minus sign, e.g. ($1,234.50). Ignored when sign_style=never."))
        .param(Param::boolean("trim_zeros").default(false).describe("Drop trailing zeros from the fraction and remove the decimal separator if the fraction becomes empty."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: &Args) -> Result<String, String> {
    format_currency(
        &a.value,
        &a.currency,
        &a.symbol_style,
        &a.position,
        a.symbol_space,
        a.decimals as i64,
        &a.rounding,
        a.grouping,
        &a.digit_grouping,
        &a.group_separator,
        &a.decimal_separator,
        &a.sign_style,
        a.accounting,
        a.trim_zeros,
    )
}

#[cfg(target_arch = "wasm32")]
struct NumberToCurrencyFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/number-to-currency-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Format a number as a currency string without exchange rates",
    skill(
        description = "Format a raw number as a currency string with a chosen currency marker, symbol/code style, placement, decimals, grouping style, separators, rounding mode, sign display, accounting parentheses, and optional trimmed zeros. This is presentation-only: it does not convert currencies or fetch exchange rates. Input can include common copied separators, currency affixes, negative parentheses, trailing minus, and scientific notation. Returns the formatted text.",
        parameters = schema_json()
    ),
)]
impl NumberToCurrencyFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "number-to-currency-formatter", |a: Args| {
            run_args(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(value: &str) -> Args {
        Args {
            value: value.to_string(),
            currency: default_currency(),
            symbol_style: default_symbol_style(),
            position: default_position(),
            symbol_space: false,
            decimals: 2,
            rounding: default_rounding(),
            grouping: true,
            digit_grouping: default_digit_grouping(),
            group_separator: default_group_separator(),
            decimal_separator: default_decimal_separator(),
            sign_style: default_sign_style(),
            accounting: false,
            trim_zeros: false,
        }
    }

    #[test]
    fn schema_json_has_no_drift_in_required_shape() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for key in [
            "value",
            "currency",
            "symbol_style",
            "position",
            "symbol_space",
            "decimals",
            "rounding",
            "grouping",
            "digit_grouping",
            "group_separator",
            "decimal_separator",
            "sign_style",
            "accounting",
            "trim_zeros",
        ] {
            assert!(props.contains_key(key), "missing schema property {key}");
            assert!(
                props[key].get("description").is_some(),
                "{key} needs a description"
            );
        }
        assert_eq!(schema["required"], serde_json::json!(["value"]));
        assert_eq!(
            props["rounding"]["enum"],
            serde_json::json!([
                "half_up",
                "half_down",
                "half_even",
                "ceil",
                "floor",
                "truncate"
            ])
        );
        assert_eq!(props["decimals"]["minimum"], 0);
        assert_eq!(props["decimals"]["maximum"], 8);
    }

    #[test]
    fn defaults_deserialize_and_format_usd() {
        let a: Args = serde_json::from_str(r#"{"value":"1234.5"}"#).unwrap();
        assert_eq!(run_args(&a).unwrap(), "$1,234.50");
    }

    #[test]
    fn all_params_are_wired() {
        let mut a = args("-1234567.5");
        a.currency = "EUR".to_string();
        a.symbol_style = "code".to_string();
        a.position = "after".to_string();
        a.symbol_space = true;
        a.decimals = 0;
        a.rounding = "half_up".to_string();
        a.grouping = true;
        a.digit_grouping = "indian".to_string();
        a.group_separator = "period".to_string();
        a.decimal_separator = "comma".to_string();
        a.sign_style = "auto".to_string();
        a.accounting = true;
        a.trim_zeros = false;
        assert_eq!(run_args(&a).unwrap(), "(12.34.568 EUR)");
    }
}
