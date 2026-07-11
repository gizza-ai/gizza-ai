//! email-reply-cleaner core — pull the fresh message out of a replied or
//! forwarded plain-text email. No wafer/wasm-bindgen deps; shared by the chat
//! skill block and the web page.
//!
//! Four independent, deterministic heuristics (each toggleable):
//!   * `remove_quotes`      — drop lines that begin with the `>` quote prefix
//!                            (including nested `>>`), leading whitespace allowed.
//!   * `remove_reply_chain` — truncate the body at the first attribution /
//!                            forwarded / header-block boundary ("On … wrote:",
//!                            "-----Original Message-----", "---------- Forwarded
//!                            message ----------", "Begin forwarded message:", or
//!                            an Outlook `From:` header line) and everything below.
//!   * `remove_signature`   — truncate at the RFC 3676 `-- ` signature delimiter
//!                            or a common mobile/app footer ("Sent from my iPhone",
//!                            "Get Outlook for …", "Sent from Mail for …").
//!   * `collapse_blank_lines` — collapse runs of blank lines to one and trim
//!                            leading/trailing blank lines.
//!
//! English-focused: the attribution detector keys on "On … wrote:". The
//! structural markers (`>`, `-- `, `From:`, forwarded rules) are
//! language-neutral. Multi-line wrapped attribution headers are not detected.

/// Options controlling which cleaning passes run. All default to `true`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Remove lines beginning with the `>` quote marker (nested `>>` too).
    pub remove_quotes: bool,
    /// Cut the quoted reply chain: the attribution / forwarded / header-block
    /// boundary line and everything after it.
    pub remove_reply_chain: bool,
    /// Cut the signature block: the `-- ` delimiter or a mobile/app footer line
    /// and everything after it.
    pub remove_signature: bool,
    /// Collapse runs of blank lines to a single blank line and trim the edges.
    pub collapse_blank_lines: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            remove_quotes: true,
            remove_reply_chain: true,
            remove_signature: true,
            collapse_blank_lines: true,
        }
    }
}

/// A line begins the quoted reply chain: an `On … wrote:` attribution, an
/// Outlook `-----Original Message-----` marker, a `---------- Forwarded message
/// ----------` / `Begin forwarded message:` rule, or an Outlook `From:` header
/// line (`From: Name <addr@host>`).
fn is_reply_boundary(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    // "On <date/name …> wrote:" — the dominant English attribution.
    if t.starts_with("On ") && t.ends_with("wrote:") {
        return true;
    }
    let low = t.to_ascii_lowercase();
    // Apple Mail forwarded header (no dashes).
    if low.starts_with("begin forwarded message") {
        return true;
    }
    // Outlook / client rules: a dashed line naming the original or forwarded msg.
    if (low.contains("original message") || low.contains("forwarded message")) && t.contains("--")
    {
        return true;
    }
    // Outlook plain-text header block starts with a `From:` line that carries an
    // address (`@` or `<`), which avoids matching prose like "From: the desk of".
    if low.starts_with("from:") && (t.contains('@') || t.contains('<')) {
        return true;
    }
    false
}

/// A line begins the signature block: the RFC 3676 `-- ` delimiter (clients may
/// strip the trailing space to `--`) or a common mobile/app footer.
fn is_signature_boundary(line: &str) -> bool {
    let t = line.trim();
    if t == "--" {
        return true;
    }
    let low = t.to_ascii_lowercase();
    low.starts_with("sent from my ")
        || low.starts_with("sent from mail for")
        || low.starts_with("get outlook for ")
}

/// A quoted line: the first non-whitespace character is `>`.
fn is_quote_line(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

/// Clean an email body with the four passes toggled individually — the shared
/// entry point for the chat skill block, the CLI, and the web page. Errors only
/// when `text` is empty/whitespace so callers get a clear message; all four
/// heuristics are otherwise infallible.
pub fn run(
    text: &str,
    remove_quotes: bool,
    remove_reply_chain: bool,
    remove_signature: bool,
    collapse_blank_lines: bool,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("provide the email text to clean".to_string());
    }
    Ok(clean(
        text,
        Options {
            remove_quotes,
            remove_reply_chain,
            remove_signature,
            collapse_blank_lines,
        },
    ))
}

/// Extract the fresh message from `text` per `opts`. Line endings are normalised
/// to `\n`; output uses `\n` and never has a trailing newline.
pub fn clean(text: &str, opts: Options) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    // 1. Find the earliest truncation boundary among the enabled cutters. Every
    //    boundary cuts to the end of the message, so the earliest one wins.
    let mut cut = lines.len();
    for (i, line) in lines.iter().enumerate() {
        let boundary = (opts.remove_reply_chain && is_reply_boundary(line))
            || (opts.remove_signature && is_signature_boundary(line));
        if boundary {
            cut = i;
            break;
        }
    }
    let kept = &lines[..cut];

    // 2. Drop quoted lines.
    let mut out: Vec<&str> = Vec::with_capacity(kept.len());
    for &line in kept {
        if opts.remove_quotes && is_quote_line(line) {
            continue;
        }
        out.push(line);
    }

    // 3. Collapse blank runs + trim leading/trailing blank lines.
    if opts.collapse_blank_lines {
        let mut collapsed: Vec<&str> = Vec::with_capacity(out.len());
        let mut prev_blank = false;
        for &line in &out {
            let blank = line.trim().is_empty();
            if blank && prev_blank {
                continue;
            }
            collapsed.push(line);
            prev_blank = blank;
        }
        while collapsed.first().is_some_and(|l| l.trim().is_empty()) {
            collapsed.remove(0);
        }
        while collapsed.last().is_some_and(|l| l.trim().is_empty()) {
            collapsed.pop();
        }
        return collapsed.join("\n");
    }

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_signature_and_reply_chain_default() {
        let input = "Hi Bob,\n\nThanks, that works for me. See you then.\n\nBest,\nAlice\n\n-- \nAlice Smith\nAcme Corp\n\nOn Mon, Jan 1, 2024 at 10:00 AM Bob <bob@example.com> wrote:\n> Are you free on Friday?\n> Let me know.\n";
        let out = clean(input, Options::default());
        assert_eq!(
            out,
            "Hi Bob,\n\nThanks, that works for me. See you then.\n\nBest,\nAlice"
        );
    }

    #[test]
    fn removes_quote_lines_only() {
        let opts = Options {
            remove_quotes: true,
            remove_reply_chain: false,
            remove_signature: false,
            collapse_blank_lines: false,
        };
        assert_eq!(clean("Hi\n> old stuff\nBye", opts), "Hi\nBye");
        // nested quotes and leading whitespace before '>' are caught too.
        assert_eq!(clean("keep\n  >> deep\n>x", opts), "keep");
    }

    #[test]
    fn cuts_at_on_wrote_attribution() {
        let input =
            "My reply here.\n\nOn Tue, 2 Jan 2024, Jane wrote:\n> earlier\nnot reached either";
        let opts = Options::default();
        assert_eq!(clean(input, opts), "My reply here.");
    }

    #[test]
    fn cuts_outlook_original_message_and_headers() {
        let input = "See below.\n\n-----Original Message-----\nFrom: X <x@y.com>\nSubject: Re: hi\n> quoted";
        assert_eq!(clean(input, Options::default()), "See below.");
        // A bare From: header (no preceding dashed rule) is also a boundary.
        let input2 = "Answer.\nFrom: Jane Doe <jane@example.com>\nSent: Monday\n> q";
        assert_eq!(clean(input2, Options::default()), "Answer.");
    }

    #[test]
    fn cuts_forwarded_message() {
        let input = "FYI below.\n\n---------- Forwarded message ----------\nFrom: A <a@b.com>\nHello world";
        assert_eq!(clean(input, Options::default()), "FYI below.");
    }

    #[test]
    fn cuts_mobile_footer() {
        let input = "Sounds good.\n\nSent from my iPhone";
        assert_eq!(clean(input, Options::default()), "Sounds good.");
        let input2 = "OK.\nGet Outlook for iOS";
        assert_eq!(clean(input2, Options::default()), "OK.");
    }

    #[test]
    fn signature_dash_delimiter_variants() {
        // "--" with or without trailing space both delimit the signature.
        assert_eq!(clean("Body\n--\nName", Options::default()), "Body");
        assert_eq!(clean("Body\n-- \nName", Options::default()), "Body");
    }

    #[test]
    fn collapse_blank_lines_folds_and_trims() {
        let opts = Options {
            remove_quotes: false,
            remove_reply_chain: false,
            remove_signature: false,
            collapse_blank_lines: true,
        };
        assert_eq!(clean("\n\na\n\n\n\nb\n\n", opts), "a\n\nb");
    }

    #[test]
    fn from_prose_is_not_a_boundary() {
        // "From:" without an address is prose, not a header — keep it.
        let input = "A note From: the desk of the CEO\nregards";
        assert_eq!(
            clean(input, Options::default()),
            "A note From: the desk of the CEO\nregards"
        );
    }

    #[test]
    fn everything_disabled_only_normalizes_newlines() {
        let opts = Options {
            remove_quotes: false,
            remove_reply_chain: false,
            remove_signature: false,
            collapse_blank_lines: false,
        };
        assert_eq!(clean("a\r\n> b\r\n", opts), "a\n> b\n");
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(clean("", Options::default()), "");
        assert_eq!(clean("   \n\n", Options::default()), "");
    }

    #[test]
    fn run_defaults_match_clean_and_reject_empty() {
        let input = "Hi Bob,\n\nThanks!\n\nOn Mon Bob wrote:\n> old";
        assert_eq!(run(input, true, true, true, true).unwrap(), "Hi Bob,\n\nThanks!");
        // Toggling a pass off changes the result.
        assert_eq!(
            run("Hi\n> old\nBye", false, true, true, true).unwrap(),
            "Hi\n> old\nBye"
        );
        // Empty / whitespace-only input is a clear error, not empty output.
        assert!(run("", true, true, true, true).is_err());
        assert!(run("   \n\n", true, true, true, true).is_err());
    }
}
