//! entropy-calculator core — Shannon entropy of a symbol sequence taken from
//! text. Pure, dependency-free: one frequency pass over the chosen symbol basis
//! (characters, UTF-8 bytes, words, or lines) plus the standard derived
//! information-theory quantities (total information, maximum entropy,
//! efficiency, redundancy, perplexity) and a symbol-frequency table.
//!
//! The model is order-0: symbols are treated as independent draws from the
//! observed distribution, so `abababab` and `aabbaabb` score identically. That
//! is the same definition every Shannon-entropy calculator uses, and it is a
//! property of the string — NOT a password-guessability estimate (see the
//! password-entropy block) and not a proof of cryptographic randomness.
//!
//! No I/O, no deps → runs on every backend including the chat Service Worker.

use std::collections::HashMap;
use std::fmt::Write as _;

/// Largest input accepted, in bytes (1 MiB).
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
/// Largest number of lines/paragraphs analysed separately in `scope = line|paragraph`.
pub const MAX_PARTS: usize = 20_000;
/// Largest value accepted for `top_symbols`.
pub const MAX_TOP_SYMBOLS: usize = 64;
/// Default value for `top_symbols`.
pub const DEFAULT_TOP_SYMBOLS: usize = 12;
/// Largest value accepted for `precision`.
pub const MAX_PRECISION: usize = 10;
/// Default value for `precision`.
pub const DEFAULT_PRECISION: usize = 4;
/// Width in cells of the frequency-table bar column.
const BAR_WIDTH: usize = 20;

/// What counts as one symbol when the frequency pass runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Unicode scalar values (`char`).
    Characters,
    /// UTF-8 bytes — the 0–8 bits/byte convention used for binary data.
    Bytes,
    /// Whitespace-separated words.
    Words,
    /// Whole lines.
    Lines,
}

impl Basis {
    /// Parse the canonical wire value.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "characters" | "character" | "chars" | "char" => Ok(Basis::Characters),
            "bytes" | "byte" => Ok(Basis::Bytes),
            "words" | "word" => Ok(Basis::Words),
            "lines" | "line" => Ok(Basis::Lines),
            other => Err(format!(
                "unknown basis '{other}'; expected one of: characters, bytes, words, lines"
            )),
        }
    }

    /// Plural noun for the symbol, used throughout the report.
    pub fn plural(self) -> &'static str {
        match self {
            Basis::Characters => "characters",
            Basis::Bytes => "bytes",
            Basis::Words => "words",
            Basis::Lines => "lines",
        }
    }

    /// Singular noun for the symbol ("bits per character").
    pub fn singular(self) -> &'static str {
        match self {
            Basis::Characters => "character",
            Basis::Bytes => "byte",
            Basis::Words => "word",
            Basis::Lines => "line",
        }
    }

    /// Canonical wire value.
    pub fn name(self) -> &'static str {
        self.plural()
    }
}

/// Logarithm base the entropy is reported in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Base 2 — bits (shannons).
    Bits,
    /// Base e — nats.
    Nats,
    /// Base 10 — dits (hartleys/bans).
    Dits,
    /// Base 3 — trits.
    Trits,
}

impl Unit {
    /// Parse the canonical wire value.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "bits" | "bit" | "shannons" => Ok(Unit::Bits),
            "nats" | "nat" => Ok(Unit::Nats),
            "dits" | "dit" | "hartleys" | "bans" => Ok(Unit::Dits),
            "trits" | "trit" => Ok(Unit::Trits),
            other => Err(format!(
                "unknown unit '{other}'; expected one of: bits, nats, dits, trits"
            )),
        }
    }

    /// Plural noun for the unit.
    pub fn plural(self) -> &'static str {
        match self {
            Unit::Bits => "bits",
            Unit::Nats => "nats",
            Unit::Dits => "dits",
            Unit::Trits => "trits",
        }
    }

    /// Canonical wire value.
    pub fn name(self) -> &'static str {
        self.plural()
    }

    /// The numeric logarithm base.
    pub fn base(self) -> f64 {
        match self {
            Unit::Bits => 2.0,
            Unit::Nats => std::f64::consts::E,
            Unit::Dits => 10.0,
            Unit::Trits => 3.0,
        }
    }

    /// `log2(base)` — the divisor that converts a bits value into this unit.
    fn log2_base(self) -> f64 {
        match self {
            Unit::Bits => 1.0,
            _ => self.base().log2(),
        }
    }

    /// How the base is written in the "maximum entropy" explanation.
    fn log_label(self) -> &'static str {
        match self {
            Unit::Bits => "log2",
            Unit::Nats => "ln",
            Unit::Dits => "log10",
            Unit::Trits => "log3",
        }
    }
}

/// Whether the text is scored as one sequence or split first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// One entropy for the whole input.
    Whole,
    /// One entropy per line, plus the combined figure.
    Line,
    /// One entropy per blank-line-separated paragraph, plus the combined figure.
    Paragraph,
}

impl Scope {
    /// Parse the canonical wire value.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "whole" | "all" | "text" => Ok(Scope::Whole),
            "line" | "lines" => Ok(Scope::Line),
            "paragraph" | "paragraphs" => Ok(Scope::Paragraph),
            other => Err(format!(
                "unknown scope '{other}'; expected one of: whole, line, paragraph"
            )),
        }
    }

    /// Canonical wire value.
    pub fn name(self) -> &'static str {
        match self {
            Scope::Whole => "whole",
            Scope::Line => "line",
            Scope::Paragraph => "paragraph",
        }
    }

    /// Label prefix used for each part in the report.
    fn part_label(self) -> &'static str {
        match self {
            Scope::Whole => "Text",
            Scope::Line => "Line",
            Scope::Paragraph => "Paragraph",
        }
    }
}

/// Everything the caller can tune.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// What counts as one symbol.
    pub basis: Basis,
    /// Logarithm base / unit of the entropy figures.
    pub unit: Unit,
    /// Whole text, or per line / per paragraph.
    pub scope: Scope,
    /// Fold upper- and lower-case together before counting.
    pub ignore_case: bool,
    /// Drop whitespace symbols (blank lines when `basis = lines`).
    pub ignore_whitespace: bool,
    /// Decimal places for every non-integer figure (0–10).
    pub precision: usize,
    /// Include the symbol-frequency table.
    pub show_frequencies: bool,
    /// How many rows the frequency table shows (0–64).
    pub top_symbols: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            basis: Basis::Characters,
            unit: Unit::Bits,
            scope: Scope::Whole,
            ignore_case: false,
            ignore_whitespace: false,
            precision: DEFAULT_PRECISION,
            show_frequencies: true,
            top_symbols: DEFAULT_TOP_SYMBOLS,
        }
    }
}

/// One row of the symbol-frequency table.
#[derive(Debug, Clone, PartialEq)]
pub struct Frequency {
    /// Printable form of the symbol (control characters are escaped).
    pub symbol: String,
    /// Number of occurrences.
    pub count: usize,
    /// Share of all symbols, in percent.
    pub share_percent: f64,
}

/// The entropy figures for one symbol sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    /// Total symbols counted.
    pub symbols: usize,
    /// Distinct symbol values seen.
    pub distinct_symbols: usize,
    /// Shannon entropy per symbol, in the requested unit.
    pub entropy: f64,
    /// `entropy × symbols` — the information content of the whole sequence.
    pub total_information: f64,
    /// `log_b(distinct_symbols)` — the entropy a uniform distribution over the
    /// same alphabet would have.
    pub max_entropy: f64,
    /// `entropy / max_entropy × 100`; 100 by convention when there is at most
    /// one distinct symbol (the distribution is trivially uniform).
    pub efficiency_percent: f64,
    /// `100 − efficiency_percent`.
    pub redundancy_percent: f64,
    /// `base^entropy` — the effective number of equally likely symbols.
    pub perplexity: f64,
    /// Frequency table rows (already truncated to `top_symbols`).
    pub frequencies: Vec<Frequency>,
    /// Distinct symbols omitted from `frequencies` by the `top_symbols` cap.
    pub frequencies_omitted: usize,
}

/// One line/paragraph in `scope = line|paragraph`.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// Human label, e.g. `Line 3`.
    pub label: String,
    /// The figures for this part.
    pub analysis: Analysis,
}

/// The full result: the combined analysis plus any per-part breakdown.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The options the report was produced with.
    pub options: Options,
    /// Figures over the whole input (always present).
    pub overall: Analysis,
    /// Per-line / per-paragraph figures; empty when `scope = whole`.
    pub parts: Vec<Part>,
}

/// Round `x` to `p` decimal places, avoiding `-0`.
fn round_to(x: f64, p: usize) -> f64 {
    let f = 10f64.powi(p as i32);
    let r = (x * f).round() / f;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// Printable form of a character: control characters and other invisibles are
/// escaped so the frequency table stays unambiguous.
fn show_char(c: char) -> String {
    match c {
        '\n' => "'\\n'".into(),
        '\r' => "'\\r'".into(),
        '\t' => "'\\t'".into(),
        ' ' => "' '".into(),
        c if (c as u32) < 0x20 || c == '\u{7f}' => format!("'\\u{{{:02x}}}'", c as u32),
        c => format!("'{c}'"),
    }
}

/// Printable form of a word/line symbol: quoted and length-limited.
fn show_text(s: &str) -> String {
    const MAX: usize = 24;
    let mut out = String::from("\"");
    for (i, c) in s.chars().enumerate() {
        if i == MAX {
            out.push('…');
            break;
        }
        match c {
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '"' => out.push_str("\\\""),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{{{:02x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Split `text` into the symbols selected by `opts`, in printable form.
fn symbols_of(text: &str, opts: &Options) -> Vec<String> {
    let folded;
    let text = if opts.ignore_case {
        folded = text.to_lowercase();
        folded.as_str()
    } else {
        text
    };
    match opts.basis {
        Basis::Characters => text
            .chars()
            .filter(|c| !(opts.ignore_whitespace && c.is_whitespace()))
            .map(show_char)
            .collect(),
        Basis::Bytes => text
            .as_bytes()
            .iter()
            .filter(|b| !(opts.ignore_whitespace && b.is_ascii_whitespace()))
            .map(|b| format!("0x{b:02x}"))
            .collect(),
        Basis::Words => text.split_whitespace().map(show_text).collect(),
        Basis::Lines => text
            .lines()
            .filter(|l| !(opts.ignore_whitespace && l.trim().is_empty()))
            .map(show_text)
            .collect(),
    }
}

/// Compute the entropy figures for an already-extracted symbol sequence.
fn analyze_symbols(symbols: &[String], opts: &Options) -> Analysis {
    let n = symbols.len();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for s in symbols {
        *counts.entry(s.as_str()).or_insert(0) += 1;
    }
    let distinct = counts.len();

    // Shannon entropy in bits, then converted into the requested unit.
    let mut h2 = 0.0f64;
    if n > 0 {
        let len = n as f64;
        for &c in counts.values() {
            let p = c as f64 / len;
            h2 -= p * p.log2();
        }
    }
    if h2 < 0.0 {
        h2 = 0.0; // floating-point noise around an exactly-uniform single symbol
    }
    let entropy = h2 / opts.unit.log2_base();
    let max_entropy = if distinct > 1 {
        (distinct as f64).log2() / opts.unit.log2_base()
    } else {
        0.0
    };
    let efficiency = if max_entropy > 0.0 {
        (entropy / max_entropy * 100.0).min(100.0)
    } else {
        100.0
    };
    let perplexity = if n > 0 { h2.exp2() } else { 0.0 };

    let mut rows: Vec<(&str, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let shown = if opts.show_frequencies {
        opts.top_symbols.min(rows.len())
    } else {
        0
    };
    let frequencies = rows
        .iter()
        .take(shown)
        .map(|(s, c)| Frequency {
            symbol: (*s).to_string(),
            count: *c,
            share_percent: round_to(*c as f64 / n.max(1) as f64 * 100.0, opts.precision),
        })
        .collect();

    Analysis {
        symbols: n,
        distinct_symbols: distinct,
        entropy: round_to(entropy, opts.precision),
        total_information: round_to(entropy * n as f64, opts.precision),
        max_entropy: round_to(max_entropy, opts.precision),
        efficiency_percent: round_to(efficiency, opts.precision),
        redundancy_percent: round_to(100.0 - efficiency, opts.precision),
        perplexity: round_to(perplexity, opts.precision),
        frequencies,
        frequencies_omitted: rows.len().saturating_sub(shown),
    }
}

/// Split `text` into paragraphs on runs of blank (whitespace-only) lines.
fn paragraphs(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                out.push(cur.join("\n"));
                cur.clear();
            }
        } else {
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        out.push(cur.join("\n"));
    }
    out
}

/// Analyze `text` under `opts`.
///
/// Errors when the input is empty, exceeds [`MAX_INPUT_BYTES`], splits into more
/// than [`MAX_PARTS`] parts, or leaves no symbols once `ignore_whitespace` has
/// been applied.
pub fn analyze(text: &str, opts: &Options) -> Result<Report, String> {
    if text.is_empty() {
        return Err("text is empty — paste the string, key, or passage to measure".into());
    }
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the maximum is {} bytes (1 MiB)",
            text.len(),
            MAX_INPUT_BYTES
        ));
    }
    if opts.precision > MAX_PRECISION {
        return Err(format!(
            "precision {} is out of range; expected 0 to {MAX_PRECISION}",
            opts.precision
        ));
    }
    if opts.top_symbols > MAX_TOP_SYMBOLS {
        return Err(format!(
            "top_symbols {} is out of range; expected 0 to {MAX_TOP_SYMBOLS}",
            opts.top_symbols
        ));
    }

    let all = symbols_of(text, opts);
    if all.is_empty() {
        return Err(format!(
            "no {} left to measure after filtering — turn off \"ignore whitespace\" or paste more text",
            opts.basis.plural()
        ));
    }
    let overall = analyze_symbols(&all, opts);

    let chunks: Vec<String> = match opts.scope {
        Scope::Whole => Vec::new(),
        Scope::Line => text.lines().map(|l| l.to_string()).collect(),
        Scope::Paragraph => paragraphs(text),
    };
    if chunks.len() > MAX_PARTS {
        return Err(format!(
            "input has {} {}s; the maximum is {MAX_PARTS}",
            chunks.len(),
            opts.scope.name()
        ));
    }
    let parts = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| Part {
            label: format!("{} {}", opts.scope.part_label(), i + 1),
            analysis: analyze_symbols(&symbols_of(chunk, opts), opts),
        })
        .collect();

    Ok(Report {
        options: opts.clone(),
        overall,
        parts,
    })
}

/// Render one analysis block (the headline figures + optional table).
fn render_analysis(out: &mut String, a: &Analysis, opts: &Options) {
    let p = opts.precision;
    let unit = opts.unit.plural();
    let sym = opts.basis.singular();
    let syms = opts.basis.plural();
    let _ = writeln!(
        out,
        "Shannon entropy: {:.p$} {unit} per {sym}",
        a.entropy,
        p = p
    );
    let _ = writeln!(
        out,
        "Total information: {:.p$} {unit} over {} {syms}",
        a.total_information,
        a.symbols,
        p = p
    );
    let _ = writeln!(out, "Distinct {syms}: {}", a.distinct_symbols);
    let _ = writeln!(
        out,
        "Maximum entropy: {:.p$} {unit} per {sym} ({} of {})",
        a.max_entropy,
        opts.unit.log_label(),
        a.distinct_symbols,
        p = p
    );
    let _ = writeln!(out, "Efficiency: {:.p$}%", a.efficiency_percent, p = p);
    let _ = writeln!(out, "Redundancy: {:.p$}%", a.redundancy_percent, p = p);
    let _ = writeln!(out, "Perplexity: {:.p$}", a.perplexity, p = p);

    if a.frequencies.is_empty() {
        return;
    }
    let _ = writeln!(out);
    let shown = a.frequencies.len();
    let _ = writeln!(
        out,
        "Symbol frequencies (top {shown} of {}):",
        a.distinct_symbols
    );
    let sp = p.min(2);
    let shares: Vec<String> = a
        .frequencies
        .iter()
        .map(|f| format!("{:.sp$}%", f.share_percent, sp = sp))
        .collect();
    let width = a
        .frequencies
        .iter()
        .map(|f| f.symbol.chars().count())
        .max()
        .unwrap_or(1);
    let max_count = a
        .frequencies
        .iter()
        .map(|f| f.count)
        .max()
        .unwrap_or(1)
        .max(1);
    let count_width = a
        .frequencies
        .iter()
        .map(|f| f.count.to_string().len())
        .max()
        .unwrap_or(1);
    let share_width = shares.iter().map(|s| s.len()).max().unwrap_or(1);
    for (f, share) in a.frequencies.iter().zip(&shares) {
        let pad = width - f.symbol.chars().count();
        let bars = ((f.count as f64 / max_count as f64) * BAR_WIDTH as f64).round() as usize;
        let _ = writeln!(
            out,
            "  {}{}  {:>cw$}  {:>sw$}  {}",
            f.symbol,
            " ".repeat(pad),
            f.count,
            share,
            "#".repeat(bars.max(1)),
            cw = count_width,
            sw = share_width,
        );
    }
    if a.frequencies_omitted > 0 {
        let _ = writeln!(
            out,
            "  … {} more distinct {} not shown",
            a.frequencies_omitted,
            opts.basis.plural()
        );
    }
}

/// Render a report as the plain-text page/CLI output.
pub fn render(report: &Report) -> String {
    let opts = &report.options;
    let p = opts.precision;
    let mut out = String::new();
    if !report.parts.is_empty() {
        for part in &report.parts {
            let a = &part.analysis;
            if a.symbols == 0 {
                let _ = writeln!(out, "{}: empty", part.label);
            } else {
                let _ = writeln!(
                    out,
                    "{}: {:.p$} {} per {} ({} {}, {} distinct, {:.p$} {} total)",
                    part.label,
                    a.entropy,
                    opts.unit.plural(),
                    opts.basis.singular(),
                    a.symbols,
                    opts.basis.plural(),
                    a.distinct_symbols,
                    a.total_information,
                    opts.unit.plural(),
                    p = p
                );
            }
        }
        let _ = writeln!(out);
        let _ = writeln!(out, "Combined:");
    }
    render_analysis(&mut out, &report.overall, opts);
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Convenience wrapper used by the web/CLI surfaces: analyze and render in one
/// call.
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    basis: &str,
    unit: &str,
    scope: &str,
    ignore_case: bool,
    ignore_whitespace: bool,
    precision: usize,
    show_frequencies: bool,
    top_symbols: usize,
) -> Result<String, String> {
    let opts = Options {
        basis: Basis::parse(basis)?,
        unit: Unit::parse(unit)?,
        scope: Scope::parse(scope)?,
        ignore_case,
        ignore_whitespace,
        precision,
        show_frequencies,
        top_symbols,
    };
    Ok(render(&analyze(text, &opts)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn password_has_exactly_2_75_bits_per_character() {
        // 's' twice + 6 singles over 8 characters: -(1/4·log2 1/4 + 6·1/8·log2 1/8) = 2.75
        let r = analyze("password", &opts()).unwrap();
        assert_eq!(r.overall.entropy, 2.75);
        assert_eq!(r.overall.total_information, 22.0);
        assert_eq!(r.overall.symbols, 8);
        assert_eq!(r.overall.distinct_symbols, 7);
        assert_eq!(r.overall.perplexity, 6.7272);
        assert!(r.parts.is_empty());
    }

    #[test]
    fn uniform_alphabet_is_fully_efficient() {
        let r = analyze("abcd", &opts()).unwrap();
        assert_eq!(r.overall.entropy, 2.0);
        assert_eq!(r.overall.max_entropy, 2.0);
        assert_eq!(r.overall.efficiency_percent, 100.0);
        assert_eq!(r.overall.redundancy_percent, 0.0);
        assert_eq!(r.overall.perplexity, 4.0);
    }

    #[test]
    fn single_repeated_symbol_has_zero_entropy() {
        let r = analyze("aaaaaaaa", &opts()).unwrap();
        assert_eq!(r.overall.entropy, 0.0);
        assert_eq!(r.overall.total_information, 0.0);
        assert_eq!(r.overall.distinct_symbols, 1);
        // Trivially uniform: max entropy 0, efficiency 100% by convention.
        assert_eq!(r.overall.efficiency_percent, 100.0);
        assert_eq!(r.overall.perplexity, 1.0);
    }

    #[test]
    fn units_convert_from_bits() {
        let mut o = opts();
        o.unit = Unit::Nats;
        assert_eq!(analyze("abcd", &o).unwrap().overall.entropy, 1.3863);
        o.unit = Unit::Dits;
        assert_eq!(analyze("abcd", &o).unwrap().overall.entropy, 0.6021);
        o.unit = Unit::Trits;
        assert_eq!(analyze("abcd", &o).unwrap().overall.entropy, 1.2619);
        // Perplexity is base-invariant: 3^1.2619 ≈ 4.
        assert_eq!(analyze("abcd", &o).unwrap().overall.perplexity, 4.0);
    }

    #[test]
    fn bytes_basis_counts_utf8_bytes() {
        // "é" is 2 UTF-8 bytes but 1 character.
        let chars = analyze("é", &opts()).unwrap();
        assert_eq!(chars.overall.symbols, 1);
        let mut o = opts();
        o.basis = Basis::Bytes;
        let bytes = analyze("é", &o).unwrap();
        assert_eq!(bytes.overall.symbols, 2);
        assert_eq!(bytes.overall.entropy, 1.0);
    }

    #[test]
    fn words_and_lines_bases() {
        let mut o = opts();
        o.basis = Basis::Words;
        let r = analyze("red green red blue", &o).unwrap();
        assert_eq!(r.overall.symbols, 4);
        assert_eq!(r.overall.distinct_symbols, 3);
        assert_eq!(r.overall.entropy, 1.5);

        o.basis = Basis::Lines;
        let r = analyze("ok\nok\nfail\nfail", &o).unwrap();
        assert_eq!(r.overall.symbols, 4);
        assert_eq!(r.overall.entropy, 1.0);
    }

    #[test]
    fn ignore_case_and_whitespace_change_the_alphabet() {
        let mut o = opts();
        o.ignore_case = true;
        let r = analyze("AaAa", &o).unwrap();
        assert_eq!(r.overall.distinct_symbols, 1);
        assert_eq!(r.overall.entropy, 0.0);

        let mut o = opts();
        o.ignore_whitespace = true;
        let r = analyze("a b\tc\n", &o).unwrap();
        assert_eq!(r.overall.symbols, 3);
        assert_eq!(r.overall.entropy, 1.585);
    }

    #[test]
    fn per_line_scope_lists_each_line_plus_combined() {
        let mut o = opts();
        o.scope = Scope::Line;
        let r = analyze("aaaa\nabcd", &o).unwrap();
        assert_eq!(r.parts.len(), 2);
        assert_eq!(r.parts[0].label, "Line 1");
        assert_eq!(r.parts[0].analysis.entropy, 0.0);
        assert_eq!(r.parts[1].analysis.entropy, 2.0);
        // Combined includes the newline symbol.
        assert_eq!(r.overall.symbols, 9);
    }

    #[test]
    fn paragraph_scope_splits_on_blank_lines() {
        let mut o = opts();
        o.scope = Scope::Paragraph;
        let r = analyze("aa\nbb\n\n\ncd", &o).unwrap();
        assert_eq!(r.parts.len(), 2);
        assert_eq!(r.parts[1].label, "Paragraph 2");
        assert_eq!(r.parts[1].analysis.symbols, 2);
    }

    #[test]
    fn frequency_table_is_capped_and_ordered() {
        let mut o = opts();
        o.top_symbols = 2;
        let r = analyze("aaabbc", &o).unwrap();
        assert_eq!(r.overall.frequencies.len(), 2);
        assert_eq!(r.overall.frequencies[0].symbol, "'a'");
        assert_eq!(r.overall.frequencies[0].count, 3);
        assert_eq!(r.overall.frequencies[1].symbol, "'b'");
        assert_eq!(r.overall.frequencies_omitted, 1);

        o.show_frequencies = false;
        let r = analyze("aaabbc", &o).unwrap();
        assert!(r.overall.frequencies.is_empty());
    }

    #[test]
    fn control_characters_are_escaped_in_the_table() {
        let r = analyze("a\nb\t", &opts()).unwrap();
        let shown: Vec<&str> = r
            .overall
            .frequencies
            .iter()
            .map(|f| f.symbol.as_str())
            .collect();
        assert!(shown.contains(&"'\\n'"), "{shown:?}");
        assert!(shown.contains(&"'\\t'"), "{shown:?}");
    }

    #[test]
    fn rendered_report_is_exact() {
        let mut o = opts();
        o.top_symbols = 3;
        let text = render(&analyze("password", &o).unwrap());
        assert_eq!(
            text,
            "Shannon entropy: 2.7500 bits per character\n\
             Total information: 22.0000 bits over 8 characters\n\
             Distinct characters: 7\n\
             Maximum entropy: 2.8074 bits per character (log2 of 7)\n\
             Efficiency: 97.9570%\n\
             Redundancy: 2.0430%\n\
             Perplexity: 6.7272\n\
             \n\
             Symbol frequencies (top 3 of 7):\n\
             \x20 's'  2  25.00%  ####################\n\
             \x20 'a'  1  12.50%  ##########\n\
             \x20 'd'  1  12.50%  ##########\n\
             \x20 … 4 more distinct characters not shown"
        );
    }

    #[test]
    fn empty_text_is_an_error() {
        let err = analyze("", &opts()).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn filtering_everything_away_is_an_error() {
        let mut o = opts();
        o.ignore_whitespace = true;
        let err = analyze("   \n\t ", &o).unwrap_err();
        assert!(err.contains("no characters left"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        let err = analyze(&big, &opts()).unwrap_err();
        assert!(err.contains("1048576"), "{err}");
        // Exactly at the cap is accepted.
        let at_cap = "a".repeat(MAX_INPUT_BYTES);
        assert!(analyze(&at_cap, &opts()).is_ok());
    }

    #[test]
    fn out_of_range_knobs_are_rejected() {
        let mut o = opts();
        o.precision = MAX_PRECISION + 1;
        assert!(analyze("abc", &o).unwrap_err().contains("precision"));
        let mut o = opts();
        o.top_symbols = MAX_TOP_SYMBOLS + 1;
        assert!(analyze("abc", &o).unwrap_err().contains("top_symbols"));
    }

    #[test]
    fn unknown_enum_values_are_rejected_by_name() {
        assert!(Basis::parse("glyphs")
            .unwrap_err()
            .contains("characters, bytes, words, lines"));
        assert!(Unit::parse("bytes")
            .unwrap_err()
            .contains("bits, nats, dits, trits"));
        assert!(Scope::parse("page")
            .unwrap_err()
            .contains("whole, line, paragraph"));
    }

    #[test]
    fn run_wrapper_round_trips_string_args() {
        let out = run(
            "abcd",
            "characters",
            "bits",
            "whole",
            false,
            false,
            2,
            false,
            12,
        )
        .unwrap();
        assert!(
            out.starts_with("Shannon entropy: 2.00 bits per character\n"),
            "{out}"
        );
        assert!(run("abcd", "glyphs", "bits", "whole", false, false, 2, false, 12).is_err());
    }
}
