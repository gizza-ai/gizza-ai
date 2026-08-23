//! Rewrite the length units inside a CSS stylesheet: `px` → `rem` (or back)
//! against a configurable root font size.
//!
//! This is a *source rewriter*, not a value calculator — it walks the
//! stylesheet with a small hand-written scanner so that everything which is not
//! a length keeps its exact original bytes: comments, quoted strings, `url(...)`
//! payloads, selectors, at-rule preludes, whitespace and indentation.
//!
//! Conversion rules mirror what the CSS ecosystem's build-time rewriters do:
//! only declaration values are touched (plus, optionally, media-query
//! conditions), a property allow/deny list with wildcards decides which
//! declarations qualify, values below a threshold stay in `px` (the hairline
//! border idiom), and the unit is matched case-sensitively so writing `1Px`
//! opts a single value out.

/// Which way the rewrite runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// `16px` → `1rem` (divide by the root font size).
    PxToRem,
    /// `1rem` → `16px` (multiply by the root font size).
    RemToPx,
}

impl Direction {
    /// Parse the user-facing string form; `None` defaults to [`Direction::PxToRem`].
    pub fn parse(s: Option<&str>) -> Result<Self, String> {
        match s.map(str::trim) {
            None | Some("") | Some("px-to-rem") => Ok(Self::PxToRem),
            Some("rem-to-px") => Ok(Self::RemToPx),
            Some(other) => Err(format!(
                "invalid direction `{other}`: expected one of px-to-rem, rem-to-px"
            )),
        }
    }

    /// The unit we look for in the source.
    fn from_unit(self) -> &'static str {
        match self {
            Self::PxToRem => "px",
            Self::RemToPx => "rem",
        }
    }

    /// The unit we emit.
    fn to_unit(self) -> &'static str {
        match self {
            Self::PxToRem => "rem",
            Self::RemToPx => "px",
        }
    }
}

/// Everything that steers a rewrite. Defaults match [`Options::default`].
#[derive(Debug, Clone)]
pub struct Options {
    /// Which way to convert.
    pub direction: Direction,
    /// The root (`html`) font size in px that 1rem stands for.
    pub root_font_size: f64,
    /// Decimal places kept on the converted number (trailing zeros trimmed).
    pub precision: usize,
    /// Property allow/deny list: comma-separated, `*` wildcards, `!` negation.
    pub properties: String,
    /// Values whose px magnitude is below this are left untouched.
    pub min_pixel_value: f64,
    /// Convert lengths inside `@media` conditions too.
    pub media_queries: bool,
    /// Comma-separated substrings; rules whose selector contains one are skipped.
    pub ignore_selectors: String,
    /// Keep the original declaration and append the converted one after it.
    pub keep_fallback: bool,
    /// Emit a bare `0` instead of `0rem`/`0px` for zero-valued lengths.
    pub unitless_zero: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            direction: Direction::PxToRem,
            root_font_size: 16.0,
            precision: 5,
            properties: "*".to_string(),
            min_pixel_value: 0.0,
            media_queries: false,
            ignore_selectors: String::new(),
            keep_fallback: false,
            unitless_zero: true,
        }
    }
}

/// A compiled property filter using the wildcard grammar shared by the
/// build-time rewriters: `*` (all), `pre*`, `*post`, `*mid*`, exact names, and
/// a leading `!` to exclude. A list with only exclusions implies "all, except".
#[derive(Debug, Clone)]
struct PropFilter {
    include: Vec<Pattern>,
    exclude: Vec<Pattern>,
}

#[derive(Debug, Clone)]
enum Pattern {
    All,
    Exact(String),
    Prefix(String),
    Suffix(String),
    Contains(String),
}

impl Pattern {
    fn parse(raw: &str) -> Self {
        let starts = raw.starts_with('*');
        let ends = raw.ends_with('*') && raw.len() > 1;
        match (starts, ends) {
            (true, true) => {
                let inner = &raw[1..raw.len() - 1];
                if inner.is_empty() {
                    Pattern::All
                } else {
                    Pattern::Contains(inner.to_ascii_lowercase())
                }
            }
            (true, false) => Pattern::Suffix(raw[1..].to_ascii_lowercase()),
            (false, true) => Pattern::Prefix(raw[..raw.len() - 1].to_ascii_lowercase()),
            (false, false) => {
                if raw == "*" {
                    Pattern::All
                } else {
                    Pattern::Exact(raw.to_ascii_lowercase())
                }
            }
        }
    }

    fn matches(&self, prop: &str) -> bool {
        match self {
            Pattern::All => true,
            Pattern::Exact(s) => prop == s,
            Pattern::Prefix(s) => prop.starts_with(s.as_str()),
            Pattern::Suffix(s) => prop.ends_with(s.as_str()),
            Pattern::Contains(s) => prop.contains(s.as_str()),
        }
    }
}

impl PropFilter {
    fn parse(list: &str) -> Self {
        let mut include = Vec::new();
        let mut exclude = Vec::new();
        for raw in list.split(',') {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            if let Some(neg) = raw.strip_prefix('!') {
                let neg = neg.trim();
                if !neg.is_empty() {
                    exclude.push(Pattern::parse(neg));
                }
            } else {
                include.push(Pattern::parse(raw));
            }
        }
        // An empty list, or one made only of exclusions, means "everything else".
        if include.is_empty() {
            include.push(Pattern::All);
        }
        Self { include, exclude }
    }

    fn allows(&self, prop: &str) -> bool {
        let prop = prop.trim().to_ascii_lowercase();
        // Custom properties (`--gap: 16px`) are matched by their literal name.
        self.include.iter().any(|p| p.matches(&prop)) && !self.exclude.iter().any(|p| p.matches(&prop))
    }
}

/// Format a converted number: rounded to `precision` decimals with trailing
/// zeros (and a trailing `.`) trimmed, and `-0` normalized to `0`.
fn format_number(v: f64, precision: usize) -> String {
    let mut s = format!("{v:.precision$}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s.is_empty() || s == "-" {
        s = "0".to_string();
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// True when `c` can appear inside a CSS identifier.
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()
}

/// Convert every qualifying length in one declaration value (or media
/// condition). Returns the rewritten text; identical to the input when nothing
/// qualified.
fn convert_value(value: &str, opts: &Options) -> String {
    let b: Vec<char> = value.chars().collect();
    let mut out = String::with_capacity(value.len());
    let mut i = 0usize;
    let from_unit: Vec<char> = opts.direction.from_unit().chars().collect();

    while i < b.len() {
        let c = b[i];

        // Quoted strings: copy verbatim, honoring backslash escapes.
        if c == '"' || c == '\'' {
            let quote = c;
            out.push(c);
            i += 1;
            while i < b.len() {
                out.push(b[i]);
                if b[i] == '\\' && i + 1 < b.len() {
                    out.push(b[i + 1]);
                    i += 2;
                    continue;
                }
                if b[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Comments inside a value: copy verbatim.
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            out.push('/');
            out.push('*');
            i += 2;
            while i < b.len() {
                if b[i] == '*' && i + 1 < b.len() && b[i + 1] == '/' {
                    out.push('*');
                    out.push('/');
                    i += 2;
                    break;
                }
                out.push(b[i]);
                i += 1;
            }
            continue;
        }

        // Identifiers (incl. `--custom-prop`, `translate3d`, `1px`-suffixed
        // names) are copied whole so a `px` inside a name is never rewritten.
        // A `url(...)` payload is copied along with its balanced parens.
        if c.is_ascii_alphabetic() || c == '_' || (c == '-' && !starts_number(&b, i + 1)) || !c.is_ascii()
        {
            let start = i;
            while i < b.len() && is_ident_char(b[i]) {
                i += 1;
            }
            let ident: String = b[start..i].iter().collect();
            out.push_str(&ident);
            if ident.eq_ignore_ascii_case("url") && i < b.len() && b[i] == '(' {
                let mut depth = 0usize;
                while i < b.len() {
                    let ch = b[i];
                    out.push(ch);
                    i += 1;
                    if ch == '(' {
                        depth += 1;
                    } else if ch == ')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
            }
            continue;
        }

        // Hex colors: `#1px` is not a length.
        if c == '#' {
            out.push('#');
            i += 1;
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                out.push(b[i]);
                i += 1;
            }
            continue;
        }

        // A number, optionally signed, optionally followed by our unit.
        if starts_number(&b, i) || ((c == '-' || c == '+') && starts_number(&b, i + 1)) {
            let start = i;
            if b[i] == '-' || b[i] == '+' {
                i += 1;
            }
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i < b.len() && b[i] == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let num_text: String = b[start..i].iter().collect();

            // Unit match is CASE-SENSITIVE: `1Px`/`1PX` is the documented
            // per-value opt-out, and it must be followed by a non-ident char.
            let unit_end = i + from_unit.len();
            let unit_matches = unit_end <= b.len()
                && b[i..unit_end] == from_unit[..]
                && (unit_end == b.len() || !is_ident_char(b[unit_end]));

            if unit_matches {
                if let Some(converted) = convert_one(&num_text, opts) {
                    out.push_str(&converted);
                    i = unit_end;
                    continue;
                }
            }
            out.push_str(&num_text);
            continue;
        }

        out.push(c);
        i += 1;
    }
    out
}

/// True when a number literal starts at `idx` (a digit, or `.` then a digit).
fn starts_number(b: &[char], idx: usize) -> bool {
    match b.get(idx) {
        Some(c) if c.is_ascii_digit() => true,
        Some('.') => matches!(b.get(idx + 1), Some(c) if c.is_ascii_digit()),
        _ => false,
    }
}

/// Convert a single numeric literal + unit, or `None` to leave it as-is
/// (unparseable, or below `min_pixel_value`).
fn convert_one(num_text: &str, opts: &Options) -> Option<String> {
    let n: f64 = num_text.parse().ok()?;
    // `min_pixel_value` is expressed in px in both directions, so compare the
    // px-side magnitude: the source value for px→rem, the result for rem→px.
    let px_magnitude = match opts.direction {
        Direction::PxToRem => n.abs(),
        Direction::RemToPx => (n * opts.root_font_size).abs(),
    };
    if opts.min_pixel_value > 0.0 && px_magnitude < opts.min_pixel_value {
        return None;
    }
    let converted = match opts.direction {
        Direction::PxToRem => n / opts.root_font_size,
        Direction::RemToPx => n * opts.root_font_size,
    };
    let formatted = format_number(converted, opts.precision);
    if opts.unitless_zero && (formatted == "0" || formatted == "-0") {
        return Some("0".to_string());
    }
    Some(format!("{formatted}{}", opts.direction.to_unit()))
}

/// Where the scanner currently is in the stylesheet.
#[derive(PartialEq)]
enum State {
    /// Accumulating a selector, an at-rule prelude, or a property name — which
    /// of those it was is only known at the terminator.
    Head,
    /// Past a `:`; this is either a declaration value or part of a selector
    /// (`a:hover`), decided by whether `{` or `;`/`}` comes first.
    Value,
}

/// Rewrite `css`, converting qualifying lengths per `opts`.
///
/// Returns an error only for options that cannot produce meaningful output
/// (a non-positive root font size).
pub fn convert(css: &str, opts: &Options) -> Result<String, String> {
    if !(opts.root_font_size.is_finite()) || opts.root_font_size <= 0.0 {
        return Err(format!(
            "invalid root_font_size `{}`: expected a positive number of pixels (e.g. 16)",
            opts.root_font_size
        ));
    }
    if opts.precision > 10 {
        return Err(format!(
            "invalid precision `{}`: expected 0-10 decimal places",
            opts.precision
        ));
    }

    let filter = PropFilter::parse(&opts.properties);
    let ignores: Vec<String> = opts
        .ignore_selectors
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let b: Vec<char> = css.chars().collect();
    let mut out = String::with_capacity(css.len() + css.len() / 8);
    let mut head = String::new();
    let mut value = String::new();
    let mut state = State::Head;
    // Selector stack for `ignore_selectors`; at-rule preludes join it too so a
    // rule nested in an ignored block stays ignored.
    let mut stack: Vec<String> = Vec::new();
    let mut i = 0usize;

    // Helper: is any enclosing selector ignored?
    macro_rules! ignored_now {
        () => {
            !ignores.is_empty()
                && stack
                    .iter()
                    .any(|sel| ignores.iter().any(|ig| sel.contains(ig.as_str())))
        };
    }

    while i < b.len() {
        let c = b[i];

        // Comments anywhere outside a string: copy verbatim into whichever
        // buffer is active so their position is preserved exactly.
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            let start = i;
            i += 2;
            while i < b.len() && !(b[i] == '*' && i + 1 < b.len() && b[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            let text: String = b[start..i].iter().collect();
            match state {
                State::Head => head.push_str(&text),
                State::Value => value.push_str(&text),
            }
            continue;
        }

        // Quoted strings: copy verbatim (a `{`/`;` inside must not be structural).
        if c == '"' || c == '\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < b.len() {
                if b[i] == '\\' && i + 1 < b.len() {
                    i += 2;
                    continue;
                }
                if b[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let text: String = b[start..i].iter().collect();
            match state {
                State::Head => head.push_str(&text),
                State::Value => value.push_str(&text),
            }
            continue;
        }

        match c {
            ':' if state == State::Head => {
                state = State::Value;
                i += 1;
            }
            '{' => {
                // Everything gathered so far was a selector / at-rule prelude,
                // including any `:` we optimistically treated as a declaration.
                let mut prelude = head.clone();
                if state == State::Value {
                    prelude.push(':');
                    prelude.push_str(&value);
                }
                let is_media = prelude.trim_start().to_ascii_lowercase().starts_with("@media");
                let emit = if is_media && opts.media_queries && !ignored_now!() {
                    convert_value(&prelude, opts)
                } else {
                    prelude.clone()
                };
                out.push_str(&emit);
                out.push('{');
                stack.push(prelude.trim().to_ascii_lowercase());
                head.clear();
                value.clear();
                state = State::Head;
                i += 1;
            }
            '}' => {
                out.push_str(&flush(&head, &value, state == State::Value, &filter, opts, ignored_now!()));
                out.push('}');
                stack.pop();
                head.clear();
                value.clear();
                state = State::Head;
                i += 1;
            }
            ';' => {
                out.push_str(&flush(&head, &value, state == State::Value, &filter, opts, ignored_now!()));
                out.push(';');
                head.clear();
                value.clear();
                state = State::Head;
                i += 1;
            }
            _ => {
                match state {
                    State::Head => head.push(c),
                    State::Value => value.push(c),
                }
                i += 1;
            }
        }
    }

    // Trailing text (a final declaration with no `;`, or stray whitespace).
    out.push_str(&flush(&head, &value, state == State::Value, &filter, opts, ignored_now!()));
    Ok(out)
}

/// Emit one buffered chunk. When it is a real declaration whose property passes
/// the filter, its value is converted (and optionally preceded by the original
/// declaration as a fallback).
fn flush(
    head: &str,
    value: &str,
    is_declaration: bool,
    filter: &PropFilter,
    opts: &Options,
    ignored: bool,
) -> String {
    if !is_declaration {
        // A selector fragment, an at-statement (`@import ...;`) or whitespace:
        // never a length we own.
        return head.to_string();
    }
    let prop = head.trim();
    if ignored || !filter.allows(prop) {
        return format!("{head}:{value}");
    }
    let converted = convert_value(value, opts);
    if converted == value {
        return format!("{head}:{value}");
    }
    if opts.keep_fallback {
        // `margin: 16px;` then the same declaration in the new unit, reusing
        // the original leading whitespace so indentation lines up.
        format!("{head}:{value};{head}:{converted}")
    } else {
        format!("{head}:{converted}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn converts_a_simple_declaration() {
        let got = convert("a { font-size: 24px; }", &opts()).unwrap();
        assert_eq!(got, "a { font-size: 1.5rem; }");
    }

    #[test]
    fn converts_multiple_lengths_in_one_value() {
        let got = convert(".c{margin:16px 8px 4px 0px}", &opts()).unwrap();
        assert_eq!(got, ".c{margin:1rem 0.5rem 0.25rem 0}");
    }

    #[test]
    fn zero_becomes_unitless_unless_disabled() {
        let mut o = opts();
        assert_eq!(convert("a{margin:0px}", &o).unwrap(), "a{margin:0}");
        o.unitless_zero = false;
        assert_eq!(convert("a{margin:0px}", &o).unwrap(), "a{margin:0rem}");
    }

    #[test]
    fn honors_a_custom_root_font_size() {
        let mut o = opts();
        o.root_font_size = 10.0;
        assert_eq!(
            convert("a{font-size:18px}", &o).unwrap(),
            "a{font-size:1.8rem}"
        );
    }

    #[test]
    fn rounds_to_the_requested_precision_and_trims_zeros() {
        let mut o = opts();
        o.precision = 3;
        assert_eq!(convert("a{width:15px}", &o).unwrap(), "a{width:0.938rem}");
        o.precision = 0;
        assert_eq!(convert("a{width:15px}", &o).unwrap(), "a{width:1rem}");
        o.precision = 5;
        assert_eq!(convert("a{width:32px}", &o).unwrap(), "a{width:2rem}");
    }

    #[test]
    fn reverse_direction_multiplies() {
        let mut o = opts();
        o.direction = Direction::RemToPx;
        assert_eq!(
            convert("a{font-size:1.5rem;margin:0.25rem}", &o).unwrap(),
            "a{font-size:24px;margin:4px}"
        );
    }

    #[test]
    fn min_pixel_value_keeps_hairline_borders() {
        let mut o = opts();
        o.min_pixel_value = 2.0;
        assert_eq!(
            convert("a{border:1px solid red;padding:16px}", &o).unwrap(),
            "a{border:1px solid red;padding:1rem}"
        );
    }

    #[test]
    fn property_allow_list_with_wildcards_and_negation() {
        let mut o = opts();
        o.properties = "font-size,*margin*".to_string();
        assert_eq!(
            convert("a{font-size:16px;margin-top:16px;padding:16px}", &o).unwrap(),
            "a{font-size:1rem;margin-top:1rem;padding:16px}"
        );

        o.properties = "*,!padding*".to_string();
        assert_eq!(
            convert("a{font-size:16px;padding-top:16px}", &o).unwrap(),
            "a{font-size:1rem;padding-top:16px}"
        );

        // Only-negations behaves as "everything except".
        o.properties = "!font-size".to_string();
        assert_eq!(
            convert("a{font-size:16px;width:16px}", &o).unwrap(),
            "a{font-size:16px;width:1rem}"
        );
    }

    #[test]
    fn media_queries_are_opt_in() {
        let css = "@media (min-width: 640px) { a { width: 32px; } }";
        let mut o = opts();
        assert_eq!(
            convert(css, &o).unwrap(),
            "@media (min-width: 640px) { a { width: 2rem; } }"
        );
        o.media_queries = true;
        assert_eq!(
            convert(css, &o).unwrap(),
            "@media (min-width: 40rem) { a { width: 2rem; } }"
        );
    }

    #[test]
    fn ignore_selectors_skips_matching_rules() {
        let mut o = opts();
        o.ignore_selectors = ".no-rem, #legacy".to_string();
        assert_eq!(
            convert("a{width:16px}.no-rem{width:16px}#legacy p{width:16px}", &o).unwrap(),
            "a{width:1rem}.no-rem{width:16px}#legacy p{width:16px}"
        );
    }

    #[test]
    fn keep_fallback_emits_both_declarations() {
        let mut o = opts();
        o.keep_fallback = true;
        assert_eq!(
            convert("a {\n  width: 16px;\n}", &o).unwrap(),
            "a {\n  width: 16px;\n  width: 1rem;\n}"
        );
    }

    #[test]
    fn leaves_comments_strings_urls_and_idents_alone() {
        let css = concat!(
            "/* 16px stays */\n",
            "a { background: url(\"a-16px.png\") no-repeat; content: \"32px\"; ",
            "transform: translate3d(0, 16px, 0); }"
        );
        let got = convert(css, &opts()).unwrap();
        assert!(got.contains("/* 16px stays */"), "got: {got}");
        assert!(got.contains("url(\"a-16px.png\")"), "got: {got}");
        assert!(got.contains("content: \"32px\""), "got: {got}");
        assert!(got.contains("translate3d(0, 1rem, 0)"), "got: {got}");
    }

    #[test]
    fn uppercase_unit_is_the_per_value_opt_out() {
        assert_eq!(
            convert("a{width:16Px;height:16px}", &opts()).unwrap(),
            "a{width:16Px;height:1rem}"
        );
    }

    #[test]
    fn pseudo_selectors_are_not_mistaken_for_declarations() {
        let got = convert("a:hover { padding: 8px; }", &opts()).unwrap();
        assert_eq!(got, "a:hover { padding: 0.5rem; }");
    }

    #[test]
    fn nested_and_at_rules_survive() {
        let css = "@import url(\"x.css\");\n.btn { width: 16px;\n  &:hover { width: 32px; } }";
        let got = convert(css, &opts()).unwrap();
        assert_eq!(
            got,
            "@import url(\"x.css\");\n.btn { width: 1rem;\n  &:hover { width: 2rem; } }"
        );
    }

    #[test]
    fn custom_properties_are_converted_and_filterable() {
        let mut o = opts();
        assert_eq!(convert(":root{--gap:16px}", &o).unwrap(), ":root{--gap:1rem}");
        o.properties = "!--*".to_string();
        assert_eq!(convert(":root{--gap:16px}", &o).unwrap(), ":root{--gap:16px}");
    }

    #[test]
    fn negative_and_fractional_values() {
        assert_eq!(
            convert("a{margin:-8px;top:.5px}", &opts()).unwrap(),
            "a{margin:-0.5rem;top:0.03125rem}"
        );
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert_eq!(convert("", &opts()).unwrap(), "");
    }

    #[test]
    fn zero_root_font_size_is_an_error() {
        let mut o = opts();
        o.root_font_size = 0.0;
        let err = convert("a{width:16px}", &o).unwrap_err();
        assert!(err.contains("invalid root_font_size"), "got: {err}");
    }

    #[test]
    fn out_of_range_precision_is_an_error() {
        let mut o = opts();
        o.precision = 11;
        let err = convert("a{width:16px}", &o).unwrap_err();
        assert!(err.contains("invalid precision"), "got: {err}");
    }

    #[test]
    fn direction_parse_defaults_and_rejects() {
        assert_eq!(Direction::parse(None).unwrap(), Direction::PxToRem);
        assert_eq!(Direction::parse(Some("px-to-rem")).unwrap(), Direction::PxToRem);
        assert_eq!(Direction::parse(Some("rem-to-px")).unwrap(), Direction::RemToPx);
        let err = Direction::parse(Some("px-to-em")).unwrap_err();
        assert!(err.contains("invalid direction"), "got: {err}");
    }
}
