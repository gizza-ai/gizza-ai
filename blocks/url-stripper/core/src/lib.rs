//! url-stripper core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Removes URLs (http/https/ftp, and
//! optionally scheme-less `www.` links) and optionally email addresses from a
//! block of text, replaces each with a chosen token (empty = delete), then
//! optionally tidies the whitespace left behind so the result reads as clean
//! prose. Pure-Rust (`regex` + `serde`).

use regex::Regex;
use serde::Serialize;

/// What was stripped and the cleaned text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stripped {
    /// The cleaned text.
    pub text: String,
    /// Number of URLs removed (http/https/ftp plus any `www.` links).
    pub urls_removed: usize,
    /// Number of email addresses removed (0 unless `remove_emails`).
    pub emails_removed: usize,
}

/// Stripping options (all defaults are the page/CLI defaults too).
#[derive(Debug, Clone)]
pub struct Options {
    /// Also remove bare email addresses (`name@example.com`).
    pub remove_emails: bool,
    /// Also remove scheme-less links that begin with `www.`.
    pub remove_www: bool,
    /// Text to put in place of each removed URL/email. Empty string = delete.
    pub replacement: String,
    /// Collapse the extra spaces left behind and trim each line so the result
    /// reads as clean prose.
    pub collapse_whitespace: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            remove_emails: false,
            remove_www: true,
            replacement: String::new(),
            collapse_whitespace: true,
        }
    }
}

thread_local! {
    // Scheme-based URLs: http/https/ftp. The char class excludes whitespace,
    // angle/round/square/curly brackets and quotes so a bracketed or quoted URL
    // stops at the delimiter (leaving the bracket in place).
    static SCHEME_RE: Regex =
        Regex::new(r#"(?i)\b(?:https?|ftp)://[^\s<>"'`()\[\]{}]+"#).unwrap();
    // Scheme-less links that begin with www.
    static WWW_RE: Regex =
        Regex::new(r#"(?i)\bwww\.[^\s<>"'`()\[\]{}]+"#).unwrap();
    // Bare email addresses.
    static EMAIL_RE: Regex =
        Regex::new(r#"(?i)\b[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}\b"#).unwrap();
    // Runs of horizontal whitespace (spaces/tabs), never newlines.
    static HSPACE_RE: Regex = Regex::new(r"[ \t]+").unwrap();
    // A single space sitting before closing punctuation.
    static SPACE_BEFORE_PUNCT: Regex = Regex::new(r#" ([,.;:!?)\]}])"#).unwrap();
    // A single space sitting just after an opening bracket.
    static SPACE_AFTER_OPEN: Regex = Regex::new(r#"([(\[]) "#).unwrap();
    // Brackets left empty once their inner URL was deleted.
    static EMPTY_BRACKETS: Regex = Regex::new(r#"\(\)|\[\]|<>"#).unwrap();
}

/// Trailing prose punctuation commonly stuck to a URL — trimmed off the match so
/// the sentence's own period/comma survives when the URL is removed.
fn trim_trailing(u: &str) -> &str {
    u.trim_end_matches(|c| matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '\u{2019}'))
}

/// Replace every (trailing-punctuation-trimmed) match of `re` in `text` with
/// `replacement`, returning the new string and the number of replacements.
fn replace_matches(text: &str, re: &Regex, replacement: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    let mut count = 0usize;
    for m in re.find_iter(text) {
        let trimmed = trim_trailing(m.as_str());
        if trimmed.is_empty() {
            continue;
        }
        let end = m.start() + trimmed.len();
        out.push_str(&text[last..m.start()]);
        out.push_str(replacement);
        last = end;
        count += 1;
    }
    out.push_str(&text[last..]);
    (out, count)
}

/// Tidy the whitespace left after removing URLs: collapse horizontal-space runs,
/// drop a space before closing punctuation / after an opening bracket, remove
/// now-empty brackets, and trim each line. Blank lines and newlines are kept.
fn clean_whitespace(s: &str) -> String {
    s.split('\n')
        .map(|line| {
            let a = HSPACE_RE.with(|re| re.replace_all(line, " ").into_owned());
            let b = SPACE_AFTER_OPEN.with(|re| re.replace_all(&a, "$1").into_owned());
            let c = EMPTY_BRACKETS.with(|re| re.replace_all(&b, "").into_owned());
            // Removing empty brackets can leave a fresh double space; collapse once more.
            let d = HSPACE_RE.with(|re| re.replace_all(&c, " ").into_owned());
            let e = SPACE_BEFORE_PUNCT.with(|re| re.replace_all(&d, "$1").into_owned());
            e.trim().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip URLs (and optionally emails) from `text` per `opts`.
pub fn strip(text: &str, opts: &Options) -> Stripped {
    let (t1, scheme_n) = SCHEME_RE.with(|re| replace_matches(text, re, &opts.replacement));
    let (t2, www_n) = if opts.remove_www {
        WWW_RE.with(|re| replace_matches(&t1, re, &opts.replacement))
    } else {
        (t1, 0)
    };
    let (t3, email_n) = if opts.remove_emails {
        EMAIL_RE.with(|re| replace_matches(&t2, re, &opts.replacement))
    } else {
        (t2, 0)
    };
    let text = if opts.collapse_whitespace {
        clean_whitespace(&t3)
    } else {
        t3
    };
    Stripped {
        text,
        urls_removed: scheme_n + www_n,
        emails_removed: email_n,
    }
}

/// Human-readable rendering (used by the page): just the cleaned text, so it is
/// copy-paste clean. Counts live in the chat/CLI JSON.
pub fn render(text: &str, opts: &Options) -> Result<String, String> {
    Ok(strip(text, opts).text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn removes_url_and_cleans_spacing() {
        let r = strip("Visit https://example.com for more details", &opts());
        assert_eq!(r.text, "Visit for more details");
        assert_eq!(r.urls_removed, 1);
        assert_eq!(r.emails_removed, 0);
    }

    #[test]
    fn keeps_sentence_period() {
        let r = strip("See https://example.com/page.", &opts());
        assert_eq!(r.text, "See.");
        assert_eq!(r.urls_removed, 1);
    }

    #[test]
    fn strips_bracketed_url_and_removes_empty_brackets() {
        let r = strip("Source (https://a.com/x) here", &opts());
        assert_eq!(r.text, "Source here");
        assert_eq!(r.urls_removed, 1);
    }

    #[test]
    fn removes_www_when_enabled() {
        let r = strip("Go to www.example.com now", &opts());
        assert_eq!(r.text, "Go to now");
        assert_eq!(r.urls_removed, 1);
    }

    #[test]
    fn keeps_www_when_disabled() {
        let o = Options {
            remove_www: false,
            ..opts()
        };
        let r = strip("Go to www.example.com now", &o);
        assert_eq!(r.text, "Go to www.example.com now");
        assert_eq!(r.urls_removed, 0);
    }

    #[test]
    fn removes_ftp() {
        let r = strip(
            "ftp://files.example.org/pub/file.zip is the mirror",
            &opts(),
        );
        assert_eq!(r.text, "is the mirror");
        assert_eq!(r.urls_removed, 1);
    }

    #[test]
    fn emails_only_when_enabled() {
        let off = strip("mail me at bob@example.com ok", &opts());
        assert_eq!(off.text, "mail me at bob@example.com ok");
        assert_eq!(off.emails_removed, 0);

        let o = Options {
            remove_emails: true,
            ..opts()
        };
        let on = strip("mail me at bob@example.com ok", &o);
        assert_eq!(on.text, "mail me at ok");
        assert_eq!(on.emails_removed, 1);
    }

    #[test]
    fn custom_replacement_token() {
        let o = Options {
            replacement: "[link]".to_string(),
            ..opts()
        };
        let r = strip("Read https://a.com/x and https://b.com/y today", &o);
        assert_eq!(r.text, "Read [link] and [link] today");
        assert_eq!(r.urls_removed, 2);
    }

    #[test]
    fn preserves_newlines_and_blank_lines() {
        let r = strip("line one https://a.com\n\nline two", &opts());
        assert_eq!(r.text, "line one\n\nline two");
    }

    #[test]
    fn no_collapse_leaves_raw_gaps() {
        let o = Options {
            collapse_whitespace: false,
            ..opts()
        };
        let r = strip("a https://a.com b", &o);
        assert_eq!(r.text, "a  b");
    }

    #[test]
    fn dedup_not_applied_counts_each() {
        let r = strip("https://a.com https://a.com", &opts());
        assert_eq!(r.urls_removed, 2);
        assert_eq!(r.text, "");
    }

    #[test]
    fn render_returns_clean_text() {
        assert_eq!(render("x https://a.com y", &opts()).unwrap(), "x y");
    }
}
