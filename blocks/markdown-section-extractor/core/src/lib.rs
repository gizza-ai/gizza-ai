//! markdown-section-extractor core — pull a single section out of a Markdown
//! document by its heading. Pure compute, no wafer/wasm-bindgen deps; shared by
//! the chat skill block and the web page.
//!
//! A "section" is the heading line plus the body that follows it, up to (but not
//! including) the next heading **at the same or a shallower level** (a smaller or
//! equal `#` count). By default nested deeper subsections are kept; set
//! `include_subsections = false` to stop at the first deeper heading and return
//! only the prose directly under the target.

/// How to match the requested heading text against a document heading.
/// All matching ignores leading/trailing whitespace on the heading text.
#[derive(Clone, Copy)]
enum Match {
    /// Case-insensitive exact match of the trimmed heading text (default).
    Exact,
    /// Case-sensitive exact match of the trimmed heading text.
    ExactCase,
    /// Case-insensitive substring match.
    Contains,
}

impl Match {
    fn parse(mode: &str) -> Result<Self, String> {
        match mode {
            "" | "exact" => Ok(Match::Exact),
            "exact_case" => Ok(Match::ExactCase),
            "contains" => Ok(Match::Contains),
            other => Err(format!(
                "invalid match {other:?}: expected \"exact\", \"exact_case\", or \"contains\""
            )),
        }
    }

    fn matches(self, heading: &str, query: &str) -> bool {
        let h = heading.trim();
        let q = query.trim();
        match self {
            Match::Exact => h.eq_ignore_ascii_case(q),
            Match::ExactCase => h == q,
            Match::Contains => h.to_lowercase().contains(&q.to_lowercase()),
        }
    }
}

/// A heading found in the document.
struct Heading {
    /// Index into the `lines` vector where the heading line lives.
    line: usize,
    /// Heading depth 1..=6 (ATX) or 1/2 (setext).
    level: u8,
    /// The heading's display text (with inline markup left intact, trimmed).
    text: String,
    /// For a setext heading, the underline occupies `line + 1`; this is the
    /// last line index the heading itself spans. For ATX it's `line`.
    last_line: usize,
}

/// Strip a trailing ATX-style closing run of `#` (e.g. `## Title ##` → `Title`).
fn strip_atx_closer(s: &str) -> &str {
    let t = s.trim_end();
    let trimmed = t.trim_end_matches('#');
    // A closing sequence must be preceded by a space (or be the whole thing);
    // otherwise the `#`s are part of the text (e.g. `C#`).
    if trimmed.len() == t.len() {
        t
    } else if trimmed.is_empty() || trimmed.ends_with(' ') || trimmed.ends_with('\t') {
        trimmed.trim_end()
    } else {
        t
    }
}

/// Count the leading spaces of `line`, treating a tab as advancing to the next
/// 4-column stop. Returns the column count (used to reject code-block-indented
/// `#` lines, which are not headings).
fn leading_indent(line: &str) -> usize {
    let mut col = 0usize;
    for c in line.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += 4 - (col % 4),
            _ => break,
        }
    }
    col
}

/// Parse an ATX heading from a single line. Returns `(level, text)` if the line
/// is a heading (1–6 leading `#`s followed by a space or end of line, with ≤3
/// leading spaces of indentation), else `None`.
fn parse_atx(line: &str) -> Option<(u8, String)> {
    if leading_indent(line) >= 4 {
        return None; // 4+ spaces → indented code block, not a heading.
    }
    let s = line.trim_start();
    let hashes = s.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &s[hashes..];
    // The `#` run must be followed by a space/tab or be the entire line.
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let text = strip_atx_closer(rest.trim()).trim().to_string();
    Some((hashes as u8, text))
}

/// Is `line` a setext underline (`===…` → level 1, `---…` → level 2)? The
/// previous line must be non-blank paragraph text for it to count, which the
/// caller checks. Returns the level.
fn setext_level(line: &str) -> Option<u8> {
    if leading_indent(line) >= 4 {
        return None;
    }
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c == '=') {
        Some(1)
    } else if t.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

/// Collect every heading in `lines`, skipping fenced code blocks (``` / ~~~ ).
fn collect_headings(lines: &[&str]) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut fence: Option<(char, usize)> = None; // (fence char, run length)
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        // Toggle fenced-code-block state on a ``` or ~~~ fence line.
        if let Some((fc, flen)) = fence {
            if leading_indent(line) < 4 {
                let run = trimmed.chars().take_while(|&c| c == fc).count();
                let after = trimmed[run..].trim();
                if run >= flen && after.is_empty() {
                    fence = None;
                }
            }
            i += 1;
            continue;
        } else if leading_indent(line) < 4 {
            let first = trimmed.chars().next();
            if matches!(first, Some('`') | Some('~')) {
                let fc = first.unwrap();
                let run = trimmed.chars().take_while(|&c| c == fc).count();
                if run >= 3 {
                    fence = Some((fc, run));
                    i += 1;
                    continue;
                }
            }
        }

        // ATX heading.
        if let Some((level, text)) = parse_atx(line) {
            out.push(Heading { line: i, level, text, last_line: i });
            i += 1;
            continue;
        }

        // Setext heading: a paragraph line followed by an `===`/`---` underline.
        if i + 1 < lines.len() {
            if let Some(level) = setext_level(lines[i + 1]) {
                let candidate = line.trim();
                // The text line must be non-blank and not itself an ATX heading
                // or a blank line; a `---` after a blank line is a thematic break.
                if !candidate.is_empty() && parse_atx(line).is_none() {
                    out.push(Heading {
                        line: i,
                        level,
                        text: candidate.to_string(),
                        last_line: i + 1,
                    });
                    i += 2;
                    continue;
                }
            }
        }

        i += 1;
    }
    out
}

/// Extract the section of `markdown` under the heading matching `heading`.
///
/// - `heading`: the heading text to find (required, non-blank after trimming).
/// - `match_mode` (`"exact"` | `"exact_case"` | `"contains"`, blank → `"exact"`):
///   how the heading text is compared. `exact` is case-insensitive exact match.
/// - `include_subsections` (default `true`): when `true` the returned section
///   includes every deeper-nested subsection (until the next same-or-shallower
///   heading); when `false` it stops at the first heading of any level after the
///   target, returning only the body directly beneath it.
/// - `include_heading` (default `true`): when `true` the matched heading line is
///   the first line of the output; when `false` only the body is returned.
///
/// Returns `Err` if the heading text is blank, the match mode is invalid, or no
/// heading matches.
pub fn extract(
    markdown: &str,
    heading: &str,
    match_mode: &str,
    include_subsections: bool,
    include_heading: bool,
) -> Result<String, String> {
    if heading.trim().is_empty() {
        return Err("heading is required: provide the heading text to extract".into());
    }
    let matcher = Match::parse(match_mode)?;
    let lines: Vec<&str> = markdown.split('\n').collect();
    let headings = collect_headings(&lines);

    // Find the first heading whose text matches.
    let idx = headings
        .iter()
        .position(|h| matcher.matches(&h.text, heading))
        .ok_or_else(|| {
            format!(
                "no heading matching {:?} found ({} heading{} in the document{})",
                heading.trim(),
                headings.len(),
                if headings.len() == 1 { "" } else { "s" },
                if headings.is_empty() {
                    String::new()
                } else {
                    let names: Vec<String> = headings
                        .iter()
                        .take(10)
                        .map(|h| format!("{:?}", h.text))
                        .collect();
                    format!(": {}", names.join(", "))
                }
            )
        })?;

    let target = &headings[idx];
    // The body starts on the line after the heading (after the setext underline).
    let body_start = target.last_line + 1;

    // Find where the section ends.
    let end_line = if include_subsections {
        // Stop at the next heading at the same or a shallower level.
        headings[idx + 1..]
            .iter()
            .find(|h| h.level <= target.level)
            .map(|h| h.line)
            .unwrap_or(lines.len())
    } else {
        // Stop at the very next heading of ANY level.
        headings
            .get(idx + 1)
            .map(|h| h.line)
            .unwrap_or(lines.len())
    };

    let start = if include_heading { target.line } else { body_start };
    // Guard: if excluding the heading and the body is empty, `start` may exceed
    // `end_line`; clamp so the slice is valid.
    let start = start.min(end_line);
    let section = lines[start..end_line].join("\n");

    // Trim a leading/trailing run of blank lines so the section is tidy, but
    // preserve interior blank lines (paragraph breaks).
    Ok(trim_blank_edges(&section))
}

/// Remove leading and trailing blank (whitespace-only) lines, keeping interior
/// structure. Returns the trimmed block without a trailing newline.
fn trim_blank_edges(s: &str) -> String {
    let lines: Vec<&str> = s.split('\n').collect();
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let last = lines.iter().rposition(|l| !l.trim().is_empty());
    match (first, last) {
        (Some(a), Some(b)) => lines[a..=b].join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Title\n\nIntro paragraph.\n\n## Install\n\nRun the installer.\n\n### Linux\n\napt install foo\n\n### Windows\n\nuse the .exe\n\n## Usage\n\nUse it like so.\n";

    #[test]
    fn extracts_section_with_nested_subsections_by_default() {
        let out = extract(DOC, "Install", "exact", true, true).unwrap();
        assert!(out.starts_with("## Install"), "got: {out}");
        assert!(out.contains("Run the installer."));
        assert!(out.contains("### Linux"));
        assert!(out.contains("apt install foo"));
        assert!(out.contains("### Windows"));
        // Stops before the next same-level heading.
        assert!(!out.contains("## Usage"));
        assert!(!out.contains("Use it like so."));
    }

    #[test]
    fn exclude_subsections_returns_only_direct_body() {
        let out = extract(DOC, "Install", "exact", false, true).unwrap();
        assert!(out.starts_with("## Install"));
        assert!(out.contains("Run the installer."));
        // Stops at the first deeper heading.
        assert!(!out.contains("### Linux"), "got: {out}");
    }

    #[test]
    fn exclude_heading_returns_body_only() {
        let out = extract(DOC, "Usage", "exact", true, false).unwrap();
        assert_eq!(out, "Use it like so.");
    }

    #[test]
    fn deepest_section_runs_to_end_of_document() {
        let out = extract(DOC, "Windows", "exact", true, true).unwrap();
        assert!(out.starts_with("### Windows"));
        assert!(out.contains("use the .exe"));
        // The next ## Usage is shallower, so it ends the ### section.
        assert!(!out.contains("## Usage"));
    }

    #[test]
    fn case_insensitive_by_default() {
        let out = extract(DOC, "install", "exact", true, true).unwrap();
        assert!(out.starts_with("## Install"));
    }

    #[test]
    fn exact_case_is_case_sensitive() {
        let err = extract(DOC, "install", "exact_case", true, true).unwrap_err();
        assert!(err.contains("no heading matching"), "got: {err}");
        let ok = extract(DOC, "Install", "exact_case", true, true).unwrap();
        assert!(ok.starts_with("## Install"));
    }

    #[test]
    fn contains_matches_substring() {
        let out = extract(DOC, "stall", "contains", true, true).unwrap();
        assert!(out.starts_with("## Install"));
    }

    #[test]
    fn level1_section_includes_all_deeper_headings_to_end() {
        // `# Title` is the only level-1 heading, so its section spans the whole
        // document, including every ## and ### below it.
        let out = extract(DOC, "Title", "contains", true, true).unwrap();
        assert!(out.starts_with("# Title"));
        assert!(out.contains("Intro paragraph."));
        assert!(out.contains("## Install"));
        assert!(out.contains("## Usage"));
    }

    #[test]
    fn contains_picks_first_matching_heading() {
        // Both "Install" and "Windows" exist; "win" matches only Windows.
        let out = extract(DOC, "win", "contains", true, true).unwrap();
        assert!(out.starts_with("### Windows"), "got: {out}");
    }

    #[test]
    fn missing_heading_errors_with_available_list() {
        let err = extract(DOC, "Nope", "exact", true, true).unwrap_err();
        assert!(err.contains("no heading matching"), "got: {err}");
        assert!(err.contains("Install"), "should list headings: {err}");
    }

    #[test]
    fn blank_heading_errors() {
        let err = extract(DOC, "   ", "exact", true, true).unwrap_err();
        assert!(err.contains("heading is required"), "got: {err}");
    }

    #[test]
    fn invalid_match_mode_errors() {
        let err = extract(DOC, "Install", "fuzzy", true, true).unwrap_err();
        assert!(err.contains("invalid match"), "got: {err}");
    }

    #[test]
    fn setext_headings_are_recognized() {
        let doc = "Title\n=====\n\nintro\n\nSection\n-------\n\nbody text\n\nNext\n----\n\nother";
        let out = extract(doc, "Section", "exact", true, true).unwrap();
        assert!(out.starts_with("Section\n-------"), "got: {out}");
        assert!(out.contains("body text"));
        // Section (level 2) ends at Next (level 2).
        assert!(!out.contains("other"));
    }

    #[test]
    fn ignores_headings_inside_fenced_code_blocks() {
        let doc = "# Real\n\n```\n# Fake heading in code\nnot a section\n```\n\nafter\n\n## Real Two\n\nx";
        // The fake heading inside the fence must NOT match.
        let err = extract(doc, "Fake heading in code", "exact", true, true).unwrap_err();
        assert!(err.contains("no heading matching"), "got: {err}");
        // The real section includes the fenced block verbatim. `# Real` is
        // level 1, so the level-2 `## Real Two` nests under it and is included.
        let out = extract(doc, "Real", "exact", true, true).unwrap();
        assert!(out.contains("# Fake heading in code"));
        assert!(out.contains("after"));
        assert!(out.contains("## Real Two"));
        // Excluding subsections stops at the first deeper heading (## Real Two).
        let direct = extract(doc, "Real", "exact", false, true).unwrap();
        assert!(direct.contains("after"));
        assert!(!direct.contains("## Real Two"), "got: {direct}");
    }

    #[test]
    fn atx_closing_hashes_are_stripped_for_matching() {
        let doc = "## Notes ##\n\nsome notes\n";
        let out = extract(doc, "Notes", "exact", true, true).unwrap();
        assert!(out.contains("some notes"));
    }

    #[test]
    fn hash_not_followed_by_space_is_not_a_heading() {
        let doc = "#NotAHeading\n\ntext\n\n## C# Tips\n\nsharp\n";
        let out = extract(doc, "C# Tips", "exact", true, true).unwrap();
        assert!(out.contains("sharp"), "got: {out}");
    }

    #[test]
    fn empty_section_yields_empty_output() {
        let doc = "## Empty\n\n## Next\n\nbody";
        let out = extract(doc, "Empty", "exact", true, true).unwrap();
        assert_eq!(out, "## Empty");
        let body = extract(doc, "Empty", "exact", true, false).unwrap();
        assert_eq!(body, "");
    }
}
