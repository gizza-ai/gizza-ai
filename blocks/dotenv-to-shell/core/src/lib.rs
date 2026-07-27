//! dotenv-to-shell core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Converts a `.env` file into `export`-prefixed shell statements (and back),
//! handling quoting and special characters so the output is safe to `source`
//! / `eval`. Two directions:
//!
//! - `to-shell` — `.env` (`KEY=VALUE`) → shell statements for a target shell
//!   dialect (`posix`/`bash` → `export KEY=value`; `fish` → `set -gx KEY value`).
//!   Values are quoted with POSIX/fish single-quote rules so `$`, backticks,
//!   spaces, `#`, quotes and newlines stay literal. `quote=auto` leaves safe
//!   values bare; `quote=single` always quotes.
//! - `to-env` — shell `export`/`set -gx`/`setenv` statements → a plain `.env`
//!   file (`KEY=value`, dotenv-quoted only when needed).
//!
//! Full-line `#` comments and blank lines are preserved (both syntaxes use `#`).
//! Keys that aren't valid shell identifiers are skipped with a `# skipped …` note
//! rather than emitting invalid syntax.

/// Convert between a `.env` file and shell export statements.
///
/// - `input`: the source text.
/// - `direction`: `"to-shell"` (default) or `"to-env"`.
/// - `shell`: target dialect for `to-shell` — `"posix"` (default), `"bash"`
///   (identical to posix), or `"fish"`. Ignored for `to-env`.
/// - `quote`: value quoting for `to-shell` — `"auto"` (default, bareword when
///   safe) or `"single"` (always single-quote). Ignored for `to-env`.
///
/// Returns `Err` only for an unknown `direction`, `shell`, or `quote` value.
pub fn convert(input: &str, direction: &str, shell: &str, quote: &str) -> Result<String, String> {
    let direction = if direction.trim().is_empty() {
        "to-shell"
    } else {
        direction.trim()
    };
    match direction {
        "to-shell" => to_shell(input, shell, quote),
        "to-env" => to_env(input),
        other => Err(format!(
            "invalid direction {other:?}: expected \"to-shell\" or \"to-env\""
        )),
    }
}

/// Target shell dialect for `to-shell`.
#[derive(Clone, Copy, PartialEq)]
enum Shell {
    /// POSIX / bash / zsh — `export KEY=value`.
    Posix,
    /// fish — `set -gx KEY value`.
    Fish,
}

fn parse_shell(shell: &str) -> Result<Shell, String> {
    match if shell.trim().is_empty() {
        "posix"
    } else {
        shell.trim()
    } {
        "posix" | "bash" => Ok(Shell::Posix),
        "fish" => Ok(Shell::Fish),
        other => Err(format!(
            "invalid shell {other:?}: expected \"posix\", \"bash\" or \"fish\""
        )),
    }
}

/// `.env` → shell statements.
fn to_shell(input: &str, shell: &str, quote: &str) -> Result<String, String> {
    let shell = parse_shell(shell)?;
    let always_quote = match if quote.trim().is_empty() {
        "auto"
    } else {
        quote.trim()
    } {
        "auto" => false,
        "single" => true,
        other => {
            return Err(format!(
                "invalid quote {other:?}: expected \"auto\" or \"single\""
            ))
        }
    };

    let mut out: Vec<String> = Vec::new();
    for raw in input.lines() {
        let trimmed = raw.trim();
        // Preserve blank lines and full-line comments verbatim (both `.env` and
        // shell use `#` for comments).
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push(raw.to_string());
            continue;
        }
        let (key, value) = match parse_env_line(trimmed) {
            Some(kv) => kv,
            // A line with no `=` isn't a valid assignment — flag it, don't drop it.
            None => {
                out.push(format!("# skipped (no '='): {trimmed}"));
                continue;
            }
        };
        if !is_valid_identifier(&key) {
            out.push(format!("# skipped {key:?}: not a valid shell variable name"));
            continue;
        }
        let quoted = match shell {
            Shell::Posix => posix_quote(&value, always_quote),
            Shell::Fish => fish_quote(&value, always_quote),
        };
        let stmt = match shell {
            Shell::Posix => format!("export {key}={quoted}"),
            Shell::Fish => format!("set -gx {key} {quoted}"),
        };
        out.push(stmt);
    }
    Ok(out.join("\n"))
}

/// shell statements → `.env`.
fn to_env(input: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    for raw in input.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push(raw.to_string());
            continue;
        }
        match parse_shell_assignment(trimmed) {
            Some((key, value)) => {
                out.push(format!("{key}={}", env_quote(&value)));
            }
            None => out.push(format!("# skipped: {trimmed}")),
        }
    }
    Ok(out.join("\n"))
}

/// Parse a `.env` assignment line (already trimmed, non-comment, non-blank) into
/// `(key, value)`. Strips an optional `export ` prefix, honors single/double
/// quotes, and drops an inline `# comment` from an unquoted value. Returns `None`
/// if there is no `=`.
fn parse_env_line(line: &str) -> Option<(String, String)> {
    let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
    let eq = line.find('=')?;
    let key = line[..eq].trim().to_string();
    let rest = line[eq + 1..].trim_start();
    let value = parse_env_value(rest);
    Some((key, value))
}

/// Interpret the right-hand side of a `.env` assignment.
fn parse_env_value(rest: &str) -> String {
    let bytes = rest.as_bytes();
    if bytes.first() == Some(&b'\'') {
        // Single-quoted: literal until the closing quote (no escapes in dotenv).
        if let Some(end) = rest[1..].find('\'') {
            return rest[1..1 + end].to_string();
        }
        // Unterminated — take the remainder literally.
        return rest[1..].to_string();
    }
    if bytes.first() == Some(&b'"') {
        // Double-quoted: unescape common backslash sequences.
        if let Some(end) = find_closing_double(&rest[1..]) {
            return unescape_double(&rest[1..1 + end]);
        }
        return unescape_double(&rest[1..]);
    }
    // Unquoted: strip an inline comment (space then `#`) and trailing whitespace.
    let mut val = rest;
    if let Some(pos) = find_inline_comment(rest) {
        val = &rest[..pos];
    }
    val.trim_end().to_string()
}

/// Byte index of the closing unescaped `"` in a double-quoted body (UTF-8 safe:
/// a `\` escapes the very next char, whatever its width).
fn find_closing_double(s: &str) -> Option<usize> {
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next(); // skip the escaped char (any width)
            }
            '"' => return Some(i),
            _ => {}
        }
    }
    None
}

fn unescape_double(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('$') => out.push('$'),
                Some('`') => out.push('`'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Position of an inline `#` comment in an unquoted value: a `#` that is at the
/// start or preceded by whitespace.
fn find_inline_comment(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            return Some(i);
        }
    }
    None
}

/// Parse a shell assignment statement into `(key, value)`. Handles
/// `export KEY=…`, bare `KEY=…`, `set -gx KEY …` (fish) and `setenv KEY …`
/// (csh). Returns `None` if it doesn't look like an assignment.
fn parse_shell_assignment(line: &str) -> Option<(String, String)> {
    // fish: `set -gx KEY value` (also `set -x`, `set --global --export`).
    if let Some(rest) = line.strip_prefix("set ") {
        let rest = rest.trim_start();
        // Drop leading option flags (`-gx`, `--export`, …).
        let mut remainder = rest;
        while remainder.starts_with('-') {
            let end = remainder.find(char::is_whitespace)?;
            remainder = remainder[end..].trim_start();
        }
        let sp = remainder.find(char::is_whitespace)?;
        let key = remainder[..sp].to_string();
        let value = unquote_shell(remainder[sp..].trim());
        if !is_valid_identifier(&key) {
            return None;
        }
        return Some((key, value));
    }
    // csh: `setenv KEY value`.
    if let Some(rest) = line.strip_prefix("setenv ") {
        let rest = rest.trim_start();
        let sp = rest.find(char::is_whitespace)?;
        let key = rest[..sp].to_string();
        let value = unquote_shell(rest[sp..].trim());
        if !is_valid_identifier(&key) {
            return None;
        }
        return Some((key, value));
    }
    // posix/bash: `export KEY=value` or bare `KEY=value`.
    let assign = line.strip_prefix("export ").unwrap_or(line).trim_start();
    let eq = assign.find('=')?;
    let key = assign[..eq].trim().to_string();
    if !is_valid_identifier(&key) {
        return None;
    }
    let value = unquote_shell(assign[eq + 1..].trim());
    Some((key, value))
}

/// Remove one layer of shell quoting from a value token (single, double, or
/// bareword). Understands POSIX `'\''` single-quote splicing.
fn unquote_shell(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.first() == Some(&b'\'') {
        return unquote_single(s);
    }
    if bytes.first() == Some(&b'"') {
        if let Some(end) = find_closing_double(&s[1..]) {
            return unescape_double(&s[1..1 + end]);
        }
        return unescape_double(&s[1..]);
    }
    // fish single-quote escaping (`\'`, `\\`) or a plain bareword.
    if bytes.first() == Some(&b'\\') || s.contains("\\'") {
        return unescape_fish_single(s);
    }
    s.to_string()
}

/// POSIX single-quoted string, honoring the `'\''` splice: `'a'\''b'` → `a'b`.
/// UTF-8 safe (scans by char; the quote/backslash markers are all ASCII).
fn unquote_single(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    let mut in_quote = false;
    while let Some(c) = chars.next() {
        if in_quote {
            if c == '\'' {
                in_quote = false; // end of a single-quoted run
            } else {
                out.push(c);
            }
        } else if c == '\'' {
            in_quote = true; // start of a single-quoted run
        } else if c == '\\' {
            // `\'` or `\\` spliced between quoted runs — the next char is literal.
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// fish single-quoted string: only `\\` and `\'` are escapes.
fn unescape_fish_single(s: &str) -> String {
    let inner = s
        .strip_prefix('\'')
        .and_then(|x| x.strip_suffix('\''))
        .unwrap_or(s);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('\'') => out.push('\''),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A valid POSIX shell variable name: `[A-Za-z_][A-Za-z0-9_]*`.
fn is_valid_identifier(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Is a value safe to emit unquoted in POSIX/fish? Non-empty and only made of
/// characters with no special meaning in either shell.
fn is_safe_bareword(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'_' | b'@' | b'%' | b'+' | b'=' | b':' | b',' | b'.' | b'/' | b'-'
                )
        })
}

/// POSIX single-quote a value: wrap in `'…'`, splicing embedded single quotes as
/// `'\''`. `$`, backticks, spaces, `#`, newlines all stay literal.
fn posix_quote(value: &str, always: bool) -> String {
    if !always && is_safe_bareword(value) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// fish single-quote a value: wrap in `'…'`, escaping `\` and `'`. fish does not
/// expand `$` inside single quotes, so it stays literal.
fn fish_quote(value: &str, always: bool) -> String {
    if !always && is_safe_bareword(value) {
        return value.to_string();
    }
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// dotenv-quote a value for `.env` output: bareword when clean, else double-quote
/// with `\n`/`\t`/`\r`/`\"`/`\\` escapes (the dotenv convention).
fn env_quote(value: &str) -> String {
    let needs = value.is_empty()
        || value.contains(char::is_whitespace)
        || value.contains('#')
        || value.contains('"')
        || value.contains('\'')
        || value.contains('$');
    if !needs {
        return value.to_string();
    }
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_shell_posix_auto_quotes_only_when_needed() {
        let env = "DB_HOST=localhost\nAPI_TOKEN=abc123\nGREETING=hello world";
        let out = convert(env, "to-shell", "posix", "auto").unwrap();
        assert_eq!(
            out,
            "export DB_HOST=localhost\nexport API_TOKEN=abc123\nexport GREETING='hello world'"
        );
    }

    #[test]
    fn to_shell_preserves_special_chars_literally() {
        // `$`, backtick and `#` must NOT be interpreted when sourced.
        let env = "PASSWORD=p@ss w#rd$`x`";
        let out = convert(env, "to-shell", "posix", "auto").unwrap();
        assert_eq!(out, "export PASSWORD='p@ss w#rd$`x`'");
    }

    #[test]
    fn to_shell_escapes_embedded_single_quote() {
        let env = "MSG=it's fine";
        let out = convert(env, "to-shell", "posix", "auto").unwrap();
        assert_eq!(out, r#"export MSG='it'\''s fine'"#);
    }

    #[test]
    fn to_shell_strips_export_prefix_and_inline_comment() {
        let env = "export PORT=8080 # the http port";
        let out = convert(env, "to-shell", "posix", "auto").unwrap();
        assert_eq!(out, "export PORT=8080");
    }

    #[test]
    fn to_shell_handles_dotenv_double_quoted_newlines() {
        let env = r#"KEY="line1\nline2""#;
        let out = convert(env, "to-shell", "posix", "auto").unwrap();
        assert_eq!(out, "export KEY='line1\nline2'");
    }

    #[test]
    fn to_shell_single_mode_always_quotes() {
        let env = "A=b";
        let out = convert(env, "to-shell", "posix", "single").unwrap();
        assert_eq!(out, "export A='b'");
    }

    #[test]
    fn to_shell_fish_uses_set_gx_and_fish_escaping() {
        let env = "MSG=it's fine\nPATHY=c:\\tmp";
        let out = convert(env, "to-shell", "fish", "single").unwrap();
        assert_eq!(out, "set -gx MSG 'it\\'s fine'\nset -gx PATHY 'c:\\\\tmp'");
    }

    #[test]
    fn to_shell_preserves_comments_and_blanks() {
        let env = "# header\n\nA=1";
        let out = convert(env, "to-shell", "posix", "auto").unwrap();
        assert_eq!(out, "# header\n\nexport A=1");
    }

    #[test]
    fn to_shell_flags_invalid_identifier() {
        let env = "1BAD=x\nGOOD=y";
        let out = convert(env, "to-shell", "posix", "auto").unwrap();
        assert_eq!(
            out,
            "# skipped \"1BAD\": not a valid shell variable name\nexport GOOD=y"
        );
    }

    #[test]
    fn to_env_parses_export_statements_back() {
        let sh = "export DB_HOST=localhost\nexport GREETING='hello world'";
        let out = convert(sh, "to-env", "", "").unwrap();
        assert_eq!(out, "DB_HOST=localhost\nGREETING=\"hello world\"");
    }

    #[test]
    fn to_env_roundtrips_posix_single_quote_splice() {
        let sh = r#"export MSG='it'\''s fine'"#;
        let out = convert(sh, "to-env", "", "").unwrap();
        assert_eq!(out, "MSG=\"it's fine\"");
    }

    #[test]
    fn to_env_parses_fish_and_setenv() {
        let sh = "set -gx PORT 8080\nsetenv NAME 'a b'";
        let out = convert(sh, "to-env", "", "").unwrap();
        assert_eq!(out, "PORT=8080\nNAME=\"a b\"");
    }

    #[test]
    fn roundtrip_env_to_shell_to_env() {
        let env = "A=1\nGREETING=hello world\nMSG=it's fine";
        let shell = convert(env, "to-shell", "posix", "auto").unwrap();
        let back = convert(&shell, "to-env", "", "").unwrap();
        assert_eq!(back, "A=1\nGREETING=\"hello world\"\nMSG=\"it's fine\"");
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert_eq!(convert("", "to-shell", "posix", "auto").unwrap(), "");
    }

    #[test]
    fn invalid_direction_errors() {
        let err = convert("A=1", "sideways", "posix", "auto").unwrap_err();
        assert!(err.contains("invalid direction"), "{err}");
    }

    #[test]
    fn invalid_shell_errors() {
        let err = convert("A=1", "to-shell", "tcsh", "auto").unwrap_err();
        assert!(err.contains("invalid shell"), "{err}");
    }

    #[test]
    fn invalid_quote_errors() {
        let err = convert("A=1", "to-shell", "posix", "double").unwrap_err();
        assert!(err.contains("invalid quote"), "{err}");
    }
}
