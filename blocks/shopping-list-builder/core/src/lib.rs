//! shopping-list-builder core — aggregate pasted recipe ingredient lines into a
//! deduplicated shopping list. Pure Rust, no network or filesystem access.

pub const MAX_INPUT_BYTES: usize = 200_000;
pub const MAX_LINES: usize = 5_000;
pub const MAX_ITEMS: usize = 2_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GroupBy {
    Category,
    Recipe,
    None,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnitSystem {
    Keep,
    Metric,
    Us,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Format {
    Markdown,
    Text,
    Csv,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Family {
    Volume,
    Weight,
    Count,
    Unknown,
}

#[derive(Clone, Debug)]
struct Unit {
    canon: &'static str,
    family: Family,
    to_base: f64,
}

#[derive(Clone, Debug)]
struct Ingredient {
    qty: Option<f64>,
    unit: Option<Unit>,
    display_unit: String,
    name: String,
    recipe: String,
}

#[derive(Clone, Debug)]
struct Item {
    key: String,
    name: String,
    qty_base: Option<f64>,
    family: Family,
    unit: String,
    sources: Vec<String>,
}

pub fn run(
    ingredients: &str,
    scale: f64,
    group_by: &str,
    unit_system: &str,
    exclude: &str,
    checkboxes: bool,
    show_sources: bool,
    format: &str,
) -> Result<String, String> {
    if ingredients.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "ingredients input is too large: {} bytes, limit is {} bytes",
            ingredients.len(),
            MAX_INPUT_BYTES
        ));
    }
    if !(0.1..=20.0).contains(&scale) || !scale.is_finite() {
        return Err(format!(
            "invalid scale {scale}: expected a number from 0.1 to 20"
        ));
    }
    let group = parse_group(group_by)?;
    let system = parse_system(unit_system)?;
    let fmt = parse_format(format)?;
    let line_count = ingredients.lines().count();
    if line_count > MAX_LINES {
        return Err(format!(
            "too many ingredient lines: {line_count}, limit is {MAX_LINES}"
        ));
    }
    let excludes = split_list(exclude);
    let parsed = parse_ingredients(ingredients, scale)?;
    if parsed.is_empty() {
        return Err("no ingredient lines found: paste lines like `2 cups flour` or `# Soup x2` followed by ingredients".to_string());
    }
    let mut items = merge_items(parsed, &excludes)?;
    if items.len() > MAX_ITEMS {
        return Err(format!(
            "too many distinct shopping items: {}, limit is {MAX_ITEMS}",
            items.len()
        ));
    }
    items.sort_by(|a, b| {
        category(&a.name)
            .cmp(category(&b.name))
            .then(a.name.cmp(&b.name))
            .then(a.unit.cmp(&b.unit))
    });
    Ok(match fmt {
        Format::Markdown => render_markdown(&items, group, system, checkboxes, show_sources),
        Format::Text => render_text(&items, group, system, show_sources),
        Format::Csv => render_csv(&items, system, show_sources),
        Format::Json => render_json(&items, system),
    })
}

fn parse_group(s: &str) -> Result<GroupBy, String> {
    match s.trim() {
        "" | "category" => Ok(GroupBy::Category),
        "recipe" => Ok(GroupBy::Recipe),
        "none" => Ok(GroupBy::None),
        other => Err(format!(
            "invalid group_by {other:?}: expected category, recipe or none"
        )),
    }
}
fn parse_system(s: &str) -> Result<UnitSystem, String> {
    match s.trim() {
        "" | "keep" => Ok(UnitSystem::Keep),
        "metric" => Ok(UnitSystem::Metric),
        "us" => Ok(UnitSystem::Us),
        other => Err(format!(
            "invalid unit_system {other:?}: expected keep, metric or us"
        )),
    }
}
fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim() {
        "" | "markdown" => Ok(Format::Markdown),
        "text" => Ok(Format::Text),
        "csv" => Ok(Format::Csv),
        "json" => Ok(Format::Json),
        other => Err(format!(
            "invalid format {other:?}: expected markdown, text, csv or json"
        )),
    }
}

fn split_list(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c == '\n' || c == ';')
        .map(normalize_name)
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_ingredients(input: &str, global_scale: f64) -> Result<Vec<Ingredient>, String> {
    let mut recipe = "Recipe".to_string();
    let mut recipe_scale = 1.0;
    let mut out = Vec::new();
    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() || line == "---" {
            continue;
        }
        if let Some(h) = line.strip_prefix('#') {
            let (name, mult) = parse_recipe_header(h.trim());
            recipe = if name.is_empty() {
                "Recipe".to_string()
            } else {
                name.to_string()
            };
            recipe_scale = mult;
            continue;
        }
        if looks_like_instruction(line) {
            continue;
        }
        let mut ing = parse_line(line);
        if let Some(q) = ing.qty.as_mut() {
            *q *= global_scale * recipe_scale;
        }
        ing.recipe = recipe.clone();
        if !ing.name.is_empty() {
            out.push(ing);
        }
    }
    Ok(out)
}

fn parse_recipe_header(h: &str) -> (&str, f64) {
    let t = h.trim();
    if let Some((name, mult)) = t.rsplit_once('x') {
        if let Ok(v) = mult.trim().parse::<f64>() {
            if v.is_finite() && v > 0.0 {
                return (name.trim(), v);
            }
        }
    }
    if let Some((name, mult)) = t.rsplit_once('×') {
        if let Ok(v) = mult.trim().parse::<f64>() {
            if v.is_finite() && v > 0.0 {
                return (name.trim(), v);
            }
        }
    }
    (t, 1.0)
}

fn looks_like_instruction(line: &str) -> bool {
    let l = line.to_ascii_lowercase();
    let starts = [
        "preheat ", "mix ", "bake ", "cook ", "stir ", "serve ", "heat ", "whisk ", "combine ",
    ];
    starts.iter().any(|p| l.starts_with(p)) && !line.chars().next().unwrap_or('0').is_ascii_digit()
}

fn parse_line(line: &str) -> Ingredient {
    let clean = line
        .trim_start_matches(|c| c == '-' || c == '*' || c == '•')
        .trim();
    let parts: Vec<&str> = clean.split_whitespace().collect();
    let mut idx = 0;
    let mut qty = None;
    if !parts.is_empty() {
        if let Some((v, used)) = parse_quantity(&parts) {
            qty = Some(v);
            idx = used;
        }
    }
    let mut unit = None;
    let mut display_unit = String::new();
    if idx < parts.len() {
        if let Some(u) = unit_for(parts[idx]) {
            display_unit = u.canon.to_string();
            unit = Some(u);
            idx += 1;
        }
    }
    let name = parts[idx..].join(" ");
    Ingredient {
        qty,
        unit,
        display_unit,
        name: tidy_name(&name),
        recipe: String::new(),
    }
}

fn parse_quantity(parts: &[&str]) -> Option<(f64, usize)> {
    let first = trim_amount(parts[0]);
    if let Some((_, hi)) = first.split_once('-').or_else(|| first.split_once('–')) {
        if let Some(v) = parse_single_qty(hi) {
            return Some((v, 1));
        }
    }
    if parts.len() >= 2 {
        if let Some(a) = parse_single_qty(first) {
            if let Some(b) = parse_single_qty(trim_amount(parts[1])) {
                if parts[1].contains('/') || is_unicode_fraction(parts[1]) {
                    return Some((a + b, 2));
                }
            }
        }
    }
    parse_single_qty(first).map(|v| (v, 1))
}
fn trim_amount(s: &str) -> &str {
    s.trim_matches(|c: char| c == '(' || c == ')' || c == '~' || c == ',')
}
fn parse_single_qty(s: &str) -> Option<f64> {
    if let Ok(v) = s.parse::<f64>() {
        return Some(v);
    }
    if let Some((a, b)) = s.split_once('/') {
        let n = a.parse::<f64>().ok()?;
        let d = b.parse::<f64>().ok()?;
        if d != 0.0 {
            return Some(n / d);
        }
    }
    unicode_fraction_value(s)
}
fn is_unicode_fraction(s: &str) -> bool {
    unicode_fraction_value(s).is_some()
}
fn unicode_fraction_value(s: &str) -> Option<f64> {
    match s {
        "¼" => Some(0.25),
        "½" => Some(0.5),
        "¾" => Some(0.75),
        "⅓" => Some(1.0 / 3.0),
        "⅔" => Some(2.0 / 3.0),
        "⅛" => Some(0.125),
        "⅜" => Some(0.375),
        "⅝" => Some(0.625),
        "⅞" => Some(0.875),
        _ => None,
    }
}

fn unit_for(raw: &str) -> Option<Unit> {
    let u = raw
        .trim_matches(|c: char| c == ',' || c == '.')
        .to_ascii_lowercase();
    let u = u.as_str();
    let (canon, family, to_base) = match u {
        "tsp" | "teaspoon" | "teaspoons" => ("tsp", Family::Volume, 4.92892),
        "tbsp" | "tablespoon" | "tablespoons" => ("tbsp", Family::Volume, 14.7868),
        "cup" | "cups" => ("cup", Family::Volume, 236.588),
        "ml" | "milliliter" | "milliliters" | "millilitre" | "millilitres" => {
            ("ml", Family::Volume, 1.0)
        }
        "l" | "liter" | "liters" | "litre" | "litres" => ("l", Family::Volume, 1000.0),
        "floz" | "fl-oz" | "fl" => ("fl oz", Family::Volume, 29.5735),
        "pint" | "pints" | "pt" => ("pint", Family::Volume, 473.176),
        "quart" | "quarts" | "qt" => ("quart", Family::Volume, 946.353),
        "gallon" | "gallons" | "gal" => ("gallon", Family::Volume, 3785.41),
        "g" | "gram" | "grams" => ("g", Family::Weight, 1.0),
        "kg" | "kilogram" | "kilograms" => ("kg", Family::Weight, 1000.0),
        "oz" | "ounce" | "ounces" => ("oz", Family::Weight, 28.3495),
        "lb" | "lbs" | "pound" | "pounds" => ("lb", Family::Weight, 453.592),
        "clove" | "cloves" => ("clove", Family::Count, 1.0),
        "can" | "cans" => ("can", Family::Count, 1.0),
        "jar" | "jars" => ("jar", Family::Count, 1.0),
        "piece" | "pieces" | "item" | "items" => ("item", Family::Count, 1.0),
        _ => return None,
    };
    Some(Unit {
        canon,
        family,
        to_base,
    })
}

fn tidy_name(name: &str) -> String {
    name.trim()
        .trim_matches(',')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn normalize_name(name: &str) -> String {
    let mut s = name.to_ascii_lowercase();
    for sep in [",", "(", ")"] {
        s = s.replace(sep, " ");
    }
    let mut words: Vec<String> = s
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect();
    for w in &mut words {
        if w.len() > 3 && w.ends_with('s') {
            w.pop();
        }
    }
    words.join(" ")
}

fn merge_items(parsed: Vec<Ingredient>, excludes: &[String]) -> Result<Vec<Item>, String> {
    let mut items: Vec<Item> = Vec::new();
    for ing in parsed {
        let name_key = normalize_name(&ing.name);
        if name_key.is_empty() || excludes.iter().any(|e| e == &name_key) {
            continue;
        }
        let family = ing
            .unit
            .as_ref()
            .map(|u| u.family)
            .unwrap_or(Family::Unknown);
        let unit_key = if matches!(family, Family::Unknown) {
            ing.unit.as_ref().map(|u| u.canon).unwrap_or("")
        } else {
            ""
        };
        let key = format!("{}|{}|{}", name_key, family_name(family), unit_key);
        if let Some(item) = items.iter_mut().find(|i| i.key == key) {
            match (item.qty_base.as_mut(), ing.qty, ing.unit.as_ref()) {
                (Some(total), Some(q), Some(u)) => *total += q * u.to_base,
                (Some(total), Some(q), None) => *total += q,
                _ => {}
            }
            if !item.sources.contains(&ing.recipe) {
                item.sources.push(ing.recipe);
            }
        } else {
            let qty_base = match (ing.qty, ing.unit.as_ref()) {
                (Some(q), Some(u)) => Some(q * u.to_base),
                (Some(q), None) => Some(q),
                _ => None,
            };
            items.push(Item {
                key,
                name: ing.name,
                qty_base,
                family,
                unit: ing.display_unit,
                sources: vec![ing.recipe],
            });
        }
    }
    Ok(items)
}
fn family_name(f: Family) -> &'static str {
    match f {
        Family::Volume => "volume",
        Family::Weight => "weight",
        Family::Count => "count",
        Family::Unknown => "unknown",
    }
}

fn render_markdown(
    items: &[Item],
    group: GroupBy,
    system: UnitSystem,
    checkboxes: bool,
    show_sources: bool,
) -> String {
    let mut out = String::from("## Shopping list\n");
    render_grouped(
        items,
        group,
        system,
        show_sources,
        |out, title| {
            out.push_str("\n### ");
            out.push_str(title);
            out.push('\n');
        },
        |out, item| {
            out.push_str(if checkboxes { "- [ ] " } else { "- " });
            out.push_str(&format_item(item, system, show_sources));
            out.push('\n');
        },
        &mut out,
    );
    out.trim_end().to_string()
}
fn render_text(items: &[Item], group: GroupBy, system: UnitSystem, show_sources: bool) -> String {
    let mut out = String::new();
    render_grouped(
        items,
        group,
        system,
        show_sources,
        |out, title| {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(title);
            out.push('\n');
        },
        |out, item| {
            out.push_str("- ");
            out.push_str(&format_item(item, system, show_sources));
            out.push('\n');
        },
        &mut out,
    );
    out.trim_end().to_string()
}
fn render_grouped<FH, FI>(
    items: &[Item],
    group: GroupBy,
    system: UnitSystem,
    show_sources: bool,
    mut header: FH,
    mut line: FI,
    out: &mut String,
) where
    FH: FnMut(&mut String, &str),
    FI: FnMut(&mut String, &Item),
{
    match group {
        GroupBy::None => {
            for item in items {
                line(out, item);
            }
        }
        GroupBy::Category => {
            let cats = [
                "Produce",
                "Dairy",
                "Meat & seafood",
                "Bakery",
                "Pantry",
                "Spices",
                "Frozen",
                "Beverages",
                "Other",
            ];
            for cat in cats {
                let before = out.len();
                let subset: Vec<&Item> =
                    items.iter().filter(|i| category(&i.name) == cat).collect();
                if !subset.is_empty() {
                    header(out, cat);
                    for item in subset {
                        line(out, item);
                    }
                } else {
                    out.truncate(before);
                }
            }
        }
        GroupBy::Recipe => {
            let mut recipes: Vec<String> = Vec::new();
            for i in items {
                for s in &i.sources {
                    if !recipes.contains(s) {
                        recipes.push(s.clone());
                    }
                }
            }
            recipes.sort();
            for r in recipes {
                header(out, &r);
                for item in items.iter().filter(|i| i.sources.contains(&r)) {
                    let _ = system;
                    let _ = show_sources;
                    line(out, item);
                }
            }
        }
    }
}

fn render_csv(items: &[Item], system: UnitSystem, show_sources: bool) -> String {
    let mut out = String::from("category,item,quantity,unit");
    if show_sources {
        out.push_str(",sources");
    }
    for item in items {
        let (q, u) = quantity_unit(item, system);
        out.push('\n');
        out.push_str(&csv_escape(category(&item.name)));
        out.push(',');
        out.push_str(&csv_escape(&item.name));
        out.push(',');
        out.push_str(&csv_escape(&q));
        out.push(',');
        out.push_str(&csv_escape(&u));
        if show_sources {
            out.push(',');
            out.push_str(&csv_escape(&item.sources.join("; ")));
        }
    }
    out
}
fn render_json(items: &[Item], system: UnitSystem) -> String {
    let mut out = String::from("{\n  \"items\": [");
    for (i, item) in items.iter().enumerate() {
        let (q, u) = quantity_unit(item, system);
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("\n    {{ \"category\": \"{}\", \"name\": \"{}\", \"quantity\": \"{}\", \"unit\": \"{}\", \"sources\": [{}] }}", json_escape(category(&item.name)), json_escape(&item.name), json_escape(&q), json_escape(&u), item.sources.iter().map(|s| format!("\"{}\"", json_escape(s))).collect::<Vec<_>>().join(", ")));
    }
    if !items.is_empty() {
        out.push('\n');
    }
    out.push_str("  ]\n}");
    out
}
fn format_item(item: &Item, system: UnitSystem, show_sources: bool) -> String {
    let (q, u) = quantity_unit(item, system);
    let mut s = String::new();
    if !q.is_empty() {
        s.push_str(&q);
        if !u.is_empty() {
            s.push(' ');
            s.push_str(&u);
        }
        s.push(' ');
    }
    s.push_str(&item.name);
    if show_sources {
        s.push_str(" — ");
        s.push_str(&item.sources.join(", "));
    }
    s
}
fn quantity_unit(item: &Item, system: UnitSystem) -> (String, String) {
    let Some(base) = item.qty_base else {
        return (String::new(), item.unit.clone());
    };
    match item.family {
        Family::Weight => match system {
            UnitSystem::Metric => {
                if base >= 1000.0 {
                    (fmt_num(base / 1000.0), "kg".into())
                } else {
                    (fmt_num(base), "g".into())
                }
            }
            UnitSystem::Us => {
                if base >= 453.592 {
                    (fmt_num(base / 453.592), "lb".into())
                } else {
                    (fmt_num(base / 28.3495), "oz".into())
                }
            }
            UnitSystem::Keep => render_base_in_unit(base, &item.unit),
        },
        Family::Volume => match system {
            UnitSystem::Metric => {
                if base >= 1000.0 {
                    (fmt_num(base / 1000.0), "l".into())
                } else {
                    (fmt_num(base), "ml".into())
                }
            }
            UnitSystem::Us => {
                if base >= 236.588 {
                    (fmt_num(base / 236.588), "cup".into())
                } else if base >= 14.7868 {
                    (fmt_num(base / 14.7868), "tbsp".into())
                } else {
                    (fmt_num(base / 4.92892), "tsp".into())
                }
            }
            UnitSystem::Keep => render_base_in_unit(base, &item.unit),
        },
        _ => (fmt_num(base), item.unit.clone()),
    }
}
fn render_base_in_unit(base: f64, unit: &str) -> (String, String) {
    if let Some(u) = unit_for(unit) {
        (fmt_num(base / u.to_base), u.canon.into())
    } else {
        (fmt_num(base), unit.into())
    }
}
fn fmt_num(v: f64) -> String {
    let rounded = (v * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 0.005 {
        format!("{}", rounded.round() as i64)
    } else {
        format!("{rounded:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}
fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
fn json_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            c => vec![c],
        })
        .collect()
}

fn category(name: &str) -> &'static str {
    let n = normalize_name(name);
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&[
        "apple", "banana", "onion", "garlic", "carrot", "pepper", "tomato", "lettuce", "spinach",
        "lemon", "lime", "potato", "cilantro", "parsley",
    ]) {
        "Produce"
    } else if has(&["milk", "cheese", "butter", "yogurt", "cream", "egg"]) {
        "Dairy"
    } else if has(&[
        "chicken", "beef", "pork", "fish", "salmon", "shrimp", "turkey", "bacon",
    ]) {
        "Meat & seafood"
    } else if has(&["bread", "bun", "tortilla", "bagel", "roll"]) {
        "Bakery"
    } else if has(&[
        "salt", "pepper", "cumin", "paprika", "cinnamon", "oregano", "basil", "spice",
    ]) {
        "Spices"
    } else if has(&["frozen", "peas", "ice cream"]) {
        "Frozen"
    } else if has(&["water", "juice", "coffee", "tea", "wine", "beer"]) {
        "Beverages"
    } else if has(&[
        "flour",
        "sugar",
        "oil",
        "rice",
        "pasta",
        "bean",
        "stock",
        "broth",
        "can",
        "tomato paste",
        "oat",
    ]) {
        "Pantry"
    } else {
        "Other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_scaled_recipes_into_categories() {
        let input = "# Pancakes x2\n1 cup flour\n1 cup milk\n2 eggs\n---\n# Sauce\n1/2 cup milk\n1 tbsp sugar";
        let out = run(input, 1.0, "category", "keep", "", false, false, "markdown").unwrap();
        assert!(out.contains("### Dairy"), "{out}");
        assert!(out.contains("2.5 cup milk"), "{out}");
        assert!(out.contains("4 egg"), "{out}");
        assert!(out.contains("2 cup flour"), "{out}");
    }
    #[test]
    fn metric_conversion_and_sources_work() {
        let input = "# Chili\n1 lb beef\n8 oz beef\n2 cloves garlic";
        let out = run(input, 1.0, "none", "metric", "", false, true, "text").unwrap();
        assert!(out.contains("680.39 g beef — Chili"), "{out}");
        assert!(out.contains("2 clove garlic — Chili"), "{out}");
    }
    #[test]
    fn csv_and_exclude() {
        let input = "1 cup flour\n2 tbsp sugar\n1 tsp salt";
        let out = run(input, 1.0, "none", "keep", "salt", false, false, "csv").unwrap();
        assert!(out.starts_with("category,item,quantity,unit"));
        assert!(out.contains("Pantry,flour,1,cup"));
        assert!(!out.contains("salt"));
    }
    #[test]
    fn json_is_machine_readable_enough() {
        let out = run(
            "2 tomatoes",
            1.0,
            "category",
            "keep",
            "",
            false,
            false,
            "json",
        )
        .unwrap();
        assert!(out.contains("\"category\": \"Produce\""));
        assert!(out.contains("\"name\": \"tomatoes\""));
    }
    #[test]
    fn rejects_bad_inputs() {
        assert!(run(
            "1 cup flour",
            0.0,
            "category",
            "keep",
            "",
            false,
            false,
            "markdown"
        )
        .unwrap_err()
        .contains("invalid scale"));
        assert!(run("", 1.0, "bad", "keep", "", false, false, "markdown")
            .unwrap_err()
            .contains("invalid group_by"));
        assert!(run(
            "mix until smooth",
            1.0,
            "category",
            "keep",
            "",
            false,
            false,
            "markdown"
        )
        .unwrap_err()
        .contains("no ingredient"));
    }
}
