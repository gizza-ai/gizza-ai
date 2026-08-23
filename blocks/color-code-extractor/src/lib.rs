//! gizza-ai/color-code-extractor — chat skill block on the shared tool abstraction.
//! Scans pasted CSS/HTML/JS/config text for every colour literal, normalises them to
//! RGBA, deduplicates into one palette and renders it as a list, CSV, JSON, CSS
//! custom properties, SCSS/LESS variables, a Tailwind colour map or an SVG swatch
//! sheet. The chat schema is single-sourced from `descriptor()` (which also drives
//! the CLI); `handle()` delegates to `block_utils::run_skill`. Pure compute — the
//! text is scanned in the sandbox, nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    output_format: String,
    #[serde(default)]
    color_format: String,
    #[serde(default)]
    sort: String,
    #[serde(default = "default_true")]
    include_counts: bool,
    #[serde(default = "default_true")]
    include_named: bool,
    #[serde(default)]
    exclude_grey: bool,
    #[serde(default)]
    exclude_monochrome: bool,
    #[serde(default)]
    uppercase: bool,
    #[serde(default)]
    limit: i64,
    #[serde(default)]
    var_prefix: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to scan — a whole stylesheet, an SCSS/LESS partial, an HTML page, a JS theme file, a JSON design-token dump or plain prose. Every colour literal anywhere in it is collected, including inside comments and strings; everything else is ignored, so pasting a full file is normal. Recognised notations are hex (#f00, #f00f, #ff0000, #ff0000ff), rgb()/rgba(), hsl()/hsla(), hwb(), the 148 CSS colour keywords and 'transparent', in both legacy comma syntax and modern space-and-slash syntax. Max 5,000,000 bytes."),
        )
        .param(
            Param::enumv(
                "output_format",
                ["list", "csv", "json", "css_vars", "scss", "less", "tailwind", "svg"],
            )
            .default("list")
            .describe("How to render the deduplicated palette. 'list' (the default) is one colour per line, ready to paste anywhere. 'csv' adds hex, rgb, hsl, alpha and count columns for a spreadsheet. 'json' is the full machine-readable record per colour, including hue/saturation/lightness, the exact keyword name when there is one, and every spelling that mapped to it. 'css_vars' emits a :root block of custom properties, 'scss' and 'less' emit variable declarations, and 'tailwind' emits a theme.extend.colors config. 'svg' draws a labelled swatch sheet you can save as an image."),
        )
        .param(
            Param::enumv("color_format", ["hex", "original", "rgb", "hsl", "hwb", "name"])
                .default("hex")
                .describe("The notation each palette entry is REWRITTEN to. 'hex' (the default) normalises everything to #rrggbb, or #rrggbbaa when the colour is translucent — this is what makes #f00, red and rgb(255,0,0) collapse visibly into one entry. 'original' keeps the first spelling exactly as it appeared in the source, useful for an audit that must quote the file. 'rgb', 'hsl' and 'hwb' convert every entry to that function syntax. 'name' uses the CSS keyword when the colour matches one exactly and falls back to hex otherwise. Deduplication happens on the colour itself, so this setting never changes which entries you get."),
        )
        .param(
            Param::enumv("sort", ["first_seen", "frequency", "hue", "lightness", "alphabetical"])
                .default("first_seen")
                .describe("Palette order. 'first_seen' (the default) is source order, which keeps a stylesheet's own structure. 'frequency' puts the most-used colours first, which is how you find the real brand palette hiding among one-off values. 'hue' walks the colour wheel red → green → blue and is the order you want for a swatch sheet. 'lightness' runs darkest to lightest, good for building a tint ramp. 'alphabetical' sorts on the rendered value."),
        )
        .param(
            Param::boolean("include_counts")
                .default(true)
                .describe("Show how many times each colour appears. On by default: 'list' appends '×N', 'csv' adds a count column, 'css_vars'/'scss'/'less' add a '2 uses' comment and 'svg' labels each swatch. Turn it off for output you are pasting straight into a stylesheet. 'json' always reports counts."),
        )
        .param(
            Param::boolean("include_named")
                .default(true)
                .describe("Treat bare CSS colour keywords such as red, gold, tan or mediumseagreen as colours. On by default, which is right for stylesheets. Turn it OFF when scanning prose or content-heavy HTML, where ordinary English words like 'orange', 'plum' and 'snow' would otherwise be collected as colours. Class names, ids and preprocessor variables (.red, #brand, $blue, @blue, --brand-red) are never matched either way."),
        )
        .param(
            Param::boolean("exclude_grey")
                .default(false)
                .describe("Drop greys — any colour whose red, green and blue channels are equal — while KEEPING pure black and pure white. Off by default. Turn it on to strip the border, shadow and divider greys out of a stylesheet audit while the text and background stay visible."),
        )
        .param(
            Param::boolean("exclude_monochrome")
                .default(false)
                .describe("Drop every neutral, including pure black and pure white, leaving only colours that carry a hue. Off by default. This is the stricter version of exclude_grey and takes precedence when both are on — use it to isolate the actual brand colours."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("Write hex digits in upper case, so #aabbcc becomes #AABBCC. Off by default because lower case is the CSS convention. The leading # and every non-hex notation are unaffected."),
        )
        .param(
            Param::integer("limit")
                .default(0)
                .min(0.0)
                .max(1000.0)
                .describe("Keep at most this many palette entries, applied AFTER sorting — so limit=8 with sort='frequency' gives the eight most-used colours, which is the usual way to pull a brand palette out of a large stylesheet. 0 (the default) keeps every unique colour."),
        )
        .param(
            Param::string("var_prefix")
                .default("color")
                .describe("Name stem for the generated variables in the css_vars, scss, less and tailwind formats: 'color' (the default) gives --color-1, $color-1, @color-1 and 'color-1'. Set it to 'brand' or 'palette' to match your naming. Letters, digits, hyphens and underscores only. Ignored by the list, csv, json and svg formats."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/color-code-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract every hex, rgb(a), hsl(a) and named colour from CSS or text as one deduplicated palette.",
    skill(
        description = "Scan any pasted text — a stylesheet, SCSS/LESS partial, HTML page, JS theme file, JSON token dump or prose — for every colour literal, and return ONE deduplicated palette. Recognises hex (#f00, #f00f, #ff0000, #ff0000ff), rgb()/rgba(), hsl()/hsla(), hwb(), the 148 CSS colour keywords and 'transparent', in both legacy comma syntax (rgb(255, 0, 0)) and modern space-and-slash syntax (rgb(255 0 0 / 50%), hsl(120deg 100% 50% / .5)); angles in deg, grad, rad or turn and the CSS-4 'none' keyword are handled. Deduplication is by COLOUR, not spelling: #f00, #FF0000, red and rgb(255,0,0) become a single entry with a usage count of four, while alpha stays part of the identity so #ff0000 and rgba(255,0,0,.5) remain separate. color_format rewrites each entry to hex (default), the original spelling, rgb, hsl, hwb or the exact CSS keyword. output_format renders the palette as a plain list (default), csv with hex/rgb/hsl/alpha/count columns, full json, a :root css_vars block, scss or less variables, a tailwind theme.extend.colors config, or an svg swatch sheet. sort orders by first_seen (default), frequency, hue, lightness or alphabetical, and limit keeps the top N after sorting — sort='frequency' with limit=8 is how you pull the real brand palette out of a big stylesheet. exclude_grey drops equal-channel greys but keeps pure black and white; exclude_monochrome drops all neutrals. include_named can be switched off when scanning prose, where words like orange, plum and snow are not colours. Class names, ids and preprocessor variables (.red, #brand {, $blue, @blue, --brand-red) are skipped, and colours built with calc() or var() cannot be resolved statically so they are skipped too. Max 5,000,000 bytes. Runs entirely in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "color-code-extractor", |a: Args| {
            gizza_ai_color_code_extractor_core::extract(
                &a.text,
                &a.output_format,
                &a.color_format,
                &a.sort,
                a.include_counts,
                a.include_named,
                a.exclude_grey,
                a.exclude_monochrome,
                a.uppercase,
                a.limit,
                &a.var_prefix,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-08-23 for the initial color-code-extractor release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to scan — a whole stylesheet, an SCSS/LESS partial, an HTML page, a JS theme file, a JSON design-token dump or plain prose. Every colour literal anywhere in it is collected, including inside comments and strings; everything else is ignored, so pasting a full file is normal. Recognised notations are hex (#f00, #f00f, #ff0000, #ff0000ff), rgb()/rgba(), hsl()/hsla(), hwb(), the 148 CSS colour keywords and 'transparent', in both legacy comma syntax and modern space-and-slash syntax. Max 5,000,000 bytes." },
                    "output_format": { "type": "string", "enum": ["list", "csv", "json", "css_vars", "scss", "less", "tailwind", "svg"], "default": "list", "description": "How to render the deduplicated palette. 'list' (the default) is one colour per line, ready to paste anywhere. 'csv' adds hex, rgb, hsl, alpha and count columns for a spreadsheet. 'json' is the full machine-readable record per colour, including hue/saturation/lightness, the exact keyword name when there is one, and every spelling that mapped to it. 'css_vars' emits a :root block of custom properties, 'scss' and 'less' emit variable declarations, and 'tailwind' emits a theme.extend.colors config. 'svg' draws a labelled swatch sheet you can save as an image." },
                    "color_format": { "type": "string", "enum": ["hex", "original", "rgb", "hsl", "hwb", "name"], "default": "hex", "description": "The notation each palette entry is REWRITTEN to. 'hex' (the default) normalises everything to #rrggbb, or #rrggbbaa when the colour is translucent — this is what makes #f00, red and rgb(255,0,0) collapse visibly into one entry. 'original' keeps the first spelling exactly as it appeared in the source, useful for an audit that must quote the file. 'rgb', 'hsl' and 'hwb' convert every entry to that function syntax. 'name' uses the CSS keyword when the colour matches one exactly and falls back to hex otherwise. Deduplication happens on the colour itself, so this setting never changes which entries you get." },
                    "sort": { "type": "string", "enum": ["first_seen", "frequency", "hue", "lightness", "alphabetical"], "default": "first_seen", "description": "Palette order. 'first_seen' (the default) is source order, which keeps a stylesheet's own structure. 'frequency' puts the most-used colours first, which is how you find the real brand palette hiding among one-off values. 'hue' walks the colour wheel red → green → blue and is the order you want for a swatch sheet. 'lightness' runs darkest to lightest, good for building a tint ramp. 'alphabetical' sorts on the rendered value." },
                    "include_counts": { "type": "boolean", "default": true, "description": "Show how many times each colour appears. On by default: 'list' appends '×N', 'csv' adds a count column, 'css_vars'/'scss'/'less' add a '2 uses' comment and 'svg' labels each swatch. Turn it off for output you are pasting straight into a stylesheet. 'json' always reports counts." },
                    "include_named": { "type": "boolean", "default": true, "description": "Treat bare CSS colour keywords such as red, gold, tan or mediumseagreen as colours. On by default, which is right for stylesheets. Turn it OFF when scanning prose or content-heavy HTML, where ordinary English words like 'orange', 'plum' and 'snow' would otherwise be collected as colours. Class names, ids and preprocessor variables (.red, #brand, $blue, @blue, --brand-red) are never matched either way." },
                    "exclude_grey": { "type": "boolean", "default": false, "description": "Drop greys — any colour whose red, green and blue channels are equal — while KEEPING pure black and pure white. Off by default. Turn it on to strip the border, shadow and divider greys out of a stylesheet audit while the text and background stay visible." },
                    "exclude_monochrome": { "type": "boolean", "default": false, "description": "Drop every neutral, including pure black and pure white, leaving only colours that carry a hue. Off by default. This is the stricter version of exclude_grey and takes precedence when both are on — use it to isolate the actual brand colours." },
                    "uppercase": { "type": "boolean", "default": false, "description": "Write hex digits in upper case, so #aabbcc becomes #AABBCC. Off by default because lower case is the CSS convention. The leading # and every non-hex notation are unaffected." },
                    "limit": { "type": "integer", "minimum": 0, "maximum": 1000, "default": 0, "description": "Keep at most this many palette entries, applied AFTER sorting — so limit=8 with sort='frequency' gives the eight most-used colours, which is the usual way to pull a brand palette out of a large stylesheet. 0 (the default) keeps every unique colour." },
                    "var_prefix": { "type": "string", "default": "color", "description": "Name stem for the generated variables in the css_vars, scss, less and tailwind formats: 'color' (the default) gives --color-1, $color-1, @color-1 and 'color-1'. Set it to 'brand' or 'palette' to match your naming. Letters, digits, hyphens and underscores only. Ignored by the list, csv, json and svg formats." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
