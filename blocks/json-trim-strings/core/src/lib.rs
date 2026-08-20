//! Pure logic for `json-trim-strings`: walk a JSON document and normalize the
//! whitespace of every string VALUE (and optionally every object KEY), leaving
//! the structure, the numbers, the booleans, the nulls and the key order exactly
//! as they were.
//!
//! This is the opposite of a minifier. A minifier removes whitespace BETWEEN
//! tokens and protects the inside of quoted strings; this removes whitespace
//! INSIDE quoted strings and never touches the structure.
//!
//! No I/O, no clock, no randomness — the same input always yields the same
//! output, on every surface (chat block, CLI, browser page).

/// Largest document accepted, in bytes. Everything is parsed into memory.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Which end(s) of a string lose their whitespace.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TrimSide {
    Both,
    Leading,
    Trailing,
    None,
}

/// What happens to whitespace between the first and last non-whitespace char.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Internal {
    Keep,
    Collapse,
    Remove,
}

/// Which characters count as whitespace.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WsClass {
    Unicode,
    Ascii,
}

/// What to do with a string that is empty once it has been trimmed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EmptyPolicy {
    Keep,
    Null,
    Drop,
}

impl WsClass {
    fn is_ws(self, c: char) -> bool {
        match self {
            // The full Unicode White_Space property: NBSP (U+00A0), narrow NBSP
            // (U+202F), ideographic space (U+3000) and friends included.
            WsClass::Unicode => c.is_whitespace(),
            // Exactly the five characters JSON itself and C-family trim()s treat
            // as blank; NBSP stays content.
            WsClass::Ascii => matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{000C}'),
        }
    }
}

fn parse_trim(s: &str) -> Result<TrimSide, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "both" => Ok(TrimSide::Both),
        "leading" | "left" | "start" => Ok(TrimSide::Leading),
        "trailing" | "right" | "end" => Ok(TrimSide::Trailing),
        "none" | "off" => Ok(TrimSide::None),
        other => Err(format!(
            "unknown trim '{other}': expected one of both, leading, trailing, none"
        )),
    }
}

fn parse_internal(s: &str) -> Result<Internal, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Ok(Internal::Keep),
        "collapse" => Ok(Internal::Collapse),
        "remove" => Ok(Internal::Remove),
        other => Err(format!(
            "unknown internal '{other}': expected one of keep, collapse, remove"
        )),
    }
}

fn parse_whitespace(s: &str) -> Result<WsClass, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "unicode" => Ok(WsClass::Unicode),
        "ascii" => Ok(WsClass::Ascii),
        other => Err(format!(
            "unknown whitespace '{other}': expected one of unicode, ascii"
        )),
    }
}

fn parse_empty(s: &str) -> Result<EmptyPolicy, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Ok(EmptyPolicy::Keep),
        "null" => Ok(EmptyPolicy::Null),
        "drop" => Ok(EmptyPolicy::Drop),
        other => Err(format!(
            "unknown empty '{other}': expected one of keep, null, drop"
        )),
    }
}

/// Split a comma-separated key list. Each entry is trimmed of ASCII whitespace;
/// blank entries are ignored, so a trailing comma is harmless.
fn key_list(spec: &str) -> Vec<String> {
    spec.split(',')
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect()
}

/// Apply the whitespace pass to one string.
fn clean(value: &str, trim: TrimSide, internal: Internal, ws: WsClass) -> String {
    // Locate the first and last non-whitespace char so the edges and the
    // interior can be treated independently.
    let first = value.char_indices().find(|(_, c)| !ws.is_ws(*c));
    let (start, _) = match first {
        Some(v) => v,
        // All whitespace: there is no interior, only edges.
        None => {
            return match trim {
                TrimSide::None => value.to_string(),
                _ => String::new(),
            }
        }
    };
    let (last_idx, last_ch) = value
        .char_indices()
        .filter(|(_, c)| !ws.is_ws(*c))
        .next_back()
        .expect("a first non-ws char implies a last one");
    let end = last_idx + last_ch.len_utf8();

    let mut out = String::with_capacity(value.len());
    if matches!(trim, TrimSide::None | TrimSide::Trailing) {
        out.push_str(&value[..start]);
    }
    match internal {
        Internal::Keep => out.push_str(&value[start..end]),
        Internal::Collapse | Internal::Remove => {
            let mut in_run = false;
            for c in value[start..end].chars() {
                if ws.is_ws(c) {
                    in_run = true;
                    continue;
                }
                if in_run && internal == Internal::Collapse {
                    out.push(' ');
                }
                in_run = false;
                out.push(c);
            }
        }
    }
    if matches!(trim, TrimSide::None | TrimSide::Leading) {
        out.push_str(&value[end..]);
    }
    out
}

/// Everything the recursive walk needs, resolved once up front.
struct Opts {
    trim: TrimSide,
    internal: Internal,
    ws: WsClass,
    keys: bool,
    empty: EmptyPolicy,
    only_keys: Vec<String>,
    skip_keys: Vec<String>,
}

impl Opts {
    /// Whether the value stored under `key` should be rewritten. The root value
    /// (and every array element outside an object) has no key: `only_keys` then
    /// gates it, because "only these fields" means nothing else qualifies.
    fn selects(&self, key: Option<&str>) -> bool {
        if let Some(k) = key {
            if self.skip_keys.iter().any(|s| s == k) {
                return false;
            }
            if !self.only_keys.is_empty() && !self.only_keys.iter().any(|s| s == k) {
                return false;
            }
            true
        } else {
            self.only_keys.is_empty()
        }
    }
}

/// Outcome of visiting one value: either a replacement, or "delete me".
enum Visited {
    Value(serde_json::Value),
    Dropped,
}

/// `key` is the object key this value is stored under, if any; `path` is the
/// JSONPath-ish location used in error messages.
fn visit(
    value: serde_json::Value,
    key: Option<&str>,
    path: &str,
    opts: &Opts,
) -> Result<Visited, String> {
    match value {
        serde_json::Value::String(s) => {
            if !opts.selects(key) {
                return Ok(Visited::Value(serde_json::Value::String(s)));
            }
            let cleaned = clean(&s, opts.trim, opts.internal, opts.ws);
            if cleaned.is_empty() && !s.is_empty() {
                match opts.empty {
                    EmptyPolicy::Keep => {}
                    EmptyPolicy::Null => return Ok(Visited::Value(serde_json::Value::Null)),
                    EmptyPolicy::Drop => return Ok(Visited::Dropped),
                }
            }
            Ok(Visited::Value(serde_json::Value::String(cleaned)))
        }
        serde_json::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for (i, item) in items.into_iter().enumerate() {
                // An array element inherits the key its array is stored under,
                // so only_keys = "tags" reaches the strings inside "tags".
                match visit(item, key, &format!("{path}[{i}]"), opts)? {
                    Visited::Value(v) => out.push(v),
                    // Dropping from an array would renumber every later index,
                    // so an emptied element becomes null instead of vanishing.
                    Visited::Dropped => out.push(serde_json::Value::Null),
                }
            }
            Ok(Visited::Value(serde_json::Value::Array(out)))
        }
        serde_json::Value::Object(members) => {
            let mut out = serde_json::Map::with_capacity(members.len());
            for (k, v) in members {
                let out_key = if opts.keys {
                    clean(&k, opts.trim, opts.internal, opts.ws)
                } else {
                    k.clone()
                };
                if out.contains_key(&out_key) {
                    return Err(format!(
                        "key collision at {path}: '{k}' trims to '{out_key}', which this object \
                         already has. Set keys = false to leave keys alone, or fix the duplicate."
                    ));
                }
                match visit(v, Some(&k), &format!("{path}.{k}"), opts)? {
                    Visited::Value(nv) => {
                        out.insert(out_key, nv);
                    }
                    Visited::Dropped => {}
                }
            }
            Ok(Visited::Value(serde_json::Value::Object(out)))
        }
        // Numbers, booleans and nulls are copied through untouched — this tool
        // never retypes a value (that is what json-coerce-types is for).
        other => Ok(Visited::Value(other)),
    }
}

/// Trim/normalize the whitespace of every string in a JSON document.
///
/// - `input` — the JSON document as text (any value: object, array, or scalar).
/// - `trim` — `both` (default) | `leading` | `trailing` | `none`.
/// - `internal` — `keep` (default) | `collapse` | `remove`.
/// - `whitespace` — `unicode` (default) | `ascii`.
/// - `keys` — also rewrite object keys.
/// - `empty` — `keep` (default) | `null` | `drop` for a string emptied by the trim.
/// - `only_keys` / `skip_keys` — comma-separated key names limiting the pass.
/// - `indent` — spaces per level in the output; `0` minifies. Clamped to 0..=8.
#[allow(clippy::too_many_arguments)]
pub fn trim_strings(
    input: &str,
    trim: &str,
    internal: &str,
    whitespace: &str,
    keys: bool,
    empty: &str,
    only_keys: &str,
    skip_keys: &str,
    indent: i64,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty: paste a JSON document, e.g. {\"name\":\"  Ada  \"}".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, which is over the {MAX_INPUT_BYTES}-byte limit",
            input.len()
        ));
    }

    let opts = Opts {
        trim: parse_trim(trim)?,
        internal: parse_internal(internal)?,
        ws: parse_whitespace(whitespace)?,
        keys,
        empty: parse_empty(empty)?,
        only_keys: key_list(only_keys),
        skip_keys: key_list(skip_keys),
    };

    let parsed: serde_json::Value = serde_json::from_str(input).map_err(|e| {
        format!(
            "invalid JSON at line {}, column {}: {}",
            e.line(),
            e.column(),
            e
        )
    })?;

    let cleaned = match visit(parsed, None, "$", &opts)? {
        Visited::Value(v) => v,
        // Only reachable if the whole document is one emptied string.
        Visited::Dropped => serde_json::Value::Null,
    };

    let indent = indent.clamp(0, 8) as usize;
    if indent == 0 {
        return serde_json::to_string(&cleaned)
            .map_err(|e| format!("could not serialize the result: {e}"));
    }
    let pad = vec![b' '; indent];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    serde::Serialize::serialize(&cleaned, &mut ser)
        .map_err(|e| format!("could not serialize the result: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("could not serialize the result: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Call with every default, so a test only states what it changes.
    fn run(input: &str) -> Result<String, String> {
        trim_strings(input, "both", "keep", "unicode", false, "keep", "", "", 0)
    }

    #[test]
    fn happy_path_trims_every_string_value_at_any_depth() {
        let out = run(r#"{"name":"  Ada  ","tags":["  x ","y  "],"nested":{"city":" Berlin "}}"#)
            .unwrap();
        assert_eq!(
            out,
            r#"{"name":"Ada","tags":["x","y"],"nested":{"city":"Berlin"}}"#
        );
    }

    #[test]
    fn error_on_invalid_json_names_the_position() {
        let err = run("{\"a\": }").unwrap_err();
        assert!(err.starts_with("invalid JSON at line 1, column 7"), "{err}");
    }

    #[test]
    fn error_on_empty_input_says_what_to_paste() {
        let err = run("   ").unwrap_err();
        assert!(err.contains("paste a JSON document"), "{err}");
    }

    #[test]
    fn non_strings_are_untouched_and_never_retyped() {
        let out = run(r#"{"n":42,"f":1.5,"b":true,"z":null}"#).unwrap();
        assert_eq!(out, r#"{"n":42,"f":1.5,"b":true,"z":null}"#);
    }

    #[test]
    fn structure_key_order_and_array_length_are_preserved() {
        let out = run(r#"{"z":" 1 ","a":" 2 ","list":[" p "," q "," r "]}"#).unwrap();
        assert_eq!(out, r#"{"z":"1","a":"2","list":["p","q","r"]}"#);
    }

    #[test]
    fn trim_sides_are_independent() {
        let doc = r#"{"v":"  hi  "}"#;
        let lead =
            trim_strings(doc, "leading", "keep", "unicode", false, "keep", "", "", 0).unwrap();
        assert_eq!(lead, r#"{"v":"hi  "}"#);
        let trail =
            trim_strings(doc, "trailing", "keep", "unicode", false, "keep", "", "", 0).unwrap();
        assert_eq!(trail, r#"{"v":"  hi"}"#);
        let none = trim_strings(doc, "none", "collapse", "unicode", false, "keep", "", "", 0)
            .unwrap();
        assert_eq!(none, r#"{"v":"  hi  "}"#);
    }

    #[test]
    fn internal_collapse_and_remove_rewrite_the_interior() {
        let doc = r#"{"name":"  Ada   Lovelace  ","sku":" AB 12  CD "}"#;
        let collapse =
            trim_strings(doc, "both", "collapse", "unicode", false, "keep", "", "", 0).unwrap();
        assert_eq!(collapse, r#"{"name":"Ada Lovelace","sku":"AB 12 CD"}"#);
        let remove =
            trim_strings(doc, "both", "remove", "unicode", false, "keep", "", "", 0).unwrap();
        assert_eq!(remove, r#"{"name":"AdaLovelace","sku":"AB12CD"}"#);
    }

    #[test]
    fn trim_none_with_collapse_rewrites_only_the_interior() {
        let out = trim_strings(
            r#"{"v":"  Ada   Lovelace  "}"#,
            "none",
            "collapse",
            "unicode",
            false,
            "keep",
            "",
            "",
            0,
        )
        .unwrap();
        assert_eq!(out, r#"{"v":"  Ada Lovelace  "}"#);
    }

    #[test]
    fn unicode_whitespace_is_trimmed_by_default_and_kept_under_ascii() {
        let doc = "{\"v\":\"\u{00A0}Paris\u{3000}\"}";
        assert_eq!(run(doc).unwrap(), r#"{"v":"Paris"}"#);
        let ascii =
            trim_strings(doc, "both", "keep", "ascii", false, "keep", "", "", 0).unwrap();
        assert_eq!(ascii, "{\"v\":\"\u{00A0}Paris\u{3000}\"}");
    }

    #[test]
    fn zero_width_characters_are_content_not_whitespace() {
        // U+200B ZERO WIDTH SPACE is not in the Unicode White_Space property.
        let out = run("{\"v\":\" \u{200B}x \"}").unwrap();
        assert_eq!(out, "{\"v\":\"\u{200B}x\"}");
    }

    #[test]
    fn keys_are_left_alone_unless_asked_for() {
        let doc = r#"{" first name ":" Ada "}"#;
        assert_eq!(run(doc).unwrap(), r#"{" first name ":"Ada"}"#);
        let with_keys =
            trim_strings(doc, "both", "keep", "unicode", true, "keep", "", "", 0).unwrap();
        assert_eq!(with_keys, r#"{"first name":"Ada"}"#);
    }

    #[test]
    fn error_on_key_collision_instead_of_silently_losing_a_field() {
        let err = trim_strings(
            r#"{" a ":1,"a":2}"#,
            "both",
            "keep",
            "unicode",
            true,
            "keep",
            "",
            "",
            0,
        )
        .unwrap_err();
        assert!(err.contains("key collision at $"), "{err}");
        assert!(err.contains("trims to 'a'"), "{err}");
    }

    #[test]
    fn empty_policy_null_and_drop() {
        let doc = r#"{"a":"   ","b":" x ","c":""}"#;
        let keep = run(doc).unwrap();
        assert_eq!(keep, r#"{"a":"","b":"x","c":""}"#);
        let null =
            trim_strings(doc, "both", "keep", "unicode", false, "null", "", "", 0).unwrap();
        // "c" was already empty, so the trim did not empty it — it stays a string.
        assert_eq!(null, r#"{"a":null,"b":"x","c":""}"#);
        let drop =
            trim_strings(doc, "both", "keep", "unicode", false, "drop", "", "", 0).unwrap();
        assert_eq!(drop, r#"{"b":"x","c":""}"#);
    }

    #[test]
    fn drop_inside_an_array_becomes_null_so_indexes_do_not_shift() {
        let out = trim_strings(
            r#"{"list":[" a ","   "," c "]}"#,
            "both",
            "keep",
            "unicode",
            false,
            "drop",
            "",
            "",
            0,
        )
        .unwrap();
        assert_eq!(out, r#"{"list":["a",null,"c"]}"#);
    }

    #[test]
    fn only_keys_limits_the_pass_and_reaches_into_arrays() {
        let out = trim_strings(
            r#"{"name":" Ada ","hash":"  abc  ","tags":[" x "," y "]}"#,
            "both",
            "keep",
            "unicode",
            false,
            "keep",
            "name,tags",
            "",
            0,
        )
        .unwrap();
        assert_eq!(out, r#"{"name":"Ada","hash":"  abc  ","tags":["x","y"]}"#);
    }

    #[test]
    fn skip_keys_protects_a_field_byte_for_byte() {
        let out = trim_strings(
            r#"{"name":" Ada ","code":"  let x = 1;  "}"#,
            "both",
            "collapse",
            "unicode",
            false,
            "keep",
            "",
            "code",
            0,
        )
        .unwrap();
        assert_eq!(out, r#"{"name":"Ada","code":"  let x = 1;  "}"#);
    }

    #[test]
    fn skip_keys_wins_over_only_keys() {
        let out = trim_strings(
            r#"{"a":" 1 ","b":" 2 "}"#,
            "both",
            "keep",
            "unicode",
            false,
            "keep",
            "a,b",
            "b",
            0,
        )
        .unwrap();
        assert_eq!(out, r#"{"a":"1","b":" 2 "}"#);
    }

    #[test]
    fn indent_pretty_prints_and_zero_minifies() {
        let doc = r#"{"v":" x "}"#;
        let pretty =
            trim_strings(doc, "both", "keep", "unicode", false, "keep", "", "", 2).unwrap();
        assert_eq!(pretty, "{\n  \"v\": \"x\"\n}");
        let four =
            trim_strings(doc, "both", "keep", "unicode", false, "keep", "", "", 4).unwrap();
        assert_eq!(four, "{\n    \"v\": \"x\"\n}");
        let min = trim_strings(doc, "both", "keep", "unicode", false, "keep", "", "", 0).unwrap();
        assert_eq!(min, r#"{"v":"x"}"#);
    }

    #[test]
    fn a_bare_string_document_is_valid_input() {
        assert_eq!(run("\"  hello  \"").unwrap(), "\"hello\"");
    }

    #[test]
    fn escapes_and_real_newlines_inside_a_value_are_handled() {
        // A literal newline is whitespace: trimmed at the edges, collapsed inside.
        let out = trim_strings(
            r#"{"v":"\n  a\n\nb  \t"}"#,
            "both",
            "collapse",
            "unicode",
            false,
            "keep",
            "",
            "",
            0,
        )
        .unwrap();
        assert_eq!(out, r#"{"v":"a b"}"#);
    }

    #[test]
    fn error_on_unknown_option_lists_the_accepted_values() {
        let err = trim_strings(
            r#"{"a":"b"}"#,
            "sideways",
            "keep",
            "unicode",
            false,
            "keep",
            "",
            "",
            0,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "unknown trim 'sideways': expected one of both, leading, trailing, none"
        );
    }

    #[test]
    fn error_when_the_document_is_too_large() {
        let big = format!("\"{}\"", "x".repeat(MAX_INPUT_BYTES));
        let err = run(&big).unwrap_err();
        assert!(err.contains("over the 5000000-byte limit"), "{err}");
    }

    #[test]
    fn indent_is_clamped_rather_than_rejected() {
        let out = trim_strings(r#"{"v":" x "}"#, "both", "keep", "unicode", false, "keep", "", "", 99)
            .unwrap();
        assert_eq!(out, "{\n        \"v\": \"x\"\n}");
    }
}
