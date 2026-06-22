//! gizza-ai/frequency-distribution core — build a frequency table over the
//! symbols of an input: Unicode characters, raw bytes, or fixed-length n-grams.
//! Each distinct symbol is reported with its count and percentage of the total,
//! ranked most→least frequent. Input is either plain text or a hex string.
//! Pure-Rust, dependency-free (besides serde for the output struct).

use std::collections::HashMap;

use serde::Serialize;

/// What symbol to tally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Unicode scalar values (chars).
    Char,
    /// Raw bytes of the input (UTF-8 bytes of text, or decoded hex bytes).
    Byte,
    /// Fixed-length character windows (overlapping), e.g. n=2 bigrams.
    Ngram(usize),
}

/// How to interpret the input string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// Treat the input literally as text.
    Text,
    /// Parse the input as a hex string (whitespace ignored) into bytes.
    Hex,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    /// Display label for the symbol (escaped for control/whitespace chars,
    /// `0xNN` for a byte, the literal n-gram window for n-grams).
    pub symbol: String,
    pub count: usize,
    /// Percentage of the total (0–100), rounded to 2 decimals.
    pub percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Distribution {
    pub entries: Vec<Entry>,
    /// Number of distinct symbols.
    pub distinct: usize,
    /// Total symbols counted (denominator for the percentages).
    pub total: usize,
}

/// Render a single character into a readable, unambiguous label.
fn label_char(c: char) -> String {
    match c {
        ' ' => "␠ (space)".to_string(),
        '\t' => "\\t (tab)".to_string(),
        '\n' => "\\n (newline)".to_string(),
        '\r' => "\\r (return)".to_string(),
        c if (c as u32) < 0x20 || (c as u32) == 0x7f => format!("\\x{:02x}", c as u32),
        c => c.to_string(),
    }
}

fn label_str(s: &str) -> String {
    // For n-gram windows: escape control/whitespace chars individually.
    s.chars()
        .map(|c| match c {
            ' ' => "␠".to_string(),
            '\t' => "\\t".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            c if (c as u32) < 0x20 || (c as u32) == 0x7f => format!("\\x{:02x}", c as u32),
            c => c.to_string(),
        })
        .collect()
}

/// Decode a hex string (whitespace and an optional `0x` prefix are ignored).
pub fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let cleaned = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned);
    if cleaned.len() % 2 != 0 {
        return Err("hex input has an odd number of digits".to_string());
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let bytes = cleaned.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex digit '{}'", b as char)),
    }
}

/// Tally a sequence of grouping keys, preserving first-seen order for stable ties.
fn tally<I>(items: I) -> (HashMap<String, usize>, Vec<String>, usize)
where
    I: IntoIterator<Item = String>,
{
    let mut map: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut total = 0usize;
    for k in items {
        let e = map.entry(k.clone()).or_insert_with(|| {
            order.push(k.clone());
            0
        });
        *e += 1;
        total += 1;
    }
    (map, order, total)
}

fn rank(map: HashMap<String, usize>, order: Vec<String>, total: usize) -> Distribution {
    let mut idx: Vec<(usize, &String)> = order.iter().enumerate().collect();
    idx.sort_by(|a, b| map[b.1].cmp(&map[a.1]).then(a.0.cmp(&b.0)));
    let entries: Vec<Entry> = idx
        .into_iter()
        .map(|(_, k)| {
            let count = map[k];
            let percent = if total == 0 {
                0.0
            } else {
                ((count as f64 / total as f64) * 10000.0).round() / 100.0
            };
            Entry {
                symbol: k.clone(),
                count,
                percent,
            }
        })
        .collect();
    let distinct = entries.len();
    Distribution {
        entries,
        distinct,
        total,
    }
}

/// Build a frequency distribution. When `case_sensitive` is false, `char`/`ngram`
/// symbols are folded to lowercase before tallying (byte mode is unaffected —
/// raw bytes have no case). Returns an error only for malformed hex or an
/// n-gram size of 0.
pub fn distribute(
    input: &str,
    kind: InputKind,
    mode: Mode,
    case_sensitive: bool,
) -> Result<Distribution, String> {
    let bytes: Vec<u8>;
    let mut text: String;
    match kind {
        InputKind::Text => {
            text = input.to_string();
            bytes = input.as_bytes().to_vec();
        }
        InputKind::Hex => {
            bytes = decode_hex(input)?;
            // For char/ngram modes over hex, interpret decoded bytes as UTF-8
            // (lossily) so the symbols are meaningful.
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
    }
    // Case folding applies to text-derived symbols only (char/ngram).
    if !case_sensitive && !matches!(mode, Mode::Byte) {
        text = text.to_lowercase();
    }

    let (map, order, total) = match mode {
        Mode::Char => tally(text.chars().map(label_char)),
        Mode::Byte => tally(bytes.iter().map(|b| format!("0x{:02x}", b))),
        Mode::Ngram(n) => {
            if n == 0 {
                return Err("n-gram size must be at least 1".to_string());
            }
            let chars: Vec<char> = text.chars().collect();
            if chars.len() < n {
                tally(Vec::<String>::new())
            } else {
                let windows: Vec<String> = (0..=chars.len() - n)
                    .map(|i| label_str(&chars[i..i + n].iter().collect::<String>()))
                    .collect();
                tally(windows)
            }
        }
    };

    Ok(rank(map, order, total))
}

/// Plain-text aligned table "count  percent%  symbol", most frequent first
/// (used by the page surface).
pub fn format_table(
    input: &str,
    kind: InputKind,
    mode: Mode,
    case_sensitive: bool,
) -> Result<String, String> {
    let d = distribute(input, kind, mode, case_sensitive)?;
    if d.entries.is_empty() {
        return Ok("(no symbols)".to_string());
    }
    let cw = d
        .entries
        .iter()
        .map(|e| e.count.to_string().len())
        .max()
        .unwrap_or(1);
    let mut lines = Vec::with_capacity(d.entries.len() + 2);
    for e in &d.entries {
        lines.push(format!(
            "{:>cw$}  {:>6.2}%  {}",
            e.count,
            e.percent,
            e.symbol,
            cw = cw
        ));
    }
    lines.push(String::new());
    lines.push(format!("{} distinct symbols, {} total", d.distinct, d.total));
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_counts_and_ranks() {
        let d = distribute("aabbbc", InputKind::Text, Mode::Char, true).unwrap();
        assert_eq!(d.total, 6);
        assert_eq!(d.distinct, 3);
        assert_eq!(d.entries[0].symbol, "b");
        assert_eq!(d.entries[0].count, 3);
        assert_eq!(d.entries[1].symbol, "a");
        assert_eq!(d.entries[1].count, 2);
    }

    #[test]
    fn percentages_round() {
        let d = distribute("aab", InputKind::Text, Mode::Char, true).unwrap();
        assert_eq!(d.entries[0].symbol, "a");
        assert_eq!(d.entries[0].percent, 66.67);
        assert_eq!(d.entries[1].percent, 33.33);
    }

    #[test]
    fn ties_keep_first_seen() {
        let d = distribute("ba", InputKind::Text, Mode::Char, true).unwrap();
        assert_eq!(d.entries[0].symbol, "b");
        assert_eq!(d.entries[1].symbol, "a");
    }

    #[test]
    fn case_insensitive_groups() {
        let d = distribute("AaBb", InputKind::Text, Mode::Char, false).unwrap();
        // folded: a,a,b,b → a:2, b:2; distinct 2.
        assert_eq!(d.distinct, 2);
        assert_eq!(d.entries[0].symbol, "a");
        assert_eq!(d.entries[0].count, 2);
        // case-sensitive keeps 4 distinct.
        let s = distribute("AaBb", InputKind::Text, Mode::Char, true).unwrap();
        assert_eq!(s.distinct, 4);
    }

    #[test]
    fn case_insensitive_ignores_byte_mode() {
        // Byte mode is raw bytes — case_sensitive=false must not change it.
        let a = distribute("Aa", InputKind::Text, Mode::Byte, false).unwrap();
        assert_eq!(a.distinct, 2); // 0x41, 0x61
    }

    #[test]
    fn whitespace_and_control_labels() {
        let d = distribute("a b\tc", InputKind::Text, Mode::Char, true).unwrap();
        let syms: Vec<&str> = d.entries.iter().map(|e| e.symbol.as_str()).collect();
        assert!(syms.contains(&"␠ (space)"));
        assert!(syms.contains(&"\\t (tab)"));
    }

    #[test]
    fn byte_mode_multibyte() {
        let bd = distribute("é", InputKind::Text, Mode::Byte, true).unwrap();
        assert_eq!(bd.total, 2);
        assert_eq!(bd.distinct, 2);
        let cd = distribute("é", InputKind::Text, Mode::Char, true).unwrap();
        assert_eq!(cd.total, 1);
        assert_eq!(cd.entries[0].symbol, "é");
    }

    #[test]
    fn ngram_bigrams() {
        let d = distribute("abab", InputKind::Text, Mode::Ngram(2), true).unwrap();
        assert_eq!(d.total, 3);
        assert_eq!(d.entries[0].symbol, "ab");
        assert_eq!(d.entries[0].count, 2);
        assert_eq!(d.entries[1].symbol, "ba");
    }

    #[test]
    fn ngram_too_long_is_empty() {
        let d = distribute("ab", InputKind::Text, Mode::Ngram(5), true).unwrap();
        assert_eq!(d.total, 0);
        assert_eq!(d.distinct, 0);
    }

    #[test]
    fn ngram_zero_errors() {
        assert!(distribute("ab", InputKind::Text, Mode::Ngram(0), true).is_err());
    }

    #[test]
    fn hex_byte_mode() {
        let d = distribute("ff00ff", InputKind::Hex, Mode::Byte, true).unwrap();
        assert_eq!(d.total, 3);
        assert_eq!(d.entries[0].symbol, "0xff");
        assert_eq!(d.entries[0].count, 2);
        assert_eq!(d.entries[1].symbol, "0x00");
    }

    #[test]
    fn hex_whitespace_and_prefix() {
        let a = decode_hex("0x de ad be ef").unwrap();
        assert_eq!(a, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn hex_odd_length_errors() {
        assert!(decode_hex("abc").is_err());
    }

    #[test]
    fn hex_invalid_digit_errors() {
        assert!(decode_hex("zz").is_err());
    }

    #[test]
    fn empty_input() {
        let d = distribute("", InputKind::Text, Mode::Char, true).unwrap();
        assert_eq!(d.total, 0);
        assert_eq!(d.distinct, 0);
    }

    #[test]
    fn table_renders() {
        let t = format_table("aab", InputKind::Text, Mode::Char, true).unwrap();
        assert!(t.contains("66.67%"));
        assert!(t.contains("2 distinct symbols, 3 total"));
    }

    #[test]
    fn table_empty() {
        let t = format_table("", InputKind::Text, Mode::Char, true).unwrap();
        assert_eq!(t, "(no symbols)");
    }
}
