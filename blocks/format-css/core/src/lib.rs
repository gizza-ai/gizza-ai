//! format-css core — a CSS / SCSS / LESS pretty-printer with declaration
//! ordering, hex-color normalization and per-selector line splitting. Pure
//! compute, shared by the chat skill block and the web page. No wafer /
//! wasm-bindgen deps.
//!
//! The parser is a small forgiving, brace-recursive tokenizer: it understands
//! nested rules (SCSS/LESS), `&` parent references, block at-rules
//! (`@media`/`@mixin`/…) and statement at-rules (`@import`/`@include`/…),
//! `$`/`@` variables, `/* … */` block comments and `//` line comments (LESS/
//! SCSS), preserving string literals and `url(http://…)` verbatim.
//!
//! Formatting is lossless with respect to values: only whitespace, casing of
//! hex colors (opt-in) and — when requested — declaration order change. Values
//! are never rewritten (no unit/shorthand optimization; that is a different
//! tool). Minification is out of scope (see `js-css-minifier`).

/// One indentation unit: `n` spaces per level, or a single tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Indent {
    /// `n` spaces per level (clamped 0..=8).
    Spaces(usize),
    /// A single tab per level.
    Tab,
}

impl Indent {
    /// The indent unit as a string (`"  "`, `"\t"`, `""`).
    fn unit(self) -> String {
        match self {
            Indent::Tab => "\t".to_string(),
            Indent::Spaces(n) => " ".repeat(n.min(8)),
        }
    }
}

/// How to order the declarations within each rule block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    /// Keep the source order (safe default — never changes the cascade).
    None,
    /// Alphabetical by property name.
    Alphabetical,
    /// Curated concentric ("idiomatic") grouping, alphabetical fallback.
    Grouped,
}

/// Parse a sort mode name (`"none"`, `"alphabetical"`/`"alpha"`, `"grouped"`).
pub fn parse_sort(name: &str) -> Option<Sort> {
    match name.trim().to_ascii_lowercase().as_str() {
        "none" | "off" | "source" => Some(Sort::None),
        "alphabetical" | "alpha" | "az" | "a-z" => Some(Sort::Alphabetical),
        "grouped" | "group" | "idiomatic" | "concentric" => Some(Sort::Grouped),
        _ => None,
    }
}

/// Formatting options.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Indentation unit.
    pub indent: Indent,
    /// Declaration ordering within each block.
    pub sort: Sort,
    /// Split a comma-separated selector list onto one selector per line.
    pub selectors_per_line: bool,
    /// Uppercase the hex digits of `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa` colors.
    pub uppercase_hex: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            indent: Indent::Spaces(2),
            sort: Sort::None,
            selectors_per_line: true,
            uppercase_hex: false,
        }
    }
}

/// One node of the parsed stylesheet.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    /// `selector { body }` — also covers block at-rules (`@media … { … }`).
    Rule { selector: String, body: Vec<Node> },
    /// A `prop: value` declaration (or a `$var: value` / `@var: value`), no `;`.
    Declaration(String),
    /// A statement at-rule / bare statement ending in `;` (`@import "x"`,
    /// `@include mixin(1)`), stored without the trailing `;`.
    Statement(String),
    /// A `/* … */` block comment (stored verbatim, trimmed).
    Comment(String),
    /// A `// …` line comment (stored verbatim, trimmed).
    LineComment(String),
}

/// Pretty-print CSS / SCSS / LESS. Returns an error on empty input.
pub fn format(src: &str, opts: Options) -> Result<String, String> {
    if src.trim().is_empty() {
        return Err("no CSS input to format".into());
    }
    let chars: Vec<char> = src.chars().collect();
    let (mut nodes, _) = parse(&chars, 0, true);
    sort_tree(&mut nodes, opts.sort);
    let unit = opts.indent.unit();
    let mut out = String::new();
    render(&nodes, 0, &unit, opts, &mut out);
    let out = out.trim_end().to_string();
    if out.is_empty() {
        return Err("no CSS input to format".into());
    }
    Ok(out + "\n")
}

/// Convenience wrapper used by the CLI/chat default path: format with the
/// default [`Options`].
pub fn run(input: &str) -> Result<String, String> {
    format(input, Options::default())
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Recursively parse a block body starting at `chars[i]`. When `top` is false
/// the scan returns at the matching `}` (index just past it); at the top level
/// it runs to end of input. Returns `(nodes, next_index)`.
fn parse(chars: &[char], mut i: usize, top: bool) -> (Vec<Node>, usize) {
    let n = chars.len();
    let mut nodes: Vec<Node> = Vec::new();
    let mut buf = String::new();
    let mut paren: usize = 0;

    while i < n {
        let c = chars[i];

        // Block comment /* … */
        if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            let mut j = i + 2;
            while j + 1 < n && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            let end = (j + 2).min(n);
            let comment: String = chars[i..end].iter().collect();
            if buf.trim().is_empty() {
                nodes.push(Node::Comment(comment.trim().to_string()));
                buf.clear();
            } else {
                // Comment mid-selector/-declaration: keep it attached inline.
                if !buf.ends_with(' ') {
                    buf.push(' ');
                }
                buf.push_str(comment.trim());
            }
            i = end;
            continue;
        }

        // Line comment // …  (SCSS/LESS) — but NOT inside url(http://…) or a
        // protocol; only when we are at paren depth 0.
        if c == '/'
            && i + 1 < n
            && chars[i + 1] == '/'
            && paren == 0
            && !buf.trim_end().ends_with(':')
        {
            let mut j = i + 2;
            while j < n && chars[j] != '\n' {
                j += 1;
            }
            let comment: String = chars[i..j].iter().collect();
            flush_statement(&mut nodes, &mut buf);
            nodes.push(Node::LineComment(comment.trim_end().to_string()));
            i = j;
            continue;
        }

        // String literal — copy verbatim (incl. any {, }, ;, // inside).
        if c == '"' || c == '\'' {
            buf.push(c);
            i += 1;
            while i < n {
                buf.push(chars[i]);
                if chars[i] == c {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        match c {
            '(' => {
                paren += 1;
                buf.push(c);
                i += 1;
            }
            ')' => {
                paren = paren.saturating_sub(1);
                buf.push(c);
                i += 1;
            }
            '{' => {
                let selector = buf.trim().to_string();
                buf.clear();
                let (body, ni) = parse(chars, i + 1, false);
                nodes.push(Node::Rule { selector, body });
                i = ni;
                paren = 0;
            }
            '}' => {
                flush_statement(&mut nodes, &mut buf);
                if !top {
                    return (nodes, i + 1);
                }
                // Stray '}' at the top level — ignore it.
                i += 1;
            }
            ';' => {
                buf.push(';');
                flush_statement(&mut nodes, &mut buf);
                i += 1;
            }
            _ => {
                buf.push(c);
                i += 1;
            }
        }
    }
    flush_statement(&mut nodes, &mut buf);
    (nodes, i)
}

/// Flush the accumulated buffer as a [`Node::Declaration`] (a `prop: value`
/// with a colon) or a [`Node::Statement`] (a bare at-rule / statement). No-op
/// when the buffer is only whitespace.
fn flush_statement(nodes: &mut Vec<Node>, buf: &mut String) {
    let text = buf.trim().trim_end_matches(';').trim();
    if text.is_empty() {
        buf.clear();
        return;
    }
    // A statement at-rule (`@import …`, `@include …`, `@extend …`, `@use …`)
    // has no top-level colon that separates a property from a value. A `$var:`
    // / `@var:` / `prop:` declaration does. Detect a top-level colon.
    if has_declaration_colon(text) {
        nodes.push(Node::Declaration(text.to_string()));
    } else {
        nodes.push(Node::Statement(text.to_string()));
    }
    buf.clear();
}

/// Is there a `:` at paren/bracket depth 0 (a `prop: value` separator) — as
/// opposed to a `:` only inside `:not(...)`, a `url(...)`, or none at all.
fn has_declaration_colon(s: &str) -> bool {
    let mut depth = 0i32;
    let mut quote = '\0';
    for c in s.chars() {
        if quote != '\0' {
            if c == quote {
                quote = '\0';
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = c,
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ':' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------------------

/// Recursively sort each rule body's declarations per `sort`.
fn sort_tree(nodes: &mut [Node], sort: Sort) {
    for node in nodes.iter_mut() {
        if let Node::Rule { body, .. } = node {
            sort_block(body, sort);
            sort_tree(body, sort);
        }
    }
}

/// Reorder the [`Node::Declaration`] entries of a single block into `sort`
/// order, leaving every non-declaration node (comments, nested rules,
/// statements) fixed in its slot. Stable within equal keys.
fn sort_block(body: &mut [Node], sort: Sort) {
    if sort == Sort::None {
        return;
    }
    let slots: Vec<usize> = body
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n, Node::Declaration(_)))
        .map(|(i, _)| i)
        .collect();
    let mut decls: Vec<Node> = slots.iter().map(|&i| body[i].clone()).collect();
    decls.sort_by(|a, b| {
        let (ka, kb) = (
            decl_prop(a).map(str::to_string),
            decl_prop(b).map(str::to_string),
        );
        match (ka, kb) {
            (Some(pa), Some(pb)) => sort_key(&pa, sort).cmp(&sort_key(&pb, sort)),
            _ => std::cmp::Ordering::Equal,
        }
    });
    for (slot, node) in slots.into_iter().zip(decls) {
        body[slot] = node;
    }
}

/// The property name of a declaration (`color` for `color: red`), lowercased.
fn decl_prop(node: &Node) -> Option<&str> {
    match node {
        Node::Declaration(text) => Some(text.split(':').next().unwrap_or("").trim()),
        _ => None,
    }
}

/// The sort key: `(group_rank, lowercased_prop)`. For [`Sort::Alphabetical`]
/// every property shares group rank 0, so it is a pure alphabetical order.
fn sort_key(prop: &str, sort: Sort) -> (usize, String) {
    let lower = prop.trim().to_ascii_lowercase();
    match sort {
        Sort::None => (0, lower),
        Sort::Alphabetical => (0, lower),
        Sort::Grouped => (group_rank(&lower), lower),
    }
}

/// Curated concentric ("idiomatic") ordering rank for a property: custom
/// properties first, then positioning → box model → border → background/color
/// → typography → visual effects → interaction. Unknown properties get a rank
/// after every known one and fall back to alphabetical (via [`sort_key`]).
fn group_rank(prop: &str) -> usize {
    if prop.starts_with("--") {
        return 0;
    }
    // Longest-prefix match so `border-top-width` ranks near `border`.
    let mut best: Option<usize> = None;
    for (rank, name) in GROUP_ORDER.iter().enumerate() {
        if prop == *name || prop.starts_with(&format!("{name}-")) {
            match best {
                Some(b) if GROUP_ORDER[b].len() >= name.len() => {}
                _ => best = Some(rank),
            }
        }
    }
    // +1 so custom-property rank 0 always precedes the ordered table.
    best.map(|r| r + 1).unwrap_or(usize::MAX)
}

/// The concentric property order (prefix-matched). Not exhaustive — anything
/// missing falls back to alphabetical after this list.
const GROUP_ORDER: &[&str] = &[
    // Positioning
    "position", "z-index", "top", "right", "bottom", "left", "inset",
    // Display / flex / grid
    "display", "flex", "flex-direction", "flex-wrap", "justify-content",
    "align-items", "align-content", "align-self", "order", "grid",
    "grid-template", "grid-template-columns", "grid-template-rows", "grid-area",
    "grid-column", "grid-row", "gap", "row-gap", "column-gap", "float", "clear",
    // Box model
    "box-sizing", "width", "min-width", "max-width", "height", "min-height",
    "max-height", "margin", "padding", "overflow", "overflow-x", "overflow-y",
    // Border / outline
    "border", "border-width", "border-style", "border-color", "border-top",
    "border-right", "border-bottom", "border-left", "border-radius", "outline",
    // Background / color
    "background", "background-color", "background-image", "background-position",
    "background-size", "background-repeat", "color", "opacity",
    // Typography
    "font", "font-family", "font-size", "font-weight", "font-style",
    "line-height", "letter-spacing", "text-align", "text-decoration",
    "text-transform", "text-shadow", "white-space", "word-break",
    // Visual effects
    "box-shadow", "transform", "transition", "animation", "filter",
    "backdrop-filter", "visibility",
    // Interaction
    "cursor", "pointer-events", "user-select",
];

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(nodes: &[Node], depth: usize, unit: &str, opts: Options, out: &mut String) {
    for node in nodes {
        match node {
            Node::Comment(c) | Node::LineComment(c) => {
                indent(out, depth, unit);
                out.push_str(c);
                out.push('\n');
            }
            Node::Statement(s) => {
                indent(out, depth, unit);
                out.push_str(&collapse_ws(s));
                out.push_str(";\n");
            }
            Node::Declaration(d) => {
                indent(out, depth, unit);
                out.push_str(&normalize_declaration(d, opts));
                out.push_str(";\n");
            }
            Node::Rule { selector, body } => {
                render_selector(selector, depth, unit, opts, out);
                render(body, depth + 1, unit, opts, out);
                indent(out, depth, unit);
                out.push_str("}\n");
            }
        }
    }
}

/// Emit `selector {` (optionally splitting a comma list one-per-line).
fn render_selector(selector: &str, depth: usize, unit: &str, opts: Options, out: &mut String) {
    let parts = split_top_level_commas(selector);
    if opts.selectors_per_line && parts.len() > 1 {
        let last = parts.len() - 1;
        for (i, part) in parts.iter().enumerate() {
            indent(out, depth, unit);
            out.push_str(&collapse_ws(part));
            if i == last {
                out.push_str(" {\n");
            } else {
                out.push_str(",\n");
            }
        }
    } else {
        indent(out, depth, unit);
        let joined = parts
            .iter()
            .map(|p| collapse_ws(p))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&joined);
        out.push_str(" {\n");
    }
}

fn indent(out: &mut String, depth: usize, unit: &str) {
    for _ in 0..depth {
        out.push_str(unit);
    }
}

/// Normalize a `prop: value` declaration: collapse whitespace on each side of
/// the first top-level colon, exactly one space after the colon, and (opt-in)
/// uppercase hex colors in the value.
fn normalize_declaration(decl: &str, opts: Options) -> String {
    match split_declaration(decl) {
        Some((prop, value)) => {
            let prop = collapse_ws(prop);
            let mut value = collapse_ws(value);
            if opts.uppercase_hex {
                value = uppercase_hex(&value);
            }
            format!("{prop}: {value}")
        }
        None => collapse_ws(decl),
    }
}

/// Split at the first top-level `:` (the `prop`/`value` separator).
fn split_declaration(decl: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    let mut quote = '\0';
    for (idx, c) in decl.char_indices() {
        if quote != '\0' {
            if c == quote {
                quote = '\0';
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = c,
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ':' if depth == 0 => return Some((&decl[..idx], &decl[idx + 1..])),
            _ => {}
        }
    }
    None
}

/// Collapse runs of whitespace to a single space and trim, without touching
/// the interior of quoted strings.
fn collapse_ws(s: &str) -> String {
    let mut out = String::new();
    let mut in_ws = false;
    let mut quote = '\0';
    for c in s.chars() {
        if quote != '\0' {
            out.push(c);
            if c == quote {
                quote = '\0';
            }
            continue;
        }
        if c == '"' || c == '\'' {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            quote = c;
            out.push(c);
            continue;
        }
        if c.is_whitespace() {
            in_ws = true;
            continue;
        }
        if in_ws && !out.is_empty() {
            out.push(' ');
        }
        in_ws = false;
        out.push(c);
    }
    out
}

/// Uppercase the hex digits of `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa` tokens in a
/// value string. Only runs of exactly 3/4/6/8 hex digits after `#` qualify, so
/// identifiers and lengths that are not colors are left untouched.
fn uppercase_hex(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if chars[i] == '#' {
            let mut j = i + 1;
            while j < n && chars[j].is_ascii_hexdigit() {
                j += 1;
            }
            let len = j - (i + 1);
            // Must be a whole token: not immediately followed by another ident
            // char (letter/digit/-/_) beyond the hex run.
            let bounded = j >= n || !(chars[j].is_ascii_alphanumeric() || chars[j] == '-' || chars[j] == '_');
            if bounded && matches!(len, 3 | 4 | 6 | 8) {
                out.push('#');
                for c in &chars[i + 1..j] {
                    out.push(c.to_ascii_uppercase());
                }
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Split a selector list on top-level commas (ignoring commas inside
/// `:not(a, b)`, `[attr="a,b"]`, and strings).
fn split_top_level_commas(sel: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote = '\0';
    let mut start = 0usize;
    for (idx, c) in sel.char_indices() {
        if quote != '\0' {
            if c == quote {
                quote = '\0';
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = c,
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&sel[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&sel[start..]);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str, opts: Options) -> String {
        format(src, opts).unwrap()
    }

    #[test]
    fn basic_declarations_default() {
        let out = fmt("a{color:red;margin:0}", Options::default());
        assert_eq!(out, "a {\n  color: red;\n  margin: 0;\n}\n");
    }

    #[test]
    fn tab_indent() {
        let opts = Options {
            indent: Indent::Tab,
            ..Options::default()
        };
        assert_eq!(fmt("a{color:red}", opts), "a {\n\tcolor: red;\n}\n");
    }

    #[test]
    fn four_space_indent() {
        let opts = Options {
            indent: Indent::Spaces(4),
            ..Options::default()
        };
        assert_eq!(fmt("a{color:red}", opts), "a {\n    color: red;\n}\n");
    }

    #[test]
    fn nested_scss_with_parent_ref() {
        let out = fmt(".btn{color:red;&:hover{color:blue}}", Options::default());
        assert_eq!(
            out,
            ".btn {\n  color: red;\n  &:hover {\n    color: blue;\n  }\n}\n"
        );
    }

    #[test]
    fn block_at_rule_nests() {
        let out = fmt("@media screen{.a{color:red}}", Options::default());
        assert_eq!(out, "@media screen {\n  .a {\n    color: red;\n  }\n}\n");
    }

    #[test]
    fn statement_at_rule_and_variable() {
        let out = fmt("@import \"x.css\";$brand:#fff;.a{color:$brand}", Options::default());
        assert_eq!(
            out,
            "@import \"x.css\";\n$brand: #fff;\n.a {\n  color: $brand;\n}\n"
        );
    }

    #[test]
    fn multi_selector_split_default_on() {
        let out = fmt("h1,h2 , h3{margin:0}", Options::default());
        assert_eq!(out, "h1,\nh2,\nh3 {\n  margin: 0;\n}\n");
    }

    #[test]
    fn multi_selector_split_off() {
        let opts = Options {
            selectors_per_line: false,
            ..Options::default()
        };
        assert_eq!(fmt("h1,h2,h3{margin:0}", opts), "h1, h2, h3 {\n  margin: 0;\n}\n");
    }

    #[test]
    fn selector_comma_in_not_not_split() {
        let out = fmt(":not(a, b){color:red}", Options::default());
        assert_eq!(out, ":not(a, b) {\n  color: red;\n}\n");
    }

    #[test]
    fn alphabetical_sort() {
        let opts = Options {
            sort: Sort::Alphabetical,
            ..Options::default()
        };
        let out = fmt("a{margin:0;color:red;background:blue}", opts);
        assert_eq!(out, "a {\n  background: blue;\n  color: red;\n  margin: 0;\n}\n");
    }

    #[test]
    fn grouped_sort_positioning_before_color() {
        let opts = Options {
            sort: Sort::Grouped,
            ..Options::default()
        };
        let out = fmt("a{color:red;position:absolute;width:1px}", opts);
        assert_eq!(
            out,
            "a {\n  position: absolute;\n  width: 1px;\n  color: red;\n}\n"
        );
    }

    #[test]
    fn grouped_sort_custom_property_first() {
        let opts = Options {
            sort: Sort::Grouped,
            ..Options::default()
        };
        let out = fmt("a{color:red;--x:1}", opts);
        assert_eq!(out, "a {\n  --x: 1;\n  color: red;\n}\n");
    }

    #[test]
    fn uppercase_hex_on() {
        let opts = Options {
            uppercase_hex: true,
            ..Options::default()
        };
        let out = fmt("a{color:#abcdef;border-color:#0a0}", opts);
        assert_eq!(
            out,
            "a {\n  color: #ABCDEF;\n  border-color: #0A0;\n}\n"
        );
    }

    #[test]
    fn uppercase_hex_off_by_default() {
        let out = fmt("a{color:#abcdef}", Options::default());
        assert_eq!(out, "a {\n  color: #abcdef;\n}\n");
    }

    #[test]
    fn uppercase_hex_leaves_id_selector_alone() {
        // A `#id` in a selector is not a declaration value → untouched.
        let opts = Options {
            uppercase_hex: true,
            ..Options::default()
        };
        let out = fmt("#abc{color:red}", opts);
        assert_eq!(out, "#abc {\n  color: red;\n}\n");
    }

    #[test]
    fn preserves_string_with_semicolon() {
        let out = fmt("a{content:\"a;b\"}", Options::default());
        assert_eq!(out, "a {\n  content: \"a;b\";\n}\n");
    }

    #[test]
    fn url_with_double_slash_not_a_comment() {
        let out = fmt("a{background:url(http://x.com/y.png)}", Options::default());
        assert_eq!(
            out,
            "a {\n  background: url(http://x.com/y.png);\n}\n"
        );
    }

    #[test]
    fn line_comment_preserved() {
        let out = fmt(".a{color:red // hi\n}", Options::default());
        assert_eq!(out, ".a {\n  color: red;\n  // hi\n}\n");
    }

    #[test]
    fn block_comment_on_own_line() {
        let out = fmt("/* header */\na{color:red}", Options::default());
        assert_eq!(out, "/* header */\na {\n  color: red;\n}\n");
    }

    #[test]
    fn empty_input_errors() {
        assert!(format("   ", Options::default()).is_err());
        assert!(format("", Options::default()).is_err());
    }

    #[test]
    fn run_uses_defaults() {
        assert_eq!(run("a{color:red}").unwrap(), "a {\n  color: red;\n}\n");
    }

    #[test]
    fn parse_sort_aliases() {
        assert_eq!(parse_sort("A-Z"), Some(Sort::Alphabetical));
        assert_eq!(parse_sort("grouped"), Some(Sort::Grouped));
        assert_eq!(parse_sort("none"), Some(Sort::None));
        assert_eq!(parse_sort("bogus"), None);
    }
}
