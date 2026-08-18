//! gizza-ai/json-key-case-convert core — rewrite every JSON object key into one
//! naming convention (camelCase, PascalCase, snake_case, kebab-case or
//! SCREAMING_SNAKE_CASE) while leaving values byte-identical. The document is
//! parsed and validated first, so malformed JSON is rejected with the parser's
//! exact line/column. Pure-Rust; shared by the chat skill block and the web page.
//!
//! Key rewriting is a two-step process:
//!
//! 1. **Split** the key into words. Any non-alphanumeric run (`_`, `-`, `.`,
//!    spaces, …) is a word boundary, and so is a lower→upper hump. Acronym runs
//!    break before their last capital when a lowercase letter follows, so
//!    `HTTPResponse` splits as `HTTP` + `Response` and `userID` as `user` + `ID`.
//!    Digits stay attached to the word they follow (`utf8`, `address1`).
//! 2. **Join** the words in the target convention.
//!
//! Two escape hatches keep real-world documents intact: leading sigils
//! (`_id`, `$schema`, `@context`) survive by default, and `preserve_keys` names
//! keys that must not be touched at all. Because renaming can make two distinct
//! keys collide inside one object, a collision is a hard error naming both keys
//! and the JSON path rather than a silent overwrite.

use serde::Serialize;
use serde_json::{Map, Value};

/// Largest input document accepted, in bytes.
pub const MAX_JSON_BYTES: usize = 5_000_000;
/// Largest nesting depth walked before giving up.
pub const MAX_DEPTH: usize = 100;

/// The naming convention keys are rewritten into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// `camelCase`
    Camel,
    /// `PascalCase`
    Pascal,
    /// `snake_case`
    Snake,
    /// `kebab-case`
    Kebab,
    /// `SCREAMING_SNAKE_CASE` (a.k.a. CONSTANT_CASE)
    Constant,
}

impl Case {
    /// Parse the `target_case` param. Accepts the canonical value plus the
    /// common spellings users type (`camelCase`, `screaming_snake`, `upper`…).
    /// Blank falls back to `camel`; anything else is an error listing the
    /// accepted values.
    pub fn parse(s: &str) -> Result<Case, String> {
        let t = s.trim();
        if t.is_empty() {
            return Ok(Case::Camel);
        }
        let norm: String = t
            .chars()
            .filter(|c| c.is_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect();
        match norm.as_str() {
            "camel" | "camelcase" | "lowercamelcase" => Ok(Case::Camel),
            "pascal" | "pascalcase" | "uppercamelcase" => Ok(Case::Pascal),
            "snake" | "snakecase" | "underscore" => Ok(Case::Snake),
            "kebab" | "kebabcase" | "dash" | "hyphen" | "spinal" => Ok(Case::Kebab),
            "constant" | "constantcase" | "screamingsnake" | "screamingsnakecase" | "macro"
            | "upper" | "uppersnake" | "uppersnakecase" => Ok(Case::Constant),
            _ => Err(format!(
                "invalid target_case {t:?}: expected one of camel, pascal, snake, kebab, constant"
            )),
        }
    }
}

/// Options controlling the rewrite.
#[derive(Clone, Debug)]
pub struct Options {
    /// Target naming convention for every rewritten key.
    pub target: Case,
    /// Rewrite keys at every nesting level (default). When off, only the
    /// outermost object's keys change — for a root array, the keys of the
    /// objects directly inside it.
    pub recurse: bool,
    /// Exact key names (case-sensitive) that must be left untouched.
    pub preserve_keys: Vec<String>,
    /// Keep a key's leading non-alphanumeric sigils (`_id`, `$schema`,
    /// `@context`) and convert only the rest. On by default.
    pub preserve_prefix: bool,
    /// Spaces of indentation per level (clamped 0..=8). `0` minifies to one
    /// compact line.
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            target: Case::Camel,
            recurse: true,
            preserve_keys: Vec::new(),
            preserve_prefix: true,
            indent: 2,
        }
    }
}

/// Split a comma-separated `preserve_keys` param into exact key names.
/// Blank entries are dropped; surrounding whitespace is trimmed.
pub fn parse_preserve_keys(list: &str) -> Vec<String> {
    list.split(',')
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .map(|k| k.to_string())
        .collect()
}

/// Split a key into its words. See the module docs for the exact rules.
pub fn split_words(key: &str) -> Vec<String> {
    let chars: Vec<char> = key.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut prev: Option<char> = None;
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev = None;
            continue;
        }
        if let Some(p) = prev {
            // `fooBar` / `v2Api` → break before the capital that starts a word.
            let hump = !p.is_uppercase() && c.is_uppercase();
            // `HTTPResponse` → break before the last capital of an acronym run.
            let acronym_tail = p.is_uppercase()
                && c.is_uppercase()
                && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if (hump || acronym_tail) && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
        prev = Some(c);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

/// Lowercase a whole word (Unicode-aware).
fn lower(word: &str) -> String {
    word.to_lowercase()
}

/// Capitalize a word: first character upper, the rest lower (`ID` → `Id`).
fn capitalize(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for (i, c) in word.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// Rewrite a single key into `target`. `preserve_prefix` keeps a leading run of
/// non-alphanumeric sigils (`_id`, `$schema`). A key with no alphanumeric
/// characters at all is returned unchanged — there is nothing to rename.
pub fn convert_key(key: &str, target: Case, preserve_prefix: bool) -> String {
    let (prefix, body) = if preserve_prefix {
        let split = key
            .char_indices()
            .find(|(_, c)| c.is_alphanumeric())
            .map(|(i, _)| i)
            .unwrap_or(key.len());
        (&key[..split], &key[split..])
    } else {
        ("", key)
    };
    let words = split_words(body);
    if words.is_empty() {
        return key.to_string();
    }
    let converted = match target {
        Case::Camel => words
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { lower(w) } else { capitalize(w) })
            .collect::<Vec<_>>()
            .concat(),
        Case::Pascal => words.iter().map(|w| capitalize(w)).collect::<Vec<_>>().concat(),
        Case::Snake => words.iter().map(|w| lower(w)).collect::<Vec<_>>().join("_"),
        Case::Kebab => words.iter().map(|w| lower(w)).collect::<Vec<_>>().join("-"),
        Case::Constant => words
            .iter()
            .map(|w| w.to_uppercase())
            .collect::<Vec<_>>()
            .join("_"),
    };
    format!("{prefix}{converted}")
}

/// Parse `json`, rewrite every object key into `opts.target`, and re-serialize.
///
/// Values are never modified and key order is preserved. Returns an error with
/// the parser's line/column for invalid JSON, and a path-qualified error when
/// two keys in the same object would collide after renaming.
pub fn convert(json: &str, opts: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON input: paste a JSON object, array, or document".into());
    }
    if json.len() > MAX_JSON_BYTES {
        return Err(format!(
            "JSON is {} bytes; the maximum is {MAX_JSON_BYTES}",
            json.len()
        ));
    }
    let mut value: Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    walk(&mut value, opts, 0, 0, "$")?;

    let n = opts.indent.min(8);
    if n == 0 {
        return serde_json::to_string(&value).map_err(|e| format!("serialize failed: {e}"));
    }
    let pad = vec![b' '; n];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("serialize failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

/// Recursive worker. `obj_depth` counts enclosing OBJECTS (arrays are
/// transparent) so `recurse = false` still reaches the objects of a root array;
/// `depth` counts every container for the depth cap.
fn walk(
    value: &mut Value,
    opts: &Options,
    obj_depth: usize,
    depth: usize,
    path: &str,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "JSON nests deeper than {MAX_DEPTH} levels at {path}; flatten it or convert a smaller subtree"
        ));
    }
    match value {
        Value::Object(map) => {
            if obj_depth == 0 || opts.recurse {
                rename_keys(map, opts, path)?;
            }
            if opts.recurse {
                for (k, v) in map.iter_mut() {
                    let child = format!("{path}.{k}");
                    walk(v, opts, obj_depth + 1, depth + 1, &child)?;
                }
            }
        }
        Value::Array(arr) => {
            if obj_depth == 0 || opts.recurse {
                for (i, v) in arr.iter_mut().enumerate() {
                    let child = format!("{path}[{i}]");
                    walk(v, opts, obj_depth, depth + 1, &child)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Rewrite one object's keys in place, preserving insertion order and refusing
/// to let two source keys land on the same name.
fn rename_keys(map: &mut Map<String, Value>, opts: &Options, path: &str) -> Result<(), String> {
    let entries: Vec<(String, Value)> = std::mem::replace(map, Map::new()).into_iter().collect();
    // Original name of whatever already claimed each output key, for the error.
    let mut claimed: Vec<(String, String)> = Vec::with_capacity(entries.len());
    for (key, val) in entries {
        let new_key = if opts.preserve_keys.iter().any(|p| p == &key) {
            key.clone()
        } else {
            convert_key(&key, opts.target, opts.preserve_prefix)
        };
        if let Some((_, first)) = claimed.iter().find(|(k, _)| k == &new_key) {
            return Err(format!(
                "key collision at {path}: {first:?} and {key:?} both become {new_key:?} — rename one in the source, or add it to preserve_keys"
            ));
        }
        claimed.push((new_key.clone(), key));
        map.insert(new_key, val);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(target: Case) -> Options {
        Options { target, indent: 0, ..Options::default() }
    }

    #[test]
    fn converts_nested_keys_to_camel() {
        let out = convert(
            r#"{"user_id":1,"profile_data":{"first_name":"ada","home-address":{"zip_code":"9"}}}"#,
            &opts(Case::Camel),
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"userId":1,"profileData":{"firstName":"ada","homeAddress":{"zipCode":"9"}}}"#
        );
    }

    #[test]
    fn converts_keys_inside_arrays() {
        let out = convert(r#"{"line_items":[{"item_id":1},{"item_id":2}]}"#, &opts(Case::Snake))
            .unwrap();
        assert_eq!(out, r#"{"line_items":[{"item_id":1},{"item_id":2}]}"#);
        let out = convert(r#"{"lineItems":[{"itemId":1}]}"#, &opts(Case::Kebab)).unwrap();
        assert_eq!(out, r#"{"line-items":[{"item-id":1}]}"#);
    }

    #[test]
    fn all_five_targets() {
        let src = r#"{"first_name":"ada"}"#;
        assert_eq!(convert(src, &opts(Case::Camel)).unwrap(), r#"{"firstName":"ada"}"#);
        assert_eq!(convert(src, &opts(Case::Pascal)).unwrap(), r#"{"FirstName":"ada"}"#);
        assert_eq!(convert(src, &opts(Case::Snake)).unwrap(), r#"{"first_name":"ada"}"#);
        assert_eq!(convert(src, &opts(Case::Kebab)).unwrap(), r#"{"first-name":"ada"}"#);
        assert_eq!(convert(src, &opts(Case::Constant)).unwrap(), r#"{"FIRST_NAME":"ada"}"#);
    }

    #[test]
    fn values_and_key_order_are_untouched() {
        let out = convert(
            r#"{"z_last":"KEEP_ME","a_first":[1,"two",null,true]}"#,
            &opts(Case::Camel),
        )
        .unwrap();
        assert_eq!(out, r#"{"zLast":"KEEP_ME","aFirst":[1,"two",null,true]}"#);
    }

    #[test]
    fn acronyms_split_on_the_last_capital() {
        assert_eq!(convert_key("HTTPResponse", Case::Camel, true), "httpResponse");
        assert_eq!(convert_key("userID", Case::Snake, true), "user_id");
        assert_eq!(convert_key("parseJSONBody", Case::Snake, true), "parse_json_body");
        assert_eq!(convert_key("ID", Case::Camel, true), "id");
    }

    #[test]
    fn digits_stay_attached_to_their_word() {
        assert_eq!(convert_key("address1", Case::Snake, true), "address1");
        assert_eq!(convert_key("utf8Encoding", Case::Snake, true), "utf8_encoding");
        assert_eq!(convert_key("v2Api", Case::Kebab, true), "v2-api");
    }

    #[test]
    fn leading_sigils_are_preserved_by_default() {
        assert_eq!(convert_key("_id", Case::Camel, true), "_id");
        assert_eq!(convert_key("__type_name", Case::Camel, true), "__typeName");
        assert_eq!(convert_key("$schema_url", Case::Camel, true), "$schemaUrl");
        // Opting out strips them.
        assert_eq!(convert_key("_id", Case::Camel, false), "id");
        assert_eq!(convert_key("$schema_url", Case::Snake, false), "schema_url");
    }

    #[test]
    fn keys_without_letters_are_left_alone() {
        assert_eq!(convert_key("___", Case::Camel, true), "___");
        assert_eq!(convert_key("", Case::Snake, true), "");
    }

    #[test]
    fn preserve_keys_skips_exact_names() {
        let o = Options {
            target: Case::Snake,
            preserve_keys: parse_preserve_keys(" Content-Type , X-Api-Key "),
            indent: 0,
            ..Options::default()
        };
        let out = convert(r#"{"Content-Type":"a","X-Api-Key":"b","userName":"c"}"#, &o).unwrap();
        assert_eq!(out, r#"{"Content-Type":"a","X-Api-Key":"b","user_name":"c"}"#);
    }

    #[test]
    fn recurse_off_touches_only_the_outermost_object() {
        let o = Options { target: Case::Camel, recurse: false, indent: 0, ..Options::default() };
        let out = convert(r#"{"outer_key":{"inner_key":1}}"#, &o).unwrap();
        assert_eq!(out, r#"{"outerKey":{"inner_key":1}}"#);
        // A root array still gets its immediate objects renamed.
        let out = convert(r#"[{"item_id":1,"sub":{"a_b":2}}]"#, &o).unwrap();
        assert_eq!(out, r#"[{"itemId":1,"sub":{"a_b":2}}]"#);
    }

    #[test]
    fn pretty_indent_two_by_default() {
        let out = convert(r#"{"a_b":1}"#, &Options::default()).unwrap();
        assert_eq!(out, "{\n  \"aB\": 1\n}");
    }

    #[test]
    fn indent_clamped_to_eight() {
        let o = Options { indent: 99, ..Options::default() };
        assert_eq!(convert(r#"{"a_b":1}"#, &o).unwrap(), "{\n        \"aB\": 1\n}");
    }

    #[test]
    fn scalar_root_passes_through() {
        assert_eq!(convert("42", &opts(Case::Camel)).unwrap(), "42");
        assert_eq!(convert(r#""hi""#, &opts(Case::Camel)).unwrap(), r#""hi""#);
    }

    #[test]
    fn rejects_invalid_json() {
        let err = convert("{bad}", &opts(Case::Camel)).unwrap_err();
        assert!(err.contains("invalid JSON"), "{err}");
        assert!(convert("", &opts(Case::Camel)).is_err());
        assert!(convert("[1,2,]", &opts(Case::Camel)).is_err());
    }

    #[test]
    fn rejects_key_collisions_with_both_names() {
        let err = convert(r#"{"user_name":1,"userName":2}"#, &opts(Case::Camel)).unwrap_err();
        assert!(err.contains("key collision at $"), "{err}");
        assert!(err.contains("\"user_name\"") && err.contains("\"userName\""), "{err}");
    }

    #[test]
    fn collision_error_names_the_nested_path() {
        let err =
            convert(r#"{"data":{"a_b":1,"aB":2}}"#, &opts(Case::Camel)).unwrap_err();
        assert!(err.contains("key collision at $.data"), "{err}");
    }

    #[test]
    fn rejects_unknown_target_case() {
        let err = Case::parse("dromedary").unwrap_err();
        assert!(err.contains("invalid target_case"), "{err}");
        assert_eq!(Case::parse("camelCase").unwrap(), Case::Camel);
        assert_eq!(Case::parse("SCREAMING_SNAKE").unwrap(), Case::Constant);
        assert_eq!(Case::parse("").unwrap(), Case::Camel);
    }

    #[test]
    fn rejects_oversized_input() {
        let big = format!("{{\"a\":\"{}\"}}", "x".repeat(MAX_JSON_BYTES));
        let err = convert(&big, &Options::default()).unwrap_err();
        assert!(err.contains("maximum is"), "{err}");
    }

    #[test]
    fn unicode_keys_convert() {
        assert_eq!(convert_key("größe_wert", Case::Camel, true), "größeWert");
        assert_eq!(convert_key("ÉtatCivil", Case::Snake, true), "état_civil");
    }
}
