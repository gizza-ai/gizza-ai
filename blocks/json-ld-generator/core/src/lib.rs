//! json-ld-generator core — build schema.org JSON-LD structured-data markup
//! from simple field inputs. No wafer/wasm-bindgen deps; shared by the chat
//! skill block and the standalone web page.
//!
//! Given a schema.org `type` (Article, Product, FAQ, …) and a set of
//! `field = value` pairs, it emits a single JSON-LD object with the correct
//! `@context`/`@type` and the supplied properties, optionally wrapped in a
//! `<script type="application/ld+json">` tag for pasting straight into a page.

use serde_json::{Map, Value};

/// Maximum number of `field = value` lines accepted. Guards against pathological
/// input while comfortably covering every real schema.org type's properties.
pub const MAX_FIELDS: usize = 200;

/// The supported schema.org `@type` values. Picking from a fixed set keeps the
/// LLM-facing schema honest and lets us apply per-type field shaping (nested
/// objects, FAQ Q&A expansion, list splitting) instead of dumping a flat blob.
pub const TYPES: &[&str] = &[
    "Article",
    "Product",
    "FAQPage",
    "Organization",
    "LocalBusiness",
    "Person",
    "Event",
    "Recipe",
    "WebSite",
    "BreadcrumbList",
    "Review",
    "VideoObject",
    "HowTo",
    "JobPosting",
    "Course",
    "SoftwareApplication",
];

/// Generate schema.org JSON-LD markup.
///
/// - `schema_type`: one of [`TYPES`] (case-insensitive, blank → `Article`).
/// - `fields`: newline-separated `name = value` (or `name: value`) pairs. Blank
///   lines and lines starting with `#` are ignored. Values support a few shapes:
///     - `keywords` / `sameAs` (and other known list fields) split on `,` into a
///       JSON array;
///     - dotted names (`author.name`, `address.streetAddress`) build a nested
///       object whose `@type` is inferred (`author`→Person, `address`→PostalAddress,
///       `publisher`/`brand`→Organization, `offers`→Offer, …);
///     - numeric-looking values for `price`/`ratingValue`/`reviewCount`/… are
///       emitted as JSON numbers.
/// - `faq`: for `FAQPage`, newline-separated `Question? | Answer` (or
///   `Question? :: Answer`) pairs, expanded into `mainEntity` Q&A items. Ignored
///   for other types.
/// - `wrap_script`: when true, wrap the JSON in a
///   `<script type="application/ld+json">…</script>` tag.
///
/// Returns `Err` on an unknown type, too many fields, an unparseable line, or
/// (for FAQPage) when no Q&A pairs are supplied.
pub fn generate(
    schema_type: &str,
    fields: &str,
    faq: &str,
    wrap_script: bool,
) -> Result<String, String> {
    let ty = canonical_type(schema_type)?;

    let mut root = Map::new();
    root.insert("@context".into(), Value::String("https://schema.org".into()));
    root.insert("@type".into(), Value::String(ty.to_string()));

    let pairs = parse_pairs(fields)?;
    if pairs.len() > MAX_FIELDS {
        return Err(format!("too many fields: {} (max {MAX_FIELDS})", pairs.len()));
    }
    for (name, value) in pairs {
        insert_field(&mut root, &name, &value);
    }

    if ty == "FAQPage" {
        let qa = parse_faq(faq)?;
        if qa.is_empty() {
            return Err(
                "FAQPage needs at least one 'Question? | Answer' pair in the FAQ field".to_string(),
            );
        }
        let entities: Vec<Value> = qa
            .into_iter()
            .map(|(q, a)| {
                let mut question = Map::new();
                question.insert("@type".into(), Value::String("Question".into()));
                question.insert("name".into(), Value::String(q));
                let mut answer = Map::new();
                answer.insert("@type".into(), Value::String("Answer".into()));
                answer.insert("text".into(), Value::String(a));
                question.insert("acceptedAnswer".into(), Value::Object(answer));
                Value::Object(question)
            })
            .collect();
        root.insert("mainEntity".into(), Value::Array(entities));
    }

    let json = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| format!("failed to serialize JSON-LD: {e}"))?;

    if wrap_script {
        Ok(format!(
            "<script type=\"application/ld+json\">\n{json}\n</script>"
        ))
    } else {
        Ok(json)
    }
}

/// Resolve `input` to a canonical schema.org type name (case-insensitive).
fn canonical_type(input: &str) -> Result<&'static str, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok("Article");
    }
    TYPES
        .iter()
        .find(|t| t.eq_ignore_ascii_case(trimmed))
        .copied()
        .ok_or_else(|| {
            format!(
                "unknown schema type {trimmed:?}: expected one of {}",
                TYPES.join(", ")
            )
        })
}

/// Parse `name = value` / `name: value` lines into ordered pairs.
fn parse_pairs(fields: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for (i, raw) in fields.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Split on the first '=' or ':' — whichever comes first.
        let sep = line
            .find('=')
            .into_iter()
            .chain(line.find(':'))
            .min()
            .ok_or_else(|| format!("line {} is not a 'name = value' pair: {raw:?}", i + 1))?;
        let name = line[..sep].trim();
        let value = line[sep + 1..].trim();
        if name.is_empty() {
            return Err(format!("line {} has an empty field name: {raw:?}", i + 1));
        }
        out.push((name.to_string(), value.to_string()));
    }
    Ok(out)
}

/// Parse FAQ `Question? | Answer` (or `:: `) lines into (q, a) pairs.
fn parse_faq(faq: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for (i, raw) in faq.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (q, a) = if let Some(idx) = line.find("::") {
            (&line[..idx], &line[idx + 2..])
        } else if let Some(idx) = line.find('|') {
            (&line[..idx], &line[idx + 1..])
        } else {
            return Err(format!(
                "FAQ line {} is not a 'Question? | Answer' pair: {raw:?}",
                i + 1
            ));
        };
        let q = q.trim();
        let a = a.trim();
        if q.is_empty() || a.is_empty() {
            return Err(format!(
                "FAQ line {} has an empty question or answer: {raw:?}",
                i + 1
            ));
        }
        out.push((q.to_string(), a.to_string()));
    }
    Ok(out)
}

/// Insert one `name → value` field into `root`, applying value shaping:
/// nested objects for dotted names, arrays for list-ish fields, numbers where
/// the schema property is numeric.
fn insert_field(root: &mut Map<String, Value>, name: &str, value: &str) {
    if let Some((head, tail)) = name.split_once('.') {
        // Nested object, e.g. author.name → { author: { @type: Person, name } }.
        let head = head.trim();
        let entry = root
            .entry(head.to_string())
            .or_insert_with(|| Value::Object(nested_object(head)));
        if let Value::Object(obj) = entry {
            // Recurse to support deeper dotting (address.streetAddress, …).
            insert_into(obj, tail.trim(), value);
        }
        return;
    }
    root.insert(name.to_string(), shaped_value(name, value));
}

/// Like `insert_field` but for an already-resolved nested object.
fn insert_into(obj: &mut Map<String, Value>, name: &str, value: &str) {
    if let Some((head, tail)) = name.split_once('.') {
        let head = head.trim();
        let entry = obj
            .entry(head.to_string())
            .or_insert_with(|| Value::Object(nested_object(head)));
        if let Value::Object(inner) = entry {
            insert_into(inner, tail.trim(), value);
        }
        return;
    }
    obj.insert(name.to_string(), shaped_value(name, value));
}

/// A fresh nested object pre-seeded with the `@type` inferred from its key.
fn nested_object(key: &str) -> Map<String, Value> {
    let mut m = Map::new();
    if let Some(ty) = inferred_type(key) {
        m.insert("@type".into(), Value::String(ty.to_string()));
    }
    m
}

/// Infer the `@type` for a nested property from its (lowercased) key.
fn inferred_type(key: &str) -> Option<&'static str> {
    Some(match key.to_ascii_lowercase().as_str() {
        "author" | "creator" => "Person",
        "publisher" | "brand" | "manufacturer" | "provider" => "Organization",
        "address" => "PostalAddress",
        "offers" | "offer" => "Offer",
        "aggregaterating" => "AggregateRating",
        "reviewrating" => "Rating",
        "geo" => "GeoCoordinates",
        "location" => "Place",
        "hiringorganization" => "Organization",
        _ => return None,
    })
}

/// Shape a leaf value: split list-ish fields into arrays, parse numeric fields
/// into JSON numbers, otherwise keep the string.
fn shaped_value(name: &str, value: &str) -> Value {
    let lname = name.to_ascii_lowercase();
    // List-valued fields → array (split on comma, trim, drop empties).
    if is_list_field(&lname) && value.contains(',') {
        let arr: Vec<Value> = value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_string()))
            .collect();
        return Value::Array(arr);
    }
    // Numeric fields → JSON number when the value parses cleanly.
    if is_numeric_field(&lname) {
        if let Ok(n) = value.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(n) {
                return Value::Number(num);
            }
        }
    }
    Value::String(value.to_string())
}

fn is_list_field(lname: &str) -> bool {
    matches!(
        lname,
        "keywords" | "ingredients" | "recipeingredient" | "articlesection" | "sameas"
    )
}

fn is_numeric_field(lname: &str) -> bool {
    matches!(
        lname,
        "price"
            | "ratingvalue"
            | "reviewcount"
            | "ratingcount"
            | "bestrating"
            | "worstrating"
            | "latitude"
            | "longitude"
            | "calories"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn parse(s: &str) -> Value {
        // Strip the script wrapper if present, then parse as JSON.
        let inner = s
            .strip_prefix("<script type=\"application/ld+json\">\n")
            .and_then(|s| s.strip_suffix("\n</script>"))
            .unwrap_or(s);
        serde_json::from_str(inner).expect("valid JSON")
    }

    #[test]
    fn article_with_nested_author_and_keywords() {
        let out = generate(
            "Article",
            "headline = Hello World\nauthor.name = Jane Doe\nkeywords = seo, json-ld, rust",
            "",
            false,
        )
        .unwrap();
        let v = parse(&out);
        assert_eq!(v["@context"], "https://schema.org");
        assert_eq!(v["@type"], "Article");
        assert_eq!(v["headline"], "Hello World");
        assert_eq!(v["author"]["@type"], "Person");
        assert_eq!(v["author"]["name"], "Jane Doe");
        assert_eq!(v["keywords"], serde_json::json!(["seo", "json-ld", "rust"]));
    }

    #[test]
    fn product_price_is_a_number_and_offers_nested() {
        let out = generate(
            "Product",
            "name = Widget\noffers.price = 19.99\noffers.priceCurrency = USD",
            "",
            false,
        )
        .unwrap();
        let v = parse(&out);
        assert_eq!(v["@type"], "Product");
        assert_eq!(v["offers"]["@type"], "Offer");
        assert!(v["offers"]["price"].is_number());
        assert_eq!(v["offers"]["price"], 19.99);
        assert_eq!(v["offers"]["priceCurrency"], "USD");
    }

    #[test]
    fn faq_expands_into_main_entity() {
        let out = generate(
            "FAQPage",
            "",
            "Is it free? | Yes, completely.\nIs it private? :: Runs in your browser.",
            false,
        )
        .unwrap();
        let v = parse(&out);
        assert_eq!(v["@type"], "FAQPage");
        let me = v["mainEntity"].as_array().unwrap();
        assert_eq!(me.len(), 2);
        assert_eq!(me[0]["@type"], "Question");
        assert_eq!(me[0]["name"], "Is it free?");
        assert_eq!(me[0]["acceptedAnswer"]["@type"], "Answer");
        assert_eq!(me[0]["acceptedAnswer"]["text"], "Yes, completely.");
        assert_eq!(me[1]["name"], "Is it private?");
        assert_eq!(me[1]["acceptedAnswer"]["text"], "Runs in your browser.");
    }

    #[test]
    fn wrap_script_emits_tag() {
        let out = generate("WebSite", "name = gizza\nurl = https://gizza.ai", "", true).unwrap();
        assert!(out.starts_with("<script type=\"application/ld+json\">\n"));
        assert!(out.trim_end().ends_with("</script>"));
        // The body is still valid JSON-LD.
        let v = parse(&out);
        assert_eq!(v["@type"], "WebSite");
        assert_eq!(v["url"], "https://gizza.ai");
    }

    #[test]
    fn type_is_case_insensitive_and_blank_defaults_to_article() {
        let v = parse(&generate("product", "name = X", "", false).unwrap());
        assert_eq!(v["@type"], "Product");
        let v = parse(&generate("", "headline = X", "", false).unwrap());
        assert_eq!(v["@type"], "Article");
    }

    #[test]
    fn comments_and_blank_lines_ignored() {
        let v = parse(
            &generate("Article", "# a comment\n\nheadline = Hi\n  # indented\n", "", false).unwrap(),
        );
        assert_eq!(v["headline"], "Hi");
        assert!(v.get("# a comment").is_none());
    }

    #[test]
    fn rejects_unknown_type() {
        let err = generate("Banana", "x = 1", "", false).unwrap_err();
        assert!(err.contains("unknown schema type"), "got: {err}");
    }

    #[test]
    fn rejects_malformed_field_line() {
        let err = generate("Article", "this has no separator", "", false).unwrap_err();
        assert!(err.contains("not a 'name = value' pair"), "got: {err}");
    }

    #[test]
    fn rejects_empty_field_name() {
        let err = generate("Article", " = value", "", false).unwrap_err();
        assert!(err.contains("empty field name"), "got: {err}");
    }

    #[test]
    fn faqpage_without_pairs_errors() {
        let err = generate("FAQPage", "name = Help", "", false).unwrap_err();
        assert!(err.contains("at least one"), "got: {err}");
    }

    #[test]
    fn faq_malformed_line_errors() {
        let err = generate("FAQPage", "", "no separator here", false).unwrap_err();
        assert!(err.contains("not a 'Question? | Answer' pair"), "got: {err}");
    }

    #[test]
    fn non_list_field_with_comma_stays_string() {
        // `name` is not a list field, so a comma does not split it.
        let v = parse(&generate("Person", "name = Doe, Jane", "", false).unwrap());
        assert_eq!(v["name"], "Doe, Jane");
    }

    #[test]
    fn deeply_nested_address() {
        let v = parse(
            &generate(
                "LocalBusiness",
                "name = Cafe\naddress.streetAddress = 1 Main St\naddress.addressLocality = Springfield",
                "",
                false,
            )
            .unwrap(),
        );
        assert_eq!(v["address"]["@type"], "PostalAddress");
        assert_eq!(v["address"]["streetAddress"], "1 Main St");
        assert_eq!(v["address"]["addressLocality"], "Springfield");
    }

    #[test]
    fn review_type_with_nested_review_rating() {
        let v = parse(
            &generate(
                "Review",
                "reviewBody = Great product\nreviewRating.ratingValue = 5\nreviewRating.bestRating = 5",
                "",
                false,
            )
            .unwrap(),
        );
        assert_eq!(v["@type"], "Review");
        assert_eq!(v["reviewRating"]["@type"], "Rating");
        assert_eq!(v["reviewRating"]["ratingValue"], 5.0);
    }

    #[test]
    fn colon_separator_supported() {
        let v = parse(&generate("Article", "headline: Colon Works", "", false).unwrap());
        assert_eq!(v["headline"], "Colon Works");
    }
}
