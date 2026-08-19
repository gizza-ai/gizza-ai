//! strip-console-logs core — remove `console.*` debug statements from JavaScript or
//! TypeScript source. Pure compute, shared by the chat skill block and the web page.
//!
//! The scanner is token-aware: strings, template literals, regular expressions and
//! comments are skipped, so a `console.log` written inside a string or a comment is
//! never touched. A call is only removed where deleting it cannot change program
//! structure — see [`Position`].

/// Largest accepted source, in characters.
pub const MAX_INPUT: usize = 500_000;

/// The console methods this tool recognises by name (canonical spelling).
pub const CONSOLE_METHODS: [&str; 22] = [
    "assert",
    "clear",
    "count",
    "countReset",
    "debug",
    "dir",
    "dirxml",
    "error",
    "group",
    "groupCollapsed",
    "groupEnd",
    "info",
    "log",
    "profile",
    "profileEnd",
    "table",
    "time",
    "timeEnd",
    "timeLog",
    "timeStamp",
    "trace",
    "warn",
];

/// Receiver objects a `console` reference may hang off (`window.console.log(x)`).
const RECEIVERS: [&str; 4] = ["window", "globalThis", "self", "global"];

/// Keywords after which a `/` starts a regular expression rather than a division.
const REGEX_AFTER: [&str; 14] = [
    "return",
    "typeof",
    "instanceof",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "do",
    "else",
    "case",
    "yield",
    "await",
    "throw",
];

/// Keywords that cannot end a statement, so a following newline does NOT insert a
/// semicolon — a `console` call after one of these is an operand, not a statement.
const NO_ASI_AFTER: [&str; 20] = [
    "typeof",
    "instanceof",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "await",
    "yield",
    "throw",
    "case",
    "default",
    "extends",
    "export",
    "as",
    "from",
    "let",
    "const",
    "var",
    "class",
];

/// What to do with a matched statement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Delete the statement (and its line, when the line holds nothing else).
    Remove,
    /// Comment the statement out, keeping it readable in the diff.
    Comment,
    /// Replace it with blank lines so every later line keeps its number.
    Blank,
}

impl Action {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "remove" => Ok(Action::Remove),
            "comment" => Ok(Action::Comment),
            "blank" => Ok(Action::Blank),
            other => Err(format!(
                "unknown action '{other}'; expected one of: remove, comment, blank"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Action::Remove => "remove",
            Action::Comment => "comment",
            Action::Blank => "blank",
        }
    }
}

/// Which output the caller wants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// The rewritten source.
    Code,
    /// A dry-run listing: nothing is rewritten, everything is reported.
    Report,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "code" => Ok(Output::Code),
            "report" => Ok(Output::Report),
            other => Err(format!(
                "unknown output '{other}'; expected one of: code, report"
            )),
        }
    }
}

/// Where a matched call sits in the program.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Position {
    /// A standalone statement — safe to delete outright.
    Statement,
    /// The un-braced body of `if` / `for` / `while` / `else` / `do` — must leave a `;`.
    ControlBody,
    /// Used as a value (assigned, an operand, an arrow body, chained) — left alone.
    Expression,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Word,
    Num,
    Str,
    Regex,
    Punct,
}

#[derive(Clone, Debug)]
struct Tok {
    start: usize,
    end: usize,
    kind: Kind,
    ch: char,
    nl_before: bool,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

fn is_ident_part(c: char) -> bool {
    is_ident_start(c) || c.is_numeric()
}

fn scan_quoted(chars: &[char], i: usize, quote: char) -> usize {
    let n = chars.len();
    let mut j = i + 1;
    while j < n {
        match chars[j] {
            '\\' => j += 2,
            '\n' => return j, // unterminated literal: stop at the line end
            c if c == quote => return j + 1,
            _ => j += 1,
        }
    }
    n
}

fn scan_template(chars: &[char], i: usize, depth: u32) -> usize {
    let n = chars.len();
    let mut j = i + 1;
    while j < n {
        match chars[j] {
            '\\' => j += 2,
            '`' => return j + 1,
            '$' if j + 1 < n && chars[j + 1] == '{' && depth < 32 => {
                j = scan_template_expr(chars, j + 2, depth + 1);
            }
            _ => j += 1,
        }
    }
    n
}

fn scan_template_expr(chars: &[char], i: usize, depth: u32) -> usize {
    let n = chars.len();
    let mut j = i;
    let mut braces = 1usize;
    while j < n {
        let c = chars[j];
        match c {
            '{' => {
                braces += 1;
                j += 1;
            }
            '}' => {
                braces -= 1;
                j += 1;
                if braces == 0 {
                    return j;
                }
            }
            '\'' | '"' => j = scan_quoted(chars, j, c),
            '`' => j = scan_template(chars, j, depth),
            '/' if j + 1 < n && chars[j + 1] == '/' => {
                while j < n && chars[j] != '\n' {
                    j += 1;
                }
            }
            '/' if j + 1 < n && chars[j + 1] == '*' => {
                j = scan_block_comment(chars, j).0;
            }
            _ => j += 1,
        }
    }
    n
}

fn scan_block_comment(chars: &[char], i: usize) -> (usize, bool) {
    let n = chars.len();
    let mut j = i + 2;
    let mut nl = false;
    while j < n {
        if chars[j] == '\n' {
            nl = true;
        }
        if chars[j] == '*' && j + 1 < n && chars[j + 1] == '/' {
            return (j + 2, nl);
        }
        j += 1;
    }
    (n, nl)
}

fn scan_regex(chars: &[char], i: usize) -> Option<usize> {
    let n = chars.len();
    let mut j = i + 1;
    let mut in_class = false;
    while j < n {
        match chars[j] {
            '\\' => j += 2,
            '\n' => return None, // regex literals cannot span lines
            '[' => {
                in_class = true;
                j += 1;
            }
            ']' => {
                in_class = false;
                j += 1;
            }
            '/' if !in_class => {
                j += 1;
                while j < n && is_ident_part(chars[j]) {
                    j += 1;
                }
                return Some(j);
            }
            _ => j += 1,
        }
    }
    None
}

fn regex_allowed(chars: &[char], toks: &[Tok]) -> bool {
    match toks.last() {
        None => true,
        Some(t) => match t.kind {
            Kind::Num | Kind::Str | Kind::Regex => false,
            Kind::Word => REGEX_AFTER.contains(&tok_text(chars, t).as_str()),
            Kind::Punct => !matches!(t.ch, ')' | ']'),
        },
    }
}

fn tok_text(chars: &[char], t: &Tok) -> String {
    chars[t.start..t.end].iter().collect()
}

fn tokenize(chars: &[char]) -> Vec<Tok> {
    let n = chars.len();
    let mut toks: Vec<Tok> = Vec::new();
    let mut i = 0usize;
    let mut nl = false;
    while i < n {
        let c = chars[i];
        if c == '\n' {
            nl = true;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            let (j, had_nl) = scan_block_comment(chars, i);
            nl = nl || had_nl;
            i = j;
            continue;
        }
        let start = i;
        let (kind, end, ch) = if c == '/' {
            match regex_allowed(chars, &toks).then(|| scan_regex(chars, i)).flatten() {
                Some(j) => (Kind::Regex, j, '\0'),
                None => (Kind::Punct, i + 1, '/'),
            }
        } else if c == '"' || c == '\'' {
            (Kind::Str, scan_quoted(chars, i, c), '\0')
        } else if c == '`' {
            (Kind::Str, scan_template(chars, i, 0), '\0')
        } else if is_ident_start(c) {
            let mut j = i + 1;
            while j < n && is_ident_part(chars[j]) {
                j += 1;
            }
            (Kind::Word, j, '\0')
        } else if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < n && (is_ident_part(chars[j]) || chars[j] == '.') {
                j += 1;
            }
            (Kind::Num, j, '\0')
        } else {
            (Kind::Punct, i + 1, c)
        };
        toks.push(Tok { start, end, kind, ch, nl_before: nl });
        nl = false;
        i = end.max(start + 1);
    }
    toks
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Hit {
    start: usize,
    end: usize,
    method: String,
    position: Position,
}

fn is_punct(toks: &[Tok], idx: usize, ch: char) -> bool {
    toks.get(idx).is_some_and(|t| t.kind == Kind::Punct && t.ch == ch)
}

/// Map every `(` token index to its `)` and back.
fn paren_pairs(toks: &[Tok]) -> (Vec<usize>, Vec<usize>) {
    let none = usize::MAX;
    let mut open_to_close = vec![none; toks.len()];
    let mut close_to_open = vec![none; toks.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if t.kind != Kind::Punct {
            continue;
        }
        if t.ch == '(' {
            stack.push(i);
        } else if t.ch == ')' {
            if let Some(o) = stack.pop() {
                open_to_close[o] = i;
                close_to_open[i] = o;
            }
        }
    }
    (open_to_close, close_to_open)
}

/// Decide whether the token at `idx` begins a statement, a control body, or an expression.
fn classify(chars: &[char], toks: &[Tok], idx: usize, close_to_open: &[usize]) -> Position {
    let Some(prev_idx) = idx.checked_sub(1) else {
        return Position::Statement;
    };
    let Some(prev) = toks.get(prev_idx) else {
        return Position::Statement;
    };
    let nl = toks[idx].nl_before;
    match prev.kind {
        Kind::Punct => match prev.ch {
            ';' | '{' | '}' => Position::Statement,
            ')' => {
                let open = close_to_open[prev_idx];
                if open == usize::MAX {
                    return Position::Expression;
                }
                let head = open
                    .checked_sub(1)
                    .and_then(|h| toks.get(h))
                    .filter(|t| t.kind == Kind::Word)
                    .map(|t| tok_text(chars, t));
                match head.as_deref() {
                    Some("if") | Some("for") | Some("while") => Position::ControlBody,
                    _ if nl => Position::Statement,
                    _ => Position::Expression,
                }
            }
            ']' if nl => Position::Statement,
            ':' => colon_position(chars, toks, prev_idx),
            _ => Position::Expression,
        },
        Kind::Word => {
            let w = tok_text(chars, prev);
            match w.as_str() {
                "else" | "do" => Position::ControlBody,
                "return" if nl => Position::Statement,
                _ if NO_ASI_AFTER.contains(&w.as_str()) => Position::Expression,
                _ if nl => Position::Statement,
                _ => Position::Expression,
            }
        }
        Kind::Num | Kind::Str | Kind::Regex if nl => Position::Statement,
        _ => Position::Expression,
    }
}

/// A `:` before the call is a statement boundary only for `case x:` / `default:`.
fn colon_position(chars: &[char], toks: &[Tok], colon_idx: usize) -> Position {
    let mut j = colon_idx;
    let mut steps = 0;
    while j > 0 && steps < 64 {
        j -= 1;
        steps += 1;
        let t = &toks[j];
        if t.kind == Kind::Word {
            let w = tok_text(chars, t);
            if w == "case" || w == "default" {
                return Position::Statement;
            }
            continue;
        }
        if t.kind == Kind::Punct && matches!(t.ch, '?' | '{' | ',' | ';' | '}' | '(' | '[') {
            return Position::Expression;
        }
    }
    Position::Expression
}

/// True when nothing after `close_idx` continues the expression.
fn ends_statement(toks: &[Tok], close_idx: usize) -> bool {
    match toks.get(close_idx + 1) {
        None => true,
        Some(t) => t.nl_before || (t.kind == Kind::Punct && t.ch == '}'),
    }
}

fn find_hits(
    chars: &[char],
    toks: &[Tok],
    wanted: &dyn Fn(&str) -> bool,
    remove_debugger: bool,
) -> Vec<Hit> {
    let (open_to_close, close_to_open) = paren_pairs(toks);
    let mut hits = Vec::new();
    let mut t = 0usize;
    while t < toks.len() {
        if toks[t].kind != Kind::Word {
            t += 1;
            continue;
        }
        let word = tok_text(chars, &toks[t]);

        if remove_debugger && word == "debugger" {
            let mut end_tok = t;
            let mut complete = true;
            if is_punct(toks, t + 1, ';') {
                end_tok = t + 1;
            } else {
                complete = ends_statement(toks, t);
            }
            if complete {
                let position = classify(chars, toks, t, &close_to_open);
                if position != Position::Expression {
                    hits.push(Hit {
                        start: toks[t].start,
                        end: toks[end_tok].end,
                        method: "debugger".to_string(),
                        position,
                    });
                    t = end_tok + 1;
                    continue;
                }
            }
            t += 1;
            continue;
        }

        if word != "console" {
            t += 1;
            continue;
        }

        // Optional `window.` / `globalThis.` / `self.` / `global.` receiver.
        let mut start_tok = t;
        if t >= 2
            && is_punct(toks, t - 1, '.')
            && toks[t - 2].kind == Kind::Word
            && RECEIVERS.contains(&tok_text(chars, &toks[t - 2]).as_str())
        {
            start_tok = t - 2;
        }

        let mut k = t + 1;
        if is_punct(toks, k, '?') && is_punct(toks, k + 1, '.') {
            k += 2;
        } else if is_punct(toks, k, '.') {
            k += 1;
        } else {
            t += 1;
            continue;
        }
        let Some(method_tok) = toks.get(k).filter(|x| x.kind == Kind::Word) else {
            t += 1;
            continue;
        };
        let method = tok_text(chars, method_tok);
        k += 1;
        if is_punct(toks, k, '?') && is_punct(toks, k + 1, '.') {
            k += 2;
        }
        if !is_punct(toks, k, '(') {
            t += 1;
            continue;
        }
        let close = open_to_close[k];
        if close == usize::MAX {
            t += 1;
            continue;
        }

        let mut end_tok = close;
        let complete = if is_punct(toks, close + 1, ';') {
            end_tok = close + 1;
            true
        } else {
            ends_statement(toks, close)
        };
        if !complete {
            t = close + 1;
            continue;
        }
        if !wanted(&method) {
            t = end_tok + 1;
            continue;
        }
        let position = classify(chars, toks, start_tok, &close_to_open);
        hits.push(Hit {
            start: toks[start_tok].start,
            end: toks[end_tok].end,
            method,
            position,
        });
        t = end_tok + 1;
    }
    hits
}

// ---------------------------------------------------------------------------
// Rewriting
// ---------------------------------------------------------------------------

fn line_starts(chars: &[char]) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, c) in chars.iter().enumerate() {
        if *c == '\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn line_of(starts: &[usize], idx: usize) -> usize {
    match starts.binary_search(&idx) {
        Ok(l) => l,
        Err(l) => l - 1,
    }
}

fn line_end(chars: &[char], starts: &[usize], line: usize) -> usize {
    let next = starts.get(line + 1).copied().unwrap_or(chars.len() + 1);
    (next - 1).min(chars.len())
}

fn collapse(chars: &[char], start: usize, end: usize) -> String {
    let raw: String = chars[start..end].iter().collect();
    let mut out = String::new();
    let mut space = false;
    for c in raw.chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    out.replace("*/", "* /")
}

fn comment_lines(chars: &[char], from: usize, to: usize) -> String {
    let block: String = chars[from..to].iter().collect();
    block
        .split('\n')
        .map(|line| {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            let rest = &line[indent.len()..];
            if rest.is_empty() {
                line.to_string()
            } else {
                format!("{indent}// {rest}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn newlines(chars: &[char], from: usize, to: usize) -> String {
    "\n".repeat(chars[from..to].iter().filter(|c| **c == '\n').count())
}

/// Build the rewritten source from the statement hits.
fn rewrite(chars: &[char], hits: &[Hit], action: Action) -> String {
    let starts = line_starts(chars);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    // Control bodies are always patched inline so the control statement keeps a body.
    for h in hits.iter().filter(|h| h.position == Position::ControlBody) {
        let repl = match action {
            Action::Comment => format!("; /* {} */", collapse(chars, h.start, h.end)),
            _ => ";".to_string(),
        };
        edits.push((h.start, h.end, repl));
    }

    // Plain statements are grouped per line so several calls sharing a line are
    // recognised as owning it together.
    let statements: Vec<&Hit> = hits
        .iter()
        .filter(|h| h.position == Position::Statement)
        .collect();
    let mut i = 0usize;
    while i < statements.len() {
        let mut group: Vec<&Hit> = vec![statements[i]];
        let mut first_line = line_of(&starts, statements[i].start);
        let mut last_line = line_of(&starts, statements[i].end.saturating_sub(1));
        let mut j = i + 1;
        while j < statements.len() {
            let f = line_of(&starts, statements[j].start);
            if f > last_line {
                break;
            }
            last_line = last_line.max(line_of(&starts, statements[j].end.saturating_sub(1)));
            first_line = first_line.min(f);
            group.push(statements[j]);
            j += 1;
        }
        i = j;

        let block_from = starts[first_line];
        let block_to = line_end(chars, &starts, last_line);
        let owns_lines = (block_from..block_to).all(|idx| {
            group.iter().any(|h| idx >= h.start && idx < h.end) || chars[idx].is_whitespace()
        });

        if owns_lines {
            match action {
                Action::Remove => {
                    if block_to < chars.len() {
                        edits.push((block_from, block_to + 1, String::new()));
                    } else if block_from > 0 {
                        edits.push((block_from - 1, block_to, String::new()));
                    } else {
                        edits.push((block_from, block_to, String::new()));
                    }
                }
                Action::Blank => {
                    edits.push((block_from, block_to, newlines(chars, block_from, block_to)))
                }
                Action::Comment => edits.push((
                    block_from,
                    block_to,
                    comment_lines(chars, block_from, block_to),
                )),
            }
        } else {
            for h in group {
                let repl = match action {
                    Action::Remove => String::new(),
                    Action::Blank => newlines(chars, h.start, h.end),
                    Action::Comment => format!("/* {} */", collapse(chars, h.start, h.end)),
                };
                edits.push((h.start, h.end, repl));
            }
        }
    }

    edits.sort_by_key(|e| e.0);
    let mut out = String::with_capacity(chars.len());
    let mut cursor = 0usize;
    for (from, to, repl) in edits {
        if from < cursor {
            continue; // defensive: never apply overlapping edits
        }
        out.extend(chars[cursor..from].iter());
        out.push_str(&repl);
        cursor = to;
    }
    out.extend(chars[cursor..].iter());
    out
}

fn snippet(chars: &[char], h: &Hit) -> String {
    let text = collapse(chars, h.start, h.end);
    if text.chars().count() > 100 {
        let short: String = text.chars().take(99).collect();
        format!("{short}…")
    } else {
        text
    }
}

fn report(chars: &[char], hits: &[Hit], targeted: &str, action: Action) -> String {
    let starts = line_starts(chars);
    let removed: Vec<&Hit> = hits
        .iter()
        .filter(|h| h.position != Position::Expression)
        .collect();
    let kept: Vec<&Hit> = hits
        .iter()
        .filter(|h| h.position == Position::Expression)
        .collect();

    let mut out = String::new();
    out.push_str(&format!("Targeted methods: {targeted}\n"));
    out.push_str(&format!("Action: {}\n", action.label()));
    out.push_str(&format!("Removed: {}\n", removed.len()));
    out.push_str(&format!("Kept in expression position: {}\n", kept.len()));

    if !removed.is_empty() {
        out.push_str("\nRemoved statements:\n");
        for h in &removed {
            let line = line_of(&starts, h.start) + 1;
            out.push_str(&format!("  line {line}: {}\n", snippet(chars, h)));
        }
        let mut counts: Vec<(String, usize)> = Vec::new();
        for h in &removed {
            match counts.iter_mut().find(|c| c.0 == h.method) {
                Some(c) => c.1 += 1,
                None => counts.push((h.method.clone(), 1)),
            }
        }
        counts.sort_by(|a, b| a.0.cmp(&b.0));
        out.push_str("\nBy method:\n");
        for (name, count) in counts {
            out.push_str(&format!("  {name}: {count}\n"));
        }
    }

    if !kept.is_empty() {
        out.push_str("\nKept (used as a value — removing could change behaviour):\n");
        for h in &kept {
            let line = line_of(&starts, h.start) + 1;
            out.push_str(&format!("  line {line}: {}\n", snippet(chars, h)));
        }
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// Parameter parsing
// ---------------------------------------------------------------------------

fn canonical_method(name: &str) -> Option<&'static str> {
    CONSOLE_METHODS
        .iter()
        .find(|m| m.eq_ignore_ascii_case(name))
        .copied()
}

fn parse_method_list(raw: &str, field: &str) -> Result<(Vec<&'static str>, bool), String> {
    let mut list = Vec::new();
    let mut all = false;
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.eq_ignore_ascii_case("all") {
            if field == "keep" {
                return Err(
                    "keep does not accept 'all'; leave methods empty instead of keeping everything"
                        .to_string(),
                );
            }
            all = true;
            continue;
        }
        let stripped = part.strip_prefix("console.").unwrap_or(part);
        match canonical_method(stripped) {
            Some(m) => {
                if !list.contains(&m) {
                    list.push(m);
                }
            }
            None => {
                return Err(format!(
                    "unknown console method '{part}' in {field}; valid methods: {} (or 'all' in methods)",
                    CONSOLE_METHODS.join(", ")
                ))
            }
        }
    }
    Ok((list, all))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Strip `console.*` statements from JavaScript or TypeScript source.
///
/// * `methods` — comma-separated console methods to strip, or `all`.
/// * `keep` — comma-separated methods never stripped (wins over `methods`).
/// * `action` — `remove`, `comment` or `blank`.
/// * `remove_debugger` — also drop `debugger;` statements.
/// * `output` — `code` for the rewritten source, `report` for a dry-run listing.
pub fn strip(
    code: &str,
    methods: &str,
    keep: &str,
    action: &str,
    remove_debugger: bool,
    output: &str,
) -> Result<String, String> {
    let action = Action::parse(action)?;
    let output = Output::parse(output)?;

    if code.trim().is_empty() {
        return Err("code is empty; paste JavaScript or TypeScript source to strip".to_string());
    }
    let len = code.chars().count();
    if len > MAX_INPUT {
        return Err(format!(
            "code is too large ({len} characters); the limit is {MAX_INPUT} characters"
        ));
    }

    let methods_raw = if methods.trim().is_empty() {
        "log,debug,info,warn"
    } else {
        methods
    };
    let (wanted, all) = parse_method_list(methods_raw, "methods")?;
    let (kept, _) = parse_method_list(keep, "keep")?;
    if !all && wanted.is_empty() && !remove_debugger {
        return Err(
            "no console methods selected; list at least one (e.g. log,warn) or use all".to_string(),
        );
    }
    let wanted_owned: Vec<&'static str> = wanted.clone();
    let kept_owned: Vec<&'static str> = kept.clone();
    let matcher = move |m: &str| -> bool {
        if kept_owned.iter().any(|k| k.eq_ignore_ascii_case(m)) {
            return false;
        }
        if all {
            return true;
        }
        wanted_owned.iter().any(|w| w.eq_ignore_ascii_case(m))
    };

    let chars: Vec<char> = code.chars().collect();
    let toks = tokenize(&chars);
    let hits = find_hits(&chars, &toks, &matcher, remove_debugger);

    match output {
        Output::Code => Ok(rewrite(&chars, &hits, action)),
        Output::Report => {
            let mut targeted = if all {
                "all".to_string()
            } else {
                wanted.join(", ")
            };
            if targeted.is_empty() {
                targeted = "(none)".to_string();
            }
            if !kept.is_empty() {
                targeted.push_str(&format!(" (keeping {})", kept.join(", ")));
            }
            if remove_debugger {
                targeted.push_str(" + debugger");
            }
            Ok(report(&chars, &hits, &targeted, action))
        }
    }
}

/// Convenience wrapper using the defaults (`log,debug,info,warn`, remove, code).
pub fn run(code: &str) -> Result<String, String> {
    strip(code, "", "", "remove", false, "code")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of(src: &str) -> String {
        strip(src, "", "", "remove", false, "code").unwrap()
    }

    #[test]
    fn removes_a_whole_console_log_line() {
        let src = "const a = 1;\nconsole.log(a);\nexport default a;\n";
        assert_eq!(code_of(src), "const a = 1;\nexport default a;\n");
    }

    #[test]
    fn empty_code_is_an_error() {
        let err = strip("   \n", "", "", "remove", false, "code").unwrap_err();
        assert!(err.contains("code is empty"), "{err}");
    }

    #[test]
    fn unknown_method_is_an_error() {
        let err = strip("console.log(1);", "warnn", "", "remove", false, "code").unwrap_err();
        assert!(err.contains("unknown console method 'warnn'"), "{err}");
        assert!(err.contains("valid methods:"), "{err}");
    }

    #[test]
    fn unknown_action_and_output_are_errors() {
        assert!(strip("console.log(1)", "", "", "erase", false, "code")
            .unwrap_err()
            .contains("unknown action 'erase'"));
        assert!(strip("console.log(1)", "", "", "remove", false, "csv")
            .unwrap_err()
            .contains("unknown output 'csv'"));
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "a".repeat(MAX_INPUT + 1);
        let err = strip(&big, "", "", "remove", false, "code").unwrap_err();
        assert!(err.contains("code is too large"), "{err}");
        // Exactly at the cap is accepted.
        let at_cap = "a".repeat(MAX_INPUT);
        assert!(strip(&at_cap, "", "", "remove", false, "code").is_ok());
    }

    #[test]
    fn keeps_error_and_warn_by_default_when_not_listed() {
        let src = "console.error('boom');\nconsole.log('x');\n";
        assert_eq!(code_of(src), "console.error('boom');\n");
    }

    #[test]
    fn all_with_keep_behaves_like_an_exclude_list() {
        let src = "console.log(1);\nconsole.error(2);\nconsole.table(3);\n";
        let out = strip(src, "all", "error", "remove", false, "code").unwrap();
        assert_eq!(out, "console.error(2);\n");
    }

    #[test]
    fn does_not_touch_strings_templates_regex_or_comments() {
        let src = concat!(
            "const s = \"console.log(1)\";\n",
            "const t = `console.log(${x})`;\n",
            "const r = /console\\.log\\(/g;\n",
            "// console.log(9)\n",
            "/* console.log(8) */\n",
            "console.log('gone');\n",
        );
        let out = code_of(src);
        assert!(out.contains("const s = \"console.log(1)\";"));
        assert!(out.contains("const t = `console.log(${x})`;"));
        assert!(out.contains("const r = /console\\.log\\(/g;"));
        assert!(out.contains("// console.log(9)"));
        assert!(out.contains("/* console.log(8) */"));
        assert!(!out.contains("'gone'"));
    }

    #[test]
    fn handles_multi_line_calls_with_nested_parens_and_strings() {
        let src = "before();\nconsole.log(\n  'a )',\n  f(g(1)),\n);\nafter();\n";
        assert_eq!(code_of(src), "before();\nafter();\n");
    }

    #[test]
    fn division_is_not_mistaken_for_a_regex() {
        let src = "const r = a / b;\nconsole.log(r);\nconst q = c / d;\n";
        assert_eq!(code_of(src), "const r = a / b;\nconst q = c / d;\n");
    }

    #[test]
    fn expression_position_calls_are_left_alone() {
        let src = concat!(
            "const a = console.log(1);\n",
            "x && console.log(2);\n",
            "items.forEach(i => console.log(i));\n",
            "console.log(3).foo;\n",
        );
        assert_eq!(code_of(src), src);
    }

    #[test]
    fn unbraced_control_body_becomes_an_empty_statement() {
        let src = "if (x) console.log(1);\nwhile (y) console.log(2);\n";
        assert_eq!(code_of(src), "if (x) ;\nwhile (y) ;\n");
    }

    #[test]
    fn else_and_do_bodies_are_handled() {
        let src = "if (a) f();\nelse console.log(1);\ndo console.log(2); while (b);\n";
        assert_eq!(code_of(src), "if (a) f();\nelse ;\ndo ; while (b);\n");
    }

    #[test]
    fn asi_style_code_without_semicolons_is_stripped() {
        let src = "const a = 1\nconsole.log(a)\nfoo()\n";
        assert_eq!(code_of(src), "const a = 1\nfoo()\n");
    }

    #[test]
    fn window_and_optional_chaining_receivers_match() {
        let src = "window.console.log(1);\nconsole?.log(2);\nconsole.log?.(3);\nkeep();\n";
        assert_eq!(code_of(src), "keep();\n");
    }

    #[test]
    fn comment_action_keeps_indentation_and_line_count() {
        let src = "function f() {\n  console.log(1);\n  return 2;\n}\n";
        let out = strip(src, "", "", "comment", false, "code").unwrap();
        assert_eq!(out, "function f() {\n  // console.log(1);\n  return 2;\n}\n");
    }

    #[test]
    fn blank_action_preserves_line_numbers() {
        let src = "a();\nconsole.log(\n  1\n);\nb();\n";
        let out = strip(src, "", "", "blank", false, "code").unwrap();
        assert_eq!(out, "a();\n\n\n\nb();\n");
        assert_eq!(src.lines().count(), out.lines().count());
    }

    #[test]
    fn inline_call_is_spliced_without_dropping_neighbouring_code() {
        let src = "doWork(); console.log('after');\n";
        assert_eq!(code_of(src), "doWork(); \n");
    }

    #[test]
    fn two_calls_sharing_a_line_drop_the_line() {
        let src = "keep();\nconsole.log(1); console.log(2);\nkeep2();\n";
        assert_eq!(code_of(src), "keep();\nkeep2();\n");
    }

    #[test]
    fn debugger_is_only_removed_when_requested() {
        let src = "debugger;\nconsole.log(1);\nfoo();\n";
        assert_eq!(code_of(src), "debugger;\nfoo();\n");
        let out = strip(src, "log", "", "remove", true, "code").unwrap();
        assert_eq!(out, "foo();\n");
    }

    #[test]
    fn typescript_source_is_supported() {
        let src = "function f<T>(v: T): T {\n  console.debug(v);\n  return v;\n}\n";
        assert_eq!(code_of(src), "function f<T>(v: T): T {\n  return v;\n}\n");
    }

    #[test]
    fn switch_case_bodies_are_statements() {
        let src = "switch (a) {\n  case 1: console.log(1);\n  default: break;\n}\n";
        let out = code_of(src);
        assert!(out.contains("case 1: "), "{out}");
        assert!(!out.contains("console.log"), "{out}");
    }

    #[test]
    fn report_lists_lines_counts_and_kept_calls() {
        let src = "console.log('a');\nconst v = console.log('b');\nconsole.warn('c');\n";
        let out = strip(src, "log,warn", "", "remove", false, "report").unwrap();
        assert!(out.contains("Removed: 2"), "{out}");
        assert!(out.contains("Kept in expression position: 1"), "{out}");
        assert!(out.contains("line 1: console.log('a');"), "{out}");
        assert!(out.contains("line 3: console.warn('c');"), "{out}");
        assert!(out.contains("  log: 1"), "{out}");
        assert!(out.contains("  warn: 1"), "{out}");
        assert!(out.contains("line 2: console.log('b')"), "{out}");
    }

    #[test]
    fn report_leaves_the_source_untouched() {
        let src = "console.log(1);\n";
        let out = strip(src, "", "", "remove", false, "report").unwrap();
        assert!(!out.starts_with("console.log"));
        assert!(out.contains("Targeted methods: log, debug, info, warn"));
    }

    #[test]
    fn methods_accept_the_console_prefix_and_odd_casing() {
        let src = "console.groupEnd();\nkeep();\n";
        let out = strip(src, "console.GROUPEND", "", "remove", false, "code").unwrap();
        assert_eq!(out, "keep();\n");
    }

    #[test]
    fn keep_only_debugger_selection_is_allowed() {
        let src = "debugger;\nconsole.log(1);\n";
        let out = strip(src, "", "log,debug,info,warn", "remove", true, "code").unwrap();
        assert_eq!(out, "console.log(1);\n");
    }

    #[test]
    fn run_uses_the_documented_defaults() {
        assert_eq!(run("console.log(1);\nfoo();\n").unwrap(), "foo();\n");
    }
}
