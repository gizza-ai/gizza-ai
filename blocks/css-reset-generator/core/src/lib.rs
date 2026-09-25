//! Generate a modern CSS reset / normalize stylesheet from toggleable opinions.
//!
//! Every rule emitted here is authored in this file — the presets are
//! *style-alike* baselines assembled from well-known reset rationales, not
//! copies of any third-party stylesheet.

/// One rendered piece of CSS: a plain rule, or an at-rule wrapping children.
#[derive(Debug, Clone)]
pub enum Node {
    Rule { sel: String, decls: Vec<String> },
    At { cond: String, children: Vec<Node> },
}

fn rule(sel: &str, decls: &[&str]) -> Node {
    Node::Rule {
        sel: sel.to_string(),
        decls: decls.iter().map(|d| d.to_string()).collect(),
    }
}

/// Section id → human title used for the section comment. The ORDER here is the
/// output order, so broad baselines (`zero-out`) come first and narrow opinions
/// layered on top win the cascade.
pub const SECTIONS: &[(&str, &str)] = &[
    ("zero-out", "Classic zero-out of every common element"),
    ("box-sizing", "Border-box sizing everywhere"),
    ("text-size-adjust", "Stop mobile browsers inflating text"),
    ("margin", "Remove default margins from flow content"),
    ("padding", "Remove default padding from lists and grouping elements"),
    ("body-defaults", "Body sizing and line height"),
    ("font-smoothing", "Smoother font rendering on macOS"),
    ("media", "Responsive, block-level media"),
    ("forms", "Form controls inherit typography"),
    ("textarea", "Usable textarea defaults"),
    ("button-reset", "Strip default button chrome"),
    ("lists", "Drop markers only where role=list keeps semantics"),
    ("lists-unstyled", "Unstyle every list"),
    ("links", "Unclassed links inherit colour"),
    ("headings", "Tighter, balanced headings"),
    ("headings-unstyled", "Headings inherit size and weight"),
    ("text-wrap", "Prettier paragraph wrapping"),
    ("overflow-wrap", "Long words break instead of overflowing"),
    ("monospace", "Consistent monospace stack"),
    ("abbr", "Dotted underline for abbreviations"),
    ("sub-sup", "Keep sub and sup out of the line box"),
    ("hr", "Predictable horizontal rule"),
    ("tables", "Collapsed table borders"),
    ("scroll-margin", "Breathing room for anchor targets"),
    ("isolation", "Isolate the app root stacking context"),
    ("interactive", "Pointer cursors on interactive elements"),
    ("focus-visible", "Visible keyboard focus ring"),
    ("reduced-motion", "Respect prefers-reduced-motion"),
    ("smooth-scroll", "Smooth scrolling when motion is welcome"),
];

/// Preset id → the section ids it turns on.
pub const PRESETS: &[(&str, &[&str])] = &[
    (
        "modern",
        &[
            "box-sizing",
            "text-size-adjust",
            "margin",
            "padding",
            "body-defaults",
            "font-smoothing",
            "media",
            "forms",
            "textarea",
            "lists",
            "links",
            "headings",
            "text-wrap",
            "overflow-wrap",
            "scroll-margin",
            "isolation",
            "interactive",
            "reduced-motion",
        ],
    ),
    ("minimal", &["box-sizing", "margin", "media", "forms"]),
    (
        "normalize",
        &[
            "text-size-adjust",
            "body-defaults",
            "forms",
            "monospace",
            "abbr",
            "sub-sup",
            "hr",
            "tables",
        ],
    ),
    ("classic", &["zero-out"]),
    (
        "preflight",
        &[
            "box-sizing",
            "text-size-adjust",
            "margin",
            "padding",
            "media",
            "forms",
            "button-reset",
            "lists-unstyled",
            "headings-unstyled",
            "monospace",
            "abbr",
            "sub-sup",
            "hr",
            "tables",
        ],
    ),
    ("none", &[]),
];

const ZERO_OUT_SELECTORS: &str = "html, body, div, span, applet, object, iframe, h1, h2, h3, h4, h5, h6, p, \
blockquote, pre, a, abbr, acronym, address, big, cite, code, del, dfn, em, img, ins, kbd, q, s, samp, \
small, strike, strong, sub, sup, tt, var, b, u, i, center, dl, dt, dd, ol, ul, li, fieldset, form, \
label, legend, table, caption, tbody, tfoot, thead, tr, th, td, article, aside, canvas, details, \
embed, figure, figcaption, footer, header, hgroup, main, menu, nav, output, ruby, section, summary, \
time, mark, audio, video";

const MONO_STACK: &str =
    "font-family: ui-monospace, SFMono-Regular, \"SF Mono\", Menlo, Consolas, \"Liberation Mono\", monospace";

/// Values the dynamic sections read from the caller's options.
struct Vars {
    line_height: String,
    min_height: String,
}

fn nodes_for(id: &str, v: &Vars) -> Vec<Node> {
    match id {
        "zero-out" => vec![
            rule(
                ZERO_OUT_SELECTORS,
                &[
                    "margin: 0",
                    "padding: 0",
                    "border: 0",
                    "font-size: 100%",
                    "font: inherit",
                    "vertical-align: baseline",
                ],
            ),
            rule(
                "article, aside, details, figcaption, figure, footer, header, hgroup, main, menu, nav, section",
                &["display: block"],
            ),
            rule("body", &["line-height: 1"]),
            rule("ol, ul", &["list-style: none"]),
            rule("blockquote, q", &["quotes: none"]),
            rule(
                "blockquote::before, blockquote::after, q::before, q::after",
                &["content: \"\"", "content: none"],
            ),
            rule("table", &["border-collapse: collapse", "border-spacing: 0"]),
        ],
        "box-sizing" => vec![rule("*, *::before, *::after", &["box-sizing: border-box"])],
        "text-size-adjust" => vec![rule(
            "html",
            &[
                "-webkit-text-size-adjust: 100%",
                "-moz-text-size-adjust: 100%",
                "text-size-adjust: 100%",
            ],
        )],
        "margin" => vec![rule(
            "body, h1, h2, h3, h4, h5, h6, p, figure, figcaption, blockquote, dl, dd, pre",
            &["margin: 0"],
        )],
        "padding" => vec![rule("ul, ol, menu, fieldset, figure, blockquote", &["padding: 0"])],
        "body-defaults" => {
            let mut decls = Vec::new();
            if !v.min_height.is_empty() {
                decls.push(format!("min-height: {}", v.min_height));
            }
            decls.push(format!("line-height: {}", v.line_height));
            vec![Node::Rule {
                sel: "body".into(),
                decls,
            }]
        }
        "font-smoothing" => vec![rule(
            "body",
            &[
                "-webkit-font-smoothing: antialiased",
                "-moz-osx-font-smoothing: grayscale",
            ],
        )],
        "media" => vec![rule(
            "img, picture, video, canvas, svg",
            &["display: block", "max-width: 100%"],
        )],
        "forms" => vec![rule(
            "input, button, textarea, select",
            &["font: inherit", "color: inherit", "letter-spacing: inherit"],
        )],
        "textarea" => vec![
            rule("textarea", &["resize: vertical"]),
            rule("textarea:not([rows])", &["min-height: 10em"]),
        ],
        "button-reset" => vec![rule(
            "button, [type=\"button\"], [type=\"reset\"], [type=\"submit\"]",
            &[
                "padding: 0",
                "border: 0",
                "background-color: transparent",
                "background-image: none",
            ],
        )],
        "lists" => vec![rule(
            "ul[role=\"list\"], ol[role=\"list\"]",
            &["list-style: none", "padding-inline-start: 0"],
        )],
        "lists-unstyled" => vec![rule("ul, ol", &["list-style: none", "padding-inline-start: 0"])],
        "links" => vec![rule(
            "a:not([class])",
            &["color: currentColor", "text-decoration-skip-ink: auto"],
        )],
        "headings" => vec![rule(
            "h1, h2, h3, h4, h5, h6",
            &["line-height: 1.1", "text-wrap: balance"],
        )],
        "headings-unstyled" => vec![rule(
            "h1, h2, h3, h4, h5, h6",
            &["font-size: inherit", "font-weight: inherit"],
        )],
        "text-wrap" => vec![rule("p", &["text-wrap: pretty"])],
        "overflow-wrap" => vec![rule("p, h1, h2, h3, h4, h5, h6", &["overflow-wrap: break-word"])],
        "monospace" => vec![rule("code, kbd, samp, pre", &[MONO_STACK, "font-size: 1em"])],
        "abbr" => vec![rule("abbr[title]", &["text-decoration: underline dotted"])],
        "sub-sup" => vec![
            rule(
                "sub, sup",
                &[
                    "font-size: 75%",
                    "line-height: 0",
                    "position: relative",
                    "vertical-align: baseline",
                ],
            ),
            rule("sub", &["bottom: -0.25em"]),
            rule("sup", &["top: -0.5em"]),
        ],
        "hr" => vec![rule(
            "hr",
            &["height: 0", "color: inherit", "border: 0", "border-top: 1px solid"],
        )],
        "tables" => vec![rule("table", &["border-collapse: collapse", "border-spacing: 0"])],
        "scroll-margin" => vec![rule(":target", &["scroll-margin-block: 5ex"])],
        "isolation" => vec![rule("#root, #__next", &["isolation: isolate"])],
        "interactive" => vec![
            rule(
                "button, [role=\"button\"], label[for], summary",
                &["cursor: pointer"],
            ),
            rule("button:disabled, [aria-disabled=\"true\"]", &["cursor: not-allowed"]),
        ],
        "focus-visible" => vec![rule(
            ":focus-visible",
            &["outline: 2px solid currentColor", "outline-offset: 2px"],
        )],
        "reduced-motion" => vec![Node::At {
            cond: "@media (prefers-reduced-motion: reduce)".into(),
            children: vec![rule(
                "*, *::before, *::after",
                &[
                    "animation-duration: 0.01ms !important",
                    "animation-iteration-count: 1 !important",
                    "transition-duration: 0.01ms !important",
                    "scroll-behavior: auto !important",
                ],
            )],
        }],
        "smooth-scroll" => vec![Node::At {
            cond: "@media (prefers-reduced-motion: no-preference)".into(),
            children: vec![rule("html", &["scroll-behavior: smooth"])],
        }],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

struct Fmt {
    indent: usize,
    minify: bool,
    where_sel: bool,
}

/// Wrap a selector list in `:where(…)` so the whole reset has zero specificity
/// and any later rule beats it. Pseudo-element selectors (`*::before`) are not
/// valid inside `:where()`, so they stay outside the wrapper — their
/// specificity is already 0, so the result is equivalent.
fn transform_sel(sel: &str, where_sel: bool) -> String {
    if !where_sel {
        return sel.to_string();
    }
    let mut wrappable: Vec<&str> = Vec::new();
    let mut plain: Vec<&str> = Vec::new();
    for part in sel.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if part.contains("::") {
            plain.push(part);
        } else {
            wrappable.push(part);
        }
    }
    if wrappable.is_empty() {
        return sel.to_string();
    }
    let mut out = format!(":where({})", wrappable.join(", "));
    for p in plain {
        out.push_str(", ");
        out.push_str(p);
    }
    out
}

/// Break a long comma-separated selector list across lines so a 60-element
/// zero-out rule stays readable.
fn wrap_selectors(sel: &str, pad: &str) -> String {
    const MAX: usize = 78;
    if sel.len() + pad.len() <= MAX || !sel.contains(", ") {
        return format!("{pad}{sel}");
    }
    let mut out = String::new();
    let mut line = String::from(pad);
    let parts: Vec<&str> = sel.split(", ").collect();
    for (i, part) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        let piece = if last {
            part.to_string()
        } else {
            format!("{part},")
        };
        if line.len() > pad.len() && line.len() + 1 + piece.len() > MAX {
            out.push_str(&line);
            out.push('\n');
            line = String::from(pad);
        }
        if line.len() > pad.len() {
            line.push(' ');
        }
        line.push_str(&piece);
    }
    out.push_str(&line);
    out
}

fn render_nodes(nodes: &[Node], depth: usize, f: &Fmt, out: &mut String) {
    for node in nodes {
        match node {
            Node::Rule { sel, decls } => {
                if decls.is_empty() {
                    continue;
                }
                let sel = transform_sel(sel, f.where_sel);
                if f.minify {
                    out.push_str(&sel.replace(", ", ","));
                    out.push('{');
                    out.push_str(&decls.join(";").replace(": ", ":"));
                    out.push('}');
                } else {
                    let pad = " ".repeat(f.indent * depth);
                    out.push_str(&wrap_selectors(&sel, &pad));
                    out.push_str(" {\n");
                    let dpad = " ".repeat(f.indent * (depth + 1));
                    for d in decls {
                        out.push_str(&dpad);
                        out.push_str(d);
                        out.push_str(";\n");
                    }
                    out.push_str(&pad);
                    out.push_str("}\n");
                }
            }
            Node::At { cond, children } => {
                if f.minify {
                    out.push_str(&cond.replace(": ", ":"));
                    out.push('{');
                    render_nodes(children, depth, f, out);
                    out.push('}');
                } else {
                    let pad = " ".repeat(f.indent * depth);
                    out.push_str(&pad);
                    out.push_str(cond);
                    out.push_str(" {\n");
                    render_nodes(children, depth + 1, f, out);
                    out.push_str(&pad);
                    out.push_str("}\n");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// public API
// ---------------------------------------------------------------------------

/// All section ids, in output order.
pub fn section_ids() -> Vec<&'static str> {
    SECTIONS.iter().map(|(id, _)| *id).collect()
}

/// All preset ids.
pub fn preset_ids() -> Vec<&'static str> {
    PRESETS.iter().map(|(id, _)| *id).collect()
}

fn fmt_num(x: f64) -> String {
    if x.fract() == 0.0 && x.is_finite() {
        format!("{}", x as i64)
    } else {
        let s = format!("{x:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn parse_list(raw: &str) -> Vec<String> {
    raw.split(|c: char| c == ',' || c.is_whitespace())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn known_sections() -> String {
    section_ids().join(", ")
}

fn validate_sections(list: &[String], field: &str) -> Result<(), String> {
    for s in list {
        if !SECTIONS.iter().any(|(id, _)| *id == s) {
            return Err(format!(
                "unknown section `{s}` in {field} — expected one of: {}",
                known_sections()
            ));
        }
    }
    Ok(())
}

fn valid_layer(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                && !part.chars().next().unwrap().is_ascii_digit()
        })
}

/// Build a CSS reset.
///
/// * `preset` — base section set (`modern`, `minimal`, `normalize`, `classic`, `preflight`, `none`).
/// * `include` / `exclude` — comma-separated section ids added to / removed from the preset.
/// * `selector_style` — `standard` or `where` (zero-specificity `:where()` wrapping).
/// * `layer` — optional `@layer` name to wrap the output in (empty = no layer).
/// * `line_height` — body line height (1–3), used by the `body-defaults` section.
/// * `body_min_height` — `100svh` / `100dvh` / `100vh` / `none`, used by `body-defaults`.
/// * `comments` — emit a banner + per-section comments.
/// * `minify` — single-line, whitespace-free output (implies no comments).
/// * `indent` — spaces per indent level (0–8) in formatted output.
#[allow(clippy::too_many_arguments)]
pub fn run(
    preset: &str,
    include: &str,
    exclude: &str,
    selector_style: &str,
    layer: &str,
    line_height: f64,
    body_min_height: &str,
    comments: bool,
    minify: bool,
    indent: f64,
) -> Result<String, String> {
    let preset_key = preset.trim().to_ascii_lowercase();
    let Some((_, base)) = PRESETS.iter().find(|(id, _)| *id == preset_key) else {
        return Err(format!(
            "unknown preset `{preset}` — expected one of: {}",
            preset_ids().join(", ")
        ));
    };

    let style = selector_style.trim().to_ascii_lowercase();
    if !matches!(style.as_str(), "standard" | "where") {
        return Err(format!(
            "unknown selector_style `{selector_style}` — expected standard or where"
        ));
    }

    if !(1.0..=3.0).contains(&line_height) {
        return Err(format!(
            "line_height must be between 1 and 3, got {}",
            fmt_num(line_height)
        ));
    }
    if !(0.0..=8.0).contains(&indent) || indent.fract() != 0.0 {
        return Err(format!(
            "indent must be a whole number of spaces between 0 and 8, got {}",
            fmt_num(indent)
        ));
    }

    let mh = body_min_height.trim().to_ascii_lowercase();
    let min_height = match mh.as_str() {
        "" | "100svh" => "100svh",
        "100dvh" => "100dvh",
        "100vh" => "100vh",
        "none" => "",
        other => {
            return Err(format!(
                "unknown body_min_height `{other}` — expected 100svh, 100dvh, 100vh or none"
            ))
        }
    };

    let layer = layer.trim();
    if !layer.is_empty() && !valid_layer(layer) {
        return Err(format!(
            "layer must be a CSS layer name such as base or base.reset (letters, digits, -, _ and . between parts), got `{layer}`"
        ));
    }

    let inc = parse_list(include);
    let exc = parse_list(exclude);
    validate_sections(&inc, "include")?;
    validate_sections(&exc, "exclude")?;

    let chosen: Vec<&'static str> = SECTIONS
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| (base.contains(id) || inc.iter().any(|s| s == id)) && !exc.iter().any(|s| s == id))
        .collect();

    if chosen.is_empty() {
        return Err(format!(
            "no sections selected — preset `{preset_key}` plus include minus exclude left nothing to emit; add sections via include (available: {})",
            known_sections()
        ));
    }

    let vars = Vars {
        line_height: fmt_num(line_height),
        min_height: min_height.to_string(),
    };
    let f = Fmt {
        indent: indent as usize,
        minify,
        where_sel: style == "where",
    };
    let with_comments = comments && !minify;
    let depth = usize::from(!layer.is_empty());

    let mut body = String::new();
    if !layer.is_empty() {
        if minify {
            body.push_str("@layer ");
            body.push_str(layer);
            body.push('{');
        } else {
            body.push_str("@layer ");
            body.push_str(layer);
            body.push_str(" {\n");
        }
    }

    for (i, id) in chosen.iter().enumerate() {
        let title = SECTIONS
            .iter()
            .find(|(sid, _)| sid == id)
            .map(|(_, t)| *t)
            .unwrap_or("");
        if with_comments {
            if i > 0 {
                body.push('\n');
            }
            body.push_str(&" ".repeat(f.indent * depth));
            body.push_str("/* ");
            body.push_str(id);
            body.push_str(" — ");
            body.push_str(title);
            body.push_str(" */\n");
        } else if !minify && i > 0 {
            body.push('\n');
        }
        render_nodes(&nodes_for(id, &vars), depth, &f, &mut body);
    }

    if !layer.is_empty() {
        if minify {
            body.push('}');
        } else {
            body.push_str("}\n");
        }
    }

    let mut out = String::new();
    if with_comments {
        out.push_str("/* CSS reset — preset: ");
        out.push_str(&preset_key);
        out.push_str(&format!(" ({} sections)\n", chosen.len()));
        out.push_str("   sections: ");
        out.push_str(&chosen.join(", "));
        out.push_str(" */\n\n");
    }
    out.push_str(&body);
    if minify {
        Ok(out.trim().to_string())
    } else {
        Ok(out.trim_end().to_string() + "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modern() -> String {
        run("modern", "", "", "standard", "", 1.5, "100svh", true, false, 2.0).unwrap()
    }

    #[test]
    fn modern_preset_has_the_table_stakes() {
        let css = modern();
        assert!(css.contains("*, *::before, *::after {\n  box-sizing: border-box;\n}"), "{css}");
        assert!(css.contains("min-height: 100svh;"), "{css}");
        assert!(css.contains("line-height: 1.5;"), "{css}");
        assert!(css.contains("img, picture, video, canvas, svg {"), "{css}");
        assert!(css.contains("input, button, textarea, select {"), "{css}");
        assert!(css.contains("@media (prefers-reduced-motion: reduce) {"), "{css}");
        assert!(css.starts_with("/* CSS reset — preset: modern (18 sections)"), "{css}");
        assert!(css.ends_with('\n'));
        // opinions the modern preset deliberately leaves out
        assert!(!css.contains("scroll-behavior: smooth"), "{css}");
        assert!(!css.contains("font-size: 100%"), "{css}");
    }

    #[test]
    fn minimal_preset_is_four_sections_only() {
        let css = run("minimal", "", "", "standard", "", 1.5, "100svh", false, false, 2.0).unwrap();
        assert!(css.contains("box-sizing: border-box"));
        assert!(css.contains("max-width: 100%"));
        assert!(!css.contains("/*"), "comments off means no comments: {css}");
        assert!(!css.contains(":target"), "{css}");
    }

    #[test]
    fn classic_preset_zeroes_elements_and_comes_first() {
        let css = run("classic", "box-sizing", "", "standard", "", 1.5, "none", false, false, 2.0)
            .unwrap();
        let zero = css.find("vertical-align: baseline").unwrap();
        let bs = css.find("box-sizing: border-box").unwrap();
        assert!(zero < bs, "zero-out must precede later opinions: {css}");
        assert!(css.contains("border-collapse: collapse"));
        assert!(css.contains("quotes: none"));
    }

    #[test]
    fn include_and_exclude_edit_the_preset() {
        let css = run(
            "minimal",
            "smooth-scroll, focus-visible",
            "media",
            "standard",
            "",
            1.5,
            "100svh",
            false,
            false,
            2.0,
        )
        .unwrap();
        assert!(css.contains("scroll-behavior: smooth"), "{css}");
        assert!(css.contains(":focus-visible"), "{css}");
        assert!(!css.contains("max-width: 100%"), "excluded section still present: {css}");
    }

    #[test]
    fn where_style_drops_specificity_but_keeps_pseudo_elements_outside() {
        let css = run("none", "box-sizing, links", "", "where", "", 1.5, "none", false, false, 2.0)
            .unwrap();
        assert!(css.contains(":where(*), *::before, *::after {"), "{css}");
        assert!(css.contains(":where(a:not([class])) {"), "{css}");
    }

    #[test]
    fn layer_wraps_and_indents_the_whole_sheet() {
        let css = run("minimal", "", "", "standard", "base.reset", 1.5, "none", false, false, 2.0)
            .unwrap();
        assert!(css.starts_with("@layer base.reset {\n"), "{css}");
        assert!(css.contains("\n  *, *::before, *::after {\n    box-sizing: border-box;\n  }"), "{css}");
        assert!(css.trim_end().ends_with('}'));
    }

    #[test]
    fn minify_is_single_line_and_comment_free() {
        let css = run("minimal", "", "", "standard", "", 1.5, "100svh", true, true, 2.0).unwrap();
        assert!(!css.contains('\n'), "{css}");
        assert!(!css.contains("/*"), "{css}");
        assert!(css.starts_with("*,*::before,*::after{box-sizing:border-box}"), "{css}");
    }

    #[test]
    fn line_height_and_min_height_flow_into_body_defaults() {
        let css = run("normalize", "", "", "standard", "", 1.75, "100dvh", false, false, 4.0)
            .unwrap();
        assert!(css.contains("body {\n    min-height: 100dvh;\n    line-height: 1.75;\n}"), "{css}");
    }

    #[test]
    fn preflight_preset_unstyles_headings_and_lists() {
        let css = run("preflight", "", "", "standard", "", 1.5, "none", false, false, 2.0).unwrap();
        assert!(css.contains("font-weight: inherit"), "{css}");
        assert!(css.contains("ul, ol {"), "{css}");
        assert!(css.contains("background-image: none"), "{css}");
    }

    #[test]
    fn every_section_id_renders_something() {
        for id in section_ids() {
            let css = run("none", id, "", "standard", "", 1.5, "100svh", false, false, 2.0)
                .unwrap_or_else(|e| panic!("section {id} failed: {e}"));
            assert!(css.contains('{'), "section {id} rendered no rule: {css}");
        }
    }

    #[test]
    fn unknown_preset_is_an_error_that_lists_the_presets() {
        let err = run("meyer", "", "", "standard", "", 1.5, "100svh", true, false, 2.0).unwrap_err();
        assert!(err.contains("unknown preset `meyer`"), "{err}");
        assert!(err.contains("preflight"), "{err}");
    }

    #[test]
    fn unknown_section_is_an_error_that_lists_the_sections() {
        let err = run("modern", "boxsizing", "", "standard", "", 1.5, "100svh", true, false, 2.0)
            .unwrap_err();
        assert!(err.contains("unknown section `boxsizing` in include"), "{err}");
        assert!(err.contains("box-sizing"), "{err}");
    }

    #[test]
    fn empty_selection_is_an_error() {
        let err = run("none", "", "", "standard", "", 1.5, "100svh", true, false, 2.0).unwrap_err();
        assert!(err.contains("no sections selected"), "{err}");
    }

    #[test]
    fn out_of_range_numbers_are_errors() {
        let err = run("modern", "", "", "standard", "", 9.0, "100svh", true, false, 2.0).unwrap_err();
        assert!(err.contains("line_height must be between 1 and 3, got 9"), "{err}");
        let err = run("modern", "", "", "standard", "", 1.5, "100svh", true, false, 12.0)
            .unwrap_err();
        assert!(err.contains("indent must be a whole number"), "{err}");
    }

    #[test]
    fn bad_layer_name_is_an_error() {
        let err = run("minimal", "", "", "standard", "not a layer!", 1.5, "none", true, false, 2.0)
            .unwrap_err();
        assert!(err.contains("layer must be a CSS layer name"), "{err}");
    }

    #[test]
    fn bad_selector_style_and_min_height_are_errors() {
        let err = run("minimal", "", "", "loose", "", 1.5, "none", true, false, 2.0).unwrap_err();
        assert!(err.contains("unknown selector_style `loose`"), "{err}");
        let err = run("minimal", "", "", "standard", "", 1.5, "50vh", true, false, 2.0).unwrap_err();
        assert!(err.contains("unknown body_min_height `50vh`"), "{err}");
    }

    #[test]
    fn long_selector_lists_wrap() {
        let css = run("classic", "", "", "standard", "", 1.5, "none", false, false, 2.0).unwrap();
        for line in css.lines() {
            assert!(line.len() <= 90, "line too long ({}): {line}", line.len());
        }
    }
}
