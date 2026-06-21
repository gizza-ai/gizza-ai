//! gizza-ai/latex-math-to-svg core — render a LaTeX math expression into a
//! standalone SVG, with no TeX install and no external crate. Pure Rust, so it
//! runs on every backend (incl. the chat Service Worker).
//!
//! This is a focused math typesetter, not a full TeX engine. It builds a box
//! model (each node has width/ascent/descent in EM units), lays the boxes out
//! horizontally, and emits `<text>`/`<line>`/`<path>` SVG primitives. It covers
//! the common inline-math subset: digits, letters (italic variables), Greek
//! letters, binary/relation operators, super/subscripts (with `{...}` groups),
//! `\frac`, `\sqrt` (+ optional index), `\sum`/`\int`/`\prod` and friends,
//! grouped `{...}`, sized delimiters via `\left(...\right)`, `\cdot`, `\times`,
//! `\pm`, `\leq`, `\geq`, `\neq`, `\to`, `\infty`, `\sin`/`\cos`/... operators
//! and explicit spacing (`\,` `\;` `\quad`). Unknown commands render their name
//! as upright text rather than erroring, so output is always produced.
//!
//! Units: 1 EM == `EM` px. Metrics are approximate (monospace-ish advance for
//! layout, real glyphs at render) but produce clean, correctly-spaced output.

mod symbols;
use symbols::lookup_symbol;

/// Pixels per EM at the base font size.
const EM: f64 = 24.0;
/// Average glyph advance as a fraction of EM (used for horizontal layout).
const CHAR_W: f64 = 0.55;
const DIGIT_W: f64 = 0.5;
/// Default ascent/descent for a row of text, in EM.
const ASCENT: f64 = 0.72;
const DESCENT: f64 = 0.22;
/// Script (super/subscript) scale factor.
const SCRIPT: f64 = 0.7;
const SCRIPT2: f64 = 0.5;
/// SVG outer padding, in px.
const PAD: f64 = 8.0;

/// Render style: text colour and whether to draw upright (text) vs italic (math).
#[derive(Clone, Copy)]
struct Style {
    /// font size in px
    size: f64,
}

/// A laid-out box. Coordinates are local; the box spans
/// `[0,width] x [-ascent, descent]` with the baseline at y=0.
struct Bx {
    width: f64,
    ascent: f64,
    descent: f64,
    /// SVG fragment, already positioned relative to this box's origin
    /// (baseline at y=0, left edge at x=0).
    svg: String,
}

impl Bx {
    fn height(&self) -> f64 {
        self.ascent + self.descent
    }
    /// Shift this box's content by (dx, dy) and return the SVG.
    fn placed(&self, dx: f64, dy: f64) -> String {
        if dx == 0.0 && dy == 0.0 {
            self.svg.clone()
        } else {
            format!(
                "<g transform=\"translate({},{})\">{}</g>",
                fmt(dx),
                fmt(dy),
                self.svg
            )
        }
    }
}

/// Format a float compactly for SVG (trim trailing zeros).
fn fmt(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn esc_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// a single ordinary character (digit, letter, punctuation)
    Char(char),
    /// a `\command` (without the leading backslash)
    Cmd(String),
    OpenBrace,
    CloseBrace,
    Caret,     // ^
    Underscore, // _
}

fn tokenize(input: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    return Err("trailing backslash with no command".into());
                }
                let first = chars[i];
                if first.is_ascii_alphabetic() {
                    let mut name = String::new();
                    while i < chars.len() && chars[i].is_ascii_alphabetic() {
                        name.push(chars[i]);
                        i += 1;
                    }
                    toks.push(Tok::Cmd(name));
                } else {
                    // control symbol: \, \{ \} \% \& \# \  etc.
                    toks.push(Tok::Cmd(first.to_string()));
                    i += 1;
                }
            }
            '{' => {
                toks.push(Tok::OpenBrace);
                i += 1;
            }
            '}' => {
                toks.push(Tok::CloseBrace);
                i += 1;
            }
            '^' => {
                toks.push(Tok::Caret);
                i += 1;
            }
            '_' => {
                toks.push(Tok::Underscore);
                i += 1;
            }
            '$' => {
                // ignore math-mode delimiters
                i += 1;
            }
            c if c.is_whitespace() => {
                i += 1; // whitespace is not significant in math mode
            }
            _ => {
                toks.push(Tok::Char(c));
                i += 1;
            }
        }
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------
// Parser → atom list
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Atom {
    /// ordinary symbol(s) drawn as italic math text
    Ord(String),
    /// operator/relation/upright text drawn upright (with surrounding space if spaced)
    Op { text: String, lspace: f64, rspace: f64 },
    /// a named function operator like \sin (upright, small right space)
    Func(String),
    /// big operator (\sum, \int, ...) that can carry limits
    BigOp(String),
    /// raw explicit horizontal space in EM
    Space(f64),
    /// a group: nested atom list
    Group(Vec<Atom>),
    Frac(Vec<Atom>, Vec<Atom>),
    Sqrt { index: Option<Vec<Atom>>, body: Vec<Atom> },
    /// \left( ... \right) with chosen delimiters
    Delim { left: String, right: String, body: Vec<Atom> },
    /// base with optional superscript/subscript
    Script {
        base: Box<Atom>,
        sup: Option<Vec<Atom>>,
        sub: Option<Vec<Atom>>,
    },
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn new(toks: Vec<Tok>) -> Self {
        Parser { toks, pos: 0 }
    }
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// Parse atoms until EOF or a close brace (which is consumed by the caller).
    fn parse_list(&mut self, until_close: bool) -> Result<Vec<Atom>, String> {
        let mut out: Vec<Atom> = Vec::new();
        loop {
            match self.peek() {
                None => {
                    if until_close {
                        return Err("unbalanced '{' — missing '}'".into());
                    }
                    break;
                }
                Some(Tok::CloseBrace) => {
                    if until_close {
                        self.next(); // consume the }
                        break;
                    }
                    return Err("unexpected '}'".into());
                }
                Some(Tok::Caret) | Some(Tok::Underscore) => {
                    // attach to the previous atom
                    let base = out.pop().ok_or("'^' or '_' with no preceding atom")?;
                    let scripted = self.parse_scripts(base)?;
                    out.push(scripted);
                }
                _ => {
                    let a = self.parse_atom()?;
                    // an atom may be immediately followed by scripts
                    if matches!(self.peek(), Some(Tok::Caret) | Some(Tok::Underscore)) {
                        let scripted = self.parse_scripts(a)?;
                        out.push(scripted);
                    } else {
                        out.push(a);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Parse one ^/_ (and possibly the other) onto `base`.
    fn parse_scripts(&mut self, base: Atom) -> Result<Atom, String> {
        let mut sup = None;
        let mut sub = None;
        loop {
            match self.peek() {
                Some(Tok::Caret) => {
                    self.next();
                    if sup.is_some() {
                        return Err("double superscript".into());
                    }
                    sup = Some(self.parse_script_arg()?);
                }
                Some(Tok::Underscore) => {
                    self.next();
                    if sub.is_some() {
                        return Err("double subscript".into());
                    }
                    sub = Some(self.parse_script_arg()?);
                }
                _ => break,
            }
        }
        Ok(Atom::Script {
            base: Box::new(base),
            sup,
            sub,
        })
    }

    /// The argument of ^ or _ : either a braced group or a single atom.
    fn parse_script_arg(&mut self) -> Result<Vec<Atom>, String> {
        match self.peek() {
            Some(Tok::OpenBrace) => {
                self.next();
                self.parse_list(true)
            }
            Some(_) => {
                let a = self.parse_atom()?;
                Ok(vec![a])
            }
            None => Err("'^' or '_' with no argument".into()),
        }
    }

    /// The argument of a command like \frac, \sqrt: a braced group or single atom.
    fn parse_arg(&mut self) -> Result<Vec<Atom>, String> {
        match self.peek() {
            Some(Tok::OpenBrace) => {
                self.next();
                self.parse_list(true)
            }
            Some(_) => Ok(vec![self.parse_atom()?]),
            None => Err("command missing its argument".into()),
        }
    }

    fn parse_atom(&mut self) -> Result<Atom, String> {
        let t = self.next().ok_or("unexpected end of input")?;
        match t {
            Tok::Char(c) => Ok(classify_char(c)),
            Tok::OpenBrace => {
                let inner = self.parse_list(true)?;
                Ok(Atom::Group(inner))
            }
            Tok::Cmd(name) => self.parse_command(&name),
            Tok::CloseBrace => Err("unexpected '}'".into()),
            Tok::Caret | Tok::Underscore => {
                Err("'^' or '_' with no preceding atom".into())
            }
        }
    }

    fn parse_command(&mut self, name: &str) -> Result<Atom, String> {
        match name {
            "frac" | "dfrac" | "tfrac" => {
                let num = self.parse_arg()?;
                let den = self.parse_arg()?;
                Ok(Atom::Frac(num, den))
            }
            "sqrt" => {
                // optional [index]
                let index = self.parse_optional_bracket()?;
                let body = self.parse_arg()?;
                Ok(Atom::Sqrt { index, body })
            }
            "left" => self.parse_delim(),
            // explicit spaces
            "," => Ok(Atom::Space(0.167)),
            ":" | ">" => Ok(Atom::Space(0.222)),
            ";" => Ok(Atom::Space(0.278)),
            "!" => Ok(Atom::Space(-0.167)),
            "quad" => Ok(Atom::Space(1.0)),
            "qquad" => Ok(Atom::Space(2.0)),
            " " => Ok(Atom::Space(0.25)),
            "{" => Ok(Atom::Op { text: "{".into(), lspace: 0.0, rspace: 0.0 }),
            "}" => Ok(Atom::Op { text: "}".into(), lspace: 0.0, rspace: 0.0 }),
            "%" => Ok(Atom::Op { text: "%".into(), lspace: 0.0, rspace: 0.0 }),
            "&" => Ok(Atom::Op { text: "&".into(), lspace: 0.0, rspace: 0.0 }),
            "#" => Ok(Atom::Op { text: "#".into(), lspace: 0.0, rspace: 0.0 }),
            "_" => Ok(Atom::Op { text: "_".into(), lspace: 0.0, rspace: 0.0 }),
            _ => {
                if let Some(kind) = symbols::function_name(name) {
                    Ok(Atom::Func(kind.to_string()))
                } else if symbols::is_big_op(name) {
                    Ok(Atom::BigOp(lookup_symbol(name).unwrap_or_else(|| name.to_string())))
                } else if let Some(sym) = lookup_symbol(name) {
                    // a known symbol — classify by category for spacing
                    Ok(classify_symbol(name, &sym))
                } else {
                    // unknown: render the command name upright
                    Ok(Atom::Func(name.to_string()))
                }
            }
        }
    }

    /// Parse an optional `[...]` argument (for \sqrt[n]).
    fn parse_optional_bracket(&mut self) -> Result<Option<Vec<Atom>>, String> {
        // a '[' is tokenized as Tok::Char('[')
        if matches!(self.peek(), Some(Tok::Char('['))) {
            self.next(); // consume [
            let mut inner = Vec::new();
            loop {
                match self.peek() {
                    Some(Tok::Char(']')) => {
                        self.next();
                        break;
                    }
                    None => return Err("unterminated '[' in \\sqrt".into()),
                    _ => {
                        let a = self.parse_atom()?;
                        if matches!(self.peek(), Some(Tok::Caret) | Some(Tok::Underscore)) {
                            inner.push(self.parse_scripts(a)?);
                        } else {
                            inner.push(a);
                        }
                    }
                }
            }
            Ok(Some(inner))
        } else {
            Ok(None)
        }
    }

    /// Parse `\left<d> ... \right<d>`.
    fn parse_delim(&mut self) -> Result<Atom, String> {
        let left = self.read_delim()?;
        let body = self.parse_until_right()?;
        let right = self.read_delim()?;
        Ok(Atom::Delim { left, right, body })
    }

    /// Read a delimiter following \left or \right.
    fn read_delim(&mut self) -> Result<String, String> {
        match self.next() {
            Some(Tok::Char('(')) => Ok("(".into()),
            Some(Tok::Char(')')) => Ok(")".into()),
            Some(Tok::Char('[')) => Ok("[".into()),
            Some(Tok::Char(']')) => Ok("]".into()),
            Some(Tok::Char('|')) => Ok("|".into()),
            Some(Tok::Char('.')) => Ok(".".into()), // null delimiter
            Some(Tok::Char('/')) => Ok("/".into()),
            Some(Tok::OpenBrace) | Some(Tok::Cmd(_)) if false => unreachable!(),
            Some(Tok::Cmd(c)) => match c.as_str() {
                "{" | "lbrace" => Ok("{".into()),
                "}" | "rbrace" => Ok("}".into()),
                "langle" => Ok("⟨".into()),
                "rangle" => Ok("⟩".into()),
                "lvert" | "vert" => Ok("|".into()),
                "rvert" => Ok("|".into()),
                "lceil" => Ok("⌈".into()),
                "rceil" => Ok("⌉".into()),
                "lfloor" => Ok("⌊".into()),
                "rfloor" => Ok("⌋".into()),
                other => Err(format!("unsupported delimiter '\\{other}'")),
            },
            other => Err(format!("expected a delimiter, found {other:?}")),
        }
    }

    fn parse_until_right(&mut self) -> Result<Vec<Atom>, String> {
        let mut out = Vec::new();
        loop {
            match self.peek() {
                Some(Tok::Cmd(c)) if c == "right" => {
                    self.next();
                    break;
                }
                None => return Err("\\left without matching \\right".into()),
                Some(Tok::CloseBrace) => return Err("unexpected '}' inside \\left...\\right".into()),
                Some(Tok::Caret) | Some(Tok::Underscore) => {
                    let base = out.pop().ok_or("'^' or '_' with no preceding atom")?;
                    out.push(self.parse_scripts(base)?);
                }
                _ => {
                    let a = self.parse_atom()?;
                    if matches!(self.peek(), Some(Tok::Caret) | Some(Tok::Underscore)) {
                        out.push(self.parse_scripts(a)?);
                    } else {
                        out.push(a);
                    }
                }
            }
        }
        Ok(out)
    }
}

fn classify_char(c: char) -> Atom {
    match c {
        '0'..='9' => Atom::Ord(c.to_string()),
        '+' | '-' | '*' => Atom::Op {
            text: if c == '-' { "\u{2212}".into() } else { c.to_string() },
            lspace: 0.22,
            rspace: 0.22,
        },
        '=' | '<' | '>' => Atom::Op { text: c.to_string(), lspace: 0.28, rspace: 0.28 },
        ',' | ';' => Atom::Op { text: c.to_string(), lspace: 0.0, rspace: 0.17 },
        '!' | '?' => Atom::Op { text: c.to_string(), lspace: 0.0, rspace: 0.0 },
        '.' | '/' | '(' | ')' | '[' | ']' | '|' => {
            Atom::Op { text: c.to_string(), lspace: 0.0, rspace: 0.0 }
        }
        ':' => Atom::Op { text: c.to_string(), lspace: 0.0, rspace: 0.28 },
        _ => Atom::Ord(c.to_string()), // letters → italic variable
    }
}

fn classify_symbol(name: &str, sym: &str) -> Atom {
    use symbols::Cat;
    match symbols::category(name) {
        Cat::Rel => Atom::Op { text: sym.into(), lspace: 0.28, rspace: 0.28 },
        Cat::Bin => Atom::Op { text: sym.into(), lspace: 0.22, rspace: 0.22 },
        Cat::Punct => Atom::Op { text: sym.into(), lspace: 0.0, rspace: 0.17 },
        Cat::Open => Atom::Op { text: sym.into(), lspace: 0.0, rspace: 0.0 },
        Cat::Ord => Atom::Ord(sym.into()),
    }
}

// ---------------------------------------------------------------------------
// Layout: atom list → Bx
// ---------------------------------------------------------------------------

fn char_advance(s: &str, size: f64) -> f64 {
    // approximate advance: digits narrower than letters
    let mut w = 0.0;
    for c in s.chars() {
        let frac = if c.is_ascii_digit() {
            DIGIT_W
        } else if c.is_ascii_punctuation() {
            0.4
        } else if !c.is_ascii() {
            0.6 // greek / symbols a touch wider
        } else {
            CHAR_W
        };
        w += frac * size;
    }
    w
}

fn layout_text(text: &str, italic: bool, size: f64, color_class: &str) -> Bx {
    let w = char_advance(text, size);
    let style = if italic { " font-style=\"italic\"" } else { "" };
    let svg = format!(
        "<text x=\"0\" y=\"0\" font-size=\"{}\" class=\"{}\"{}>{}</text>",
        fmt(size),
        color_class,
        style,
        esc_xml(text)
    );
    Bx {
        width: w,
        ascent: ASCENT * size,
        descent: DESCENT * size,
        svg,
    }
}

/// Lay out an atom list horizontally, returning a single box.
fn layout_list(atoms: &[Atom], style: Style) -> Bx {
    let mut x = 0.0_f64;
    let mut ascent = 0.0_f64;
    let mut descent = 0.0_f64;
    let mut svg = String::new();
    let mut last_was_atom = false;

    for atom in atoms {
        // leading space for spaced operators
        if let Atom::Op { lspace, .. } = atom {
            if last_was_atom {
                x += lspace * style.size;
            }
        }
        let (b, rspace) = layout_atom(atom, style);
        svg.push_str(&b.placed(x, 0.0));
        x += b.width;
        if let Atom::Space(_) = atom {
            // pure space, doesn't count as adjacency
        } else {
            last_was_atom = true;
        }
        x += rspace;
        ascent = ascent.max(b.ascent);
        descent = descent.max(b.descent);
    }
    if ascent == 0.0 {
        ascent = ASCENT * style.size;
    }
    if descent == 0.0 {
        descent = DESCENT * style.size;
    }
    Bx {
        width: x,
        ascent,
        descent,
        svg,
    }
}

/// Lay out a single atom; returns (box, trailing-space-in-px).
fn layout_atom(atom: &Atom, style: Style) -> (Bx, f64) {
    match atom {
        Atom::Ord(s) => (layout_text(s, is_italic_ord(s), style.size, "var"), 0.0),
        Atom::Op { text, rspace, .. } => {
            (layout_text(text, false, style.size, "op"), rspace * style.size)
        }
        Atom::Func(name) => {
            let b = layout_text(name, false, style.size, "func");
            (b, 0.167 * style.size)
        }
        Atom::Space(em) => (
            Bx {
                width: em * style.size,
                ascent: 0.0,
                descent: 0.0,
                svg: String::new(),
            },
            0.0,
        ),
        Atom::Group(inner) => (layout_list(inner, style), 0.0),
        Atom::Frac(num, den) => (layout_frac(num, den, style), 0.0),
        Atom::Sqrt { index, body } => (layout_sqrt(index.as_deref(), body, style), 0.0),
        Atom::Delim { left, right, body } => (layout_delim(left, right, body, style), 0.0),
        Atom::BigOp(sym) => (layout_text(sym, false, style.size * 1.35, "op"), 0.0),
        Atom::Script { base, sup, sub } => (layout_script(base, sup.as_deref(), sub.as_deref(), style), 0.0),
    }
}

/// Single-letter Latin variables are italic; multi-char (e.g. transformed) too.
fn is_italic_ord(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_alphabetic())
}

fn layout_frac(num: &[Atom], den: &[Atom], style: Style) -> Bx {
    let sub = Style { size: style.size * 0.92 };
    let nb = layout_list(num, sub);
    let db = layout_list(den, sub);
    let rule = (style.size * 0.05).max(1.0);
    let gap = style.size * 0.12;
    let width = nb.width.max(db.width) + style.size * 0.2;
    let nx = (width - nb.width) / 2.0;
    let dx = (width - db.width) / 2.0;
    // numerator sits above the baseline, denominator below
    let axis = style.size * 0.26; // fraction bar height above baseline
    let num_baseline = axis - rule / 2.0 - gap - nb.descent;
    let den_baseline = axis + rule / 2.0 + gap + db.ascent;
    let mut svg = String::new();
    svg.push_str(&nb.placed(nx, num_baseline));
    svg.push_str(&db.placed(dx, den_baseline));
    svg.push_str(&format!(
        "<line x1=\"0\" y1=\"{}\" x2=\"{}\" y2=\"{}\" class=\"rule\" stroke-width=\"{}\"/>",
        fmt(axis),
        fmt(width),
        fmt(axis),
        fmt(rule)
    ));
    Bx {
        width,
        ascent: -num_baseline + nb.ascent,
        descent: den_baseline + db.descent,
        svg,
    }
}

fn layout_sqrt(index: Option<&[Atom]>, body: &[Atom], style: Style) -> Bx {
    let inner = layout_list(body, style);
    let pad = style.size * 0.12;
    let extra_top = style.size * 0.12;
    let body_h = inner.height() + extra_top;
    let surd_w = style.size * 0.55;
    let body_x = surd_w + pad;
    let top = -inner.ascent - extra_top;
    let bottom = inner.descent;
    let rule = (style.size * 0.045).max(1.0);
    // surd path: a checkmark from low-left up to the top-left of the bar
    let foot_y = bottom - body_h * 0.25;
    let p = format!(
        "M {} {} L {} {} L {} {} L {} {}",
        fmt(surd_w * 0.05),
        fmt(foot_y),
        fmt(surd_w * 0.32),
        fmt(bottom),
        fmt(surd_w * 0.62),
        fmt(top + rule / 2.0),
        fmt(body_x + inner.width + pad),
        fmt(top + rule / 2.0)
    );
    let mut svg = String::new();
    // optional index
    let mut origin = 0.0;
    if let Some(idx) = index {
        let isub = Style { size: style.size * SCRIPT2 };
        let ib = layout_list(idx, isub);
        let iy = top - ib.descent + style.size * 0.05;
        svg.push_str(&ib.placed(0.0, iy));
        origin = (ib.width - surd_w * 0.3).max(0.0);
    }
    let g = format!(
        "<g transform=\"translate({},0)\">{}<path d=\"{}\" class=\"rule\" fill=\"none\" stroke-width=\"{}\"/></g>",
        fmt(origin),
        inner.placed(origin + body_x, 0.0),
        p,
        fmt(rule)
    );
    svg.push_str(&g);
    Bx {
        width: origin + body_x + inner.width + pad,
        ascent: -top,
        descent: bottom,
        svg,
    }
}

fn delim_glyph(d: &str) -> Option<&'static str> {
    match d {
        "(" => Some("("),
        ")" => Some(")"),
        "[" => Some("["),
        "]" => Some("]"),
        "|" => Some("|"),
        "{" => Some("{"),
        "}" => Some("}"),
        "/" => Some("/"),
        "⟨" => Some("⟨"),
        "⟩" => Some("⟩"),
        "⌈" => Some("⌈"),
        "⌉" => Some("⌉"),
        "⌊" => Some("⌊"),
        "⌋" => Some("⌋"),
        _ => None,
    }
}

fn layout_delim(left: &str, right: &str, body: &[Atom], style: Style) -> Bx {
    let inner = layout_list(body, style);
    // scale delimiters to inner height
    let h = inner.height().max(style.size);
    let dscale = (h / (ASCENT + DESCENT) / style.size).max(1.0);
    let dsize = style.size * dscale;
    let dy = (inner.ascent - inner.descent) / 2.0 + dsize * 0.32;

    let mut x = 0.0;
    let mut svg = String::new();
    if left != "." {
        if let Some(g) = delim_glyph(left) {
            let dw = char_advance(g, dsize) * 0.6 + style.size * 0.1;
            svg.push_str(&format!(
                "<text x=\"0\" y=\"{}\" font-size=\"{}\" class=\"op\">{}</text>",
                fmt(dy),
                fmt(dsize),
                esc_xml(g)
            ));
            x += dw;
        }
    }
    let inner_x = x;
    svg.push_str(&inner.placed(inner_x, 0.0));
    x += inner.width;
    if right != "." {
        if let Some(g) = delim_glyph(right) {
            let dw = char_advance(g, dsize) * 0.6 + style.size * 0.1;
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" class=\"op\">{}</text>",
                fmt(x),
                fmt(dy),
                fmt(dsize),
                esc_xml(g)
            ));
            x += dw;
        }
    }
    let ascent = inner.ascent.max(dsize * ASCENT - dy.min(0.0).abs());
    Bx {
        width: x,
        ascent: ascent.max(inner.ascent),
        descent: inner.descent.max(dsize * 0.32),
        svg,
    }
}

fn layout_script(base: &Atom, sup: Option<&[Atom]>, sub: Option<&[Atom]>, style: Style) -> Bx {
    let (bb, _) = layout_atom(base, style);
    let is_big = matches!(base, Atom::BigOp(_));
    let ssize = style.size * SCRIPT;
    let sstyle = Style { size: ssize };

    if is_big {
        // limits go above / below
        return layout_limits(bb, sup, sub, sstyle);
    }

    let mut width = bb.width;
    let mut ascent = bb.ascent;
    let mut descent = bb.descent;
    let mut svg = bb.placed(0.0, 0.0);
    let sup_shift = style.size * 0.45;
    let sub_shift = style.size * 0.18;
    let mut max_script_w = 0.0_f64;

    if let Some(s) = sup {
        let sbx = layout_list(s, sstyle);
        let y = -(bb.ascent * 0.55) - sup_shift + sbx.descent;
        svg.push_str(&sbx.placed(bb.width + style.size * 0.02, y));
        ascent = ascent.max(-y + sbx.ascent);
        max_script_w = max_script_w.max(sbx.width);
    }
    if let Some(s) = sub {
        let sbx = layout_list(s, sstyle);
        let y = sub_shift + sbx.ascent;
        svg.push_str(&sbx.placed(bb.width + style.size * 0.02, y));
        descent = descent.max(y + sbx.descent);
        max_script_w = max_script_w.max(sbx.width);
    }
    width += max_script_w + style.size * 0.04;
    Bx {
        width,
        ascent,
        descent,
        svg,
    }
}

fn layout_limits(base: Bx, sup: Option<&[Atom]>, sub: Option<&[Atom]>, sstyle: Style) -> Bx {
    let supb = sup.map(|s| layout_list(s, sstyle));
    let subb = sub.map(|s| layout_list(s, sstyle));
    let width = base
        .width
        .max(supb.as_ref().map(|b| b.width).unwrap_or(0.0))
        .max(subb.as_ref().map(|b| b.width).unwrap_or(0.0));
    let bx = (width - base.width) / 2.0;
    let mut svg = base.placed(bx, 0.0);
    let mut ascent = base.ascent;
    let mut descent = base.descent;
    let gap = sstyle.size * 0.25;
    if let Some(b) = supb {
        let x = (width - b.width) / 2.0;
        let y = -base.ascent - gap - b.descent;
        svg.push_str(&b.placed(x, y));
        ascent = -y + b.ascent;
    }
    if let Some(b) = subb {
        let x = (width - b.width) / 2.0;
        let y = base.descent + gap + b.ascent;
        svg.push_str(&b.placed(x, y));
        descent = y + b.descent;
    }
    Bx {
        width,
        ascent,
        descent,
        svg,
    }
}

// ---------------------------------------------------------------------------
// Public entry
// ---------------------------------------------------------------------------

/// Render a LaTeX math expression into a standalone SVG document string.
/// `color` is any CSS color (e.g. `#1a1a1a`); empty → default dark.
pub fn render_svg(input: &str, color: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("empty expression".into());
    }
    if trimmed.chars().count() > 4000 {
        return Err("expression too long (max 4000 characters)".into());
    }
    let toks = tokenize(trimmed)?;
    let mut parser = Parser::new(toks);
    let atoms = parser.parse_list(false)?;
    if atoms.is_empty() {
        return Err("nothing to render".into());
    }
    let style = Style { size: EM };
    let body = layout_list(&atoms, style);

    let w = body.width + PAD * 2.0;
    let h = body.height() + PAD * 2.0;
    let baseline = PAD + body.ascent;
    let col = sanitize_color(color);

    let svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"{label}\">\n\
<style>text{{font-family:'Latin Modern Math','STIX Two Math','Cambria Math','Times New Roman',serif;fill:{col};dominant-baseline:alphabetic;}}.func{{font-style:normal;}}.op{{font-style:normal;}}.rule{{stroke:{col};}}</style>\n\
<g transform=\"translate({px},{by})\">{content}</g>\n\
</svg>\n",
        w = fmt(w),
        h = fmt(h),
        label = esc_xml(trimmed),
        col = col,
        px = fmt(PAD),
        by = fmt(baseline),
        content = body.svg,
    );
    Ok(svg)
}

fn sanitize_color(c: &str) -> String {
    let c = c.trim();
    if c.is_empty() {
        return "#1a1a1a".to_string();
    }
    // allow #hex, rgb(...), and a small set of named colors; reject anything
    // with characters that could break out of the style attribute.
    let ok = c.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(ch, '#' | '(' | ')' | ',' | '.' | '%' | ' ')
    });
    if ok {
        c.to_string()
    } else {
        "#1a1a1a".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> String {
        render_svg(s, "").expect("should render")
    }

    #[test]
    fn renders_simple_expression() {
        let svg = ok("a+b=c");
        assert!(svg.starts_with("<?xml"));
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        // the variables show up as text
        assert!(svg.contains(">a</text>") || svg.contains(">a<"));
    }

    #[test]
    fn renders_fraction() {
        let svg = ok("\\frac{a}{b}");
        assert!(svg.contains("<line")); // the fraction rule
        assert!(svg.contains("class=\"rule\""));
    }

    #[test]
    fn renders_superscript() {
        let svg = ok("x^2");
        assert!(svg.contains(">2</text>"));
        // the script font-size is smaller than the base 24
        assert!(svg.contains("font-size=\"16.8\"") || svg.contains("font-size=\"16"));
    }

    #[test]
    fn renders_sqrt() {
        let svg = ok("\\sqrt{x+1}");
        assert!(svg.contains("<path")); // the surd
    }

    #[test]
    fn renders_greek_and_symbols() {
        let svg = ok("\\alpha + \\beta \\leq \\gamma");
        assert!(svg.contains("α"));
        assert!(svg.contains("β"));
        assert!(svg.contains("≤"));
    }

    #[test]
    fn renders_sum_with_limits() {
        let svg = ok("\\sum_{i=1}^{n} i");
        // big operator present + limits laid out
        assert!(svg.contains("∑"));
        assert!(svg.contains(">n</text>"));
    }

    #[test]
    fn renders_left_right_delims() {
        let svg = ok("\\left(\\frac{a}{b}\\right)");
        assert!(svg.contains("("));
        assert!(svg.contains(")"));
    }

    #[test]
    fn quadratic_formula() {
        let svg = ok("x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}");
        assert!(svg.contains("±"));
        assert!(svg.contains("<path"));
        assert!(svg.contains("<line"));
    }

    #[test]
    fn unknown_command_renders_as_text() {
        // should not error; renders the name upright
        let svg = render_svg("\\foobar x", "").expect("renders");
        assert!(svg.contains("foobar"));
    }

    #[test]
    fn empty_is_error() {
        assert!(render_svg("   ", "").is_err());
    }

    #[test]
    fn unbalanced_brace_is_error() {
        assert!(render_svg("\\frac{a}{b", "").is_err());
    }

    #[test]
    fn color_is_sanitized() {
        let svg = render_svg("x", "#ff0000").unwrap();
        assert!(svg.contains("#ff0000"));
        let bad = render_svg("x", "red\"></style><script>").unwrap();
        assert!(!bad.contains("<script>"));
        assert!(bad.contains("#1a1a1a"));
    }
}
