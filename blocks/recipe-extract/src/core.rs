//! recipe-extract core — pure compute: turn a recipe page's HTML into a clean,
//! structured recipe (title, ingredients, steps, times, yield, nutrition) and
//! render it as Markdown, plain text or JSON.
//!
//! The engine reads the recipe the page already publishes for search engines:
//! schema.org/Recipe markup, first as JSON-LD (`<script
//! type="application/ld+json">`, including `@graph` wrappers), then as
//! microdata (`itemtype="…schema.org/Recipe"` + `itemprop` attributes). That is
//! why the blog story, ads, comments and newsletter boxes never appear in the
//! output: they are not part of the markup we read.
//!
//! No wafer/network deps here — this module is native-testable and wasm-safe
//! (`scraper`/html5ever only).

use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use serde_json::Value;

/// Recursion cap when hunting for a Recipe node inside a JSON-LD document.
const MAX_JSON_DEPTH: usize = 12;

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Headed Markdown: `# Title`, bullet ingredients, numbered steps.
    Markdown,
    /// Plain text, no Markdown punctuation.
    Text,
    /// The full structured record as pretty-printed JSON.
    Json,
}

impl Format {
    /// Parse the user-facing string form. `None`/empty defaults to `Markdown`.
    pub fn parse(s: Option<&str>) -> Result<Self, String> {
        match s.map(str::trim) {
            None | Some("") | Some("markdown") => Ok(Self::Markdown),
            Some("text") => Ok(Self::Text),
            Some("json") => Ok(Self::Json),
            Some(other) => Err(format!(
                "invalid format `{other}`: expected one of markdown, text, json"
            )),
        }
    }

    /// Canonical string form (echoed in errors/tests).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Text => "text",
            Self::Json => "json",
        }
    }
}

/// One block of steps. `name` is set for a schema.org `HowToSection`
/// ("For the sauce"); plain recipes have a single unnamed group.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepGroup {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub steps: Vec<String>,
}

/// One nutrition line, already labelled for display.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NutritionFact {
    pub name: String,
    pub value: String,
}

/// The extracted recipe. Serializes directly as the `format="json"` output.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Recipe {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prep_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cook_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time: Option<String>,
    #[serde(rename = "yield", skip_serializing_if = "Option::is_none")]
    pub yields: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuisine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    pub ingredients: Vec<String>,
    pub instructions: Vec<StepGroup>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nutrition: Vec<NutritionFact>,
    /// The multiplier applied to the ingredient quantities (1 = as published).
    pub scale: f64,
    /// Which markup the recipe came from: `json-ld` or `microdata`.
    pub markup: String,
}

/// Extract a recipe from a page's `html`.
///
/// `source` is the page URL (echoed in the output; pass `None` when parsing
/// standalone HTML). `scale` multiplies every ingredient quantity the parser
/// recognizes (1.0 = leave as published). `include_nutrition` keeps the
/// schema.org nutrition block.
///
/// Errors when the page publishes no schema.org/Recipe markup, or when the
/// markup exists but carries neither ingredients nor steps.
pub fn extract(
    html: &str,
    source: Option<&str>,
    scale: f64,
    include_nutrition: bool,
) -> Result<Recipe, String> {
    let doc = Html::parse_document(html);
    let mut recipe = from_json_ld(&doc)
        .or_else(|| from_microdata(&doc))
        .ok_or_else(|| {
            "no recipe markup found: the page publishes no schema.org/Recipe data \
             (JSON-LD or microdata). Recipes rendered only by JavaScript, or behind \
             a paywall or bot check, cannot be read this way — try the site's \
             print/printer-friendly version of the page."
                .to_string()
        })?;

    if recipe.ingredients.is_empty() && recipe.instructions.is_empty() {
        return Err(
            "found schema.org/Recipe markup, but it lists no ingredients and no steps \
             (the page may only publish a recipe stub, e.g. a roundup or category page)"
                .to_string(),
        );
    }
    if recipe.title.is_empty() {
        recipe.title = "Recipe".to_string();
    }
    if !include_nutrition {
        recipe.nutrition.clear();
    }
    if (scale - 1.0).abs() > 1e-9 {
        recipe.ingredients = recipe
            .ingredients
            .iter()
            .map(|i| scale_ingredient(i, scale))
            .collect();
    }
    recipe.scale = scale;
    recipe.source = source.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    Ok(recipe)
}

// ---------------------------------------------------------------------------
// JSON-LD path
// ---------------------------------------------------------------------------

fn from_json_ld(doc: &Html) -> Option<Recipe> {
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    for el in doc.select(&sel) {
        let raw = el.text().collect::<String>();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Some sites emit raw newlines/tabs inside JSON strings, which is invalid
        // JSON. Outside strings that whitespace is insignificant, so retrying with
        // control characters flattened recovers those pages without affecting
        // well-formed ones.
        let parsed = serde_json::from_str::<Value>(trimmed).or_else(|_| {
            let relaxed: String = trimmed
                .chars()
                .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
                .collect();
            serde_json::from_str::<Value>(&relaxed)
        });
        let Ok(value) = parsed else { continue };
        if let Some(node) = find_recipe(&value, 0) {
            return Some(recipe_from_node(node));
        }
    }
    None
}

/// Depth-first hunt for the first `@type: Recipe` object (handles top-level
/// arrays, `@graph` wrappers and recipes nested under `mainEntity`).
fn find_recipe(v: &Value, depth: usize) -> Option<&Value> {
    if depth > MAX_JSON_DEPTH {
        return None;
    }
    match v {
        Value::Array(items) => items.iter().find_map(|i| find_recipe(i, depth + 1)),
        Value::Object(map) => {
            if has_type(map.get("@type"), "Recipe") {
                return Some(v);
            }
            map.values()
                .filter(|val| val.is_array() || val.is_object())
                .find_map(|val| find_recipe(val, depth + 1))
        }
        _ => None,
    }
}

/// `@type` may be a string or an array; values may be bare (`Recipe`) or
/// prefixed (`http://schema.org/Recipe`).
fn has_type(v: Option<&Value>, want: &str) -> bool {
    fn matches(s: &str, want: &str) -> bool {
        s.rsplit('/').next().unwrap_or(s).eq_ignore_ascii_case(want)
    }
    match v {
        Some(Value::String(s)) => matches(s, want),
        Some(Value::Array(items)) => items
            .iter()
            .any(|i| i.as_str().map(|s| matches(s, want)).unwrap_or(false)),
        _ => false,
    }
}

fn recipe_from_node(node: &Value) -> Recipe {
    Recipe {
        title: text_field(node.get("name")).unwrap_or_default(),
        source: None,
        author: text_field(node.get("author")),
        description: text_field(node.get("description")),
        image: url_field(node.get("image")),
        prep_time: node
            .get("prepTime")
            .and_then(text_field_raw)
            .and_then(|s| humanize_duration(&s)),
        cook_time: node
            .get("cookTime")
            .and_then(text_field_raw)
            .and_then(|s| humanize_duration(&s)),
        total_time: node
            .get("totalTime")
            .and_then(text_field_raw)
            .and_then(|s| humanize_duration(&s)),
        yields: yield_field(node.get("recipeYield")),
        category: joined_field(node.get("recipeCategory")),
        cuisine: joined_field(node.get("recipeCuisine")),
        rating: rating_field(node.get("aggregateRating")),
        ingredients: string_list(
            node.get("recipeIngredient").or_else(|| node.get("ingredients")),
        ),
        instructions: parse_instructions(node.get("recipeInstructions")),
        nutrition: nutrition_facts(node.get("nutrition")),
        scale: 1.0,
        markup: "json-ld".to_string(),
    }
}

/// A scalar-ish JSON value as display text (entities decoded, tags stripped).
fn text_field(v: Option<&Value>) -> Option<String> {
    text_field_raw(v?).map(|s| clean_text(&s)).filter(|s| !s.is_empty())
}

/// The same, without the HTML clean-up (used for machine values like durations).
fn text_field_raw(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(items) => items.iter().find_map(text_field_raw),
        Value::Object(map) => ["name", "text", "@value", "headline", "url"]
            .iter()
            .find_map(|k| map.get(*k).and_then(text_field_raw)),
        _ => None,
    }
}

fn url_field(v: Option<&Value>) -> Option<String> {
    fn first_url(v: &Value) -> Option<String> {
        match v {
            Value::String(s) => Some(s.trim().to_string()),
            Value::Array(items) => items.iter().find_map(first_url),
            Value::Object(map) => map
                .get("url")
                .or_else(|| map.get("contentUrl"))
                .and_then(first_url),
            _ => None,
        }
    }
    first_url(v?).filter(|s| !s.is_empty())
}

/// `recipeCategory`/`recipeCuisine` are a string or a list of strings.
fn joined_field(v: Option<&Value>) -> Option<String> {
    let items = string_list(v);
    if items.is_empty() {
        None
    } else {
        Some(items.join(", "))
    }
}

/// `recipeYield` is a string ("12 slices"), a number (4 = servings) or a list.
fn yield_field(v: Option<&Value>) -> Option<String> {
    let raw = match v? {
        Value::Array(items) => items.iter().find_map(text_field_raw)?,
        other => text_field_raw(other)?,
    };
    let text = clean_text(&raw);
    if text.is_empty() {
        return None;
    }
    if text.chars().all(|c| c.is_ascii_digit()) {
        let plural = if text == "1" { "serving" } else { "servings" };
        return Some(format!("{text} {plural}"));
    }
    Some(text)
}

fn rating_field(v: Option<&Value>) -> Option<String> {
    let map = v?.as_object()?;
    let value = text_field_raw(map.get("ratingValue")?)?;
    let value = clean_text(&value);
    if value.is_empty() {
        return None;
    }
    match map.get("ratingCount").and_then(text_field_raw) {
        Some(count) => Some(format!("{value} ({} ratings)", clean_text(&count))),
        None => Some(value),
    }
}

/// `recipeIngredient` (and friends) as a flat list of clean strings.
fn string_list(v: Option<&Value>) -> Vec<String> {
    fn push_value(out: &mut Vec<String>, v: &Value) {
        match v {
            Value::String(s) => {
                for line in s.split('\n') {
                    let cleaned = clean_text(line);
                    if !cleaned.is_empty() {
                        out.push(cleaned);
                    }
                }
            }
            Value::Number(n) => out.push(n.to_string()),
            Value::Array(items) => items.iter().for_each(|i| push_value(out, i)),
            Value::Object(_) => {
                if let Some(t) = text_field(Some(v)) {
                    out.push(t);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Some(v) = v {
        push_value(&mut out, v);
    }
    out
}

/// `recipeInstructions` in every shape schema.org allows: one string (possibly
/// HTML), a list of strings, a list of `HowToStep`s, or `HowToSection`s that
/// each carry their own `itemListElement` steps.
fn parse_instructions(v: Option<&Value>) -> Vec<StepGroup> {
    let Some(v) = v else { return Vec::new() };
    let mut groups: Vec<StepGroup> = Vec::new();
    let mut loose: Vec<String> = Vec::new();

    match v {
        Value::String(s) => loose.extend(steps_from_html(s)),
        Value::Array(items) => {
            for item in items {
                if has_type(item.get("@type"), "HowToSection")
                    || (item.get("itemListElement").is_some() && item.get("text").is_none())
                {
                    let steps = collect_steps(item.get("itemListElement"));
                    if steps.is_empty() {
                        continue;
                    }
                    if !loose.is_empty() {
                        groups.push(StepGroup {
                            name: None,
                            steps: std::mem::take(&mut loose),
                        });
                    }
                    groups.push(StepGroup {
                        name: text_field(item.get("name")),
                        steps,
                    });
                } else {
                    loose.extend(step_text(item));
                }
            }
        }
        other => loose.extend(step_text(other)),
    }

    if !loose.is_empty() {
        groups.push(StepGroup {
            name: None,
            steps: loose,
        });
    }
    groups
}

fn collect_steps(v: Option<&Value>) -> Vec<String> {
    let Some(v) = v else { return Vec::new() };
    match v {
        Value::Array(items) => items.iter().flat_map(step_text).collect(),
        other => step_text(other),
    }
}

/// One instruction entry → zero or more steps.
fn step_text(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => steps_from_html(s),
        Value::Object(map) => {
            if let Some(nested) = map.get("itemListElement") {
                return collect_steps(Some(nested));
            }
            map.get("text")
                .or_else(|| map.get("name"))
                .and_then(|t| t.as_str().map(steps_from_html))
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Split one instruction string into steps: `<li>`/`<p>` items when it carries
/// HTML, otherwise its non-empty lines.
fn steps_from_html(s: &str) -> Vec<String> {
    if s.contains('<') {
        let frag = Html::parse_fragment(s);
        for tag in ["li", "p"] {
            if let Ok(sel) = Selector::parse(tag) {
                let items: Vec<String> = frag
                    .select(&sel)
                    .map(|e| collapse_ws(&e.text().collect::<String>()))
                    .filter(|t| !t.is_empty())
                    .collect();
                if !items.is_empty() {
                    return items;
                }
            }
        }
    }
    let cleaned = clean_text_preserving_lines(s);
    cleaned
        .lines()
        .map(collapse_ws)
        .filter(|l| !l.is_empty())
        .collect()
}

fn nutrition_facts(v: Option<&Value>) -> Vec<NutritionFact> {
    /// schema.org `NutritionInformation` fields, in the order cooks read them.
    /// `true` = the spec expresses the value in grams, so a bare number gets a
    /// `g` unit; `calories` gets `kcal`.
    const FIELDS: &[(&str, &str, bool)] = &[
        ("calories", "Calories", false),
        ("servingSize", "Serving size", false),
        ("carbohydrateContent", "Carbohydrates", true),
        ("proteinContent", "Protein", true),
        ("fatContent", "Fat", true),
        ("saturatedFatContent", "Saturated fat", true),
        ("unsaturatedFatContent", "Unsaturated fat", true),
        ("transFatContent", "Trans fat", true),
        ("fiberContent", "Fiber", true),
        ("sugarContent", "Sugar", true),
        ("cholesterolContent", "Cholesterol", false),
        ("sodiumContent", "Sodium", false),
    ];
    let Some(map) = v.and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, label, grams) in FIELDS {
        let Some(raw) = map.get(*key).and_then(text_field_raw) else {
            continue;
        };
        let value = clean_text(&raw);
        if value.is_empty() {
            continue;
        }
        let numeric = value.parse::<f64>().is_ok();
        let value = match (numeric, *key, *grams) {
            (true, "calories", _) => format!("{value} kcal"),
            (true, _, true) => format!("{value} g"),
            _ => value,
        };
        out.push(NutritionFact {
            name: (*label).to_string(),
            value,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Microdata fallback
// ---------------------------------------------------------------------------

/// Best-effort microdata reader for pages that predate JSON-LD: find the
/// `itemtype="…schema.org/Recipe"` scope and read its `itemprop` descendants.
fn from_microdata(doc: &Html) -> Option<Recipe> {
    let scope_sel = Selector::parse("[itemtype]").ok()?;
    let scope = doc.select(&scope_sel).find(|e| {
        e.value()
            .attr("itemtype")
            .map(|t| t.to_ascii_lowercase().contains("schema.org/recipe"))
            .unwrap_or(false)
    })?;
    let prop_sel = Selector::parse("[itemprop]").ok()?;

    let mut recipe = Recipe {
        markup: "microdata".to_string(),
        scale: 1.0,
        ..Recipe::default()
    };
    let mut steps: Vec<String> = Vec::new();
    let mut nutrition: Vec<NutritionFact> = Vec::new();

    for el in scope.select(&prop_sel) {
        let Some(prop) = el.value().attr("itemprop") else {
            continue;
        };
        let value = microdata_value(&el);
        if value.is_empty() {
            continue;
        }
        match prop.to_ascii_lowercase().as_str() {
            "name" if recipe.title.is_empty() => recipe.title = value,
            "recipeingredient" | "ingredients" => recipe.ingredients.push(value),
            "recipeinstructions" | "instructions" => steps.extend(steps_from_element(&el)),
            "preptime" => recipe.prep_time = humanize_duration(&value),
            "cooktime" => recipe.cook_time = humanize_duration(&value),
            "totaltime" => recipe.total_time = humanize_duration(&value),
            "recipeyield" | "yield" => recipe.yields = Some(value),
            "author" if recipe.author.is_none() => recipe.author = Some(value),
            "description" if recipe.description.is_none() => recipe.description = Some(value),
            "image" | "photo" if recipe.image.is_none() => recipe.image = Some(value),
            "recipecategory" if recipe.category.is_none() => recipe.category = Some(value),
            "recipecuisine" if recipe.cuisine.is_none() => recipe.cuisine = Some(value),
            "calories" => nutrition.push(NutritionFact {
                name: "Calories".into(),
                value,
            }),
            _ => {}
        }
    }
    if !steps.is_empty() {
        recipe.instructions.push(StepGroup {
            name: None,
            steps,
        });
    }
    recipe.nutrition = nutrition;
    Some(recipe)
}

/// The displayed value of a microdata property: machine-readable attributes win
/// over the element's text (`<time datetime="PT30M">half an hour</time>`).
fn microdata_value(el: &ElementRef) -> String {
    let v = el.value();
    let tag = el.value().name();
    let raw = v
        .attr("content")
        .or_else(|| v.attr("datetime"))
        .map(str::to_string)
        .or_else(|| match tag {
            "img" => v.attr("src").map(str::to_string),
            "a" | "link" => v.attr("href").map(str::to_string),
            "meta" => v.attr("content").map(str::to_string),
            _ => None,
        })
        .unwrap_or_else(|| el.text().collect::<String>());
    collapse_ws(&raw)
}

/// A microdata instructions container: its `<li>` items, else its lines.
fn steps_from_element(el: &ElementRef) -> Vec<String> {
    if let Ok(sel) = Selector::parse("li") {
        let items: Vec<String> = el
            .select(&sel)
            .map(|e| collapse_ws(&e.text().collect::<String>()))
            .filter(|t| !t.is_empty())
            .collect();
        if !items.is_empty() {
            return items;
        }
    }
    el.text()
        .collect::<String>()
        .lines()
        .map(collapse_ws)
        .filter(|l| !l.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Decode entities, strip tags and collapse whitespace to one line.
pub fn clean_text(s: &str) -> String {
    collapse_ws(&clean_text_preserving_lines(s))
}

/// Decode entities and strip tags, keeping line breaks (so a multi-line
/// instruction string can still be split into steps).
fn clean_text_preserving_lines(s: &str) -> String {
    if s.contains('<') || s.contains('&') {
        Html::parse_fragment(s).root_element().text().collect::<String>()
    } else {
        s.to_string()
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// ISO-8601 duration (`PT1H30M`) → `1 hr 30 min`. Non-ISO values (some sites
/// write `30 minutes`) pass through cleaned; a zero/blank duration is `None`.
pub fn humanize_duration(s: &str) -> Option<String> {
    let trimmed = collapse_ws(s);
    if trimmed.is_empty() {
        return None;
    }
    let upper = trimmed.to_ascii_uppercase();
    if !upper.starts_with('P') {
        return Some(trimmed);
    }
    let mut minutes = 0f64;
    let mut num = String::new();
    let mut in_time = false;
    for c in upper.chars().skip(1) {
        match c {
            'T' => in_time = true,
            '0'..='9' | '.' => num.push(c),
            ',' => num.push('.'),
            unit => {
                let n: f64 = num.parse().unwrap_or(0.0);
                num.clear();
                minutes += match unit {
                    'W' => n * 7.0 * 24.0 * 60.0,
                    'D' => n * 24.0 * 60.0,
                    'H' => n * 60.0,
                    'M' if in_time => n,
                    // A month before `T` is nonsense in a recipe; ignore it.
                    'M' => 0.0,
                    'S' => n / 60.0,
                    _ => 0.0,
                };
            }
        }
    }
    let total = minutes.round() as i64;
    if total <= 0 {
        return None;
    }
    let days = total / 1440;
    let hours = (total % 1440) / 60;
    let mins = total % 60;
    let mut parts: Vec<String> = Vec::new();
    if days > 0 {
        parts.push(format!("{days} day{}", if days == 1 { "" } else { "s" }));
    }
    if hours > 0 {
        parts.push(format!("{hours} hr"));
    }
    if mins > 0 {
        parts.push(format!("{mins} min"));
    }
    Some(parts.join(" "))
}

// ---------------------------------------------------------------------------
// Ingredient scaling
// ---------------------------------------------------------------------------

/// Unicode vulgar fractions cooks' sites actually use.
const VULGAR: &[(char, f64)] = &[
    ('½', 0.5),
    ('⅓', 1.0 / 3.0),
    ('⅔', 2.0 / 3.0),
    ('¼', 0.25),
    ('¾', 0.75),
    ('⅕', 0.2),
    ('⅖', 0.4),
    ('⅗', 0.6),
    ('⅘', 0.8),
    ('⅙', 1.0 / 6.0),
    ('⅚', 5.0 / 6.0),
    ('⅛', 0.125),
    ('⅜', 0.375),
    ('⅝', 0.625),
    ('⅞', 0.875),
];

/// Multiply the leading quantity of one ingredient line by `factor`.
///
/// Handles whole numbers, decimals, `1/2`, `1 1/2`, `1½`, `½` and ranges
/// (`2-3`, `2 to 3`). A line with no leading quantity ("Salt to taste") is
/// returned unchanged, as is the unit/ingredient text after the number.
pub fn scale_ingredient(line: &str, factor: f64) -> String {
    let trimmed = line.trim_start();
    let lead = &line[..line.len() - trimmed.len()];
    let Some((first, mut consumed)) = read_quantity(trimmed) else {
        return line.to_string();
    };
    let mut out = format_qty(first * factor);

    // Optional range: "2-3 cloves", "2 to 3 cloves".
    let rest = &trimmed[consumed..];
    let gap = rest.len() - rest.trim_start().len();
    let after_gap = &rest[gap..];
    let separator: Option<(String, usize)> = if after_gap.starts_with("to ") {
        Some((" to ".to_string(), 3))
    } else {
        after_gap
            .chars()
            .next()
            .filter(|c| matches!(c, '-' | '\u{2013}' | '\u{2014}'))
            .map(|c| (c.to_string(), c.len_utf8()))
    };
    if let Some((sep_text, sep_len)) = separator {
        let tail = &after_gap[sep_len..];
        let pad = tail.len() - tail.trim_start().len();
        if let Some((second, second_len)) = read_quantity(tail.trim_start()) {
            out.push_str(&sep_text);
            out.push_str(&format_qty(second * factor));
            consumed += gap + sep_len + pad + second_len;
        }
    }
    format!("{lead}{out}{}", &trimmed[consumed..])
}

/// Read a leading quantity, returning its value and the bytes consumed.
///
/// Recognizes `2`, `1.5`, `1/2`, `1 1/2`, `1½`, `1 ½` and `½`.
fn read_quantity(s: &str) -> Option<(f64, usize)> {
    let bytes = s.as_bytes();
    // Leading integer/decimal run (a `.` only counts between digits).
    let mut i = 0;
    while i < bytes.len()
        && (bytes[i].is_ascii_digit()
            || (bytes[i] == b'.'
                && i > 0
                && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)))
    {
        i += 1;
    }
    let first: Option<f64> = if i > 0 { s[..i].parse::<f64>().ok() } else { None };

    // "1/2" — the digits just read are a numerator, not a whole part.
    if let Some(numerator) = first {
        if bytes.get(i) == Some(&b'/') {
            if let Some((den, len)) = read_digits(&s[i + 1..]) {
                if den != 0.0 {
                    return Some((numerator / den, i + 1 + len));
                }
            }
        }
    }

    let mut value = first.unwrap_or(0.0);
    let rest = &s[i..];
    let gap = rest.len() - rest.trim_start().len();
    let tail = &rest[gap..];

    // "1 1/2" — whole part, space, fraction.
    if first.is_some() && gap > 0 {
        if let Some((numerator, num_len)) = read_digits(tail) {
            if tail.as_bytes().get(num_len) == Some(&b'/') {
                if let Some((den, den_len)) = read_digits(&tail[num_len + 1..]) {
                    if den != 0.0 {
                        return Some((
                            value + numerator / den,
                            i + gap + num_len + 1 + den_len,
                        ));
                    }
                }
            }
        }
    }

    // "½", "1½", "1 ½".
    if let Some(c) = tail.chars().next() {
        if let Some((_, v)) = VULGAR.iter().find(|(ch, _)| *ch == c) {
            value += v;
            return Some((value, i + gap + c.len_utf8()));
        }
    }

    first.map(|n| (n, i))
}

/// Read a run of ASCII digits at the start of `s` → (value, bytes consumed).
fn read_digits(s: &str) -> Option<(f64, usize)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    s[..i].parse::<f64>().ok().map(|v| (v, i))
}

/// Render a scaled quantity the way a recipe would write it: whole numbers
/// plain, common fractions as fractions, everything else to two decimals.
pub fn format_qty(v: f64) -> String {
    const FRACTIONS: &[(f64, &str)] = &[
        (0.125, "1/8"),
        (1.0 / 6.0, "1/6"),
        (0.25, "1/4"),
        (1.0 / 3.0, "1/3"),
        (0.375, "3/8"),
        (0.5, "1/2"),
        (0.625, "5/8"),
        (2.0 / 3.0, "2/3"),
        (0.75, "3/4"),
        (5.0 / 6.0, "5/6"),
        (0.875, "7/8"),
    ];
    if !v.is_finite() || v <= 0.0 {
        return trim_float(v);
    }
    let whole = v.floor();
    let frac = v - whole;
    if frac < 0.01 {
        return format!("{}", whole as i64);
    }
    if frac > 0.99 {
        return format!("{}", whole as i64 + 1);
    }
    for (value, label) in FRACTIONS {
        if (frac - value).abs() < 0.015 {
            return if whole >= 1.0 {
                format!("{} {label}", whole as i64)
            } else {
                (*label).to_string()
            };
        }
    }
    trim_float(v)
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render an extracted recipe in the requested format.
pub fn render(recipe: &Recipe, format: Format) -> String {
    match format {
        Format::Markdown => render_markdown(recipe),
        Format::Text => render_text(recipe),
        Format::Json => {
            serde_json::to_string_pretty(recipe).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        }
    }
}

/// The one-line facts strip: `Prep 20 min | Cook 45 min | Total 1 hr 5 min | Yield: 12 slices`.
fn meta_line(recipe: &Recipe, separator: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = &recipe.prep_time {
        parts.push(format!("Prep {v}"));
    }
    if let Some(v) = &recipe.cook_time {
        parts.push(format!("Cook {v}"));
    }
    if let Some(v) = &recipe.total_time {
        parts.push(format!("Total {v}"));
    }
    if let Some(v) = &recipe.yields {
        parts.push(format!("Yield: {v}"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(separator))
    }
}

fn scale_note(recipe: &Recipe) -> Option<String> {
    if (recipe.scale - 1.0).abs() < 1e-9 {
        return None;
    }
    Some(format!(
        "Ingredients scaled {}x from the published recipe (times and steps unchanged).",
        trim_float(recipe.scale)
    ))
}

fn render_markdown(recipe: &Recipe) -> String {
    let mut out = format!("# {}\n", recipe.title);
    if let Some(author) = &recipe.author {
        out.push_str(&format!("\nBy {author}\n"));
    }
    if let Some(source) = &recipe.source {
        out.push_str(&format!("\nSource: {source}\n"));
    }
    if let Some(meta) = meta_line(recipe, " · ") {
        out.push_str(&format!("\n{meta}\n"));
    }
    if let Some(note) = scale_note(recipe) {
        out.push_str(&format!("\n{note}\n"));
    }
    if !recipe.ingredients.is_empty() {
        out.push_str("\n## Ingredients\n\n");
        for item in &recipe.ingredients {
            out.push_str(&format!("- {item}\n"));
        }
    }
    if !recipe.instructions.is_empty() {
        out.push_str("\n## Instructions\n");
        for group in &recipe.instructions {
            if let Some(name) = &group.name {
                out.push_str(&format!("\n### {name}\n"));
            }
            out.push('\n');
            for (i, step) in group.steps.iter().enumerate() {
                out.push_str(&format!("{}. {step}\n", i + 1));
            }
        }
    }
    if !recipe.nutrition.is_empty() {
        out.push_str("\n## Nutrition\n\n");
        for fact in &recipe.nutrition {
            out.push_str(&format!("- {}: {}\n", fact.name, fact.value));
        }
    }
    out
}

fn render_text(recipe: &Recipe) -> String {
    let mut out = format!("{}\n", recipe.title);
    if let Some(author) = &recipe.author {
        out.push_str(&format!("By {author}\n"));
    }
    if let Some(source) = &recipe.source {
        out.push_str(&format!("Source: {source}\n"));
    }
    if let Some(meta) = meta_line(recipe, " | ") {
        out.push_str(&format!("{meta}\n"));
    }
    if let Some(note) = scale_note(recipe) {
        out.push_str(&format!("{note}\n"));
    }
    if !recipe.ingredients.is_empty() {
        out.push_str("\nINGREDIENTS\n");
        for item in &recipe.ingredients {
            out.push_str(&format!("- {item}\n"));
        }
    }
    if !recipe.instructions.is_empty() {
        out.push_str("\nINSTRUCTIONS\n");
        for group in &recipe.instructions {
            if let Some(name) = &group.name {
                out.push_str(&format!("\n{name}\n"));
            }
            for (i, step) in group.steps.iter().enumerate() {
                out.push_str(&format!("{}. {step}\n", i + 1));
            }
        }
    }
    if !recipe.nutrition.is_empty() {
        out.push_str("\nNUTRITION\n");
        for fact in &recipe.nutrition {
            out.push_str(&format!("- {}: {}\n", fact.name, fact.value));
        }
    }
    out
}
