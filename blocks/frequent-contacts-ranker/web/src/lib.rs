//! Browser-facing wasm-bindgen wrapper for /tools/frequent-contacts-ranker/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    mbox: &str,
    count: &str,
    include_cc: &str,
    exclude: &str,
    skip_automated: &str,
    half_life_days: &str,
    min_messages: &str,
    limit: &str,
    sort: &str,
    format: &str,
) -> Result<String, JsValue> {
    let count = if count.is_empty() { "both" } else { count };
    let sort = if sort.is_empty() { "score" } else { sort };
    let format = if format.is_empty() { "report" } else { format };
    let include_cc = matches!(include_cc, "true" | "1" | "on" | "yes");
    let skip_automated = matches!(skip_automated, "true" | "1" | "on" | "yes");
    let half_life_days: f64 = if half_life_days.trim().is_empty() {
        180.0
    } else {
        half_life_days.trim().parse().map_err(|_| {
            JsValue::from_str("half-life must be a number of days (0 turns recency off)")
        })?
    };
    let min_messages: i64 = if min_messages.trim().is_empty() {
        1
    } else {
        min_messages
            .trim()
            .parse()
            .map_err(|_| JsValue::from_str("minimum messages must be a whole number of 1 or more"))?
    };
    let limit: i64 = if limit.trim().is_empty() {
        25
    } else {
        limit
            .trim()
            .parse()
            .map_err(|_| JsValue::from_str("limit must be a whole number (0 = every contact)"))?
    };
    gizza_ai_frequent_contacts_ranker_core::run(
        mbox,
        count,
        include_cc,
        exclude,
        skip_automated,
        half_life_days,
        min_messages,
        limit,
        sort,
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
