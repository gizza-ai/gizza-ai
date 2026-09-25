//! Browser-facing wasm-bindgen wrapper for /tools/open-graph-tags/.
//!
//! The page driver passes every field as a STRING (the pure path does no numeric
//! coercion), so numbers and booleans are parsed here. Checkboxes arrive as
//! "true"/"false"; a blank value falls back to the descriptor's default.
use gizza_ai_open_graph_tags_core as core;
use wasm_bindgen::prelude::*;

/// Parse a checkbox field, falling back to `default` when the field is absent/blank.
fn flag(v: &str, default: bool) -> bool {
    let v = v.trim();
    if v.is_empty() {
        default
    } else {
        matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
    }
}

/// Parse a pixel dimension; blank means 0 ("omit the tag").
fn dimension(label: &str, v: &str) -> Result<u32, String> {
    let v = v.trim();
    if v.is_empty() {
        return Ok(0);
    }
    v.parse::<u32>().map_err(|_| {
        format!("{label} must be a whole number of pixels (0 omits the tag) — got \"{v}\"")
    })
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    title: &str,
    description: &str,
    url: &str,
    image: &str,
    image_alt: &str,
    image_width: &str,
    image_height: &str,
    site_name: &str,
    og_type: &str,
    twitter_card: &str,
    twitter_site: &str,
    twitter_creator: &str,
    locale: &str,
    author: &str,
    include_basic: &str,
    include_twitter: &str,
    include_schema: &str,
    group_comments: &str,
    warnings: &str,
) -> Result<String, JsValue> {
    let opts = core::Options {
        title: title.to_string(),
        description: description.to_string(),
        url: url.to_string(),
        image: image.to_string(),
        image_alt: image_alt.to_string(),
        image_width: dimension("image_width", image_width).map_err(|e| JsValue::from_str(&e))?,
        image_height: dimension("image_height", image_height).map_err(|e| JsValue::from_str(&e))?,
        site_name: site_name.to_string(),
        og_type: og_type.to_string(),
        twitter_card: twitter_card.to_string(),
        twitter_site: twitter_site.to_string(),
        twitter_creator: twitter_creator.to_string(),
        locale: locale.to_string(),
        author: author.to_string(),
        include_basic: flag(include_basic, true),
        include_twitter: flag(include_twitter, true),
        include_schema: flag(include_schema, false),
        group_comments: flag(group_comments, true),
        warnings: flag(warnings, true),
    };
    core::generate(&opts).map_err(|e| JsValue::from_str(&e))
}
