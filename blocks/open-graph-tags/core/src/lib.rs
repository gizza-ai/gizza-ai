//! gizza-ai/open-graph-tags core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Turns a page's title/description/URL/image (plus optional social metadata) into a
//! copy-pasteable block of `<head>` markup: standard meta tags, Open Graph (`og:*`) tags
//! for Facebook / LinkedIn / Slack / Discord, Twitter Card (`twitter:*`) tags for X, and
//! optional schema.org `itemprop` tags. Values are HTML-escaped so a title containing
//! `&`, `<` or `"` can't break out of the `content="…"` attribute.

/// Accepted `og:type` values. `website` is the default; the rest cover the object types
/// social crawlers actually treat specially.
pub const OG_TYPES: [&str; 7] = [
    "website",
    "article",
    "product",
    "profile",
    "video.other",
    "music.song",
    "book",
];

/// Accepted `twitter:card` values.
pub const TWITTER_CARDS: [&str; 4] = ["summary_large_image", "summary", "player", "app"];

/// Recommended `<title>` length ceiling — longer titles get truncated in most previews.
pub const TITLE_MAX: usize = 60;
/// Recommended meta-description bounds (characters).
pub const DESC_MIN: usize = 50;
/// Recommended meta-description ceiling (characters).
pub const DESC_MAX: usize = 160;
/// Upper bound accepted for `og:image:width` / `og:image:height` (pixels).
pub const MAX_IMAGE_DIMENSION: u32 = 10000;

/// Every field the generator understands. All strings are trimmed by [`generate`];
/// an empty string means "omit the corresponding tag".
#[derive(Debug, Default, Clone)]
pub struct Options {
    pub title: String,
    pub description: String,
    pub url: String,
    pub image: String,
    pub image_alt: String,
    /// `og:image:width` in pixels; 0 omits the tag.
    pub image_width: u32,
    /// `og:image:height` in pixels; 0 omits the tag.
    pub image_height: u32,
    pub site_name: String,
    pub og_type: String,
    pub twitter_card: String,
    pub twitter_site: String,
    pub twitter_creator: String,
    pub locale: String,
    pub author: String,
    pub include_basic: bool,
    pub include_twitter: bool,
    pub include_schema: bool,
    pub group_comments: bool,
    pub warnings: bool,
}

impl Options {
    /// The defaults the descriptor advertises: everything but the schema.org block on.
    pub fn new(title: &str) -> Self {
        Options {
            title: title.to_string(),
            og_type: "website".into(),
            twitter_card: "summary_large_image".into(),
            locale: "en_US".into(),
            include_basic: true,
            include_twitter: true,
            include_schema: false,
            group_comments: true,
            warnings: true,
            ..Default::default()
        }
    }
}

/// Escape a value for use inside a double-quoted HTML attribute.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            // Newlines inside an attribute are legal but wreck the copy-paste block.
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// Escape text placed between `<title>` and `</title>` (no attribute quoting needed).
fn esc_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace(['\n', '\r'], " ")
}

/// Make a value safe to place inside an HTML comment: `--` would close it early.
fn esc_comment(s: &str) -> String {
    let mut out = s.replace("--", "- -");
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Normalize a social handle to the `@name` form crawlers expect. Accepts `name`,
/// `@name`, or a profile URL like `https://x.com/name`.
fn handle(raw: &str) -> String {
    let mut s = raw.trim();
    for prefix in ["https://", "http://", "www.", "x.com/", "twitter.com/", "@"] {
        s = s.strip_prefix(prefix).unwrap_or(s);
    }
    // A pasted profile URL can carry a query string or trailing slash.
    let s = s.split(['/', '?', '#']).next().unwrap_or(s).trim();
    if s.is_empty() {
        String::new()
    } else {
        format!("@{s}")
    }
}

/// True when `u` is an absolute http(s) URL — the only form social crawlers resolve.
fn is_absolute_http(u: &str) -> bool {
    let lower = u.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://")) && u.len() > "https://".len()
}

fn tag_property(buf: &mut String, property: &str, content: &str) {
    buf.push_str(&format!(
        "<meta property=\"{}\" content=\"{}\">\n",
        property,
        esc(content)
    ));
}

fn tag_name(buf: &mut String, name: &str, content: &str) {
    buf.push_str(&format!(
        "<meta name=\"{}\" content=\"{}\">\n",
        name,
        esc(content)
    ));
}

/// Build the `<head>` markup described by `opts`.
///
/// Errors when `title` is blank, or when `og_type` / `twitter_card` is not one of the
/// advertised values, or when an image dimension exceeds [`MAX_IMAGE_DIMENSION`].
pub fn generate(opts: &Options) -> Result<String, String> {
    let title = opts.title.trim();
    if title.is_empty() {
        return Err(
            "title is required — pass the page title, e.g. title=\"How to bake sourdough\"".into(),
        );
    }

    let og_type = {
        let t = opts.og_type.trim();
        let t = if t.is_empty() { "website" } else { t };
        if !OG_TYPES.contains(&t) {
            return Err(format!(
                "og_type must be one of {} — got \"{t}\"",
                OG_TYPES.join(", ")
            ));
        }
        t
    };

    let twitter_card = {
        let c = opts.twitter_card.trim();
        let c = if c.is_empty() {
            "summary_large_image"
        } else {
            c
        };
        if !TWITTER_CARDS.contains(&c) {
            return Err(format!(
                "twitter_card must be one of {} — got \"{c}\"",
                TWITTER_CARDS.join(", ")
            ));
        }
        c
    };

    for (label, v) in [
        ("image_width", opts.image_width),
        ("image_height", opts.image_height),
    ] {
        if v > MAX_IMAGE_DIMENSION {
            return Err(format!(
                "{label} must be between 0 and {MAX_IMAGE_DIMENSION} pixels (0 omits the tag) — got {v}"
            ));
        }
    }

    let description = opts.description.trim();
    let url = opts.url.trim();
    let image = opts.image.trim();
    let image_alt = opts.image_alt.trim();
    let site_name = opts.site_name.trim();
    let locale = opts.locale.trim();
    let author = opts.author.trim();
    let twitter_site = handle(&opts.twitter_site);
    let twitter_creator = handle(&opts.twitter_creator);

    let mut out = String::new();

    // ---- Standard meta tags -------------------------------------------------
    if opts.include_basic {
        if opts.group_comments {
            out.push_str("<!-- Standard meta tags -->\n");
        }
        out.push_str(&format!("<title>{}</title>\n", esc_text(title)));
        if !description.is_empty() {
            tag_name(&mut out, "description", description);
        }
        if !author.is_empty() {
            tag_name(&mut out, "author", author);
        }
        if !url.is_empty() {
            out.push_str(&format!("<link rel=\"canonical\" href=\"{}\">\n", esc(url)));
        }
        out.push('\n');
    }

    // ---- Open Graph ---------------------------------------------------------
    if opts.group_comments {
        out.push_str("<!-- Open Graph — Facebook, LinkedIn, Slack, Discord -->\n");
    }
    tag_property(&mut out, "og:type", og_type);
    tag_property(&mut out, "og:title", title);
    if !description.is_empty() {
        tag_property(&mut out, "og:description", description);
    }
    if !url.is_empty() {
        tag_property(&mut out, "og:url", url);
    }
    if !site_name.is_empty() {
        tag_property(&mut out, "og:site_name", site_name);
    }
    if !image.is_empty() {
        tag_property(&mut out, "og:image", image);
        if !image_alt.is_empty() {
            tag_property(&mut out, "og:image:alt", image_alt);
        }
        if opts.image_width > 0 {
            tag_property(&mut out, "og:image:width", &opts.image_width.to_string());
        }
        if opts.image_height > 0 {
            tag_property(&mut out, "og:image:height", &opts.image_height.to_string());
        }
    }
    if !locale.is_empty() {
        tag_property(&mut out, "og:locale", locale);
    }
    if og_type == "article" && !author.is_empty() {
        tag_property(&mut out, "article:author", author);
    }
    if og_type == "profile" && !author.is_empty() {
        tag_property(&mut out, "profile:username", author);
    }

    // ---- Twitter / X --------------------------------------------------------
    if opts.include_twitter {
        out.push('\n');
        if opts.group_comments {
            out.push_str("<!-- Twitter Card — X -->\n");
        }
        tag_name(&mut out, "twitter:card", twitter_card);
        tag_name(&mut out, "twitter:title", title);
        if !description.is_empty() {
            tag_name(&mut out, "twitter:description", description);
        }
        if !image.is_empty() {
            tag_name(&mut out, "twitter:image", image);
            if !image_alt.is_empty() {
                tag_name(&mut out, "twitter:image:alt", image_alt);
            }
        }
        if !twitter_site.is_empty() {
            tag_name(&mut out, "twitter:site", &twitter_site);
        }
        if !twitter_creator.is_empty() {
            tag_name(&mut out, "twitter:creator", &twitter_creator);
        }
    }

    // ---- schema.org itemprop ------------------------------------------------
    if opts.include_schema {
        out.push('\n');
        if opts.group_comments {
            out.push_str("<!-- schema.org -->\n");
        }
        out.push_str(&format!(
            "<meta itemprop=\"name\" content=\"{}\">\n",
            esc(title)
        ));
        if !description.is_empty() {
            out.push_str(&format!(
                "<meta itemprop=\"description\" content=\"{}\">\n",
                esc(description)
            ));
        }
        if !image.is_empty() {
            out.push_str(&format!(
                "<meta itemprop=\"image\" content=\"{}\">\n",
                esc(image)
            ));
        }
    }

    // ---- Checks -------------------------------------------------------------
    if opts.warnings {
        let notes = check(
            opts,
            title,
            description,
            url,
            image,
            image_alt,
            site_name,
            twitter_card,
        );
        out.push_str("\n<!-- Checks\n");
        for n in &notes {
            out.push_str(&format!("     * {}\n", esc_comment(n)));
        }
        out.push_str("-->\n");
    }

    Ok(out.trim_end().to_string() + "\n")
}

/// Non-blocking advisory notes shown in the trailing `<!-- Checks -->` block.
#[allow(clippy::too_many_arguments)]
fn check(
    opts: &Options,
    title: &str,
    description: &str,
    url: &str,
    image: &str,
    image_alt: &str,
    site_name: &str,
    twitter_card: &str,
) -> Vec<String> {
    let mut notes = Vec::new();
    let title_len = title.chars().count();
    if title_len > TITLE_MAX {
        notes.push(format!(
            "Title is {title_len} characters; {TITLE_MAX} or fewer avoids truncation in most previews."
        ));
    }

    let desc_len = description.chars().count();
    if description.is_empty() {
        notes.push(
            "No description — og:description and the meta description were omitted. Most previews show a description line.".into(),
        );
    } else if desc_len < DESC_MIN {
        notes.push(format!(
            "Description is {desc_len} characters; {DESC_MIN}-{DESC_MAX} reads best in previews."
        ));
    } else if desc_len > DESC_MAX {
        notes.push(format!(
            "Description is {desc_len} characters; over {DESC_MAX} is usually truncated."
        ));
    }

    if url.is_empty() {
        notes.push("No url — og:url and the canonical link were omitted. Set the page's absolute URL so shares of tracking-parameter variants collapse onto one canonical page.".into());
    } else if !is_absolute_http(url) {
        notes.push(format!(
            "url \"{url}\" is not an absolute http(s) URL; crawlers do not resolve relative paths."
        ));
    }

    if image.is_empty() {
        if twitter_card == "summary_large_image" {
            notes.push("No image, but twitter_card is summary_large_image — X falls back to a plain text card. Add an image or use twitter_card=summary.".into());
        } else {
            notes.push("No image — previews will render without a thumbnail. 1200x630 pixels suits the 1.91:1 crop Facebook and LinkedIn use.".into());
        }
    } else {
        if !is_absolute_http(image) {
            notes.push(format!(
                "image \"{image}\" is not an absolute http(s) URL; crawlers do not resolve relative paths."
            ));
        }
        if image_alt.is_empty() {
            notes
                .push("No image_alt — og:image:alt describes the image for screen readers.".into());
        }
        if (opts.image_width > 0) != (opts.image_height > 0) {
            notes.push("Set both image_width and image_height, or neither — crawlers need the pair to reserve layout space.".into());
        }
    }

    if site_name.is_empty() {
        notes.push(
            "No site_name — og:site_name was omitted; it labels the source above the card.".into(),
        );
    }

    if notes.is_empty() {
        notes.push("No issues found.".into());
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> Options {
        let mut o = Options::new("How to bake sourdough");
        o.description =
            "A step-by-step sourdough guide covering starter, autolyse, bulk ferment and bake."
                .into();
        o.url = "https://example.com/sourdough".into();
        o.image = "https://example.com/og/sourdough.png".into();
        o.image_alt = "A sliced sourdough loaf".into();
        o.image_width = 1200;
        o.image_height = 630;
        o.site_name = "Example Bakery".into();
        o
    }

    #[test]
    fn happy_path_emits_every_section() {
        let out = generate(&full()).unwrap();
        assert!(out.contains("<title>How to bake sourdough</title>"));
        assert!(out.contains("<link rel=\"canonical\" href=\"https://example.com/sourdough\">"));
        assert!(out.contains("<meta property=\"og:type\" content=\"website\">"));
        assert!(out.contains("<meta property=\"og:title\" content=\"How to bake sourdough\">"));
        assert!(out.contains("<meta property=\"og:image:width\" content=\"1200\">"));
        assert!(out.contains("<meta property=\"og:image:height\" content=\"630\">"));
        assert!(out.contains("<meta property=\"og:locale\" content=\"en_US\">"));
        assert!(out.contains("<meta name=\"twitter:card\" content=\"summary_large_image\">"));
        assert!(
            out.contains("<meta name=\"twitter:image:alt\" content=\"A sliced sourdough loaf\">")
        );
        assert!(out.contains("* No issues found."));
        // schema.org is opt-in.
        assert!(!out.contains("itemprop"));
    }

    #[test]
    fn blank_title_is_an_error() {
        let o = Options::new("   ");
        let err = generate(&o).unwrap_err();
        assert!(err.contains("title is required"), "got: {err}");
    }

    #[test]
    fn unknown_og_type_is_an_error() {
        let mut o = full();
        o.og_type = "widget".into();
        let err = generate(&o).unwrap_err();
        assert!(err.contains("og_type must be one of"), "got: {err}");
        assert!(err.contains("widget"), "error names the bad value: {err}");
    }

    #[test]
    fn unknown_twitter_card_is_an_error() {
        let mut o = full();
        o.twitter_card = "big".into();
        assert!(generate(&o)
            .unwrap_err()
            .contains("twitter_card must be one of"));
    }

    #[test]
    fn oversized_image_dimension_is_an_error() {
        let mut o = full();
        o.image_width = MAX_IMAGE_DIMENSION + 1;
        let err = generate(&o).unwrap_err();
        assert!(
            err.contains("image_width must be between 0 and 10000"),
            "got: {err}"
        );
    }

    #[test]
    fn attribute_values_are_escaped() {
        let mut o = full();
        o.title = "Tom & Jerry's \"best\" <hits>".into();
        let out = generate(&o).unwrap();
        assert!(out.contains(
            "<meta property=\"og:title\" content=\"Tom &amp; Jerry&#39;s &quot;best&quot; &lt;hits&gt;\">"
        ));
        // The <title> element needs text escaping, not attribute escaping.
        assert!(out.contains("<title>Tom &amp; Jerry's \"best\" &lt;hits&gt;</title>"));
    }

    #[test]
    fn handles_are_normalized_to_at_form() {
        let mut o = full();
        o.twitter_site = "examplebakery".into();
        o.twitter_creator = "https://x.com/some_baker/".into();
        let out = generate(&o).unwrap();
        assert!(out.contains("<meta name=\"twitter:site\" content=\"@examplebakery\">"));
        assert!(out.contains("<meta name=\"twitter:creator\" content=\"@some_baker\">"));
    }

    #[test]
    fn article_type_adds_article_author() {
        let mut o = full();
        o.og_type = "article".into();
        o.author = "Dana Ruiz".into();
        let out = generate(&o).unwrap();
        assert!(out.contains("<meta property=\"article:author\" content=\"Dana Ruiz\">"));
        assert!(out.contains("<meta name=\"author\" content=\"Dana Ruiz\">"));
    }

    #[test]
    fn profile_type_adds_profile_username() {
        let mut o = full();
        o.og_type = "profile".into();
        o.author = "dana".into();
        assert!(generate(&o)
            .unwrap()
            .contains("<meta property=\"profile:username\" content=\"dana\">"));
    }

    #[test]
    fn section_toggles_drop_their_blocks() {
        let mut o = full();
        o.include_basic = false;
        o.include_twitter = false;
        o.include_schema = true;
        o.group_comments = false;
        o.warnings = false;
        let out = generate(&o).unwrap();
        assert!(!out.contains("<title>"));
        assert!(!out.contains("twitter:"));
        assert!(!out.contains("<!--"));
        assert!(out.contains("<meta itemprop=\"name\" content=\"How to bake sourdough\">"));
        assert!(out.contains(
            "<meta itemprop=\"image\" content=\"https://example.com/og/sourdough.png\">"
        ));
    }

    #[test]
    fn checks_flag_short_description_and_relative_image() {
        let mut o = full();
        o.description = "Too short.".into();
        o.image = "/og/sourdough.png".into();
        let out = generate(&o).unwrap();
        assert!(out.contains("Description is 10 characters"), "got: {out}");
        assert!(out.contains("is not an absolute http(s) URL"), "got: {out}");
    }

    #[test]
    fn checks_flag_large_image_card_without_an_image() {
        let mut o = full();
        o.image = String::new();
        o.image_alt = String::new();
        let out = generate(&o).unwrap();
        assert!(
            out.contains("twitter_card is summary_large_image"),
            "got: {out}"
        );
    }

    #[test]
    fn checks_flag_a_lone_image_dimension() {
        let mut o = full();
        o.image_height = 0;
        let out = generate(&o).unwrap();
        assert!(
            out.contains("Set both image_width and image_height"),
            "got: {out}"
        );
        // …and the lone dimension is still emitted, since it was explicitly asked for.
        assert!(out.contains("og:image:width"));
        assert!(!out.contains("og:image:height"));
    }

    #[test]
    fn comment_block_cannot_be_closed_early_by_a_value() {
        let mut o = full();
        o.url = "example.com/a--b".into();
        let out = generate(&o).unwrap();
        assert!(out.contains("example.com/a- -b"), "got: {out}");
        // Exactly one comment opener/closer pair for the checks block.
        assert_eq!(out.matches("<!-- Checks").count(), 1);
        assert_eq!(out.matches("-->").count(), 4); // 3 section headers + checks
    }

    #[test]
    fn empty_optional_fields_omit_their_tags() {
        let out = generate(&Options::new("Bare")).unwrap();
        assert!(out.contains("<meta property=\"og:title\" content=\"Bare\">"));
        // Assert the TAGS are gone — the checks comment names them on purpose.
        for tag in ["og:description", "og:url", "og:image", "og:site_name"] {
            assert!(
                !out.contains(&format!("<meta property=\"{tag}\"")),
                "{tag} should be omitted:\n{out}"
            );
        }
        assert!(!out.contains("<link rel=\"canonical\""));
        assert!(!out.contains("<meta name=\"description\""));
    }

    #[test]
    fn long_title_is_flagged_by_character_count_not_bytes() {
        let mut o = full();
        o.title = "é".repeat(61);
        let out = generate(&o).unwrap();
        assert!(out.contains("Title is 61 characters"), "got: {out}");
    }
}
