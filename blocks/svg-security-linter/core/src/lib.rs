//! svg-security-linter core — pure compute shared by the chat skill block and the web page.
//!
//! Scans SVG markup as TEXT for the constructs that turn an uploaded or inlined SVG into an
//! XSS vector: `<script>` elements, `on*` event handlers, `javascript:` URLs (including
//! character-reference obfuscation), `<foreignObject>`, embedded HTML/active elements, SMIL
//! animations that retarget `href`/`style`/`on*`, external and protocol-relative references,
//! CSS `@import` / remote `url(...)` / `expression(...)`, `data:` URLs, DOCTYPE/ENTITY
//! declarations and stylesheet processing instructions.
//!
//! Nothing is rendered, parsed by a DOM, fetched or executed — the document is walked by a
//! forgiving, quote-aware tokenizer (same shape as svg-optimize's) that records byte offsets so
//! every finding carries a line and column, which matters because real-world SVGs are often
//! minified onto a single line.
//!
//! This is a REPORTER, not a sanitizer: it never emits a "cleaned" file. A blocklist scrub of
//! untrusted SVG gives false assurance; use an allowlist parser server-side for that.

use serde_json::json;
use std::fmt::Write as _;

/// Largest document accepted, in bytes. Anything larger is rejected rather than truncated.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Hard cap on reported findings; the overflow count is stated in the report, never dropped
/// silently.
pub const MAX_FINDINGS: usize = 500;

/// Every rule code this linter can emit, with its baseline severity, in taxonomy order.
pub const RULE_CODES: [&str; 14] = [
    "SCRIPT",
    "EVENT-HANDLER",
    "JS-URL",
    "FOREIGN-OBJECT",
    "EMBEDDED-HTML",
    "ANIMATE-HREF",
    "HANDLER",
    "DOCTYPE-ENTITY",
    "DATA-URI",
    "EXTERNAL-REF",
    "CSS-IMPORT",
    "XML-STYLESHEET",
    "UNKNOWN-NS",
    "ANCHOR-TARGET",
];

/// Elements that carry active content or embed a second document inside a graphic.
const EMBEDDED_HTML_ELEMENTS: [&str; 19] = [
    "iframe", "object", "embed", "frame", "frameset", "applet", "base", "link", "meta", "html",
    "body", "audio", "video", "source", "track", "form", "input", "button", "marquee",
];

/// SMIL animation elements that can retarget another element's attribute.
const ANIMATION_ELEMENTS: [&str; 5] = [
    "animate",
    "set",
    "animatetransform",
    "animatemotion",
    "animatecolor",
];

/// Attribute names whose value is fetched as a URL.
const URL_ATTRS: [&str; 8] = [
    "href",
    "src",
    "data",
    "poster",
    "action",
    "formaction",
    "xml:base",
    "base",
];

/// Namespace URIs that are expected in a static graphic (SVG itself, linking, XML, and the
/// metadata/editor namespaces every drawing app writes).
const KNOWN_NAMESPACES: [&str; 12] = [
    "http://www.w3.org/2000/svg",
    "http://www.w3.org/1999/xlink",
    "http://www.w3.org/xml/1998/namespace",
    "http://www.inkscape.org/namespaces/inkscape",
    "http://sodipodi.sourceforge.net/dtd/sodipodi-0.0.dtd",
    "http://purl.org/dc/elements/1.1/",
    "http://creativecommons.org/ns#",
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
    "http://www.serif.com",
    "http://www.bohemiancoding.com/sketch/ns",
    "http://ns.adobe.com/adobeillustrator/10.0/",
    "http://svgjs.dev/svgjs",
];

/// URL schemes that execute code when the reference is activated.
const SCRIPT_SCHEMES: [&str; 4] = ["javascript:", "vbscript:", "livescript:", "mocha:"];

/// `data:` media types that can carry markup and therefore script.
const SCRIPTABLE_DATA_TYPES: [&str; 6] = [
    "text/html",
    "image/svg+xml",
    "application/xhtml+xml",
    "text/xml",
    "application/xml",
    "application/xhtml",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
        }
    }

    fn parse_min(s: &str) -> Result<Severity, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" | "low" => Severity::Low,
            "medium" => Severity::Medium,
            "high" => Severity::High,
            other => {
                return Err(format!(
                    "unknown min_severity '{other}' (use all, medium, or high)"
                ))
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Csv,
}

impl Format {
    fn parse(s: &str) -> Result<Format, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "report" => Format::Text,
            "json" => Format::Json,
            "csv" => Format::Csv,
            other => return Err(format!("unknown format '{other}' (use text, json, or csv)")),
        })
    }
}

#[derive(Debug, Clone)]
struct Finding {
    offset: usize,
    line: usize,
    column: usize,
    code: &'static str,
    severity: Severity,
    element: String,
    attribute: Option<String>,
    message: String,
    snippet: String,
}

/// Lint SVG markup for XSS-risk constructs.
///
/// - `svg`: the markup (max [`MAX_INPUT_BYTES`] bytes). Never rendered or fetched.
/// - `min_severity`: all/low | medium | high — a DISPLAY filter only; the verdict is computed
///   before it is applied.
/// - `allow_external`: treat http(s)/protocol-relative references as an accepted policy and
///   drop `EXTERNAL-REF` findings.
/// - `ignore`: comma- or space-separated rule codes to suppress.
/// - `format`: text | json | csv.
pub fn lint(
    svg: &str,
    min_severity: &str,
    allow_external: bool,
    ignore: &str,
    format: &str,
) -> Result<String, String> {
    if svg.trim().is_empty() {
        return Err("svg input is empty; paste the contents of an .svg file".into());
    }
    if svg.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "svg is too large ({} bytes); the limit is {MAX_INPUT_BYTES} bytes",
            svg.len()
        ));
    }
    let min = Severity::parse_min(min_severity)?;
    let format = Format::parse(format)?;
    let ignored = parse_ignore(ignore)?;
    if find_ci(svg, "<svg", 0).is_none() {
        return Err(
            "input does not look like SVG markup: no '<svg' element found (paste the full file, \
             including the <svg ...> root element)"
                .into(),
        );
    }

    let starts = line_starts(svg);
    let mut findings = Vec::new();
    scan(svg, &starts, &mut findings);

    // Policy filters. These change the VERDICT (unlike min_severity, which only hides rows).
    if allow_external {
        findings.retain(|f| f.code != "EXTERNAL-REF");
    }
    findings.retain(|f| !ignored.iter().any(|c| c == f.code));
    findings.sort_by(|a, b| a.offset.cmp(&b.offset).then(a.code.cmp(b.code)));
    findings.dedup_by(|a, b| a.offset == b.offset && a.code == b.code);

    let verdict = if findings.iter().any(|f| f.severity == Severity::High) {
        "unsafe"
    } else if findings.is_empty() {
        "clean"
    } else {
        "review"
    };

    let total_before_filter = findings.len();
    findings.retain(|f| f.severity >= min);
    let hidden = total_before_filter - findings.len();
    let truncated = findings.len().saturating_sub(MAX_FINDINGS);
    findings.truncate(MAX_FINDINGS);

    Ok(match format {
        Format::Text => render_text(verdict, &findings, hidden, truncated),
        Format::Json => render_json(verdict, &findings, hidden, truncated),
        Format::Csv => render_csv(&findings),
    })
}

fn parse_ignore(list: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in list.split([',', ' ', '\t', '\n', ';']) {
        let code = raw.trim().to_ascii_uppercase();
        if code.is_empty() {
            continue;
        }
        if !RULE_CODES.contains(&code.as_str()) {
            return Err(format!(
                "unknown rule code '{code}' in ignore (valid codes: {})",
                RULE_CODES.join(", ")
            ));
        }
        out.push(code);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------------------------

struct Attr {
    name: String,
    value: String,
    /// Byte offset of the attribute name in the source.
    offset: usize,
}

/// Walk the document, emitting findings. Forgiving by design: malformed markup is scanned as
/// far as it parses rather than rejected, because a hostile file is rarely well-formed.
fn scan(svg: &str, starts: &[usize], out: &mut Vec<Finding>) {
    let bytes = svg.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &svg[i..];
        if rest.starts_with("<!--") {
            i = find_from(svg, "-->", i + 4).map_or(bytes.len(), |p| p + 3);
        } else if rest.starts_with("<![CDATA[") {
            i = find_from(svg, "]]>", i + 9).map_or(bytes.len(), |p| p + 3);
        } else if rest.starts_with("<!") {
            let end = declaration_end(svg, i);
            check_declaration(&svg[i..end], i, svg, starts, out);
            i = end;
        } else if rest.starts_with("<?") {
            let end = find_from(svg, "?>", i + 2).map_or(bytes.len(), |p| p + 2);
            check_processing_instruction(&svg[i..end], i, svg, starts, out);
            i = end;
        } else if rest.starts_with("</") {
            i = find_from(svg, ">", i).map_or(bytes.len(), |p| p + 1);
        } else {
            let (name, attrs, self_closing, end) = parse_tag(svg, i);
            if name.is_empty() {
                i += 1;
                continue;
            }
            let local = local_name(&name);
            check_element(&local, &attrs, i, svg, starts, out);
            i = end;
            // Raw-text elements: their content is CSS/JS, not markup.
            if !self_closing && (local == "style" || local == "script") {
                let close = find_ci(svg, &format!("</{name}"), i).unwrap_or(bytes.len());
                let text = &svg[i.min(close)..close];
                if local == "style" {
                    scan_css(text, i, "style", None, svg, starts, out);
                }
                i = close;
            }
        }
    }
}

/// End offset (exclusive) of a `<! ... >` declaration, honouring an internal `[ ... ]` subset.
fn declaration_end(svg: &str, start: usize) -> usize {
    let bytes = svg.as_bytes();
    let mut i = start + 2;
    let mut in_subset = false;
    let mut quote = 0u8;
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'[' {
            in_subset = true;
        } else if c == b']' {
            in_subset = false;
        } else if c == b'>' && !in_subset {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

/// Parse `<name attr="v" ...>` starting at `start`. Returns the raw tag name, its attributes,
/// whether it self-closes, and the offset just past `>`.
fn parse_tag(svg: &str, start: usize) -> (String, Vec<Attr>, bool, usize) {
    let bytes = svg.as_bytes();
    let mut i = start + 1;
    let name_start = i;
    while i < bytes.len() && !is_space(bytes[i]) && bytes[i] != b'>' && bytes[i] != b'/' {
        i += 1;
    }
    let name = svg[name_start..i].to_string();
    let mut attrs = Vec::new();
    let mut self_closing = false;
    loop {
        while i < bytes.len() && is_space(bytes[i]) {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] == b'>' {
            i += 1;
            break;
        }
        if bytes[i] == b'/' {
            self_closing = true;
            i += 1;
            continue;
        }
        let attr_start = i;
        while i < bytes.len()
            && !is_space(bytes[i])
            && bytes[i] != b'='
            && bytes[i] != b'>'
            && bytes[i] != b'/'
        {
            i += 1;
        }
        if i == attr_start {
            i += 1;
            continue;
        }
        let attr_name = svg[attr_start..i].to_string();
        while i < bytes.len() && is_space(bytes[i]) {
            i += 1;
        }
        let mut value = String::new();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && is_space(bytes[i]) {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let vs = i;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                value = svg[vs..i.min(bytes.len())].to_string();
                if i < bytes.len() {
                    i += 1;
                }
            } else {
                let vs = i;
                while i < bytes.len() && !is_space(bytes[i]) && bytes[i] != b'>' {
                    i += 1;
                }
                value = svg[vs..i].to_string();
            }
        }
        attrs.push(Attr {
            name: attr_name,
            value,
            offset: attr_start,
        });
    }
    (name, attrs, self_closing, i.max(start + 1))
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0x0c)
}

/// Lowercased name with any namespace prefix removed (`svg:script` → `script`).
fn local_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    match lower.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => lower,
    }
}

// ---------------------------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------------------------

fn check_declaration(
    decl: &str,
    offset: usize,
    svg: &str,
    starts: &[usize],
    out: &mut Vec<Finding>,
) {
    let lower = decl.to_ascii_lowercase();
    if !lower.starts_with("<!doctype") && !lower.starts_with("<!entity") {
        return;
    }
    let entities = lower.matches("<!entity").count();
    let external = lower.contains("system") || lower.contains("public");
    let severity = if external {
        Severity::High
    } else {
        Severity::Medium
    };
    let message = if external {
        "DOCTYPE declares an external entity (SYSTEM/PUBLIC); an XML parser with entity \
         resolution enabled can be made to read local files or reach the network (XXE)"
            .to_string()
    } else if entities > 0 {
        format!(
            "DOCTYPE defines {entities} internal entit{} — nested entity definitions are the \
             billion-laughs expansion vector and are not needed by a static graphic",
            if entities == 1 { "y" } else { "ies" }
        )
    } else {
        "DOCTYPE declaration present; a static SVG does not need one and DTD processing is an \
         attack surface in XML parsers"
            .to_string()
    };
    push(
        out, svg, starts, offset, "DOCTYPE-ENTITY", severity, "!doctype", None, message,
    );
}

fn check_processing_instruction(
    pi: &str,
    offset: usize,
    svg: &str,
    starts: &[usize],
    out: &mut Vec<Finding>,
) {
    let lower = pi.to_ascii_lowercase();
    if !lower.starts_with("<?xml-stylesheet") {
        return;
    }
    push(
        out,
        svg,
        starts,
        offset,
        "XML-STYLESHEET",
        Severity::Medium,
        "?xml-stylesheet",
        None,
        "an <?xml-stylesheet?> processing instruction pulls in a stylesheet before any \
         element-level filtering applies; remote or attacker-controlled CSS can load fonts, \
         images and (in older engines) behaviours"
            .to_string(),
    );
}

fn check_element(
    local: &str,
    attrs: &[Attr],
    offset: usize,
    svg: &str,
    starts: &[usize],
    out: &mut Vec<Finding>,
) {
    match local {
        "script" => {
            let remote = attrs.iter().any(|a| {
                let n = local_name(&a.name);
                (n == "href" || n == "src") && !a.value.trim().is_empty()
            });
            let message = if remote {
                "<script> element with an href/src loads and runs external JavaScript when the \
                 SVG is opened directly or inlined into a page"
                    .to_string()
            } else {
                "<script> element runs JavaScript when the SVG is opened directly or inlined \
                 into a page (it stays inert inside an <img> tag, but not in an <object>, \
                 <iframe> or inline <svg>)"
                    .to_string()
            };
            push(
                out,
                svg,
                starts,
                offset,
                "SCRIPT",
                Severity::High,
                "script",
                None,
                message,
            );
        }
        "foreignobject" => push(
            out,
            svg,
            starts,
            offset,
            "FOREIGN-OBJECT",
            Severity::High,
            "foreignObject",
            None,
            "<foreignObject> embeds arbitrary HTML (and therefore scriptable content) inside the \
             graphic; sanitizers that only strip <script> miss it"
                .to_string(),
        ),
        "handler" => push(
            out,
            svg,
            starts,
            offset,
            "HANDLER",
            Severity::High,
            "handler",
            None,
            "<handler> is the SVG 1.2 event-handler element and executes its body like a script"
                .to_string(),
        ),
        _ => {
            if EMBEDDED_HTML_ELEMENTS.contains(&local) {
                push(
                    out,
                    svg,
                    starts,
                    offset,
                    "EMBEDDED-HTML",
                    Severity::High,
                    local,
                    None,
                    format!(
                        "<{local}> is an HTML/active-content element, not a drawing primitive; \
                         inside an SVG it can load another document or change how relative \
                         references resolve"
                    ),
                );
            }
        }
    }

    if ANIMATION_ELEMENTS.contains(&local) {
        if let Some(target) = attrs
            .iter()
            .find(|a| local_name(&a.name) == "attributename")
        {
            let t = local_name(&target.value);
            if t == "href" || t == "style" || t.starts_with("on") {
                push(
                    out,
                    svg,
                    starts,
                    target.offset,
                    "ANIMATE-HREF",
                    Severity::High,
                    local,
                    Some(target.name.clone()),
                    format!(
                        "SMIL <{local}> retargets '{}', so the value can be swapped for a \
                         javascript: URL or an event handler after the document loads — this \
                         survives filters that only look at static attributes",
                        target.value
                    ),
                );
            }
        }
    }

    let mut has_target_blank = false;
    let mut rel_value = String::new();
    for attr in attrs {
        check_attribute(local, attr, svg, starts, out);
        let n = local_name(&attr.name);
        if n == "target" && attr.value.trim().eq_ignore_ascii_case("_blank") {
            has_target_blank = true;
        }
        if n == "rel" {
            rel_value = attr.value.to_ascii_lowercase();
        }
    }
    if local == "a" && has_target_blank && !rel_value.split_whitespace().any(|r| r == "noopener") {
        push(
            out,
            svg,
            starts,
            offset,
            "ANCHOR-TARGET",
            Severity::Low,
            "a",
            Some("target".into()),
            "<a target=\"_blank\"> without rel=\"noopener\" hands the opened page a window.opener \
             reference it can use to redirect the original tab"
                .to_string(),
        );
    }
}

fn check_attribute(
    element: &str,
    attr: &Attr,
    svg: &str,
    starts: &[usize],
    out: &mut Vec<Finding>,
) {
    let raw_lower = attr.name.to_ascii_lowercase();
    let local = local_name(&attr.name);
    let decoded = decode_entities(&attr.value);
    let obfuscated = decoded != attr.value;

    // Event handlers: on* is never a legitimate presentation attribute.
    if local.len() > 2
        && local.starts_with("on")
        && local[2..].chars().all(|c| c.is_ascii_alphabetic())
    {
        push(
            out,
            svg,
            starts,
            attr.offset,
            "EVENT-HANDLER",
            Severity::High,
            element,
            Some(attr.name.clone()),
            format!(
                "'{}' is an inline event handler: its JavaScript runs when the event fires, with \
                 no <script> element anywhere in the file",
                attr.name
            ),
        );
        return;
    }
    if raw_lower.starts_with("ev:") || (local == "event" && element == "handler") {
        push(
            out,
            svg,
            starts,
            attr.offset,
            "HANDLER",
            Severity::High,
            element,
            Some(attr.name.clone()),
            format!("'{}' wires an XML-events listener to scripted content", attr.name),
        );
        return;
    }

    // Namespace declarations are not fetched, but an unexpected one signals foreign content.
    if raw_lower == "xmlns" || raw_lower.starts_with("xmlns:") {
        let uri = decoded.trim().to_ascii_lowercase();
        if !uri.is_empty() && !KNOWN_NAMESPACES.contains(&uri.as_str()) {
            push(
                out,
                svg,
                starts,
                attr.offset,
                "UNKNOWN-NS",
                Severity::Low,
                element,
                Some(attr.name.clone()),
                format!(
                    "declares the unexpected namespace '{}'; elements in a foreign namespace are \
                     interpreted by whatever handles that namespace, not by the SVG renderer",
                    decoded.trim()
                ),
            );
        }
        return;
    }

    // style="" is CSS, scanned as CSS (so url(), @import and expression() are all covered).
    if local == "style" {
        scan_css(
            &decoded,
            attr.offset,
            element,
            Some(&attr.name),
            svg,
            starts,
            out,
        );
        return;
    }

    let scheme_view = collapse_scheme(&decoded);

    if let Some(scheme) = SCRIPT_SCHEMES.iter().find(|s| scheme_view.starts_with(*s)) {
        let extra = if obfuscated {
            " (obfuscated with XML character references)"
        } else {
            ""
        };
        push(
            out,
            svg,
            starts,
            attr.offset,
            "JS-URL",
            Severity::High,
            element,
            Some(attr.name.clone()),
            format!(
                "'{}' points at a {scheme} URL{extra}; activating the reference executes the code",
                attr.name
            ),
        );
        return;
    }

    if scheme_view.starts_with("data:") {
        report_data_uri(&decoded, element, Some(&attr.name), attr.offset, svg, starts, out);
        return;
    }

    if URL_ATTRS.contains(&local.as_str()) || URL_ATTRS.contains(&raw_lower.as_str()) {
        if let Some(kind) = remote_kind(&scheme_view) {
            push(
                out,
                svg,
                starts,
                attr.offset,
                "EXTERNAL-REF",
                Severity::Medium,
                element,
                Some(attr.name.clone()),
                format!(
                    "'{}' references {kind} ({}); rendering the file makes that request, which \
                     leaks the viewer's IP and user agent and lets the remote host swap the \
                     content later",
                    attr.name,
                    truncate(decoded.trim(), 100)
                ),
            );
            return;
        }
    }

    // Presentation attributes such as fill/filter/mask can carry url(...) too.
    if decoded.to_ascii_lowercase().contains("url(") {
        scan_css(
            &decoded,
            attr.offset,
            element,
            Some(&attr.name),
            svg,
            starts,
            out,
        );
    }
}

/// Scan a CSS fragment (a `<style>` body or a `style`/presentation attribute value).
fn scan_css(
    css: &str,
    base: usize,
    element: &str,
    attribute: Option<&str>,
    svg: &str,
    starts: &[usize],
    out: &mut Vec<Finding>,
) {
    let lower = css.to_ascii_lowercase();
    let at = |idx: usize| base + idx.min(css.len());

    if let Some(idx) = lower.find("@import") {
        push(
            out,
            svg,
            starts,
            at(idx),
            "CSS-IMPORT",
            Severity::Medium,
            element,
            attribute.map(str::to_string),
            "CSS '@import' pulls in another stylesheet when the graphic renders; the imported \
             sheet is fetched from wherever the URL points and is not covered by markup filtering"
                .to_string(),
        );
    }
    if let Some(idx) = lower.find("expression(") {
        push(
            out,
            svg,
            starts,
            at(idx),
            "JS-URL",
            Severity::High,
            element,
            attribute.map(str::to_string),
            "CSS 'expression(...)' evaluates JavaScript in legacy engines".to_string(),
        );
    }
    for scheme in SCRIPT_SCHEMES {
        if let Some(idx) = lower.find(scheme) {
            push(
                out,
                svg,
                starts,
                at(idx),
                "JS-URL",
                Severity::High,
                element,
                attribute.map(str::to_string),
                format!("CSS contains a {scheme} URL, which executes when the rule is applied"),
            );
            break;
        }
    }

    // url(...) targets.
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("url(") {
        let open = from + rel + 4;
        let close = lower[open..].find(')').map(|p| open + p).unwrap_or(css.len());
        let target = css[open.min(css.len())..close]
            .trim()
            .trim_matches(['"', '\''])
            .trim()
            .to_string();
        from = close.max(open) + 1;
        let view = collapse_scheme(&decode_entities(&target));
        if view.starts_with("data:") {
            report_data_uri(&target, element, attribute, at(open), svg, starts, out);
        } else if let Some(kind) = remote_kind(&view) {
            push(
                out,
                svg,
                starts,
                at(open),
                "EXTERNAL-REF",
                Severity::Medium,
                element,
                attribute.map(str::to_string),
                format!(
                    "CSS url() references {kind} ({}); the request fires when the graphic renders",
                    truncate(&target, 100)
                ),
            );
        }
        if from >= css.len() {
            break;
        }
    }
}

/// Classify and report a `data:` URL. Markup-bearing media types can carry script.
#[allow(clippy::too_many_arguments)]
fn report_data_uri(
    value: &str,
    element: &str,
    attribute: Option<&str>,
    offset: usize,
    svg: &str,
    starts: &[usize],
    out: &mut Vec<Finding>,
) {
    let view = collapse_scheme(value);
    let after = &view[5.min(view.len())..];
    let mime = after
        .split([';', ','])
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let scriptable =
        SCRIPTABLE_DATA_TYPES.contains(&mime.as_str()) || mime.contains("javascript");
    let (severity, message) = if scriptable {
        (
            Severity::High,
            format!(
                "'data:{mime}' embeds a whole markup document inline; it can contain its own \
                 <script> and is a common way to smuggle script past a filter that only looks at \
                 the outer file"
            ),
        )
    } else if mime.starts_with("image/") || mime.is_empty() {
        (
            Severity::Low,
            format!(
                "embedded 'data:{}' payload; common in exported graphics, but it also hides its \
                 contents from a quick visual review",
                if mime.is_empty() { "…" } else { mime.as_str() }
            ),
        )
    } else {
        (
            Severity::Medium,
            format!(
                "embedded 'data:{mime}' payload of a non-image media type; inline base64 hides \
                 what is actually being loaded"
            ),
        )
    };
    push(
        out,
        svg,
        starts,
        offset,
        "DATA-URI",
        severity,
        element,
        attribute.map(str::to_string),
        message,
    );
}

/// `Some(description)` when a URL points off-origin.
fn remote_kind(view: &str) -> Option<&'static str> {
    if view.starts_with("http://") {
        Some("an external host over plain HTTP")
    } else if view.starts_with("https://") {
        Some("an external host")
    } else if view.starts_with("ftp://") {
        Some("an external FTP host")
    } else if view.starts_with("//") {
        Some("a protocol-relative external host")
    } else {
        None
    }
}

/// Lowercase and strip the whitespace/control characters browsers ignore inside a URL scheme,
/// so `java\nscript:` and ` JavaScript :` both normalise to `javascript:`.
fn collapse_scheme(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut seen_colon = false;
    for c in value.chars() {
        if !seen_colon && (c.is_whitespace() || (c as u32) < 0x20) {
            continue;
        }
        if c == ':' {
            seen_colon = true;
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// Decode the XML character references an attacker uses to hide a scheme.
fn decode_entities(value: &str) -> String {
    if !value.contains('&') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let chars: Vec<char> = value.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let end = (i + 1..chars.len().min(i + 12)).find(|&j| chars[j] == ';');
        let Some(end) = end else {
            out.push('&');
            i += 1;
            continue;
        };
        let body: String = chars[i + 1..end].iter().collect();
        let decoded = if let Some(hex) = body
            .strip_prefix("#x")
            .or_else(|| body.strip_prefix("#X"))
        {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else if let Some(dec) = body.strip_prefix('#') {
            dec.parse::<u32>().ok().and_then(char::from_u32)
        } else {
            match body.to_ascii_lowercase().as_str() {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "tab" => Some('\t'),
                "newline" => Some('\n'),
                _ => None,
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                i = end + 1;
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Positions + rendering
// ---------------------------------------------------------------------------------------------

fn line_starts(s: &str) -> Vec<usize> {
    let mut v = vec![0usize];
    for (i, b) in s.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v
}

/// 1-based (line, column) of a byte offset. Column counts characters, not bytes.
fn locate(svg: &str, starts: &[usize], offset: usize) -> (usize, usize) {
    let line = match starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i - 1,
    };
    let start = starts[line];
    let col = svg[start..offset.min(svg.len())].chars().count() + 1;
    (line + 1, col)
}

/// The source line holding `offset`, trimmed and truncated for display.
fn snippet_at(svg: &str, starts: &[usize], offset: usize) -> String {
    let line = match starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i - 1,
    };
    let start = starts[line];
    let end = starts.get(line + 1).copied().unwrap_or(svg.len());
    truncate(svg[start..end].trim_end_matches(['\r', '\n']).trim(), 160)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}…")
}

#[allow(clippy::too_many_arguments)]
fn push(
    out: &mut Vec<Finding>,
    svg: &str,
    starts: &[usize],
    offset: usize,
    code: &'static str,
    severity: Severity,
    element: &str,
    attribute: Option<String>,
    message: String,
) {
    let (line, column) = locate(svg, starts, offset);
    out.push(Finding {
        offset,
        line,
        column,
        code,
        severity,
        element: element.to_string(),
        attribute,
        message,
        snippet: snippet_at(svg, starts, offset),
    });
}

fn counts(findings: &[Finding]) -> (usize, usize, usize) {
    let h = findings
        .iter()
        .filter(|f| f.severity == Severity::High)
        .count();
    let m = findings
        .iter()
        .filter(|f| f.severity == Severity::Medium)
        .count();
    let l = findings
        .iter()
        .filter(|f| f.severity == Severity::Low)
        .count();
    (h, m, l)
}

fn render_text(verdict: &str, findings: &[Finding], hidden: usize, truncated: usize) -> String {
    let (high, medium, low) = counts(findings);
    let mut out = format!(
        "SVG security lint · verdict: {verdict} · {} findings · {high} high · {medium} medium · {low} low",
        findings.len()
    );
    if findings.is_empty() {
        out.push_str(if hidden > 0 {
            "\n\nNothing at or above the selected severity. Lower the threshold to see the rest."
        } else {
            "\n\nNo XSS-risk constructs found."
        });
    }
    for f in findings {
        let target = match &f.attribute {
            Some(a) => format!("<{}> {a}", f.element),
            None => format!("<{}>", f.element),
        };
        let _ = write!(
            out,
            "\n\nL{}:{} [{}] {}: {} — {}",
            f.line,
            f.column,
            f.severity.label(),
            f.code,
            target,
            f.message
        );
        if !f.snippet.is_empty() {
            let _ = write!(out, "\n  {}", f.snippet);
        }
    }
    if hidden > 0 {
        let _ = write!(
            out,
            "\n\n{hidden} finding(s) below the selected severity are hidden; the verdict above \
             still counts them."
        );
    }
    if truncated > 0 {
        let _ = write!(
            out,
            "\n\n{truncated} further finding(s) were not listed (report capped at {MAX_FINDINGS})."
        );
    }
    out
}

fn render_json(verdict: &str, findings: &[Finding], hidden: usize, truncated: usize) -> String {
    let (high, medium, low) = counts(findings);
    let items: Vec<_> = findings
        .iter()
        .map(|f| {
            json!({
                "line": f.line,
                "column": f.column,
                "severity": f.severity.label(),
                "code": f.code,
                "element": f.element,
                "attribute": f.attribute,
                "message": f.message,
                "snippet": f.snippet,
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "verdict": verdict,
        "summary": {
            "findings": findings.len(),
            "high": high,
            "medium": medium,
            "low": low,
            "hidden_by_min_severity": hidden,
            "not_listed": truncated,
        },
        "findings": items,
    }))
    .unwrap()
}

fn render_csv(findings: &[Finding]) -> String {
    let mut out = String::from("line,column,severity,code,element,attribute,message,snippet");
    for f in findings {
        let _ = write!(
            out,
            "\n{},{},{},{},{},{},{},{}",
            f.line,
            f.column,
            f.severity.label(),
            f.code,
            csv_field(&f.element),
            csv_field(f.attribute.as_deref().unwrap_or("")),
            csv_field(&f.message),
            csv_field(&f.snippet)
        );
    }
    out
}

fn csv_field(v: &str) -> String {
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn find_from(hay: &str, needle: &str, from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..].find(needle).map(|p| p + from)
}

/// Case-insensitive search from `from`, returning a byte offset into `hay`.
fn find_ci(hay: &str, needle: &str, from: usize) -> Option<usize> {
    if from >= hay.len() {
        return None;
    }
    hay[from..]
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
        .map(|p| p + from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(report: &str) -> Vec<String> {
        report
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                if !l.starts_with('L') {
                    return None;
                }
                let after = l.split("] ").nth(1)?;
                Some(after.split(':').next()?.to_string())
            })
            .collect()
    }

    const HOSTILE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)">
  <script>alert(document.domain)</script>
  <a href="javascript:alert(2)" target="_blank"><text>go</text></a>
  <image href="https://evil.example/pixel.png"/>
</svg>"#;

    #[test]
    fn flags_script_event_handler_and_js_url() {
        let report = lint(HOSTILE, "all", false, "", "text").unwrap();
        let found = codes(&report);
        for want in ["EVENT-HANDLER", "SCRIPT", "JS-URL", "EXTERNAL-REF"] {
            assert!(found.iter().any(|c| c == want), "missing {want} in {report}");
        }
        assert!(report.starts_with("SVG security lint · verdict: unsafe"), "{report}");
    }

    #[test]
    fn clean_graphic_has_no_findings() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
  <title>Dot</title>
  <circle cx="5" cy="5" r="4" fill="#0a7" stroke="none"/>
</svg>"##;
        let report = lint(svg, "all", false, "", "text").unwrap();
        assert!(report.contains("verdict: clean"), "{report}");
        assert!(report.contains("No XSS-risk constructs found."), "{report}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = lint("   ", "all", false, "", "text").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn non_svg_input_is_an_error() {
        let err = lint("{\"not\": \"svg\"}", "all", false, "", "text").unwrap_err();
        assert!(err.contains("no '<svg' element found"), "{err}");
    }

    #[test]
    fn rejects_input_over_the_cap() {
        let big = format!("<svg>{}</svg>", "a".repeat(MAX_INPUT_BYTES));
        let err = lint(&big, "all", false, "", "text").unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn rejects_unknown_enum_values_and_rule_codes() {
        assert!(lint(HOSTILE, "critical", false, "", "text")
            .unwrap_err()
            .contains("unknown min_severity"));
        assert!(lint(HOSTILE, "all", false, "", "yaml")
            .unwrap_err()
            .contains("unknown format"));
        assert!(lint(HOSTILE, "all", false, "NOPE", "text")
            .unwrap_err()
            .contains("unknown rule code"));
    }

    #[test]
    fn detects_foreign_object_and_embedded_html() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="10" height="10">
        <iframe src="https://evil.example/"></iframe></foreignObject></svg>"#;
        let found = codes(&lint(svg, "all", false, "", "text").unwrap());
        assert!(found.iter().any(|c| c == "FOREIGN-OBJECT"), "{found:?}");
        assert!(found.iter().any(|c| c == "EMBEDDED-HTML"), "{found:?}");
    }

    #[test]
    fn detects_smil_href_retargeting() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><a><set attributeName="href" to="javascript:alert(1)"/></a></svg>"#;
        let found = codes(&lint(svg, "all", false, "", "text").unwrap());
        assert!(found.iter().any(|c| c == "ANIMATE-HREF"), "{found:?}");
    }

    #[test]
    fn decodes_character_reference_obfuscation() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><a href="&#106;avascript&#58;alert(1)">x</a></svg>"#;
        let report = lint(svg, "all", false, "", "text").unwrap();
        assert!(report.contains("JS-URL"), "{report}");
        assert!(report.contains("obfuscated with XML character references"), "{report}");
    }

    #[test]
    fn detects_doctype_entities_and_stylesheet_pi() {
        let svg = r#"<?xml-stylesheet type="text/css" href="https://evil.example/x.css"?>
<!DOCTYPE svg [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<svg xmlns="http://www.w3.org/2000/svg"><text>&xxe;</text></svg>"#;
        let found = codes(&lint(svg, "all", false, "", "text").unwrap());
        assert!(found.iter().any(|c| c == "DOCTYPE-ENTITY"), "{found:?}");
        assert!(found.iter().any(|c| c == "XML-STYLESHEET"), "{found:?}");
    }

    #[test]
    fn detects_css_import_and_remote_url() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>@import url("https://evil.example/a.css");
        .c { fill: url(https://evil.example/p.svg#g); }</style><rect class="c" width="1" height="1"/></svg>"#;
        let found = codes(&lint(svg, "all", false, "", "text").unwrap());
        assert!(found.iter().any(|c| c == "CSS-IMPORT"), "{found:?}");
        assert!(found.iter().any(|c| c == "EXTERNAL-REF"), "{found:?}");
    }

    #[test]
    fn data_uri_severity_depends_on_media_type() {
        let html = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="data:text/html;base64,PHNjcmlwdD4="/></svg>"#;
        assert!(lint(html, "all", false, "", "text")
            .unwrap()
            .contains("verdict: unsafe"));
        let png = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,iVBORw0KGgo="/></svg>"#;
        let report = lint(png, "all", false, "", "text").unwrap();
        assert!(report.contains("[low] DATA-URI"), "{report}");
        assert!(report.contains("verdict: review"), "{report}");
    }

    #[test]
    fn allow_external_drops_external_ref_findings() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="https://cdn.example/a.png"/></svg>"#;
        assert!(lint(svg, "all", false, "", "text")
            .unwrap()
            .contains("EXTERNAL-REF"));
        let allowed = lint(svg, "all", true, "", "text").unwrap();
        assert!(!allowed.contains("EXTERNAL-REF"), "{allowed}");
        assert!(allowed.contains("verdict: clean"), "{allowed}");
    }

    #[test]
    fn min_severity_hides_rows_but_not_the_verdict() {
        let report = lint(HOSTILE, "high", false, "", "text").unwrap();
        assert!(report.contains("verdict: unsafe"), "{report}");
        assert!(!report.contains("EXTERNAL-REF"), "{report}");
        assert!(report.contains("below the selected severity are hidden"), "{report}");
    }

    #[test]
    fn ignore_suppresses_a_rule_and_can_change_the_verdict() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><rect width="1" height="1"/></svg>"#;
        let report = lint(svg, "all", false, "event-handler", "text").unwrap();
        assert!(report.contains("verdict: clean"), "{report}");
    }

    #[test]
    fn unknown_namespace_is_low_and_known_editor_ones_are_not() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:h="http://www.w3.org/1999/xhtml"><rect width="1" height="1"/></svg>"#;
        let report = lint(svg, "all", false, "", "text").unwrap();
        assert_eq!(codes(&report), vec!["UNKNOWN-NS".to_string()], "{report}");
        assert!(report.contains("[low]"), "{report}");
    }

    #[test]
    fn comments_do_not_produce_findings() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><!-- <script>alert(1)</script> onload="x" --><rect width="1" height="1"/></svg>"#;
        let report = lint(svg, "all", false, "", "text").unwrap();
        assert!(report.contains("verdict: clean"), "{report}");
    }

    #[test]
    fn json_format_reports_verdict_summary_and_findings() {
        let out = lint(HOSTILE, "all", false, "", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["verdict"], "unsafe");
        assert_eq!(v["summary"]["high"], 3);
        assert_eq!(v["summary"]["medium"], 1);
        let first = &v["findings"][0];
        assert_eq!(first["code"], "EVENT-HANDLER");
        assert_eq!(first["attribute"], "onload");
        assert_eq!(first["line"], 1);
    }

    #[test]
    fn csv_format_has_a_header_and_quotes_commas() {
        let out = lint(HOSTILE, "high", false, "", "csv").unwrap();
        let mut lines = out.lines();
        assert_eq!(
            lines.next().unwrap(),
            "line,column,severity,code,element,attribute,message,snippet"
        );
        let row = lines.next().unwrap();
        assert!(row.starts_with("1,41,high,EVENT-HANDLER,svg,onload,"), "{row}");
        assert!(row.contains('"'), "message with commas must be quoted: {row}");
    }

    #[test]
    fn minified_single_line_svg_still_reports_columns() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1" onclick="x()"/><circle onmouseover="y()"/></svg>"#;
        let out = lint(svg, "all", false, "", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["high"], 2);
        let c0 = v["findings"][0]["column"].as_u64().unwrap();
        let c1 = v["findings"][1]["column"].as_u64().unwrap();
        assert!(c0 < c1 && c0 > 1, "columns should differ: {c0} {c1}");
    }
}
