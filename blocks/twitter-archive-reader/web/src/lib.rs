//! Browser-facing wasm-bindgen wrapper for /tools/twitter-archive-reader/.
//! The page passes every field as a string (in declared meta.toml order); this
//! parses the option fields and delegates to the pure core.
use gizza_ai_twitter_archive_reader_core::{render, Format, Options, Output, Sort};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    tweets: &str,
    output: &str,
    format: &str,
    sort: &str,
    search: &str,
    since: &str,
    until: &str,
    include_replies: &str,
    include_retweets: &str,
    expand_urls: &str,
    top_count: &str,
    max_tweets: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        output: Output::parse(output),
        format: Format::parse(format).map_err(|e| JsValue::from_str(&e))?,
        sort: Sort::parse(sort).map_err(|e| JsValue::from_str(&e))?,
        search: opt(search),
        since: opt(since),
        until: opt(until),
        include_replies: truthy(include_replies),
        include_retweets: truthy(include_retweets),
        expand_urls: truthy(expand_urls),
        top_count: top_count.trim().parse::<usize>().unwrap_or(5),
        max_tweets: max_tweets.trim().parse::<usize>().unwrap_or(0),
    };
    render(tweets, &opts).map_err(|e| JsValue::from_str(&e))
}
