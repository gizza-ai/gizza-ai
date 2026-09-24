//! gizza-ai/css-reset-generator — build a CSS reset / normalize stylesheet from
//! toggleable opinions. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI and the page form); handle() delegates to
//! block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_preset")]
    preset: String,
    #[serde(default)]
    include: String,
    #[serde(default)]
    exclude: String,
    #[serde(default = "default_selector_style")]
    selector_style: String,
    #[serde(default)]
    layer: String,
    #[serde(default = "default_line_height")]
    line_height: f64,
    #[serde(default = "default_min_height")]
    body_min_height: String,
    #[serde(default = "default_true")]
    comments: bool,
    #[serde(default)]
    minify: bool,
    #[serde(default = "default_indent")]
    indent: f64,
}

fn default_preset() -> String {
    "modern".into()
}
fn default_selector_style() -> String {
    "standard".into()
}
fn default_min_height() -> String {
    "100svh".into()
}
fn default_line_height() -> f64 {
    1.5
}
fn default_indent() -> f64 {
    2.0
}
fn default_true() -> bool {
    true
}

/// The section vocabulary, repeated in the include/exclude descriptions so chat
/// and CLI callers can pick sections without a second lookup.
const SECTION_LIST: &str = "zero-out, box-sizing, text-size-adjust, margin, padding, body-defaults, \
font-smoothing, media, forms, textarea, button-reset, lists, lists-unstyled, links, headings, \
headings-unstyled, text-wrap, overflow-wrap, monospace, abbr, sub-sup, hr, tables, scroll-margin, \
isolation, interactive, focus-visible, reduced-motion, smooth-scroll";

const PRESET_DESC: &str = "Base set of sections to emit. modern is an opinionated present-day reset (border-box, margin zeroing, block media, inheriting form controls, balanced headings, reduced-motion guard). minimal is the four rules almost every project wants. normalize smooths cross-browser element differences instead of erasing them. classic is the aggressive zero-out of every common element. preflight is a utility-framework style baseline that also unstyles headings, lists and buttons. none starts empty so include alone decides the output. All presets are style-alike baselines authored for this tool, not copies of any third-party stylesheet.";
const INCLUDE_DESC: &str = "Extra sections to switch on, on top of the preset. Comma or space separated. Sections always render in the tool's canonical order, not the order you list them. Available sections: zero-out, box-sizing, text-size-adjust, margin, padding, body-defaults, font-smoothing, media, forms, textarea, button-reset, lists, lists-unstyled, links, headings, headings-unstyled, text-wrap, overflow-wrap, monospace, abbr, sub-sup, hr, tables, scroll-margin, isolation, interactive, focus-visible, reduced-motion, smooth-scroll.";
const EXCLUDE_DESC: &str = "Sections to switch off. Comma or space separated. Applied after include, so a section named in both is excluded. Same vocabulary as include: zero-out, box-sizing, text-size-adjust, margin, padding, body-defaults, font-smoothing, media, forms, textarea, button-reset, lists, lists-unstyled, links, headings, headings-unstyled, text-wrap, overflow-wrap, monospace, abbr, sub-sup, hr, tables, scroll-margin, isolation, interactive, focus-visible, reduced-motion, smooth-scroll.";
const SELECTOR_DESC: &str = "Selector specificity. standard emits plain selectors. where wraps each selector list in :where(), giving the whole reset zero specificity so any later rule of yours wins without an override war. Pseudo-element parts such as *::before stay outside the wrapper because :where() does not accept them; their specificity is already zero, so the result is equivalent.";
const LAYER_DESC: &str = "Optional CSS cascade layer to wrap the sheet in, for example base or base.reset. Leave blank for no @layer wrapper. Layer names accept letters, digits, hyphen and underscore, with dots separating nested parts.";
const LINE_HEIGHT_DESC: &str = "Body line height written by the body-defaults section, from 1 to 3. Default 1.5. Ignored when body-defaults is not part of the selection.";
const MIN_HEIGHT_DESC: &str = "Body min-height written by the body-defaults section. 100svh uses the small viewport unit, which does not jump when mobile browser chrome collapses. 100dvh tracks the dynamic viewport. 100vh is the widely supported classic. none omits the declaration. Ignored when body-defaults is not part of the selection.";
const COMMENTS_DESC: &str = "Emit a header comment listing the preset and chosen sections, plus one comment per section. Turn off for a bare stylesheet. Always off when minify is true.";
const MINIFY_DESC: &str = "Emit the stylesheet on a single line with no comments or optional whitespace. Overrides comments and indent.";
const INDENT_DESC: &str = "Spaces per indent level in formatted output, 0 to 8. Default 2. Ignored when minify is true.";

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv(
                "preset",
                ["modern", "minimal", "normalize", "classic", "preflight", "none"],
            )
            .default("modern")
            .describe(PRESET_DESC),
        )
        .param(Param::string("include").default("").describe(INCLUDE_DESC))
        .param(Param::string("exclude").default("").describe(EXCLUDE_DESC))
        .param(
            Param::enumv("selector_style", ["standard", "where"])
                .default("standard")
                .describe(SELECTOR_DESC),
        )
        .param(Param::string("layer").default("").describe(LAYER_DESC))
        .param(
            Param::number("line_height")
                .min(1.0)
                .max(3.0)
                .default(1.5)
                .describe(LINE_HEIGHT_DESC),
        )
        .param(
            Param::enumv("body_min_height", ["100svh", "100dvh", "100vh", "none"])
                .default("100svh")
                .describe(MIN_HEIGHT_DESC),
        )
        .param(Param::boolean("comments").default(true).describe(COMMENTS_DESC))
        .param(Param::boolean("minify").default(false).describe(MINIFY_DESC))
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe(INDENT_DESC),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/css-reset-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a CSS reset stylesheet from toggleable sections, presets and cascade layers",
    skill(
        description = "Generate a CSS reset or normalize stylesheet from toggleable opinions. Pick a preset (modern, minimal, normalize, classic zero-out, utility-framework preflight, or none), then add or remove any of 29 individual sections such as box-sizing, margin, media, forms, lists, headings, reduced-motion and focus-visible. Output can use zero-specificity :where() selectors, be wrapped in an @layer, carry per-section comments, use a chosen indent width, and be minified. Body line height and min-height units are configurable. Every rule is authored by this tool, not copied from a third-party stylesheet.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "css-reset-generator", |a: Args| {
            gizza_ai_css_reset_generator_core::run(
                &a.preset,
                &a.include,
                &a.exclude,
                &a.selector_style,
                &a.layer,
                a.line_height,
                &a.body_min_height,
                a.comments,
                a.minify,
                a.indent,
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

    fn core_run(a: &Args) -> Result<String, String> {
        gizza_ai_css_reset_generator_core::run(
            &a.preset,
            &a.include,
            &a.exclude,
            &a.selector_style,
            &a.layer,
            a.line_height,
            &a.body_min_height,
            a.comments,
            a.minify,
            a.indent,
        )
    }

    #[test]
    fn dump_schema() {
        println!("SCHEMA_BEGIN{}SCHEMA_END", schema_json());
    }

    #[test]
    fn schema_matches_authored_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            // No `required` key: every param has a default, so the tool answers
            // with the modern reset when called with an empty argument object.
            "additionalProperties": false,
            "properties": {
                "preset": {
                    "type": "string",
                    "enum": ["modern", "minimal", "normalize", "classic", "preflight", "none"],
                    "default": "modern",
                    "description": PRESET_DESC
                },
                "include": { "type": "string", "default": "", "description": INCLUDE_DESC },
                "exclude": { "type": "string", "default": "", "description": EXCLUDE_DESC },
                "selector_style": {
                    "type": "string",
                    "enum": ["standard", "where"],
                    "default": "standard",
                    "description": SELECTOR_DESC
                },
                "layer": { "type": "string", "default": "", "description": LAYER_DESC },
                "line_height": {
                    "type": "number",
                    "minimum": 1,
                    "maximum": 3,
                    "default": 1.5,
                    "description": LINE_HEIGHT_DESC
                },
                "body_min_height": {
                    "type": "string",
                    "enum": ["100svh", "100dvh", "100vh", "none"],
                    "default": "100svh",
                    "description": MIN_HEIGHT_DESC
                },
                "comments": { "type": "boolean", "default": true, "description": COMMENTS_DESC },
                "minify": { "type": "boolean", "default": false, "description": MINIFY_DESC },
                "indent": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 8,
                    "default": 2,
                    "description": INDENT_DESC
                }
            }
        });
        assert_eq!(actual, authored);
    }

    #[test]
    fn every_enum_variant_and_default_is_accepted_by_core() {
        // The descriptor advertises these values; core must accept every one.
        for preset in ["modern", "minimal", "normalize", "classic", "preflight"] {
            let a: Args =
                serde_json::from_str(&format!(r#"{{"preset":"{preset}"}}"#)).unwrap();
            assert!(core_run(&a).is_ok(), "preset {preset} rejected");
        }
        for style in ["standard", "where"] {
            let a: Args =
                serde_json::from_str(&format!(r#"{{"selector_style":"{style}"}}"#)).unwrap();
            assert!(core_run(&a).is_ok(), "selector_style {style} rejected");
        }
        for mh in ["100svh", "100dvh", "100vh", "none"] {
            let a: Args =
                serde_json::from_str(&format!(r#"{{"body_min_height":"{mh}"}}"#)).unwrap();
            assert!(core_run(&a).is_ok(), "body_min_height {mh} rejected");
        }
        for section in gizza_ai_css_reset_generator_core::section_ids() {
            let a: Args =
                serde_json::from_str(&format!(r#"{{"preset":"none","include":"{section}"}}"#))
                    .unwrap();
            assert!(core_run(&a).is_ok(), "section {section} rejected");
            assert!(SECTION_LIST.contains(section), "{section} missing from SECTION_LIST");
        }
        assert_eq!(
            SECTION_LIST.split(", ").count(),
            gizza_ai_css_reset_generator_core::section_ids().len()
        );
    }

    #[test]
    fn schema_defaults_produce_the_modern_reset() {
        let a: Args = serde_json::from_str("{}").unwrap();
        let css = core_run(&a).unwrap();
        assert!(css.starts_with("/* CSS reset — preset: modern"), "{css}");
        assert!(css.contains("  box-sizing: border-box;"), "{css}");
        assert!(css.contains("min-height: 100svh;"), "{css}");
        assert!(css.contains("line-height: 1.5;"), "{css}");
    }

    #[test]
    fn schema_bounds_match_core_validation() {
        // The descriptor's minimum/maximum are the same limits core enforces.
        for (json, needle) in [
            (r#"{"line_height":3.5}"#, "line_height must be between 1 and 3"),
            (r#"{"indent":9}"#, "indent must be a whole number"),
            (r#"{"preset":"tailwind"}"#, "unknown preset `tailwind`"),
        ] {
            let a: Args = serde_json::from_str(json).unwrap();
            let err = core_run(&a).unwrap_err();
            assert!(err.contains(needle), "{json} → {err}");
        }
        // The advertised bounds themselves are accepted.
        for json in [r#"{"line_height":1}"#, r#"{"line_height":3}"#, r#"{"indent":0}"#, r#"{"indent":8}"#] {
            let a: Args = serde_json::from_str(json).unwrap();
            assert!(core_run(&a).is_ok(), "boundary {json} rejected");
        }
    }
}
