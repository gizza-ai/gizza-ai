//! regex-match-generator core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps, and no third-party crates: the supported regex subset is parsed
//! into a small AST here and walked by a deterministic generator.
//!
//! Supported subset: literals, escaped metacharacters, `.`, character classes and ranges
//! (including negation), the shorthands `\d \D \w \W \s \S`, groups (capturing, `(?:…)` and
//! named), alternation, the quantifiers `? * + {n} {n,} {n,m}` (lazy suffixes accepted and
//! treated as their greedy equivalent, because generation is about the *language*, not match
//! preference), and anchors, which are ignored because every generated string is a whole match.
//!
//! Deliberately rejected with a specific message: lookaround, backreferences, atomic/possessive
//! quantifiers, inline flags, POSIX classes, `\p{…}` properties and `\b`/`\B`.

use std::collections::BTreeSet;

/// Longest accepted pattern, in characters.
pub const MAX_PATTERN_LEN: usize = 2000;
/// Largest accepted `count`.
pub const MAX_COUNT: usize = 200;
/// Largest accepted `max_repeat`.
pub const MAX_REPEAT_LIMIT: u32 = 50;
/// Largest accepted `max_length`, in characters per sample.
pub const MAX_LENGTH_LIMIT: usize = 2000;
/// Deepest group nesting accepted.
pub const MAX_DEPTH: usize = 40;
/// Largest explicit `{n,m}` bound accepted.
pub const MAX_EXPLICIT_REPEAT: u32 = 10_000;
/// Largest number of characters one character class may expand to.
pub const MAX_CLASS_SIZE: usize = 4096;

/// Generation styles, in descriptor order.
pub const STYLES: [&str; 4] = ["random", "sequential", "shortest", "longest"];
/// Output formats, in descriptor order.
pub const OUTPUTS: [&str; 3] = ["lines", "json", "csv"];

/// First character of the generatable alphabet (space).
const ALPHABET_LO: u32 = 0x20;
/// Last character of the generatable alphabet (`~`).
const ALPHABET_HI: u32 = 0x7E;

const TOO_LONG: &str =
    "the pattern cannot be generated within max_length — raise max_length or lower max_repeat";

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Node {
    /// Matches the empty string — an ignored anchor, or an empty alternation branch.
    Empty,
    Literal(char),
    /// A materialised set of candidate characters, in the order they were written.
    Class(Vec<char>),
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: u32,
        /// `u32::MAX` means unbounded (`*`, `+`, `{n,}`).
        max: u32,
    },
}

/// Shortest string this node can produce, in characters.
fn min_len(node: &Node) -> usize {
    match node {
        Node::Empty => 0,
        Node::Literal(_) | Node::Class(_) => 1,
        Node::Seq(items) => items.iter().map(min_len).sum(),
        Node::Alt(branches) => branches.iter().map(min_len).min().unwrap_or(0),
        Node::Repeat { node, min, .. } => (*min as usize).saturating_mul(min_len(node)),
    }
}

// ---------------------------------------------------------------------------
// Alphabet helpers
// ---------------------------------------------------------------------------

fn alphabet() -> Vec<char> {
    (ALPHABET_LO..=ALPHABET_HI).filter_map(char::from_u32).collect()
}

fn digits() -> Vec<char> {
    ('0'..='9').collect()
}

fn word_chars() -> Vec<char> {
    let mut v: Vec<char> = ('a'..='z').collect();
    v.extend('A'..='Z');
    v.extend('0'..='9');
    v.push('_');
    v
}

/// The whitespace characters this generator will emit for `\s`. `\n`/`\r` are left out so a
/// generated sample stays on one line; both still match `\s` in every flavour.
fn space_chars() -> Vec<char> {
    vec![' ', '\t']
}

/// Everything in the generatable alphabet that is not in `excluded`.
fn complement(excluded: &[char]) -> Vec<char> {
    let set: BTreeSet<char> = excluded.iter().copied().collect();
    alphabet().into_iter().filter(|c| !set.contains(c)).collect()
}

fn dedupe(chars: Vec<char>) -> Vec<char> {
    let mut seen = BTreeSet::new();
    chars.into_iter().filter(|c| seen.insert(*c)).collect()
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

enum Escape {
    Char(char),
    Chars(Vec<char>),
    /// A zero-width anchor (`\A`, `\z`, `\Z`) — ignored during generation.
    Anchor,
}

enum Member {
    One(char),
    Many(Vec<char>),
}

fn unsupported(what: &str) -> String {
    format!("{what} is not supported by this generator — see the supported subset on the page")
}

struct Parser<'a> {
    src: &'a [char],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a [char]) -> Self {
        Parser { src, pos: 0, depth: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.src.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn parse(&mut self) -> Result<Node, String> {
        let node = self.parse_alternation()?;
        match self.peek() {
            None => Ok(node),
            Some(')') => Err(format!("unmatched ')' at position {}", self.pos + 1)),
            Some(c) => Err(format!("unexpected '{c}' at position {}", self.pos + 1)),
        }
    }

    fn parse_alternation(&mut self) -> Result<Node, String> {
        let mut branches = vec![self.parse_sequence()?];
        while self.peek() == Some('|') {
            self.pos += 1;
            branches.push(self.parse_sequence()?);
        }
        Ok(if branches.len() == 1 {
            branches.pop().unwrap()
        } else {
            Node::Alt(branches)
        })
    }

    fn parse_sequence(&mut self) -> Result<Node, String> {
        let mut items: Vec<Node> = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let start = self.pos;
            let atom = self.parse_atom()?;
            let node = match self.parse_quantifier()? {
                Some((min, max)) => {
                    if matches!(atom, Node::Empty) {
                        return Err(format!(
                            "nothing to repeat at position {}: an anchor cannot be quantified",
                            start + 1
                        ));
                    }
                    Node::Repeat { node: Box::new(atom), min, max }
                }
                None => atom,
            };
            items.push(node);
        }
        Ok(match items.len() {
            0 => Node::Empty,
            1 => items.pop().unwrap(),
            _ => Node::Seq(items),
        })
    }

    fn parse_atom(&mut self) -> Result<Node, String> {
        let start = self.pos;
        let c = self
            .bump()
            .ok_or_else(|| "unexpected end of pattern".to_string())?;
        match c {
            '(' => self.parse_group(start),
            '[' => self.parse_class(start),
            '.' => Ok(Node::Class(alphabet())),
            '^' | '$' => Ok(Node::Empty),
            '*' | '+' | '?' => Err(format!(
                "nothing to repeat: '{c}' at position {} has no preceding atom",
                start + 1
            )),
            ')' => Err(format!("unmatched ')' at position {}", start + 1)),
            '{' => {
                // `{2,3}` here would be a quantifier with no atom in front of it; anything else
                // is a literal brace, which is what most flavours do.
                self.pos = start;
                if self.try_braces()?.is_some() {
                    return Err(format!(
                        "nothing to repeat: the quantifier at position {} has no preceding atom",
                        start + 1
                    ));
                }
                self.pos = start + 1;
                Ok(Node::Literal('{'))
            }
            '\\' => Ok(match self.parse_escape(false)? {
                Escape::Char(ch) => Node::Literal(ch),
                Escape::Chars(v) => Node::Class(v),
                Escape::Anchor => Node::Empty,
            }),
            other => Ok(Node::Literal(other)),
        }
    }

    /// `self.pos` sits just past the opening `(`.
    fn parse_group(&mut self, start: usize) -> Result<Node, String> {
        if self.depth >= MAX_DEPTH {
            return Err(format!("pattern nests groups more than {MAX_DEPTH} deep"));
        }
        if self.peek() == Some('?') {
            match self.peek_at(1) {
                Some(':') => self.pos += 2,
                Some('=') => return Err(unsupported("lookahead `(?=…)`")),
                Some('!') => return Err(unsupported("negative lookahead `(?!…)`")),
                Some('>') => return Err(unsupported("atomic groups `(?>…)`")),
                Some('#') => return Err(unsupported("inline comments `(?#…)`")),
                Some('<') => match self.peek_at(2) {
                    Some('=') => return Err(unsupported("lookbehind `(?<=…)`")),
                    Some('!') => return Err(unsupported("negative lookbehind `(?<!…)`")),
                    _ => self.skip_group_name('>')?,
                },
                Some('\'') => self.skip_group_name('\'')?,
                Some('P') => match self.peek_at(2) {
                    // `(?P<name>…)` is a named group; `(?P=name)` is a backreference.
                    Some('<') => {
                        self.pos += 1;
                        self.skip_group_name('>')?;
                    }
                    _ => return Err(unsupported("`(?P=name)` backreferences")),
                },
                None => return Err(format!("unclosed group opened at position {}", start + 1)),
                _ => return Err(unsupported("inline flag groups such as `(?i)`")),
            }
        }
        self.depth += 1;
        let inner = self.parse_alternation()?;
        self.depth -= 1;
        if self.bump() != Some(')') {
            return Err(format!("unclosed group opened at position {}", start + 1));
        }
        Ok(inner)
    }

    /// Skips `?<name>` / `?'name'`; `self.pos` sits at the `?`.
    fn skip_group_name(&mut self, close: char) -> Result<(), String> {
        self.pos += 2;
        while let Some(c) = self.bump() {
            if c == close {
                return Ok(());
            }
        }
        Err("unclosed group name — expected a closing `>`".to_string())
    }

    /// `self.pos` sits just past the opening `[`.
    fn parse_class(&mut self, start: usize) -> Result<Node, String> {
        let negated = self.peek() == Some('^');
        if negated {
            self.pos += 1;
        }
        let mut items: Vec<char> = Vec::new();
        let mut first = true;
        let mut closed = false;
        while let Some(c) = self.bump() {
            if c == ']' && !first {
                closed = true;
                break;
            }
            first = false;
            if c == '[' && self.peek() == Some(':') {
                return Err(unsupported("POSIX classes such as `[[:alpha:]]`"));
            }
            let member = if c == '\\' {
                match self.parse_escape(true)? {
                    Escape::Char(ch) => Member::One(ch),
                    Escape::Chars(v) => Member::Many(v),
                    Escape::Anchor => Member::One(c),
                }
            } else {
                Member::One(c)
            };
            let range_follows =
                self.peek() == Some('-') && self.peek_at(1).is_some_and(|n| n != ']');
            match member {
                Member::One(lo) if range_follows => {
                    self.pos += 1;
                    let raw = self
                        .bump()
                        .ok_or_else(|| "unclosed character range".to_string())?;
                    let hi = if raw == '\\' {
                        match self.parse_escape(true)? {
                            Escape::Char(ch) => ch,
                            _ => {
                                return Err(
                                    "a character range cannot end with a shorthand class such as `\\d`"
                                        .to_string(),
                                )
                            }
                        }
                    } else {
                        raw
                    };
                    if (hi as u32) < (lo as u32) {
                        return Err(format!(
                            "invalid character range `{lo}-{hi}`: the end must not come before the start"
                        ));
                    }
                    let span = (hi as u32 - lo as u32 + 1) as usize;
                    if span > MAX_CLASS_SIZE {
                        return Err(format!(
                            "the range `{lo}-{hi}` expands to {span} characters; ranges are capped at {MAX_CLASS_SIZE}"
                        ));
                    }
                    items.extend((lo as u32..=hi as u32).filter_map(char::from_u32));
                }
                Member::One(ch) => items.push(ch),
                Member::Many(v) => {
                    if range_follows {
                        return Err(
                            "a character range cannot start with a shorthand class such as `\\d`"
                                .to_string(),
                        );
                    }
                    items.extend(v);
                }
            }
            if items.len() > MAX_CLASS_SIZE {
                return Err(format!(
                    "character class expands to more than {MAX_CLASS_SIZE} characters — narrow the ranges"
                ));
            }
        }
        if !closed {
            return Err(format!(
                "unclosed character class opened at position {}",
                start + 1
            ));
        }
        let chosen = if negated {
            let excluded: BTreeSet<char> = items.into_iter().collect();
            let v: Vec<char> = alphabet().into_iter().filter(|c| !excluded.contains(c)).collect();
            if v.is_empty() {
                return Err(
                    "the negated character class excludes every character this generator can produce (printable ASCII)"
                        .to_string(),
                );
            }
            v
        } else {
            let v = dedupe(items);
            if v.is_empty() {
                return Err(format!(
                    "empty character class at position {} — `[]` matches nothing",
                    start + 1
                ));
            }
            v
        };
        Ok(Node::Class(chosen))
    }

    fn parse_escape(&mut self, in_class: bool) -> Result<Escape, String> {
        let start = self.pos;
        let c = self
            .bump()
            .ok_or_else(|| "pattern ends with a trailing backslash".to_string())?;
        Ok(match c {
            'd' => Escape::Chars(digits()),
            'D' => Escape::Chars(complement(&digits())),
            'w' => Escape::Chars(word_chars()),
            'W' => Escape::Chars(complement(&word_chars())),
            's' => Escape::Chars(space_chars()),
            'S' => Escape::Chars(complement(&[' '])),
            'n' => Escape::Char('\n'),
            't' => Escape::Char('\t'),
            'r' => Escape::Char('\r'),
            'f' => Escape::Char('\u{0c}'),
            'v' => Escape::Char('\u{0b}'),
            'a' => Escape::Char('\u{07}'),
            'e' => Escape::Char('\u{1b}'),
            '0' => Escape::Char('\0'),
            'x' => Escape::Char(self.parse_hex(2)?),
            'u' => Escape::Char(self.parse_hex(4)?),
            'b' if in_class => Escape::Char('\u{08}'),
            'b' | 'B' => {
                return Err(format!(
                    "`\\{c}` (word boundary) is not supported: this tool emits whole strings, so a \
                     boundary assertion has nothing to sit between — delete it, or spell the \
                     boundary out with an explicit character class"
                ))
            }
            'A' | 'z' | 'Z' if !in_class => Escape::Anchor,
            '1'..='9' => {
                return Err(format!(
                    "`\\{c}` (backreference) is not supported: repeating an earlier group's text \
                     needs a matching engine, not a generator"
                ))
            }
            'k' => return Err(unsupported("`\\k<name>` named backreferences")),
            'p' | 'P' => {
                return Err(unsupported(
                    "`\\p{…}` Unicode property classes (use an explicit range such as `[a-z]`)",
                ))
            }
            'Q' | 'E' => {
                return Err(unsupported(
                    "`\\Q…\\E` literal quoting (escape the characters individually)",
                ))
            }
            other if other.is_ascii_alphanumeric() => {
                return Err(format!(
                    "unsupported escape sequence `\\{other}` at position {}",
                    start
                ))
            }
            other => Escape::Char(other),
        })
    }

    /// `\xHH`, `\x{H…}`, `\uHHHH`.
    fn parse_hex(&mut self, width: usize) -> Result<char, String> {
        let mut hex = String::new();
        if self.peek() == Some('{') {
            self.pos += 1;
            while let Some(c) = self.bump() {
                if c == '}' {
                    break;
                }
                hex.push(c);
            }
            if hex.is_empty() {
                return Err("empty `\\x{…}` escape".to_string());
            }
        } else {
            for _ in 0..width {
                match self.bump() {
                    Some(c) if c.is_ascii_hexdigit() => hex.push(c),
                    _ => {
                        return Err(format!(
                            "expected {width} hex digits after the escape, e.g. `\\x41`"
                        ))
                    }
                }
            }
        }
        let value = u32::from_str_radix(&hex, 16)
            .map_err(|_| format!("`{hex}` is not a hexadecimal number"))?;
        char::from_u32(value).ok_or_else(|| format!("`{hex}` is not a valid character code"))
    }

    fn parse_quantifier(&mut self) -> Result<Option<(u32, u32)>, String> {
        let bounds = match self.peek() {
            Some('*') => {
                self.pos += 1;
                Some((0, u32::MAX))
            }
            Some('+') => {
                self.pos += 1;
                Some((1, u32::MAX))
            }
            Some('?') => {
                self.pos += 1;
                Some((0, 1))
            }
            Some('{') => self.try_braces()?,
            _ => None,
        };
        if bounds.is_some() {
            match self.peek() {
                // Lazy: the language is identical, only the match preference differs.
                Some('?') => self.pos += 1,
                Some('+') => {
                    return Err(unsupported("possessive quantifiers such as `a*+`"));
                }
                Some('*') => {
                    return Err(format!(
                        "nothing to repeat: '*' at position {} follows another quantifier",
                        self.pos + 1
                    ));
                }
                _ => {}
            }
        }
        Ok(bounds)
    }

    /// Parses `{n}` / `{n,}` / `{n,m}`. A `{` that does not start a valid quantifier is left
    /// unconsumed so the caller can treat it as a literal brace.
    fn try_braces(&mut self) -> Result<Option<(u32, u32)>, String> {
        let start = self.pos;
        self.pos += 1; // '{'
        let mut lo = String::new();
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            lo.push(self.bump().unwrap());
        }
        if lo.is_empty() {
            self.pos = start;
            return Ok(None);
        }
        let min: u32 = match lo.parse() {
            Ok(v) => v,
            Err(_) => {
                return Err(format!(
                    "repetition count `{lo}` is too large; the maximum is {MAX_EXPLICIT_REPEAT}"
                ))
            }
        };
        let max = match self.peek() {
            Some('}') => {
                self.pos += 1;
                min
            }
            Some(',') => {
                self.pos += 1;
                let mut hi = String::new();
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    hi.push(self.bump().unwrap());
                }
                if self.peek() != Some('}') {
                    self.pos = start;
                    return Ok(None);
                }
                self.pos += 1;
                if hi.is_empty() {
                    u32::MAX
                } else {
                    match hi.parse() {
                        Ok(v) => v,
                        Err(_) => {
                            return Err(format!(
                                "repetition count `{hi}` is too large; the maximum is {MAX_EXPLICIT_REPEAT}"
                            ))
                        }
                    }
                }
            }
            _ => {
                self.pos = start;
                return Ok(None);
            }
        };
        if min > MAX_EXPLICIT_REPEAT || (max != u32::MAX && max > MAX_EXPLICIT_REPEAT) {
            return Err(format!(
                "repetition counts are capped at {MAX_EXPLICIT_REPEAT}"
            ));
        }
        if max < min {
            return Err(format!(
                "invalid quantifier `{{{min},{max}}}`: the upper bound is below the lower bound"
            ));
        }
        Ok(Some((min, max)))
    }
}

// ---------------------------------------------------------------------------
// Choosers — one per generation style
// ---------------------------------------------------------------------------

trait Chooser {
    /// Picks an index in `0..n` (`n` is always ≥ 1).
    fn pick(&mut self, n: usize) -> usize;
    /// Picks a repetition count in `min..=max`.
    fn reps(&mut self, min: u32, max: u32) -> u32;
}

/// Seeded xorshift64* — no dependency, identical on every surface.
struct RandomChooser {
    state: u64,
}

impl RandomChooser {
    fn new(seed: u64, index: u64) -> Self {
        let state = seed
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(index.wrapping_mul(0x632b_e59b_d9b4_e019))
            | 1;
        RandomChooser { state }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

impl Chooser for RandomChooser {
    fn pick(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    fn reps(&mut self, min: u32, max: u32) -> u32 {
        min + (self.next_u64() % (max - min + 1) as u64) as u32
    }
}

/// Odometer over the sample index: each decision point becomes the next digit of a mixed-radix
/// counter, so consecutive samples walk the pattern's choices systematically.
struct SequentialChooser {
    index: u64,
    stride: u64,
}

impl SequentialChooser {
    fn new(index: u64) -> Self {
        SequentialChooser { index, stride: 1 }
    }
    fn digit(&mut self, n: u64) -> u64 {
        let d = (self.index / self.stride) % n;
        self.stride = self.stride.saturating_mul(n).max(1);
        d
    }
}

impl Chooser for SequentialChooser {
    fn pick(&mut self, n: usize) -> usize {
        self.digit(n as u64) as usize
    }
    fn reps(&mut self, min: u32, max: u32) -> u32 {
        min + self.digit((max - min + 1) as u64) as u32
    }
}

/// Always the first alternative and the fewest repetitions — the shortest match.
struct MinChooser;
impl Chooser for MinChooser {
    fn pick(&mut self, _n: usize) -> usize {
        0
    }
    fn reps(&mut self, min: u32, _max: u32) -> u32 {
        min
    }
}

/// Always the last alternative and the most repetitions allowed — the longest match.
struct MaxChooser;
impl Chooser for MaxChooser {
    fn pick(&mut self, n: usize) -> usize {
        n - 1
    }
    fn reps(&mut self, _min: u32, max: u32) -> u32 {
        max
    }
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

fn generate(
    node: &Node,
    chooser: &mut dyn Chooser,
    budget: usize,
    max_repeat: u32,
) -> Result<String, String> {
    match node {
        Node::Empty => Ok(String::new()),
        Node::Literal(c) => {
            if budget < 1 {
                return Err(TOO_LONG.to_string());
            }
            Ok(c.to_string())
        }
        Node::Class(candidates) => {
            if budget < 1 {
                return Err(TOO_LONG.to_string());
            }
            Ok(candidates[chooser.pick(candidates.len())].to_string())
        }
        Node::Seq(items) => {
            let mins: Vec<usize> = items.iter().map(min_len).collect();
            let mut suffix: usize = mins.iter().sum();
            let mut remaining = budget;
            let mut out = String::new();
            for (i, item) in items.iter().enumerate() {
                suffix -= mins[i];
                if remaining < suffix + mins[i] {
                    return Err(TOO_LONG.to_string());
                }
                let piece = generate(item, chooser, remaining - suffix, max_repeat)?;
                remaining -= piece.chars().count();
                out.push_str(&piece);
            }
            Ok(out)
        }
        Node::Alt(branches) => {
            let feasible: Vec<&Node> = branches
                .iter()
                .filter(|branch| min_len(branch) <= budget)
                .collect();
            if feasible.is_empty() {
                return Err(TOO_LONG.to_string());
            }
            let choice = chooser.pick(feasible.len());
            generate(feasible[choice], chooser, budget, max_repeat)
        }
        Node::Repeat { node, min, max } => {
            let child_min = min_len(node);
            // max_repeat caps unbounded and oversized quantifiers, but never below the minimum
            // the pattern demands.
            let hard_max = if *max == u32::MAX {
                (*min).max(max_repeat)
            } else {
                (*max).min(max_repeat).max(*min)
            };
            let capped = if child_min == 0 {
                hard_max
            } else {
                hard_max.min((budget / child_min) as u32)
            };
            if capped < *min {
                return Err(TOO_LONG.to_string());
            }
            let reps = chooser.reps(*min, capped);
            let mut out = String::new();
            let mut remaining = budget;
            for _ in 0..reps {
                let piece = generate(node, chooser, remaining, max_repeat)?;
                let len = piece.chars().count();
                if len > remaining {
                    return Err(TOO_LONG.to_string());
                }
                remaining -= len;
                out.push_str(&piece);
            }
            Ok(out)
        }
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn csv_field(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Generates sample strings that match `pattern`.
///
/// * `count` — how many samples to return (1..=[`MAX_COUNT`]).
/// * `style` — one of [`STYLES`].
/// * `seed` — PRNG seed; the same seed always produces the same samples.
/// * `max_repeat` — cap applied to `*`, `+` and `{n,}` (1..=[`MAX_REPEAT_LIMIT`]).
/// * `max_length` — per-sample character cap (1..=[`MAX_LENGTH_LIMIT`]).
/// * `unique` — drop duplicate samples.
/// * `output` — one of [`OUTPUTS`].
#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern: &str,
    count: usize,
    style: &str,
    seed: u64,
    max_repeat: u32,
    max_length: usize,
    unique: bool,
    output: &str,
) -> Result<String, String> {
    if !STYLES.contains(&style) {
        return Err(format!(
            "unknown style `{style}` — expected one of {}",
            STYLES.join(", ")
        ));
    }
    if !OUTPUTS.contains(&output) {
        return Err(format!(
            "unknown output `{output}` — expected one of {}",
            OUTPUTS.join(", ")
        ));
    }
    if pattern.is_empty() {
        return Err(
            "pattern is empty — enter a regular expression such as `[A-Z]{3}-\\d{4}`".to_string(),
        );
    }
    let chars: Vec<char> = pattern.chars().collect();
    if chars.len() > MAX_PATTERN_LEN {
        return Err(format!(
            "pattern is {} characters; the maximum is {MAX_PATTERN_LEN}",
            chars.len()
        ));
    }
    if count == 0 || count > MAX_COUNT {
        return Err(format!("count must be between 1 and {MAX_COUNT}"));
    }
    if max_repeat == 0 || max_repeat > MAX_REPEAT_LIMIT {
        return Err(format!(
            "max_repeat must be between 1 and {MAX_REPEAT_LIMIT}"
        ));
    }
    if max_length == 0 || max_length > MAX_LENGTH_LIMIT {
        return Err(format!(
            "max_length must be between 1 and {MAX_LENGTH_LIMIT}"
        ));
    }

    let ast = Parser::new(&chars).parse()?;
    let shortest = min_len(&ast);
    if shortest > max_length {
        return Err(format!(
            "the shortest string matching this pattern is {shortest} characters, which is over \
             max_length = {max_length}"
        ));
    }

    let deterministic = style == "shortest" || style == "longest";
    let attempts_cap = if unique {
        count.saturating_mul(20).saturating_add(50)
    } else {
        count
    };
    let mut samples: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut index: u64 = 0;
    let mut attempts = 0usize;
    while samples.len() < count && attempts < attempts_cap {
        attempts += 1;
        let mut chooser: Box<dyn Chooser> = match style {
            "random" => Box::new(RandomChooser::new(seed, index)),
            "sequential" => Box::new(SequentialChooser::new(index)),
            "shortest" => Box::new(MinChooser),
            _ => Box::new(MaxChooser),
        };
        index += 1;
        let sample = generate(&ast, chooser.as_mut(), max_length, max_repeat)?;
        if unique && !seen.insert(sample.clone()) {
            if deterministic {
                // Every sample of these styles is identical, so there is nothing left to find.
                break;
            }
            continue;
        }
        samples.push(sample);
    }

    Ok(match output {
        "json" => {
            let mut out = String::from("{\n");
            out.push_str(&format!("  \"pattern\": \"{}\",\n", json_escape(pattern)));
            out.push_str(&format!("  \"style\": \"{style}\",\n"));
            out.push_str(&format!("  \"seed\": {seed},\n"));
            out.push_str(&format!("  \"max_repeat\": {max_repeat},\n"));
            out.push_str(&format!("  \"max_length\": {max_length},\n"));
            out.push_str(&format!("  \"unique\": {unique},\n"));
            out.push_str(&format!("  \"requested\": {count},\n"));
            out.push_str(&format!("  \"returned\": {},\n", samples.len()));
            out.push_str("  \"samples\": [\n");
            for (i, sample) in samples.iter().enumerate() {
                let comma = if i + 1 == samples.len() { "" } else { "," };
                out.push_str(&format!("    \"{}\"{comma}\n", json_escape(sample)));
            }
            out.push_str("  ]\n}\n");
            out
        }
        "csv" => {
            let mut out = String::from("index,sample\n");
            for (i, sample) in samples.iter().enumerate() {
                out.push_str(&format!("{},{}\n", i + 1, csv_field(sample)));
            }
            out
        }
        _ => {
            let mut out = String::new();
            for sample in &samples {
                out.push_str(sample);
                out.push('\n');
            }
            out
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(pattern: &str, count: usize, style: &str) -> String {
        run(pattern, count, style, 42, 4, 200, true, "lines").unwrap()
    }

    #[test]
    fn shortest_style_takes_the_minimum_of_every_choice() {
        assert_eq!(gen("[A-Z]{3}-\\d{4}", 5, "shortest"), "AAA-0000\n");
        assert_eq!(gen("colou?rs?", 5, "shortest"), "color\n");
        assert_eq!(gen("(cat|dog|bird)", 5, "shortest"), "cat\n");
        assert_eq!(gen("ab*c", 5, "shortest"), "ac\n");
    }

    #[test]
    fn longest_style_takes_the_maximum_allowed_by_max_repeat() {
        assert_eq!(gen("[A-Z]{3}-\\d{4}", 5, "longest"), "ZZZ-9999\n");
        assert_eq!(gen("colou?rs?", 5, "longest"), "colours\n");
        assert_eq!(gen("(cat|dog|bird)", 5, "longest"), "bird\n");
        // `*` is capped by max_repeat = 4.
        assert_eq!(gen("ab*c", 5, "longest"), "abbbbc\n");
    }

    #[test]
    fn sequential_style_walks_choices_like_an_odometer() {
        assert_eq!(gen("[abc][xy]", 6, "sequential"), "ax\nbx\ncx\nay\nby\ncy\n");
        assert_eq!(gen("\\d{2}", 3, "sequential"), "00\n10\n20\n");
    }

    #[test]
    fn random_style_is_deterministic_for_a_seed_and_varies_with_it() {
        let a = run("[A-Z]{3}-\\d{4}", 4, "random", 42, 4, 200, true, "lines").unwrap();
        let b = run("[A-Z]{3}-\\d{4}", 4, "random", 42, 4, 200, true, "lines").unwrap();
        let c = run("[A-Z]{3}-\\d{4}", 4, "random", 7, 4, 200, true, "lines").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        for line in a.lines() {
            assert_eq!(line.len(), 8, "unexpected sample {line}");
            assert!(line[0..3].chars().all(|c| c.is_ascii_uppercase()));
            assert_eq!(&line[3..4], "-");
            assert!(line[4..].chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn unique_drops_duplicates_and_collapses_deterministic_styles() {
        assert_eq!(gen("[ab]", 10, "random").lines().count(), 2);
        assert_eq!(gen("x", 5, "shortest"), "x\n");
        // With unique off, the requested count is returned verbatim.
        assert_eq!(
            run("x", 3, "shortest", 42, 4, 200, false, "lines").unwrap(),
            "x\nx\nx\n"
        );
    }

    #[test]
    fn anchors_and_lazy_quantifiers_are_accepted() {
        assert_eq!(gen("^a+?$", 1, "shortest"), "a\n");
        assert_eq!(gen("\\Aab{2,3}?\\z", 1, "longest"), "abbb\n");
    }

    #[test]
    fn shorthands_and_negated_classes_stay_inside_their_sets() {
        let out = run("\\w\\d\\s\\S", 20, "random", 5, 4, 200, false, "lines").unwrap();
        for line in out.lines() {
            let c: Vec<char> = line.chars().collect();
            assert!(c[0].is_ascii_alphanumeric() || c[0] == '_');
            assert!(c[1].is_ascii_digit());
            assert!(c[2] == ' ' || c[2] == '\t');
            assert!(c[3] != ' ');
        }
        let out = run("[^a-y]", 30, "random", 9, 4, 200, false, "lines").unwrap();
        assert!(out.lines().all(|l| !('a'..='y').contains(&l.chars().next().unwrap())));
    }

    #[test]
    fn escapes_and_literal_braces_parse() {
        assert_eq!(gen("\\$\\d\\.\\d{2}", 1, "shortest"), "$0.00\n");
        assert_eq!(gen("a\\x41\\u0062", 1, "shortest"), "aAb\n");
        assert_eq!(gen("a{b", 1, "shortest"), "a{b\n");
        assert_eq!(gen("[\\]\\-x]{3}", 1, "shortest"), "]]]\n");
    }

    #[test]
    fn nested_groups_and_alternation_compose() {
        assert_eq!(
            gen("(https?)://(www\\.)?example\\.(com|org)", 1, "shortest"),
            "http://example.com\n"
        );
        assert_eq!(
            gen("(?:a|b)(?<year>19|20)\\d{2}", 1, "longest"),
            "b2099\n"
        );
    }

    #[test]
    fn json_output_reports_the_run_settings() {
        let out = run("[ab]", 2, "sequential", 42, 4, 200, true, "json").unwrap();
        assert_eq!(
            out,
            "{\n  \"pattern\": \"[ab]\",\n  \"style\": \"sequential\",\n  \"seed\": 42,\n  \
             \"max_repeat\": 4,\n  \"max_length\": 200,\n  \"unique\": true,\n  \
             \"requested\": 2,\n  \"returned\": 2,\n  \"samples\": [\n    \"a\",\n    \"b\"\n  ]\n}\n"
        );
    }

    #[test]
    fn csv_output_quotes_every_sample() {
        let out = run("[a\"]", 2, "sequential", 42, 4, 200, true, "csv").unwrap();
        assert_eq!(out, "index,sample\n1,\"a\"\n2,\"\"\"\"\n");
    }

    #[test]
    fn max_repeat_caps_unbounded_quantifiers() {
        assert_eq!(
            run("a+", 1, "longest", 42, 1, 200, true, "lines").unwrap(),
            "a\n"
        );
        assert_eq!(
            run("a+", 1, "longest", 42, 12, 200, true, "lines").unwrap(),
            "aaaaaaaaaaaa\n"
        );
        // Never below the minimum the pattern itself demands.
        assert_eq!(
            run("a{6,8}", 1, "longest", 42, 2, 200, true, "lines").unwrap(),
            "aaaaaa\n"
        );
    }

    #[test]
    fn max_length_bounds_every_sample() {
        let out = run("[a-z]{1,50}", 8, "random", 3, 50, 12, false, "lines").unwrap();
        assert!(out.lines().all(|l| l.len() <= 12), "{out}");
        let err = run("\\d{20}", 1, "random", 42, 4, 10, true, "lines").unwrap_err();
        assert!(err.contains("shortest string matching this pattern is 20"), "{err}");
    }

    #[test]
    fn rejects_unsupported_constructs_with_a_specific_message() {
        for (pattern, needle) in [
            ("foo(?=bar)", "lookahead"),
            ("(?<!a)b", "lookbehind"),
            ("(a)\\1", "backreference"),
            ("(?i)abc", "inline flag"),
            ("\\bword\\b", "word boundary"),
            ("[[:alpha:]]", "POSIX"),
            ("\\p{L}", "Unicode property"),
            ("a*+", "possessive"),
            ("(?>ab)", "atomic"),
        ] {
            let err = run(pattern, 1, "random", 42, 4, 200, true, "lines").unwrap_err();
            assert!(err.contains(needle), "{pattern}: {err}");
        }
    }

    #[test]
    fn rejects_malformed_patterns() {
        for (pattern, needle) in [
            ("", "pattern is empty"),
            ("(ab", "unclosed group"),
            ("ab)", "unmatched ')'"),
            ("[a-z", "unclosed character class"),
            ("[z-a]", "invalid character range"),
            ("a{3,1}", "upper bound is below"),
            ("*a", "nothing to repeat"),
            ("a\\", "trailing backslash"),
            ("[^\\x20-\\x7E]", "excludes every character"),
        ] {
            let err = run(pattern, 1, "random", 42, 4, 200, true, "lines").unwrap_err();
            assert!(err.contains(needle), "{pattern}: {err}");
        }
    }

    #[test]
    fn rejects_out_of_range_settings() {
        assert!(run("a", 0, "random", 42, 4, 200, true, "lines")
            .unwrap_err()
            .contains("count must be between 1 and 200"));
        assert!(run("a", 201, "random", 42, 4, 200, true, "lines")
            .unwrap_err()
            .contains("count must be between 1 and 200"));
        assert!(run("a", 1, "random", 42, 51, 200, true, "lines")
            .unwrap_err()
            .contains("max_repeat must be between 1 and 50"));
        assert!(run("a", 1, "random", 42, 4, 5000, true, "lines")
            .unwrap_err()
            .contains("max_length must be between 1 and 2000"));
        assert!(run("a", 1, "fastest", 42, 4, 200, true, "lines")
            .unwrap_err()
            .contains("unknown style"));
        assert!(run("a", 1, "random", 42, 4, 200, true, "yaml")
            .unwrap_err()
            .contains("unknown output"));
    }
}
