//! gizza-ai/recipe-extract — fetch a recipe page via wafer-run/network and
//! return the recipe itself: title, ingredients, numbered steps, prep/cook/total
//! time, yield and nutrition, with the blog story, ads and comments left behind.
//!
//! The parsing engine lives in [`core`] (pure, native-testable); this file is
//! the thin chat/CLI wrapper: parse args → fetch → extract → render. Like
//! web-fetch and css-select-extract this is a NETWORK block, so it ships no
//! browser page: a page's WASM cannot fetch a third-party URL (CORS).

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

pub mod core;

/// Bytes of HTML fetched when `max_bytes` is omitted. Recipe pages are heavy
/// (inline scripts, base64 images); 4 MiB covers the long tail.
const DEFAULT_MAX_BYTES: usize = 4 << 20; // 4 MiB
/// Hard ceiling on `max_bytes`, so one page can't exhaust the sandbox's memory.
const MAX_HTML_BYTES: usize = 8 << 20; // 8 MiB
/// Ingredient-scaling bounds (a 0 or negative multiplier has no meaning).
const MIN_SCALE: f64 = 0.01;
const MAX_SCALE: f64 = 100.0;

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    scale: Option<f64>,
    #[serde(default)]
    include_nutrition: Option<bool>,
    #[serde(default)]
    max_bytes: Option<usize>,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
/// recipe-extract is `Input::None` — `url` is a normal required string param
/// (it has no `ref`), so there is no `url`⊕`ref` `oneOf`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("url")
                .required()
                .describe("Absolute http or https URL of the recipe page to read."),
        )
        .param(
            Param::enumv("format", ["markdown", "text", "json"])
                .default("markdown")
                .describe(
                    "Output shape: \"markdown\" (default) = headed Markdown with bullet ingredients and numbered steps, \"text\" = plain text, \"json\" = the full structured record (title, ingredients, instructions, times, yield, nutrition, image, source).",
                ),
        )
        .param(
            Param::number("scale")
                .min(MIN_SCALE)
                .max(MAX_SCALE)
                .default(1.0)
                .describe(
                    "Multiply every ingredient quantity by this factor (default 1 = as published; 2 = double, 0.5 = half). Whole numbers, decimals, 1/2, 1 1/2 and unicode fractions are rescaled; quantities the parser can't read (\"salt to taste\") and the step text are left untouched.",
                ),
        )
        .param(
            Param::boolean("include_nutrition")
                .default(true)
                .describe(
                    "Include the page's published nutrition facts (calories, protein, fat, …) when it has any (default true). Set false for just the recipe.",
                ),
        )
        .param(
            Param::integer("max_bytes")
                .min(1.0)
                .max(MAX_HTML_BYTES as f64)
                .describe(
                    "Maximum number of bytes of HTML to download (default 4194304, max 8388608). A larger page is rejected rather than truncated, so half a recipe is never returned.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Validate and normalize the ingredient-scaling factor.
fn parse_scale(scale: Option<f64>) -> Result<f64, String> {
    let scale = scale.unwrap_or(1.0);
    if !scale.is_finite() || !(MIN_SCALE..=MAX_SCALE).contains(&scale) {
        return Err(format!(
            "invalid scale `{scale}`: expected a number between {MIN_SCALE} and {MAX_SCALE}"
        ));
    }
    Ok(scale)
}

/// Validate and clamp the download cap.
fn parse_max_bytes(max_bytes: Option<usize>) -> Result<usize, String> {
    let bytes = max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    if bytes == 0 || bytes > MAX_HTML_BYTES {
        return Err(format!(
            "invalid max_bytes `{bytes}`: expected 1..={MAX_HTML_BYTES}"
        ));
    }
    Ok(bytes)
}

/// Reject anything that isn't an absolute http(s) URL before spending a fetch.
fn parse_url(url: &str) -> Result<&str, String> {
    let trimmed = url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Ok(trimmed)
    } else {
        Err(format!(
            "invalid url `{trimmed}`: expected an absolute http:// or https:// recipe page URL"
        ))
    }
}

#[cfg(target_arch = "wasm32")]
struct RecipeExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/recipe-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fetch a recipe page and extract the clean recipe",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Fetch a recipe page and return just the recipe: title, ingredients, numbered steps, prep/cook/total time, yield and nutrition — without the blog story, ads or comments. Reads the schema.org/Recipe data the page publishes (JSON-LD, with a microdata fallback). Set format='markdown' (default), 'text' or 'json', and scale=2 to double the ingredient quantities.",
        parameters = schema_json()
    ),
)]
impl RecipeExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    run_skill(&body, "recipe-extract", |args: Args| {
        let format = core::Format::parse(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;
        let scale = parse_scale(args.scale).map_err(SkillError::InvalidArgs)?;
        let max_bytes = parse_max_bytes(args.max_bytes).map_err(SkillError::InvalidArgs)?;
        let include_nutrition = args.include_nutrition.unwrap_or(true);
        let url = parse_url(&args.url).map_err(SkillError::InvalidArgs)?.to_string();

        let resp = wafer_sdk::clients::network::do_request("GET", &url, &HashMap::new(), None)?;
        if resp.status_code >= 400 {
            return Err(SkillError::HttpStatus {
                status: resp.status_code,
                url,
            });
        }
        if resp.body.len() > max_bytes {
            return Err(SkillError::TooLarge {
                kind: "recipe page",
                bytes: resp.body.len(),
                cap: max_bytes,
            });
        }
        let html = String::from_utf8_lossy(&resp.body);
        let recipe = core::extract(&html, Some(&url), scale, include_nutrition)
            .map_err(SkillError::InvalidArgs)?;
        Ok(core::render(&recipe, format))
    })
}

#[cfg(test)]
mod tests {
    use super::core::{
        clean_text, extract, format_qty, humanize_duration, render, scale_ingredient, Format,
    };
    use super::{parse_max_bytes, parse_scale, parse_url, schema_json, MAX_HTML_BYTES};

    /// A modern recipe page: JSON-LD inside an `@graph`, HowToStep objects,
    /// nutrition, and 900 words of blog story that must NOT come through.
    const JSON_LD_PAGE: &str = r##"
<!doctype html><html><head>
<title>The Best Apple Cake — My Life Story</title>
<script type="application/ld+json">
{"@context":"https://schema.org","@graph":[
  {"@type":"WebSite","name":"A Cooking Blog"},
  {"@type":"Recipe",
   "name":"Spiced Apple Cake",
   "author":{"@type":"Person","name":"Jane Cook"},
   "description":"A moist apple cake with cinnamon &amp; nutmeg.",
   "image":["https://example.com/cake.jpg"],
   "prepTime":"PT20M","cookTime":"PT45M","totalTime":"PT1H5M",
   "recipeYield":"12 slices",
   "recipeCategory":"Dessert","recipeCuisine":"American",
   "aggregateRating":{"@type":"AggregateRating","ratingValue":"4.8","ratingCount":"231"},
   "recipeIngredient":["2 cups flour","1 1/2 tsp cinnamon","3 apples, peeled","1/2 cup butter"],
   "recipeInstructions":[
     {"@type":"HowToStep","text":"Heat the oven to 180 C."},
     {"@type":"HowToStep","text":"Mix the <b>dry</b> ingredients."},
     {"@type":"HowToStep","text":"Fold in the apples and bake for 45 minutes."}],
   "nutrition":{"@type":"NutritionInformation","calories":"320 kcal","proteinContent":"4","fatContent":"12"}}
]}
</script></head>
<body><article><p>It was the summer of 1998 and my grandmother...</p>
<p>Scroll down for the recipe! But first, my trip to Vermont.</p></article></body></html>
"##;

    /// An older page with microdata only, `<time datetime>` durations, and an
    /// instructions list rendered as `<li>` items.
    const MICRODATA_PAGE: &str = r##"
<!doctype html><html><body>
<div itemscope itemtype="http://schema.org/Recipe">
  <h1 itemprop="name">Simple Pancakes</h1>
  <span itemprop="author">Sam Baker</span>
  <time itemprop="prepTime" datetime="PT5M">five minutes</time>
  <time itemprop="cookTime" datetime="PT10M">ten minutes</time>
  <span itemprop="recipeYield">8 pancakes</span>
  <li itemprop="recipeIngredient">1 cup flour</li>
  <li itemprop="recipeIngredient">1 egg</li>
  <li itemprop="recipeIngredient">3/4 cup milk</li>
  <div itemprop="recipeInstructions"><ol>
    <li>Whisk everything together.</li>
    <li>Fry in a hot pan.</li>
  </ol></div>
</div></body></html>
"##;

    /// A page with sectioned instructions (HowToSection) and a single-string
    /// ingredient/instruction shape — both legal schema.org.
    const SECTIONED_PAGE: &str = r##"
<!doctype html><html><head>
<script type="application/ld+json">
{"@context":"http://schema.org","@type":["Recipe","NewsArticle"],
 "name":"Roast Chicken",
 "recipeYield":4,
 "recipeIngredient":["1 chicken","2-3 cloves garlic","½ lemon"],
 "recipeInstructions":[
   {"@type":"HowToSection","name":"For the bird","itemListElement":[
      {"@type":"HowToStep","text":"Pat the chicken dry."},
      {"@type":"HowToStep","text":"Season generously."}]},
   {"@type":"HowToSection","name":"To roast","itemListElement":[
      {"@type":"HowToStep","text":"Roast at 200 C for an hour."}]}]}
</script></head><body></body></html>
"##;

    // -- schema drift guard ------------------------------------------------

    /// The LLM-facing chat schema is the tool's contract: pin it so a
    /// descriptor edit can never silently change what chat/CLI advertise.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Absolute http or https URL of the recipe page to read." },
                    "format": { "type": "string", "enum": ["markdown", "text", "json"], "default": "markdown", "description": "Output shape: \"markdown\" (default) = headed Markdown with bullet ingredients and numbered steps, \"text\" = plain text, \"json\" = the full structured record (title, ingredients, instructions, times, yield, nutrition, image, source)." },
                    "scale":  { "type": "number", "minimum": 0.01, "maximum": 100, "default": 1.0, "description": "Multiply every ingredient quantity by this factor (default 1 = as published; 2 = double, 0.5 = half). Whole numbers, decimals, 1/2, 1 1/2 and unicode fractions are rescaled; quantities the parser can't read (\"salt to taste\") and the step text are left untouched." },
                    "include_nutrition": { "type": "boolean", "default": true, "description": "Include the page's published nutrition facts (calories, protein, fat, …) when it has any (default true). Set false for just the recipe." },
                    "max_bytes": { "type": "integer", "minimum": 1, "maximum": 8388608, "description": "Maximum number of bytes of HTML to download (default 4194304, max 8388608). A larger page is rejected rather than truncated, so half a recipe is never returned." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    // -- happy paths -------------------------------------------------------

    #[test]
    fn extracts_json_ld_recipe_and_drops_the_blog_story() {
        let r = extract(JSON_LD_PAGE, Some("https://example.com/cake"), 1.0, true).unwrap();
        assert_eq!(r.title, "Spiced Apple Cake");
        assert_eq!(r.author.as_deref(), Some("Jane Cook"));
        assert_eq!(r.markup, "json-ld");
        assert_eq!(r.prep_time.as_deref(), Some("20 min"));
        assert_eq!(r.cook_time.as_deref(), Some("45 min"));
        assert_eq!(r.total_time.as_deref(), Some("1 hr 5 min"));
        assert_eq!(r.yields.as_deref(), Some("12 slices"));
        assert_eq!(r.category.as_deref(), Some("Dessert"));
        assert_eq!(r.rating.as_deref(), Some("4.8 (231 ratings)"));
        assert_eq!(r.image.as_deref(), Some("https://example.com/cake.jpg"));
        assert_eq!(r.ingredients.len(), 4);
        assert_eq!(r.ingredients[1], "1 1/2 tsp cinnamon");
        assert_eq!(r.instructions.len(), 1);
        assert_eq!(r.instructions[0].steps.len(), 3);
        // Tags inside a step are stripped, entities decoded.
        assert_eq!(r.instructions[0].steps[1], "Mix the dry ingredients.");
        assert_eq!(
            r.description.as_deref(),
            Some("A moist apple cake with cinnamon & nutmeg.")
        );
        // The blog filler never reaches the output.
        let rendered = render(&r, Format::Markdown);
        assert!(!rendered.contains("grandmother"), "blog story leaked: {rendered}");
        assert!(!rendered.contains("Vermont"), "blog story leaked: {rendered}");
    }

    #[test]
    fn markdown_output_is_exact() {
        let r = extract(JSON_LD_PAGE, Some("https://example.com/cake"), 1.0, true).unwrap();
        let expected = "\
# Spiced Apple Cake

By Jane Cook

Source: https://example.com/cake

Prep 20 min · Cook 45 min · Total 1 hr 5 min · Yield: 12 slices

## Ingredients

- 2 cups flour
- 1 1/2 tsp cinnamon
- 3 apples, peeled
- 1/2 cup butter

## Instructions

1. Heat the oven to 180 C.
2. Mix the dry ingredients.
3. Fold in the apples and bake for 45 minutes.

## Nutrition

- Calories: 320 kcal
- Protein: 4 g
- Fat: 12 g
";
        assert_eq!(render(&r, Format::Markdown), expected);
    }

    #[test]
    fn text_output_drops_markdown_punctuation() {
        let r = extract(JSON_LD_PAGE, None, 1.0, false).unwrap();
        let out = render(&r, Format::Text);
        assert!(out.starts_with("Spiced Apple Cake\nBy Jane Cook\n"), "got: {out}");
        assert!(out.contains("\nINGREDIENTS\n- 2 cups flour\n"), "got: {out}");
        assert!(out.contains("\nINSTRUCTIONS\n1. Heat the oven to 180 C.\n"), "got: {out}");
        assert!(!out.contains('#'), "text output must not use Markdown headings");
        // include_nutrition = false drops the whole block.
        assert!(!out.contains("NUTRITION"), "got: {out}");
    }

    #[test]
    fn json_output_carries_the_structured_record() {
        let r = extract(JSON_LD_PAGE, Some("https://example.com/cake"), 1.0, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&render(&r, Format::Json)).unwrap();
        assert_eq!(v["title"], "Spiced Apple Cake");
        assert_eq!(v["yield"], "12 slices");
        assert_eq!(v["ingredients"][0], "2 cups flour");
        assert_eq!(v["instructions"][0]["steps"][0], "Heat the oven to 180 C.");
        assert_eq!(v["nutrition"][0]["name"], "Calories");
        assert_eq!(v["markup"], "json-ld");
        assert_eq!(v["scale"], 1.0);
    }

    #[test]
    fn microdata_fallback_reads_older_pages() {
        let r = extract(MICRODATA_PAGE, None, 1.0, true).unwrap();
        assert_eq!(r.markup, "microdata");
        assert_eq!(r.title, "Simple Pancakes");
        assert_eq!(r.author.as_deref(), Some("Sam Baker"));
        // <time datetime="PT5M"> wins over the human text "five minutes".
        assert_eq!(r.prep_time.as_deref(), Some("5 min"));
        assert_eq!(r.cook_time.as_deref(), Some("10 min"));
        assert_eq!(r.yields.as_deref(), Some("8 pancakes"));
        assert_eq!(r.ingredients, vec!["1 cup flour", "1 egg", "3/4 cup milk"]);
        assert_eq!(
            r.instructions[0].steps,
            vec!["Whisk everything together.", "Fry in a hot pan."]
        );
    }

    #[test]
    fn sections_numeric_yield_and_type_arrays_are_handled() {
        let r = extract(SECTIONED_PAGE, None, 1.0, true).unwrap();
        assert_eq!(r.title, "Roast Chicken");
        // "@type": ["Recipe", "NewsArticle"] still counts as a Recipe.
        assert_eq!(r.markup, "json-ld");
        // A bare number yield is servings.
        assert_eq!(r.yields.as_deref(), Some("4 servings"));
        assert_eq!(r.instructions.len(), 2);
        assert_eq!(r.instructions[0].name.as_deref(), Some("For the bird"));
        assert_eq!(r.instructions[1].name.as_deref(), Some("To roast"));
        assert_eq!(r.instructions[1].steps, vec!["Roast at 200 C for an hour."]);
        let md = render(&r, Format::Markdown);
        assert!(md.contains("### For the bird"), "got: {md}");
    }

    #[test]
    fn scaling_doubles_quantities_and_leaves_prose_alone() {
        let r = extract(JSON_LD_PAGE, None, 2.0, true).unwrap();
        assert_eq!(
            r.ingredients,
            vec![
                "4 cups flour",
                "3 tsp cinnamon",
                "6 apples, peeled",
                "1 cup butter",
            ]
        );
        let md = render(&r, Format::Markdown);
        assert!(md.contains("Ingredients scaled 2x from the published recipe"), "got: {md}");
        // Steps are never rewritten — 45 minutes stays 45 minutes.
        assert!(md.contains("bake for 45 minutes"), "got: {md}");
    }

    #[test]
    fn scaling_handles_ranges_and_unicode_fractions() {
        let r = extract(SECTIONED_PAGE, None, 2.0, true).unwrap();
        assert_eq!(
            r.ingredients,
            vec!["2 chicken", "4-6 cloves garlic", "1 lemon"]
        );
    }

    // -- unit-level behaviour ---------------------------------------------

    #[test]
    fn scale_ingredient_covers_the_quantity_forms() {
        assert_eq!(scale_ingredient("2 cups flour", 2.0), "4 cups flour");
        assert_eq!(scale_ingredient("1/2 cup butter", 2.0), "1 cup butter");
        assert_eq!(scale_ingredient("1 1/2 tsp salt", 2.0), "3 tsp salt");
        assert_eq!(scale_ingredient("1 1/2 tsp salt", 0.5), "3/4 tsp salt");
        assert_eq!(scale_ingredient("1.5 kg beef", 2.0), "3 kg beef");
        assert_eq!(scale_ingredient("½ lemon", 3.0), "1 1/2 lemon");
        assert_eq!(scale_ingredient("1½ cups milk", 2.0), "3 cups milk");
        assert_eq!(scale_ingredient("2 to 3 cloves garlic", 2.0), "4 to 6 cloves garlic");
        assert_eq!(scale_ingredient("2-3 cloves garlic", 3.0), "6-9 cloves garlic");
        // No leading quantity → untouched, including "1-inch" style prose.
        assert_eq!(scale_ingredient("Salt to taste", 4.0), "Salt to taste");
        assert_eq!(scale_ingredient("1-inch piece ginger", 2.0), "2-inch piece ginger");
        // Leading whitespace is preserved.
        assert_eq!(scale_ingredient("  2 eggs", 2.0), "  4 eggs");
    }

    #[test]
    fn format_qty_prefers_cook_friendly_fractions() {
        assert_eq!(format_qty(4.0), "4");
        assert_eq!(format_qty(0.5), "1/2");
        assert_eq!(format_qty(1.5), "1 1/2");
        assert_eq!(format_qty(0.75), "3/4");
        assert_eq!(format_qty(2.0 / 3.0), "2/3");
        assert_eq!(format_qty(1.4), "1.4");
    }

    #[test]
    fn humanize_duration_reads_iso_and_passes_prose_through() {
        assert_eq!(humanize_duration("PT30M").as_deref(), Some("30 min"));
        assert_eq!(humanize_duration("PT1H30M").as_deref(), Some("1 hr 30 min"));
        assert_eq!(humanize_duration("PT2H").as_deref(), Some("2 hr"));
        assert_eq!(humanize_duration("P1DT2H").as_deref(), Some("1 day 2 hr"));
        assert_eq!(humanize_duration("PT90S").as_deref(), Some("2 min"));
        assert_eq!(humanize_duration("45 minutes").as_deref(), Some("45 minutes"));
        assert_eq!(humanize_duration("PT0M"), None);
        assert_eq!(humanize_duration("   "), None);
    }

    #[test]
    fn clean_text_strips_tags_and_decodes_entities() {
        assert_eq!(clean_text("Salt &amp; <b>pepper</b>"), "Salt & pepper");
        assert_eq!(clean_text("  spaced\n  out  "), "spaced out");
    }

    // -- error paths -------------------------------------------------------

    #[test]
    fn page_without_recipe_markup_is_a_clear_error() {
        let err = extract(
            "<html><body><h1>Just a blog post</h1><p>No markup here.</p></body></html>",
            None,
            1.0,
            true,
        )
        .unwrap_err();
        assert!(err.contains("no recipe markup found"), "got: {err}");
        assert!(err.contains("print"), "error should suggest a workaround: {err}");
    }

    #[test]
    fn recipe_markup_without_content_is_a_clear_error() {
        let stub = r#"<html><head><script type="application/ld+json">
            {"@context":"https://schema.org","@type":"Recipe","name":"Empty"}
        </script></head><body></body></html>"#;
        let err = extract(stub, None, 1.0, true).unwrap_err();
        assert!(err.contains("no ingredients and no steps"), "got: {err}");
    }

    #[test]
    fn invalid_format_is_rejected() {
        let err = Format::parse(Some("yaml")).unwrap_err();
        assert!(err.contains("expected one of markdown, text, json"), "got: {err}");
        assert_eq!(Format::parse(None).unwrap(), Format::Markdown);
        assert_eq!(Format::parse(Some(" ")).unwrap(), Format::Markdown);
    }

    #[test]
    fn arg_validation_rejects_out_of_range_values() {
        assert_eq!(parse_scale(None).unwrap(), 1.0);
        assert_eq!(parse_scale(Some(0.5)).unwrap(), 0.5);
        assert!(parse_scale(Some(0.0)).unwrap_err().contains("invalid scale"));
        assert!(parse_scale(Some(-2.0)).unwrap_err().contains("invalid scale"));
        assert!(parse_scale(Some(1000.0)).unwrap_err().contains("invalid scale"));
        assert!(parse_scale(Some(f64::NAN)).is_err());

        assert_eq!(parse_max_bytes(None).unwrap(), 4 << 20);
        assert_eq!(parse_max_bytes(Some(MAX_HTML_BYTES)).unwrap(), MAX_HTML_BYTES);
        assert!(parse_max_bytes(Some(0)).is_err());
        assert!(parse_max_bytes(Some(MAX_HTML_BYTES + 1)).is_err());

        assert_eq!(parse_url(" https://example.com/x ").unwrap(), "https://example.com/x");
        assert!(parse_url("example.com/x").unwrap_err().contains("invalid url"));
        assert!(parse_url("file:///etc/passwd").unwrap_err().contains("invalid url"));
    }
}
