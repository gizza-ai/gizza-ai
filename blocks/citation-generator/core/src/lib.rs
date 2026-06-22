//! citation-generator core — format a single reference into APA, MLA, or
//! Chicago (notes-bibliography, bibliography entry) style from structured
//! fields. Pure Rust, no wafer/wasm-bindgen deps; shared by the chat skill
//! block and the web page.
//!
//! The input is a set of bibliographic fields (authors, title, year,
//! container/journal/site, publisher, volume/issue/pages, URL, DOI, accessed
//! date). The output is a ready-to-paste reference-list / works-cited /
//! bibliography entry in the chosen style.
//!
//! Scope: one source at a time, common source types (book, journal article,
//! web page). This is a deterministic string formatter — it does not look
//! anything up, fetch metadata, or guess missing fields.

/// Citation style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// APA 7th edition (reference-list entry).
    Apa,
    /// MLA 9th edition (works-cited entry).
    Mla,
    /// Chicago 17th (notes-bibliography bibliography entry).
    Chicago,
    /// Harvard (author-date reference-list entry).
    Harvard,
}

impl Style {
    fn parse(s: &str) -> Result<Style, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "apa" | "apa7" | "" => Ok(Style::Apa),
            "mla" | "mla9" => Ok(Style::Mla),
            "chicago" | "chicago-author-date" | "cmos" => Ok(Style::Chicago),
            "harvard" | "harvard-author-date" => Ok(Style::Harvard),
            other => Err(format!(
                "unknown style '{other}'; expected one of: apa, mla, chicago, harvard"
            )),
        }
    }
}

/// One parsed personal name: a family name and the remaining given name(s).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Name {
    family: String,
    given: String,
}

impl Name {
    /// Parse a single author token. Accepts either "Family, Given" (a comma
    /// splits family/given) or "Given Family" (last whitespace-token = family).
    /// A single token with no space/comma is treated as a family name only
    /// (e.g. a corporate/organization author).
    fn parse(raw: &str) -> Name {
        let raw = raw.trim();
        if let Some((fam, giv)) = raw.split_once(',') {
            return Name {
                family: fam.trim().to_string(),
                given: giv.trim().to_string(),
            };
        }
        match raw.rsplit_once(char::is_whitespace) {
            Some((giv, fam)) => Name {
                family: fam.trim().to_string(),
                given: giv.trim().to_string(),
            },
            None => Name {
                family: raw.to_string(),
                given: String::new(),
            },
        }
    }

    /// Whether this looks like a person (has a given name) vs. an organization
    /// (single token, no given name).
    fn is_person(&self) -> bool {
        !self.given.is_empty()
    }

    /// Initials of the given name(s): "John Quincy" -> "J. Q.". Hyphenated
    /// parts keep the hyphen: "Jean-Paul" -> "J.-P.".
    fn given_initials(&self) -> String {
        let mut out = String::new();
        for word in self.given.split_whitespace() {
            // Each whitespace word may contain hyphen-joined parts.
            let mut piece = String::new();
            for (i, part) in word.split('-').enumerate() {
                if let Some(c) = part.chars().next() {
                    if i > 0 {
                        piece.push('-');
                    }
                    piece.push(c.to_ascii_uppercase());
                    piece.push('.');
                }
            }
            if !piece.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&piece);
            }
        }
        out
    }
}

/// Parse the authors string into individual names. Authors are separated by
/// `;` (preferred, unambiguous) OR by ` and ` / `&`. Empty after trimming →
/// no authors.
fn parse_authors(raw: &str) -> Vec<Name> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    let parts: Vec<&str> = if raw.contains(';') {
        raw.split(';').collect()
    } else {
        // Split on " and " (word boundaries) and "&".
        raw.split(" and ").flat_map(|s| s.split('&')).collect()
    };
    parts
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(Name::parse)
        .collect()
}

/// Quote a title with the closing period INSIDE the quotes (MLA/Chicago style:
/// `"Title."`). If the title already ends with `!`/`?`, that mark is kept and
/// no period is added.
fn quoted_title(title: &str) -> String {
    let t = title.trim_end();
    if t.ends_with('!') || t.ends_with('?') {
        format!("\"{t}\"")
    } else {
        format!("\"{}.\"", t.trim_end_matches('.'))
    }
}

/// Ensure a sentence-ending period without doubling one.
fn with_period(s: &str) -> String {
    let s = s.trim_end();
    if s.is_empty() {
        return String::new();
    }
    if s.ends_with('.') || s.ends_with('!') || s.ends_with('?') {
        s.to_string()
    } else {
        format!("{s}.")
    }
}

/// Title-case a heading the way MLA/Chicago title-style does: capitalize the
/// first letter of each significant word; small function words stay lowercase
/// unless first/last.
fn title_case(s: &str) -> String {
    const SMALL: &[&str] = &[
        "a", "an", "and", "as", "at", "but", "by", "for", "if", "in", "nor", "of", "off", "on",
        "or", "per", "so", "the", "to", "up", "via", "vs", "yet",
    ];
    let words: Vec<&str> = s.split_whitespace().collect();
    let n = words.len();
    let mut out: Vec<String> = Vec::with_capacity(n);
    for (i, w) in words.iter().enumerate() {
        let lower = w.to_ascii_lowercase();
        let is_small = SMALL.contains(&lower.as_str());
        if i != 0 && i != n.saturating_sub(1) && is_small {
            out.push(lower);
        } else {
            out.push(capitalize_first(w));
        }
    }
    out.join(" ")
}

/// Uppercase the first alphabetic char, leave the rest as given.
fn capitalize_first(w: &str) -> String {
    let mut chars = w.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// Sentence-case a heading the way APA does: capitalize only the first word
/// (and anything after a colon), lowercase the rest — EXCEPT we leave any word
/// the user wrote with an internal capital alone (proper nouns / acronyms).
fn sentence_case(s: &str) -> String {
    let words: Vec<&str> = s.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut capitalize_next = true;
    for w in &words {
        if capitalize_next {
            out.push(capitalize_first(w));
        } else if has_internal_capital(w) {
            // Preserve proper nouns / acronyms the user capitalized.
            out.push(w.to_string());
        } else {
            out.push(w.to_ascii_lowercase());
        }
        capitalize_next = w.ends_with(':');
    }
    out.join(" ")
}

/// True if the word has an uppercase letter anywhere except a leading cap
/// (i.e. an acronym like "NASA" or a mid-word capital like "iPhone"/"YouTube").
fn has_internal_capital(w: &str) -> bool {
    let mut chars = w.chars();
    let _ = chars.next(); // skip the first char
    chars.any(|c| c.is_ascii_uppercase())
}

/// All the bibliographic fields. Blank fields are simply omitted from output.
#[derive(Debug, Clone, Default)]
pub struct Fields {
    pub authors: String,
    pub title: String,
    pub year: String,
    /// Journal name / book-collection / website name (the "container").
    pub container: String,
    pub publisher: String,
    pub volume: String,
    pub issue: String,
    pub pages: String,
    pub url: String,
    pub doi: String,
    /// Date the online source was accessed (free-form, e.g. "12 Mar. 2024").
    pub accessed: String,
}

/// Format `fields` into a citation in `style_str` (apa | mla | chicago).
/// Errors only on an unknown style or when there is nothing to cite (no title
/// AND no author).
pub fn cite(style_str: &str, fields: &Fields) -> Result<String, String> {
    let style = Style::parse(style_str)?;
    let authors = parse_authors(&fields.authors);
    let title = fields.title.trim();
    if title.is_empty() && authors.is_empty() {
        return Err("a citation needs at least a title or an author".into());
    }
    Ok(match style {
        Style::Apa => format_apa(&authors, fields),
        Style::Mla => format_mla(&authors, fields),
        Style::Chicago => format_chicago(&authors, fields),
        Style::Harvard => format_harvard(&authors, fields),
    })
}

// ----- APA 7 -----------------------------------------------------------------

/// APA author list: "Family, I. I., Family, I. I., & Family, I. I."
fn apa_authors(authors: &[Name]) -> String {
    let formatted: Vec<String> = authors
        .iter()
        .map(|n| {
            if n.is_person() {
                format!("{}, {}", n.family, n.given_initials())
            } else {
                n.family.clone()
            }
        })
        .collect();
    join_apa(&formatted)
}

/// Join with commas and an ampersand before the last (APA/Chicago bibliography).
fn join_apa(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{}, & {}", items[0], items[1]),
        _ => {
            let (last, head) = items.split_last().unwrap();
            format!("{}, & {}", head.join(", "), last)
        }
    }
}

fn format_apa(authors: &[Name], f: &Fields) -> String {
    let mut s = String::new();
    let author_str = apa_authors(authors);
    if !author_str.is_empty() {
        s.push_str(&with_period(&author_str));
        s.push(' ');
    }
    if !f.year.trim().is_empty() {
        s.push_str(&format!("({}). ", f.year.trim()));
    } else {
        s.push_str("(n.d.). ");
    }
    let title = sentence_case(f.title.trim());
    if !title.is_empty() {
        // In APA, an article title is plain; the container (journal) is italic.
        // We can't render italics in plain text, so the journal is left plain
        // and only structurally placed.
        s.push_str(&with_period(&title));
        s.push(' ');
    }
    let container = f.container.trim();
    if !container.is_empty() {
        s.push_str(&title_case(container));
        // volume(issue), pages
        let vol = f.volume.trim();
        let iss = f.issue.trim();
        if !vol.is_empty() {
            s.push_str(&format!(", {vol}"));
            if !iss.is_empty() {
                s.push_str(&format!("({iss})"));
            }
        }
        let pages = f.pages.trim();
        if !pages.is_empty() {
            s.push_str(&format!(", {pages}"));
        }
        s = with_period(&s);
        s.push(' ');
    } else if !f.publisher.trim().is_empty() {
        s.push_str(&with_period(f.publisher.trim()));
        s.push(' ');
    }
    let doi = f.doi.trim();
    let url = f.url.trim();
    if !doi.is_empty() {
        s.push_str(&format!("https://doi.org/{}", doi.trim_start_matches("https://doi.org/")));
    } else if !url.is_empty() {
        s.push_str(url);
    }
    s.trim().to_string()
}

// ----- MLA 9 -----------------------------------------------------------------

/// MLA author list: first author "Family, Given", subsequent "Given Family",
/// "and" before the last; 3+ authors → "First Author, et al."
fn mla_authors(authors: &[Name]) -> String {
    match authors.len() {
        0 => String::new(),
        1 => mla_first_name(&authors[0]),
        2 => format!(
            "{}, and {}",
            mla_first_name(&authors[0]),
            mla_rest_name(&authors[1])
        ),
        _ => format!("{}, et al", mla_first_name(&authors[0])),
    }
}

fn mla_first_name(n: &Name) -> String {
    if n.is_person() {
        format!("{}, {}", n.family, n.given)
    } else {
        n.family.clone()
    }
}

fn mla_rest_name(n: &Name) -> String {
    if n.is_person() {
        format!("{} {}", n.given, n.family)
    } else {
        n.family.clone()
    }
}

fn format_mla(authors: &[Name], f: &Fields) -> String {
    let mut s = String::new();
    let author_str = mla_authors(authors);
    if !author_str.is_empty() {
        s.push_str(&with_period(&author_str));
        s.push(' ');
    }
    let title = title_case(f.title.trim());
    if !title.is_empty() {
        // MLA: article/page title in quotes, standalone work (book) in italics.
        // Use quotes when there is a container (it's a part of a larger work),
        // otherwise present as a standalone title.
        if !f.container.trim().is_empty() {
            s.push_str(&quoted_title(&title));
        } else {
            s.push_str(&with_period(&title));
        }
        s.push(' ');
    }
    let container = f.container.trim();
    if !container.is_empty() {
        s.push_str(&title_case(container));
        let vol = f.volume.trim();
        let iss = f.issue.trim();
        if !vol.is_empty() {
            s.push_str(&format!(", vol. {vol}"));
        }
        if !iss.is_empty() {
            s.push_str(&format!(", no. {iss}"));
        }
        if !f.year.trim().is_empty() {
            s.push_str(&format!(", {}", f.year.trim()));
        }
        if !f.pages.trim().is_empty() {
            s.push_str(&format!(", pp. {}", f.pages.trim()));
        }
        s = with_period(&s);
        s.push(' ');
    } else {
        if !f.publisher.trim().is_empty() {
            s.push_str(&format!("{}, ", f.publisher.trim()));
        }
        if !f.year.trim().is_empty() {
            s.push_str(&with_period(f.year.trim()));
            s.push(' ');
        }
    }
    let url = f.url.trim();
    let doi = f.doi.trim();
    if !doi.is_empty() {
        s.push_str(&with_period(&format!(
            "https://doi.org/{}",
            doi.trim_start_matches("https://doi.org/")
        )));
        s.push(' ');
    } else if !url.is_empty() {
        s.push_str(&with_period(url));
        s.push(' ');
    }
    if !f.accessed.trim().is_empty() {
        s.push_str(&with_period(&format!("Accessed {}", f.accessed.trim())));
    }
    s.trim().to_string()
}

// ----- Chicago 17 (notes-bibliography, bibliography entry) -------------------

/// Chicago bibliography author list: first "Family, Given", subsequent
/// "Given Family", "and" before the last.
fn chicago_authors(authors: &[Name]) -> String {
    match authors.len() {
        0 => String::new(),
        1 => mla_first_name(&authors[0]),
        _ => {
            let first = mla_first_name(&authors[0]);
            let rest: Vec<String> = authors[1..].iter().map(mla_rest_name).collect();
            match rest.len() {
                1 => format!("{first}, and {}", rest[0]),
                _ => {
                    let (last, head) = rest.split_last().unwrap();
                    format!("{first}, {}, and {last}", head.join(", "))
                }
            }
        }
    }
}

fn format_chicago(authors: &[Name], f: &Fields) -> String {
    let mut s = String::new();
    let author_str = chicago_authors(authors);
    if !author_str.is_empty() {
        s.push_str(&with_period(&author_str));
        s.push(' ');
    }
    let title = title_case(f.title.trim());
    let container = f.container.trim();
    if !title.is_empty() {
        if !container.is_empty() {
            s.push_str(&quoted_title(&title));
        } else {
            s.push_str(&with_period(&title));
        }
        s.push(' ');
    }
    if !container.is_empty() {
        s.push_str(&title_case(container));
        let vol = f.volume.trim();
        let iss = f.issue.trim();
        if !vol.is_empty() {
            s.push_str(&format!(" {vol}"));
            if !iss.is_empty() {
                s.push_str(&format!(", no. {iss}"));
            }
        }
        if !f.year.trim().is_empty() {
            s.push_str(&format!(" ({})", f.year.trim()));
        }
        if !f.pages.trim().is_empty() {
            s.push_str(&format!(": {}", f.pages.trim()));
        }
        s = with_period(&s);
        s.push(' ');
    } else {
        let publisher = f.publisher.trim();
        let year = f.year.trim();
        if !publisher.is_empty() && !year.is_empty() {
            s.push_str(&with_period(&format!("{publisher}, {year}")));
            s.push(' ');
        } else if !publisher.is_empty() {
            s.push_str(&with_period(publisher));
            s.push(' ');
        } else if !year.is_empty() {
            s.push_str(&with_period(year));
            s.push(' ');
        }
    }
    let doi = f.doi.trim();
    let url = f.url.trim();
    if !doi.is_empty() {
        s.push_str(&with_period(&format!(
            "https://doi.org/{}",
            doi.trim_start_matches("https://doi.org/")
        )));
        s.push(' ');
    } else if !url.is_empty() {
        s.push_str(&with_period(url));
        s.push(' ');
    }
    s.trim().to_string()
}

// ----- Harvard (author-date, reference-list entry) ---------------------------

/// Harvard author list: "Family, I.I., Family, I.I. and Family, I.I." — like
/// APA initials but joined with "and" (no Oxford ampersand).
fn harvard_authors(authors: &[Name]) -> String {
    let formatted: Vec<String> = authors
        .iter()
        .map(|n| {
            if n.is_person() {
                // Harvard initials have no spaces between them: "J.Q."
                format!("{}, {}", n.family, n.given_initials().replace(' ', ""))
            } else {
                n.family.clone()
            }
        })
        .collect();
    match formatted.len() {
        0 => String::new(),
        1 => formatted[0].clone(),
        _ => {
            let (last, head) = formatted.split_last().unwrap();
            format!("{} and {last}", head.join(", "))
        }
    }
}

fn format_harvard(authors: &[Name], f: &Fields) -> String {
    let mut s = String::new();
    let author_str = harvard_authors(authors);
    if !author_str.is_empty() {
        s.push(' ');
        s.insert_str(0, &author_str);
        s = s.trim_end().to_string();
        s.push(' ');
    }
    let year = f.year.trim();
    if year.is_empty() {
        s.push_str("(no date) ");
    } else {
        s.push_str(&format!("({year}) "));
    }
    let container = f.container.trim();
    let title = title_case(f.title.trim());
    if !title.is_empty() {
        if !container.is_empty() {
            // Part of a larger work: single-quoted title.
            s.push_str(&format!("'{}', ", title.trim_end_matches('.')));
        } else {
            // Standalone work (book): title then period.
            s.push_str(&with_period(&title));
            s.push(' ');
        }
    }
    if !container.is_empty() {
        s.push_str(&title_case(container));
        let vol = f.volume.trim();
        let iss = f.issue.trim();
        if !vol.is_empty() {
            s.push_str(&format!(", {vol}"));
            if !iss.is_empty() {
                s.push_str(&format!("({iss})"));
            }
        }
        if !f.pages.trim().is_empty() {
            s.push_str(&format!(", pp. {}", f.pages.trim()));
        }
        s = with_period(&s);
        s.push(' ');
    } else if !f.publisher.trim().is_empty() {
        s.push_str(&with_period(f.publisher.trim()));
        s.push(' ');
    }
    let doi = f.doi.trim();
    let url = f.url.trim();
    if !doi.is_empty() {
        s.push_str(&with_period(&format!(
            "Available at: https://doi.org/{}",
            doi.trim_start_matches("https://doi.org/")
        )));
        s.push(' ');
    } else if !url.is_empty() {
        s.push_str(&format!("Available at: {url} "));
        if !f.accessed.trim().is_empty() {
            s.push_str(&with_period(&format!("(Accessed: {})", f.accessed.trim())));
        } else {
            s = with_period(s.trim_end());
        }
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> Fields {
        Fields {
            authors: "Doe, Jane".into(),
            title: "A Study of Things".into(),
            year: "2020".into(),
            publisher: "Penguin".into(),
            ..Default::default()
        }
    }

    fn article() -> Fields {
        Fields {
            authors: "Doe, Jane; Smith, John".into(),
            title: "On the nature of things".into(),
            year: "2021".into(),
            container: "Journal of Studies".into(),
            volume: "12".into(),
            issue: "3".into(),
            pages: "45-67".into(),
            doi: "10.1000/xyz".into(),
            ..Default::default()
        }
    }

    #[test]
    fn apa_book() {
        let out = cite("apa", &book()).unwrap();
        assert_eq!(out, "Doe, J. (2020). A study of things. Penguin.");
    }

    #[test]
    fn apa_article_with_doi() {
        let out = cite("apa", &article()).unwrap();
        assert_eq!(
            out,
            "Doe, J., & Smith, J. (2021). On the nature of things. Journal of Studies, 12(3), 45-67. https://doi.org/10.1000/xyz"
        );
    }

    #[test]
    fn mla_article() {
        let out = cite("mla", &article()).unwrap();
        assert_eq!(
            out,
            "Doe, Jane, and John Smith. \"On the Nature of Things.\" Journal of Studies, vol. 12, no. 3, 2021, pp. 45-67. https://doi.org/10.1000/xyz."
        );
    }

    #[test]
    fn mla_three_authors_etal() {
        let mut f = article();
        f.authors = "Doe, Jane; Smith, John; Roe, Amy".into();
        let out = cite("mla", &f).unwrap();
        assert!(out.starts_with("Doe, Jane, et al."), "got: {out}");
    }

    #[test]
    fn chicago_article() {
        let out = cite("chicago", &article()).unwrap();
        assert_eq!(
            out,
            "Doe, Jane, and John Smith. \"On the Nature of Things.\" Journal of Studies 12, no. 3 (2021): 45-67. https://doi.org/10.1000/xyz."
        );
    }

    #[test]
    fn mla_webpage_accessed() {
        let f = Fields {
            authors: "".into(),
            title: "How to cite".into(),
            container: "Example Site".into(),
            year: "2023".into(),
            url: "https://example.com/cite".into(),
            accessed: "12 Mar. 2024".into(),
            ..Default::default()
        };
        let out = cite("mla", &f).unwrap();
        assert_eq!(
            out,
            "\"How to Cite.\" Example Site, 2023. https://example.com/cite. Accessed 12 Mar. 2024."
        );
    }

    #[test]
    fn apa_no_year_is_nd() {
        let mut f = book();
        f.year = "".into();
        let out = cite("apa", &f).unwrap();
        assert!(out.contains("(n.d.)"), "got: {out}");
    }

    #[test]
    fn given_initials_handles_hyphen() {
        let n = Name::parse("Curie, Marie-Sklodowska");
        assert_eq!(n.given_initials(), "M.-S.");
    }

    #[test]
    fn name_parse_given_family() {
        let n = Name::parse("John Smith");
        assert_eq!(n.family, "Smith");
        assert_eq!(n.given, "John");
    }

    #[test]
    fn org_author_no_given() {
        let n = Name::parse("UNESCO");
        assert!(!n.is_person());
        assert_eq!(n.family, "UNESCO");
    }

    #[test]
    fn harvard_article() {
        let out = cite("harvard", &article()).unwrap();
        assert_eq!(
            out,
            "Doe, J. and Smith, J. (2021) 'On the Nature of Things', Journal of Studies, 12(3), pp. 45-67. Available at: https://doi.org/10.1000/xyz."
        );
    }

    #[test]
    fn harvard_book() {
        let out = cite("harvard", &book()).unwrap();
        assert_eq!(out, "Doe, J. (2020) A Study of Things. Penguin.");
    }

    #[test]
    fn unknown_style_errors() {
        assert!(cite("turabian", &book()).is_err());
    }

    #[test]
    fn empty_title_and_author_errors() {
        let f = Fields {
            year: "2020".into(),
            ..Default::default()
        };
        assert!(cite("apa", &f).is_err());
    }

    #[test]
    fn sentence_case_preserves_acronym() {
        assert_eq!(sentence_case("the role of NASA in space"), "The role of NASA in space");
    }

    #[test]
    fn doi_url_not_doubled() {
        let mut f = article();
        f.doi = "https://doi.org/10.1/abc".into();
        let out = cite("apa", &f).unwrap();
        assert!(out.ends_with("https://doi.org/10.1/abc"), "got: {out}");
        assert!(!out.contains("doi.org/https"), "got: {out}");
    }
}
