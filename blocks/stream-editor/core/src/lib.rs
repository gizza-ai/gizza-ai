//! stream-editor core — a browser-local ed/sed-style stream editor.
//!
//! Parses a sed command script (addresses + commands) once, then runs it over the
//! input text in a single pass using sed's cycle model: each input line becomes the
//! pattern space, every command whose address matches is applied in order, and the
//! pattern space is auto-printed at the end of the cycle unless quiet mode is on.
//!
//! Everything is pure compute — no filesystem, no shell — so the file/shell commands
//! (`r`, `R`, `w`, `W`, `e`, `F`) report a clear "not available" error instead of
//! silently doing nothing.

use regex::{Captures, Regex, RegexBuilder};
use std::collections::HashMap;

/// Script shown as the descriptor/page default: rename a word, then drop blank lines.
pub const DEFAULT_SCRIPT: &str = "s/foo/bar/g\n/^[[:space:]]*$/d";
/// Default safety cap on emitted lines.
pub const DEFAULT_MAX_OUTPUT_LINES: usize = 100_000;
/// Executed-command cap — catches runaway `b`/`t` branch loops.
const STEP_LIMIT: usize = 20_000_000;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Which regular-expression dialect the script's patterns are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegexFlavor {
    /// POSIX basic regular expressions — what plain `sed` uses (`\(group\)`, `\|`, `\+`).
    Basic,
    /// Extended regular expressions — what `sed -E` uses (`(group)`, `|`, `+`).
    Extended,
}

impl RegexFlavor {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "basic" | "bre" | "sed" => Ok(Self::Basic),
            "extended" | "ere" | "modern" => Ok(Self::Extended),
            other => Err(format!(
                "regex_flavor must be \"basic\" or \"extended\" (got {other:?})"
            )),
        }
    }
}

/// Line terminator used to join the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "lf" | "unix" | "\\n" => Ok(Self::Lf),
            "crlf" | "windows" | "dos" | "\\r\\n" => Ok(Self::Crlf),
            other => Err(format!(
                "line_ending must be \"lf\" or \"crlf\" (got {other:?})"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    /// `sed -n`: suppress the automatic end-of-cycle print; only `p`/`P`/`=`/`l` output.
    pub quiet: bool,
    /// Fold case in every pattern of the script.
    pub ignore_case: bool,
    /// Load the whole input into one pattern space instead of one line per cycle.
    pub whole_buffer: bool,
    pub flavor: RegexFlavor,
    pub line_ending: LineEnding,
    pub max_output_lines: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            quiet: false,
            ignore_case: false,
            whole_buffer: false,
            flavor: RegexFlavor::Basic,
            line_ending: LineEnding::Lf,
            max_output_lines: DEFAULT_MAX_OUTPUT_LINES,
        }
    }
}

// ---------------------------------------------------------------------------
// Program representation
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Pat {
    Re(Regex),
    /// The empty regex `//` — reuse the most recently applied one.
    Last,
}

#[derive(Clone)]
enum Addr {
    Line(usize),
    Last,
    Pat(Pat),
    /// GNU `first~step`.
    Step {
        first: usize,
        step: usize,
    },
}

#[derive(Clone)]
enum EndAddr {
    Addr(Addr),
    /// GNU `addr,+N`.
    Plus(usize),
    /// GNU `addr,~N`.
    Multiple(usize),
}

#[derive(Clone)]
enum AddrSpec {
    None,
    One(Addr),
    Range {
        start: Addr,
        end: EndAddr,
        /// `0,/re/` — the end regex may already match on line 1.
        zero_start: bool,
    },
}

#[derive(Clone)]
enum ReplPart {
    Lit(String),
    Group(usize),
    Whole,
    Upper,
    Lower,
    UpperOne,
    LowerOne,
    EndCase,
}

#[derive(Clone)]
struct Subst {
    pat: Pat,
    repl: Vec<ReplPart>,
    global: bool,
    occurrence: usize,
    print: bool,
}

#[derive(Clone)]
enum Cmd {
    BlockStart { end: usize },
    BlockEnd,
    Label(String),
    Branch(Option<String>),
    BranchIfSub(Option<String>),
    BranchIfNoSub(Option<String>),
    Subst(Subst),
    Trans { from: Vec<char>, to: Vec<char> },
    Delete,
    DeleteFirst,
    Print,
    PrintFirst,
    Insert(String),
    AppendText(String),
    Change(String),
    Next,
    NextAppend,
    Hold,
    HoldAppend,
    Get,
    GetAppend,
    Exchange,
    Quit { print: bool },
    LineNum,
    List(usize),
    Zap,
    Noop,
}

struct Instr {
    addr: AddrSpec,
    negate: bool,
    cmd: Cmd,
}

struct Program {
    instrs: Vec<Instr>,
    labels: HashMap<String, usize>,
    /// A leading `#n` line means quiet mode.
    quiet: bool,
}

// ---------------------------------------------------------------------------
// Regex translation (BRE / ERE → the Rust regex engine)
// ---------------------------------------------------------------------------

fn is_meta(c: char) -> bool {
    matches!(
        c,
        '.' | '*' | '[' | ']' | '^' | '$' | '\\' | '(' | ')' | '{' | '}' | '|' | '+' | '?'
    )
}

/// Copy a `[...]` bracket expression verbatim, honouring POSIX quirks (`[]a]`,
/// `[^]a]`, `[[:alpha:]]`). `i` points at the `[`; returns the index after `]`.
fn copy_bracket(src: &[char], mut i: usize, out: &mut String) -> Result<usize, String> {
    let start = i;
    out.push('[');
    i += 1;
    if src.get(i) == Some(&'^') {
        out.push('^');
        i += 1;
    }
    if src.get(i) == Some(&']') {
        out.push_str("\\]");
        i += 1;
    }
    while i < src.len() {
        let c = src[i];
        if c == '[' && matches!(src.get(i + 1), Some(':') | Some('.') | Some('=')) {
            let kind = src[i + 1];
            let close: Vec<char> = vec![kind, ']'];
            let mut j = i + 2;
            while j + 1 < src.len() && !(src[j] == close[0] && src[j + 1] == close[1]) {
                j += 1;
            }
            if j + 1 >= src.len() {
                return Err(format!(
                    "unterminated [{kind}...{kind}] class inside the bracket expression starting at position {start}"
                ));
            }
            for c in &src[i..=j + 1] {
                out.push(*c);
            }
            i = j + 2;
            continue;
        }
        if c == ']' {
            out.push(']');
            return Ok(i + 1);
        }
        if c == '\\' {
            // POSIX brackets take backslash literally; the Rust engine does not.
            out.push_str("\\\\");
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    Err("unterminated bracket expression — a '[' has no matching ']'".to_string())
}

/// Rewrite a sed pattern into Rust regex syntax.
fn translate_pattern(pattern: &str, flavor: RegexFlavor) -> Result<String, String> {
    let src: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut i = 0usize;
    // Tracks whether we are at a position where `*` / `^` are not yet quantifiable.
    let mut at_start = true;
    while i < src.len() {
        let c = src[i];
        match c {
            '\\' => {
                let n = match src.get(i + 1) {
                    Some(n) => *n,
                    None => return Err("script pattern ends with a lone backslash".to_string()),
                };
                i += 2;
                match n {
                    '1'..='9' => {
                        return Err(format!(
                            "backreference \\{n} in a pattern is not supported by the browser regex engine (replacements may still use \\{n})"
                        ))
                    }
                    '<' | '>' | 'b' => out.push_str("\\b"),
                    '`' => out.push_str("\\A"),
                    '\'' => out.push_str("\\z"),
                    '(' | ')' | '{' | '}' | '|' | '+' | '?' => {
                        if flavor == RegexFlavor::Basic {
                            // In BRE these escapes carry the special meaning.
                            out.push(n);
                        } else {
                            out.push('\\');
                            out.push(n);
                        }
                    }
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    _ => {
                        out.push('\\');
                        out.push(n);
                    }
                }
                at_start = matches!(n, '(' | '|') && flavor == RegexFlavor::Basic;
            }
            '[' => {
                i = copy_bracket(&src, i, &mut out)?;
                at_start = false;
            }
            '^' => {
                if flavor == RegexFlavor::Basic && !at_start {
                    out.push_str("\\^");
                } else {
                    out.push('^');
                }
                i += 1;
            }
            '$' => {
                let ere = flavor == RegexFlavor::Extended;
                let tail = i + 1 >= src.len()
                    || (flavor == RegexFlavor::Basic
                        && src[i + 1] == '\\'
                        && matches!(src.get(i + 2), Some(')') | Some('|')))
                    || (ere && matches!(src[i + 1], ')' | '|'));
                if tail {
                    out.push('$');
                } else {
                    out.push_str("\\$");
                }
                i += 1;
                at_start = false;
            }
            '*' => {
                if at_start {
                    out.push_str("\\*");
                } else {
                    out.push('*');
                }
                i += 1;
                at_start = false;
            }
            '(' | ')' | '{' | '}' | '|' | '+' | '?' => {
                if flavor == RegexFlavor::Basic {
                    out.push('\\');
                    out.push(c);
                    at_start = false;
                } else {
                    out.push(c);
                    at_start = matches!(c, '(' | '|');
                }
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
                at_start = false;
            }
        }
    }
    Ok(out)
}

fn build_regex(
    pattern: &str,
    flavor: RegexFlavor,
    icase: bool,
    multiline: bool,
) -> Result<Regex, String> {
    let translated = translate_pattern(pattern, flavor)?;
    RegexBuilder::new(&translated)
        .case_insensitive(icase)
        // sed's `.` matches newlines inside the pattern space; `M` flips both switches.
        .dot_matches_new_line(!multiline)
        .multi_line(multiline)
        .size_limit(8 << 20)
        .build()
        .map_err(|e| {
            let first = e
                .to_string()
                .lines()
                .last()
                .unwrap_or("invalid")
                .to_string();
            format!("invalid regular expression /{pattern}/: {first}")
        })
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    src: Vec<char>,
    i: usize,
    opts: &'a Options,
}

impl<'a> Parser<'a> {
    fn new(script: &str, opts: &'a Options) -> Self {
        Self {
            src: script.chars().collect(),
            i: 0,
            opts,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src.get(self.i).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }

    fn skip_blanks(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.i += 1;
        }
    }

    fn skip_separators(&mut self) {
        while matches!(
            self.peek(),
            Some(' ') | Some('\t') | Some('\n') | Some('\r') | Some(';')
        ) {
            self.i += 1;
        }
    }

    fn rest_of_line(&mut self) -> String {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.i += 1;
        }
        self.src[start..self.i].iter().collect()
    }

    fn number(&mut self) -> usize {
        let mut n = 0usize;
        while let Some(c) = self.peek() {
            match c.to_digit(10) {
                Some(d) => {
                    n = n.saturating_mul(10).saturating_add(d as usize);
                    self.i += 1;
                }
                None => break,
            }
        }
        n
    }

    /// Read a `<delim>…<delim>` chunk, resolving `\<delim>` to a literal delimiter.
    fn delimited(&mut self, delim: char, what: &str) -> Result<String, String> {
        let mut out = String::new();
        loop {
            let c = match self.bump() {
                Some(c) => c,
                None => {
                    return Err(format!(
                        "unterminated {what}: expected a closing {delim:?} delimiter"
                    ))
                }
            };
            if c == '\\' {
                match self.bump() {
                    Some(n) if n == delim => {
                        // An escaped delimiter is the literal character; keep the
                        // backslash when that character is a regex metacharacter.
                        if is_meta(n) {
                            out.push('\\');
                        }
                        out.push(n);
                    }
                    Some(n) => {
                        out.push('\\');
                        out.push(n);
                    }
                    None => {
                        return Err(format!("unterminated {what}: script ends with a backslash"))
                    }
                }
                continue;
            }
            if c == delim {
                return Ok(out);
            }
            if c == '\n' {
                return Err(format!(
                    "unterminated {what}: a newline appeared before the closing {delim:?}"
                ));
            }
            out.push(c);
        }
    }

    fn regex_delim(&mut self, delim: char, what: &str) -> Result<Pat, String> {
        let raw = self.delimited(delim, what)?;
        let mut icase = self.opts.ignore_case;
        let mut multiline = false;
        while let Some(c) = self.peek() {
            match c {
                'I' => {
                    icase = true;
                    self.i += 1;
                }
                'M' => {
                    multiline = true;
                    self.i += 1;
                }
                _ => break,
            }
        }
        if raw.is_empty() && !icase && !multiline {
            return Ok(Pat::Last);
        }
        Ok(Pat::Re(build_regex(
            &raw,
            self.opts.flavor,
            icase,
            multiline,
        )?))
    }

    fn parse_one_addr(&mut self) -> Result<Option<Addr>, String> {
        match self.peek() {
            Some(c) if c.is_ascii_digit() => {
                let n = self.number();
                if self.peek() == Some('~') {
                    self.i += 1;
                    let step = self.number();
                    return Ok(Some(Addr::Step { first: n, step }));
                }
                Ok(Some(Addr::Line(n)))
            }
            Some('$') => {
                self.i += 1;
                Ok(Some(Addr::Last))
            }
            Some('/') => {
                self.i += 1;
                Ok(Some(Addr::Pat(self.regex_delim('/', "address regex")?)))
            }
            Some('\\') => {
                self.i += 1;
                let delim = self
                    .bump()
                    .ok_or_else(|| "address \\ must be followed by a delimiter".to_string())?;
                Ok(Some(Addr::Pat(self.regex_delim(delim, "address regex")?)))
            }
            _ => Ok(None),
        }
    }

    fn parse_addr_spec(&mut self) -> Result<AddrSpec, String> {
        let start = match self.parse_one_addr()? {
            Some(a) => a,
            None => return Ok(AddrSpec::None),
        };
        self.skip_blanks();
        if self.peek() != Some(',') {
            return Ok(AddrSpec::One(start));
        }
        self.i += 1;
        self.skip_blanks();
        let end = match self.peek() {
            Some('+') => {
                self.i += 1;
                EndAddr::Plus(self.number())
            }
            Some('~') => {
                self.i += 1;
                let n = self.number();
                if n == 0 {
                    return Err("addr,~N requires N to be at least 1".to_string());
                }
                EndAddr::Multiple(n)
            }
            _ => match self.parse_one_addr()? {
                Some(a) => EndAddr::Addr(a),
                None => return Err("expected a second address after ','".to_string()),
            },
        };
        let zero_start = matches!(start, Addr::Line(0));
        if zero_start && !matches!(end, EndAddr::Addr(Addr::Pat(_))) {
            return Err("address 0 is only allowed as 0,/regex/".to_string());
        }
        Ok(AddrSpec::Range {
            start,
            end,
            zero_start,
        })
    }

    /// Text argument of `a`, `i`, `c` — GNU one-liner form or the classic `a\` form.
    fn parse_text(&mut self, cmd: char) -> Result<String, String> {
        self.skip_blanks();
        if self.peek() == Some('\\') {
            self.i += 1;
            if self.peek() == Some('\n') {
                self.i += 1;
            }
        }
        let mut out = String::new();
        loop {
            match self.bump() {
                None => break,
                Some('\n') => break,
                Some('\\') => match self.bump() {
                    Some('\n') => out.push('\n'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('\\') => out.push('\\'),
                    Some(c) => out.push(c),
                    None => break,
                },
                Some(c) => out.push(c),
            }
        }
        if out.is_empty() {
            return Err(format!(
                "command '{cmd}' needs text to insert, e.g. {cmd} hello (or {cmd}\\ then the text)"
            ));
        }
        Ok(out)
    }

    fn parse_label(&mut self, what: &str) -> Result<Option<String>, String> {
        self.skip_blanks();
        let start = self.i;
        while let Some(c) = self.peek() {
            if c == '\n' || c == ';' || c == '}' {
                break;
            }
            self.i += 1;
        }
        let label: String = self.src[start..self.i].iter().collect();
        let label = label.trim().to_string();
        if label.is_empty() {
            if what == "label definition" {
                return Err("':' needs a label name, e.g. :top".to_string());
            }
            return Ok(None);
        }
        Ok(Some(label))
    }

    fn parse_subst(&mut self) -> Result<Cmd, String> {
        let delim = match self.bump() {
            Some(d) if d != '\\' && d != '\n' && !d.is_alphanumeric() => d,
            Some(d) => {
                return Err(format!(
                    "s command delimiter must be punctuation such as / # | , (got {d:?})"
                ))
            }
            None => return Err("s command needs a pattern, e.g. s/old/new/".to_string()),
        };
        let raw_pat = self.delimited(delim, "s command pattern")?;
        let raw_repl = self.delimited(delim, "s command replacement")?;

        let mut global = false;
        let mut occurrence = 0usize;
        let mut print = false;
        let mut icase = self.opts.ignore_case;
        let mut multiline = false;
        while let Some(c) = self.peek() {
            match c {
                'g' => {
                    global = true;
                    self.i += 1;
                }
                'p' => {
                    print = true;
                    self.i += 1;
                }
                'i' | 'I' => {
                    icase = true;
                    self.i += 1;
                }
                'm' | 'M' => {
                    multiline = true;
                    self.i += 1;
                }
                '0'..='9' => {
                    let n = self.number();
                    if n == 0 {
                        return Err("s command occurrence flag must be 1 or more".to_string());
                    }
                    occurrence = n;
                }
                'w' | 'W' => {
                    return Err(
                        "the s///w flag writes to a file, which is not available in the browser"
                            .to_string(),
                    )
                }
                'e' => return Err(
                    "the s///e flag runs a shell command, which is not available in the browser"
                        .to_string(),
                ),
                _ => break,
            }
        }
        let pat = if raw_pat.is_empty() && !icase && !multiline {
            Pat::Last
        } else {
            Pat::Re(build_regex(&raw_pat, self.opts.flavor, icase, multiline)?)
        };
        Ok(Cmd::Subst(Subst {
            pat,
            repl: parse_replacement(&raw_repl),
            global,
            occurrence: occurrence.max(1),
            print,
        }))
    }

    fn parse_trans(&mut self) -> Result<Cmd, String> {
        let delim = match self.bump() {
            Some(d) if d != '\\' && d != '\n' && !d.is_alphanumeric() => d,
            Some(d) => {
                return Err(format!(
                    "y command delimiter must be punctuation such as / # | , (got {d:?})"
                ))
            }
            None => return Err("y command needs two sets, e.g. y/abc/xyz/".to_string()),
        };
        let from = unescape_set(&self.delimited(delim, "y command source set")?);
        let to = unescape_set(&self.delimited(delim, "y command target set")?);
        if from.len() != to.len() {
            return Err(format!(
                "y command needs both sets to be the same length (got {} and {} characters)",
                from.len(),
                to.len()
            ));
        }
        Ok(Cmd::Trans { from, to })
    }

    fn parse(mut self) -> Result<Program, String> {
        let mut instrs: Vec<Instr> = Vec::new();
        let mut labels: HashMap<String, usize> = HashMap::new();
        let mut open_blocks: Vec<usize> = Vec::new();
        let mut quiet = false;

        // A script starting with `#n` means quiet mode.
        if self.src.first() == Some(&'#') && self.src.get(1) == Some(&'n') {
            if matches!(self.src.get(2), None | Some('\n')) {
                quiet = true;
                self.i = 2;
            }
        }

        loop {
            self.skip_separators();
            let c = match self.peek() {
                Some(c) => c,
                None => break,
            };
            if c == '#' {
                self.rest_of_line();
                continue;
            }
            if c == '}' {
                self.i += 1;
                let open = open_blocks
                    .pop()
                    .ok_or_else(|| "unexpected '}' — no matching '{' is open".to_string())?;
                instrs.push(Instr {
                    addr: AddrSpec::None,
                    negate: false,
                    cmd: Cmd::BlockEnd,
                });
                let end = instrs.len() - 1;
                if let Cmd::BlockStart { end: slot } = &mut instrs[open].cmd {
                    *slot = end;
                }
                continue;
            }

            let addr = self.parse_addr_spec()?;
            self.skip_blanks();
            let mut negate = false;
            while self.peek() == Some('!') {
                negate = !negate;
                self.i += 1;
                self.skip_blanks();
            }
            let cmd_char = match self.bump() {
                Some(c) => c,
                None => return Err("script ends with an address but no command".to_string()),
            };
            let cmd = match cmd_char {
                '{' => {
                    open_blocks.push(instrs.len());
                    Cmd::BlockStart { end: 0 }
                }
                's' => self.parse_subst()?,
                'y' => self.parse_trans()?,
                'd' => Cmd::Delete,
                'D' => Cmd::DeleteFirst,
                'p' => Cmd::Print,
                'P' => Cmd::PrintFirst,
                'i' => Cmd::Insert(self.parse_text('i')?),
                'a' => Cmd::AppendText(self.parse_text('a')?),
                'c' => Cmd::Change(self.parse_text('c')?),
                'n' => Cmd::Next,
                'N' => Cmd::NextAppend,
                'h' => Cmd::Hold,
                'H' => Cmd::HoldAppend,
                'g' => Cmd::Get,
                'G' => Cmd::GetAppend,
                'x' => Cmd::Exchange,
                'z' => Cmd::Zap,
                '=' => Cmd::LineNum,
                'q' => {
                    self.skip_blanks();
                    self.number();
                    Cmd::Quit { print: true }
                }
                'Q' => {
                    self.skip_blanks();
                    self.number();
                    Cmd::Quit { print: false }
                }
                'l' => {
                    self.skip_blanks();
                    let n = if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                        self.number()
                    } else {
                        70
                    };
                    Cmd::List(n)
                }
                ':' => {
                    let name = self
                        .parse_label("label definition")?
                        .ok_or_else(|| "':' needs a label name, e.g. :top".to_string())?;
                    if labels.contains_key(&name) {
                        return Err(format!("label :{name} is defined more than once"));
                    }
                    labels.insert(name.clone(), instrs.len());
                    Cmd::Label(name)
                }
                'b' => Cmd::Branch(self.parse_label("branch target")?),
                't' => Cmd::BranchIfSub(self.parse_label("branch target")?),
                'T' => Cmd::BranchIfNoSub(self.parse_label("branch target")?),
                'v' => {
                    self.rest_of_line();
                    Cmd::Noop
                }
                'r' | 'R' | 'w' | 'W' => {
                    return Err(format!(
                        "command '{cmd_char}' reads or writes a file, which is not available in this browser-only editor"
                    ))
                }
                'e' => {
                    return Err(
                        "command 'e' runs a shell command, which is not available in this browser-only editor"
                            .to_string(),
                    )
                }
                'F' => {
                    return Err(
                        "command 'F' prints the input file name; there is no file here, the input is pasted text"
                            .to_string(),
                    )
                }
                other => {
                    return Err(format!(
                        "unknown command {other:?} — supported commands are s y d D p P i a c n N h H g G x z q Q = l b t T : {{ }} and #comments"
                    ))
                }
            };

            if matches!(cmd, Cmd::Label(_)) && !matches!(addr, AddrSpec::None) {
                return Err("a label (':') cannot take an address".to_string());
            }
            instrs.push(Instr { addr, negate, cmd });
        }

        if !open_blocks.is_empty() {
            return Err("unbalanced '{' — a block was never closed with '}'".to_string());
        }
        for (idx, instr) in instrs.iter().enumerate() {
            let target = match &instr.cmd {
                Cmd::Branch(t) | Cmd::BranchIfSub(t) | Cmd::BranchIfNoSub(t) => t,
                _ => continue,
            };
            if let Some(name) = target {
                if !labels.contains_key(name) {
                    return Err(format!(
                        "branch at command {} jumps to :{name}, which is never defined",
                        idx + 1
                    ));
                }
            }
        }
        Ok(Program {
            instrs,
            labels,
            quiet,
        })
    }
}

fn unescape_set(raw: &str) -> Vec<char> {
    let src: Vec<char> = raw.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() {
        if src[i] == '\\' && i + 1 < src.len() {
            let c = match src[i + 1] {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                other => other,
            };
            out.push(c);
            i += 2;
        } else {
            out.push(src[i]);
            i += 1;
        }
    }
    out
}

fn parse_replacement(raw: &str) -> Vec<ReplPart> {
    let src: Vec<char> = raw.chars().collect();
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    macro_rules! flush {
        () => {
            if !lit.is_empty() {
                parts.push(ReplPart::Lit(std::mem::take(&mut lit)));
            }
        };
    }
    while i < src.len() {
        match src[i] {
            '&' => {
                flush!();
                parts.push(ReplPart::Whole);
                i += 1;
            }
            '\\' if i + 1 < src.len() => {
                let n = src[i + 1];
                i += 2;
                match n {
                    '0'..='9' => {
                        flush!();
                        parts.push(ReplPart::Group(n.to_digit(10).unwrap() as usize));
                    }
                    'n' => lit.push('\n'),
                    't' => lit.push('\t'),
                    'r' => lit.push('\r'),
                    'U' => {
                        flush!();
                        parts.push(ReplPart::Upper);
                    }
                    'L' => {
                        flush!();
                        parts.push(ReplPart::Lower);
                    }
                    'u' => {
                        flush!();
                        parts.push(ReplPart::UpperOne);
                    }
                    'l' => {
                        flush!();
                        parts.push(ReplPart::LowerOne);
                    }
                    'E' => {
                        flush!();
                        parts.push(ReplPart::EndCase);
                    }
                    other => lit.push(other),
                }
            }
            c => {
                lit.push(c);
                i += 1;
            }
        }
    }
    flush!();
    parts
}

#[derive(Clone, Copy, PartialEq)]
enum Case {
    None,
    Upper,
    Lower,
}

fn push_cased(out: &mut String, text: &str, span: &mut Case, one: &mut Case) {
    for ch in text.chars() {
        let cased: String = match *one {
            Case::Upper => {
                *one = Case::None;
                ch.to_uppercase().collect()
            }
            Case::Lower => {
                *one = Case::None;
                ch.to_lowercase().collect()
            }
            Case::None => match *span {
                Case::Upper => ch.to_uppercase().collect(),
                Case::Lower => ch.to_lowercase().collect(),
                Case::None => ch.to_string(),
            },
        };
        out.push_str(&cased);
    }
}

fn expand(caps: &Captures, repl: &[ReplPart], out: &mut String) -> Result<(), String> {
    let mut span = Case::None;
    let mut one = Case::None;
    for part in repl {
        match part {
            ReplPart::Lit(s) => push_cased(out, s, &mut span, &mut one),
            ReplPart::Whole => {
                let t = caps.get(0).map(|m| m.as_str()).unwrap_or("").to_string();
                push_cased(out, &t, &mut span, &mut one);
            }
            ReplPart::Group(n) => {
                if *n >= caps.len() {
                    return Err(format!(
                        "replacement refers to \\{n} but the pattern has only {} capture group(s)",
                        caps.len().saturating_sub(1)
                    ));
                }
                let t = caps.get(*n).map(|m| m.as_str()).unwrap_or("").to_string();
                push_cased(out, &t, &mut span, &mut one);
            }
            ReplPart::Upper => {
                span = Case::Upper;
                one = Case::None;
            }
            ReplPart::Lower => {
                span = Case::Lower;
                one = Case::None;
            }
            ReplPart::UpperOne => one = Case::Upper,
            ReplPart::LowerOne => one = Case::Lower,
            ReplPart::EndCase => {
                span = Case::None;
                one = Case::None;
            }
        }
    }
    Ok(())
}

fn escape_list(s: &str, wrap: usize) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    let push = |chunk: &str, out: &mut String, col: &mut usize| {
        if wrap > 1 && *col + chunk.len() > wrap.saturating_sub(1) {
            out.push_str("\\\n");
            *col = 0;
        }
        out.push_str(chunk);
        *col += chunk.len();
    };
    for ch in s.chars() {
        let chunk = match ch {
            '\\' => "\\\\".to_string(),
            '\n' => "\\n".to_string(),
            '\t' => "\\t".to_string(),
            '\r' => "\\r".to_string(),
            '\x07' => "\\a".to_string(),
            '\x08' => "\\b".to_string(),
            '\x0c' => "\\f".to_string(),
            '\x0b' => "\\v".to_string(),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => format!("\\{:03o}", c as u32),
            c => c.to_string(),
        };
        push(&chunk, &mut out, &mut col);
    }
    out.push('$');
    out
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct RangeRt {
    active: bool,
    end_line: Option<usize>,
    /// A finished `0,/re/` range never restarts.
    done: bool,
}

struct AddrHit {
    matched: bool,
    last_of_range: bool,
}

enum Flow {
    EndCycle { auto_print: bool },
    Restart,
    Quit { auto_print: bool },
}

struct Machine<'a> {
    lines: &'a [String],
    next_idx: usize,
    pattern: String,
    hold: String,
    line_no: usize,
    sub_made: bool,
    last_re: Option<Regex>,
    ranges: Vec<RangeRt>,
    out: Vec<String>,
    appends: Vec<String>,
    quiet: bool,
    max_output_lines: usize,
    truncated: bool,
    steps: usize,
}

impl<'a> Machine<'a> {
    fn emit(&mut self, s: String) {
        if self.out.len() >= self.max_output_lines {
            self.truncated = true;
            return;
        }
        self.out.push(s);
    }

    fn is_last(&self) -> bool {
        self.next_idx >= self.lines.len()
    }

    fn read_line(&mut self) -> Option<String> {
        let l = self.lines.get(self.next_idx).cloned()?;
        self.next_idx += 1;
        self.line_no = self.next_idx;
        Some(l)
    }

    fn resolve(&mut self, pat: &Pat) -> Result<Regex, String> {
        match pat {
            Pat::Re(r) => {
                self.last_re = Some(r.clone());
                Ok(r.clone())
            }
            Pat::Last => self.last_re.clone().ok_or_else(|| {
                "the empty regex // has no earlier regular expression to reuse".to_string()
            }),
        }
    }

    fn addr_matches_one(&mut self, a: &Addr) -> Result<bool, String> {
        Ok(match a {
            Addr::Line(n) => self.line_no == *n,
            Addr::Last => self.is_last(),
            Addr::Pat(p) => {
                let re = self.resolve(p)?;
                re.is_match(&self.pattern)
            }
            Addr::Step { first, step } => {
                if *step == 0 {
                    self.line_no == *first
                } else {
                    self.line_no >= *first && (self.line_no - *first) % *step == 0
                }
            }
        })
    }

    fn addr_hit(&mut self, idx: usize, instr: &Instr) -> Result<AddrHit, String> {
        let hit = match &instr.addr {
            AddrSpec::None => AddrHit {
                matched: true,
                last_of_range: true,
            },
            AddrSpec::One(a) => AddrHit {
                matched: self.addr_matches_one(a)?,
                last_of_range: true,
            },
            AddrSpec::Range {
                start,
                end,
                zero_start,
            } => {
                let rt = self.ranges[idx];
                if !rt.active {
                    let starts = if *zero_start {
                        !rt.done
                    } else {
                        self.addr_matches_one(start)?
                    };
                    if !starts {
                        AddrHit {
                            matched: false,
                            last_of_range: false,
                        }
                    } else {
                        // Decide where the range stops.
                        let (active, end_line, ends_here) = match end {
                            EndAddr::Addr(Addr::Line(n)) => {
                                if *n <= self.line_no && !*zero_start {
                                    (false, None, true)
                                } else {
                                    (true, Some(*n), false)
                                }
                            }
                            EndAddr::Plus(n) => {
                                if *n == 0 {
                                    (false, None, true)
                                } else {
                                    (true, Some(self.line_no + n), false)
                                }
                            }
                            EndAddr::Multiple(n) => {
                                let m = self.line_no.div_ceil(*n) * *n;
                                if m <= self.line_no {
                                    (false, None, true)
                                } else {
                                    (true, Some(m), false)
                                }
                            }
                            EndAddr::Addr(a) => {
                                // A regex/`$` end is re-checked on later lines; when the
                                // range starts at 0 it may already match on this line.
                                if *zero_start && self.addr_matches_one(a)? {
                                    (false, None, true)
                                } else {
                                    (true, None, false)
                                }
                            }
                        };
                        self.ranges[idx] = RangeRt {
                            active,
                            end_line,
                            // A `0,/re/` range is one-shot: once it closes it never reopens.
                            done: *zero_start && ends_here,
                        };
                        AddrHit {
                            matched: true,
                            last_of_range: ends_here,
                        }
                    }
                } else {
                    let ends = match (rt.end_line, end) {
                        (Some(n), _) => self.line_no >= n,
                        (None, EndAddr::Addr(a)) => {
                            let a = a.clone();
                            self.addr_matches_one(&a)?
                        }
                        (None, _) => true,
                    };
                    if ends {
                        self.ranges[idx] = RangeRt {
                            active: false,
                            end_line: None,
                            done: *zero_start,
                        };
                    }
                    AddrHit {
                        matched: true,
                        last_of_range: ends,
                    }
                }
            }
        };
        Ok(AddrHit {
            matched: hit.matched != instr.negate,
            last_of_range: hit.last_of_range,
        })
    }

    fn substitute(&mut self, s: &Subst) -> Result<(), String> {
        let re = self.resolve(&s.pat)?;
        let hay = std::mem::take(&mut self.pattern);
        let mut out = String::with_capacity(hay.len());
        let mut last = 0usize;
        let mut seen = 0usize;
        let mut changed = false;
        for caps in re.captures_iter(&hay) {
            let m = caps.get(0).unwrap();
            seen += 1;
            if seen < s.occurrence {
                continue;
            }
            out.push_str(&hay[last..m.start()]);
            expand(&caps, &s.repl, &mut out)?;
            last = m.end();
            changed = true;
            if !s.global {
                break;
            }
        }
        if changed {
            out.push_str(&hay[last..]);
            self.pattern = out;
            self.sub_made = true;
            if s.print {
                let p = self.pattern.clone();
                self.emit(p);
            }
        } else {
            self.pattern = hay;
        }
        Ok(())
    }

    fn run_cycle(&mut self, prog: &Program) -> Result<Flow, String> {
        let n = prog.instrs.len();
        let mut pc = 0usize;
        loop {
            if pc >= n {
                return Ok(Flow::EndCycle { auto_print: true });
            }
            self.steps += 1;
            if self.steps > STEP_LIMIT {
                return Err(format!(
                    "script did not finish after {STEP_LIMIT} commands — check for a b/t branch loop that never reaches its end condition"
                ));
            }
            let instr = &prog.instrs[pc];
            let hit = self.addr_hit(pc, instr)?;
            if !hit.matched {
                pc = match instr.cmd {
                    Cmd::BlockStart { end } => end + 1,
                    _ => pc + 1,
                };
                continue;
            }
            match &instr.cmd {
                Cmd::BlockStart { .. } | Cmd::BlockEnd | Cmd::Label(_) | Cmd::Noop => pc += 1,
                Cmd::Branch(target) => {
                    pc = match target {
                        Some(l) => prog.labels[l],
                        None => n,
                    }
                }
                Cmd::BranchIfSub(target) => {
                    if self.sub_made {
                        self.sub_made = false;
                        pc = match target {
                            Some(l) => prog.labels[l],
                            None => n,
                        };
                    } else {
                        pc += 1;
                    }
                }
                Cmd::BranchIfNoSub(target) => {
                    if !self.sub_made {
                        pc = match target {
                            Some(l) => prog.labels[l],
                            None => n,
                        };
                    } else {
                        self.sub_made = false;
                        pc += 1;
                    }
                }
                Cmd::Subst(s) => {
                    self.substitute(s)?;
                    pc += 1;
                }
                Cmd::Trans { from, to } => {
                    self.pattern = self
                        .pattern
                        .chars()
                        .map(|c| match from.iter().position(|f| *f == c) {
                            Some(i) => to[i],
                            None => c,
                        })
                        .collect();
                    pc += 1;
                }
                Cmd::Delete => return Ok(Flow::EndCycle { auto_print: false }),
                Cmd::DeleteFirst => {
                    return match self.pattern.find('\n') {
                        Some(i) => {
                            self.pattern = self.pattern[i + 1..].to_string();
                            Ok(Flow::Restart)
                        }
                        None => Ok(Flow::EndCycle { auto_print: false }),
                    }
                }
                Cmd::Print => {
                    let p = self.pattern.clone();
                    self.emit(p);
                    pc += 1;
                }
                Cmd::PrintFirst => {
                    let head = match self.pattern.find('\n') {
                        Some(i) => self.pattern[..i].to_string(),
                        None => self.pattern.clone(),
                    };
                    self.emit(head);
                    pc += 1;
                }
                Cmd::Insert(text) => {
                    self.emit(text.clone());
                    pc += 1;
                }
                Cmd::AppendText(text) => {
                    self.appends.push(text.clone());
                    pc += 1;
                }
                Cmd::Change(text) => {
                    if hit.last_of_range {
                        self.emit(text.clone());
                    }
                    return Ok(Flow::EndCycle { auto_print: false });
                }
                Cmd::Next => {
                    if !self.quiet {
                        let p = self.pattern.clone();
                        self.emit(p);
                    }
                    self.flush_appends();
                    match self.read_line() {
                        Some(l) => {
                            self.pattern = l;
                            pc += 1;
                        }
                        None => return Ok(Flow::Quit { auto_print: false }),
                    }
                }
                Cmd::NextAppend => match self.read_line() {
                    Some(l) => {
                        self.pattern.push('\n');
                        self.pattern.push_str(&l);
                        pc += 1;
                    }
                    None => {
                        return Ok(Flow::Quit {
                            auto_print: !self.quiet,
                        })
                    }
                },
                Cmd::Hold => {
                    self.hold = self.pattern.clone();
                    pc += 1;
                }
                Cmd::HoldAppend => {
                    self.hold.push('\n');
                    let p = self.pattern.clone();
                    self.hold.push_str(&p);
                    pc += 1;
                }
                Cmd::Get => {
                    self.pattern = self.hold.clone();
                    pc += 1;
                }
                Cmd::GetAppend => {
                    self.pattern.push('\n');
                    let h = self.hold.clone();
                    self.pattern.push_str(&h);
                    pc += 1;
                }
                Cmd::Exchange => {
                    std::mem::swap(&mut self.pattern, &mut self.hold);
                    pc += 1;
                }
                Cmd::Quit { print } => {
                    return Ok(Flow::Quit {
                        auto_print: *print && !self.quiet,
                    })
                }
                Cmd::LineNum => {
                    let n = self.line_no;
                    self.emit(n.to_string());
                    pc += 1;
                }
                Cmd::List(wrap) => {
                    let s = escape_list(&self.pattern, *wrap);
                    self.emit(s);
                    pc += 1;
                }
                Cmd::Zap => {
                    self.pattern.clear();
                    pc += 1;
                }
            }
        }
    }

    fn flush_appends(&mut self) {
        for text in std::mem::take(&mut self.appends) {
            self.emit(text);
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Apply `script` (a sed/ed-style command script) to `text` in a single pass.
pub fn run(text: &str, script: &str, opts: &Options) -> Result<String, String> {
    if opts.max_output_lines == 0 {
        return Err("max_output_lines must be at least 1".to_string());
    }
    if script.trim().is_empty() {
        return Err(
            "script is empty — give at least one command, e.g. s/old/new/g or /^$/d".to_string(),
        );
    }
    let prog = Parser::new(script, opts).parse()?;
    let quiet = opts.quiet || prog.quiet;

    let normalized = text.replace("\r\n", "\n");
    let had_trailing_nl = normalized.ends_with('\n');
    let body = if had_trailing_nl {
        &normalized[..normalized.len() - 1]
    } else {
        &normalized[..]
    };
    let lines: Vec<String> = if normalized.is_empty() {
        Vec::new()
    } else if opts.whole_buffer {
        vec![body.to_string()]
    } else {
        body.split('\n').map(|s| s.to_string()).collect()
    };

    let mut m = Machine {
        lines: &lines,
        next_idx: 0,
        pattern: String::new(),
        hold: String::new(),
        line_no: 0,
        sub_made: false,
        last_re: None,
        ranges: vec![
            RangeRt {
                active: false,
                end_line: None,
                done: false
            };
            prog.instrs.len()
        ],
        out: Vec::new(),
        appends: Vec::new(),
        quiet,
        max_output_lines: opts.max_output_lines,
        truncated: false,
        steps: 0,
    };

    'input: while let Some(line) = m.read_line() {
        m.pattern = line;
        m.sub_made = false;
        m.appends.clear();
        loop {
            match m.run_cycle(&prog)? {
                Flow::Restart => continue,
                Flow::EndCycle { auto_print } => {
                    if auto_print && !quiet {
                        let p = m.pattern.clone();
                        m.emit(p);
                    }
                    m.flush_appends();
                    break;
                }
                Flow::Quit { auto_print } => {
                    if auto_print {
                        let p = m.pattern.clone();
                        m.emit(p);
                    }
                    m.flush_appends();
                    break 'input;
                }
            }
        }
        if m.truncated {
            break;
        }
    }

    let mut out = m.out.join("\n");
    if !m.out.is_empty() && had_trailing_nl && !m.truncated {
        out.push('\n');
    }
    if opts.line_ending == LineEnding::Crlf {
        out = out.replace('\n', "\r\n");
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    fn go(text: &str, script: &str) -> String {
        run(text, script, &opts()).expect("script should run")
    }

    #[test]
    fn substitutes_globally() {
        assert_eq!(go("foo foo bar", "s/foo/baz/g"), "baz baz bar");
    }

    #[test]
    fn substitutes_first_match_only_without_g() {
        assert_eq!(go("foo foo", "s/foo/baz/"), "baz foo");
    }

    #[test]
    fn substitutes_nth_occurrence_and_beyond() {
        assert_eq!(go("a a a a", "s/a/X/3"), "a a X a");
        assert_eq!(go("a a a a", "s/a/X/2g"), "a X X X");
    }

    #[test]
    fn deletes_blank_lines() {
        assert_eq!(
            go("one\n\ntwo\n   \nthree", DEFAULT_SCRIPT),
            "one\ntwo\nthree"
        );
    }

    #[test]
    fn default_script_also_renames_foo() {
        assert_eq!(go("foo and foo\n\nx", DEFAULT_SCRIPT), "bar and bar\nx");
    }

    #[test]
    fn deletes_by_line_number_and_range() {
        assert_eq!(go("1\n2\n3\n4\n5", "2,4d"), "1\n5");
        assert_eq!(go("1\n2\n3", "$d"), "1\n2");
    }

    #[test]
    fn quiet_mode_prints_only_matches() {
        let o = Options {
            quiet: true,
            ..opts()
        };
        assert_eq!(run("a\nbb\nccc", "/b/p", &o).unwrap(), "bb");
    }

    #[test]
    fn hash_n_first_line_enables_quiet() {
        assert_eq!(go("a\nbb\nccc", "#n\n/b/p"), "bb");
    }

    #[test]
    fn inserts_and_appends_text() {
        assert_eq!(go("a\nb", "1i header"), "header\na\nb");
        assert_eq!(go("a\nb", "$a footer"), "a\nb\nfooter");
        assert_eq!(go("a\nb", "2c CHANGED"), "a\nCHANGED");
    }

    #[test]
    fn classic_backslash_text_form_works() {
        assert_eq!(go("a", "1i\\\nhello"), "hello\na");
    }

    #[test]
    fn changes_whole_range_once() {
        assert_eq!(go("1\n2\n3\n4", "2,3c MERGED"), "1\nMERGED\n4");
    }

    #[test]
    fn regex_address_with_negation() {
        assert_eq!(go("keep\ndrop me\nkeep2", "/drop/!d"), "drop me");
    }

    #[test]
    fn range_between_two_regexes() {
        assert_eq!(go("a\nBEGIN\nx\nEND\nb", "/BEGIN/,/END/d"), "a\nb");
    }

    #[test]
    fn step_and_relative_addresses() {
        assert_eq!(go("1\n2\n3\n4\n5\n6", "0~2d"), "1\n3\n5");
        assert_eq!(go("1\n2\n3\n4\n5", "2,+2d"), "1\n5");
    }

    #[test]
    fn zero_comma_regex_stops_at_first_match() {
        assert_eq!(go("a\nb\na\nc", "0,/a/d"), "b\na\nc");
    }

    #[test]
    fn blocks_group_commands_under_one_address() {
        assert_eq!(go("x1\ny2\nx3", "/^x/{s/x/X/;s/$/!/}"), "X1!\ny2\nX3!");
    }

    #[test]
    fn transliterates_characters() {
        assert_eq!(go("abc", "y/abc/xyz/"), "xyz");
    }

    #[test]
    fn transliterate_length_mismatch_errors() {
        let e = run("abc", "y/ab/xyz/", &opts()).unwrap_err();
        assert!(e.contains("same length"), "{e}");
    }

    #[test]
    fn joins_all_lines_with_the_classic_branch_idiom() {
        assert_eq!(go("a\nb\nc", ":a\nN\n$!ba\ns/\\n/, /g"), "a, b, c");
    }

    #[test]
    fn hold_space_reverses_lines_like_tac() {
        assert_eq!(go("1\n2\n3", "1!G\nh\n$!d"), "3\n2\n1");
    }

    #[test]
    fn basic_regex_groups_use_backslash_parens() {
        assert_eq!(
            go("john smith", "s/\\(\\w*\\) \\(\\w*\\)/\\2, \\1/"),
            "smith, john"
        );
    }

    #[test]
    fn extended_regex_flavor_uses_bare_parens() {
        let o = Options {
            flavor: RegexFlavor::Extended,
            ..opts()
        };
        assert_eq!(
            run("john smith", "s/(\\w+) (\\w+)/\\2, \\1/", &o).unwrap(),
            "smith, john"
        );
        // Alternation is bare in ERE.
        assert_eq!(run("cat dog", "s/cat|dog/pet/g", &o).unwrap(), "pet pet");
    }

    #[test]
    fn basic_flavor_treats_bare_parens_as_literals() {
        assert_eq!(go("(hi)", "s/(hi)/ok/"), "ok");
    }

    #[test]
    fn case_conversion_escapes_in_replacement() {
        assert_eq!(go("hello world", "s/\\w\\+/\\u&/g"), "Hello World");
        assert_eq!(go("shout", "s/.*/\\U&/"), "SHOUT");
        assert_eq!(go("MiXeD", "s/.*/\\L&/"), "mixed");
    }

    #[test]
    fn ampersand_and_escaped_ampersand() {
        assert_eq!(go("42", "s/[0-9]*/[&]/"), "[42]");
        assert_eq!(go("42", "s/42/a\\&b/"), "a&b");
    }

    #[test]
    fn alternate_delimiters_work() {
        assert_eq!(go("/usr/local", "s|/usr|/opt|"), "/opt/local");
        assert_eq!(go("a.b", "s#\\.#-#"), "a-b");
    }

    #[test]
    fn empty_regex_reuses_the_previous_one() {
        assert_eq!(go("foofoo", "/foo/s//bar/"), "barfoo");
    }

    #[test]
    fn ignore_case_option_folds_every_pattern() {
        let o = Options {
            ignore_case: true,
            ..opts()
        };
        assert_eq!(run("Foo FOO", "s/foo/x/g", &o).unwrap(), "x x");
    }

    #[test]
    fn per_command_case_insensitive_flag() {
        assert_eq!(go("Foo\nbar", "/FOO/Id"), "bar");
        assert_eq!(go("Foo", "s/foo/x/I"), "x");
    }

    #[test]
    fn line_numbers_and_quit() {
        let o = Options {
            quiet: true,
            ..opts()
        };
        assert_eq!(run("a\nb\nc", "/b/=", &o).unwrap(), "2");
        assert_eq!(go("a\nb\nc", "2q"), "a\nb");
        assert_eq!(go("a\nb\nc", "2Q"), "a");
    }

    #[test]
    fn list_command_escapes_control_characters() {
        let o = Options {
            quiet: true,
            ..opts()
        };
        assert_eq!(run("a\tb", "l", &o).unwrap(), "a\\tb$");
    }

    #[test]
    fn whole_buffer_mode_matches_across_lines() {
        let o = Options {
            whole_buffer: true,
            ..opts()
        };
        assert_eq!(run("a\nb\nc", "s/\\n/,/g", &o).unwrap(), "a,b,c");
    }

    #[test]
    fn crlf_input_is_normalized_and_output_can_be_crlf() {
        assert_eq!(go("a\r\nb", "s/a/A/"), "A\nb");
        let o = Options {
            line_ending: LineEnding::Crlf,
            ..opts()
        };
        assert_eq!(run("a\nb", "s/a/A/", &o).unwrap(), "A\r\nb");
    }

    #[test]
    fn trailing_newline_is_preserved() {
        assert_eq!(go("a\nb\n", "s/a/A/"), "A\nb\n");
        assert_eq!(go("a\nb", "s/a/A/"), "A\nb");
    }

    #[test]
    fn comments_and_semicolons_are_ignored() {
        assert_eq!(go("a\nb", "# drop b\n/b/d ; s/a/A/"), "A");
    }

    #[test]
    fn max_output_lines_caps_the_result() {
        let o = Options {
            max_output_lines: 2,
            ..opts()
        };
        assert_eq!(run("1\n2\n3\n4", "p", &o).unwrap(), "1\n1");
    }

    #[test]
    fn unknown_command_reports_what_is_supported() {
        let e = run("a", "Z", &opts()).unwrap_err();
        assert!(e.contains("unknown command"), "{e}");
        assert!(e.contains("supported commands"), "{e}");
    }

    #[test]
    fn file_and_shell_commands_are_refused_clearly() {
        assert!(run("a", "w out.txt", &opts())
            .unwrap_err()
            .contains("not available"));
        assert!(run("a", "e ls", &opts())
            .unwrap_err()
            .contains("not available"));
        assert!(run("a", "s/a/b/w f.txt", &opts())
            .unwrap_err()
            .contains("not available"));
    }

    #[test]
    fn unterminated_s_command_errors() {
        let e = run("a", "s/a/b", &opts()).unwrap_err();
        assert!(e.contains("unterminated"), "{e}");
    }

    #[test]
    fn unbalanced_block_errors() {
        let e = run("a", "/a/{p", &opts()).unwrap_err();
        assert!(e.contains("unbalanced"), "{e}");
        let e2 = run("a", "p}", &opts()).unwrap_err();
        assert!(e2.contains("no matching"), "{e2}");
    }

    #[test]
    fn missing_label_errors() {
        let e = run("a", "b nowhere", &opts()).unwrap_err();
        assert!(e.contains("never defined"), "{e}");
    }

    #[test]
    fn invalid_regex_errors_with_the_pattern() {
        let e = run("a", "s/[unclosed/x/", &opts()).unwrap_err();
        assert!(
            e.contains("bracket") || e.contains("invalid regular expression"),
            "{e}"
        );
    }

    #[test]
    fn backreference_in_pattern_is_reported() {
        let e = run("aa", "s/\\(a\\)\\1/x/", &opts()).unwrap_err();
        assert!(e.contains("backreference"), "{e}");
    }

    #[test]
    fn empty_script_is_rejected() {
        let e = run("a", "   ", &opts()).unwrap_err();
        assert!(e.contains("script is empty"), "{e}");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(go("", "s/a/b/"), "");
    }

    #[test]
    fn runaway_branch_loop_is_stopped() {
        let e = run("a", ":x\nbx", &opts()).unwrap_err();
        assert!(e.contains("did not finish"), "{e}");
    }

    #[test]
    fn t_branch_runs_only_after_a_substitution() {
        // Squeeze runs of spaces without a global flag, using t to loop.
        assert_eq!(go("a    b", ":r\ns/  / /\ntr"), "a b");
    }

    #[test]
    fn print_first_line_of_multiline_pattern() {
        let o = Options {
            quiet: true,
            ..opts()
        };
        assert_eq!(run("a\nb", "N\nP", &o).unwrap(), "a");
    }

    #[test]
    fn delete_first_line_of_pattern_space() {
        // Classic "squeeze repeated blank lines" idiom.
        assert_eq!(go("a\n\n\n\nb", "/^$/{N;/^\\n$/D}"), "a\n\nb");
    }

    #[test]
    fn multiline_flag_anchors_each_line() {
        let o = Options {
            whole_buffer: true,
            ..opts()
        };
        assert_eq!(run("a\nb", "s/^/> /Mg", &o).unwrap(), "> a\n> b");
    }
}
