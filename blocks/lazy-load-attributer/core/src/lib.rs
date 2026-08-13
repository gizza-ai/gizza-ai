//! gizza-ai/lazy-load-attributer core — add `loading="lazy"` and
//! `decoding="async"` to `<img>`/`<iframe>` tags that lack them.
//!
//! A forgiving, dependency-free tag scanner (HTML is not well-formed XML). Only
//! the matched start tags are rewritten; every other byte of the document —
//! text, comments, doctype, `script`/`style` bodies, attribute formatting — is
//! copied through verbatim. An attribute that is already present is NEVER
//! overwritten, so re-running the tool is a no-op.

/// Which element kinds to rewrite.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Targets {
    Both,
    Images,
    Iframes,
}

impl Targets {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "both" | "" => Ok(Targets::Both),
            "images" => Ok(Targets::Images),
            "iframes" => Ok(Targets::Iframes),
            other => Err(format!(
                "unknown targets '{other}' (expected: both, images, iframes)"
            )),
        }
    }

    fn wants(&self, tag: &str) -> bool {
        match self {
            Targets::Both => tag == "img" || tag == "iframe",
            Targets::Images => tag == "img",
            Targets::Iframes => tag == "iframe",
        }
    }
}

/// Value written for `decoding`, or `None` to leave the attribute alone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decoding {
    Async,
    Sync,
    Auto,
    None,
}

impl Decoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "async" | "" => Ok(Decoding::Async),
            "sync" => Ok(Decoding::Sync),
            "auto" => Ok(Decoding::Auto),
            "none" => Ok(Decoding::None),
            other => Err(format!(
                "unknown decoding '{other}' (expected: async, sync, auto, none)"
            )),
        }
    }

    fn value(&self) -> Option<&'static str> {
        match self {
            Decoding::Async => Some("async"),
            Decoding::Sync => Some("sync"),
            Decoding::Auto => Some("auto"),
            Decoding::None => None,
        }
    }
}

/// Output shape: the rewritten HTML, or a human-readable change report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    Html,
    Report,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "html" | "" => Ok(Output::Html),
            "report" => Ok(Output::Report),
            other => Err(format!("unknown output '{other}' (expected: html, report)")),
        }
    }
}

/// Everything the caller can tune. Mirrors the descriptor params 1:1.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub targets: Targets,
    pub decoding: Decoding,
    /// Leave the first N matched images untouched (LCP / above-the-fold guard).
    /// Counts images only — an iframe is never an LCP candidate here.
    pub skip_first: usize,
    /// Write `loading="eager"` on those first N images instead of nothing.
    pub eager_first: bool,
    /// Write `fetchpriority="high"` on the very first image.
    pub fetchpriority_first: bool,
    /// Honour `skip-lazy`/`no-lazy` classes and `data-skip-lazy`/`data-no-lazy`.
    pub respect_skip_markers: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            targets: Targets::Both,
            decoding: Decoding::Async,
            skip_first: 0,
            eager_first: false,
            fetchpriority_first: false,
            respect_skip_markers: true,
        }
    }
}

/// Per-run counters, surfaced by `output = report`.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub images_seen: usize,
    pub iframes_seen: usize,
    pub loading_added: usize,
    pub decoding_added: usize,
    pub fetchpriority_added: usize,
    /// Already carried the attribute we would have written.
    pub already_set: usize,
    /// Held back by `skip_first`.
    pub skipped_first: usize,
    /// Held back by a `skip-lazy`/`no-lazy` marker.
    pub skipped_marker: usize,
    /// Held back because the tag has no `src`.
    pub skipped_no_src: usize,
}

/// Maximum input size — keeps a pasted page bounded on the browser surface.
pub const MAX_HTML_BYTES: usize = 2_000_000;
/// Upper bound on `skip_first`.
pub const MAX_SKIP_FIRST: usize = 50;

/// Index just past the tag's closing `>`, respecting quoted attribute values.
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

/// Lowercased element name of a start tag, e.g. `<IMG src=x>` → `img`.
fn tag_name(raw: &str) -> String {
    raw.trim_start_matches('<')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// True if the start tag carries `name` as an attribute. Scans attribute
/// positions rather than substring-matching, so `data-loading` / a `loading`
/// substring inside an attribute VALUE never counts as a match.
fn has_attr(raw: &str, name: &str) -> bool {
    attr_value(raw, name).is_some()
}

/// The value of attribute `name` on a start tag, if present. Returns an empty
/// string for a bare/valueless attribute.
fn attr_value(raw: &str, name: &str) -> Option<String> {
    let b = raw.as_bytes();
    // Skip `<tagname`; attributes start after it.
    let mut i = 1;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-') {
        i += 1;
    }
    while i < b.len() {
        // Skip separators.
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/' || b[i] == b'>') {
            i += 1;
        }
        if i >= b.len() {
            return None;
        }
        // Read the attribute name.
        let name_start = i;
        while i < b.len()
            && !b[i].is_ascii_whitespace()
            && b[i] != b'='
            && b[i] != b'>'
            && b[i] != b'/'
        {
            i += 1;
        }
        if i == name_start {
            i += 1;
            continue;
        }
        let this = raw[name_start..i].to_ascii_lowercase();
        // Optional `= value`.
        let mut ws = i;
        while ws < b.len() && b[ws].is_ascii_whitespace() {
            ws += 1;
        }
        let mut value = String::new();
        if ws < b.len() && b[ws] == b'=' {
            i = ws + 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = raw[vs..i.min(raw.len())].to_string();
                if i < b.len() {
                    i += 1; // past the closing quote
                }
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                value = raw[vs..i.min(raw.len())].to_string();
            }
        }
        if this == name {
            return Some(value);
        }
    }
    None
}

/// True if the tag opts out via the cross-tool lazy-load skip vocabulary:
/// a `skip-lazy` / `no-lazy` class, or a `data-skip-lazy` / `data-no-lazy`
/// attribute (any value, including bare).
fn has_skip_marker(raw: &str) -> bool {
    if has_attr(raw, "data-skip-lazy") || has_attr(raw, "data-no-lazy") {
        return true;
    }
    match attr_value(raw, "class") {
        Some(classes) => classes
            .split_whitespace()
            .any(|c| c.eq_ignore_ascii_case("skip-lazy") || c.eq_ignore_ascii_case("no-lazy")),
        None => false,
    }
}

/// Insert `attrs` just before the tag's terminator, preserving `/>` if present.
fn insert_attrs(raw: &str, attrs: &[(&str, &str)]) -> String {
    if attrs.is_empty() {
        return raw.to_string();
    }
    // Split off the trailing `>` (and a preceding `/` for XHTML-style tags).
    let body = raw.strip_suffix('>').unwrap_or(raw);
    let (body, tail) = match body.strip_suffix('/') {
        Some(b) => (b, " />"),
        None => (body, ">"),
    };
    let mut out = body.trim_end().to_string();
    for (k, v) in attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(v);
        out.push('"');
    }
    // A tag that was `<img/>` keeps its self-closing form; `<img>` stays plain.
    if tail == " />" {
        out.push_str(" />");
    } else {
        out.push('>');
    }
    out
}

/// Rewrite `html`, returning the new markup plus per-run counters.
pub fn transform(html: &str, opts: &Options) -> Result<(String, Stats), String> {
    if html.trim().is_empty() {
        return Err("no HTML input — paste some markup containing <img> or <iframe> tags".into());
    }
    if html.len() > MAX_HTML_BYTES {
        return Err(format!(
            "HTML is too large: {} bytes (limit {} bytes)",
            html.len(),
            MAX_HTML_BYTES
        ));
    }
    if opts.skip_first > MAX_SKIP_FIRST {
        return Err(format!(
            "skip_first must be 0-{MAX_SKIP_FIRST} (got {})",
            opts.skip_first
        ));
    }

    let b = html.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n + 64);
    let mut stats = Stats::default();
    let mut image_index = 0usize;
    let mut i = 0usize;

    while i < n {
        if b[i] != b'<' {
            let start = i;
            while i < n && b[i] != b'<' {
                i += 1;
            }
            out.push_str(&html[start..i]);
            continue;
        }
        // Comments are opaque — copy through so `<!-- <img> -->` is untouched.
        if html[i..].starts_with("<!--") {
            let end = html[i..].find("-->").map(|p| i + p + 3).unwrap_or(n);
            out.push_str(&html[i..end]);
            i = end;
            continue;
        }
        let end = scan_tag(b, i);
        let raw = &html[i..end];
        i = end;

        // Closing tags, doctype and declarations pass through untouched.
        if raw.len() < 2 || b'/' == raw.as_bytes()[1] || raw.as_bytes()[1] == b'!' {
            out.push_str(raw);
            continue;
        }
        let name = tag_name(raw);
        if !opts.targets.wants(&name) {
            out.push_str(raw);
            continue;
        }
        let is_img = name == "img";
        if is_img {
            stats.images_seen += 1;
        } else {
            stats.iframes_seen += 1;
        }
        let this_image = if is_img {
            image_index += 1;
            image_index
        } else {
            0
        };

        // Nothing to defer without a source (matches WordPress core's rule).
        if !has_attr(raw, "src") && !has_attr(raw, "srcset") {
            stats.skipped_no_src += 1;
            out.push_str(raw);
            continue;
        }
        if opts.respect_skip_markers && has_skip_marker(raw) {
            stats.skipped_marker += 1;
            out.push_str(raw);
            continue;
        }

        // The first `skip_first` IMAGES are the above-the-fold candidates.
        let in_skip_window = is_img && this_image <= opts.skip_first;
        let mut owned: Vec<(String, String)> = Vec::new();

        if in_skip_window {
            stats.skipped_first += 1;
            if opts.eager_first && !has_attr(raw, "loading") {
                owned.push(("loading".into(), "eager".into()));
            }
        } else if has_attr(raw, "loading") {
            stats.already_set += 1;
        } else {
            owned.push(("loading".into(), "lazy".into()));
            stats.loading_added += 1;
        }

        // `decoding` is valid on <img> only; iframes have no decode step.
        if is_img {
            if let Some(v) = opts.decoding.value() {
                if !has_attr(raw, "decoding") {
                    owned.push(("decoding".into(), v.into()));
                    stats.decoding_added += 1;
                }
            }
        }
        if opts.fetchpriority_first && this_image == 1 && !has_attr(raw, "fetchpriority") {
            owned.push(("fetchpriority".into(), "high".into()));
            stats.fetchpriority_added += 1;
        }

        let borrowed: Vec<(&str, &str)> = owned
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        out.push_str(&insert_attrs(raw, &borrowed));
    }

    Ok((out, stats))
}

/// Human-readable change report for `output = report`.
pub fn report(stats: &Stats, opts: &Options) -> String {
    let mut s = String::new();
    s.push_str("Lazy-load attribute report\n");
    s.push_str("==========================\n\n");
    s.push_str(&format!("  <img> tags found:       {}\n", stats.images_seen));
    s.push_str(&format!(
        "  <iframe> tags found:    {}\n\n",
        stats.iframes_seen
    ));
    s.push_str(&format!(
        "  loading=\"lazy\" added:    {}\n",
        stats.loading_added
    ));
    if let Some(v) = opts.decoding.value() {
        s.push_str(&format!(
            "  decoding=\"{v}\" added:   {}\n",
            stats.decoding_added
        ));
    }
    if opts.fetchpriority_first {
        s.push_str(&format!(
            "  fetchpriority=\"high\":    {}\n",
            stats.fetchpriority_added
        ));
    }
    s.push_str("\nLeft unchanged\n--------------\n");
    s.push_str(&format!(
        "  already had loading:    {}\n",
        stats.already_set
    ));
    s.push_str(&format!(
        "  first {} image(s) kept eager: {}\n",
        opts.skip_first, stats.skipped_first
    ));
    s.push_str(&format!(
        "  skip-lazy / no-lazy:    {}\n",
        stats.skipped_marker
    ));
    s.push_str(&format!(
        "  no src or srcset:       {}\n",
        stats.skipped_no_src
    ));
    s
}

/// Entry point: transform, then render as HTML or as a report.
pub fn run(html: &str, opts: &Options, output: Output) -> Result<String, String> {
    let (rewritten, stats) = transform(html, opts)?;
    Ok(match output {
        Output::Html => rewritten,
        Output::Report => report(&stats, opts),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html_of(input: &str, opts: &Options) -> String {
        run(input, opts, Output::Html).unwrap()
    }

    #[test]
    fn adds_loading_and_decoding_to_a_plain_img() {
        let got = html_of(r#"<img src="a.png">"#, &Options::default());
        assert_eq!(got, r#"<img src="a.png" loading="lazy" decoding="async">"#);
    }

    #[test]
    fn adds_loading_to_an_iframe_but_not_decoding() {
        let got = html_of(r#"<iframe src="https://e.example/v"></iframe>"#, &Options::default());
        assert_eq!(
            got,
            r#"<iframe src="https://e.example/v" loading="lazy"></iframe>"#
        );
    }

    #[test]
    fn never_overrides_an_existing_attribute() {
        let src = r#"<img src="a.png" loading="eager" decoding="sync">"#;
        assert_eq!(html_of(src, &Options::default()), src);
    }

    #[test]
    fn is_idempotent() {
        let once = html_of(r#"<img src="a.png">"#, &Options::default());
        let twice = html_of(&once, &Options::default());
        assert_eq!(once, twice);
    }

    #[test]
    fn leaves_surrounding_markup_byte_for_byte() {
        let src = "<!DOCTYPE html>\n<p class='x'>hi &amp; bye</p>\n<img src=a.png>\n<br/>";
        let got = html_of(src, &Options::default());
        assert_eq!(
            got,
            "<!DOCTYPE html>\n<p class='x'>hi &amp; bye</p>\n<img src=a.png loading=\"lazy\" decoding=\"async\">\n<br/>"
        );
    }

    #[test]
    fn skips_images_without_a_source() {
        let src = r#"<img alt="spacer">"#;
        assert_eq!(html_of(src, &Options::default()), src);
    }

    #[test]
    fn srcset_only_image_still_qualifies() {
        let got = html_of(r#"<img srcset="a.png 1x">"#, &Options::default());
        assert_eq!(got, r#"<img srcset="a.png 1x" loading="lazy" decoding="async">"#);
    }

    #[test]
    fn skip_first_leaves_the_lcp_image_alone() {
        let opts = Options {
            skip_first: 1,
            ..Options::default()
        };
        let got = html_of(r#"<img src="hero.png"><img src="b.png">"#, &opts);
        assert_eq!(
            got,
            r#"<img src="hero.png" decoding="async"><img src="b.png" loading="lazy" decoding="async">"#
        );
    }

    #[test]
    fn eager_first_marks_the_skipped_images() {
        let opts = Options {
            skip_first: 1,
            eager_first: true,
            fetchpriority_first: true,
            ..Options::default()
        };
        let got = html_of(r#"<img src="hero.png"><img src="b.png">"#, &opts);
        assert_eq!(
            got,
            r#"<img src="hero.png" loading="eager" decoding="async" fetchpriority="high"><img src="b.png" loading="lazy" decoding="async">"#
        );
    }

    #[test]
    fn skip_markers_are_respected_and_can_be_disabled() {
        let src = r#"<img src="a.png" class="hero skip-lazy"><img src="b.png" data-no-lazy="1">"#;
        assert_eq!(html_of(src, &Options::default()), src);
        let opts = Options {
            respect_skip_markers: false,
            ..Options::default()
        };
        let got = html_of(src, &opts);
        assert!(got.contains(r#"class="hero skip-lazy" loading="lazy""#));
        assert!(got.contains(r#"data-no-lazy="1" loading="lazy""#));
    }

    #[test]
    fn targets_can_narrow_to_images_or_iframes() {
        let src = r#"<img src="a.png"><iframe src="b"></iframe>"#;
        let images = Options {
            targets: Targets::Images,
            ..Options::default()
        };
        let got = html_of(src, &images);
        assert!(got.contains(r#"<img src="a.png" loading="lazy""#));
        assert!(got.contains(r#"<iframe src="b">"#));

        let iframes = Options {
            targets: Targets::Iframes,
            ..Options::default()
        };
        let got = html_of(src, &iframes);
        assert_eq!(got, r#"<img src="a.png"><iframe src="b" loading="lazy"></iframe>"#);
    }

    #[test]
    fn decoding_value_is_selectable_and_can_be_omitted() {
        let opts = Options {
            decoding: Decoding::Sync,
            ..Options::default()
        };
        assert_eq!(
            html_of(r#"<img src="a.png">"#, &opts),
            r#"<img src="a.png" loading="lazy" decoding="sync">"#
        );
        let opts = Options {
            decoding: Decoding::None,
            ..Options::default()
        };
        assert_eq!(
            html_of(r#"<img src="a.png">"#, &opts),
            r#"<img src="a.png" loading="lazy">"#
        );
    }

    #[test]
    fn self_closing_tags_keep_their_form() {
        let got = html_of(r#"<img src="a.png" />"#, &Options::default());
        assert_eq!(got, r#"<img src="a.png" loading="lazy" decoding="async" />"#);
    }

    #[test]
    fn uppercase_tags_and_attributes_are_matched() {
        let got = html_of(r#"<IMG SRC="a.png" LOADING="eager">"#, &Options::default());
        assert_eq!(got, r#"<IMG SRC="a.png" LOADING="eager" decoding="async">"#);
    }

    #[test]
    fn gt_inside_an_attribute_value_is_safe() {
        let got = html_of(r#"<img src="a.png" alt="x > y">"#, &Options::default());
        assert_eq!(
            got,
            r#"<img src="a.png" alt="x > y" loading="lazy" decoding="async">"#
        );
    }

    #[test]
    fn tags_inside_comments_are_untouched() {
        let src = "<!-- <img src=\"a.png\"> --><img src=\"b.png\">";
        let got = html_of(src, &Options::default());
        assert!(got.starts_with("<!-- <img src=\"a.png\"> -->"));
        assert!(got.contains(r#"<img src="b.png" loading="lazy""#));
    }

    #[test]
    fn data_loading_attribute_is_not_mistaken_for_loading() {
        let got = html_of(r#"<img src="a.png" data-loading="x">"#, &Options::default());
        assert!(got.contains(r#"loading="lazy""#));
    }

    #[test]
    fn report_counts_every_outcome() {
        let opts = Options {
            skip_first: 1,
            ..Options::default()
        };
        let src = r#"<img src="hero.png"><img src="b.png"><img src="c.png" loading="eager"><img alt="no-src"><img src="d.png" class="no-lazy"><iframe src="e"></iframe>"#;
        let (_, stats) = transform(src, &opts).unwrap();
        assert_eq!(stats.images_seen, 5);
        assert_eq!(stats.iframes_seen, 1);
        assert_eq!(stats.loading_added, 2); // b.png + the iframe
        assert_eq!(stats.already_set, 1);
        assert_eq!(stats.skipped_first, 1);
        assert_eq!(stats.skipped_marker, 1);
        assert_eq!(stats.skipped_no_src, 1);
        let text = report(&stats, &opts);
        assert!(text.contains("<img> tags found:       5"));
    }

    #[test]
    fn errors_on_empty_input() {
        let err = run("   ", &Options::default(), Output::Html).unwrap_err();
        assert!(err.contains("no HTML input"), "{err}");
    }

    #[test]
    fn errors_on_oversized_input() {
        let big = "x".repeat(MAX_HTML_BYTES + 1);
        let err = run(&big, &Options::default(), Output::Html).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn errors_on_out_of_range_skip_first() {
        let opts = Options {
            skip_first: MAX_SKIP_FIRST + 1,
            ..Options::default()
        };
        let err = run("<img src=a>", &opts, Output::Html).unwrap_err();
        assert!(err.contains("skip_first"), "{err}");
    }

    #[test]
    fn enum_parsers_reject_unknown_values() {
        assert!(Targets::parse("videos").is_err());
        assert!(Decoding::parse("fast").is_err());
        assert!(Output::parse("yaml").is_err());
        assert_eq!(Targets::parse("IMAGES").unwrap(), Targets::Images);
        assert_eq!(Decoding::parse("").unwrap(), Decoding::Async);
        assert_eq!(Output::parse("report").unwrap(), Output::Report);
    }
}
