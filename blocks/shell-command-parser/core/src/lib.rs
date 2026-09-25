//! shell-command-parser core — pure compute, shared by the chat skill block and the web page.
//!
//! Tokenizes and parses a POSIX/bash command line into a structured tree of
//! statements, and-or lists, pipelines, simple commands, environment
//! assignments, redirections (including here-documents) and quoted words.
//! Nothing is ever executed and no expansion is performed — `$HOME`, `$(date)`
//! and `*.txt` are reported as expansions/globs, not resolved.

use serde_json::{json, Map, Value};

/// Longest command line accepted, in bytes.
pub const MAX_INPUT: usize = 200_000;
/// Deepest `(` / `{` nesting accepted before the parser gives up.
pub const MAX_DEPTH: usize = 32;

// ---------------------------------------------------------------------------
// Words
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quote {
    None,
    Single,
    Double,
    AnsiC,
    Mixed,
}

impl Quote {
    fn as_str(self) -> &'static str {
        match self {
            Quote::None => "none",
            Quote::Single => "single",
            Quote::Double => "double",
            Quote::AnsiC => "ansi-c",
            Quote::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    /// `parameter` | `command` | `arithmetic` | `process`
    pub kind: &'static str,
    pub text: String,
}

/// One shell word: its raw source text plus the literal value after quote
/// removal (expansions are left verbatim — nothing is evaluated).
#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    pub value: String,
    pub quote: Quote,
    pub expansions: Vec<Expansion>,
    pub glob: bool,
}

impl Word {
    fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("text".into(), json!(self.text));
        m.insert("value".into(), json!(self.value));
        m.insert("quoting".into(), json!(self.quote.as_str()));
        m.insert(
            "expansions".into(),
            Value::Array(
                self.expansions
                    .iter()
                    .map(|e| json!({ "kind": e.kind, "text": e.text }))
                    .collect(),
            ),
        );
        m.insert("glob".into(), json!(self.glob));
        Value::Object(m)
    }
}

// ---------------------------------------------------------------------------
// Here-documents
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Heredoc {
    pub delimiter: String,
    pub strip_tabs: bool,
    pub expand: bool,
    pub body: String,
    pub terminated: bool,
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Tok {
    Word(Word),
    /// `;` `;;` `&` `&&` `||` `|` `|&` `(` `)` `\n`
    Op(String),
    Redir {
        fd: Option<String>,
        op: String,
        heredoc: Option<usize>,
    },
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

struct Lexer {
    src: Vec<char>,
    i: usize,
    toks: Vec<Tok>,
    heredocs: Vec<Heredoc>,
    pending: Vec<usize>,
    notes: Vec<String>,
}

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t' || c == '\r'
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl Lexer {
    fn new(src: &str) -> Self {
        Lexer {
            src: src.chars().collect(),
            i: 0,
            toks: Vec::new(),
            heredocs: Vec::new(),
            pending: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn peek(&self, off: usize) -> Option<char> {
        self.src.get(self.i + off).copied()
    }

    fn starts_with(&self, s: &str) -> bool {
        s.chars().enumerate().all(|(k, c)| self.peek(k) == Some(c))
    }

    fn skip_blanks(&mut self) {
        loop {
            match self.peek(0) {
                Some(c) if is_blank(c) => self.i += 1,
                Some('\\') if self.peek(1) == Some('\n') => self.i += 2,
                _ => break,
            }
        }
    }

    fn run(mut self) -> Result<(Vec<Tok>, Vec<Heredoc>, Vec<String>), String> {
        loop {
            self.skip_blanks();
            let c = match self.peek(0) {
                Some(c) => c,
                None => break,
            };
            if c == '#' {
                while let Some(ch) = self.peek(0) {
                    if ch == '\n' {
                        break;
                    }
                    self.i += 1;
                }
                continue;
            }
            if c == '\n' {
                self.i += 1;
                self.read_pending_heredocs();
                self.toks.push(Tok::Op("\n".into()));
                continue;
            }
            if self.try_operator() {
                continue;
            }
            if self.try_redirect()? {
                continue;
            }
            let w = self.read_word()?;
            self.toks.push(Tok::Word(w));
        }
        // A here-doc opened on the final line still needs its body read.
        self.read_pending_heredocs();
        Ok((self.toks, self.heredocs, self.notes))
    }

    /// Longest match wins, so `&&` beats `&` and `||`/`|&` beat `|`.
    fn try_operator(&mut self) -> bool {
        for op in [";;", "&&", "||", "|&", ";", "|", "(", ")"] {
            if self.starts_with(op) {
                self.i += op.chars().count();
                self.toks.push(Tok::Op(op.into()));
                return true;
            }
        }
        if self.peek(0) == Some('&') {
            // `&>` / `&>>` are redirections, handled by try_redirect.
            if self.peek(1) == Some('>') {
                return false;
            }
            self.i += 1;
            self.toks.push(Tok::Op("&".into()));
            return true;
        }
        false
    }

    /// Consumes `[fd]<op>` (and `&>`/`&>>`). Returns false when the position is
    /// not a redirection, so the caller falls through to `read_word`.
    fn try_redirect(&mut self) -> Result<bool, String> {
        let start = self.i;
        let mut fd: Option<String> = None;

        // `&>` / `&>>` — redirect stdout and stderr together (bash).
        if self.starts_with("&>>") {
            self.i += 3;
            self.push_redirect(None, "&>>")?;
            return Ok(true);
        }
        if self.starts_with("&>") {
            self.i += 2;
            self.push_redirect(None, "&>")?;
            return Ok(true);
        }

        // An IO number: digits glued to the operator, e.g. `2>&1`.
        let mut k = 0;
        while matches!(self.peek(k), Some(c) if c.is_ascii_digit()) {
            k += 1;
        }
        if k > 0 && matches!(self.peek(k), Some('<') | Some('>')) {
            fd = Some(self.src[self.i..self.i + k].iter().collect());
            self.i += k;
        } else if self.peek(0) == Some('{') {
            // bash `{fd}>file` — allocate a descriptor into a variable.
            let mut j = 1;
            if matches!(self.peek(j), Some(c) if is_name_start(c)) {
                j += 1;
                while matches!(self.peek(j), Some(c) if is_name_char(c)) {
                    j += 1;
                }
                if self.peek(j) == Some('}') && matches!(self.peek(j + 1), Some('<') | Some('>')) {
                    fd = Some(self.src[self.i..=self.i + j].iter().collect());
                    self.i += j + 1;
                }
            }
        }

        let op = ["<<<", "<<-", "<<", "<&", "<>", "<", ">>", ">&", ">|", ">"]
            .into_iter()
            .find(|op| self.starts_with(op));
        let op = match op {
            Some(op) => op,
            None => {
                self.i = start;
                return Ok(false);
            }
        };
        // `<(cmd)` / `>(cmd)` are process substitutions — words, not redirects.
        if (op == "<" || op == ">") && self.peek(1) == Some('(') {
            self.i = start;
            return Ok(false);
        }
        self.i += op.chars().count();
        self.push_redirect(fd, op)?;
        Ok(true)
    }

    fn push_redirect(&mut self, fd: Option<String>, op: &str) -> Result<(), String> {
        let mut heredoc = None;
        if op == "<<" || op == "<<-" {
            self.skip_blanks();
            if self.peek(0).is_none() || self.peek(0) == Some('\n') {
                return Err(format!(
                    "expected a here-document delimiter after '{}', found end of line",
                    op
                ));
            }
            let delim_word = self.read_word()?;
            let idx = self.heredocs.len();
            self.heredocs.push(Heredoc {
                delimiter: delim_word.value.clone(),
                strip_tabs: op == "<<-",
                expand: delim_word.quote == Quote::None,
                body: String::new(),
                terminated: false,
            });
            self.pending.push(idx);
            heredoc = Some(idx);
            self.toks.push(Tok::Redir {
                fd,
                op: op.into(),
                heredoc,
            });
            self.toks.push(Tok::Word(delim_word));
            return Ok(());
        }
        self.toks.push(Tok::Redir {
            fd,
            op: op.into(),
            heredoc,
        });
        Ok(())
    }

    fn read_pending_heredocs(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for idx in pending {
            let delim = self.heredocs[idx].delimiter.clone();
            let strip = self.heredocs[idx].strip_tabs;
            let mut body = String::new();
            let mut terminated = false;
            loop {
                if self.i >= self.src.len() {
                    break;
                }
                let start = self.i;
                while self.i < self.src.len() && self.src[self.i] != '\n' {
                    self.i += 1;
                }
                let raw: String = self.src[start..self.i].iter().collect();
                if self.i < self.src.len() {
                    self.i += 1;
                }
                let line: &str = if strip {
                    raw.trim_start_matches('\t')
                } else {
                    raw.as_str()
                };
                if line == delim {
                    terminated = true;
                    break;
                }
                body.push_str(line);
                body.push('\n');
            }
            self.heredocs[idx].body = body;
            self.heredocs[idx].terminated = terminated;
            if !terminated {
                self.notes.push(format!(
                    "here-document delimiter '{}' was never found; its body was read to the end of the input",
                    delim
                ));
            }
        }
    }

    // -- word reading ------------------------------------------------------

    fn read_word(&mut self) -> Result<Word, String> {
        let mut text = String::new();
        let mut value = String::new();
        let mut quotes: Vec<Quote> = Vec::new();
        let mut expansions: Vec<Expansion> = Vec::new();
        let mut glob = false;
        let mut bare = false;

        loop {
            let c = match self.peek(0) {
                Some(c) => c,
                None => break,
            };
            match c {
                ' ' | '\t' | '\r' | '\n' | ';' | '&' | '|' | '(' | ')' => break,
                '<' | '>' if self.peek(1) == Some('(') => {
                    let start = self.i;
                    self.i += 1; // the < or >
                    let end = self.scan_balanced('(', ')')?;
                    let raw: String = self.src[start..end].iter().collect();
                    self.i = end;
                    expansions.push(Expansion {
                        kind: "process",
                        text: raw.clone(),
                    });
                    text.push_str(&raw);
                    value.push_str(&raw);
                    bare = true;
                }
                '<' | '>' => break,
                '\'' => {
                    let start = self.i;
                    self.i += 1;
                    let mut inner = String::new();
                    loop {
                        match self.peek(0) {
                            Some('\'') => {
                                self.i += 1;
                                break;
                            }
                            Some(ch) => {
                                inner.push(ch);
                                self.i += 1;
                            }
                            None => {
                                return Err(format!(
                                    "unterminated single quote opened at character {}",
                                    start + 1
                                ))
                            }
                        }
                    }
                    text.push_str(&self.src[start..self.i].iter().collect::<String>());
                    value.push_str(&inner);
                    quotes.push(Quote::Single);
                }
                '"' => {
                    let start = self.i;
                    self.i += 1;
                    self.read_double_quoted(&mut value, &mut expansions, start)?;
                    text.push_str(&self.src[start..self.i].iter().collect::<String>());
                    quotes.push(Quote::Double);
                }
                '$' if self.peek(1) == Some('\'') => {
                    let start = self.i;
                    self.i += 2;
                    let mut inner = String::new();
                    loop {
                        match self.peek(0) {
                            Some('\\') => {
                                let esc = self.peek(1);
                                self.i += 2;
                                let decoded = match esc {
                                    Some('n') => '\n',
                                    Some('t') => '\t',
                                    Some('r') => '\r',
                                    Some('0') => '\0',
                                    Some('e') => '\u{1b}',
                                    Some('a') => '\u{7}',
                                    Some('b') => '\u{8}',
                                    Some('\\') => '\\',
                                    Some('\'') => '\'',
                                    Some(other) => {
                                        inner.push('\\');
                                        other
                                    }
                                    None => {
                                        return Err(format!(
                                            "unterminated $'...' quote opened at character {}",
                                            start + 1
                                        ))
                                    }
                                };
                                inner.push(decoded);
                            }
                            Some('\'') => {
                                self.i += 1;
                                break;
                            }
                            Some(ch) => {
                                inner.push(ch);
                                self.i += 1;
                            }
                            None => {
                                return Err(format!(
                                    "unterminated $'...' quote opened at character {}",
                                    start + 1
                                ))
                            }
                        }
                    }
                    text.push_str(&self.src[start..self.i].iter().collect::<String>());
                    value.push_str(&inner);
                    quotes.push(Quote::AnsiC);
                }
                '$' if self.peek(1) == Some('"') => {
                    let start = self.i;
                    self.i += 2;
                    self.read_double_quoted(&mut value, &mut expansions, start)?;
                    text.push_str(&self.src[start..self.i].iter().collect::<String>());
                    quotes.push(Quote::Double);
                }
                '$' => {
                    let raw = self.scan_dollar(&mut expansions)?;
                    text.push_str(&raw);
                    value.push_str(&raw);
                    bare = true;
                }
                '`' => {
                    let start = self.i;
                    self.i += 1;
                    loop {
                        match self.peek(0) {
                            Some('\\') => self.i += 2,
                            Some('`') => {
                                self.i += 1;
                                break;
                            }
                            Some(_) => self.i += 1,
                            None => {
                                return Err(format!(
                                "unterminated backtick command substitution opened at character {}",
                                start + 1
                            ))
                            }
                        }
                    }
                    let raw: String = self.src[start..self.i].iter().collect();
                    expansions.push(Expansion {
                        kind: "command",
                        text: raw.clone(),
                    });
                    text.push_str(&raw);
                    value.push_str(&raw);
                    bare = true;
                }
                '\\' => {
                    match self.peek(1) {
                        Some('\n') => self.i += 2,
                        Some(esc) => {
                            text.push('\\');
                            text.push(esc);
                            value.push(esc);
                            self.i += 2;
                            bare = true;
                        }
                        None => return Err(
                            "trailing backslash: the command line ends with an escape character"
                                .to_string(),
                        ),
                    }
                }
                '*' | '?' | '[' => {
                    glob = true;
                    text.push(c);
                    value.push(c);
                    self.i += 1;
                    bare = true;
                }
                _ => {
                    text.push(c);
                    value.push(c);
                    self.i += 1;
                    bare = true;
                }
            }
        }

        let quote = if quotes.is_empty() {
            Quote::None
        } else if bare || quotes.iter().any(|q| *q != quotes[0]) {
            Quote::Mixed
        } else {
            quotes[0]
        };
        Ok(Word {
            text,
            value,
            quote,
            expansions,
            glob,
        })
    }

    /// Reads the body of a `"..."` that has already had its opening quote
    /// consumed, appending the dequoted contents to `value`.
    fn read_double_quoted(
        &mut self,
        value: &mut String,
        expansions: &mut Vec<Expansion>,
        start: usize,
    ) -> Result<(), String> {
        loop {
            match self.peek(0) {
                Some('"') => {
                    self.i += 1;
                    return Ok(());
                }
                Some('\\') => match self.peek(1) {
                    // Inside double quotes a backslash only escapes these.
                    Some(esc @ ('$' | '`' | '"' | '\\')) => {
                        value.push(esc);
                        self.i += 2;
                    }
                    Some('\n') => self.i += 2,
                    Some(other) => {
                        value.push('\\');
                        value.push(other);
                        self.i += 2;
                    }
                    None => {
                        return Err(format!(
                            "unterminated double quote opened at character {}",
                            start + 1
                        ))
                    }
                },
                Some('$') => {
                    let raw = self.scan_dollar(expansions)?;
                    value.push_str(&raw);
                }
                Some('`') => {
                    let bstart = self.i;
                    self.i += 1;
                    loop {
                        match self.peek(0) {
                            Some('\\') => self.i += 2,
                            Some('`') => {
                                self.i += 1;
                                break;
                            }
                            Some(_) => self.i += 1,
                            None => {
                                return Err(format!(
                                "unterminated backtick command substitution opened at character {}",
                                bstart + 1
                            ))
                            }
                        }
                    }
                    let raw: String = self.src[bstart..self.i].iter().collect();
                    expansions.push(Expansion {
                        kind: "command",
                        text: raw.clone(),
                    });
                    value.push_str(&raw);
                }
                Some(ch) => {
                    value.push(ch);
                    self.i += 1;
                }
                None => {
                    return Err(format!(
                        "unterminated double quote opened at character {}",
                        start + 1
                    ))
                }
            }
        }
    }

    /// At a `$`: consumes the expansion and returns its raw source text.
    fn scan_dollar(&mut self, expansions: &mut Vec<Expansion>) -> Result<String, String> {
        let start = self.i;
        match self.peek(1) {
            Some('(') if self.peek(2) == Some('(') => {
                self.i += 1;
                let end = self.scan_balanced('(', ')')?;
                self.i = end;
                let raw: String = self.src[start..self.i].iter().collect();
                expansions.push(Expansion {
                    kind: "arithmetic",
                    text: raw.clone(),
                });
                Ok(raw)
            }
            Some('(') => {
                self.i += 1;
                let end = self.scan_balanced('(', ')')?;
                self.i = end;
                let raw: String = self.src[start..self.i].iter().collect();
                expansions.push(Expansion {
                    kind: "command",
                    text: raw.clone(),
                });
                Ok(raw)
            }
            Some('{') => {
                self.i += 1;
                let end = self.scan_balanced('{', '}')?;
                self.i = end;
                let raw: String = self.src[start..self.i].iter().collect();
                expansions.push(Expansion {
                    kind: "parameter",
                    text: raw.clone(),
                });
                Ok(raw)
            }
            Some(c) if is_name_start(c) => {
                self.i += 1;
                while matches!(self.peek(0), Some(c) if is_name_char(c)) {
                    self.i += 1;
                }
                let raw: String = self.src[start..self.i].iter().collect();
                expansions.push(Expansion {
                    kind: "parameter",
                    text: raw.clone(),
                });
                Ok(raw)
            }
            Some(c) if "@*#?$!-0123456789".contains(c) => {
                self.i += 2;
                let raw: String = self.src[start..self.i].iter().collect();
                expansions.push(Expansion {
                    kind: "parameter",
                    text: raw.clone(),
                });
                Ok(raw)
            }
            // A lone `$` is literal.
            _ => {
                self.i += 1;
                Ok("$".to_string())
            }
        }
    }

    /// From an `open` delimiter at `self.i`, returns the index just past the
    /// matching `close`, honouring nesting, quotes and backslash escapes.
    fn scan_balanced(&self, open: char, close: char) -> Result<usize, String> {
        let mut j = self.i;
        debug_assert_eq!(self.src.get(j).copied(), Some(open));
        let mut depth = 0usize;
        while j < self.src.len() {
            let c = self.src[j];
            match c {
                '\\' => j += 1,
                '\'' => {
                    j += 1;
                    while j < self.src.len() && self.src[j] != '\'' {
                        j += 1;
                    }
                }
                '"' => {
                    j += 1;
                    while j < self.src.len() && self.src[j] != '"' {
                        if self.src[j] == '\\' {
                            j += 1;
                        }
                        j += 1;
                    }
                }
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(j + 1);
                    }
                }
                _ => {}
            }
            j += 1;
        }
        Err(format!(
            "unterminated '{}' expansion opened at character {} — no matching '{}'",
            if open == '(' { "$(" } else { "${" },
            self.i + 1,
            close
        ))
    }
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Assignment {
    pub name: String,
    pub value: Word,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Redirect {
    pub fd: Option<String>,
    pub op: String,
    pub target: Option<Word>,
    pub heredoc: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct SimpleCmd {
    pub assignments: Vec<Assignment>,
    pub words: Vec<Word>,
    pub redirects: Vec<Redirect>,
}

#[derive(Debug, Clone)]
pub enum Cmd {
    Simple(SimpleCmd),
    Subshell {
        body: ListNode,
        redirects: Vec<Redirect>,
    },
    Group {
        body: ListNode,
        redirects: Vec<Redirect>,
    },
    Keyword(String),
}

#[derive(Debug, Clone)]
pub struct PipeElement {
    /// The operator that feeds this element, e.g. `|` or `|&`. None for the first.
    pub pipe_op: Option<String>,
    pub cmd: Cmd,
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    pub negated: bool,
    pub elements: Vec<PipeElement>,
}

#[derive(Debug, Clone)]
pub struct AndOrPart {
    /// `&&` or `||`. None for the first pipeline.
    pub operator: Option<String>,
    pub pipeline: Pipeline,
}

#[derive(Debug, Clone)]
pub struct AndOr {
    pub parts: Vec<AndOrPart>,
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub and_or: AndOr,
    /// `;`, `;;`, `&` (background) or None when the statement just ends.
    pub terminator: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListNode {
    pub statements: Vec<Statement>,
}

/// Shell reserved words. In command position each is reported as its own
/// `keyword` node — the parser does not build nested if/for/while blocks.
const RESERVED: [&str; 13] = [
    "if", "then", "elif", "else", "fi", "for", "while", "until", "do", "done", "case", "esac",
    "select",
];

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    i: usize,
    notes: Vec<String>,
    command_count: usize,
    max_depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }

    fn peek_op(&self) -> Option<&str> {
        match self.toks.get(self.i) {
            Some(Tok::Op(o)) => Some(o.as_str()),
            _ => None,
        }
    }

    fn peek_word_text(&self) -> Option<&str> {
        match self.toks.get(self.i) {
            Some(Tok::Word(w)) => Some(w.text.as_str()),
            _ => None,
        }
    }

    fn skip_newlines(&mut self) {
        while self.peek_op() == Some("\n") {
            self.i += 1;
        }
    }

    fn parse_list(&mut self, depth: usize) -> Result<ListNode, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "nesting is deeper than the supported limit of {} levels of ( ) or {{ }}",
                MAX_DEPTH
            ));
        }
        self.max_depth = self.max_depth.max(depth);
        let mut statements = Vec::new();
        loop {
            self.skip_newlines();
            match self.peek() {
                None => break,
                Some(Tok::Op(o)) if o == ")" => break,
                _ => {}
            }
            if self.peek_word_text() == Some("}") {
                break;
            }
            let and_or = self.parse_and_or(depth)?;
            let terminator = match self.peek_op() {
                Some(";") => {
                    self.i += 1;
                    Some(";".to_string())
                }
                Some(";;") => {
                    self.i += 1;
                    Some(";;".to_string())
                }
                Some("&") => {
                    self.i += 1;
                    Some("&".to_string())
                }
                Some("\n") => {
                    self.i += 1;
                    Some(";".to_string())
                }
                _ => None,
            };
            statements.push(Statement { and_or, terminator });
            // Without a terminator the command parser stopped at a closer, at a
            // reserved word (`do`, `done`, …) or at the end of the input. Only
            // the last of those ends the list.
        }
        if statements.is_empty() {
            return Err("expected a command, found an empty statement".to_string());
        }
        Ok(ListNode { statements })
    }

    fn parse_and_or(&mut self, depth: usize) -> Result<AndOr, String> {
        let mut parts = vec![AndOrPart {
            operator: None,
            pipeline: self.parse_pipeline(depth)?,
        }];
        loop {
            let op = match self.peek_op() {
                Some("&&") => "&&",
                Some("||") => "||",
                _ => break,
            };
            self.i += 1;
            self.skip_newlines();
            if self.peek().is_none() {
                return Err(format!(
                    "expected a command after '{}', found the end of the input",
                    op
                ));
            }
            parts.push(AndOrPart {
                operator: Some(op.to_string()),
                pipeline: self.parse_pipeline(depth)?,
            });
        }
        Ok(AndOr { parts })
    }

    fn parse_pipeline(&mut self, depth: usize) -> Result<Pipeline, String> {
        let mut negated = false;
        if self.peek_word_text() == Some("!") {
            negated = true;
            self.i += 1;
        }
        let mut elements = vec![PipeElement {
            pipe_op: None,
            cmd: self.parse_cmd(depth)?,
        }];
        loop {
            let op = match self.peek_op() {
                Some("|") => "|",
                Some("|&") => "|&",
                _ => break,
            };
            self.i += 1;
            self.skip_newlines();
            if self.peek().is_none() {
                return Err(format!(
                    "expected a command after '{}', found the end of the input",
                    op
                ));
            }
            elements.push(PipeElement {
                pipe_op: Some(op.to_string()),
                cmd: self.parse_cmd(depth)?,
            });
        }
        Ok(Pipeline { negated, elements })
    }

    fn parse_cmd(&mut self, depth: usize) -> Result<Cmd, String> {
        match self.peek() {
            None => Err("expected a command, found the end of the input".to_string()),
            Some(Tok::Op(o)) if o == "(" => {
                self.i += 1;
                let body = self.parse_list(depth + 1)?;
                if self.peek_op() != Some(")") {
                    return Err(
                        "expected a closing ')' for the subshell, found the end of the input"
                            .to_string(),
                    );
                }
                self.i += 1;
                let redirects = self.parse_redirects()?;
                Ok(Cmd::Subshell { body, redirects })
            }
            Some(Tok::Op(o)) => Err(format!("expected a command, found the operator '{}'", o)),
            Some(Tok::Redir { .. }) => {
                // A bare redirection with no command, e.g. `> out.txt`.
                let redirects = self.parse_redirects()?;
                self.command_count += 1;
                Ok(Cmd::Simple(SimpleCmd {
                    assignments: Vec::new(),
                    words: Vec::new(),
                    redirects,
                }))
            }
            Some(Tok::Word(w)) => {
                let text = w.text.clone();
                if text == "{" {
                    self.i += 1;
                    let body = self.parse_list(depth + 1)?;
                    if self.peek_word_text() != Some("}") {
                        return Err(
                            "expected a closing '}' for the brace group, found the end of the input"
                                .to_string(),
                        );
                    }
                    self.i += 1;
                    let redirects = self.parse_redirects()?;
                    return Ok(Cmd::Group { body, redirects });
                }
                if RESERVED.contains(&text.as_str()) {
                    self.i += 1;
                    if !self.notes.iter().any(|n| n.starts_with("control-flow")) {
                        self.notes.push(
                            "control-flow keywords (if/then/for/while/case/…) are reported as keyword nodes; \
                             this parser does not build nested if/for/while blocks"
                                .to_string(),
                        );
                    }
                    return Ok(Cmd::Keyword(text));
                }
                self.parse_simple(depth)
            }
        }
    }

    fn parse_simple(&mut self, _depth: usize) -> Result<Cmd, String> {
        let mut assignments = Vec::new();
        let mut words: Vec<Word> = Vec::new();
        let mut redirects = Vec::new();
        loop {
            match self.peek() {
                Some(Tok::Redir { .. }) => {
                    redirects.extend(self.parse_redirects()?);
                }
                Some(Tok::Word(w)) => {
                    let w = w.clone();
                    if words.is_empty() {
                        if let Some(a) = split_assignment(&w) {
                            assignments.push(a);
                            self.i += 1;
                            continue;
                        }
                        // Reserved words and `}` are only special in command position.
                        if RESERVED.contains(&w.text.as_str()) || w.text == "}" {
                            break;
                        }
                    }
                    words.push(w);
                    self.i += 1;
                }
                _ => break,
            }
        }
        if words.is_empty() && redirects.is_empty() && assignments.is_empty() {
            return Err("expected a command, found nothing".to_string());
        }
        self.command_count += 1;
        Ok(Cmd::Simple(SimpleCmd {
            assignments,
            words,
            redirects,
        }))
    }

    fn parse_redirects(&mut self) -> Result<Vec<Redirect>, String> {
        let mut out = Vec::new();
        while let Some(Tok::Redir { fd, op, heredoc }) = self.peek().cloned() {
            self.i += 1;
            let target = match self.peek() {
                Some(Tok::Word(w)) => {
                    let w = w.clone();
                    self.i += 1;
                    Some(w)
                }
                _ => {
                    return Err(format!(
                        "expected a target after the redirection '{}', found {}",
                        op,
                        match self.peek() {
                            None => "the end of the input".to_string(),
                            Some(Tok::Op(o)) => format!("the operator '{}'", o),
                            _ => "another redirection".to_string(),
                        }
                    ))
                }
            };
            out.push(Redirect {
                fd,
                op,
                target,
                heredoc,
            });
        }
        Ok(out)
    }
}

fn split_assignment(w: &Word) -> Option<Assignment> {
    // Only an unquoted NAME= prefix counts; `"A=b"` is a plain argument.
    let text = &w.text;
    let eq = text.find('=')?;
    if eq == 0 {
        return None;
    }
    let name = &text[..eq];
    let mut chars = name.chars();
    if !is_name_start(chars.next()?) || !chars.all(is_name_char) {
        return None;
    }
    let raw_value = &text[eq + 1..];
    let value_word = Lexer::new(raw_value).run().ok().and_then(|(toks, _, _)| {
        toks.into_iter().find_map(|t| match t {
            Tok::Word(w) => Some(w),
            _ => None,
        })
    });
    let value = value_word.unwrap_or(Word {
        text: raw_value.to_string(),
        value: String::new(),
        quote: Quote::None,
        expansions: Vec::new(),
        glob: false,
    });
    Some(Assignment {
        name: name.to_string(),
        value,
        text: text.clone(),
    })
}

// ---------------------------------------------------------------------------
// Redirect semantics
// ---------------------------------------------------------------------------

fn fd_label(fd: &Option<String>, default: &str) -> String {
    match fd {
        None => default.to_string(),
        Some(f) if f == "0" => "stdin (fd 0)".to_string(),
        Some(f) if f == "1" => "stdout (fd 1)".to_string(),
        Some(f) if f == "2" => "stderr (fd 2)".to_string(),
        Some(f) if f.starts_with('{') => format!("the descriptor allocated into {}", f),
        Some(f) => format!("fd {}", f),
    }
}

/// `(kind, plain-English description)` for one redirection.
fn redirect_meaning(r: &Redirect, heredocs: &[Heredoc]) -> (&'static str, String) {
    let target = r
        .target
        .as_ref()
        .map(|w| w.value.clone())
        .unwrap_or_default();
    match r.op.as_str() {
        ">" => (
            "file-write",
            format!(
                "{} is written to {} (created or truncated)",
                fd_label(&r.fd, "stdout (fd 1)"),
                target
            ),
        ),
        ">|" => (
            "file-write-force",
            format!(
                "{} is written to {} (truncated even when noclobber is set)",
                fd_label(&r.fd, "stdout (fd 1)"),
                target
            ),
        ),
        ">>" => (
            "file-append",
            format!(
                "{} is appended to {}",
                fd_label(&r.fd, "stdout (fd 1)"),
                target
            ),
        ),
        "<" => (
            "file-read",
            format!(
                "{} is read from {}",
                fd_label(&r.fd, "stdin (fd 0)"),
                target
            ),
        ),
        "<>" => (
            "file-read-write",
            format!(
                "{} is opened on {} for both reading and writing",
                fd_label(&r.fd, "stdin (fd 0)"),
                target
            ),
        ),
        ">&" | "<&" => {
            let src_default = if r.op == ">&" {
                "stdout (fd 1)"
            } else {
                "stdin (fd 0)"
            };
            if target == "-" {
                (
                    "fd-close",
                    format!("{} is closed", fd_label(&r.fd, src_default)),
                )
            } else if target.chars().all(|c| c.is_ascii_digit()) && !target.is_empty() {
                (
                    "fd-duplicate",
                    format!(
                        "{} is pointed at the same place as {}",
                        fd_label(&r.fd, src_default),
                        fd_label(&Some(target.clone()), "")
                    ),
                )
            } else {
                (
                    "file-write",
                    format!(
                        "{} is written to {} (created or truncated)",
                        fd_label(&r.fd, src_default),
                        target
                    ),
                )
            }
        }
        "&>" => (
            "file-write-all",
            format!(
                "stdout (fd 1) and stderr (fd 2) are both written to {} (created or truncated)",
                target
            ),
        ),
        "&>>" => (
            "file-append-all",
            format!(
                "stdout (fd 1) and stderr (fd 2) are both appended to {}",
                target
            ),
        ),
        "<<" | "<<-" => {
            let hd = r.heredoc.and_then(|i| heredocs.get(i));
            let delim = hd.map(|h| h.delimiter.clone()).unwrap_or(target);
            let expand = hd.map(|h| h.expand).unwrap_or(true);
            (
                "heredoc",
                format!(
                    "{} is read from a here-document ending at '{}' ({}{})",
                    fd_label(&r.fd, "stdin (fd 0)"),
                    delim,
                    if expand {
                        "expansions apply"
                    } else {
                        "quoted delimiter, so nothing is expanded"
                    },
                    if r.op == "<<-" {
                        ", leading tabs stripped"
                    } else {
                        ""
                    }
                ),
            )
        }
        "<<<" => (
            "herestring",
            format!(
                "{} is read from the here-string {:?}",
                fd_label(&r.fd, "stdin (fd 0)"),
                target
            ),
        ),
        other => ("other", format!("redirection '{}' to {}", other, target)),
    }
}

fn redirect_json(r: &Redirect, heredocs: &[Heredoc]) -> Value {
    let (kind, description) = redirect_meaning(r, heredocs);
    let mut m = Map::new();
    m.insert("operator".into(), json!(r.op));
    m.insert(
        "fd".into(),
        match &r.fd {
            Some(f) => json!(f),
            None => Value::Null,
        },
    );
    m.insert("kind".into(), json!(kind));
    m.insert(
        "target".into(),
        match &r.target {
            Some(w) => json!(w.value),
            None => Value::Null,
        },
    );
    if let Some(w) = &r.target {
        m.insert("target_word".into(), w.to_json());
    }
    if let Some(idx) = r.heredoc {
        m.insert("heredoc".into(), json!(idx));
    }
    m.insert("description".into(), json!(description));
    Value::Object(m)
}

/// A one-line source rendering of a redirection, e.g. `2>&1` or `>> out.log`.
fn redirect_source(r: &Redirect) -> String {
    let fd = r.fd.clone().unwrap_or_default();
    let target = r
        .target
        .as_ref()
        .map(|w| w.text.clone())
        .unwrap_or_default();
    let glue = if r.op.ends_with('&') { "" } else { " " };
    format!("{}{}{}{}", fd, r.op, glue, target)
}

// ---------------------------------------------------------------------------
// JSON rendering
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    heredocs: &'a [Heredoc],
}

fn cmd_json(cmd: &Cmd, ctx: &Ctx) -> Value {
    match cmd {
        Cmd::Simple(c) => {
            let mut m = Map::new();
            m.insert("type".into(), json!("command"));
            m.insert(
                "name".into(),
                match c.words.first() {
                    Some(w) => json!(w.value),
                    None => Value::Null,
                },
            );
            m.insert(
                "argv".into(),
                Value::Array(c.words.iter().map(|w| json!(w.value)).collect()),
            );
            m.insert(
                "words".into(),
                Value::Array(c.words.iter().map(|w| w.to_json()).collect()),
            );
            m.insert(
                "assignments".into(),
                Value::Array(
                    c.assignments
                        .iter()
                        .map(|a| {
                            json!({
                                "name": a.name,
                                "value": a.value.value,
                                "text": a.text,
                                "quoting": a.value.quote.as_str(),
                                "expansions": a.value.expansions.iter()
                                    .map(|e| json!({ "kind": e.kind, "text": e.text }))
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect(),
                ),
            );
            m.insert(
                "redirects".into(),
                Value::Array(
                    c.redirects
                        .iter()
                        .map(|r| redirect_json(r, ctx.heredocs))
                        .collect(),
                ),
            );
            Value::Object(m)
        }
        Cmd::Subshell { body, redirects } => json!({
            "type": "subshell",
            "body": list_json(body, ctx),
            "redirects": redirects.iter().map(|r| redirect_json(r, ctx.heredocs)).collect::<Vec<_>>(),
        }),
        Cmd::Group { body, redirects } => json!({
            "type": "group",
            "body": list_json(body, ctx),
            "redirects": redirects.iter().map(|r| redirect_json(r, ctx.heredocs)).collect::<Vec<_>>(),
        }),
        Cmd::Keyword(w) => json!({ "type": "keyword", "word": w }),
    }
}

fn pipeline_json(p: &Pipeline, ctx: &Ctx) -> Value {
    if !p.negated && p.elements.len() == 1 {
        return cmd_json(&p.elements[0].cmd, ctx);
    }
    json!({
        "type": "pipeline",
        "negated": p.negated,
        "commands": p.elements.iter().map(|e| {
            let mut v = cmd_json(&e.cmd, ctx);
            if let (Some(op), Value::Object(m)) = (&e.pipe_op, &mut v) {
                m.insert("piped_from_previous".into(), json!(op));
            }
            v
        }).collect::<Vec<_>>(),
    })
}

fn and_or_json(a: &AndOr, ctx: &Ctx) -> Value {
    if a.parts.len() == 1 {
        return pipeline_json(&a.parts[0].pipeline, ctx);
    }
    json!({
        "type": "and_or",
        "parts": a.parts.iter().map(|p| json!({
            "operator": match &p.operator { Some(o) => json!(o), None => Value::Null },
            "node": pipeline_json(&p.pipeline, ctx),
        })).collect::<Vec<_>>(),
    })
}

fn list_json(l: &ListNode, ctx: &Ctx) -> Value {
    if l.statements.len() == 1 && l.statements[0].terminator.as_deref().unwrap_or(";") == ";" {
        return and_or_json(&l.statements[0].and_or, ctx);
    }
    json!({
        "type": "list",
        "statements": l.statements.iter().map(|s| json!({
            "terminator": match &s.terminator { Some(t) => json!(t), None => Value::Null },
            "background": s.terminator.as_deref() == Some("&"),
            "node": and_or_json(&s.and_or, ctx),
        })).collect::<Vec<_>>(),
    })
}

// ---------------------------------------------------------------------------
// Tree rendering
// ---------------------------------------------------------------------------

fn quote_note(w: &Word) -> String {
    let mut bits = Vec::new();
    match w.quote {
        Quote::Single => bits.push("single-quoted".to_string()),
        Quote::Double => bits.push("double-quoted".to_string()),
        Quote::AnsiC => bits.push("$'...' quoted".to_string()),
        Quote::Mixed => bits.push("mixed quoting".to_string()),
        Quote::None => {}
    }
    if w.glob {
        bits.push("glob".to_string());
    }
    for e in &w.expansions {
        bits.push(format!("{} {}", e.kind, e.text));
    }
    if bits.is_empty() {
        String::new()
    } else {
        format!("  [{}]", bits.join(", "))
    }
}

struct Tree {
    out: String,
}

impl Tree {
    fn line(&mut self, prefix: &str, last: bool, text: &str) -> String {
        self.out.push_str(prefix);
        self.out.push_str(if last { "└─ " } else { "├─ " });
        self.out.push_str(text);
        self.out.push('\n');
        format!("{}{}", prefix, if last { "   " } else { "│  " })
    }

    fn root(&mut self, text: &str) {
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn cmd(&mut self, cmd: &Cmd, prefix: &str, hd: &[Heredoc]) {
        match cmd {
            Cmd::Simple(c) => {
                let n = c.assignments.len() + c.words.len().saturating_sub(1) + c.redirects.len();
                let mut k = 0;
                for a in &c.assignments {
                    k += 1;
                    self.line(
                        prefix,
                        k == n,
                        &format!("env: {}={}{}", a.name, a.value.value, quote_note(&a.value)),
                    );
                }
                for (idx, w) in c.words.iter().enumerate().skip(1) {
                    k += 1;
                    self.line(
                        prefix,
                        k == n,
                        &format!("arg[{}]: {}{}", idx, w.value, quote_note(w)),
                    );
                }
                for r in &c.redirects {
                    k += 1;
                    let (_, desc) = redirect_meaning(r, hd);
                    self.line(
                        prefix,
                        k == n,
                        &format!("redirect: {}  — {}", redirect_source(r), desc),
                    );
                }
            }
            Cmd::Subshell { body, redirects } | Cmd::Group { body, redirects } => {
                let n = 1 + redirects.len();
                let sub = self.line(prefix, n == 1, "body");
                self.list(body, &sub, hd);
                for (k, r) in redirects.iter().enumerate() {
                    let (_, desc) = redirect_meaning(r, hd);
                    self.line(
                        prefix,
                        k + 2 == n + 1,
                        &format!("redirect: {}  — {}", redirect_source(r), desc),
                    );
                }
            }
            Cmd::Keyword(_) => {}
        }
    }

    fn cmd_label(cmd: &Cmd) -> String {
        match cmd {
            Cmd::Simple(c) => match c.words.first() {
                Some(w) => format!("command: {}", w.value),
                None => "command: (redirection only)".to_string(),
            },
            Cmd::Subshell { .. } => "subshell: ( … )".to_string(),
            Cmd::Group { .. } => "group: { … }".to_string(),
            Cmd::Keyword(w) => format!("keyword: {}", w),
        }
    }

    fn pipeline(&mut self, p: &Pipeline, prefix: &str, hd: &[Heredoc], root: bool) {
        if !p.negated && p.elements.len() == 1 {
            let label = Tree::cmd_label(&p.elements[0].cmd);
            if root {
                self.root(&label);
                self.cmd(&p.elements[0].cmd, prefix, hd);
            } else {
                let sub = self.line(prefix, true, &label);
                self.cmd(&p.elements[0].cmd, &sub, hd);
            }
            return;
        }
        let label = format!(
            "{}pipeline ({} commands)",
            if p.negated { "! negated " } else { "" },
            p.elements.len()
        );
        let base = if root {
            self.root(&label);
            prefix.to_string()
        } else {
            self.line(prefix, true, &label)
        };
        for (k, e) in p.elements.iter().enumerate() {
            let last = k + 1 == p.elements.len();
            let head = match &e.pipe_op {
                Some(op) => format!("{} {}", op, Tree::cmd_label(&e.cmd)),
                None => Tree::cmd_label(&e.cmd),
            };
            let sub = self.line(&base, last, &head);
            self.cmd(&e.cmd, &sub, hd);
        }
    }

    fn and_or(&mut self, a: &AndOr, prefix: &str, hd: &[Heredoc], root: bool) {
        if a.parts.len() == 1 {
            self.pipeline(&a.parts[0].pipeline, prefix, hd, root);
            return;
        }
        let label = "and-or list".to_string();
        let base = if root {
            self.root(&label);
            prefix.to_string()
        } else {
            self.line(prefix, true, &label)
        };
        for (k, p) in a.parts.iter().enumerate() {
            let last = k + 1 == a.parts.len();
            let head = match &p.operator {
                Some(o) => format!(
                    "{} {}",
                    o,
                    if o == "&&" {
                        "(run only if the previous succeeded)"
                    } else {
                        "(run only if the previous failed)"
                    }
                ),
                None => "first".to_string(),
            };
            let sub = self.line(&base, last, &head);
            self.pipeline(&p.pipeline, &sub, hd, false);
        }
    }

    fn list(&mut self, l: &ListNode, prefix: &str, hd: &[Heredoc]) {
        for (k, s) in l.statements.iter().enumerate() {
            let last = k + 1 == l.statements.len();
            let head = format!(
                "statement {}{}",
                k + 1,
                if s.terminator.as_deref() == Some("&") {
                    " (background &)"
                } else {
                    ""
                }
            );
            let sub = self.line(prefix, last, &head);
            self.and_or(&s.and_or, &sub, hd, false);
        }
    }

    fn root_list(&mut self, l: &ListNode, hd: &[Heredoc]) {
        if l.statements.len() == 1 && l.statements[0].terminator.as_deref() != Some("&") {
            self.and_or(&l.statements[0].and_or, "", hd, true);
        } else {
            self.root(&format!("list ({} statements)", l.statements.len()));
            self.list(l, "", hd);
        }
    }
}

// ---------------------------------------------------------------------------
// Explain rendering
// ---------------------------------------------------------------------------

fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@+,%".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn argv_line(c: &SimpleCmd) -> String {
    let mut parts: Vec<String> = c
        .assignments
        .iter()
        .map(|a| format!("{}={}", a.name, shell_quote(&a.value.value)))
        .collect();
    parts.extend(c.words.iter().map(|w| shell_quote(&w.value)));
    parts.join(" ")
}

struct Explain {
    out: String,
    step: usize,
}

impl Explain {
    fn bullet(&mut self, indent: usize, text: &str) {
        self.out
            .push_str(&format!("{}- {}\n", " ".repeat(indent), text));
    }

    fn cmd(&mut self, cmd: &Cmd, indent: usize, hd: &[Heredoc], lead: &str) {
        match cmd {
            Cmd::Simple(c) => {
                let name = c
                    .words
                    .first()
                    .map(|w| w.value.clone())
                    .unwrap_or_else(|| "(redirection only)".to_string());
                self.step += 1;
                self.out.push_str(&format!(
                    "{}{}{} runs `{}`\n",
                    " ".repeat(indent),
                    self.step,
                    if lead.is_empty() {
                        ".".to_string()
                    } else {
                        format!(". {}", lead)
                    },
                    argv_line(c)
                ));
                if !c.words.is_empty() {
                    self.bullet(
                        indent + 3,
                        &format!("program: {} · {} argument(s)", name, c.words.len() - 1),
                    );
                }
                for a in &c.assignments {
                    self.bullet(
                        indent + 3,
                        &format!(
                            "environment: {} is set to {:?} for this command only",
                            a.name, a.value.value
                        ),
                    );
                }
                for (idx, w) in c.words.iter().enumerate().skip(1) {
                    let mut notes = Vec::new();
                    match w.quote {
                        Quote::Single => {
                            notes.push("single-quoted, so nothing inside is expanded".to_string())
                        }
                        Quote::Double => notes.push(
                            "double-quoted, so it stays one argument and variables still expand"
                                .to_string(),
                        ),
                        Quote::AnsiC => notes
                            .push("$'...' quoted, so backslash escapes are decoded".to_string()),
                        Quote::Mixed => notes.push("mixed quoting".to_string()),
                        Quote::None => {}
                    }
                    if w.glob {
                        notes.push(
                            "contains a glob, so the shell expands it to matching filenames"
                                .to_string(),
                        );
                    }
                    for e in &w.expansions {
                        notes.push(match e.kind {
                            "command" => format!("command substitution {}", e.text),
                            "arithmetic" => format!("arithmetic expansion {}", e.text),
                            "process" => format!("process substitution {}", e.text),
                            _ => format!("parameter expansion {}", e.text),
                        });
                    }
                    if !notes.is_empty() {
                        self.bullet(
                            indent + 3,
                            &format!("argument {} ({}): {}", idx, w.value, notes.join("; ")),
                        );
                    }
                }
                for r in &c.redirects {
                    let (_, desc) = redirect_meaning(r, hd);
                    self.bullet(indent + 3, &format!("`{}` — {}", redirect_source(r), desc));
                    if let Some(h) = r.heredoc.and_then(|i| hd.get(i)) {
                        for l in h.body.lines() {
                            self.bullet(indent + 6, &format!("here-doc line: {}", l));
                        }
                    }
                }
            }
            Cmd::Subshell { body, redirects } | Cmd::Group { body, redirects } => {
                let kind = if matches!(cmd, Cmd::Subshell { .. }) {
                    "a subshell ( … ) — it runs in its own process, so cd and variable changes do not leak out"
                } else {
                    "a brace group { … } — it runs in the current shell, so cd and variable changes persist"
                };
                self.out.push_str(&format!(
                    "{}{} {}\n",
                    " ".repeat(indent),
                    if lead.is_empty() { "Then" } else { lead },
                    kind
                ));
                self.list(body, indent + 3, hd);
                for r in redirects {
                    let (_, desc) = redirect_meaning(r, hd);
                    self.bullet(indent + 3, &format!("`{}` — {}", redirect_source(r), desc));
                }
            }
            Cmd::Keyword(w) => {
                self.out.push_str(&format!(
                    "{}shell keyword `{}` (control flow)\n",
                    " ".repeat(indent),
                    w
                ));
            }
        }
    }

    fn pipeline(&mut self, p: &Pipeline, indent: usize, hd: &[Heredoc], lead: &str) {
        if p.negated {
            self.out.push_str(&format!(
                "{}`!` inverts the exit status of the following pipeline\n",
                " ".repeat(indent)
            ));
        }
        for (k, e) in p.elements.iter().enumerate() {
            let lead = match &e.pipe_op {
                Some(op) if op == "|&" => {
                    "its stdout AND stderr are piped (|&) into the next command, which".to_string()
                }
                Some(_) => "its stdout is piped (|) into the next command, which".to_string(),
                None => lead.to_string(),
            };
            let _ = k;
            self.cmd(&e.cmd, indent, hd, &lead);
        }
    }

    fn and_or(&mut self, a: &AndOr, indent: usize, hd: &[Heredoc]) {
        for p in &a.parts {
            let lead = match &p.operator {
                Some(o) if o == "&&" => "only if everything above succeeded (&&),".to_string(),
                Some(_) => "only if the previous part FAILED (||),".to_string(),
                None => String::new(),
            };
            self.pipeline(&p.pipeline, indent, hd, &lead);
        }
    }

    fn list(&mut self, l: &ListNode, indent: usize, hd: &[Heredoc]) {
        for s in &l.statements {
            self.and_or(&s.and_or, indent, hd);
            if s.terminator.as_deref() == Some("&") {
                self.bullet(
                    indent + 3,
                    "`&` — this statement runs in the background; the shell does not wait for it",
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Flat command listing
// ---------------------------------------------------------------------------

fn collect_commands<'a>(l: &'a ListNode, depth: usize, out: &mut Vec<(usize, &'a SimpleCmd)>) {
    for s in &l.statements {
        for p in &s.and_or.parts {
            for e in &p.pipeline.elements {
                match &e.cmd {
                    Cmd::Simple(c) => out.push((depth, c)),
                    Cmd::Subshell { body, .. } | Cmd::Group { body, .. } => {
                        collect_commands(body, depth + 1, out)
                    }
                    Cmd::Keyword(_) => {}
                }
            }
        }
    }
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - n))
    }
}

fn commands_table(l: &ListNode) -> String {
    let mut cmds = Vec::new();
    collect_commands(l, 0, &mut cmds);
    let rows: Vec<[String; 5]> = cmds
        .iter()
        .enumerate()
        .map(|(k, (depth, c))| {
            let name = c
                .words
                .first()
                .map(|w| w.value.clone())
                .unwrap_or_else(|| "(redirection only)".to_string());
            let args = c
                .words
                .iter()
                .skip(1)
                .map(|w| shell_quote(&w.value))
                .collect::<Vec<_>>()
                .join(" ");
            let env = c
                .assignments
                .iter()
                .map(|a| format!("{}={}", a.name, a.value.value))
                .collect::<Vec<_>>()
                .join(" ");
            let red = c
                .redirects
                .iter()
                .map(redirect_source)
                .collect::<Vec<_>>()
                .join(" ");
            [
                format!("{}", k + 1),
                format!("{}{}", "  ".repeat(*depth), name),
                if args.is_empty() { "-".into() } else { args },
                if red.is_empty() { "-".into() } else { red },
                if env.is_empty() { "-".into() } else { env },
            ]
        })
        .collect();
    let headers = ["#", "COMMAND", "ARGUMENTS", "REDIRECTS", "ENV"];
    let mut widths = headers.map(|h| h.chars().count());
    for r in &rows {
        for k in 0..5 {
            widths[k] = widths[k].max(r[k].chars().count());
        }
    }
    let mut out = String::new();
    for k in 0..5 {
        out.push_str(&pad(headers[k], widths[k]));
        if k < 4 {
            out.push_str("  ");
        }
    }
    out.push('\n');
    for r in &rows {
        for k in 0..5 {
            out.push_str(&pad(&r[k], widths[k]));
            if k < 4 {
                out.push_str("  ");
            }
        }
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub const FORMATS: [&str; 4] = ["json", "tree", "explain", "commands"];

/// Parses `command` and renders it in `format`
/// (`json` | `tree` | `explain` | `commands`). `pretty` only affects `json`.
pub fn run(command: &str, format: &str, pretty: bool) -> Result<String, String> {
    let format = if format.trim().is_empty() {
        "json"
    } else {
        format.trim()
    };
    if !FORMATS.contains(&format) {
        return Err(format!(
            "expected format to be one of json, tree, explain, commands — got {:?}",
            format
        ));
    }
    if command.len() > MAX_INPUT {
        return Err(format!(
            "command is {} bytes, which is over the {} byte limit",
            command.len(),
            MAX_INPUT
        ));
    }
    if command.trim().is_empty() {
        return Err(
            "command is empty — paste a shell command line, e.g. grep -rn \"FIXME\" src/ | sort -u > matches.txt"
                .to_string(),
        );
    }

    let (toks, heredocs, mut notes) = Lexer::new(command).run()?;
    if toks
        .iter()
        .all(|t| matches!(t, Tok::Op(o) if o == "\n" || o == ";"))
    {
        return Err(
            "no command found — the input is only comments, blank lines or separators".to_string(),
        );
    }
    let mut parser = Parser {
        toks,
        i: 0,
        notes: Vec::new(),
        command_count: 0,
        max_depth: 0,
    };
    let list = parser.parse_list(0)?;
    if let Some(tok) = parser.peek() {
        return Err(match tok {
            Tok::Op(o) if o == ")" => "unexpected ')' — no matching '(' was opened".to_string(),
            Tok::Op(o) => format!("unexpected operator '{}'", o),
            Tok::Word(w) => format!("unexpected word '{}'", w.text),
            Tok::Redir { op, .. } => format!("unexpected redirection '{}'", op),
        });
    }
    notes.extend(parser.notes);

    let ctx = Ctx {
        heredocs: &heredocs,
    };
    match format {
        "tree" => {
            let mut t = Tree { out: String::new() };
            t.root_list(&list, &heredocs);
            for n in &notes {
                t.out.push_str(&format!("\nnote: {}\n", n));
            }
            Ok(t.out.trim_end().to_string())
        }
        "explain" => {
            let mut e = Explain {
                out: String::new(),
                step: 0,
            };
            e.list(&list, 0, &heredocs);
            for n in &notes {
                e.out.push_str(&format!("\nnote: {}\n", n));
            }
            Ok(e.out.trim_end().to_string())
        }
        "commands" => Ok(commands_table(&list).trim_end().to_string()),
        _ => {
            let mut cmds = Vec::new();
            collect_commands(&list, 0, &mut cmds);
            let doc = json!({
                "input": command,
                "statements": list.statements.len(),
                "commands": cmds.len(),
                "max_nesting": parser.max_depth,
                "heredocs": heredocs.iter().map(|h| json!({
                    "delimiter": h.delimiter,
                    "strip_tabs": h.strip_tabs,
                    "expands": h.expand,
                    "terminated": h.terminated,
                    "body": h.body,
                })).collect::<Vec<_>>(),
                "notes": notes,
                "tree": list_json(&list, &ctx),
            });
            if pretty {
                serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
            } else {
                serde_json::to_string(&doc).map_err(|e| e.to_string())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(cmd: &str) -> Value {
        serde_json::from_str(&run(cmd, "json", false).unwrap()).unwrap()
    }

    #[test]
    fn happy_simple_command() {
        let v = parse("ls -la /tmp");
        assert_eq!(v["tree"]["type"], "command");
        assert_eq!(v["tree"]["name"], "ls");
        assert_eq!(v["tree"]["argv"], json!(["ls", "-la", "/tmp"]));
        assert_eq!(v["commands"], 1);
        assert_eq!(v["statements"], 1);
    }

    #[test]
    fn happy_pipeline_with_redirect_and_env() {
        let v = parse("LC_ALL=C grep -rn 'FIXME' src/ 2>/dev/null | sort -u > matches.txt");
        let p = &v["tree"];
        assert_eq!(p["type"], "pipeline");
        assert_eq!(p["commands"][0]["name"], "grep");
        assert_eq!(p["commands"][0]["assignments"][0]["name"], "LC_ALL");
        assert_eq!(p["commands"][0]["assignments"][0]["value"], "C");
        assert_eq!(p["commands"][0]["redirects"][0]["fd"], "2");
        assert_eq!(p["commands"][0]["redirects"][0]["kind"], "file-write");
        assert_eq!(p["commands"][0]["redirects"][0]["target"], "/dev/null");
        assert_eq!(p["commands"][1]["piped_from_previous"], "|");
        assert_eq!(p["commands"][1]["redirects"][0]["kind"], "file-write");
        assert_eq!(p["commands"][1]["redirects"][0]["target"], "matches.txt");
        // 'FIXME' keeps its single quoting, and the value drops the quotes.
        assert_eq!(p["commands"][0]["words"][2]["quoting"], "single");
        assert_eq!(p["commands"][0]["words"][2]["value"], "FIXME");
    }

    #[test]
    fn happy_and_or_and_background() {
        let v = parse("make build && ./run.sh & echo started");
        assert_eq!(v["tree"]["type"], "list");
        assert_eq!(v["tree"]["statements"][0]["background"], true);
        assert_eq!(v["tree"]["statements"][0]["node"]["type"], "and_or");
        assert_eq!(
            v["tree"]["statements"][0]["node"]["parts"][1]["operator"],
            "&&"
        );
        assert_eq!(v["tree"]["statements"][1]["node"]["name"], "echo");
    }

    #[test]
    fn happy_dup_and_append_redirects() {
        let v = parse("./job.sh >> run.log 2>&1");
        let r = &v["tree"]["redirects"];
        assert_eq!(r[0]["kind"], "file-append");
        assert_eq!(r[1]["kind"], "fd-duplicate");
        assert_eq!(r[1]["fd"], "2");
        assert_eq!(r[1]["target"], "1");
        assert!(r[1]["description"]
            .as_str()
            .unwrap()
            .contains("stderr (fd 2)"));
    }

    #[test]
    fn happy_subshell_and_group() {
        let v = parse("(cd /tmp && ls) | wc -l");
        assert_eq!(v["tree"]["type"], "pipeline");
        assert_eq!(v["tree"]["commands"][0]["type"], "subshell");
        assert_eq!(v["max_nesting"], 1);
        let g = parse("{ echo a; echo b; } > both.txt");
        assert_eq!(g["tree"]["type"], "group");
        assert_eq!(g["tree"]["redirects"][0]["target"], "both.txt");
        assert_eq!(g["tree"]["body"]["statements"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn happy_expansions_globs_and_quoting() {
        let v = parse(r#"echo "$HOME/$(date +%F)" 'raw $x' *.txt"#);
        let w = &v["tree"]["words"];
        assert_eq!(w[1]["quoting"], "double");
        assert_eq!(w[1]["expansions"][0]["kind"], "parameter");
        assert_eq!(w[1]["expansions"][0]["text"], "$HOME");
        assert_eq!(w[1]["expansions"][1]["kind"], "command");
        assert_eq!(w[1]["expansions"][1]["text"], "$(date +%F)");
        assert_eq!(w[2]["quoting"], "single");
        assert_eq!(w[2]["value"], "raw $x");
        assert_eq!(w[2]["expansions"], json!([]));
        assert_eq!(w[3]["glob"], true);
    }

    #[test]
    fn happy_heredoc_body_is_captured() {
        let v = parse("cat <<'EOF' > f.txt\nline $one\nline two\nEOF\necho done");
        assert_eq!(v["heredocs"][0]["delimiter"], "EOF");
        assert_eq!(v["heredocs"][0]["expands"], false);
        assert_eq!(v["heredocs"][0]["terminated"], true);
        assert_eq!(v["heredocs"][0]["body"], "line $one\nline two\n");
        assert_eq!(v["tree"]["statements"][1]["node"]["name"], "echo");
    }

    #[test]
    fn happy_process_substitution_stays_a_word() {
        let v = parse("diff <(sort a.txt) <(sort b.txt)");
        assert_eq!(v["tree"]["name"], "diff");
        assert_eq!(
            v["tree"]["argv"],
            json!(["diff", "<(sort a.txt)", "<(sort b.txt)"])
        );
        assert_eq!(v["tree"]["words"][1]["expansions"][0]["kind"], "process");
    }

    #[test]
    fn happy_comments_and_line_continuations() {
        let v = parse("tar -czf out.tgz \\\n  src/ # archive the sources\n");
        assert_eq!(v["tree"]["argv"], json!(["tar", "-czf", "out.tgz", "src/"]));
    }

    #[test]
    fn happy_tree_format() {
        let out = run("grep -rn FIXME src/ | wc -l", "tree", true).unwrap();
        assert!(out.starts_with("pipeline (2 commands)"), "{}", out);
        assert!(out.contains("| command: wc"), "{}", out);
    }

    #[test]
    fn happy_explain_format() {
        let out = run("cat f.txt | sort -u > s.txt", "explain", true).unwrap();
        assert!(out.contains("1. runs `cat f.txt`"), "{}", out);
        assert!(out.contains("stdout is piped (|)"), "{}", out);
        assert!(
            out.contains("stdout (fd 1) is written to s.txt (created or truncated)"),
            "{}",
            out
        );
    }

    #[test]
    fn happy_commands_format() {
        let out = run("a -1 | b -2 > o.txt", "commands", true).unwrap();
        assert!(out.lines().next().unwrap().starts_with("#  COMMAND"));
        assert_eq!(out.lines().count(), 3);
        assert!(out.contains("> o.txt"), "{}", out);
    }

    #[test]
    fn error_empty_input() {
        let e = run("   ", "json", true).unwrap_err();
        assert!(e.contains("command is empty"), "{}", e);
    }

    #[test]
    fn error_unterminated_single_quote() {
        let e = run("echo 'oops", "json", true).unwrap_err();
        assert!(e.contains("unterminated single quote"), "{}", e);
    }

    #[test]
    fn error_unterminated_double_quote() {
        let e = run("echo \"oops", "json", true).unwrap_err();
        assert!(e.contains("unterminated double quote"), "{}", e);
    }

    #[test]
    fn error_dangling_pipe() {
        let e = run("ls |", "json", true).unwrap_err();
        assert!(e.contains("expected a command after '|'"), "{}", e);
    }

    #[test]
    fn error_redirect_without_target() {
        let e = run("ls > ", "json", true).unwrap_err();
        assert!(
            e.contains("expected a target after the redirection '>'"),
            "{}",
            e
        );
    }

    #[test]
    fn error_unmatched_paren() {
        let e = run("(cd /tmp && ls", "json", true).unwrap_err();
        assert!(e.contains("closing ')'"), "{}", e);
        let e2 = run("ls)", "json", true).unwrap_err();
        assert!(e2.contains("no matching '('"), "{}", e2);
    }

    #[test]
    fn error_unknown_format() {
        let e = run("ls", "yaml", true).unwrap_err();
        assert!(e.contains("expected format to be one of"), "{}", e);
    }

    #[test]
    fn error_input_too_long() {
        let big = "a ".repeat(MAX_INPUT);
        let e = run(&big, "json", true).unwrap_err();
        assert!(e.contains("byte limit"), "{}", e);
    }

    #[test]
    fn unterminated_heredoc_is_a_note_not_an_error() {
        let v = parse("cat <<EOF\nbody\n");
        assert_eq!(v["heredocs"][0]["terminated"], false);
        assert!(v["notes"][0].as_str().unwrap().contains("was never found"));
    }

    #[test]
    fn keyword_nodes_carry_a_note() {
        let v = parse("for f in *.txt; do echo $f; done");
        assert_eq!(v["tree"]["statements"][0]["node"]["type"], "keyword");
        assert!(v["notes"][0].as_str().unwrap().starts_with("control-flow"));
    }

    #[test]
    fn quoted_assignment_is_an_argument_not_env() {
        let v = parse("echo A=1");
        assert_eq!(v["tree"]["argv"], json!(["echo", "A=1"]));
        assert_eq!(v["tree"]["assignments"], json!([]));
    }

    #[test]
    fn ampersand_redirects_both_streams() {
        let v = parse("./x &> all.log");
        assert_eq!(v["tree"]["redirects"][0]["kind"], "file-write-all");
        assert_eq!(v["tree"]["redirects"][0]["target"], "all.log");
    }

    #[test]
    fn herestring_and_negation() {
        let v = parse("! grep -q x <<< \"$line\"");
        assert_eq!(v["tree"]["type"], "pipeline");
        assert_eq!(v["tree"]["negated"], true);
        assert_eq!(
            v["tree"]["commands"][0]["redirects"][0]["kind"],
            "herestring"
        );
    }

    #[test]
    fn json_is_pretty_when_asked() {
        assert!(run("ls", "json", true).unwrap().contains("\n  \"input\""));
        assert!(!run("ls", "json", false).unwrap().contains('\n'));
    }
}
