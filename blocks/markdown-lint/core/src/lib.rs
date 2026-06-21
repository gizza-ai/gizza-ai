//! markdown-lint core — find (and optionally auto-fix) common Markdown style and
//! consistency issues. Dependency-free, line-based linter inspired by the widely
//! used markdownlint rule set (MD0xx). Shared by the chat skill block and the web page.
//!
//! Two modes:
//!   - mode = "check": return a human-readable report of every issue found, with
//!     `line:col  MDxxx  message`, plus a summary count.
//!   - mode = "fix":   return the auto-fixed Markdown (the fixable rules applied),
//!     followed by a short note of any remaining issues that can't be auto-fixed.
//!
//! Fenced code blocks (``` / ~~~) are tracked so rules that apply to prose
//! (heading spacing, list markers, trailing punctuation, …) don't false-fire on
//! code, while whitespace rules (hard tabs, trailing whitespace) still apply.

/// A single lint finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub line: usize,
    pub col: usize,
    pub rule: &'static str,
    pub message: String,
    /// Whether this rule was auto-fixable (so `fix` could address it).
    pub fixable: bool,
}

fn is_fence(trimmed: &str) -> Option<char> {
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// ATX heading: leading `#`s (1-6) — returns (hashes, rest_after_hashes).
fn atx_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    // A real heading needs a space (or end of line) after the hashes; `#tag` is not.
    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
        Some((hashes, rest))
    } else {
        None
    }
}

/// Unordered list marker at the start of (trimmed) line → the marker char.
fn ul_marker(trimmed: &str) -> Option<char> {
    let mut ch = trimmed.chars();
    match ch.next() {
        Some(m @ ('*' | '+' | '-')) => {
            // Must be followed by a space to be a list item (avoids `*emph*`, `---`).
            if matches!(ch.next(), Some(' ') | Some('\t')) {
                Some(m)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Analyze `md`, returning every issue found (sorted by line then col).
pub fn lint(md: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    let lines: Vec<&str> = md.split('\n').collect();
    // `md.split('\n')` yields a trailing "" when the text ends with '\n'.
    let has_trailing_newline = md.ends_with('\n');
    let logical_len = if has_trailing_newline && !lines.is_empty() {
        lines.len() - 1
    } else {
        lines.len()
    };

    let mut in_fence = false;
    let mut fence_char = '`';
    let mut blank_run = 0usize;
    let mut first_ul_marker: Option<char> = None;
    let mut prev_blank_or_start = true;
    let mut h1_count = 0usize;
    let mut prev_heading_level = 0usize;

    for (idx, raw) in lines.iter().take(logical_len).enumerate() {
        let line_no = idx + 1;
        let line = *raw;
        let trimmed = line.trim_start();

        // Fence toggling (MD-aware): a line starting with ``` / ~~~ flips state.
        if let Some(fc) = is_fence(trimmed) {
            let opening = !in_fence;
            if opening {
                in_fence = true;
                fence_char = fc;
                // MD040 fenced code block should declare a language.
                let info = trimmed.trim_start_matches(fc).trim();
                if info.is_empty() {
                    issues.push(Issue {
                        line: line_no,
                        col: 1,
                        rule: "MD040",
                        message: "Fenced code block should specify a language".into(),
                        fixable: false,
                    });
                }
            } else if fc == fence_char {
                in_fence = false;
            }
        }

        // --- whitespace rules apply everywhere (incl. inside code) ---
        // MD009 trailing whitespace.
        if line.ends_with(' ') || line.ends_with('\t') {
            // Allow exactly two trailing spaces (Markdown hard line break) outside code.
            let trailing = line.len() - line.trim_end().len();
            let hard_break = !in_fence && line.ends_with("  ") && trailing == 2;
            if !hard_break {
                issues.push(Issue {
                    line: line_no,
                    col: line.trim_end().len() + 1,
                    rule: "MD009",
                    message: "Trailing whitespace".into(),
                    fixable: true,
                });
            }
        }
        // MD010 hard tabs.
        if line.contains('\t') {
            let col = line.find('\t').unwrap() + 1;
            issues.push(Issue {
                line: line_no,
                col,
                rule: "MD010",
                message: "Hard tab character (use spaces)".into(),
                fixable: true,
            });
        }

        let is_blank = line.trim().is_empty();
        if is_blank {
            blank_run += 1;
            // MD012 multiple consecutive blank lines.
            if blank_run == 2 {
                issues.push(Issue {
                    line: line_no,
                    col: 1,
                    rule: "MD012",
                    message: "Multiple consecutive blank lines".into(),
                    fixable: true,
                });
            }
            prev_blank_or_start = true;
            continue;
        }
        blank_run = 0;

        // --- prose-only rules (skip inside fenced code) ---
        if !in_fence {
            // MD018/MD019: ATX heading spacing.
            if trimmed.starts_with('#') {
                let hashes = trimmed.chars().take_while(|&c| c == '#').count();
                if (1..=6).contains(&hashes) {
                    let after = &trimmed[hashes..];
                    if !after.is_empty()
                        && !after.starts_with(' ')
                        && !after.starts_with('\t')
                    {
                        // `#Heading` — no space after hashes.
                        issues.push(Issue {
                            line: line_no,
                            col: line.len() - trimmed.len() + hashes + 1,
                            rule: "MD018",
                            message: "No space after hash on ATX heading".into(),
                            fixable: true,
                        });
                    } else if after.starts_with("  ") {
                        // Multiple spaces after hashes.
                        issues.push(Issue {
                            line: line_no,
                            col: line.len() - trimmed.len() + hashes + 1,
                            rule: "MD019",
                            message: "Multiple spaces after hash on ATX heading".into(),
                            fixable: true,
                        });
                    }
                }
            }

            if let Some((hashes, rest)) = atx_heading(line) {
                // MD001 heading levels should increment by one (no skipping).
                if prev_heading_level != 0 && hashes > prev_heading_level + 1 {
                    issues.push(Issue {
                        line: line_no,
                        col: 1,
                        rule: "MD001",
                        message: format!(
                            "Heading level jumps from h{prev_heading_level} to h{hashes} (increment by one)"
                        ),
                        fixable: false,
                    });
                }
                prev_heading_level = hashes;
                // MD025 multiple top-level (h1) headings.
                let ts = line.trim_start();
                if ts.starts_with("# ") || ts == "#" {
                    h1_count += 1;
                    if h1_count == 2 {
                        issues.push(Issue {
                            line: line_no,
                            col: 1,
                            rule: "MD025",
                            message: "Multiple top-level (H1) headings in document".into(),
                            fixable: false,
                        });
                    }
                }
                // MD026 trailing punctuation in heading.
                let text = rest.trim();
                if let Some(last) = text.chars().last() {
                    if matches!(last, '.' | ',' | ';' | ':' | '!') {
                        issues.push(Issue {
                            line: line_no,
                            col: 1,
                            rule: "MD026",
                            message: format!("Trailing punctuation '{last}' in heading"),
                            fixable: true,
                        });
                    }
                }
                // MD022 headings should be surrounded by blank lines (before).
                if !prev_blank_or_start {
                    issues.push(Issue {
                        line: line_no,
                        col: 1,
                        rule: "MD022",
                        message: "Heading should be preceded by a blank line".into(),
                        fixable: true,
                    });
                }
            }

            // MD004 list marker consistency.
            if let Some(m) = ul_marker(trimmed) {
                match first_ul_marker {
                    None => first_ul_marker = Some(m),
                    Some(first) if first != m => {
                        issues.push(Issue {
                            line: line_no,
                            col: line.len() - trimmed.len() + 1,
                            rule: "MD004",
                            message: format!(
                                "Inconsistent unordered list marker '{m}' (expected '{first}')"
                            ),
                            fixable: true,
                        });
                    }
                    _ => {}
                }
            }
        }

        prev_blank_or_start = false;
    }

    // MD047 file should end with a single newline.
    if !md.is_empty() && !has_trailing_newline {
        issues.push(Issue {
            line: logical_len.max(1),
            col: lines
                .get(logical_len.saturating_sub(1))
                .map(|l| l.len() + 1)
                .unwrap_or(1),
            rule: "MD047",
            message: "File should end with a single newline".into(),
            fixable: true,
        });
    }
    // MD012 also: trailing blank lines at end of file.
    if has_trailing_newline {
        let mut trailing_blank = 0usize;
        for raw in lines.iter().take(logical_len).rev() {
            if raw.trim().is_empty() {
                trailing_blank += 1;
            } else {
                break;
            }
        }
        if trailing_blank >= 1 {
            issues.push(Issue {
                line: logical_len,
                col: 1,
                rule: "MD012",
                message: "Trailing blank line(s) at end of file".into(),
                fixable: true,
            });
        }
    }

    issues.sort_by(|a, b| (a.line, a.col).cmp(&(b.line, b.col)));
    issues
}

/// Auto-fix the fixable issues and return the corrected Markdown.
pub fn fix(md: &str) -> String {
    let has_trailing_newline = md.ends_with('\n');
    let lines: Vec<&str> = md.split('\n').collect();
    let logical_len = if has_trailing_newline && !lines.is_empty() {
        lines.len() - 1
    } else {
        lines.len()
    };

    let mut in_fence = false;
    let mut fence_char = '`';
    let mut first_ul_marker: Option<char> = None;
    let mut out_lines: Vec<String> = Vec::new();
    let mut prev_blank = true;
    let mut blank_run = 0usize;

    for raw in lines.iter().take(logical_len) {
        let mut line = raw.to_string();

        // MD010: expand hard tabs to 4 spaces (column-naive but deterministic).
        if line.contains('\t') {
            line = line.replace('\t', "    ");
        }

        let trimmed_start = line.trim_start().to_string();

        // Track fence state.
        if let Some(fc) = is_fence(&trimmed_start) {
            if !in_fence {
                in_fence = true;
                fence_char = fc;
            } else if fc == fence_char {
                in_fence = false;
            }
        }

        // MD009: strip trailing whitespace (keep a 2-space hard break outside code).
        let trailing = line.len() - line.trim_end().len();
        if !in_fence && line.ends_with("  ") && trailing == 2 {
            line = format!("{}  ", line.trim_end());
        } else {
            line = line.trim_end().to_string();
        }

        let is_blank = line.trim().is_empty();
        if is_blank {
            blank_run += 1;
            // MD012: collapse multiple consecutive blank lines into one.
            if blank_run >= 2 {
                continue;
            }
            prev_blank = true;
            out_lines.push(String::new());
            continue;
        }
        blank_run = 0;

        if !in_fence {
            // Fix ATX heading spacing (MD018/MD019) and trailing punctuation (MD026).
            let indent_len = line.len() - line.trim_start().len();
            let indent = line[..indent_len].to_string();
            let body = line[indent_len..].to_string();
            if body.starts_with('#') {
                let hashes = body.chars().take_while(|&c| c == '#').count();
                if (1..=6).contains(&hashes) {
                    let after = &body[hashes..];
                    let after_trimmed = after.trim_start();
                    let needs_space_fix = after.is_empty()
                        || !after.starts_with(' ')
                        || after.starts_with("  ");
                    if !after_trimmed.is_empty() {
                        let mut text = after_trimmed.to_string();
                        // MD026 trailing punctuation.
                        let mut punct_changed = false;
                        while let Some(last) = text.chars().last() {
                            if matches!(last, '.' | ',' | ';' | ':' | '!') {
                                text.pop();
                                text = text.trim_end().to_string();
                                punct_changed = true;
                            } else {
                                break;
                            }
                        }
                        if needs_space_fix || punct_changed {
                            line = format!("{indent}{} {text}", "#".repeat(hashes));
                        }
                    } else if needs_space_fix {
                        line = format!("{indent}{}", "#".repeat(hashes));
                    }
                }
            }

            // MD022: ensure a blank line before a heading.
            if atx_heading(&line).is_some() && !prev_blank && !out_lines.is_empty() {
                out_lines.push(String::new());
            }

            // MD004: normalize unordered list markers to the first-seen marker.
            if let Some(m) = ul_marker(line.trim_start()) {
                match first_ul_marker {
                    None => first_ul_marker = Some(m),
                    Some(first) if first != m => {
                        let il = line.len() - line.trim_start().len();
                        let rest = &line.trim_start()[1..];
                        line = format!("{}{first}{rest}", &line[..il]);
                    }
                    _ => {}
                }
            }
        }

        out_lines.push(line);
        prev_blank = false;
    }

    // Drop trailing blank lines (MD012 at EOF).
    while out_lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        out_lines.pop();
    }

    let mut result = out_lines.join("\n");
    // MD047: end with exactly one newline (unless the file was empty).
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

/// Render a human-readable report of issues.
pub fn report(issues: &[Issue]) -> String {
    if issues.is_empty() {
        return "No issues found — Markdown looks clean.".into();
    }
    let mut s = String::new();
    for i in issues {
        s.push_str(&format!("{}:{}  {}  {}\n", i.line, i.col, i.rule, i.message));
    }
    let fixable = issues.iter().filter(|i| i.fixable).count();
    s.push_str(&format!(
        "\n{} issue(s) found ({} auto-fixable).",
        issues.len(),
        fixable
    ));
    s
}

/// Top-level entry shared by all surfaces. `mode` is "check" (default) or "fix".
pub fn run(md: &str, mode: &str) -> Result<String, String> {
    if md.trim().is_empty() {
        return Err("no Markdown input".into());
    }
    match mode {
        "fix" => {
            let fixed = fix(md);
            // Note remaining (non-fixable) issues found in the fixed output.
            let remaining: Vec<Issue> =
                lint(&fixed).into_iter().filter(|i| !i.fixable).collect();
            if remaining.is_empty() {
                Ok(fixed)
            } else {
                let mut s = fixed;
                s.push_str(
                    "\n<!-- markdown-lint: remaining issues that need manual review:\n",
                );
                for i in &remaining {
                    s.push_str(&format!("  {}:{}  {}  {}\n", i.line, i.col, i.rule, i.message));
                }
                s.push_str("-->\n");
                Ok(s)
            }
        }
        "check" | "" => Ok(report(&lint(md))),
        other => Err(format!("unknown mode '{other}' (use 'check' or 'fix')")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_trailing_whitespace() {
        let issues = lint("hello   \nworld\n");
        assert!(issues.iter().any(|i| i.rule == "MD009" && i.line == 1));
    }

    #[test]
    fn allows_hard_break_two_spaces() {
        let issues = lint("line one  \nline two\n");
        assert!(!issues.iter().any(|i| i.rule == "MD009"));
    }

    #[test]
    fn detects_hard_tab() {
        let issues = lint("a\tb\n");
        assert!(issues.iter().any(|i| i.rule == "MD010"));
    }

    #[test]
    fn detects_multiple_blank_lines() {
        let issues = lint("a\n\n\nb\n");
        assert!(issues.iter().any(|i| i.rule == "MD012"));
    }

    #[test]
    fn detects_no_space_after_hash() {
        let issues = lint("#Heading\n");
        assert!(issues.iter().any(|i| i.rule == "MD018"));
    }

    #[test]
    fn detects_multiple_spaces_after_hash() {
        let issues = lint("#  Heading\n");
        assert!(issues.iter().any(|i| i.rule == "MD019"));
    }

    #[test]
    fn detects_heading_trailing_punctuation() {
        let issues = lint("# Hello world.\n");
        assert!(issues.iter().any(|i| i.rule == "MD026"));
    }

    #[test]
    fn detects_inconsistent_list_markers() {
        let issues = lint("* a\n- b\n");
        assert!(issues.iter().any(|i| i.rule == "MD004"));
    }

    #[test]
    fn detects_missing_final_newline() {
        let issues = lint("no newline");
        assert!(issues.iter().any(|i| i.rule == "MD047"));
    }

    #[test]
    fn detects_multiple_h1() {
        let issues = lint("# One\n\nsome text\n\n# Two\n");
        assert!(issues.iter().any(|i| i.rule == "MD025"));
    }

    #[test]
    fn detects_heading_level_skip() {
        let issues = lint("# One\n\n### Three\n");
        assert!(issues.iter().any(|i| i.rule == "MD001"));
    }

    #[test]
    fn allows_incrementing_headings() {
        let issues = lint("# One\n\n## Two\n\n### Three\n");
        assert!(!issues.iter().any(|i| i.rule == "MD001"));
    }

    #[test]
    fn detects_fence_without_language() {
        let issues = lint("```\ncode\n```\n");
        assert!(issues.iter().any(|i| i.rule == "MD040"));
    }

    #[test]
    fn allows_fence_with_language() {
        let issues = lint("```rust\ncode\n```\n");
        assert!(!issues.iter().any(|i| i.rule == "MD040"));
    }

    #[test]
    fn skips_rules_inside_code_fence() {
        let md = "```\n#Heading\nsome line.\n```\n";
        let issues = lint(md);
        assert!(!issues.iter().any(|i| i.rule == "MD018"));
    }

    #[test]
    fn whitespace_rules_still_fire_in_code() {
        let md = "```\ncode   \n```\n";
        let issues = lint(md);
        assert!(issues.iter().any(|i| i.rule == "MD009"));
    }

    #[test]
    fn fix_strips_trailing_ws() {
        let out = fix("hello   \nworld\n");
        assert_eq!(out, "hello\nworld\n");
    }

    #[test]
    fn fix_keeps_hard_break() {
        let out = fix("line  \ntwo\n");
        assert_eq!(out, "line  \ntwo\n");
    }

    #[test]
    fn fix_expands_tabs() {
        let out = fix("a\tb\n");
        assert_eq!(out, "a    b\n");
    }

    #[test]
    fn fix_collapses_blank_lines() {
        let out = fix("a\n\n\n\nb\n");
        assert_eq!(out, "a\n\nb\n");
    }

    #[test]
    fn fix_heading_spacing() {
        let out = fix("#Heading\n");
        assert_eq!(out, "# Heading\n");
        let out2 = fix("#   Heading\n");
        assert_eq!(out2, "# Heading\n");
    }

    #[test]
    fn fix_heading_punctuation() {
        let out = fix("# Hello world.\n");
        assert_eq!(out, "# Hello world\n");
    }

    #[test]
    fn fix_normalizes_list_markers() {
        let out = fix("* a\n- b\n+ c\n");
        assert_eq!(out, "* a\n* b\n* c\n");
    }

    #[test]
    fn fix_adds_final_newline() {
        let out = fix("text");
        assert_eq!(out, "text\n");
    }

    #[test]
    fn fix_inserts_blank_before_heading() {
        let out = fix("text\n# Heading\n");
        assert_eq!(out, "text\n\n# Heading\n");
    }

    #[test]
    fn fix_is_idempotent() {
        let dirty = "#Title.  \ntext\n\n\n* a\n- b\tx\n";
        let once = fix(dirty);
        let twice = fix(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn run_check_reports_clean() {
        let out = run("# Title\n\nclean text\n", "check").unwrap();
        assert!(out.contains("No issues"));
    }

    #[test]
    fn run_fix_returns_fixed() {
        let out = run("#Title\n", "fix").unwrap();
        assert!(out.starts_with("# Title"));
    }

    #[test]
    fn run_rejects_empty() {
        assert!(run("   ", "check").is_err());
    }

    #[test]
    fn run_rejects_unknown_mode() {
        assert!(run("x\n", "bogus").is_err());
    }
}
