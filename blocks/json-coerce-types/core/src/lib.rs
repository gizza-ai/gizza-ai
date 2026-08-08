//! gizza-ai/json-coerce-types core — walk a VALID JSON document and coerce
//! loosely-typed string scalars into native JSON types: `"42"` → `42`,
//! `"true"` → `true`, `"null"` → `null`.
//!
//! The input is parsed and validated first, so malformed JSON is rejected with
//! the parser's exact line/column instead of producing garbled output. Key
//! order is preserved (serde_json `preserve_order`) and nothing is ever
//! deleted — only the TYPE of a string scalar can change. Object keys stay
//! strings (JSON has no other key type); values that don't match a coercion
//! rule survive byte-for-byte.
//!
//! Safety choices that differ from the loose JS libraries:
//!   - **leading zeros are kept as strings by default** (`"0005"`, `"007"`,
//!     zip codes and zero-padded IDs), opt in with `LeadingZeros::Coerce`;
//!   - an integer string too large for `i64`/`u64` stays a **string** rather
//!     than silently losing precision through `f64`;
//!   - `skip_keys` / `only_keys` scope the sweep to the keys you name.
//!
//! Pure-Rust; shared by the chat skill block, the CLI, and the web page. No
//! wafer/wasm-bindgen deps and no host/WASI calls.

use serde::Serialize;
use serde_json::{Map, Number, Value};

/// Largest accepted JSON document, in bytes.
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// How many changed paths the `report` output lists before summarizing.
pub const MAX_REPORT_ROWS: usize = 500;
/// Largest accepted indentation, in spaces.
pub const MAX_INDENT: usize = 8;

/// What to do with a numeric-looking string carrying redundant leading zeros
/// (`"0005"`, `"007"`, `"01"`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeadingZeros {
    /// Leave it as a string — the default, so zip codes and zero-padded IDs
    /// survive. `"0005"` stays `"0005"`.
    Keep,
    /// Coerce it anyway: `"0005"` becomes `5`.
    Coerce,
}

impl LeadingZeros {
    /// Parse the `leading_zeros` param. Blank/unknown falls back to `keep`.
    pub fn parse(s: &str) -> LeadingZeros {
        match s.trim().to_ascii_lowercase().as_str() {
            "coerce" | "strip" | "true" | "yes" => LeadingZeros::Coerce,
            _ => LeadingZeros::Keep,
        }
    }
}

/// What an empty string value becomes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EmptyStrings {
    /// Leave `""` exactly as it is — the default.
    Keep,
    /// Turn `""` into `null` (what CSV importers usually mean by a blank cell).
    Null,
}

impl EmptyStrings {
    /// Parse the `empty_strings` param. Blank/unknown falls back to `keep`.
    pub fn parse(s: &str) -> EmptyStrings {
        match s.trim().to_ascii_lowercase().as_str() {
            "null" | "nullify" | "true" | "yes" => EmptyStrings::Null,
            _ => EmptyStrings::Keep,
        }
    }
}

/// What the tool returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// The retyped JSON document.
    Json,
    /// A plain-text list of every coerced path with its before → after value.
    Report,
}

impl Output {
    /// Parse the `output` param. Blank/unknown falls back to `json`.
    pub fn parse(s: &str) -> Output {
        match s.trim().to_ascii_lowercase().as_str() {
            "report" | "changes" | "diff" => Output::Report,
            _ => Output::Json,
        }
    }
}

/// Everything that steers the sweep.
#[derive(Clone, Debug)]
pub struct Options {
    /// Coerce numeric strings into JSON numbers.
    pub numbers: bool,
    /// Coerce `"true"`/`"false"` (any case) into JSON booleans.
    pub booleans: bool,
    /// Coerce `"null"` (any case) into JSON null.
    pub nulls: bool,
    /// Also treat `yes`/`no`/`on`/`off`/`y`/`n` as booleans.
    pub bool_synonyms: bool,
    /// Extra tokens that become null, e.g. `NA`, `N/A`, `-`. Case-insensitive.
    pub null_tokens: Vec<String>,
    /// What `""` becomes.
    pub empty_strings: EmptyStrings,
    /// Trim leading/trailing whitespace from string values before testing (and
    /// keep the trimmed form when nothing coerces).
    pub trim: bool,
    /// What to do with `"0005"`-style values.
    pub leading_zeros: LeadingZeros,
    /// Accept `,` as a thousands separator, so `"1,234.5"` becomes `1234.5`.
    pub thousands: bool,
    /// Object keys that are never coerced (they and their whole subtree are
    /// left alone). Case-sensitive, exact match.
    pub skip_keys: Vec<String>,
    /// When non-empty, ONLY values under these object keys are considered.
    pub only_keys: Vec<String>,
    /// Spaces of indentation per level; 0 minifies.
    pub indent: usize,
    /// What to return.
    pub output: Output,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            numbers: true,
            booleans: true,
            nulls: true,
            bool_synonyms: false,
            null_tokens: Vec::new(),
            empty_strings: EmptyStrings::Keep,
            trim: false,
            leading_zeros: LeadingZeros::Keep,
            thousands: false,
            skip_keys: Vec::new(),
            only_keys: Vec::new(),
            indent: 2,
            output: Output::Json,
        }
    }
}

/// Split a comma-separated key list into trimmed, non-empty entries.
pub fn split_keys(s: &str) -> Vec<String> {
    s.split(',')
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .map(|k| k.to_string())
        .collect()
}

/// One coerced value, for the report output.
#[derive(Serialize, Debug, PartialEq)]
pub struct Change {
    /// Dotted/bracketed path to the value, e.g. `user.tags[0]`.
    pub path: String,
    /// The original string.
    pub from: String,
    /// The value it became, rendered as compact JSON.
    pub to: String,
}

/// Result of a sweep.
#[derive(Debug)]
pub struct Coerced {
    /// The retyped document.
    pub value: Value,
    /// Every value whose type changed, in document order.
    pub changes: Vec<Change>,
    /// How many string values were examined.
    pub scanned: usize,
}

/// Surface entry point: every option arrives the way the chat schema, the CLI
/// and the page query string spell it (enums as their string variant, key lists
/// comma-separated), so the three wrappers stay one-liners.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    numbers: bool,
    booleans: bool,
    nulls: bool,
    bool_synonyms: bool,
    null_tokens: &str,
    empty_strings: &str,
    trim: bool,
    leading_zeros: &str,
    thousands: bool,
    skip_keys: &str,
    only_keys: &str,
    indent: usize,
    output: &str,
) -> Result<String, String> {
    let opts = Options {
        numbers,
        booleans,
        nulls,
        bool_synonyms,
        null_tokens: split_keys(null_tokens),
        empty_strings: EmptyStrings::parse(empty_strings),
        trim,
        leading_zeros: LeadingZeros::parse(leading_zeros),
        thousands,
        skip_keys: split_keys(skip_keys),
        only_keys: split_keys(only_keys),
        indent: indent.min(MAX_INDENT),
        output: Output::parse(output),
    };
    coerce(input, &opts)
}

/// Coerce `input` and render the requested output.
pub fn coerce(input: &str, opts: &Options) -> Result<String, String> {
    let done = coerce_to_value(input, opts)?;
    match opts.output {
        Output::Json => render(&done.value, opts.indent),
        Output::Report => Ok(render_report(&done)),
    }
}

/// Parse `input`, sweep it, and return the retyped tree plus the change list.
pub fn coerce_to_value(input: &str, opts: &Options) -> Result<Coerced, String> {
    if input.trim().is_empty() {
        return Err(
            "nothing to coerce: the input is empty — paste a JSON document such as {\"age\": \"42\", \"active\": \"true\"}"
                .into(),
        );
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large: {} bytes, maximum {} bytes",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    let value: Value = serde_json::from_str(input)
        .map_err(|e| format!("invalid JSON: {e} — this tool retypes VALID JSON; repair it first"))?;

    let mut w = Walker {
        opts,
        changes: Vec::new(),
        scanned: 0,
    };
    // The root has no key, so it only takes part in the sweep when the caller
    // hasn't narrowed it to specific keys.
    let root_eligible = opts.only_keys.is_empty();
    let value = w.walk(value, "$", root_eligible);
    Ok(Coerced {
        value,
        changes: w.changes,
        scanned: w.scanned,
    })
}

/// Serialize with `indent` spaces per level; 0 minifies onto one line.
pub fn render(value: &Value, indent: usize) -> Result<String, String> {
    if indent == 0 {
        return serde_json::to_string(value).map_err(|e| format!("could not serialize result: {e}"));
    }
    let pad = vec![b' '; indent.min(MAX_INDENT)];
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value
        .serialize(&mut ser)
        .map_err(|e| format!("could not serialize result: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("could not serialize result: {e}"))
}

/// Render the plain-text change report.
fn render_report(done: &Coerced) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} string value(s) scanned, {} coerced.\n",
        done.scanned,
        done.changes.len()
    ));
    if done.changes.is_empty() {
        out.push_str("\nNothing was coerced — every string value was already the right type, was excluded by the key filters, or did not match an enabled rule.\n");
        return out;
    }
    out.push('\n');
    for c in done.changes.iter().take(MAX_REPORT_ROWS) {
        out.push_str(&format!("{}: {:?} -> {}\n", c.path, c.from, c.to));
    }
    if done.changes.len() > MAX_REPORT_ROWS {
        out.push_str(&format!(
            "... and {} more (listing capped at {})\n",
            done.changes.len() - MAX_REPORT_ROWS,
            MAX_REPORT_ROWS
        ));
    }
    out
}

struct Walker<'a> {
    opts: &'a Options,
    changes: Vec<Change>,
    scanned: usize,
}

impl Walker<'_> {
    /// Recurse into `value`. `eligible` says whether a string sitting HERE may
    /// be coerced — it is turned off under a skipped key and turned on under an
    /// `only_keys` key.
    fn walk(&mut self, value: Value, path: &str, eligible: bool) -> Value {
        match value {
            Value::Object(map) => {
                let mut out = Map::with_capacity(map.len());
                for (k, v) in map {
                    if self.opts.skip_keys.iter().any(|s| *s == k) {
                        // The whole subtree under a skipped key is left alone.
                        out.insert(k, v);
                        continue;
                    }
                    let child_eligible = if self.opts.only_keys.iter().any(|s| *s == k) {
                        true
                    } else {
                        eligible
                    };
                    let child_path = format!("{path}.{k}");
                    let v = self.walk(v, &child_path, child_eligible);
                    out.insert(k, v);
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(
                items
                    .into_iter()
                    .enumerate()
                    // Array elements inherit their parent key's eligibility —
                    // an element has no key of its own to match against.
                    .map(|(i, v)| self.walk(v, &format!("{path}[{i}]"), eligible))
                    .collect(),
            ),
            Value::String(s) => {
                self.scanned += 1;
                if !eligible {
                    return Value::String(s);
                }
                let candidate = if self.opts.trim { s.trim() } else { s.as_str() };
                match self.coerce_scalar(candidate) {
                    Some(v) => {
                        self.changes.push(Change {
                            path: path.to_string(),
                            from: s.clone(),
                            to: serde_json::to_string(&v).unwrap_or_else(|_| "?".into()),
                        });
                        v
                    }
                    // Trimming still applies even when nothing coerces, so the
                    // option is useful on its own.
                    None if self.opts.trim && candidate.len() != s.len() => {
                        Value::String(candidate.to_string())
                    }
                    None => Value::String(s),
                }
            }
            other => other,
        }
    }

    /// The single-value rule. Returns `None` when the string stays a string.
    fn coerce_scalar(&self, s: &str) -> Option<Value> {
        if s.is_empty() {
            return match self.opts.empty_strings {
                EmptyStrings::Null => Some(Value::Null),
                EmptyStrings::Keep => None,
            };
        }
        let lower = s.to_ascii_lowercase();
        if self.opts.nulls && lower == "null" {
            return Some(Value::Null);
        }
        if self
            .opts
            .null_tokens
            .iter()
            .any(|t| t.eq_ignore_ascii_case(s))
        {
            return Some(Value::Null);
        }
        if self.opts.booleans {
            match lower.as_str() {
                "true" => return Some(Value::Bool(true)),
                "false" => return Some(Value::Bool(false)),
                _ => {}
            }
            if self.opts.bool_synonyms {
                match lower.as_str() {
                    "yes" | "on" | "y" => return Some(Value::Bool(true)),
                    "no" | "off" | "n" => return Some(Value::Bool(false)),
                    _ => {}
                }
            }
        }
        if self.opts.numbers {
            return self.coerce_number(s);
        }
        None
    }

    /// Numeric coercion: strict JSON number grammar, plus the opt-in
    /// relaxations. Returns `None` for anything that would lose information.
    fn coerce_number(&self, s: &str) -> Option<Value> {
        let cleaned = if self.opts.thousands {
            strip_thousands(s)?
        } else {
            s.to_string()
        };
        let (sign, digits) = match cleaned.strip_prefix('-') {
            Some(rest) => (-1i32, rest),
            None => (1i32, cleaned.as_str()),
        };
        if digits.is_empty() {
            return None;
        }
        // Split into integer part / fraction+exponent tail, validating as we go.
        let int_end = digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len());
        let int_part = &digits[..int_end];
        let tail = &digits[int_end..];
        if int_part.is_empty() {
            // JSON requires a digit before the '.', so ".5" is not a number.
            return None;
        }
        if int_part.len() > 1 && int_part.starts_with('0') && self.opts.leading_zeros == LeadingZeros::Keep {
            return None;
        }
        if !valid_tail(tail) {
            return None;
        }
        if tail.is_empty() {
            // Pure integer — keep it exact, or leave it alone entirely rather
            // than rounding a big value through f64.
            let normalized = format!("{}{}", if sign < 0 { "-" } else { "" }, int_part);
            if let Ok(n) = normalized.parse::<i64>() {
                return Some(Value::Number(n.into()));
            }
            if sign > 0 {
                if let Ok(n) = normalized.parse::<u64>() {
                    return Some(Value::Number(n.into()));
                }
            }
            return None;
        }
        let normalized = format!("{}{}{}", if sign < 0 { "-" } else { "" }, int_part, tail);
        let f = normalized.parse::<f64>().ok()?;
        // "1e400" parses as infinity and has no JSON representation.
        Number::from_f64(f).map(Value::Number)
    }
}

/// Validate the part after the integer digits: an optional `.<digits>` fraction
/// followed by an optional `e[+-]<digits>` exponent, and nothing else.
fn valid_tail(tail: &str) -> bool {
    if tail.is_empty() {
        return true;
    }
    let mut rest = tail;
    if let Some(after_dot) = rest.strip_prefix('.') {
        let frac_end = after_dot
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after_dot.len());
        if frac_end == 0 {
            return false; // "1." is not a JSON number
        }
        rest = &after_dot[frac_end..];
    }
    if rest.is_empty() {
        return true;
    }
    let exp = match rest.strip_prefix(['e', 'E']) {
        Some(e) => e,
        None => return false,
    };
    let exp = exp.strip_prefix(['+', '-']).unwrap_or(exp);
    !exp.is_empty() && exp.bytes().all(|b| b.is_ascii_digit())
}

/// Remove `,` thousands separators, but only from well-formed groupings —
/// `"1,234,567"` yes, `"1,23"` or `"12,34.5,6"` no.
fn strip_thousands(s: &str) -> Option<String> {
    if !s.contains(',') {
        return Some(s.to_string());
    }
    let (body, tail) = match s.find(['.', 'e', 'E']) {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    };
    if tail.contains(',') {
        return None;
    }
    let (sign, digits) = match body.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", body),
    };
    let groups: Vec<&str> = digits.split(',').collect();
    if groups.len() < 2 {
        return None;
    }
    if groups[0].is_empty() || groups[0].len() > 3 {
        return None;
    }
    if groups[1..].iter().any(|g| g.len() != 3) {
        return None;
    }
    if groups.iter().any(|g| !g.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    Some(format!("{sign}{}{tail}", groups.concat()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            indent: 0,
            ..Options::default()
        }
    }

    #[test]
    fn coerces_numbers_booleans_and_nulls_at_any_depth() {
        let out = coerce(
            r#"{"age":"42","active":"true","note":"null","nested":{"score":"3.5","tags":["1","x"]}}"#,
            &opts(),
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"age":42,"active":true,"note":null,"nested":{"score":3.5,"tags":[1,"x"]}}"#
        );
    }

    #[test]
    fn invalid_json_is_rejected_with_position() {
        let err = coerce("{bad}", &opts()).unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "got {err}");
        assert!(err.contains("line 1"), "got {err}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = coerce("   ", &opts()).unwrap_err();
        assert!(err.contains("nothing to coerce"), "got {err}");
    }

    #[test]
    fn leading_zeros_are_kept_by_default_and_coerced_on_demand() {
        let json = r#"{"zip":"02134","id":"007"}"#;
        assert_eq!(coerce(json, &opts()).unwrap(), r#"{"zip":"02134","id":"007"}"#);
        let o = Options {
            leading_zeros: LeadingZeros::Coerce,
            ..opts()
        };
        assert_eq!(coerce(json, &o).unwrap(), r#"{"zip":2134,"id":7}"#);
        // A lone "0" has no redundant zero and always coerces.
        assert_eq!(coerce(r#"["0"]"#, &opts()).unwrap(), "[0]");
    }

    #[test]
    fn oversized_integers_stay_strings_rather_than_losing_precision() {
        // 30 digits: no exact i64/u64 fit, and f64 would round it.
        let out = coerce(r#"{"n":"123456789012345678901234567890"}"#, &opts()).unwrap();
        assert_eq!(out, r#"{"n":"123456789012345678901234567890"}"#);
        // i64::MAX still fits exactly.
        assert_eq!(
            coerce(r#"["9223372036854775807"]"#, &opts()).unwrap(),
            "[9223372036854775807]"
        );
        // Above i64::MAX but inside u64.
        assert_eq!(
            coerce(r#"["9223372036854775808"]"#, &opts()).unwrap(),
            "[9223372036854775808]"
        );
    }

    #[test]
    fn non_json_number_shapes_stay_strings() {
        for s in [
            "0x1f", "1.", ".5", "1e", "+5", "1 2", "NaN", "Infinity", "12px", "", "1,234",
        ] {
            let json = serde_json::to_string(&serde_json::json!([s])).unwrap();
            let out = coerce(&json, &opts()).unwrap();
            assert_eq!(out, json, "{s:?} should have stayed a string");
        }
    }

    #[test]
    fn type_switches_can_be_turned_off_individually() {
        let json = r#"{"a":"1","b":"true","c":"null"}"#;
        let o = Options {
            numbers: false,
            ..opts()
        };
        assert_eq!(coerce(json, &o).unwrap(), r#"{"a":"1","b":true,"c":null}"#);
        let o = Options {
            booleans: false,
            nulls: false,
            ..opts()
        };
        assert_eq!(coerce(json, &o).unwrap(), r#"{"a":1,"b":"true","c":"null"}"#);
    }

    #[test]
    fn boolean_matching_is_case_insensitive_with_opt_in_synonyms() {
        assert_eq!(
            coerce(r#"["TRUE","False","NULL"]"#, &opts()).unwrap(),
            "[true,false,null]"
        );
        assert_eq!(coerce(r#"["yes","off"]"#, &opts()).unwrap(), r#"["yes","off"]"#);
        let o = Options {
            bool_synonyms: true,
            ..opts()
        };
        assert_eq!(coerce(r#"["yes","off","Y","n"]"#, &o).unwrap(), "[true,false,true,false]");
    }

    #[test]
    fn null_tokens_and_empty_strings_are_opt_in() {
        let json = r#"{"a":"NA","b":"","c":"-"}"#;
        assert_eq!(coerce(json, &opts()).unwrap(), json);
        let o = Options {
            null_tokens: split_keys("NA, N/A, -"),
            empty_strings: EmptyStrings::Null,
            ..opts()
        };
        assert_eq!(coerce(json, &o).unwrap(), r#"{"a":null,"b":null,"c":null}"#);
    }

    #[test]
    fn trim_applies_before_testing_and_tidies_leftovers() {
        let json = r#"{"a":"  42  ","b":"  hi  "}"#;
        assert_eq!(coerce(json, &opts()).unwrap(), json);
        let o = Options { trim: true, ..opts() };
        assert_eq!(coerce(json, &o).unwrap(), r#"{"a":42,"b":"hi"}"#);
    }

    #[test]
    fn thousands_separators_are_opt_in_and_grouping_checked() {
        let o = Options {
            thousands: true,
            ..opts()
        };
        assert_eq!(coerce(r#"["1,234","385,134","1,234.5"]"#, &o).unwrap(), "[1234,385134,1234.5]");
        // Malformed groupings are left alone rather than silently mangled.
        assert_eq!(
            coerce(r#"["1,23","12,3456","1,234,5"]"#, &o).unwrap(),
            r#"["1,23","12,3456","1,234,5"]"#
        );
    }

    #[test]
    fn skip_keys_protects_a_whole_subtree() {
        let o = Options {
            skip_keys: split_keys("zip,raw"),
            ..opts()
        };
        let out = coerce(
            r#"{"zip":"2134","age":"42","raw":{"n":"1","deep":["2"]}}"#,
            &o,
        )
        .unwrap();
        assert_eq!(out, r#"{"zip":"2134","age":42,"raw":{"n":"1","deep":["2"]}}"#);
    }

    #[test]
    fn only_keys_narrows_the_sweep_including_nested_arrays() {
        let o = Options {
            only_keys: split_keys("counts"),
            ..opts()
        };
        let out = coerce(r#"{"age":"42","counts":["1","2"],"meta":{"n":"3"}}"#, &o).unwrap();
        assert_eq!(out, r#"{"age":"42","counts":[1,2],"meta":{"n":"3"}}"#);
    }

    #[test]
    fn root_scalar_and_key_order_are_handled() {
        assert_eq!(coerce(r#""42""#, &opts()).unwrap(), "42");
        assert_eq!(
            coerce(r#"{"z":"1","a":"2","m":"3"}"#, &opts()).unwrap(),
            r#"{"z":1,"a":2,"m":3}"#
        );
    }

    #[test]
    fn indent_controls_formatting() {
        let o = Options { indent: 2, ..opts() };
        assert_eq!(coerce(r#"{"a":"1"}"#, &o).unwrap(), "{\n  \"a\": 1\n}");
        let o = Options { indent: 4, ..opts() };
        assert_eq!(coerce(r#"{"a":"1"}"#, &o).unwrap(), "{\n    \"a\": 1\n}");
    }

    #[test]
    fn report_lists_every_change_with_its_path() {
        let o = Options {
            output: Output::Report,
            ..opts()
        };
        let out = coerce(r#"{"age":"42","tags":["true","x"]}"#, &o).unwrap();
        assert!(out.starts_with("3 string value(s) scanned, 2 coerced."), "got {out}");
        assert!(out.contains("$.age: \"42\" -> 42"), "got {out}");
        assert!(out.contains("$.tags[0]: \"true\" -> true"), "got {out}");
    }

    #[test]
    fn report_says_so_when_nothing_changed() {
        let o = Options {
            output: Output::Report,
            ..opts()
        };
        let out = coerce(r#"{"a":"hello"}"#, &o).unwrap();
        assert!(out.contains("Nothing was coerced"), "got {out}");
    }

    #[test]
    fn already_typed_values_are_untouched() {
        let json = r#"{"a":1,"b":true,"c":null,"d":1.5,"e":[],"f":{}}"#;
        assert_eq!(coerce(json, &opts()).unwrap(), json);
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = format!("\"{}\"", "x".repeat(MAX_INPUT_BYTES));
        let err = coerce(&big, &opts()).unwrap_err();
        assert!(err.contains("too large"), "got {err}");
    }

    /// `run` with the schema defaults — the exact call the chat block, the CLI
    /// and the page make when the user changes nothing.
    fn run_default(input: &str) -> Result<String, String> {
        run(
            input, true, true, true, false, "", "keep", false, "keep", false, "", "", 0, "json",
        )
    }

    #[test]
    fn run_wires_the_surface_defaults_through() {
        assert_eq!(
            run_default(r#"{"age":"42","active":"true","note":"null","zip":"02134"}"#).unwrap(),
            r#"{"age":42,"active":true,"note":null,"zip":"02134"}"#
        );
        assert!(run_default("{bad}").unwrap_err().starts_with("invalid JSON:"));
    }

    #[test]
    fn run_parses_every_option_string() {
        let out = run(
            r#"{"a":"  1,234 ","b":"yes","c":"NA","d":"","e":"007","keep":{"n":"1"}}"#,
            true,      // numbers
            true,      // booleans
            true,      // nulls
            true,      // bool_synonyms
            "NA, N/A", // null_tokens
            "null",    // empty_strings
            true,      // trim
            "coerce",  // leading_zeros
            true,      // thousands
            "keep",    // skip_keys
            "",        // only_keys
            0,         // indent
            "json",
        )
        .unwrap();
        assert_eq!(
            out,
            r#"{"a":1234,"b":true,"c":null,"d":null,"e":7,"keep":{"n":"1"}}"#
        );
    }

    #[test]
    fn run_clamps_indent_and_renders_the_report() {
        // Anything above MAX_INDENT is clamped rather than rejected.
        assert_eq!(
            run(
                r#"{"a":"1"}"#, true, true, true, false, "", "keep", false, "keep", false, "", "",
                99, "json",
            )
            .unwrap(),
            format!("{{\n{}\"a\": 1\n}}", " ".repeat(MAX_INDENT))
        );
        let report = run(
            r#"{"a":"1"}"#,
            true,
            true,
            true,
            false,
            "",
            "keep",
            false,
            "keep",
            false,
            "",
            "",
            2,
            "report",
        )
        .unwrap();
        assert!(report.contains("$.a: \"1\" -> 1"), "got {report}");
    }

    #[test]
    fn parsers_of_option_strings_fall_back_to_defaults() {
        assert_eq!(LeadingZeros::parse("coerce"), LeadingZeros::Coerce);
        assert_eq!(LeadingZeros::parse("nonsense"), LeadingZeros::Keep);
        assert_eq!(EmptyStrings::parse("null"), EmptyStrings::Null);
        assert_eq!(EmptyStrings::parse(""), EmptyStrings::Keep);
        assert_eq!(Output::parse("report"), Output::Report);
        assert_eq!(Output::parse("json"), Output::Json);
        assert_eq!(split_keys(" a , ,b "), vec!["a".to_string(), "b".to_string()]);
    }
}
