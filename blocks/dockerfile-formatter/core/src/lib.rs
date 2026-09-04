//! dockerfile-formatter core — pure compute, shared by the chat skill block and the web page.
//!
//! Normalizes a Dockerfile's instruction casing, continuation indentation, blank
//! lines and comment spacing without reordering or rewriting instruction
//! arguments. Parser directives (`# syntax=`, `# escape=`, `# check=`) and
//! heredoc bodies are passed through verbatim, because both are byte-sensitive.

/// Every instruction the Dockerfile grammar defines. Anything else at the start
/// of a logical line is a typo, and formatting it silently would hide the bug.
pub const INSTRUCTIONS: [&str; 18] = [
    "ADD",
    "ARG",
    "CMD",
    "COPY",
    "ENTRYPOINT",
    "ENV",
    "EXPOSE",
    "FROM",
    "HEALTHCHECK",
    "LABEL",
    "MAINTAINER",
    "ONBUILD",
    "RUN",
    "SHELL",
    "STOPSIGNAL",
    "USER",
    "VOLUME",
    "WORKDIR",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Upper,
    Lower,
    Preserve,
}

/// One physical line inside a logical instruction.
enum Seg {
    Code(String),
    Comment(String),
}

enum Node {
    Blank,
    /// A standalone comment or a top-of-file parser directive (already rendered).
    Text(String),
    Instr {
        is_from: bool,
        lines: Vec<String>,
    },
}

fn parse_case(s: &str) -> Result<Case, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "upper" => Ok(Case::Upper),
        "lower" => Ok(Case::Lower),
        "preserve" => Ok(Case::Preserve),
        other => Err(format!(
            "unknown instruction_case '{other}' (expected upper, lower or preserve)"
        )),
    }
}

fn apply_case(word: &str, case: Case) -> String {
    match case {
        Case::Upper => word.to_ascii_uppercase(),
        Case::Lower => word.to_ascii_lowercase(),
        Case::Preserve => word.to_string(),
    }
}

fn split_first_token(s: &str) -> (String, String) {
    match s.find(char::is_whitespace) {
        Some(p) => (s[..p].to_string(), s[p..].trim().to_string()),
        None => (s.to_string(), String::new()),
    }
}

/// A line continues when it ends in an ODD number of escape characters — an even
/// run is an escaped literal backslash, not a continuation.
fn ends_with_continuation(s: &str, escape: char) -> bool {
    s.chars().rev().take_while(|c| *c == escape).count() % 2 == 1
}

/// `# key=value` at the very top of the file. Returns the lowercased key.
fn directive_key(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix('#')?;
    let (k, _) = rest.split_once('=')?;
    let key = k.trim().to_ascii_lowercase();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(key)
}

fn directive_value(line: &str) -> String {
    line.trim_start()
        .strip_prefix('#')
        .and_then(|r| r.split_once('='))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_default()
}

/// Normalize `#comment` to `# comment`. Banner comments (`####`), shebang-ish
/// `#!` lines and empty `#` are left alone — a space would only damage them.
fn render_comment(trimmed: &str, normalize: bool) -> String {
    let body = trimmed.strip_prefix('#').unwrap_or(trimmed).trim_end();
    if !normalize {
        return format!("#{body}");
    }
    match body.chars().next() {
        None => "#".to_string(),
        Some(c) if c.is_whitespace() => format!("# {}", body.trim_start()),
        Some('#') | Some('!') => format!("#{body}"),
        Some(_) => format!("# {body}"),
    }
}

/// Heredoc openers on a line: `<<EOF`, `<<-EOF`, `<<'EOF'`, `<<"EOF"`.
/// Returns each delimiter with whether it was opened in `<<-` (tab-stripping) form.
fn find_heredocs(line: &str) -> Vec<(String, bool)> {
    let b: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k + 1 < b.len() {
        if b[k] == '<' && b[k + 1] == '<' {
            let prev_ok = k == 0 || b[k - 1].is_whitespace();
            let mut j = k + 2;
            let dash = j < b.len() && b[j] == '-';
            if dash {
                j += 1;
            }
            if j < b.len() && (b[j] == '\'' || b[j] == '"') {
                j += 1;
            }
            let start = j;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '_') {
                j += 1;
            }
            if prev_ok && j > start {
                out.push((b[start..j].iter().collect::<String>(), dash));
                k = j;
                continue;
            }
        }
        k += 1;
    }
    out
}

/// Format a Dockerfile.
///
/// * `instruction_case` — `upper` | `lower` | `preserve` for instruction keywords
///   (and the `AS` in `FROM … AS stage`).
/// * `indent` — spaces prefixed to every continuation line (0-8).
/// * `align_continuations` — pad so the trailing escape characters line up.
/// * `max_blank_lines` — cap on consecutive blank lines (0-5).
/// * `blank_line_between_stages` — guarantee one blank line before each later `FROM`.
/// * `normalize_comments` — ensure a single space after `#`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    instruction_case: &str,
    indent: usize,
    align_continuations: bool,
    max_blank_lines: usize,
    blank_line_between_stages: bool,
    normalize_comments: bool,
) -> Result<String, String> {
    let case = parse_case(instruction_case)?;
    if indent > 8 {
        return Err(format!(
            "indent must be between 0 and 8 spaces (got {indent})"
        ));
    }
    if max_blank_lines > 5 {
        return Err(format!(
            "max_blank_lines must be between 0 and 5 (got {max_blank_lines})"
        ));
    }
    if input.trim().is_empty() {
        return Err("input is empty — paste a Dockerfile to format".to_string());
    }

    let raw: Vec<&str> = input
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();

    let mut nodes: Vec<Node> = Vec::new();
    let mut escape = '\\';
    let mut i = 0;

    // Parser directives only count while nothing else has been seen yet.
    while i < raw.len() {
        let t = raw[i].trim();
        let key = match directive_key(t) {
            Some(k) if matches!(k.as_str(), "syntax" | "escape" | "check") => k,
            _ => break,
        };
        if key == "escape" {
            let v = directive_value(t);
            match v.chars().next() {
                Some('\\') => escape = '\\',
                Some('`') => escape = '`',
                _ => {
                    return Err(format!(
                        "line {}: escape directive must be \\ or ` (got '{v}')",
                        i + 1
                    ))
                }
            }
        }
        nodes.push(Node::Text(t.to_string()));
        i += 1;
    }

    while i < raw.len() {
        let t = raw[i].trim();
        if t.is_empty() {
            nodes.push(Node::Blank);
            i += 1;
            continue;
        }
        if t.starts_with('#') {
            nodes.push(Node::Text(render_comment(t, normalize_comments)));
            i += 1;
            continue;
        }

        let start_line = i + 1;
        let mut segs: Vec<Seg> = Vec::new();
        loop {
            let cur = raw[i].trim();
            let continues = ends_with_continuation(cur, escape);
            let content = if continues {
                cur[..cur.len() - escape.len_utf8()].trim_end().to_string()
            } else {
                cur.to_string()
            };
            segs.push(Seg::Code(content));
            i += 1;
            if !continues {
                break;
            }
            // Comments and blank lines inside a continuation do not end it.
            while i < raw.len() {
                let n = raw[i].trim();
                if n.is_empty() {
                    i += 1;
                } else if n.starts_with('#') {
                    segs.push(Seg::Comment(render_comment(n, normalize_comments)));
                    i += 1;
                } else {
                    break;
                }
            }
            if i >= raw.len() {
                return Err(format!(
                    "line {start_line}: unexpected end of file after a line continuation"
                ));
            }
        }

        // Heredoc bodies belong to the instruction and are copied byte for byte.
        let mut heredoc: Vec<String> = Vec::new();
        for (delim, dash) in segs
            .iter()
            .filter_map(|s| match s {
                Seg::Code(c) => Some(find_heredocs(c)),
                Seg::Comment(_) => None,
            })
            .flatten()
        {
            let mut closed = false;
            while i < raw.len() {
                let line = raw[i];
                heredoc.push(line.to_string());
                i += 1;
                let candidate = if dash {
                    line.trim_start_matches(['\t', ' '])
                } else {
                    line
                };
                if candidate.trim_end() == delim {
                    closed = true;
                    break;
                }
            }
            if !closed {
                return Err(format!("line {start_line}: unterminated heredoc '{delim}'"));
            }
        }

        let (is_from, lines) = render_instruction(
            &segs,
            heredoc,
            start_line,
            case,
            indent,
            align_continuations,
            escape,
        )?;
        nodes.push(Node::Instr { is_from, lines });
    }

    if blank_line_between_stages {
        insert_stage_blanks(&mut nodes);
    }

    let mut out: Vec<String> = Vec::new();
    let mut pending = 0usize;
    let mut started = false;
    for node in nodes {
        match node {
            Node::Blank => {
                if started {
                    pending += 1;
                }
            }
            Node::Text(t) => {
                push_blanks(&mut out, pending, max_blank_lines);
                pending = 0;
                started = true;
                out.push(t);
            }
            Node::Instr { lines, .. } => {
                push_blanks(&mut out, pending, max_blank_lines);
                pending = 0;
                started = true;
                out.extend(lines);
            }
        }
    }

    if out.is_empty() {
        return Err("input has no Dockerfile content to format".to_string());
    }
    out.push(String::new());
    Ok(out.join("\n"))
}

fn push_blanks(out: &mut Vec<String>, pending: usize, max_blank_lines: usize) {
    for _ in 0..pending.min(max_blank_lines) {
        out.push(String::new());
    }
}

/// Guarantee one blank line before every stage after the first, placed ABOVE the
/// comment block that documents the stage rather than between comment and `FROM`.
fn insert_stage_blanks(nodes: &mut Vec<Node>) {
    let mut inserts: Vec<usize> = Vec::new();
    let mut seen_content = false;
    for idx in 0..nodes.len() {
        match &nodes[idx] {
            Node::Blank => {}
            Node::Text(_) => seen_content = true,
            Node::Instr { is_from, .. } => {
                if *is_from && seen_content {
                    let mut j = idx;
                    while j > 0 && matches!(nodes[j - 1], Node::Text(_)) {
                        j -= 1;
                    }
                    if j > 0 && !matches!(nodes[j - 1], Node::Blank) {
                        inserts.push(j);
                    }
                }
                seen_content = true;
            }
        }
    }
    for p in inserts.into_iter().rev() {
        nodes.insert(p, Node::Blank);
    }
}

fn render_instruction(
    segs: &[Seg],
    heredoc: Vec<String>,
    start_line: usize,
    case: Case,
    indent: usize,
    align: bool,
    escape: char,
) -> Result<(bool, Vec<String>), String> {
    let first = match &segs[0] {
        Seg::Code(c) => c.clone(),
        Seg::Comment(_) => unreachable!("an instruction always starts with a code line"),
    };
    let (keyword, mut rest) = split_first_token(&first);
    let upper = keyword.to_ascii_uppercase();
    if !INSTRUCTIONS.contains(&upper.as_str()) {
        return Err(format!(
            "line {start_line}: unknown Dockerfile instruction '{keyword}'"
        ));
    }
    let mut head = apply_case(&keyword, case);

    if upper == "ONBUILD" {
        let (sub, sub_rest) = split_first_token(&rest);
        let sub_upper = sub.to_ascii_uppercase();
        if !INSTRUCTIONS.contains(&sub_upper.as_str())
            || matches!(sub_upper.as_str(), "ONBUILD" | "FROM" | "MAINTAINER")
        {
            return Err(format!(
                "line {start_line}: ONBUILD must be followed by another instruction (not '{sub}')"
            ));
        }
        head.push(' ');
        head.push_str(&apply_case(&sub, case));
        rest = sub_rest;
    }

    if upper == "FROM" {
        // FROM arguments are whitespace-insensitive, so the AS keyword can be
        // cased and runs of spaces collapsed without touching user data.
        rest = rest
            .split_whitespace()
            .map(|t| {
                if t.eq_ignore_ascii_case("as") {
                    apply_case(t, case)
                } else {
                    t.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
    }

    let mut lines: Vec<(String, bool)> = Vec::new();
    lines.push((
        if rest.is_empty() {
            head
        } else {
            format!("{head} {rest}")
        },
        true,
    ));
    let pad = " ".repeat(indent);
    for seg in &segs[1..] {
        match seg {
            Seg::Code(c) => lines.push((format!("{pad}{c}"), true)),
            Seg::Comment(c) => lines.push((format!("{pad}{c}"), false)),
        }
    }

    // Every code line except the last one carries the escape character; comment
    // lines never do (the parser strips them before joining the continuation).
    let last_code = lines
        .iter()
        .rposition(|(_, is_code)| *is_code)
        .expect("at least one code line");
    let needing: Vec<usize> = (0..last_code).filter(|&k| lines[k].1).collect();
    let width = if align {
        needing
            .iter()
            .map(|&k| lines[k].0.chars().count())
            .max()
            .unwrap_or(0)
    } else {
        0
    };
    for &k in &needing {
        let fill = width.saturating_sub(lines[k].0.chars().count());
        lines[k].0 = format!("{}{} {}", lines[k].0, " ".repeat(fill), escape);
    }

    let mut out: Vec<String> = lines.into_iter().map(|(t, _)| t).collect();
    out.extend(heredoc);
    Ok((upper == "FROM", out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(input: &str) -> String {
        run(input, "upper", 4, false, 1, true, true).unwrap()
    }

    #[test]
    fn uppercases_instructions_and_reindents_continuations() {
        let out = fmt(
            "from   alpine:3.20 as   build\nrun apt-get update \\\n  && apt-get install -y curl\n",
        );
        assert_eq!(
            out,
            "FROM alpine:3.20 AS build\nRUN apt-get update \\\n    && apt-get install -y curl\n"
        );
    }

    #[test]
    fn lowercase_and_preserve_modes() {
        assert_eq!(
            run("From alpine As b\n", "lower", 4, false, 1, true, true).unwrap(),
            "from alpine as b\n"
        );
        assert_eq!(
            run("From alpine As b\n", "preserve", 4, false, 1, true, true).unwrap(),
            "From alpine As b\n"
        );
    }

    #[test]
    fn normalizes_crlf_trailing_space_and_blank_runs() {
        let out = fmt("FROM alpine\r\n\r\n\r\n\r\nUSER app   \r\n");
        assert_eq!(out, "FROM alpine\n\nUSER app\n");
    }

    #[test]
    fn max_blank_lines_zero_removes_all_blank_lines() {
        let out = run(
            "FROM alpine\n\n\nUSER app\n",
            "upper",
            4,
            false,
            0,
            false,
            true,
        )
        .unwrap();
        assert_eq!(out, "FROM alpine\nUSER app\n");
    }

    #[test]
    fn separates_stages_with_a_blank_line_above_their_comments() {
        let out = fmt("FROM alpine AS build\nRUN build.sh\n# runtime stage\nFROM alpine\n");
        assert_eq!(
            out,
            "FROM alpine AS build\nRUN build.sh\n\n# runtime stage\nFROM alpine\n"
        );
    }

    #[test]
    fn aligns_continuation_escapes_when_requested() {
        let out = run(
            "RUN a \\\n&& bbbb \\\n&& c\n",
            "upper",
            4,
            true,
            1,
            true,
            true,
        )
        .unwrap();
        assert_eq!(out, "RUN a       \\\n    && bbbb \\\n    && c\n");
    }

    #[test]
    fn keeps_comments_inside_a_continuation_without_an_escape() {
        let out = fmt("RUN a \\\n#note\n && b\n");
        assert_eq!(out, "RUN a \\\n    # note\n    && b\n");
    }

    #[test]
    fn normalize_comments_can_be_disabled_and_banners_are_untouched() {
        assert_eq!(
            fmt("#hello\n####\n#!x\nFROM a\n"),
            "# hello\n####\n#!x\nFROM a\n"
        );
        assert_eq!(
            run("#hello\nFROM a\n", "upper", 4, false, 1, true, false).unwrap(),
            "#hello\nFROM a\n"
        );
    }

    #[test]
    fn honors_the_escape_parser_directive() {
        let out = fmt("# escape=`\nfrom alpine\nrun a `\n  && b\n");
        assert_eq!(out, "# escape=`\nFROM alpine\nRUN a `\n    && b\n");
    }

    #[test]
    fn passes_heredoc_bodies_through_verbatim() {
        let out = fmt("run <<EOF\n  keep   this\n    indented\nEOF\nuser app\n");
        assert_eq!(
            out,
            "RUN <<EOF\n  keep   this\n    indented\nEOF\nUSER app\n"
        );
    }

    #[test]
    fn does_not_treat_an_escaped_backslash_as_a_continuation() {
        let out = fmt("RUN printf a\\\\\nUSER app\n");
        assert_eq!(out, "RUN printf a\\\\\nUSER app\n");
    }

    #[test]
    fn cases_the_instruction_after_onbuild() {
        assert_eq!(fmt("onbuild copy . /app\n"), "ONBUILD COPY . /app\n");
    }

    #[test]
    fn rejects_an_unknown_instruction() {
        let err = fmt_err("FROM alpine\nRNU echo hi\n");
        assert!(err.contains("line 2"), "{err}");
        assert!(
            err.contains("unknown Dockerfile instruction 'RNU'"),
            "{err}"
        );
    }

    #[test]
    fn rejects_a_dangling_continuation() {
        let err = fmt_err("RUN echo hi \\\n");
        assert!(
            err.contains("unexpected end of file after a line continuation"),
            "{err}"
        );
    }

    #[test]
    fn rejects_an_unterminated_heredoc() {
        let err = fmt_err("RUN <<EOF\necho hi\n");
        assert!(err.contains("unterminated heredoc 'EOF'"), "{err}");
    }

    #[test]
    fn rejects_empty_input_and_bad_options() {
        assert!(fmt_err("   \n").contains("input is empty"));
        assert!(run("FROM a\n", "title", 4, false, 1, true, true)
            .unwrap_err()
            .contains("unknown instruction_case"));
        assert!(run("FROM a\n", "upper", 9, false, 1, true, true)
            .unwrap_err()
            .contains("indent must be between 0 and 8"));
        assert!(run("FROM a\n", "upper", 4, false, 9, true, true)
            .unwrap_err()
            .contains("max_blank_lines must be between 0 and 5"));
        assert!(
            run("ONBUILD FROM alpine\n", "upper", 4, false, 1, true, true)
                .unwrap_err()
                .contains("ONBUILD must be followed by another instruction")
        );
    }

    fn fmt_err(input: &str) -> String {
        run(input, "upper", 4, false, 1, true, true).unwrap_err()
    }
}
