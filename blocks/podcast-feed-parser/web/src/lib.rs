//! Browser-facing wasm-bindgen wrapper for /tools/podcast-feed-parser/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(feed: &str, limit: &str, order: &str, include_descriptions: &str) -> Result<String, JsValue> {
    use gizza_ai_podcast_feed_parser_core::{to_json, Options, Order};

    let limit = limit.trim().parse::<usize>().unwrap_or(0);
    let include_descriptions = matches!(
        include_descriptions.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let opt = Options {
        limit,
        order: Order::parse(order),
        include_descriptions,
    };
    to_json(feed, &opt).map_err(|e| JsValue::from_str(&e))
}
