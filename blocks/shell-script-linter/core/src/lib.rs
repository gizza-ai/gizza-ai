//! shell-script-linter core — pure compute shared by the chat skill block and web page.
//!
//! Heuristic, offline linting for pasted bash/sh scripts. Nothing is executed and
//! nothing is written to disk: the script is scanned as text. Comments, single-quoted
//! strings and here-doc bodies are masked before the rules run so that examples inside
//! a comment do not produce findings.
//!
//! This is deliberately smaller than a full shell grammar. It reports the pitfalls that
//! can be detected reliably from masked tokens and line-aware scans: block-structure
//! errors, missing shebang, missing strict-mode options, unquoted expansions, useless
//! `cat`, backticks, subshell scope traps, unguarded `cd`, `ls` parsing, spaced
//! assignments, single-bracket tests, risky `rm -rf`, and POSIX-shell bashisms.

use regex::Regex;
use serde_json::json;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Largest script accepted, in bytes. Anything larger is rejected with an error
/// rather than silently truncated.
pub const MAX_INPUT_BYTES: usize = 200_000;

/// Every rule code this linter can emit, in report order of interest.
pub const RULE_CODES: [&str; 12] = [
    "SYNTAX",
    "MISSING-SHEBANG",
    "STRICT-MODE",
    "UNQUOTED-VAR",
    "USELESS-CAT",
    "BACKTICKS",
    "SUBSHELL-SCOPE",
    "UNCHECKED-CD",
    "PARSE-LS",
    "ASSIGN-SPACES",
    "LEGACY-TEST",
    "RM-RISK",
];

/// `SH-BASHISM` only applies to POSIX shells, so it lives outside [`RULE_CODES`]'
/// shared list but is still ignorable by name.
pub const SH_RULE_CODE: &str = "SH-BASHISM";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }

    fn parse_min(s: &str) -> Result<Severity, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" | "info" => Severity::Info,
            "warning" | "warn" => Severity::Warning,
            "error" | "errors" => Severity::Error,
            other => {
                return Err(format!(
                    "unknown min_severity '{other}' (use all, warning, or error)"
                ))
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
}

impl Format {
    fn parse(s: &str) -> Result<Format, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "report" => Format::Text,
            "json" => Format::Json,
            other => return Err(format!("unknown format '{other}' (use text or json)")),
        })
    }
}

#[derive(Debug, Clone)]
struct Finding {
    line: usize,
    code: &'static str,
    severity: Severity,
    message: String,
    snippet: String,
}

/// Result of masking: `code` has comments and every quoted region blanked out;
/// `code_dq` keeps double-quoted regions intact (needed by rules that must see a
/// quoted expansion, such as `rm -rf "$dir"/`). Both are byte-aligned with the input.
struct Masked {
    code: String,
    code_dq: String,
    /// Line number of an unterminated quote, if the scan ended inside one.
    unterminated: Option<(usize, char)>,
}

/// Lint a shell script.
///
/// - `script`: the script text (max [`MAX_INPUT_BYTES`] bytes).
/// - `shell`: auto | bash | sh | dash | zsh. `auto` reads the shebang and falls back to bash.
/// - `min_severity`: all/info | warning | error.
/// - `ignore`: comma/space separated rule codes to suppress.
/// - `format`: text | json.
pub fn lint(
    script: &str,
    shell: &str,
    min_severity: &str,
    ignore: &str,
    format: &str,
) -> Result<String, String> {
    if script.trim().is_empty() {
        return Err("script input is empty".into());
    }
    if script.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "script is too large ({} bytes); the limit is {MAX_INPUT_BYTES} bytes",
            script.len()
        ));
    }
    let requested = parse_shell(shell)?;
    let min = Severity::parse_min(min_severity)?;
    let format = Format::parse(format)?;
    let ignored = parse_ignore(ignore);

    let resolved = if requested == "auto" {
        detect_shell(script)
    } else {
        requested
    };
    let posix = matches!(resolved, "sh" | "dash");

    let masked = mask(script);
    let lines = line_starts(script);

    let mut findings = Vec::new();
    structural_checks(script, &masked, &lines, &mut findings);
    let strict = strict_mode(&masked.code);
    hygiene_checks(script, &masked, &lines, &strict, &mut findings);
    pitfall_checks(script, &masked, &lines, &strict, posix, &mut findings);
    if posix {
        posix_checks(script, &masked, &lines, &mut findings);
    } else {
        legacy_test_check(script, &masked, &lines, &mut findings);
    }

    // One finding per (line, rule): a line with three unquoted expansions is one problem.
    let mut seen = BTreeSet::new();
    findings.retain(|f| seen.insert((f.line, f.code)));
    findings.retain(|f| f.severity >= min && !ignored.contains(f.code));
    findings.sort_by(|a, b| a.line.cmp(&b.line).then(a.code.cmp(b.code)));

    Ok(match format {
        Format::Text => render_text(resolved, &findings),
        Format::Json => render_json(resolved, &findings),
    })
}

fn parse_shell(s: &str) -> Result<&'static str, String> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" | "detect" => "auto",
        "bash" => "bash",
        "sh" | "posix" => "sh",
        "dash" => "dash",
        "zsh" => "zsh",
        other => {
            return Err(format!(
                "unknown shell '{other}' (use auto, bash, sh, dash, or zsh)"
            ))
        }
    })
}

/// Read the shebang to pick a shell; scripts without one are linted as bash.
fn detect_shell(script: &str) -> &'static str {
    let first = script.lines().next().unwrap_or("");
    if !first.starts_with("#!") {
        return "bash";
    }
    let lower = first.to_ascii_lowercase();
    if lower.contains("zsh") {
        "zsh"
    } else if lower.contains("bash") {
        "bash"
    } else if lower.contains("dash") {
        "dash"
    } else if lower.contains("sh") {
        "sh"
    } else {
        "bash"
    }
}

fn parse_ignore(s: &str) -> BTreeSet<&'static str> {
    let mut set = BTreeSet::new();
    for part in s.split(|c: char| c == ',' || c.is_whitespace()) {
        let upper = part.trim().to_ascii_uppercase();
        if upper.is_empty() {
            continue;
        }
        if let Some(code) = RULE_CODES
            .iter()
            .chain(std::iter::once(&SH_RULE_CODE))
            .find(|c| **c == upper)
        {
            set.insert(*code);
        }
    }
    set
}

// ---------------------------------------------------------------------------
// masking
// ---------------------------------------------------------------------------

/// Blank out comments, quoted strings and here-doc bodies while preserving byte
/// offsets and newlines, so a regex match position still maps to the right line.
fn mask(script: &str) -> Masked {
    let b = script.as_bytes();
    let mut code = Vec::with_capacity(b.len());
    let mut code_dq = Vec::with_capacity(b.len());
    let mut unterminated = None;
    // Here-doc terminators queued by `<<WORD` on the current line.
    let mut pending: Vec<String> = Vec::new();
    let mut heredoc: Option<String> = None;
    let mut line = 1usize;
    let mut i = 0usize;

    // Push a masked byte to both variants.
    macro_rules! blank {
        ($c:expr) => {{
            let c = if $c == b'\n' { b'\n' } else { b' ' };
            code.push(c);
            code_dq.push(c);
        }};
    }
    macro_rules! keep {
        ($c:expr) => {{
            code.push($c);
            code_dq.push($c);
        }};
    }

    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            keep!(c);
            i += 1;
            line += 1;
            if heredoc.is_none() && !pending.is_empty() {
                heredoc = Some(pending.remove(0));
            }
            continue;
        }
        if let Some(term) = heredoc.clone() {
            // Consume the whole line; if it is the terminator, leave here-doc mode.
            let end = b[i..].iter().position(|&x| x == b'\n').map(|p| i + p).unwrap_or(b.len());
            let text = &script[i..end];
            if text.trim() == term {
                heredoc = None;
            }
            for &x in &b[i..end] {
                blank!(x);
            }
            i = end;
            continue;
        }
        match c {
            b'\\' => {
                // An escaped byte is never a quote, comment or expansion.
                blank!(b' ');
                i += 1;
                if i < b.len() {
                    if b[i] == b'\n' {
                        keep!(b'\n');
                        line += 1;
                    } else {
                        blank!(b' ');
                    }
                    i += 1;
                }
            }
            b'\'' => {
                let start_line = line;
                blank!(b' ');
                i += 1;
                let mut closed = false;
                while i < b.len() {
                    let x = b[i];
                    blank!(x);
                    if x == b'\n' {
                        line += 1;
                    }
                    i += 1;
                    if x == b'\'' {
                        closed = true;
                        break;
                    }
                }
                if !closed && unterminated.is_none() {
                    unterminated = Some((start_line, '\''));
                }
            }
            b'"' => {
                let start_line = line;
                code.push(b' ');
                code_dq.push(b'"');
                i += 1;
                let mut closed = false;
                while i < b.len() {
                    let x = b[i];
                    if x == b'\\' && i + 1 < b.len() {
                        code.push(b' ');
                        code_dq.push(b' ');
                        code.push(b' ');
                        code_dq.push(b' ');
                        if b[i + 1] == b'\n' {
                            line += 1;
                        }
                        i += 2;
                        continue;
                    }
                    code.push(if x == b'\n' { b'\n' } else { b' ' });
                    code_dq.push(x);
                    if x == b'\n' {
                        line += 1;
                    }
                    i += 1;
                    if x == b'"' {
                        closed = true;
                        break;
                    }
                }
                if !closed && unterminated.is_none() {
                    unterminated = Some((start_line, '"'));
                }
            }
            b'#' => {
                // `#` only starts a comment at the beginning of a word.
                let prev = if i == 0 { b'\n' } else { b[i - 1] };
                if matches!(prev, b'\n' | b' ' | b'\t' | b';' | b'(' | b'&' | b'|') {
                    while i < b.len() && b[i] != b'\n' {
                        blank!(b' ');
                        i += 1;
                    }
                } else {
                    keep!(c);
                    i += 1;
                }
            }
            b'<' if i + 1 < b.len() && b[i + 1] == b'<' && !(i + 2 < b.len() && b[i + 2] == b'<') => {
                keep!(b'<');
                keep!(b'<');
                i += 2;
                if i < b.len() && b[i] == b'-' {
                    keep!(b'-');
                    i += 1;
                }
                while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
                    keep!(b[i]);
                    i += 1;
                }
                let mut word = String::new();
                while i < b.len() {
                    let x = b[i];
                    if x.is_ascii_alphanumeric() || matches!(x, b'_' | b'.' | b'-') {
                        word.push(x as char);
                        keep!(x);
                        i += 1;
                    } else if matches!(x, b'\'' | b'"') {
                        blank!(b' ');
                        i += 1;
                    } else {
                        break;
                    }
                }
                if !word.is_empty() {
                    pending.push(word);
                }
            }
            _ => {
                keep!(c);
                i += 1;
            }
        }
    }

    Masked {
        code: String::from_utf8(code).unwrap_or_default(),
        code_dq: String::from_utf8(code_dq).unwrap_or_default(),
        unterminated,
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn line_starts(script: &str) -> Vec<usize> {
    let mut v = vec![0usize];
    for (i, ch) in script.char_indices() {
        if ch == '\n' {
            v.push(i + 1);
        }
    }
    v
}

fn line_of(starts: &[usize], offset: usize) -> usize {
    match starts.binary_search(&offset) {
        Ok(i) => i + 1,
        Err(i) => i.max(1),
    }
}

fn snippet_at(script: &str, line: usize) -> String {
    let raw = script.lines().nth(line.saturating_sub(1)).unwrap_or("").trim();
    if raw.chars().count() > 120 {
        let cut: String = raw.chars().take(117).collect();
        format!("{cut}...")
    } else {
        raw.to_string()
    }
}

fn push(
    findings: &mut Vec<Finding>,
    script: &str,
    line: usize,
    code: &'static str,
    severity: Severity,
    message: impl Into<String>,
) {
    findings.push(Finding {
        line,
        code,
        severity,
        message: message.into(),
        snippet: snippet_at(script, line),
    });
}

#[derive(Debug, Default, Clone, Copy)]
struct Strict {
    errexit: bool,
    nounset: bool,
    pipefail: bool,
}

impl Strict {
    fn missing(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if !self.errexit {
            v.push("set -e");
        }
        if !self.nounset {
            v.push("set -u");
        }
        if !self.pipefail {
            v.push("set -o pipefail");
        }
        v
    }
}

/// Look for `set -e` / `set -u` / `set -o pipefail` in any of their spellings.
fn strict_mode(code: &str) -> Strict {
    let mut s = Strict::default();
    for raw in code.lines() {
        let line = raw.trim();
        let rest = if let Some(r) = line.strip_prefix("set ") {
            r
        } else if let Some(r) = line.strip_prefix("set\t") {
            r
        } else {
            continue;
        };
        let mut tokens = rest.split_whitespace().peekable();
        while let Some(tok) = tokens.next() {
            if tok == "-o" {
                match tokens.next() {
                    Some("errexit") => s.errexit = true,
                    Some("nounset") => s.nounset = true,
                    Some("pipefail") => s.pipefail = true,
                    _ => {}
                }
            } else if let Some(flags) = tok.strip_prefix('-') {
                // `set -euo pipefail` — the `o` consumes the following word.
                let mut chars = flags.chars().peekable();
                while let Some(f) = chars.next() {
                    match f {
                        'e' => s.errexit = true,
                        'u' => s.nounset = true,
                        'o' => {
                            if chars.peek().is_none() {
                                match tokens.next() {
                                    Some("errexit") => s.errexit = true,
                                    Some("nounset") => s.nounset = true,
                                    Some("pipefail") => s.pipefail = true,
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    s
}

// ---------------------------------------------------------------------------
// rules
// ---------------------------------------------------------------------------

/// Block keywords must nest and close: `if`/`fi`, `do`/`done`, `case`/`esac`.
fn structural_checks(script: &str, masked: &Masked, starts: &[usize], findings: &mut Vec<Finding>) {
    if let Some((line, quote)) = masked.unterminated {
        let kind = if quote == '\'' { "single" } else { "double" };
        push(
            findings,
            script,
            line,
            "SYNTAX",
            Severity::Error,
            format!("unterminated {kind} quote opened here and never closed"),
        );
    }

    let word = Regex::new(r"(?m)\b(if|fi|do|done|case|esac)\b").unwrap();
    let mut stack: Vec<(&str, usize)> = Vec::new();
    for m in word.find_iter(&masked.code) {
        let kw = m.as_str();
        let line = line_of(starts, m.start());
        match kw {
            "if" | "do" | "case" => stack.push((kw, line)),
            closer => {
                let opener = match closer {
                    "fi" => "if",
                    "done" => "do",
                    _ => "case",
                };
                match stack.iter().rposition(|(k, _)| *k == opener) {
                    Some(idx) => {
                        stack.truncate(idx);
                    }
                    None => push(
                        findings,
                        script,
                        line,
                        "SYNTAX",
                        Severity::Error,
                        format!("'{closer}' without a matching '{opener}'"),
                    ),
                }
            }
        }
    }
    for (kw, line) in stack {
        let closer = match kw {
            "if" => "fi",
            "do" => "done",
            _ => "esac",
        };
        push(
            findings,
            script,
            line,
            "SYNTAX",
            Severity::Error,
            format!("'{kw}' block is never closed with '{closer}'"),
        );
    }
}

/// Shebang + strict-mode audit — the two things every script should start with.
fn hygiene_checks(
    script: &str,
    _masked: &Masked,
    _starts: &[usize],
    strict: &Strict,
    findings: &mut Vec<Finding>,
) {
    if !script.starts_with("#!") {
        push(
            findings,
            script,
            1,
            "MISSING-SHEBANG",
            Severity::Warning,
            "no shebang on line 1; add '#!/usr/bin/env bash' (or '#!/bin/sh') so the script runs under a known shell",
        );
    }
    let missing = strict.missing();
    if !missing.is_empty() {
        push(
            findings,
            script,
            1,
            "STRICT-MODE",
            Severity::Warning,
            format!(
                "missing {} — add 'set -euo pipefail' near the top so the script aborts on errors, unset variables, and failed pipeline stages",
                missing.join(", ")
            ),
        );
    }
}

/// The common runtime pitfalls: unquoted expansions, useless cat, backticks,
/// subshell scope loss, unguarded cd, `ls` parsing, spaced assignments, rm -rf.
fn pitfall_checks(
    script: &str,
    masked: &Masked,
    starts: &[usize],
    strict: &Strict,
    posix: bool,
    findings: &mut Vec<Finding>,
) {
    unquoted_var_check(script, masked, starts, posix, findings);

    let useless_cat = Regex::new(r"\bcat\b[^|;&\n<>]*\|[^|]").unwrap();
    for m in useless_cat.find_iter(&masked.code) {
        push(
            findings,
            script,
            line_of(starts, m.start()),
            "USELESS-CAT",
            Severity::Info,
            "useless use of cat: redirect the file into the command ('cmd < file') or pass it as an argument",
        );
    }

    for (idx, ch) in masked.code.char_indices() {
        if ch == '`' {
            push(
                findings,
                script,
                line_of(starts, idx),
                "BACKTICKS",
                Severity::Info,
                "backtick command substitution is legacy syntax; use $( ... ), which nests and quotes predictably",
            );
        }
    }

    let subshell = Regex::new(r"\|\s*(?:while|read)\b").unwrap();
    for m in subshell.find_iter(&masked.code) {
        push(
            findings,
            script,
            line_of(starts, m.start()),
            "SUBSHELL-SCOPE",
            Severity::Warning,
            "the right side of a pipeline runs in a subshell, so variables set inside this loop are lost afterwards; read from a redirect instead: while read -r line; do ...; done < <(cmd)",
        );
    }

    if !strict.errexit {
        let cd = Regex::new(r"(?m)^\s*cd\s+[^\n]*$").unwrap();
        for m in cd.find_iter(&masked.code) {
            let text = m.as_str();
            if text.contains("||") || text.contains("&&") || text.contains(';') {
                continue;
            }
            push(
                findings,
                script,
                line_of(starts, m.start()),
                "UNCHECKED-CD",
                Severity::Warning,
                "unguarded cd: if the directory is missing the script keeps running in the wrong place; write 'cd dir || exit 1' or enable 'set -e'",
            );
        }
    }

    let parse_ls = Regex::new(r"(?:\$\(\s*ls\b|`\s*ls\b)").unwrap();
    for m in parse_ls.find_iter(&masked.code) {
        push(
            findings,
            script,
            line_of(starts, m.start()),
            "PARSE-LS",
            Severity::Info,
            "parsing ls output breaks on spaces and newlines in filenames; iterate a glob ('for f in ./*') or use find -print0 with read -d ''",
        );
    }

    let assign = Regex::new(r"(?m)^[ \t]*([A-Za-z_][A-Za-z0-9_]*)[ \t]+=").unwrap();
    for caps in assign.captures_iter(&masked.code) {
        let m = caps.get(0).unwrap();
        let name = caps.get(1).unwrap().as_str();
        if matches!(name, "test" | "let" | "declare" | "local" | "export" | "readonly") {
            continue;
        }
        push(
            findings,
            script,
            line_of(starts, m.start()),
            "ASSIGN-SPACES",
            Severity::Error,
            format!(
                "spaces around '=' make this a command, not an assignment; write {name}=value with no spaces"
            ),
        );
    }

    rm_risk_check(script, masked, starts, findings);
}

/// `rm -rf` where the target is an expansion (or literally `/`). Runs against the
/// double-quote-preserving mask so `rm -rf "$dir"/` is still visible.
fn rm_risk_check(script: &str, masked: &Masked, starts: &[usize], findings: &mut Vec<Finding>) {
    let rm = Regex::new(r"\brm\s+((?:--?[A-Za-z-]+\s+)+)(\S+)").unwrap();
    for caps in rm.captures_iter(&masked.code_dq) {
        let m = caps.get(0).unwrap();
        let flags = caps.get(1).unwrap().as_str();
        let target = caps.get(2).unwrap().as_str();
        let recursive = flags.contains("--recursive")
            || flags
                .split_whitespace()
                .any(|f| !f.starts_with("--") && f.contains('r') || f.contains('R'));
        let force = flags.contains("--force")
            || flags
                .split_whitespace()
                .any(|f| !f.starts_with("--") && f.contains('f'));
        if !(recursive && force) {
            continue;
        }
        let line = line_of(starts, m.start());
        if target == "/" || target == "/*" {
            push(
                findings,
                script,
                line,
                "RM-RISK",
                Severity::Error,
                "'rm -rf /' deletes the whole filesystem; this is almost certainly not what the script means",
            );
        } else if target.contains('$') {
            push(
                findings,
                script,
                line,
                "RM-RISK",
                Severity::Error,
                format!(
                    "'rm -rf {target}' expands a variable: if it is empty or unset the command deletes far more than intended; guard it with 'set -u' plus [ -n \"$var\" ] and drop the trailing slash"
                ),
            );
        }
    }
}

/// `$name` / `${name}` outside quotes word-splits and glob-expands.
fn unquoted_var_check(
    script: &str,
    masked: &Masked,
    starts: &[usize],
    posix: bool,
    findings: &mut Vec<Finding>,
) {
    let code = &masked.code;
    let bytes = code.as_bytes();
    let expansion = Regex::new(r"\$\{?([A-Za-z_][A-Za-z0-9_]*|[0-9]|@)\}?").unwrap();
    for caps in expansion.captures_iter(code) {
        let m = caps.get(0).unwrap();
        let name = caps.get(1).unwrap().as_str();
        // `$var` directly after `=` is a plain assignment (`x=$y`) — safe in every shell.
        if m.start() > 0 && bytes[m.start() - 1] == b'=' {
            continue;
        }
        // `$(...)` command substitution, `$((...))` arithmetic — not a bare word.
        if m.as_str().starts_with("$(") {
            continue;
        }
        let line = line_of(starts, m.start());
        let line_text = code.lines().nth(line - 1).unwrap_or("");
        // Inside [[ ... ]] and (( ... )) no word splitting happens.
        if !posix && line_text.contains("[[") {
            continue;
        }
        if line_text.contains("((") {
            continue;
        }
        // `case $x in` and `for x in` headers do not split the word being matched.
        let trimmed = line_text.trim_start();
        if trimmed.starts_with("case ") {
            continue;
        }
        let shown = if name == "@" {
            "\"$@\"".to_string()
        } else {
            format!("\"${{{name}}}\"")
        };
        push(
            findings,
            script,
            line,
            "UNQUOTED-VAR",
            Severity::Warning,
            format!(
                "unquoted {} splits on whitespace and expands globs; write {shown} instead",
                m.as_str()
            ),
        );
    }
}

/// bash/zsh only: prefer `[[ ... ]]` over the POSIX `[ ... ]` builtin.
fn legacy_test_check(script: &str, masked: &Masked, starts: &[usize], findings: &mut Vec<Finding>) {
    let single = Regex::new(r"(?m)(?:^|\bif |\bwhile |\belif |&&\s*|\|\|\s*)\[\s").unwrap();
    for m in single.find_iter(&masked.code) {
        let at = m.start() + m.as_str().find('[').unwrap_or(0);
        if masked.code.as_bytes().get(at + 1) == Some(&b'[') {
            continue;
        }
        push(
            findings,
            script,
            line_of(starts, at),
            "LEGACY-TEST",
            Severity::Info,
            "single-bracket [ ... ] is the POSIX test builtin; in bash prefer [[ ... ]], which does not word-split or glob its operands",
        );
    }
}

/// sh/dash only: constructs that only exist in bash/zsh.
fn posix_checks(script: &str, masked: &Masked, starts: &[usize], findings: &mut Vec<Finding>) {
    let checks: [(&str, &str); 5] = [
        (r"\[\[", "[[ ... ]] is a bash keyword; POSIX sh only has [ ... ]"),
        (
            r"(?m)^\s*function\s+[A-Za-z_]",
            "the 'function' keyword is not POSIX; declare functions as 'name() { ... }'",
        ),
        (
            r"(?m)^\s*source\s+\S",
            "'source' is a bashism; POSIX sh uses '. ./file'",
        ),
        (
            r"(?m)^\s*[A-Za-z_][A-Za-z0-9_]*=\(",
            "arrays are not available in POSIX sh; use separate variables or \"$@\"",
        ),
        (
            r"\[[^\[\]\n]*\s==\s",
            "'==' inside [ ... ] is a bashism; POSIX test compares strings with a single '='",
        ),
    ];
    for (pattern, message) in checks {
        let re = Regex::new(pattern).unwrap();
        for m in re.find_iter(&masked.code) {
            push(
                findings,
                script,
                line_of(starts, m.start()),
                SH_RULE_CODE,
                Severity::Warning,
                message,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn counts(findings: &[Finding]) -> (usize, usize, usize) {
    let e = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let w = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let i = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();
    (e, w, i)
}

fn render_text(shell: &str, findings: &[Finding]) -> String {
    let (errors, warnings, info) = counts(findings);
    let mut out = format!(
        "Shell lint ({shell}) · {} findings · {errors} errors · {warnings} warnings · {info} info",
        findings.len()
    );
    if findings.is_empty() {
        out.push_str("\n\nNo issues found.");
        return out;
    }
    for f in findings {
        let _ = write!(
            out,
            "\n\nL{} [{}] {}: {}",
            f.line,
            f.severity.label(),
            f.code,
            f.message
        );
        if !f.snippet.is_empty() {
            let _ = write!(out, "\n  {}", f.snippet);
        }
    }
    out
}

fn render_json(shell: &str, findings: &[Finding]) -> String {
    let (errors, warnings, info) = counts(findings);
    let items: Vec<_> = findings
        .iter()
        .map(|f| {
            json!({
                "line": f.line,
                "code": f.code,
                "severity": f.severity.label(),
                "message": f.message,
                "snippet": f.snippet,
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "shell": shell,
        "summary": { "findings": findings.len(), "errors": errors, "warnings": warnings, "info": info },
        "findings": items,
    }))
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(report: &str) -> Vec<&str> {
        report
            .lines()
            .filter_map(|l| l.split_once("] ").and_then(|(_, r)| r.split_once(": ")))
            .map(|(code, _)| code)
            .collect()
    }

    #[test]
    fn flags_unquoted_variable_and_missing_strict_mode() {
        let script = "#!/usr/bin/env bash\nrm -r $TARGET\n";
        let out = lint(script, "auto", "all", "", "text").unwrap();
        assert!(out.starts_with("Shell lint (bash)"), "{out}");
        assert!(codes(&out).contains(&"UNQUOTED-VAR"), "{out}");
        assert!(codes(&out).contains(&"STRICT-MODE"), "{out}");
        assert!(out.contains("write \"${TARGET}\" instead"), "{out}");
    }

    #[test]
    fn empty_script_is_an_error() {
        let err = lint("   \n ", "auto", "all", "", "text").unwrap_err();
        assert_eq!(err, "script input is empty");
    }

    #[test]
    fn rejects_input_over_the_cap() {
        let big = "e".repeat(MAX_INPUT_BYTES + 1);
        let err = lint(&big, "bash", "all", "", "text").unwrap_err();
        assert!(err.starts_with("script is too large (200001 bytes)"), "{err}");
        let at_cap = "#!/bin/bash\nset -euo pipefail\n".to_string()
            + &"true\n".repeat((MAX_INPUT_BYTES - 30) / 5);
        assert!(at_cap.len() <= MAX_INPUT_BYTES);
        assert!(lint(&at_cap, "bash", "all", "", "text").is_ok());
    }

    #[test]
    fn rejects_unknown_enum_values() {
        assert!(lint("true", "fish", "all", "", "text")
            .unwrap_err()
            .starts_with("unknown shell 'fish'"));
        assert!(lint("true", "bash", "loud", "", "text")
            .unwrap_err()
            .starts_with("unknown min_severity 'loud'"));
        assert!(lint("true", "bash", "all", "", "yaml")
            .unwrap_err()
            .starts_with("unknown format 'yaml'"));
    }

    #[test]
    fn clean_strict_script_has_no_findings() {
        let script = "#!/usr/bin/env bash\nset -euo pipefail\n\nmain() {\n  printf '%s\\n' \"${1:-hi}\"\n}\nmain \"$@\"\n";
        let out = lint(script, "auto", "all", "", "text").unwrap();
        assert!(out.contains("No issues found."), "{out}");
    }

    #[test]
    fn comments_and_single_quotes_are_masked() {
        let script = "#!/bin/bash\nset -euo pipefail\n# rm -rf $HOME and cat x | wc -l\necho 'cat y | wc -l'\n";
        let out = lint(script, "bash", "all", "", "text").unwrap();
        assert!(out.contains("No issues found."), "{out}");
    }

    #[test]
    fn heredoc_bodies_are_masked() {
        let script = "#!/bin/bash\nset -euo pipefail\ncat <<EOF\nrm -rf $HOME\nEOF\n";
        let out = lint(script, "bash", "all", "", "text").unwrap();
        assert!(out.contains("No issues found."), "{out}");
    }

    #[test]
    fn detects_useless_cat_backticks_and_subshell_scope() {
        let script = "#!/bin/bash\nset -euo pipefail\ncat log.txt | grep err\nnow=`date`\nfind . | while read -r f; do n=1; done\n";
        let out = lint(script, "auto", "all", "", "text").unwrap();
        let c = codes(&out);
        assert!(c.contains(&"USELESS-CAT"), "{out}");
        assert!(c.contains(&"BACKTICKS"), "{out}");
        assert!(c.contains(&"SUBSHELL-SCOPE"), "{out}");
    }

    #[test]
    fn detects_structural_block_errors() {
        let script = "#!/bin/bash\nset -euo pipefail\nif [ -f x ]; then\n  echo hi\n";
        let out = lint(script, "bash", "error", "", "text").unwrap();
        assert!(out.contains("'if' block is never closed with 'fi'"), "{out}");
        let extra = lint("#!/bin/bash\nset -euo pipefail\ndone\n", "bash", "error", "", "text").unwrap();
        assert!(extra.contains("'done' without a matching 'do'"), "{extra}");
    }

    #[test]
    fn detects_unterminated_quote() {
        let script = "#!/bin/bash\nset -euo pipefail\necho \"oops\n";
        let out = lint(script, "bash", "error", "", "text").unwrap();
        assert!(out.contains("unterminated double quote"), "{out}");
    }

    #[test]
    fn strict_mode_message_names_only_the_missing_options() {
        let script = "#!/bin/bash\nset -eu\ntrue\n";
        let out = lint(script, "bash", "all", "", "text").unwrap();
        assert!(out.contains("missing set -o pipefail"), "{out}");
        assert!(!out.contains("set -e,"), "{out}");
        let combined = lint("#!/bin/bash\nset -o errexit\nset -o nounset\nset -o pipefail\ntrue\n", "bash", "all", "", "text").unwrap();
        assert!(combined.contains("No issues found."), "{combined}");
    }

    #[test]
    fn unchecked_cd_only_without_errexit() {
        let loose = lint("#!/bin/bash\nset -u\ncd /tmp/build\n", "bash", "all", "", "text").unwrap();
        assert!(codes(&loose).contains(&"UNCHECKED-CD"), "{loose}");
        let strict = lint("#!/bin/bash\nset -euo pipefail\ncd /tmp/build\n", "bash", "all", "", "text").unwrap();
        assert!(!codes(&strict).contains(&"UNCHECKED-CD"), "{strict}");
        let guarded = lint("#!/bin/bash\nset -u\ncd /tmp/build || exit 1\n", "bash", "all", "", "text").unwrap();
        assert!(!codes(&guarded).contains(&"UNCHECKED-CD"), "{guarded}");
    }

    #[test]
    fn detects_assign_spaces_parse_ls_and_rm_risk() {
        let script = "#!/bin/bash\nset -euo pipefail\nCOUNT = 3\nfor f in $(ls); do echo \"$f\"; done\nrm -rf \"$BUILD\"/\n";
        let out = lint(script, "bash", "all", "", "text").unwrap();
        let c = codes(&out);
        assert!(c.contains(&"ASSIGN-SPACES"), "{out}");
        assert!(c.contains(&"PARSE-LS"), "{out}");
        assert!(c.contains(&"RM-RISK"), "{out}");
    }

    #[test]
    fn posix_shell_flags_bashisms_but_bash_does_not() {
        let script = "#!/bin/sh\nset -eu\nif [[ -f x ]]; then\n  source ./lib.sh\nfi\n";
        let sh = lint(script, "auto", "all", "", "text").unwrap();
        assert!(sh.starts_with("Shell lint (sh)"), "{sh}");
        assert!(codes(&sh).contains(&"SH-BASHISM"), "{sh}");
        let bash = lint(script, "bash", "all", "", "text").unwrap();
        assert!(!codes(&bash).contains(&"SH-BASHISM"), "{bash}");
    }

    #[test]
    fn legacy_test_is_bash_only() {
        let script = "#!/bin/bash\nset -euo pipefail\nif [ -f x ]; then\n  true\nfi\n";
        let bash = lint(script, "bash", "all", "", "text").unwrap();
        assert!(codes(&bash).contains(&"LEGACY-TEST"), "{bash}");
        let sh = lint("#!/bin/sh\nset -eu\nif [ -f x ]; then\n  true\nfi\n", "auto", "all", "", "text").unwrap();
        assert!(!codes(&sh).contains(&"LEGACY-TEST"), "{sh}");
    }

    #[test]
    fn zsh_is_detected_and_reported() {
        let out = lint("#!/bin/zsh\nset -euo pipefail\ntrue\n", "auto", "all", "", "text").unwrap();
        assert!(out.starts_with("Shell lint (zsh)"), "{out}");
        let dash = lint("#!/bin/bash\nset -eu\ntrue\n", "dash", "all", "", "text").unwrap();
        assert!(dash.starts_with("Shell lint (dash)"), "{dash}");
    }

    #[test]
    fn severity_filter_and_ignore_list_apply() {
        let script = "cat a | wc -l\n";
        let all = lint(script, "bash", "all", "", "text").unwrap();
        assert!(codes(&all).contains(&"USELESS-CAT"), "{all}");
        let warn = lint(script, "bash", "warning", "", "text").unwrap();
        assert!(!codes(&warn).contains(&"USELESS-CAT"), "{warn}");
        assert!(codes(&warn).contains(&"MISSING-SHEBANG"), "{warn}");
        let ignored = lint(script, "bash", "all", "USELESS-CAT, MISSING-SHEBANG", "text").unwrap();
        assert!(!codes(&ignored).contains(&"USELESS-CAT"), "{ignored}");
        assert!(!codes(&ignored).contains(&"MISSING-SHEBANG"), "{ignored}");
        let errors_only = lint(script, "bash", "error", "", "text").unwrap();
        assert!(errors_only.contains("· 0 findings ·"), "{errors_only}");
    }

    #[test]
    fn json_format_reports_summary_and_findings() {
        let out = lint("cat a | wc -l\n", "bash", "all", "", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["shell"], "bash");
        assert_eq!(v["summary"]["warnings"], 2);
        assert_eq!(v["summary"]["info"], 1);
        let first = &v["findings"][0];
        assert_eq!(first["line"], 1);
        assert_eq!(first["severity"], "warning");
        assert_eq!(first["snippet"], "cat a | wc -l");
    }

    #[test]
    fn one_finding_per_rule_per_line() {
        let out = lint("#!/bin/bash\nset -euo pipefail\necho $A $B $C\n", "bash", "all", "", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["findings"], 1);
        assert_eq!(v["findings"][0]["code"], "UNQUOTED-VAR");
    }
}
