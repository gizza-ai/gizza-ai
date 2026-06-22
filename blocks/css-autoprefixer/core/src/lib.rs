//! css-autoprefixer core — pure compute, shared by the chat skill block and the web page.
//!
//! Adds vendor prefixes (`-webkit-`, `-moz-`, `-ms-`, `-o-`) to the CSS declarations
//! that still need them in modern browsers, leaving everything else untouched. The
//! prefix set is a curated table (see `PROP_PREFIXES` / value-prefix handling) of the
//! properties and values that real-world autoprefixers still emit for current targets;
//! it is intentionally NOT exhaustive of every historical prefix.
//!
//! The parser is a forgiving declaration scanner: it walks the stylesheet a character
//! at a time, tracks whether it is inside a `{ … }` rule body, skips `/* … */`
//! comments and quoted strings, and for each `prop: value;` declaration inside a body
//! emits the needed prefixed clones immediately BEFORE the original declaration
//! (un-prefixed last, so it wins on a fully-supporting browser). Output indentation +
//! formatting of the original is preserved; the tool never reformats the input.

/// Which vendor prefixes a *property* still needs, in cascade order (prefixed first,
/// standard last). Only properties whose prefixed name is `<prefix>property` go here;
/// properties that change their *name* under a prefix are handled in `prefixed_name`.
const PROP_PREFIXES: &[(&str, &[&str])] = &[
    ("appearance", &["-webkit-", "-moz-"]),
    ("user-select", &["-webkit-", "-moz-", "-ms-"]),
    ("backdrop-filter", &["-webkit-"]),
    ("box-decoration-break", &["-webkit-"]),
    ("clip-path", &["-webkit-"]),
    ("mask", &["-webkit-"]),
    ("mask-image", &["-webkit-"]),
    ("mask-size", &["-webkit-"]),
    ("mask-position", &["-webkit-"]),
    ("mask-repeat", &["-webkit-"]),
    ("mask-clip", &["-webkit-"]),
    ("mask-origin", &["-webkit-"]),
    ("mask-composite", &["-webkit-"]),
    ("hyphens", &["-webkit-", "-ms-"]),
    ("text-size-adjust", &["-webkit-", "-moz-", "-ms-"]),
    ("text-decoration-skip-ink", &["-webkit-"]),
    ("text-emphasis", &["-webkit-"]),
    ("text-emphasis-position", &["-webkit-"]),
    ("text-orientation", &["-webkit-"]),
    ("writing-mode", &["-webkit-", "-ms-"]),
    ("font-feature-settings", &["-webkit-", "-moz-"]),
    ("background-clip", &["-webkit-"]),
    ("box-reflect", &["-webkit-"]),
    ("filter", &["-webkit-"]),
    ("touch-action", &["-ms-"]),
    ("print-color-adjust", &["-webkit-"]),
];

/// Properties whose prefixed form uses a *different name* (not just a prefix glued on).
/// Maps the standard property to its `(prefix-name)` clones.
const PROP_RENAMES: &[(&str, &[&str])] = &[
    ("tab-size", &["-moz-tab-size", "-o-tab-size"]),
    ("background-clip", &["-webkit-background-clip"]),
];

/// The minimum length a token must reach before we bother looking at a property —
/// guards against pathological inputs but mainly documents intent. (Unused as a hard
/// limit; declarations of any length are processed.)
const _: () = ();

#[derive(Clone)]
struct Decl {
    /// Bytes of leading whitespace/indentation captured before the property name.
    indent: String,
    prop: String,
    value: String,
    /// The exact trailing text after the value up to and including the `;` (or `}`
    /// boundary) so we reproduce the author's spacing/terminator verbatim.
    terminator: String,
}

/// Compute the prefixed clones for `d`, then (when `dedup`) drop any whose
/// `prop:value` key already appears in `seen`, recording the kept ones in `seen`.
fn clones_collect(d: &Decl, dedup: bool, seen: &mut Vec<String>) -> Vec<Decl> {
    let mut kept = Vec::new();
    for cl in prefixed_clones(d) {
        if dedup {
            let key = format!("{}:{}", cl.prop.trim().to_ascii_lowercase(), cl.value.trim());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
        }
        kept.push(cl);
    }
    kept
}

/// Build the prefixed clones for one declaration, in cascade order, NOT including the
/// original. Returns an empty Vec when nothing needs prefixing.
fn prefixed_clones(d: &Decl) -> Vec<Decl> {
    let prop_lc = d.prop.trim().to_ascii_lowercase();
    let mut out = Vec::new();

    // 1. Property-name renames (tab-size, background-clip:text variants).
    for (std, names) in PROP_RENAMES {
        if prop_lc == *std {
            // background-clip only needs -webkit- when the value is `text`.
            if *std == "background-clip" && !value_has_word(&d.value, "text") {
                continue;
            }
            for name in *names {
                out.push(Decl {
                    indent: d.indent.clone(),
                    prop: apply_case(&d.prop, name),
                    value: d.value.clone(),
                    terminator: d.terminator.clone(),
                });
            }
        }
    }

    // 2. Straight `<prefix>property` clones.
    for (std, prefixes) in PROP_PREFIXES {
        if prop_lc == *std {
            // background-clip standard name also gets the plain -webkit- prefix when
            // value is `text` (Safari historically needed -webkit-background-clip,
            // already covered above; the plain -webkit-background-clip equals it, so
            // skip the generic path for background-clip to avoid a dup).
            if *std == "background-clip" {
                continue;
            }
            for pre in *prefixes {
                out.push(Decl {
                    indent: d.indent.clone(),
                    prop: format!("{}{}", pre, apply_case(&d.prop, &prop_lc)),
                    value: prefix_value(&prop_lc, &d.value),
                    terminator: d.terminator.clone(),
                });
            }
        }
    }

    // 3. Value-prefixed declarations (display:flex, position:sticky, etc.) where the
    //    PROPERTY name is unchanged but the VALUE needs a prefix.
    out.extend(value_prefix_clones(d, &prop_lc));

    out
}

/// Returns true if `value` contains `word` as a standalone token (case-insensitive),
/// not as a substring of a larger identifier.
fn value_has_word(value: &str, word: &str) -> bool {
    value
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .any(|t| t.eq_ignore_ascii_case(word))
}

/// Apply the casing of `original` (all-upper / all-lower / mixed) loosely: autoprefixers
/// emit lowercase property names, which is what we do — `target` is already lowercase.
/// Kept as a hook; currently returns the (already-lowercased) `target` unchanged.
fn apply_case(_original: &str, target: &str) -> String {
    target.to_string()
}

/// Some prefixed property values also need rewriting (e.g. `mask` referencing gradients
/// is fine; `filter` values are unchanged). Currently a pass-through — present so the
/// prefixed clone always routes its value through one place.
fn prefix_value(_prop: &str, value: &str) -> String {
    value.to_string()
}

/// Value-level prefixing: properties whose name stays the same but whose VALUE needs a
/// vendor-prefixed form. Each entry maps (property, standard-value) → the prefixed
/// `(property, value)` declarations to emit before the original.
fn value_prefix_clones(d: &Decl, prop_lc: &str) -> Vec<Decl> {
    let val_lc = d.value.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    let mut push = |prop: &str, val: &str| {
        out.push(Decl {
            indent: d.indent.clone(),
            prop: prop.to_string(),
            value: val.to_string(),
            terminator: d.terminator.clone(),
        });
    };
    match (prop_lc, val_lc.as_str()) {
        ("display", "flex") => {
            push("display", "-webkit-box");
            push("display", "-webkit-flex");
            push("display", "-ms-flexbox");
        }
        ("display", "inline-flex") => {
            push("display", "-webkit-inline-box");
            push("display", "-webkit-inline-flex");
            push("display", "-ms-inline-flexbox");
        }
        ("display", "grid") => {
            push("display", "-ms-grid");
        }
        ("display", "inline-grid") => {
            push("display", "-ms-inline-grid");
        }
        ("position", "sticky") => {
            push("position", "-webkit-sticky");
        }
        _ => {
            // width:max-content / min-content / fit-content still want -webkit-/-moz-
            // forms on some intrinsic-size properties.
            if matches!(prop_lc, "width" | "height" | "min-width" | "min-height" | "max-width" | "max-height")
                && matches!(val_lc.as_str(), "max-content" | "min-content" | "fit-content")
            {
                push(prop_lc, &format!("-webkit-{}", val_lc));
                push(prop_lc, &format!("-moz-{}", val_lc));
            }
        }
    }
    out
}

/// Add vendor prefixes to `css`. Returns the prefixed stylesheet. Never fails for
/// well-formed-ish input; an empty input yields an empty string. `dedup` (when true)
/// suppresses emitting a prefixed declaration that is already present verbatim earlier
/// in the same rule body.
pub fn prefix(css: &str, dedup: bool) -> Result<String, String> {
    if css.trim().is_empty() {
        return Err("input CSS is empty".into());
    }
    let bytes: Vec<char> = css.chars().collect();
    let n = bytes.len();
    let mut out = String::with_capacity(css.len() + css.len() / 4);
    // Brace depth: declarations only meaningful when depth > 0 (inside a rule body).
    let mut depth: i32 = 0;
    let mut i = 0usize;
    // Per-rule-body set of already-emitted declaration strings (trimmed) for dedup.
    let mut seen: Vec<String> = Vec::new();

    while i < n {
        let c = bytes[i];
        // Comments: copy verbatim.
        if c == '/' && i + 1 < n && bytes[i + 1] == '*' {
            let start = i;
            i += 2;
            while i + 1 < n && !(bytes[i] == '*' && bytes[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(n);
            out.extend(&bytes[start..i]);
            continue;
        }
        // Strings: copy verbatim.
        if c == '"' || c == '\'' {
            let quote = c;
            out.push(c);
            i += 1;
            while i < n {
                out.push(bytes[i]);
                if bytes[i] == '\\' && i + 1 < n {
                    out.push(bytes[i + 1]);
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if c == '{' {
            depth += 1;
            seen.clear();
            out.push(c);
            i += 1;
            continue;
        }
        if c == '}' {
            depth -= 1;
            if depth < 0 {
                depth = 0;
            }
            seen.clear();
            out.push(c);
            i += 1;
            continue;
        }

        if depth > 0 {
            // Try to read a declaration: [whitespace] prop : value (; | lookahead })
            // Capture indentation.
            let decl_start = i;
            let mut j = i;
            // leading whitespace
            while j < n && (bytes[j] == ' ' || bytes[j] == '\t' || bytes[j] == '\n' || bytes[j] == '\r') {
                j += 1;
            }
            let indent: String = bytes[i..j].iter().collect();
            // property name: letters, digits, '-' (and leading -- for custom props)
            let prop_start = j;
            while j < n {
                let pc = bytes[j];
                if pc.is_ascii_alphanumeric() || pc == '-' || pc == '_' {
                    j += 1;
                } else {
                    break;
                }
            }
            let prop: String = bytes[prop_start..j].iter().collect();
            // optional whitespace then ':'
            let mut k = j;
            while k < n && (bytes[k] == ' ' || bytes[k] == '\t') {
                k += 1;
            }
            if !prop.is_empty() && k < n && bytes[k] == ':' && !prop.starts_with("--") {
                k += 1; // consume ':'
                // skip spaces after colon
                while k < n && (bytes[k] == ' ' || bytes[k] == '\t') {
                    k += 1;
                }
                // value: read until ';' or '}' or end, respecting strings/parens.
                let val_start = k;
                let mut paren = 0i32;
                while k < n {
                    let vc = bytes[k];
                    if vc == '(' {
                        paren += 1;
                    } else if vc == ')' {
                        if paren > 0 {
                            paren -= 1;
                        }
                    } else if vc == '"' || vc == '\'' {
                        let q = vc;
                        k += 1;
                        while k < n {
                            if bytes[k] == '\\' && k + 1 < n {
                                k += 2;
                                continue;
                            }
                            if bytes[k] == q {
                                k += 1;
                                break;
                            }
                            k += 1;
                        }
                        continue;
                    } else if (vc == ';' || vc == '}') && paren == 0 {
                        break;
                    }
                    k += 1;
                }
                let raw_value: String = bytes[val_start..k].iter().collect();
                // terminator: the ';' if present (don't consume a closing '}').
                let (terminator, after) = if k < n && bytes[k] == ';' {
                    (";".to_string(), k + 1)
                } else {
                    (String::new(), k)
                };
                // Trim trailing whitespace off the value but keep it in the terminator
                // so re-rendering is faithful.
                let trimmed_len = raw_value.trim_end().len();
                let value = raw_value[..trimmed_len].to_string();
                let trailing_ws = raw_value[trimmed_len..].to_string();
                let terminator = format!("{}{}", trailing_ws, terminator);

                let decl = Decl {
                    indent: indent.clone(),
                    prop: prop.clone(),
                    value,
                    terminator,
                };
                // `inline_indent` is the author's indentation with leading newlines
                // stripped, used to align each injected prefix line; `lead` is the
                // newline(s) the author put before the property. We emit:
                //   <lead><inline_indent><prefix1>;\n<inline_indent><prefix2>;\n…<original prop:value…>
                // so every injected line carries the same indentation as the original and
                // no blank line is introduced.
                let inline_indent: String =
                    indent.trim_start_matches(['\n', '\r']).to_string();
                let lead: String = {
                    let stripped_len = indent.trim_start_matches(['\n', '\r']).len();
                    indent[..indent.len() - stripped_len].to_string()
                };
                let emitted_clones = clones_collect(&decl, dedup, &mut seen);
                if !emitted_clones.is_empty() {
                    // leading whitespace (newline) of the original, once.
                    out.push_str(&lead);
                    for cl in &emitted_clones {
                        let term = if cl.terminator.trim_end().ends_with(';') {
                            cl.terminator.trim_end().to_string()
                        } else {
                            format!("{};", cl.terminator.trim_end())
                        };
                        out.push_str(&inline_indent);
                        out.push_str(&cl.prop);
                        out.push_str(": ");
                        out.push_str(&cl.value);
                        out.push_str(&term);
                        out.push('\n');
                    }
                    // original prop:value… without its leading newline (replaced by lead
                    // above) and without re-emitting nothing else — keep inline_indent.
                    out.push_str(&inline_indent);
                    out.push_str(&decl.prop);
                    out.push_str(": ");
                    out.push_str(&decl.value);
                    out.push_str(&decl.terminator);
                } else {
                    // No prefixes needed: emit the original byte-for-byte.
                    out.extend(&bytes[decl_start..after]);
                }
                // Record the original for dedup of later identical decls.
                let orig_key = format!("{}:{}", decl.prop.trim().to_ascii_lowercase(), decl.value.trim());
                if dedup && !seen.contains(&orig_key) {
                    seen.push(orig_key);
                }
                i = after;
                continue;
            }
            // Not a declaration we recognize (selector text in nested at-rules,
            // stray token): copy this char and move on.
            out.push(c);
            i += 1;
            continue;
        }

        // Outside any rule body: copy verbatim (selectors, at-rule preludes).
        out.push(c);
        i += 1;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_select_gets_three_prefixes() {
        let out = prefix("a {\n  user-select: none;\n}\n", true).unwrap();
        assert!(out.contains("-webkit-user-select: none;"), "{out}");
        assert!(out.contains("-moz-user-select: none;"), "{out}");
        assert!(out.contains("-ms-user-select: none;"), "{out}");
        // Standard declaration preserved and last among the user-select group.
        assert!(out.contains("\n  user-select: none;"), "{out}");
        let webkit = out.find("-webkit-user-select").unwrap();
        let std = out.find("\n  user-select: none").unwrap();
        assert!(webkit < std, "prefixed must come before standard");
    }

    #[test]
    fn display_flex_value_prefixes() {
        let out = prefix(".x{display:flex}", true).unwrap();
        assert!(out.contains("display: -webkit-box;"), "{out}");
        assert!(out.contains("display: -webkit-flex;"), "{out}");
        assert!(out.contains("display: -ms-flexbox;"), "{out}");
        assert!(out.contains("display: flex"), "{out}");
    }

    #[test]
    fn position_sticky_value_prefix() {
        let out = prefix("div { position: sticky; }", true).unwrap();
        assert!(out.contains("position: -webkit-sticky;"), "{out}");
        assert!(out.contains("position: sticky;"), "{out}");
    }

    #[test]
    fn tab_size_renames() {
        let out = prefix("pre { tab-size: 4; }", true).unwrap();
        assert!(out.contains("-moz-tab-size: 4;"), "{out}");
        assert!(out.contains("-o-tab-size: 4;"), "{out}");
        assert!(out.contains("\n  tab-size: 4;") || out.contains("tab-size: 4;"), "{out}");
    }

    #[test]
    fn background_clip_text_only() {
        let yes = prefix(".g { background-clip: text; }", true).unwrap();
        assert!(yes.contains("-webkit-background-clip: text;"), "{yes}");
        let no = prefix(".g { background-clip: border-box; }", true).unwrap();
        assert!(!no.contains("-webkit-background-clip"), "{no}");
    }

    #[test]
    fn untouched_property_unchanged() {
        let css = "a { color: red; margin: 0; }";
        assert_eq!(prefix(css, true).unwrap(), css);
    }

    #[test]
    fn comments_and_strings_preserved() {
        let css = "/* user-select: none; */\na { content: \"user-select: x\"; }";
        let out = prefix(css, true).unwrap();
        assert!(!out.contains("-webkit-user-select"), "must not prefix inside comment/string: {out}");
        assert!(out.contains("/* user-select: none; */"), "{out}");
    }

    #[test]
    fn custom_properties_untouched() {
        let css = ":root { --user-select: none; }";
        let out = prefix(css, true).unwrap();
        assert!(!out.contains("-webkit"), "{out}");
        assert_eq!(out, css);
    }

    #[test]
    fn dedup_suppresses_existing_prefix() {
        let css = "a {\n  -webkit-user-select: none;\n  user-select: none;\n}";
        let out = prefix(css, true).unwrap();
        // -webkit- should appear exactly once (the author's), not duplicated.
        let count = out.matches("-webkit-user-select: none;").count();
        assert_eq!(count, 1, "dedup must not duplicate existing -webkit-: {out}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(prefix("   \n  ", true).is_err());
    }

    #[test]
    fn no_double_blank_lines_injected() {
        let out = prefix("a {\n  user-select: none;\n}\n", true).unwrap();
        assert!(!out.contains("\n\n"), "must not introduce blank lines: {out:?}");
        // Each injected line keeps the 2-space indent.
        assert!(out.contains("\n  -webkit-user-select: none;\n  -moz-user-select"), "{out:?}");
    }

    #[test]
    fn max_content_width_prefixes() {
        let out = prefix(".x { width: max-content; }", true).unwrap();
        assert!(out.contains("-webkit-max-content"), "{out}");
        assert!(out.contains("-moz-max-content"), "{out}");
    }
}
