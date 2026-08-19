//! html-to-jsx core — pure compute, shared by the chat skill block and the web page.
//!
//! Converts an HTML snippet into React JSX: `class` → `className`, `for` →
//! `htmlFor`, React attribute casing (`tabindex` → `tabIndex`, …), inline
//! `style="..."` strings → `style={{ … }}` objects with camelCased properties,
//! valueless boolean attributes → `{true}`, self-closed void tags, HTML
//! comments → `{/* … */}`, and JSX-safe text.
//!
//! Dependency-free by design (the whole block must build for wasm32): a small
//! forgiving tokenizer, the same shape the sibling `html-formatter` block uses,
//! plus a printer. HTML is not well-formed XML, so the parser tolerates
//! unclosed tags, implicit `</li>`/`</p>`/`</td>` closes, unquoted attribute
//! values, and stray text.

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

/// HTML void elements — they never have a closing tag and self-close in JSX.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements whose text content is CDATA in HTML: captured verbatim and emitted
/// as a JSX template-literal child.
const RAW_TEXT: &[&str] = &["script", "style"];

/// Elements whose whitespace is significant: children render inline, verbatim.
const PRESERVE: &[&str] = &["pre", "textarea"];

/// Block-level elements that implicitly close an open `<p>`.
const CLOSES_P: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "details",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "ul",
];

/// Known HTML element names — used to decide whether a tag name may be
/// lowercased. Anything not here keeps its authored case, so React components
/// (`<MyWidget>`) and camelCase SVG (`<linearGradient>`) survive untouched.
const HTML_ELEMENTS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "area",
    "article",
    "aside",
    "audio",
    "b",
    "base",
    "bdi",
    "bdo",
    "big",
    "blockquote",
    "body",
    "br",
    "button",
    "canvas",
    "caption",
    "center",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "datalist",
    "dd",
    "del",
    "details",
    "dfn",
    "dialog",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "font",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "i",
    "iframe",
    "img",
    "input",
    "ins",
    "kbd",
    "label",
    "legend",
    "li",
    "link",
    "main",
    "map",
    "mark",
    "menu",
    "meta",
    "meter",
    "nav",
    "noscript",
    "object",
    "ol",
    "optgroup",
    "option",
    "output",
    "p",
    "param",
    "picture",
    "pre",
    "progress",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "script",
    "search",
    "section",
    "select",
    "slot",
    "small",
    "source",
    "span",
    "strike",
    "strong",
    "style",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "time",
    "title",
    "tr",
    "track",
    "tt",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
];

/// SVG element names React expects in camelCase, keyed by their lowercase form
/// (so `<clippath>` authored in lowercase HTML still comes out right).
const SVG_TAGS: &[(&str, &str)] = &[
    ("animatemotion", "animateMotion"),
    ("animatetransform", "animateTransform"),
    ("clippath", "clipPath"),
    ("feblend", "feBlend"),
    ("fecolormatrix", "feColorMatrix"),
    ("fecomponenttransfer", "feComponentTransfer"),
    ("fecomposite", "feComposite"),
    ("feconvolvematrix", "feConvolveMatrix"),
    ("fediffuselighting", "feDiffuseLighting"),
    ("fedisplacementmap", "feDisplacementMap"),
    ("fedistantlight", "feDistantLight"),
    ("fedropshadow", "feDropShadow"),
    ("feflood", "feFlood"),
    ("fefunca", "feFuncA"),
    ("fefuncb", "feFuncB"),
    ("fefuncg", "feFuncG"),
    ("fefuncr", "feFuncR"),
    ("fegaussianblur", "feGaussianBlur"),
    ("feimage", "feImage"),
    ("femerge", "feMerge"),
    ("femergenode", "feMergeNode"),
    ("femorphology", "feMorphology"),
    ("feoffset", "feOffset"),
    ("fepointlight", "fePointLight"),
    ("fespecularlighting", "feSpecularLighting"),
    ("fespotlight", "feSpotLight"),
    ("fetile", "feTile"),
    ("feturbulence", "feTurbulence"),
    ("foreignobject", "foreignObject"),
    ("glyphref", "glyphRef"),
    ("lineargradient", "linearGradient"),
    ("radialgradient", "radialGradient"),
    ("textpath", "textPath"),
];

/// HTML attribute (lowercase) → React prop name.
const ATTRS: &[(&str, &str)] = &[
    ("accept-charset", "acceptCharset"),
    ("accesskey", "accessKey"),
    ("allowfullscreen", "allowFullScreen"),
    ("autocapitalize", "autoCapitalize"),
    ("autocomplete", "autoComplete"),
    ("autocorrect", "autoCorrect"),
    ("autofocus", "autoFocus"),
    ("autoplay", "autoPlay"),
    ("cellpadding", "cellPadding"),
    ("cellspacing", "cellSpacing"),
    ("charset", "charSet"),
    ("class", "className"),
    ("classid", "classID"),
    ("colspan", "colSpan"),
    ("contenteditable", "contentEditable"),
    ("contextmenu", "contextMenu"),
    ("controlslist", "controlsList"),
    ("crossorigin", "crossOrigin"),
    ("datetime", "dateTime"),
    ("dirname", "dirName"),
    ("enctype", "encType"),
    ("enterkeyhint", "enterKeyHint"),
    ("for", "htmlFor"),
    ("formaction", "formAction"),
    ("formenctype", "formEncType"),
    ("formmethod", "formMethod"),
    ("formnovalidate", "formNoValidate"),
    ("formtarget", "formTarget"),
    ("frameborder", "frameBorder"),
    ("hreflang", "hrefLang"),
    ("http-equiv", "httpEquiv"),
    ("imagesizes", "imageSizes"),
    ("imagesrcset", "imageSrcSet"),
    ("inputmode", "inputMode"),
    ("itemid", "itemID"),
    ("itemprop", "itemProp"),
    ("itemref", "itemRef"),
    ("itemscope", "itemScope"),
    ("itemtype", "itemType"),
    ("keyparams", "keyParams"),
    ("keytype", "keyType"),
    ("marginheight", "marginHeight"),
    ("marginwidth", "marginWidth"),
    ("maxlength", "maxLength"),
    ("mediagroup", "mediaGroup"),
    ("minlength", "minLength"),
    ("nomodule", "noModule"),
    ("novalidate", "noValidate"),
    ("playsinline", "playsInline"),
    ("radiogroup", "radioGroup"),
    ("readonly", "readOnly"),
    ("referrerpolicy", "referrerPolicy"),
    ("rowspan", "rowSpan"),
    ("spellcheck", "spellCheck"),
    ("srcdoc", "srcDoc"),
    ("srclang", "srcLang"),
    ("srcset", "srcSet"),
    ("tabindex", "tabIndex"),
    ("usemap", "useMap"),
    // SVG presentation/geometry attributes React expects in camelCase. Plain
    // hyphenated SVG attributes not listed here fall through to the generic
    // hyphen→camelCase rule below, which produces the same answer.
    ("viewbox", "viewBox"),
    ("preserveaspectratio", "preserveAspectRatio"),
    ("gradientunits", "gradientUnits"),
    ("gradienttransform", "gradientTransform"),
    ("patternunits", "patternUnits"),
    ("patterncontentunits", "patternContentUnits"),
    ("patterntransform", "patternTransform"),
    ("clippathunits", "clipPathUnits"),
    ("maskunits", "maskUnits"),
    ("maskcontentunits", "maskContentUnits"),
    ("markerwidth", "markerWidth"),
    ("markerheight", "markerHeight"),
    ("markerunits", "markerUnits"),
    ("refx", "refX"),
    ("refy", "refY"),
    ("spreadmethod", "spreadMethod"),
    ("startoffset", "startOffset"),
    ("textlength", "textLength"),
    ("lengthadjust", "lengthAdjust"),
    ("stddeviation", "stdDeviation"),
    ("baseprofile", "baseProfile"),
    ("attributename", "attributeName"),
    ("attributetype", "attributeType"),
    ("repeatcount", "repeatCount"),
    ("repeatdur", "repeatDur"),
    ("keysplines", "keySplines"),
    ("keytimes", "keyTimes"),
    ("calcmode", "calcMode"),
    ("pathlength", "pathLength"),
    ("filterunits", "filterUnits"),
    ("primitiveunits", "primitiveUnits"),
    ("edgemode", "edgeMode"),
    ("xchannelselector", "xChannelSelector"),
    ("ychannelselector", "yChannelSelector"),
];

/// Multi-word DOM events whose React name is not just `on` + capitalised rest.
const EVENTS: &[(&str, &str)] = &[
    ("onanimationend", "onAnimationEnd"),
    ("onanimationiteration", "onAnimationIteration"),
    ("onanimationstart", "onAnimationStart"),
    ("onauxclick", "onAuxClick"),
    ("onbeforeinput", "onBeforeInput"),
    ("oncanplay", "onCanPlay"),
    ("oncanplaythrough", "onCanPlayThrough"),
    ("oncompositionend", "onCompositionEnd"),
    ("oncompositionstart", "onCompositionStart"),
    ("oncompositionupdate", "onCompositionUpdate"),
    ("oncontextmenu", "onContextMenu"),
    ("ondblclick", "onDoubleClick"),
    ("ondragend", "onDragEnd"),
    ("ondragenter", "onDragEnter"),
    ("ondragexit", "onDragExit"),
    ("ondragleave", "onDragLeave"),
    ("ondragover", "onDragOver"),
    ("ondragstart", "onDragStart"),
    ("ondurationchange", "onDurationChange"),
    ("onfocusin", "onFocus"),
    ("onfocusout", "onBlur"),
    ("onkeydown", "onKeyDown"),
    ("onkeypress", "onKeyPress"),
    ("onkeyup", "onKeyUp"),
    ("onloadeddata", "onLoadedData"),
    ("onloadedmetadata", "onLoadedMetadata"),
    ("onloadstart", "onLoadStart"),
    ("onmousedown", "onMouseDown"),
    ("onmouseenter", "onMouseEnter"),
    ("onmouseleave", "onMouseLeave"),
    ("onmousemove", "onMouseMove"),
    ("onmouseout", "onMouseOut"),
    ("onmouseover", "onMouseOver"),
    ("onmouseup", "onMouseUp"),
    ("onpointercancel", "onPointerCancel"),
    ("onpointerdown", "onPointerDown"),
    ("onpointerenter", "onPointerEnter"),
    ("onpointerleave", "onPointerLeave"),
    ("onpointermove", "onPointerMove"),
    ("onpointerout", "onPointerOut"),
    ("onpointerover", "onPointerOver"),
    ("onpointerup", "onPointerUp"),
    ("onratechange", "onRateChange"),
    ("onselectionchange", "onSelectionChange"),
    ("ontimeupdate", "onTimeUpdate"),
    ("ontouchcancel", "onTouchCancel"),
    ("ontouchend", "onTouchEnd"),
    ("ontouchmove", "onTouchMove"),
    ("ontouchstart", "onTouchStart"),
    ("ontransitionend", "onTransitionEnd"),
    ("onvolumechange", "onVolumeChange"),
];

/// React props whose value is a number, not a string, when it looks numeric.
const NUMERIC_PROPS: &[&str] = &[
    "tabIndex",
    "colSpan",
    "rowSpan",
    "maxLength",
    "minLength",
    "size",
    "rows",
    "cols",
    "span",
    "start",
];

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comments {
    Jsx,
    Strip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoolAttrs {
    Explicit,
    Shorthand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueAttrs {
    Default,
    Keep,
}

struct Opts {
    unit: String,
    comments: Comments,
    bools: BoolAttrs,
    values: ValueAttrs,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Node {
    Text(String),
    Comment(String),
    Element(Element),
}

#[derive(Debug, Clone)]
struct Element {
    /// Tag name as it will be printed (case already normalised).
    name: String,
    /// Lowercased tag name, for table lookups.
    lname: String,
    /// `(raw name, value)`; `None` = valueless boolean attribute.
    attrs: Vec<(String, Option<String>)>,
    children: Vec<Node>,
    /// Verbatim CDATA for `script`/`style`.
    raw_text: Option<String>,
    self_closed: bool,
}

/// Index just past a tag's closing `>`, respecting quoted attribute values.
fn scan_tag(b: &[u8], start: usize) -> usize {
    let mut j = start + 1;
    let mut quote = 0u8;
    while j < b.len() {
        let c = b[j];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return j + 1;
        }
        j += 1;
    }
    b.len()
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.')
}

/// Parse the inside of a start tag (everything after `<`, before `>`).
fn parse_start_tag(inner: &str) -> (String, Vec<(String, Option<String>)>, bool) {
    let mut chars: Vec<char> = inner.chars().collect();
    // Drop a trailing `/` (self-closing marker) before attribute scanning.
    let mut self_closed = false;
    while chars.last().is_some_and(|c| c.is_whitespace()) {
        chars.pop();
    }
    if chars.last() == Some(&'/') {
        self_closed = true;
        chars.pop();
    }

    let mut i = 0usize;
    let n = chars.len();
    let mut name = String::new();
    while i < n && is_name_char(chars[i]) {
        name.push(chars[i]);
        i += 1;
    }

    let mut attrs: Vec<(String, Option<String>)> = Vec::new();
    while i < n {
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= n {
            break;
        }
        if !is_name_char(chars[i]) {
            // Stray punctuation (a lone `/`, `"`, …) — skip it.
            i += 1;
            continue;
        }
        let mut aname = String::new();
        while i < n && is_name_char(chars[i]) {
            aname.push(chars[i]);
            i += 1;
        }
        let save = i;
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i < n && chars[i] == '=' {
            i += 1;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            let mut value = String::new();
            if i < n && (chars[i] == '"' || chars[i] == '\'') {
                let q = chars[i];
                i += 1;
                while i < n && chars[i] != q {
                    value.push(chars[i]);
                    i += 1;
                }
                i += 1; // closing quote
            } else {
                while i < n && !chars[i].is_whitespace() {
                    value.push(chars[i]);
                    i += 1;
                }
            }
            attrs.push((aname, Some(value)));
        } else {
            i = save;
            attrs.push((aname, None));
        }
    }
    (name, attrs, self_closed)
}

fn normalize_tag(raw: &str) -> (String, String) {
    let lower = raw.to_ascii_lowercase();
    if HTML_ELEMENTS.contains(&lower.as_str()) {
        return (lower.clone(), lower);
    }
    if let Some((_, camel)) = SVG_TAGS.iter().find(|(k, _)| *k == lower) {
        return ((*camel).to_string(), lower);
    }
    (raw.to_string(), lower)
}

/// True when opening `open` implicitly closes the currently open `top`.
fn implicitly_closes(open: &str, top: &str) -> bool {
    match top {
        "li" => open == "li",
        "option" => open == "option" || open == "optgroup",
        "optgroup" => open == "optgroup",
        "dt" | "dd" => open == "dt" || open == "dd",
        "td" | "th" => matches!(open, "td" | "th" | "tr" | "tbody" | "tfoot" | "thead"),
        "tr" => matches!(open, "tr" | "tbody" | "tfoot" | "thead"),
        "thead" | "tbody" => matches!(open, "tbody" | "tfoot" | "thead"),
        "p" => CLOSES_P.contains(&open),
        _ => false,
    }
}

fn push_node(stack: &mut Vec<Element>, roots: &mut Vec<Node>, node: Node) {
    match stack.last_mut() {
        Some(e) => e.children.push(node),
        None => roots.push(node),
    }
}

fn close_top(stack: &mut Vec<Element>, roots: &mut Vec<Node>) {
    if let Some(e) = stack.pop() {
        push_node(stack, roots, Node::Element(e));
    }
}

fn parse(html: &str) -> Vec<Node> {
    let b = html.as_bytes();
    let n = b.len();
    let lower_doc = html.to_ascii_lowercase();
    let mut i = 0usize;
    let mut roots: Vec<Node> = Vec::new();
    let mut stack: Vec<Element> = Vec::new();

    while i < n {
        if b[i] == b'<' {
            if html[i..].starts_with("<!--") {
                let (body, end) = match html[i + 4..].find("-->") {
                    Some(p) => (&html[i + 4..i + 4 + p], i + 4 + p + 3),
                    None => (&html[i + 4..], n),
                };
                push_node(&mut stack, &mut roots, Node::Comment(body.to_string()));
                i = end;
                continue;
            }
            // Doctype / declaration / processing instruction — no JSX equivalent.
            if i + 1 < n && (b[i + 1] == b'!' || b[i + 1] == b'?') {
                i = scan_tag(b, i);
                continue;
            }
            if i + 1 < n && b[i + 1] == b'/' {
                let end = scan_tag(b, i);
                let raw = &html[i + 2..end.saturating_sub(1).max(i + 2)];
                let name = raw
                    .trim()
                    .chars()
                    .take_while(|c| is_name_char(*c))
                    .collect::<String>()
                    .to_ascii_lowercase();
                if let Some(pos) = stack.iter().rposition(|e| e.lname == name) {
                    while stack.len() > pos {
                        close_top(&mut stack, &mut roots);
                    }
                }
                i = end;
                continue;
            }
            // A start tag — but only if a name follows; otherwise it is text.
            if i + 1 >= n || !html[i + 1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
                let start = i;
                i += 1;
                while i < n && b[i] != b'<' {
                    i += 1;
                }
                push_node(
                    &mut stack,
                    &mut roots,
                    Node::Text(html[start..i].to_string()),
                );
                continue;
            }
            let end = scan_tag(b, i);
            let inner = &html[i + 1..end.saturating_sub(1).max(i + 1)];
            let (raw_name, attrs, self_closed) = parse_start_tag(inner);
            let (name, lname) = normalize_tag(&raw_name);

            while stack
                .last()
                .is_some_and(|top| implicitly_closes(&lname, &top.lname))
            {
                close_top(&mut stack, &mut roots);
            }

            let mut el = Element {
                name,
                lname: lname.clone(),
                attrs,
                children: Vec::new(),
                raw_text: None,
                self_closed,
            };

            if RAW_TEXT.contains(&lname.as_str()) && !self_closed {
                let pat = format!("</{lname}");
                let (text, next) = match lower_doc[end..].find(&pat) {
                    Some(p) => (&html[end..end + p], scan_tag(b, end + p)),
                    None => (&html[end..], n),
                };
                el.raw_text = Some(text.to_string());
                push_node(&mut stack, &mut roots, Node::Element(el));
                i = next;
                continue;
            }

            if self_closed || VOID.contains(&lname.as_str()) {
                push_node(&mut stack, &mut roots, Node::Element(el));
            } else {
                stack.push(el);
            }
            i = end;
            continue;
        }

        let start = i;
        while i < n && b[i] != b'<' {
            i += 1;
        }
        push_node(
            &mut stack,
            &mut roots,
            Node::Text(html[start..i].to_string()),
        );
    }

    while !stack.is_empty() {
        close_top(&mut stack, &mut roots);
    }
    roots
}

// ---------------------------------------------------------------------------
// Attribute / style conversion
// ---------------------------------------------------------------------------

fn camel(name: &str) -> String {
    let mut out = String::new();
    for (idx, part) in name.split(['-', ':']).filter(|p| !p.is_empty()).enumerate() {
        if idx == 0 {
            out.push_str(part);
        } else {
            let mut cs = part.chars();
            if let Some(c) = cs.next() {
                out.extend(c.to_uppercase());
                out.push_str(cs.as_str());
            }
        }
    }
    out
}

/// Map an HTML attribute name to its React prop name.
fn map_attr(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("data-") || lower.starts_with("aria-") {
        return lower;
    }
    if let Some((_, jsx)) = EVENTS.iter().find(|(k, _)| *k == lower) {
        return (*jsx).to_string();
    }
    if let Some(rest) = lower.strip_prefix("on") {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric()) {
            let mut out = String::from("on");
            let mut cs = rest.chars();
            if let Some(c) = cs.next() {
                out.extend(c.to_uppercase());
                out.push_str(cs.as_str());
            }
            return out;
        }
    }
    if let Some((_, jsx)) = ATTRS.iter().find(|(k, _)| *k == lower) {
        return (*jsx).to_string();
    }
    if lower.contains('-') || lower.contains(':') {
        return camel(&lower);
    }
    // Unknown, unhyphenated: keep the authored case so `viewBox`-style names and
    // React component props survive.
    if raw.chars().any(|c| c.is_ascii_uppercase()) {
        raw.to_string()
    } else {
        lower
    }
}

/// Split a `style="…"` declaration list on `;`, ignoring separators inside
/// quotes or parentheses (`url(a;b)`, `content: ";"`).
fn split_decls(style: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut quote = '\0';
    for c in style.chars() {
        if quote != '\0' {
            cur.push(c);
            if c == quote {
                quote = '\0';
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = c;
                cur.push(c);
            }
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ';' if depth <= 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// CSS property → the React style-object key.
fn style_key(prop: &str) -> String {
    if prop.starts_with("--") {
        return format!("\"{prop}\"");
    }
    // `-ms-` is the one vendor prefix React spells with a lowercase leading
    // letter (`msFlex`); every other prefix capitalises (`WebkitBoxShadow`).
    let key = if let Some(rest) = prop.strip_prefix("-ms-") {
        camel(&format!("ms-{rest}"))
    } else if let Some(rest) = prop.strip_prefix('-') {
        let c = camel(rest);
        let mut cs = c.chars();
        match cs.next() {
            Some(f) => f.to_uppercase().chain(cs).collect(),
            None => String::new(),
        }
    } else {
        camel(prop)
    };
    if key.is_empty()
        || !key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    {
        format!("\"{key}\"")
    } else {
        key
    }
}

fn js_string(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `color: red; font-size: 12px` → `{ color: "red", fontSize: "12px" }`.
fn style_object(style: &str) -> Option<String> {
    let mut pairs: Vec<String> = Vec::new();
    for decl in split_decls(style) {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let Some(colon) = decl.find(':') else {
            continue;
        };
        let prop = decl[..colon].trim().to_string();
        let value = decl[colon + 1..].trim();
        if prop.is_empty() || value.is_empty() {
            continue;
        }
        let prop = if prop.starts_with("--") {
            prop
        } else {
            prop.to_ascii_lowercase()
        };
        pairs.push(format!("{}: {}", style_key(&prop), js_string(value)));
    }
    if pairs.is_empty() {
        return None;
    }
    Some(format!("{{{{ {} }}}}", pairs.join(", ")))
}

fn attr_value_escape(v: &str) -> String {
    v.replace('"', "&quot;")
}

/// Render one element's attribute list, including the leading space.
fn render_attrs(el: &Element, o: &Opts) -> String {
    let mut out = String::new();
    for (raw_name, raw_value) in &el.attrs {
        let mut prop = map_attr(raw_name);
        if o.values == ValueAttrs::Default
            && matches!(el.lname.as_str(), "input" | "textarea" | "select")
        {
            match prop.as_str() {
                "value" => prop = "defaultValue".into(),
                "checked" => prop = "defaultChecked".into(),
                _ => {}
            }
        }
        match raw_value {
            None => match o.bools {
                BoolAttrs::Explicit => out.push_str(&format!(" {prop}={{true}}")),
                BoolAttrs::Shorthand => out.push_str(&format!(" {prop}")),
            },
            Some(v) => {
                if prop == "style" {
                    if let Some(obj) = style_object(v) {
                        out.push_str(&format!(" style={obj}"));
                    }
                    continue;
                }
                if prop.starts_with("on") && prop.len() > 2 && !v.trim().is_empty() {
                    // Inline handler source → an arrow function, so React gets a
                    // function instead of a string.
                    out.push_str(&format!(" {prop}={{() => {{ {} }}}}", v.trim()));
                    continue;
                }
                if NUMERIC_PROPS.contains(&prop.as_str()) && v.trim().parse::<i64>().is_ok() {
                    out.push_str(&format!(" {prop}={{{}}}", v.trim()));
                    continue;
                }
                out.push_str(&format!(" {prop}=\"{}\"", attr_value_escape(v)));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Printing
// ---------------------------------------------------------------------------

fn jsx_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '{' => out.push_str("{'{'}"),
            '}' => out.push_str("{'}'}"),
            '<' => out.push_str("{'<'}"),
            _ => out.push(c),
        }
    }
    out
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn jsx_comment(body: &str) -> String {
    // `*/` inside a comment would terminate it early.
    format!("{{/* {} */}}", collapse_ws(body).replace("*/", "* /"))
}

fn template_literal(raw: &str) -> String {
    format!(
        "{{`{}`}}",
        raw.replace('\\', "\\\\")
            .replace('`', "\\`")
            .replace("${", "\\${")
    )
}

/// Children that survive filtering (comments per option, whitespace-only text
/// dropped outside whitespace-preserving elements).
fn kept<'a>(children: &'a [Node], preserve: bool, o: &Opts) -> Vec<&'a Node> {
    children
        .iter()
        .filter(|c| match c {
            Node::Comment(_) => o.comments == Comments::Jsx,
            Node::Text(t) => preserve || !t.trim().is_empty(),
            Node::Element(_) => true,
        })
        .collect()
}

/// Render a node on a single line (used inside `pre`/`textarea`).
fn render_inline(node: &Node, o: &Opts) -> String {
    match node {
        Node::Text(t) => jsx_text(t),
        Node::Comment(c) => jsx_comment(c),
        Node::Element(el) => {
            let attrs = render_attrs(el, o);
            if let Some(raw) = &el.raw_text {
                return format!("<{0}{1}>{2}</{0}>", el.name, attrs, template_literal(raw));
            }
            let kids = kept(&el.children, true, o);
            if kids.is_empty() {
                if el.self_closed || VOID.contains(&el.lname.as_str()) {
                    return format!("<{}{} />", el.name, attrs);
                }
                return format!("<{0}{1}></{0}>", el.name, attrs);
            }
            let inner: String = kids.iter().map(|c| render_inline(c, o)).collect();
            format!("<{0}{1}>{2}</{0}>", el.name, attrs, inner)
        }
    }
}

fn indent(out: &mut String, depth: usize, unit: &str) {
    for _ in 0..depth {
        out.push_str(unit);
    }
}

fn render_node(node: &Node, depth: usize, o: &Opts, out: &mut String) {
    match node {
        Node::Text(t) => {
            let text = collapse_ws(t);
            if text.is_empty() {
                return;
            }
            indent(out, depth, &o.unit);
            out.push_str(&jsx_text(&text));
            out.push('\n');
        }
        Node::Comment(c) => {
            indent(out, depth, &o.unit);
            out.push_str(&jsx_comment(c));
            out.push('\n');
        }
        Node::Element(el) => render_element(el, depth, o, out),
    }
}

fn render_element(el: &Element, depth: usize, o: &Opts, out: &mut String) {
    let attrs = render_attrs(el, o);
    let preserve = PRESERVE.contains(&el.lname.as_str());

    if let Some(raw) = &el.raw_text {
        indent(out, depth, &o.unit);
        if raw.trim().is_empty() {
            out.push_str(&format!("<{0}{1}></{0}>\n", el.name, attrs));
        } else {
            out.push_str(&format!(
                "<{0}{1}>{2}</{0}>\n",
                el.name,
                attrs,
                template_literal(raw)
            ));
        }
        return;
    }

    let kids = kept(&el.children, preserve, o);

    if kids.is_empty() {
        indent(out, depth, &o.unit);
        if el.self_closed || VOID.contains(&el.lname.as_str()) {
            out.push_str(&format!("<{}{} />\n", el.name, attrs));
        } else {
            out.push_str(&format!("<{0}{1}></{0}>\n", el.name, attrs));
        }
        return;
    }

    if preserve {
        let inner: String = kids.iter().map(|c| render_inline(c, o)).collect();
        indent(out, depth, &o.unit);
        out.push_str(&format!("<{0}{1}>{2}</{0}>\n", el.name, attrs, inner));
        return;
    }

    if kids.len() == 1 {
        if let Node::Text(t) = kids[0] {
            let text = collapse_ws(t);
            indent(out, depth, &o.unit);
            out.push_str(&format!(
                "<{0}{1}>{2}</{0}>\n",
                el.name,
                attrs,
                jsx_text(&text)
            ));
            return;
        }
    }

    indent(out, depth, &o.unit);
    out.push_str(&format!("<{}{}>\n", el.name, attrs));
    for c in kids {
        render_node(c, depth + 1, o, out);
    }
    indent(out, depth, &o.unit);
    out.push_str(&format!("</{}>\n", el.name));
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn is_identifier(s: &str) -> bool {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn pick<'a>(
    name: &str,
    value: &'a str,
    allowed: &[&str],
    default: &'a str,
) -> Result<String, String> {
    let v = value.trim();
    let v = if v.is_empty() { default } else { v };
    if allowed.contains(&v) {
        Ok(v.to_string())
    } else {
        Err(format!(
            "unknown {name} '{v}' — expected one of: {}",
            allowed.join(", ")
        ))
    }
}

/// Convert an HTML snippet to JSX.
///
/// * `indent` — `"2"`, `"4"` or `"tab"` (default `"2"`).
/// * `component` — empty for a bare JSX fragment, otherwise an identifier: the
///   result is wrapped in `export default function <name>() { return (…); }`.
/// * `comments` — `"jsx"` keeps HTML comments as `{/* … */}`, `"strip"` drops them.
/// * `boolean_attrs` — `"explicit"` renders valueless attributes as `{true}`,
///   `"shorthand"` leaves them bare.
/// * `value_attrs` — `"default"` rewrites `value`/`checked` on form controls to
///   `defaultValue`/`defaultChecked`; `"keep"` leaves them alone.
pub fn html_to_jsx(
    html: &str,
    indent_opt: &str,
    component: &str,
    comments: &str,
    boolean_attrs: &str,
    value_attrs: &str,
) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("no HTML input — paste an HTML snippet to convert".into());
    }

    let unit = match pick("indent", indent_opt, &["2", "4", "tab"], "2")?.as_str() {
        "4" => "    ".to_string(),
        "tab" => "\t".to_string(),
        _ => "  ".to_string(),
    };
    let comments = match pick("comments", comments, &["jsx", "strip"], "jsx")?.as_str() {
        "strip" => Comments::Strip,
        _ => Comments::Jsx,
    };
    let bools = match pick(
        "boolean_attrs",
        boolean_attrs,
        &["explicit", "shorthand"],
        "explicit",
    )?
    .as_str()
    {
        "shorthand" => BoolAttrs::Shorthand,
        _ => BoolAttrs::Explicit,
    };
    let values = match pick("value_attrs", value_attrs, &["default", "keep"], "default")?.as_str() {
        "keep" => ValueAttrs::Keep,
        _ => ValueAttrs::Default,
    };

    let component = component.trim();
    if !component.is_empty() && !is_identifier(component) {
        return Err(format!(
            "invalid component name '{component}' — use letters, digits, _ or $ and do not start with a digit"
        ));
    }

    let o = Opts {
        unit,
        comments,
        bools,
        values,
    };

    let roots = parse(html);
    let kept_roots = kept(&roots, false, &o);
    if kept_roots.is_empty() {
        return Err(
            "no convertible HTML found — the input has no elements, text or comments".into(),
        );
    }

    let base = if component.is_empty() { 0 } else { 2 };
    let mut body = String::new();
    if kept_roots.len() == 1 {
        render_node(kept_roots[0], base, &o, &mut body);
    } else {
        indent(&mut body, base, &o.unit);
        body.push_str("<>\n");
        for c in &kept_roots {
            render_node(c, base + 1, &o, &mut body);
        }
        indent(&mut body, base, &o.unit);
        body.push_str("</>\n");
    }

    if component.is_empty() {
        return Ok(body.trim_end().to_string());
    }
    let u = &o.unit;
    Ok(format!(
        "export default function {component}() {{\n{u}return (\n{body}{u});\n}}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(html: &str) -> String {
        html_to_jsx(html, "2", "", "jsx", "explicit", "default").unwrap()
    }

    #[test]
    fn class_and_for_are_renamed() {
        assert_eq!(
            conv(r#"<label class="lbl" for="n">Name</label>"#),
            r#"<label className="lbl" htmlFor="n">Name</label>"#
        );
    }

    #[test]
    fn react_attribute_casing() {
        assert_eq!(
            conv(r#"<input tabindex="2" readonly maxlength="8" autocomplete="off">"#),
            r#"<input tabIndex={2} readOnly={true} maxLength={8} autoComplete="off" />"#
        );
    }

    #[test]
    fn inline_style_becomes_an_object() {
        assert_eq!(
            conv(
                r##"<p style="color: red; font-size: 12px; -webkit-box-shadow: none; --brand: #0af">hi</p>"##
            ),
            r##"<p style={{ color: "red", fontSize: "12px", WebkitBoxShadow: "none", "--brand": "#0af" }}>hi</p>"##
        );
    }

    #[test]
    fn void_tags_self_close_and_nesting_indents() {
        assert_eq!(
            conv("<div class=\"a\"><img src=\"x.png\"><br><span>hi</span></div>"),
            "<div className=\"a\">\n  <img src=\"x.png\" />\n  <br />\n  <span>hi</span>\n</div>"
        );
    }

    #[test]
    fn comments_and_text_are_jsx_safe() {
        assert_eq!(
            conv("<!-- note --><p>a { b } c</p>"),
            "<>\n  {/* note */}\n  <p>a {'{'} b {'}'} c</p>\n</>"
        );
    }

    #[test]
    fn comments_can_be_stripped() {
        assert_eq!(
            html_to_jsx(
                "<!-- x --><p>hi</p>",
                "2",
                "",
                "strip",
                "explicit",
                "default"
            )
            .unwrap(),
            "<p>hi</p>"
        );
    }

    #[test]
    fn boolean_shorthand_and_kept_value_attrs() {
        assert_eq!(
            html_to_jsx(
                r#"<input disabled value="a">"#,
                "2",
                "",
                "jsx",
                "shorthand",
                "keep"
            )
            .unwrap(),
            r#"<input disabled value="a" />"#
        );
        assert_eq!(
            conv(r#"<input checked value="a">"#),
            r#"<input defaultChecked={true} defaultValue="a" />"#
        );
    }

    #[test]
    fn component_wrapper_and_tab_indent() {
        assert_eq!(
            html_to_jsx("<div><b>hi</b></div>", "tab", "Card", "jsx", "explicit", "default")
                .unwrap(),
            "export default function Card() {\n\treturn (\n\t\t<div>\n\t\t\t<b>hi</b>\n\t\t</div>\n\t);\n}"
        );
    }

    #[test]
    fn svg_and_data_attributes() {
        assert_eq!(
            conv(r#"<svg viewbox="0 0 8 8" xmlns="http://www.w3.org/2000/svg"><path stroke-width="2" data-id="p1" aria-hidden="true" d="M0 0"/></svg>"#),
            "<svg viewBox=\"0 0 8 8\" xmlns=\"http://www.w3.org/2000/svg\">\n  <path strokeWidth=\"2\" data-id=\"p1\" aria-hidden=\"true\" d=\"M0 0\" />\n</svg>"
        );
    }

    #[test]
    fn implicit_closes_and_unclosed_tags() {
        assert_eq!(
            conv("<ul><li>a<li>b</ul>"),
            "<ul>\n  <li>a</li>\n  <li>b</li>\n</ul>"
        );
        assert_eq!(conv("<div><p>x"), "<div>\n  <p>x</p>\n</div>");
    }

    #[test]
    fn inline_handlers_become_functions() {
        assert_eq!(
            conv(r#"<button onclick="save()" onmouseover="hi()">Go</button>"#),
            "<button onClick={() => { save() }} onMouseOver={() => { hi() }}>Go</button>"
        );
    }

    #[test]
    fn style_and_script_content_is_preserved() {
        assert_eq!(
            conv("<style>.a { color: red; }</style>"),
            "<style>{`.a { color: red; }`}</style>"
        );
    }

    #[test]
    fn pre_keeps_whitespace() {
        assert_eq!(conv("<pre>  a\n  b</pre>"), "<pre>  a\n  b</pre>");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = html_to_jsx("   ", "2", "", "jsx", "explicit", "default").unwrap_err();
        assert!(err.contains("no HTML input"), "{err}");
    }

    #[test]
    fn unknown_option_is_an_error() {
        let err = html_to_jsx("<p>x</p>", "8", "", "jsx", "explicit", "default").unwrap_err();
        assert!(err.contains("unknown indent"), "{err}");
    }

    #[test]
    fn invalid_component_name_is_an_error() {
        let err = html_to_jsx("<p>x</p>", "2", "9lives", "jsx", "explicit", "default").unwrap_err();
        assert!(err.contains("invalid component name"), "{err}");
    }

    #[test]
    fn doctype_only_input_is_an_error() {
        let err =
            html_to_jsx("<!DOCTYPE html>", "2", "", "jsx", "explicit", "default").unwrap_err();
        assert!(err.contains("no convertible HTML"), "{err}");
    }
}
