//! Scientific calculator core shared by the chat skill block and the web page.
//!
//! The evaluator is intentionally self-contained and wasm-safe: it implements a
//! small Pratt parser over complex `f64` values, worksheet-style assignments,
//! `ans`, angle modes, and deterministic text/JSON formatting.

use num_complex::Complex64;
use serde::Serialize;
use std::collections::BTreeMap;
use std::f64::consts::{E, PI, TAU};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleUnit {
    Radians,
    Degrees,
    Gradians,
}

impl AngleUnit {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "radians" | "rad" => Ok(Self::Radians),
            "degrees" | "deg" => Ok(Self::Degrees),
            "gradians" | "grad" => Ok(Self::Gradians),
            other => Err(format!("unknown angle_unit '{other}'")),
        }
    }

    fn to_radians(self, z: Complex64) -> Complex64 {
        match self {
            Self::Radians => z,
            Self::Degrees => z * (PI / 180.0),
            Self::Gradians => z * (PI / 200.0),
        }
    }

    fn from_radians(self, z: Complex64) -> Complex64 {
        match self {
            Self::Radians => z,
            Self::Degrees => z * (180.0 / PI),
            Self::Gradians => z * (200.0 / PI),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notation {
    Auto,
    Fixed,
    Scientific,
    Engineering,
}

impl Notation {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "auto" => Ok(Self::Auto),
            "fixed" => Ok(Self::Fixed),
            "scientific" => Ok(Self::Scientific),
            "engineering" => Ok(Self::Engineering),
            other => Err(format!("unknown notation '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexForm {
    Rectangular,
    Polar,
}

impl ComplexForm {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "rectangular" => Ok(Self::Rectangular),
            "polar" => Ok(Self::Polar),
            other => Err(format!("unknown complex_form '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            other => Err(format!("unknown output_format '{other}'")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub expression: String,
    pub variables: String,
    pub angle_unit: AngleUnit,
    pub precision: usize,
    pub notation: Notation,
    pub complex_form: ComplexForm,
    pub group_digits: bool,
    pub output_format: OutputFormat,
}

impl Options {
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            variables: String::new(),
            angle_unit: AngleUnit::Radians,
            precision: 12,
            notation: Notation::Auto,
            complex_form: ComplexForm::Rectangular,
            group_digits: false,
            output_format: OutputFormat::Text,
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonLine {
    input: String,
    result: String,
    real: f64,
    imaginary: f64,
}

#[derive(Debug, Serialize)]
struct JsonOutput {
    results: Vec<JsonLine>,
    variables: BTreeMap<String, String>,
}

/// Backwards-compatible single-expression entry point used by the scaffold and
/// a few generated examples.
pub fn run(input: &str) -> Result<String, String> {
    evaluate(Options::new(input))
}

pub fn evaluate(opts: Options) -> Result<String, String> {
    if !(1..=200).contains(&opts.precision) {
        return Err("precision must be between 1 and 200 significant digits".into());
    }

    let mut vars = builtin_vars();
    for stmt in split_statements(&opts.variables) {
        if stmt.trim().is_empty() {
            continue;
        }
        eval_statement(stmt.trim(), &mut vars, opts.angle_unit)?;
    }

    let mut lines = Vec::new();
    for stmt in split_statements(&opts.expression) {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        let (input, value) = eval_statement(stmt, &mut vars, opts.angle_unit)?;
        vars.insert("ans".to_string(), value);
        lines.push((input, value));
    }
    if lines.is_empty() {
        return Err("expression is empty".into());
    }

    let mut formatted_vars = BTreeMap::new();
    for (k, v) in vars
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "pi" | "π" | "e" | "tau" | "phi" | "i" | "ans"))
    {
        formatted_vars.insert(k.clone(), format_value(*v, &opts));
    }

    match opts.output_format {
        OutputFormat::Text => {
            let rendered: Vec<String> = lines
                .into_iter()
                .map(|(input, value)| {
                    let out = format_value(value, &opts);
                    if input.contains('=') {
                        format!("{input} -> {out}")
                    } else {
                        out
                    }
                })
                .collect();
            Ok(rendered.join("\n"))
        }
        OutputFormat::Json => {
            let results = lines
                .into_iter()
                .map(|(input, value)| JsonLine {
                    input,
                    result: format_value(value, &opts),
                    real: value.re,
                    imaginary: value.im,
                })
                .collect();
            serde_json::to_string_pretty(&JsonOutput {
                results,
                variables: formatted_vars,
            })
            .map_err(|e| format!("json encode failed: {e}"))
        }
    }
}

fn builtin_vars() -> BTreeMap<String, Complex64> {
    BTreeMap::from([
        ("pi".to_string(), Complex64::new(PI, 0.0)),
        ("π".to_string(), Complex64::new(PI, 0.0)),
        ("e".to_string(), Complex64::new(E, 0.0)),
        ("tau".to_string(), Complex64::new(TAU, 0.0)),
        (
            "phi".to_string(),
            Complex64::new((1.0 + 5.0_f64.sqrt()) / 2.0, 0.0),
        ),
        ("i".to_string(), Complex64::new(0.0, 1.0)),
        ("ans".to_string(), Complex64::new(0.0, 0.0)),
    ])
}

fn split_statements(s: &str) -> Vec<&str> {
    s.split(|c| c == '\n' || c == ';').collect()
}

fn eval_statement(
    stmt: &str,
    vars: &mut BTreeMap<String, Complex64>,
    angle: AngleUnit,
) -> Result<(String, Complex64), String> {
    if let Some((name, rhs)) = split_assignment(stmt) {
        if is_reserved(name) {
            return Err(format!("'{name}' is a reserved constant/function name"));
        }
        let value = Parser::new(rhs, vars, angle).parse()?;
        vars.insert(name.to_string(), value);
        Ok((format!("{name} = {rhs}"), value))
    } else {
        Ok((stmt.to_string(), Parser::new(stmt, vars, angle).parse()?))
    }
}

fn split_assignment(stmt: &str) -> Option<(&str, &str)> {
    let idx = stmt.find('=')?;
    if stmt[idx + 1..].contains('=') {
        return None;
    }
    let name = stmt[..idx].trim();
    let rhs = stmt[idx + 1..].trim();
    if rhs.is_empty() || !is_identifier(name) {
        return None;
    }
    Some((name, rhs))
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_reserved(s: &str) -> bool {
    matches!(
        s,
        "pi" | "π"
            | "e"
            | "tau"
            | "phi"
            | "i"
            | "ans"
            | "sin"
            | "cos"
            | "tan"
            | "asin"
            | "acos"
            | "atan"
            | "sinh"
            | "cosh"
            | "tanh"
            | "asinh"
            | "acosh"
            | "atanh"
            | "ln"
            | "log"
            | "log2"
            | "sqrt"
            | "cbrt"
            | "root"
            | "abs"
            | "sign"
            | "floor"
            | "ceil"
            | "round"
            | "trunc"
            | "gcd"
            | "lcm"
            | "ncr"
            | "npr"
            | "hypot"
            | "atan2"
            | "min"
            | "max"
            | "pow"
            | "mod"
            | "fact"
            | "re"
            | "im"
            | "arg"
            | "conj"
            | "sec"
            | "csc"
            | "cot"
    )
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Percent,
    Bang,
    LParen,
    RParen,
    Comma,
    End,
}

struct Lexer<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self {
        Self { s, i: 0 }
    }

    fn next(&mut self) -> Result<Token, String> {
        self.skip_ws();
        let rest = &self.s[self.i..];
        let Some(c) = rest.chars().next() else {
            return Ok(Token::End);
        };
        match c {
            '+' => {
                self.i += 1;
                Ok(Token::Plus)
            }
            '-' | '−' => {
                self.i += c.len_utf8();
                Ok(Token::Minus)
            }
            '*' | '×' => {
                self.i += c.len_utf8();
                Ok(Token::Star)
            }
            '/' | '÷' => {
                self.i += c.len_utf8();
                Ok(Token::Slash)
            }
            '^' => {
                self.i += 1;
                Ok(Token::Caret)
            }
            '%' => {
                self.i += 1;
                Ok(Token::Percent)
            }
            '!' => {
                self.i += 1;
                Ok(Token::Bang)
            }
            '(' => {
                self.i += 1;
                Ok(Token::LParen)
            }
            ')' => {
                self.i += 1;
                Ok(Token::RParen)
            }
            ',' => {
                self.i += 1;
                Ok(Token::Comma)
            }
            '0'..='9' | '.' => self.number(),
            c if c.is_alphabetic() || c == '_' || c == 'π' => self.ident(),
            _ => Err(format!("unexpected character '{c}' at byte {}", self.i)),
        }
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.s[self.i..].chars().next() {
            if c.is_whitespace() {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn number(&mut self) -> Result<Token, String> {
        let start = self.i;
        let mut seen_digit = false;
        while let Some(c) = self.s[self.i..].chars().next() {
            if c.is_ascii_digit() {
                seen_digit = true;
                self.i += 1;
            } else if c == '.' {
                self.i += 1;
            } else {
                break;
            }
        }
        if let Some(c) = self.s[self.i..].chars().next() {
            if c == 'e' || c == 'E' {
                let save = self.i;
                self.i += 1;
                if let Some(sign) = self.s[self.i..].chars().next() {
                    if sign == '+' || sign == '-' {
                        self.i += 1;
                    }
                }
                let before = self.i;
                while let Some(d) = self.s[self.i..].chars().next() {
                    if d.is_ascii_digit() {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                if before == self.i {
                    self.i = save;
                }
            }
        }
        if !seen_digit {
            return Err(format!("invalid number at byte {start}"));
        }
        self.s[start..self.i]
            .parse::<f64>()
            .map(Token::Num)
            .map_err(|e| format!("invalid number '{}': {e}", &self.s[start..self.i]))
    }

    fn ident(&mut self) -> Result<Token, String> {
        let start = self.i;
        while let Some(c) = self.s[self.i..].chars().next() {
            if c.is_alphanumeric() || c == '_' || c == 'π' {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
        Ok(Token::Ident(self.s[start..self.i].to_ascii_lowercase()))
    }
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    cur: Token,
    vars: &'a BTreeMap<String, Complex64>,
    angle: AngleUnit,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str, vars: &'a BTreeMap<String, Complex64>, angle: AngleUnit) -> Self {
        Self {
            lexer: Lexer::new(s),
            cur: Token::End,
            vars,
            angle,
        }
    }

    fn parse(mut self) -> Result<Complex64, String> {
        self.bump()?;
        let value = self.expr(0)?;
        if self.cur != Token::End {
            return Err(format!("unexpected token {:?}", self.cur));
        }
        if !value.re.is_finite() || !value.im.is_finite() {
            return Err("result is non-finite".into());
        }
        Ok(value)
    }

    fn bump(&mut self) -> Result<(), String> {
        self.cur = self.lexer.next()?;
        Ok(())
    }

    fn expr(&mut self, min_bp: u8) -> Result<Complex64, String> {
        let mut lhs = match self.cur.clone() {
            Token::Num(n) => {
                self.bump()?;
                Complex64::new(n, 0.0)
            }
            Token::Ident(name) => {
                self.bump()?;
                if self.cur == Token::LParen {
                    self.bump()?;
                    let mut args = Vec::new();
                    if self.cur != Token::RParen {
                        loop {
                            args.push(self.expr(0)?);
                            if self.cur == Token::Comma {
                                self.bump()?;
                                continue;
                            }
                            break;
                        }
                    }
                    if self.cur != Token::RParen {
                        return Err("expected ')' after function arguments".into());
                    }
                    self.bump()?;
                    call_fn(&name, &args, self.angle)?
                } else {
                    *self
                        .vars
                        .get(&name)
                        .ok_or_else(|| format!("unknown variable or constant '{name}'"))?
                }
            }
            Token::Minus => {
                self.bump()?;
                -self.expr(9)?
            }
            Token::Plus => {
                self.bump()?;
                self.expr(9)?
            }
            Token::LParen => {
                self.bump()?;
                let v = self.expr(0)?;
                if self.cur != Token::RParen {
                    return Err("expected ')'".into());
                }
                self.bump()?;
                v
            }
            other => return Err(format!("expected expression, got {other:?}")),
        };

        loop {
            if should_implicit_multiply(&self.cur) && min_bp <= 5 {
                let rhs = self.expr(6)?;
                lhs *= rhs;
                continue;
            }
            match self.cur.clone() {
                Token::Bang => {
                    if min_bp > 10 {
                        break;
                    }
                    self.bump()?;
                    lhs = factorial_value(lhs)?;
                }
                Token::Plus => {
                    let (l, r) = (1, 2);
                    if l < min_bp {
                        break;
                    }
                    self.bump()?;
                    let rhs = self.expr(r)?;
                    lhs += rhs;
                }
                Token::Minus => {
                    let (l, r) = (1, 2);
                    if l < min_bp {
                        break;
                    }
                    self.bump()?;
                    let rhs = self.expr(r)?;
                    lhs -= rhs;
                }
                Token::Star => {
                    let (l, r) = (5, 6);
                    if l < min_bp {
                        break;
                    }
                    self.bump()?;
                    let rhs = self.expr(r)?;
                    lhs *= rhs;
                }
                Token::Slash => {
                    let (l, r) = (5, 6);
                    if l < min_bp {
                        break;
                    }
                    self.bump()?;
                    let rhs = self.expr(r)?;
                    lhs /= rhs;
                }
                Token::Percent => {
                    let (l, r) = (5, 6);
                    if l < min_bp {
                        break;
                    }
                    self.bump()?;
                    let rhs = self.expr(r)?;
                    lhs = real_binary(lhs, rhs, |a, b| a % b, "%")?;
                }
                Token::Caret => {
                    let (l, r) = (7, 7);
                    if l < min_bp {
                        break;
                    }
                    self.bump()?;
                    let rhs = self.expr(r)?;
                    lhs = lhs.powc(rhs);
                }
                _ => break,
            }
        }
        Ok(lhs)
    }
}

fn should_implicit_multiply(tok: &Token) -> bool {
    matches!(tok, Token::Num(_) | Token::Ident(_) | Token::LParen)
}

fn expect_arity(name: &str, args: &[Complex64], arities: &[usize]) -> Result<(), String> {
    if arities.contains(&args.len()) {
        Ok(())
    } else {
        Err(format!(
            "{name} expects {:?} arguments, got {}",
            arities,
            args.len()
        ))
    }
}

fn real_arg(z: Complex64, name: &str) -> Result<f64, String> {
    if z.im.abs() > 1e-12 {
        Err(format!("{name} expects a real argument"))
    } else {
        Ok(z.re)
    }
}

fn real_binary(
    a: Complex64,
    b: Complex64,
    f: impl FnOnce(f64, f64) -> f64,
    name: &str,
) -> Result<Complex64, String> {
    Ok(Complex64::new(
        f(real_arg(a, name)?, real_arg(b, name)?),
        0.0,
    ))
}

fn int_arg(z: Complex64, name: &str) -> Result<i64, String> {
    let x = real_arg(z, name)?;
    if x.fract().abs() > 1e-12 {
        return Err(format!("{name} expects an integer"));
    }
    Ok(x as i64)
}

fn call_fn(name: &str, args: &[Complex64], angle: AngleUnit) -> Result<Complex64, String> {
    use Complex64 as C;
    Ok(match name {
        "sin" => {
            expect_arity(name, args, &[1])?;
            angle.to_radians(args[0]).sin()
        }
        "cos" => {
            expect_arity(name, args, &[1])?;
            angle.to_radians(args[0]).cos()
        }
        "tan" => {
            expect_arity(name, args, &[1])?;
            angle.to_radians(args[0]).tan()
        }
        "asin" => {
            expect_arity(name, args, &[1])?;
            angle.from_radians(args[0].asin())
        }
        "acos" => {
            expect_arity(name, args, &[1])?;
            angle.from_radians(args[0].acos())
        }
        "atan" => {
            expect_arity(name, args, &[1])?;
            angle.from_radians(args[0].atan())
        }
        "sinh" => {
            expect_arity(name, args, &[1])?;
            args[0].sinh()
        }
        "cosh" => {
            expect_arity(name, args, &[1])?;
            args[0].cosh()
        }
        "tanh" => {
            expect_arity(name, args, &[1])?;
            args[0].tanh()
        }
        "asinh" => {
            expect_arity(name, args, &[1])?;
            args[0].asinh()
        }
        "acosh" => {
            expect_arity(name, args, &[1])?;
            args[0].acosh()
        }
        "atanh" => {
            expect_arity(name, args, &[1])?;
            args[0].atanh()
        }
        "sec" => {
            expect_arity(name, args, &[1])?;
            C::new(1.0, 0.0) / angle.to_radians(args[0]).cos()
        }
        "csc" => {
            expect_arity(name, args, &[1])?;
            C::new(1.0, 0.0) / angle.to_radians(args[0]).sin()
        }
        "cot" => {
            expect_arity(name, args, &[1])?;
            C::new(1.0, 0.0) / angle.to_radians(args[0]).tan()
        }
        "ln" => {
            expect_arity(name, args, &[1])?;
            args[0].ln()
        }
        "log" => {
            expect_arity(name, args, &[1, 2])?;
            if args.len() == 1 {
                args[0].log10()
            } else {
                args[0].ln() / args[1].ln()
            }
        }
        "log2" => {
            expect_arity(name, args, &[1])?;
            args[0].ln() / C::new(2.0_f64.ln(), 0.0)
        }
        "sqrt" => {
            expect_arity(name, args, &[1])?;
            if args[0].im.abs() <= 1e-12 && args[0].re < 0.0 {
                C::new(0.0, (-args[0].re).sqrt())
            } else {
                args[0].sqrt()
            }
        }
        "cbrt" => {
            expect_arity(name, args, &[1])?;
            args[0].powf(1.0 / 3.0)
        }
        "root" => {
            expect_arity(name, args, &[2])?;
            args[0].powc(C::new(1.0, 0.0) / args[1])
        }
        "abs" => {
            expect_arity(name, args, &[1])?;
            C::new(args[0].norm(), 0.0)
        }
        "sign" => {
            expect_arity(name, args, &[1])?;
            C::new(real_arg(args[0], name)?.signum(), 0.0)
        }
        "floor" => {
            expect_arity(name, args, &[1])?;
            C::new(real_arg(args[0], name)?.floor(), 0.0)
        }
        "ceil" => {
            expect_arity(name, args, &[1])?;
            C::new(real_arg(args[0], name)?.ceil(), 0.0)
        }
        "round" => {
            expect_arity(name, args, &[1])?;
            C::new(real_arg(args[0], name)?.round(), 0.0)
        }
        "trunc" => {
            expect_arity(name, args, &[1])?;
            C::new(real_arg(args[0], name)?.trunc(), 0.0)
        }
        "gcd" => {
            expect_arity(name, args, &[2])?;
            C::new(
                gcd(int_arg(args[0], name)?.abs(), int_arg(args[1], name)?.abs()) as f64,
                0.0,
            )
        }
        "lcm" => {
            expect_arity(name, args, &[2])?;
            let a = int_arg(args[0], name)?.abs();
            let b = int_arg(args[1], name)?.abs();
            C::new((a / gcd(a, b) * b) as f64, 0.0)
        }
        "ncr" => {
            expect_arity(name, args, &[2])?;
            C::new(
                comb(int_arg(args[0], name)?, int_arg(args[1], name)?)? as f64,
                0.0,
            )
        }
        "npr" => {
            expect_arity(name, args, &[2])?;
            C::new(
                perm(int_arg(args[0], name)?, int_arg(args[1], name)?)? as f64,
                0.0,
            )
        }
        "hypot" => {
            expect_arity(name, args, &[2])?;
            C::new(
                real_arg(args[0], name)?.hypot(real_arg(args[1], name)?),
                0.0,
            )
        }
        "atan2" => {
            expect_arity(name, args, &[2])?;
            angle.from_radians(C::new(
                real_arg(args[0], name)?.atan2(real_arg(args[1], name)?),
                0.0,
            ))
        }
        "min" => {
            if args.is_empty() {
                return Err("min expects at least one argument".into());
            }
            C::new(
                args.iter()
                    .map(|z| real_arg(*z, name))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .fold(f64::INFINITY, f64::min),
                0.0,
            )
        }
        "max" => {
            if args.is_empty() {
                return Err("max expects at least one argument".into());
            }
            C::new(
                args.iter()
                    .map(|z| real_arg(*z, name))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .fold(f64::NEG_INFINITY, f64::max),
                0.0,
            )
        }
        "pow" => {
            expect_arity(name, args, &[2])?;
            args[0].powc(args[1])
        }
        "mod" => {
            expect_arity(name, args, &[2])?;
            real_binary(args[0], args[1], |a, b| a % b, name)?
        }
        "fact" => {
            expect_arity(name, args, &[1])?;
            factorial_value(args[0])?
        }
        "re" => {
            expect_arity(name, args, &[1])?;
            C::new(args[0].re, 0.0)
        }
        "im" => {
            expect_arity(name, args, &[1])?;
            C::new(args[0].im, 0.0)
        }
        "arg" => {
            expect_arity(name, args, &[1])?;
            angle.from_radians(C::new(args[0].arg(), 0.0))
        }
        "conj" => {
            expect_arity(name, args, &[1])?;
            args[0].conj()
        }
        _ => return Err(format!("unknown function '{name}'")),
    })
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.abs()
}

fn comb(n: i64, k: i64) -> Result<u64, String> {
    if n < 0 || k < 0 || k > n {
        return Err("ncr expects 0 <= k <= n".into());
    }
    let k = k.min(n - k) as u64;
    let mut out = 1u64;
    for i in 1..=k {
        out = out.checked_mul((n as u64) + 1 - i).ok_or("ncr overflow")? / i;
    }
    Ok(out)
}

fn perm(n: i64, k: i64) -> Result<u64, String> {
    if n < 0 || k < 0 || k > n {
        return Err("npr expects 0 <= k <= n".into());
    }
    let mut out = 1u64;
    for i in 0..k as u64 {
        out = out.checked_mul(n as u64 - i).ok_or("npr overflow")?;
    }
    Ok(out)
}

fn factorial_value(z: Complex64) -> Result<Complex64, String> {
    let n = int_arg(z, "factorial")?;
    if !(0..=170).contains(&n) {
        return Err("factorial expects an integer from 0 to 170".into());
    }
    let mut out = 1.0;
    for i in 2..=n {
        out *= i as f64;
    }
    Ok(Complex64::new(out, 0.0))
}

fn format_value(value: Complex64, opts: &Options) -> String {
    if opts.complex_form == ComplexForm::Polar && value.im.abs() > 1e-12 {
        let angle = opts
            .angle_unit
            .from_radians(Complex64::new(value.arg(), 0.0))
            .re;
        return format!(
            "{} ∠ {}°",
            format_real(value.norm(), opts),
            format_real(angle, opts)
        );
    }
    if value.im.abs() <= 1e-12 {
        format_real(value.re, opts)
    } else if value.re.abs() <= 1e-12 {
        if (value.im - 1.0).abs() <= 1e-12 {
            "i".to_string()
        } else if (value.im + 1.0).abs() <= 1e-12 {
            "-i".to_string()
        } else {
            format!("{}i", format_real(value.im, opts))
        }
    } else if value.im < 0.0 {
        format!(
            "{} - {}i",
            format_real(value.re, opts),
            format_real(value.im.abs(), opts)
        )
    } else {
        format!(
            "{} + {}i",
            format_real(value.re, opts),
            format_real(value.im, opts)
        )
    }
}

fn format_real(x: f64, opts: &Options) -> String {
    let p = opts.precision.min(15);
    let s = match opts.notation {
        Notation::Fixed => format!("{x:.p$}"),
        Notation::Scientific => format!("{x:.p$e}"),
        Notation::Engineering => format_engineering(x, p),
        Notation::Auto => format!("{x:.p$}"),
    };
    let s = trim_number(s);
    if opts.group_digits {
        group_number(&s)
    } else {
        s
    }
}

fn format_engineering(x: f64, precision: usize) -> String {
    if x == 0.0 || !x.is_finite() {
        return format!("{x:.precision$}");
    }
    let sign = if x < 0.0 { "-" } else { "" };
    let abs = x.abs();
    let exp = (abs.log10().floor() as i32).div_euclid(3) * 3;
    let mant = abs / 10f64.powi(exp);
    format!("{sign}{mant:.precision$}e{exp:+03}")
}

fn trim_number(mut s: String) -> String {
    if let Some(eidx) = s.find(['e', 'E']) {
        let exp = s[eidx..].to_string();
        let mut mant = s[..eidx].to_string();
        trim_decimal(&mut mant);
        mant + &exp
    } else {
        trim_decimal(&mut s);
        s
    }
}

fn trim_decimal(s: &mut String) {
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
}

fn group_number(s: &str) -> String {
    let (mant, exp) = s.split_once(['e', 'E']).map_or((s, ""), |(a, b)| (a, b));
    let (sign, body) = mant
        .strip_prefix('-')
        .map_or(("", mant), |rest| ("-", rest));
    let (int, frac) = body.split_once('.').map_or((body, ""), |(a, b)| (a, b));
    let mut out = String::new();
    for (i, ch) in int.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let int_grouped: String = out.chars().rev().collect();
    let mut result = format!("{sign}{int_grouped}");
    if !frac.is_empty() {
        result.push('.');
        result.push_str(frac);
    }
    if !exp.is_empty() {
        result.push('e');
        result.push_str(exp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(expr: &str) -> String {
        evaluate(Options::new(expr)).unwrap()
    }

    #[test]
    fn evaluates_scientific_functions_and_constants() {
        let mut opts = Options::new("sin(90) + cos(0)");
        opts.angle_unit = AngleUnit::Degrees;
        assert_eq!(evaluate(opts).unwrap(), "2");
        assert_eq!(eval("sqrt(16) + 3! + ncr(5,2)"), "20");
        assert!(eval("2pi").starts_with("6.283185"));
    }

    #[test]
    fn supports_variables_multiline_and_ans() {
        let mut opts = Options::new("x = 7\ny = x^2\ny + ans");
        opts.variables = "a = 3".into();
        assert_eq!(evaluate(opts).unwrap(), "x = 7 -> 7\ny = x^2 -> 49\n98");
    }

    #[test]
    fn supports_complex_numbers() {
        assert_eq!(eval("sqrt(-1)"), "i");
        assert_eq!(eval("(2+3i) + conj(2+3i)"), "4");
        assert!(eval("abs(3+4i)").starts_with('5'));
    }

    #[test]
    fn formats_json_and_grouping() {
        let mut opts = Options::new("10000+5");
        opts.group_digits = true;
        opts.output_format = OutputFormat::Json;
        let out = evaluate(opts).unwrap();
        assert!(out.contains("10,005"), "{out}");
    }

    #[test]
    fn rejects_bad_inputs() {
        assert!(evaluate(Options::new("unknown + 1"))
            .unwrap_err()
            .contains("unknown"));
        assert!(evaluate(Options::new("fact(200)"))
            .unwrap_err()
            .contains("factorial"));
    }
}
