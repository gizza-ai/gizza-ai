//! page-weight-analyzer core — parse pasted HTML and report a front-end
//! performance snapshot: resource counts, inline / render-blocking scripts and
//! stylesheets, an estimated request count and an estimated transfer-weight
//! budget. Pure compute, no wafer/wasm-bindgen deps; shared by the chat skill
//! block and the web page.
//!
//! No network is used — external sub-resources can't be downloaded, so their
//! transfer sizes are *estimated* from typical median file sizes (documented in
//! `AVG_*_KB`). The pasted HTML's own byte size is the only measured weight.
//! Everything else (counts, blocking classification) is derived directly from
//! the markup with a small, forgiving, quote-aware tag scanner — HTML is not
//! well-formed XML, so a real tokenizer (not an XML parser) is used.

// ---- estimated median transfer sizes (gzip/br), rough HTTP-Archive-style
// medians, used ONLY when the real file can't be measured offline ----
const AVG_JS_KB: f64 = 30.0;
const AVG_CSS_KB: f64 = 16.0;
const AVG_IMG_KB: f64 = 30.0;
const AVG_FONT_KB: f64 = 25.0;
const AVG_IFRAME_KB: f64 = 60.0;

// ---- common performance-budget guidance (lower is better) ----
const BUDGET_WEIGHT_KB: f64 = 1600.0; // ~1.6 MB total transfer
const BUDGET_REQUESTS: usize = 50;

/// A single external sub-resource discovered in the markup (used for the
/// optional per-resource listing).
struct Resource {
    kind: &'static str,
    url: String,
}

/// The full analysis of a single HTML document.
#[derive(Default)]
struct Analysis {
    html_bytes: usize,

    // scripts
    ext_scripts: usize,        // <script src> (incl. data: URIs)
    ext_script_reqs: usize,    // external scripts that are real network requests
    inline_scripts: usize,     // <script> with no src
    inline_script_bytes: usize,
    parser_blocking_ext_scripts: usize, // external classic, no async/defer
    inline_blocking_scripts: usize,     // inline classic (run synchronously)
    async_scripts: usize,
    defer_scripts: usize,
    module_scripts: usize,
    data_scripts: usize, // non-executable inline (json / ld+json / importmap / template)

    // stylesheets
    ext_styles: usize,
    ext_style_reqs: usize,
    inline_styles: usize,
    inline_style_bytes: usize,
    render_blocking_styles: usize,

    // other resources
    images: usize,
    image_reqs: usize,
    lazy_images: usize,
    iframes: usize,
    iframe_reqs: usize,
    media: usize, // <video> + <audio> elements
    media_reqs: usize,

    // resource hints
    preload: usize,
    font_preloads: usize,
    prefetch: usize,
    preconnect: usize,
    dns_prefetch: usize,

    // inline data: URIs seen on script/style/img (not network requests)
    data_uris: usize,

    resources: Vec<Resource>,
}

/// Analyze `html` and return a report (`output` = "report", default) or a
/// machine-readable summary (`output` = "json"). When `list_resources` is true
/// the report/JSON includes every external resource URL found, grouped by type.
pub fn analyze(html: &str, output: &str, list_resources: bool) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("html is required: paste the HTML source of a page to analyze".into());
    }
    let a = scan(html);
    match output.trim() {
        "" | "report" | "text" => Ok(render_report(&a, list_resources)),
        "json" => Ok(render_json(&a, list_resources)),
        other => Err(format!(
            "invalid output {other:?}: expected \"report\" or \"json\""
        )),
    }
}

// ---------------------------------------------------------------------------
// Scanner
// ---------------------------------------------------------------------------

fn scan(html: &str) -> Analysis {
    let b = html.as_bytes();
    let mut a = Analysis {
        html_bytes: b.len(),
        ..Default::default()
    };
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        // <!-- comment -->
        if b[i..].starts_with(b"<!--") {
            i = find_ci(b, b"-->", i + 4).map(|p| p + 3).unwrap_or(b.len());
            continue;
        }
        // <!doctype ...> or other declaration
        if i + 1 < b.len() && b[i + 1] == b'!' {
            i = find_byte(b, b'>', i + 1).map(|p| p + 1).unwrap_or(b.len());
            continue;
        }
        // </closing>
        if i + 1 < b.len() && b[i + 1] == b'/' {
            i = find_byte(b, b'>', i + 1).map(|p| p + 1).unwrap_or(b.len());
            continue;
        }
        // a start tag must begin <a-z / <A-Z
        if i + 1 >= b.len() || !b[i + 1].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let tag_end = find_tag_end(b, i);
        let (name, attrs) = parse_tag(&b[i + 1..tag_end]);
        i = (tag_end + 1).min(b.len());

        match name.as_str() {
            "script" => {
                // capture raw inner content up to </script>
                let close = find_ci(b, b"</script", i).unwrap_or(b.len());
                let inner_len = close.saturating_sub(i);
                i = find_byte(b, b'>', close)
                    .map(|p| p + 1)
                    .unwrap_or(b.len())
                    .max(i);
                handle_script(&mut a, &attrs, inner_len);
            }
            "style" => {
                let close = find_ci(b, b"</style", i).unwrap_or(b.len());
                let inner_len = close.saturating_sub(i);
                i = find_byte(b, b'>', close)
                    .map(|p| p + 1)
                    .unwrap_or(b.len())
                    .max(i);
                a.inline_styles += 1;
                a.inline_style_bytes += inner_len;
            }
            "link" => handle_link(&mut a, &attrs),
            "img" => {
                a.images += 1;
                if attr_is(&attrs, "loading", "lazy") {
                    a.lazy_images += 1;
                }
                if let Some(src) = get(&attrs, "src") {
                    record_url(&mut a, "image", src);
                }
            }
            "iframe" => {
                a.iframes += 1;
                if let Some(src) = get(&attrs, "src") {
                    if record_url(&mut a, "iframe", src) {
                        a.iframe_reqs += 1;
                    }
                }
            }
            "video" | "audio" => {
                a.media += 1;
                if let Some(src) = get(&attrs, "src") {
                    if record_url(&mut a, "media", src) {
                        a.media_reqs += 1;
                    }
                }
            }
            "source" => {
                // a <source> inside <video>/<audio>; <source srcset> in <picture>
                // is an image candidate. Count whichever URL it carries.
                if let Some(src) = get(&attrs, "src") {
                    if record_url(&mut a, "media", src) {
                        a.media_reqs += 1;
                    }
                }
            }
            _ => {}
        }
    }
    a
}

fn handle_script(a: &mut Analysis, attrs: &[(String, String)], inner_len: usize) {
    let typ = get(attrs, "type").unwrap_or("");
    let executable = is_js_type(typ);
    let module = is_module(typ);
    let has_async = has(attrs, "async");
    let has_defer = has(attrs, "defer");

    if module {
        a.module_scripts += 1;
    } else if has_async {
        a.async_scripts += 1;
    } else if has_defer {
        a.defer_scripts += 1;
    }

    match get(attrs, "src") {
        Some(src) => {
            a.ext_scripts += 1;
            let is_data = src.trim_start().to_ascii_lowercase().starts_with("data:");
            if is_data {
                a.data_uris += 1;
            } else {
                a.ext_script_reqs += 1;
                a.resources.push(Resource {
                    kind: "script",
                    url: src.to_string(),
                });
            }
            // parser-blocking: external classic JS without async/defer/module
            if executable && !module && !has_async && !has_defer && !is_data {
                a.parser_blocking_ext_scripts += 1;
            }
        }
        None => {
            a.inline_scripts += 1;
            a.inline_script_bytes += inner_len;
            if !executable {
                // e.g. application/json, application/ld+json, importmap, template
                a.data_scripts += 1;
            } else if !module {
                // inline classic scripts always run synchronously where they sit
                a.inline_blocking_scripts += 1;
            }
        }
    }
}

fn handle_link(a: &mut Analysis, attrs: &[(String, String)]) {
    let rel = get(attrs, "rel").unwrap_or("").to_ascii_lowercase();
    let rels: Vec<&str> = rel.split_whitespace().collect();
    let href = get(attrs, "href");
    let is_data = href
        .map(|h| h.trim_start().to_ascii_lowercase().starts_with("data:"))
        .unwrap_or(false);

    if rels.contains(&"stylesheet") {
        a.ext_styles += 1;
        let media = get(attrs, "media").unwrap_or("").to_ascii_lowercase();
        let print_only =
            media.contains("print") && !media.contains("screen") && !media.contains("all");
        let disabled = has(attrs, "disabled");
        if is_data {
            a.data_uris += 1;
        } else if let Some(h) = href {
            a.ext_style_reqs += 1;
            a.resources.push(Resource {
                kind: "stylesheet",
                url: h.to_string(),
            });
        }
        if !is_data && !disabled && !print_only {
            a.render_blocking_styles += 1;
        }
    } else if rels.contains(&"preload") || rels.contains(&"modulepreload") {
        a.preload += 1;
        if attr_is(attrs, "as", "font") {
            a.font_preloads += 1;
            if let Some(h) = href {
                if !is_data {
                    a.resources.push(Resource {
                        kind: "font",
                        url: h.to_string(),
                    });
                }
            }
        }
    } else if rels.contains(&"prefetch") {
        a.prefetch += 1;
    } else if rels.contains(&"preconnect") {
        a.preconnect += 1;
    } else if rels.contains(&"dns-prefetch") {
        a.dns_prefetch += 1;
    }
}

/// Record an external URL as an image-or-other request unless it's a data: URI.
/// Returns true when it counts as a real network request.
fn record_url(a: &mut Analysis, kind: &'static str, url: &str) -> bool {
    if url.trim_start().to_ascii_lowercase().starts_with("data:") {
        a.data_uris += 1;
        return false;
    }
    if kind == "image" {
        a.image_reqs += 1;
    }
    a.resources.push(Resource {
        kind,
        url: url.to_string(),
    });
    true
}

// ---------------------------------------------------------------------------
// Estimates + rendering
// ---------------------------------------------------------------------------

struct Estimate {
    requests: usize,
    weight_kb: f64,
    measured_html_kb: f64,
    est_js_kb: f64,
    est_css_kb: f64,
    est_img_kb: f64,
    est_iframe_kb: f64,
    est_font_kb: f64,
}

fn estimate(a: &Analysis) -> Estimate {
    let measured_html_kb = a.html_bytes as f64 / 1024.0;
    let est_js_kb = a.ext_script_reqs as f64 * AVG_JS_KB;
    let est_css_kb = a.ext_style_reqs as f64 * AVG_CSS_KB;
    let est_img_kb = a.image_reqs as f64 * AVG_IMG_KB;
    let est_iframe_kb = a.iframe_reqs as f64 * AVG_IFRAME_KB;
    let est_font_kb = a.font_preloads as f64 * AVG_FONT_KB;
    let requests = 1 // the HTML document itself
        + a.ext_script_reqs
        + a.ext_style_reqs
        + a.image_reqs
        + a.iframe_reqs
        + a.media_reqs
        + a.font_preloads;
    let weight_kb =
        measured_html_kb + est_js_kb + est_css_kb + est_img_kb + est_iframe_kb + est_font_kb;
    Estimate {
        requests,
        weight_kb,
        measured_html_kb,
        est_js_kb,
        est_css_kb,
        est_img_kb,
        est_iframe_kb,
        est_font_kb,
    }
}

fn render_report(a: &Analysis, list_resources: bool) -> String {
    let e = estimate(a);
    let mut s = String::new();
    s.push_str("Page Weight Analysis\n");
    s.push_str("====================\n\n");

    s.push_str("Document\n");
    s.push_str(&format!(
        "  HTML source size: {} ({} bytes)\n\n",
        kb(a.html_bytes),
        group(a.html_bytes)
    ));

    let total_scripts = a.ext_scripts + a.inline_scripts;
    s.push_str(&format!("Scripts ({total_scripts} total)\n"));
    s.push_str(&format!(
        "  External: {}  (parser-blocking: {}, async: {}, defer: {}, module: {})\n",
        a.ext_scripts, a.parser_blocking_ext_scripts, a.async_scripts, a.defer_scripts, a.module_scripts
    ));
    s.push_str(&format!(
        "  Inline:   {}  ({}, parser-blocking: {}",
        a.inline_scripts,
        kb(a.inline_script_bytes),
        a.inline_blocking_scripts
    ));
    if a.data_scripts > 0 {
        s.push_str(&format!(", data/json: {}", a.data_scripts));
    }
    s.push_str(")\n\n");

    let total_styles = a.ext_styles + a.inline_styles;
    s.push_str(&format!("Stylesheets ({total_styles} total)\n"));
    s.push_str(&format!(
        "  External <link>: {}  (render-blocking: {})\n",
        a.ext_styles, a.render_blocking_styles
    ));
    s.push_str(&format!(
        "  Inline <style>:  {}  ({})\n\n",
        a.inline_styles,
        kb(a.inline_style_bytes)
    ));

    s.push_str("Other resources\n");
    s.push_str(&format!(
        "  Images:         {}  (lazy-loaded: {})\n",
        a.images, a.lazy_images
    ));
    s.push_str(&format!("  iframes:        {}\n", a.iframes));
    s.push_str(&format!("  Audio/Video:    {}\n", a.media));
    s.push_str(&format!(
        "  Resource hints: preload {}, prefetch {}, preconnect {}, dns-prefetch {}\n",
        a.preload, a.prefetch, a.preconnect, a.dns_prefetch
    ));
    if a.font_preloads > 0 {
        s.push_str(&format!("  Font preloads:  {}\n", a.font_preloads));
    }
    if a.data_uris > 0 {
        s.push_str(&format!(
            "  Inline data: URIs: {} (embedded, not separate requests)\n",
            a.data_uris
        ));
    }
    s.push('\n');

    s.push_str(&format!(
        "Estimated network requests: {}  (1 HTML + {} sub-resources)\n",
        e.requests,
        e.requests - 1
    ));
    s.push_str("  A lower bound — assets requested by CSS/JS at runtime aren't counted.\n\n");

    s.push_str(&format!(
        "Estimated transfer weight (rough): ~{}\n",
        kbf(e.weight_kb)
    ));
    s.push_str(&format!(
        "  measured HTML:      {}\n",
        kbf(e.measured_html_kb)
    ));
    if a.ext_script_reqs > 0 {
        s.push_str(&format!(
            "  est. external JS:   ~{} ({} × ~{:.0} KB)\n",
            kbf(e.est_js_kb),
            a.ext_script_reqs,
            AVG_JS_KB
        ));
    }
    if a.ext_style_reqs > 0 {
        s.push_str(&format!(
            "  est. external CSS:  ~{} ({} × ~{:.0} KB)\n",
            kbf(e.est_css_kb),
            a.ext_style_reqs,
            AVG_CSS_KB
        ));
    }
    if a.image_reqs > 0 {
        s.push_str(&format!(
            "  est. images:        ~{} ({} × ~{:.0} KB)\n",
            kbf(e.est_img_kb),
            a.image_reqs,
            AVG_IMG_KB
        ));
    }
    if a.iframe_reqs > 0 {
        s.push_str(&format!(
            "  est. iframes:       ~{} ({} × ~{:.0} KB)\n",
            kbf(e.est_iframe_kb),
            a.iframe_reqs,
            AVG_IFRAME_KB
        ));
    }
    if a.font_preloads > 0 {
        s.push_str(&format!(
            "  est. fonts:         ~{} ({} × ~{:.0} KB)\n",
            kbf(e.est_font_kb),
            a.font_preloads,
            AVG_FONT_KB
        ));
    }
    s.push_str(
        "  External estimates use typical median file sizes — real sizes depend on the actual files.\n",
    );
    if a.media_reqs > 0 {
        s.push_str(&format!(
            "  Plus {} audio/video resource(s) — size varies too widely to estimate.\n",
            a.media_reqs
        ));
    }
    s.push('\n');

    // Budget verdict
    s.push_str("Performance budget\n");
    let weight_over = e.weight_kb > BUDGET_WEIGHT_KB;
    let req_over = e.requests > BUDGET_REQUESTS;
    s.push_str(&format!(
        "  Weight:   {} (~{} vs ~{:.0} KB budget)\n",
        if weight_over { "OVER" } else { "within" },
        kbf(e.weight_kb),
        BUDGET_WEIGHT_KB
    ));
    s.push_str(&format!(
        "  Requests: {} ({} vs {} budget)\n\n",
        if req_over { "OVER" } else { "within" },
        e.requests,
        BUDGET_REQUESTS
    ));

    // Recommendations
    s.push_str("Render-blocking & recommendations\n");
    let mut any = false;
    if a.parser_blocking_ext_scripts > 0 {
        any = true;
        s.push_str(&format!(
            "  • {} parser-blocking external script(s): add `defer` (or `async`), use `type=\"module\"`, or move them to the end of <body>.\n",
            a.parser_blocking_ext_scripts
        ));
    }
    if a.inline_blocking_scripts > 0 {
        any = true;
        s.push_str(&format!(
            "  • {} inline script(s) run synchronously and block HTML parsing at their position.\n",
            a.inline_blocking_scripts
        ));
    }
    if a.render_blocking_styles > 0 {
        any = true;
        s.push_str(&format!(
            "  • {} render-blocking stylesheet(s) delay first paint: inline critical CSS, split non-critical CSS, or load it with media/onload.\n",
            a.render_blocking_styles
        ));
    }
    if a.lazy_images < a.images && a.images > 0 {
        any = true;
        s.push_str(&format!(
            "  • {} of {} image(s) are not lazy-loaded: add loading=\"lazy\" to below-the-fold images.\n",
            a.images - a.lazy_images,
            a.images
        ));
    }
    if !any {
        s.push_str("  No render-blocking scripts or stylesheets detected. Looks lean.\n");
    }

    if list_resources && !a.resources.is_empty() {
        s.push_str("\nExternal resources\n");
        for kind in ["script", "stylesheet", "image", "font", "iframe", "media"] {
            let urls: Vec<&Resource> = a.resources.iter().filter(|r| r.kind == kind).collect();
            if urls.is_empty() {
                continue;
            }
            s.push_str(&format!("  {} ({})\n", plural(kind), urls.len()));
            for r in urls {
                s.push_str(&format!("    - {}\n", r.url));
            }
        }
    }

    s.trim_end().to_string()
}

fn render_json(a: &Analysis, list_resources: bool) -> String {
    let e = estimate(a);
    let mut s = String::new();
    s.push('{');
    s.push_str(&format!("\"html_bytes\":{},", a.html_bytes));
    s.push_str("\"scripts\":{");
    s.push_str(&format!("\"total\":{},", a.ext_scripts + a.inline_scripts));
    s.push_str(&format!("\"external\":{},", a.ext_scripts));
    s.push_str(&format!("\"inline\":{},", a.inline_scripts));
    s.push_str(&format!("\"inline_bytes\":{},", a.inline_script_bytes));
    s.push_str(&format!(
        "\"parser_blocking_external\":{},",
        a.parser_blocking_ext_scripts
    ));
    s.push_str(&format!("\"inline_blocking\":{},", a.inline_blocking_scripts));
    s.push_str(&format!("\"async\":{},", a.async_scripts));
    s.push_str(&format!("\"defer\":{},", a.defer_scripts));
    s.push_str(&format!("\"module\":{},", a.module_scripts));
    s.push_str(&format!("\"data\":{}", a.data_scripts));
    s.push_str("},");
    s.push_str("\"stylesheets\":{");
    s.push_str(&format!("\"total\":{},", a.ext_styles + a.inline_styles));
    s.push_str(&format!("\"external\":{},", a.ext_styles));
    s.push_str(&format!("\"inline\":{},", a.inline_styles));
    s.push_str(&format!("\"inline_bytes\":{},", a.inline_style_bytes));
    s.push_str(&format!("\"render_blocking\":{}", a.render_blocking_styles));
    s.push_str("},");
    s.push_str("\"resources\":{");
    s.push_str(&format!("\"images\":{},", a.images));
    s.push_str(&format!("\"lazy_images\":{},", a.lazy_images));
    s.push_str(&format!("\"iframes\":{},", a.iframes));
    s.push_str(&format!("\"media\":{},", a.media));
    s.push_str(&format!("\"preload\":{},", a.preload));
    s.push_str(&format!("\"font_preloads\":{},", a.font_preloads));
    s.push_str(&format!("\"prefetch\":{},", a.prefetch));
    s.push_str(&format!("\"preconnect\":{},", a.preconnect));
    s.push_str(&format!("\"dns_prefetch\":{},", a.dns_prefetch));
    s.push_str(&format!("\"data_uris\":{}", a.data_uris));
    s.push_str("},");
    s.push_str("\"estimate\":{");
    s.push_str(&format!("\"requests\":{},", e.requests));
    s.push_str(&format!("\"weight_kb\":{:.1},", e.weight_kb));
    s.push_str(&format!("\"weight_bytes\":{},", (e.weight_kb * 1024.0) as u64));
    s.push_str(&format!(
        "\"over_weight_budget\":{},",
        e.weight_kb > BUDGET_WEIGHT_KB
    ));
    s.push_str(&format!(
        "\"over_request_budget\":{}",
        e.requests > BUDGET_REQUESTS
    ));
    s.push('}');
    if list_resources {
        s.push_str(",\"external_resources\":[");
        for (n, r) in a.resources.iter().enumerate() {
            if n > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "{{\"type\":\"{}\",\"url\":\"{}\"}}",
                r.kind,
                json_escape(&r.url)
            ));
        }
        s.push(']');
    }
    s.push('}');
    s
}

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------

fn plural(kind: &str) -> &'static str {
    match kind {
        "script" => "Scripts",
        "stylesheet" => "Stylesheets",
        "image" => "Images",
        "font" => "Fonts",
        "iframe" => "iframes",
        "media" => "Audio/Video",
        _ => "Other",
    }
}

fn is_js_type(t: &str) -> bool {
    let lower = t.trim().to_ascii_lowercase();
    let base = lower.split(';').next().unwrap_or("").trim();
    matches!(
        base,
        "" | "text/javascript"
            | "application/javascript"
            | "module"
            | "text/ecmascript"
            | "application/ecmascript"
            | "text/jscript"
            | "application/x-javascript"
    )
}

fn is_module(t: &str) -> bool {
    t.trim().eq_ignore_ascii_case("module")
}

fn kb(bytes: usize) -> String {
    format!("{:.1} KB", bytes as f64 / 1024.0)
}

fn kbf(kb: f64) -> String {
    if kb >= 1024.0 {
        format!("{:.2} MB", kb / 1024.0)
    } else {
        format!("{:.1} KB", kb)
    }
}

/// Group a byte count with thousands separators (e.g. 12540 → "12,540").
fn group(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::new();
    let len = bytes.len();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c as char);
    }
    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

// ---- byte-level tag scanning ----

fn find_byte(b: &[u8], needle: u8, from: usize) -> Option<usize> {
    b.get(from..)?.iter().position(|&c| c == needle).map(|p| p + from)
}

/// Case-insensitive subsequence search (ASCII) from `from`.
fn find_ci(b: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > b.len() {
        return None;
    }
    let last = b.len().checked_sub(needle.len())?;
    (from..=last).find(|&i| {
        b[i..i + needle.len()]
            .iter()
            .zip(needle)
            .all(|(x, y)| x.eq_ignore_ascii_case(y))
    })
}

/// Find the index of the `>` that closes the tag starting at `start` (a `<`),
/// respecting quoted attribute values. Returns `b.len()` if unterminated.
fn find_tag_end(b: &[u8], start: usize) -> usize {
    let mut k = start + 1;
    let mut quote = 0u8;
    while k < b.len() {
        let c = b[k];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return k;
        }
        k += 1;
    }
    b.len()
}

/// Parse a start tag's inner bytes (everything between `<` and `>`) into a
/// lowercased tag name and its attribute (name, value) pairs.
fn parse_tag(s: &[u8]) -> (String, Vec<(String, String)>) {
    let mut i = 0usize;
    // tag name
    let start = i;
    while i < s.len() && (s[i].is_ascii_alphanumeric() || s[i] == b'-' || s[i] == b':') {
        i += 1;
    }
    let name = String::from_utf8_lossy(&s[start..i]).to_ascii_lowercase();

    let mut attrs: Vec<(String, String)> = Vec::new();
    while i < s.len() {
        // skip whitespace and stray '/'
        while i < s.len() && (s[i].is_ascii_whitespace() || s[i] == b'/') {
            i += 1;
        }
        if i >= s.len() {
            break;
        }
        // attribute name
        let ns = i;
        while i < s.len()
            && !s[i].is_ascii_whitespace()
            && s[i] != b'='
            && s[i] != b'/'
            && s[i] != b'>'
        {
            i += 1;
        }
        if i == ns {
            i += 1;
            continue;
        }
        let aname = String::from_utf8_lossy(&s[ns..i]).to_ascii_lowercase();
        // optional value
        while i < s.len() && s[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut aval = String::new();
        if i < s.len() && s[i] == b'=' {
            i += 1;
            while i < s.len() && s[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < s.len() && (s[i] == b'"' || s[i] == b'\'') {
                let q = s[i];
                i += 1;
                let vs = i;
                while i < s.len() && s[i] != q {
                    i += 1;
                }
                aval = String::from_utf8_lossy(&s[vs..i]).to_string();
                if i < s.len() {
                    i += 1; // skip closing quote
                }
            } else {
                let vs = i;
                while i < s.len() && !s[i].is_ascii_whitespace() && s[i] != b'>' {
                    i += 1;
                }
                aval = String::from_utf8_lossy(&s[vs..i]).to_string();
            }
        }
        attrs.push((aname, aval));
    }
    (name, attrs)
}

fn get<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn has(attrs: &[(String, String)], name: &str) -> bool {
    attrs.iter().any(|(k, _)| k == name)
}

fn attr_is(attrs: &[(String, String)], name: &str, value: &str) -> bool {
    get(attrs, name)
        .map(|v| v.trim().eq_ignore_ascii_case(value))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>Demo</title>
  <link rel="stylesheet" href="/main.css">
  <link rel="stylesheet" href="/print.css" media="print">
  <link rel="preload" href="/font.woff2" as="font" crossorigin>
  <link rel="preconnect" href="https://cdn.example.com">
  <style>body{margin:0}</style>
  <script src="/blocking.js"></script>
  <script src="/app.js" defer></script>
  <script src="https://cdn.example.com/lib.js" async></script>
  <script type="module" src="/m.js"></script>
  <script type="application/ld+json">{"@context":"x"}</script>
</head>
<body>
  <script>console.log('inline')</script>
  <img src="/a.png">
  <img src="/b.png" loading="lazy">
  <img src="data:image/png;base64,AAAA">
  <iframe src="https://www.youtube.com/embed/x"></iframe>
</body>
</html>"#;

    #[test]
    fn counts_scripts_correctly() {
        let a = scan(PAGE);
        // 4 external (blocking, app.js defer, lib.js async, m.js module) + 1 inline JS + 1 inline ld+json
        assert_eq!(a.ext_scripts, 4, "external scripts");
        assert_eq!(a.inline_scripts, 2, "inline scripts (js + ld+json)");
        assert_eq!(a.parser_blocking_ext_scripts, 1, "only blocking.js blocks");
        assert_eq!(a.async_scripts, 1);
        assert_eq!(a.defer_scripts, 1);
        assert_eq!(a.module_scripts, 1);
        assert_eq!(a.inline_blocking_scripts, 1, "the console.log inline");
        assert_eq!(a.data_scripts, 1, "ld+json is non-executable");
    }

    #[test]
    fn counts_stylesheets_correctly() {
        let a = scan(PAGE);
        assert_eq!(a.ext_styles, 2, "main + print");
        assert_eq!(a.render_blocking_styles, 1, "print.css is not blocking");
        assert_eq!(a.inline_styles, 1);
        assert!(a.inline_style_bytes > 0);
    }

    #[test]
    fn counts_other_resources() {
        let a = scan(PAGE);
        assert_eq!(a.images, 3, "two real + one data uri");
        assert_eq!(a.image_reqs, 2, "data: image is not a request");
        assert_eq!(a.lazy_images, 1);
        assert_eq!(a.iframes, 1);
        assert_eq!(a.preconnect, 1);
        assert_eq!(a.preload, 1);
        assert_eq!(a.font_preloads, 1);
        assert!(a.data_uris >= 1);
    }

    #[test]
    fn estimate_includes_html_and_externals() {
        let a = scan(PAGE);
        let e = estimate(&a);
        // requests: 1 html + 4 scripts + 2 css + 2 img + 1 iframe + 1 font = 11
        assert_eq!(e.requests, 11, "request count");
        assert!(e.weight_kb > a.html_bytes as f64 / 1024.0, "weight adds externals");
    }

    #[test]
    fn report_renders_key_sections() {
        let out = analyze(PAGE, "report", false).unwrap();
        assert!(out.contains("Page Weight Analysis"), "{out}");
        assert!(out.contains("Scripts ("));
        assert!(out.contains("Stylesheets ("));
        assert!(out.contains("Estimated network requests:"));
        assert!(out.contains("Performance budget"));
        assert!(out.contains("parser-blocking external script"));
    }

    #[test]
    fn json_output_is_parseable_shape() {
        let out = analyze(PAGE, "json", true).unwrap();
        assert!(out.starts_with('{') && out.ends_with('}'), "{out}");
        assert!(out.contains("\"html_bytes\":"));
        assert!(out.contains("\"parser_blocking_external\":1"));
        assert!(out.contains("\"external_resources\":["));
        assert!(out.contains("\"type\":\"script\""));
    }

    #[test]
    fn list_resources_lists_urls() {
        let out = analyze(PAGE, "report", true).unwrap();
        assert!(out.contains("External resources"));
        assert!(out.contains("/main.css"));
        assert!(out.contains("/blocking.js"));
        assert!(out.contains("/font.woff2"));
    }

    #[test]
    fn empty_html_errors() {
        let err = analyze("   ", "report", false).unwrap_err();
        assert!(err.contains("html is required"), "{err}");
    }

    #[test]
    fn invalid_output_errors() {
        let err = analyze("<p>x</p>", "csv", false).unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
    }

    #[test]
    fn handles_quotes_with_angle_brackets_in_attrs() {
        // a > inside a quoted attribute must not end the tag early
        let html = r#"<img alt="a > b" src="/x.png"><script src="/y.js"></script>"#;
        let a = scan(html);
        assert_eq!(a.images, 1);
        assert_eq!(a.image_reqs, 1);
        assert_eq!(a.ext_scripts, 1);
    }

    #[test]
    fn ignores_script_like_content_in_comments() {
        let html = "<!-- <script src=/evil.js></script> --><p>ok</p>";
        let a = scan(html);
        assert_eq!(a.ext_scripts, 0, "commented-out script not counted");
    }

    #[test]
    fn unclosed_script_does_not_panic() {
        let html = "<script>var x = 1;";
        let a = scan(html);
        assert_eq!(a.inline_scripts, 1);
        assert!(a.inline_script_bytes > 0);
    }

    #[test]
    fn no_blocking_resources_message() {
        let html = "<p>Hello</p><img src=/a.png loading=lazy>";
        let out = analyze(html, "report", false).unwrap();
        assert!(out.contains("No render-blocking"), "{out}");
    }
}
