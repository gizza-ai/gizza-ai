//! index-of-coincidence core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! The Index of Coincidence (IC) is the probability that two letters drawn at
//! random (without replacement) from a text are the same. For a text with letter
//! counts `n_i` over an alphabet of size `c` and total letter count `N`:
//!
//! ```text
//! IC_raw        = Σ n_i (n_i - 1) / (N (N - 1))
//! IC_normalized = c * IC_raw
//! ```
//!
//! The *normalized* form (kappa-plaintext) is what most references quote: it is
//! ~1.73 for English plaintext, ~1.0 for random/uniform text, and drops toward
//! 1.0 as a polyalphabetic cipher (Vigenère, etc.) flattens the distribution. The
//! raw form is ~0.0667 for English, ~0.0385 for random over 26 letters.
//!
//! Only the 26 Latin letters `A`-`Z` are counted (case-folded); all other
//! characters (digits, spaces, punctuation, non-Latin) are ignored. This is the
//! classical cryptanalytic convention.

/// Size of the alphabet the IC is computed over (the 26 Latin letters).
pub const ALPHABET_SIZE: usize = 26;

/// Expected normalized IC of English plaintext (≈1.73; raw ≈0.0667).
pub const ENGLISH_IC_NORMALIZED: f64 = 1.73;

/// Maximum candidate key length `analyze` will examine for period estimation.
pub const MAX_PERIOD: u32 = 40;

/// A computed Index-of-Coincidence report for a text.
#[derive(Debug, Clone, PartialEq)]
pub struct IcReport {
    /// Number of letters (A-Z, case-folded) that were counted.
    pub letter_count: usize,
    /// Raw IC: Σ n_i(n_i-1) / (N(N-1)). ≈0.0667 English, ≈0.0385 random/26.
    pub raw: f64,
    /// Normalized IC = ALPHABET_SIZE * raw. ≈1.73 English, ≈1.0 random.
    pub normalized: f64,
    /// Per-letter counts in A..=Z order (index 0 = 'A').
    pub counts: [u32; ALPHABET_SIZE],
    /// Optional per-period analysis (key-length estimation), one entry per
    /// candidate period `1..=max_period`, each its average column IC (normalized).
    pub period_ics: Vec<PeriodIc>,
}

/// Average column Index of Coincidence for one candidate key length.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodIc {
    /// Candidate key length / period (≥1).
    pub period: u32,
    /// Mean of the per-column normalized IC over the `period` columns.
    pub avg_normalized_ic: f64,
}

/// Fold a `char` to a letter index `0..26` if it is an ASCII letter, else `None`.
fn letter_index(ch: char) -> Option<usize> {
    if ch.is_ascii_alphabetic() {
        Some((ch.to_ascii_uppercase() as u8 - b'A') as usize)
    } else {
        None
    }
}

/// Collect the A..=Z case-folded letters of `text` as letter indices `0..26`.
fn letters(text: &str) -> Vec<usize> {
    text.chars().filter_map(letter_index).collect()
}

/// Raw IC of a slice of letter indices. `< 2` letters → 0.0 (undefined, reported
/// as zero so the surfaces never divide by zero).
fn raw_ic(indices: &[usize]) -> f64 {
    let n = indices.len();
    if n < 2 {
        return 0.0;
    }
    let mut counts = [0u64; ALPHABET_SIZE];
    for &i in indices {
        counts[i] += 1;
    }
    let numerator: u64 = counts.iter().map(|&c| c * c.saturating_sub(1)).sum();
    numerator as f64 / (n as f64 * (n as f64 - 1.0))
}

/// Compute the Index of Coincidence report for `text`.
///
/// - `max_period`: when `> 0`, also estimate the polyalphabetic key length by
///   splitting the letters into `p` columns (every `p`-th letter) for each
///   `p in 1..=max_period.min(MAX_PERIOD)` and averaging the per-column IC. The
///   period whose average normalized IC is closest to the English value (~1.73)
///   is the likely key length. `0` skips this analysis.
///
/// Returns `Err` only when `text` contains no letters at all (IC undefined).
pub fn analyze(text: &str, max_period: u32) -> Result<IcReport, String> {
    let indices = letters(text);
    if indices.is_empty() {
        return Err("input contains no letters (A-Z) to analyze".to_string());
    }

    let mut counts = [0u32; ALPHABET_SIZE];
    for &i in &indices {
        counts[i] += 1;
    }

    let raw = raw_ic(&indices);
    let normalized = ALPHABET_SIZE as f64 * raw;

    let mut period_ics = Vec::new();
    if max_period > 0 {
        let cap = max_period.min(MAX_PERIOD);
        for p in 1..=cap {
            let pu = p as usize;
            // Build the `p` columns (letters at positions ≡ col mod p).
            let mut sum = 0.0;
            for col in 0..pu {
                let column: Vec<usize> = indices.iter().skip(col).step_by(pu).copied().collect();
                sum += ALPHABET_SIZE as f64 * raw_ic(&column);
            }
            period_ics.push(PeriodIc {
                period: p,
                avg_normalized_ic: sum / pu as f64,
            });
        }
    }

    Ok(IcReport {
        letter_count: indices.len(),
        raw,
        normalized,
        counts,
        period_ics,
    })
}

/// Friedman's key-length estimate for a Vigenère-style polyalphabetic cipher,
/// from the raw IC and letter count `n`. Uses the classic English reference
/// constants — expected IC of English plaintext (0.0665) and of uniform random
/// text over 26 letters (0.0385 = 1/26):
///
/// ```text
/// K ≈ 0.0265·n / ((0.0665 − IC_raw) + n·(IC_raw − 0.0385))
/// ```
///
/// Returns `None` when the denominator is non-positive (e.g. plaintext-level IC,
/// where the formula degenerates and the answer is effectively 1) or `n < 2`.
pub fn friedman_key_length(raw: f64, n: usize) -> Option<f64> {
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let denom = (0.0665 - raw) + nf * (raw - 0.0385);
    if denom <= 0.0 {
        return None;
    }
    Some(0.0265 * nf / denom)
}

/// Human-readable interpretation of a normalized IC value.
pub fn interpret(normalized: f64) -> &'static str {
    if normalized >= 1.6 {
        "monoalphabetic / plaintext-like (English plaintext ≈ 1.73; a simple substitution or transposition preserves this)"
    } else if normalized >= 1.2 {
        "intermediate — likely a short-key polyalphabetic cipher, or a non-English / mixed-language plaintext"
    } else {
        "polyalphabetic / random-like (≈ 1.0 means a flat distribution: a long-key Vigenère, a one-time pad, or already-random data)"
    }
}

/// Render a full plain-text report for the chat block and the page.
pub fn report(text: &str, max_period: u32, show_counts: bool) -> Result<String, String> {
    let r = analyze(text, max_period)?;
    let mut out = String::new();
    out.push_str(&format!("Letters analyzed: {}\n", r.letter_count));
    out.push_str(&format!("Index of Coincidence (normalized): {:.4}\n", r.normalized));
    out.push_str(&format!("Index of Coincidence (raw):        {:.5}\n", r.raw));
    out.push_str(&format!("Interpretation: {}\n", interpret(r.normalized)));

    if let Some(k) = friedman_key_length(r.raw, r.letter_count) {
        // Only meaningful when the IC is below plaintext level (k > ~1.5).
        if k >= 1.5 {
            out.push_str(&format!(
                "Friedman key-length estimate: {:.1} (assumes English; rounds to the nearest whole number)\n",
                k
            ));
        }
    }

    if show_counts {
        out.push_str("\nLetter frequencies:\n");
        for (i, &c) in r.counts.iter().enumerate() {
            if c > 0 {
                let pct = 100.0 * c as f64 / r.letter_count as f64;
                out.push_str(&format!(
                    "  {}: {:>5}  ({:.2}%)\n",
                    (b'A' + i as u8) as char,
                    c,
                    pct
                ));
            }
        }
    }

    if !r.period_ics.is_empty() {
        out.push_str("\nKey-length (period) estimation — average column IC, normalized:\n");
        for p in &r.period_ics {
            out.push_str(&format!("  period {:>2}: {:.4}\n", p.period, p.avg_normalized_ic));
        }
        // Best period > 1 whose avg column IC is highest (closest to plaintext).
        if let Some(best) = r
            .period_ics
            .iter()
            .filter(|p| p.period > 1)
            .max_by(|a, b| {
                a.avg_normalized_ic
                    .partial_cmp(&b.avg_normalized_ic)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            out.push_str(&format!(
                "Likely key length: {} (highest average column IC = {:.4})\n",
                best.period, best.avg_normalized_ic
            ));
        }
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_plaintext_is_near_173() {
        // A chunk of English prose should land near 1.73 normalized.
        let text = "The Index of Coincidence is a statistic used in cryptanalysis to \
            measure the unevenness of a letter distribution, helping to distinguish \
            monoalphabetic ciphers from polyalphabetic ones and to estimate key length.";
        let r = analyze(text, 0).unwrap();
        assert!(r.normalized > 1.4, "expected English-like IC, got {}", r.normalized);
        // raw == normalized / 26
        assert!((r.raw * ALPHABET_SIZE as f64 - r.normalized).abs() < 1e-9);
    }

    #[test]
    fn uniform_text_matches_formula() {
        // Each of the 26 letters exactly twice → N=52, every n_i=2.
        // raw = 26*(2*1) / (52*51) = 52/2652 ; normalized = 26*raw.
        // (The expected ≈1.0 for "random" is the N→∞ limit; finite uniform text
        // sits below it because of the (N-1) without-replacement correction.)
        let text: String = ('A'..='Z').flat_map(|c| [c, c]).collect();
        let r = analyze(&text, 0).unwrap();
        assert_eq!(r.letter_count, 52);
        let expected_raw = 52.0 / (52.0 * 51.0);
        assert!((r.raw - expected_raw).abs() < 1e-12, "got {}", r.raw);
        assert!((r.normalized - 26.0 * expected_raw).abs() < 1e-12);
        // A flat distribution is well below the plaintext value.
        assert!(r.normalized < 1.0);
    }

    #[test]
    fn all_same_letter_is_max_ic() {
        // "AAAA" → every pair matches → raw IC == 1.0, normalized == 26.
        let r = analyze("AAAA", 0).unwrap();
        assert!((r.raw - 1.0).abs() < 1e-9);
        assert!((r.normalized - 26.0).abs() < 1e-9);
    }

    #[test]
    fn ignores_non_letters_and_case() {
        let r = analyze("a, A; 1 2 3 — a!", 0).unwrap();
        assert_eq!(r.letter_count, 3); // three 'a'/'A'
        assert_eq!(r.counts[0], 3);
    }

    #[test]
    fn period_estimation_recovers_key_length() {
        // Vigenère with key "KEY" (period 3) over repeated English-ish text.
        let pt = "WEAREDISCOVEREDSAVEYOURSELFWEAREDISCOVEREDSAVEYOURSELF";
        let key = "KEY";
        let ct: String = pt
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .enumerate()
            .map(|(i, c)| {
                let p = c.to_ascii_uppercase() as u8 - b'A';
                let k = key.as_bytes()[i % key.len()] - b'A';
                (b'A' + (p + k) % 26) as char
            })
            .collect();
        let r = analyze(&ct, 9).unwrap();
        let best = r
            .period_ics
            .iter()
            .filter(|p| p.period > 1)
            .max_by(|a, b| a.avg_normalized_ic.partial_cmp(&b.avg_normalized_ic).unwrap())
            .unwrap();
        // Best period should be a multiple of 3 (3, 6, or 9).
        assert_eq!(best.period % 3, 0, "best period was {}", best.period);
    }

    #[test]
    fn empty_or_no_letters_errors() {
        assert!(analyze("", 0).is_err());
        assert!(analyze("12345 !!! ???", 0).is_err());
    }

    #[test]
    fn single_letter_has_zero_ic() {
        // Undefined (N<2) → reported as 0, not a panic.
        let r = analyze("A", 0).unwrap();
        assert_eq!(r.letter_count, 1);
        assert_eq!(r.raw, 0.0);
    }

    #[test]
    fn report_includes_counts_when_requested() {
        let out = report("hello world", 0, true).unwrap();
        assert!(out.contains("Index of Coincidence"));
        assert!(out.contains("L:")); // 'L' appears 3x in "hello world"
    }

    #[test]
    fn friedman_estimate_tracks_key_length() {
        // Plaintext-level IC → estimate ≈ 1 (single alphabet).
        let k1 = friedman_key_length(0.0667, 200).unwrap();
        assert!((k1 - 1.0).abs() < 0.15, "plaintext IC should give ~1, got {}", k1);
        // Lower IC (more polyalphabetic) → larger estimated key.
        let k2 = friedman_key_length(0.045, 500).unwrap();
        assert!(k2 > k1 + 1.0, "lower IC should give a longer key: {} vs {}", k2, k1);
        // n < 2 → undefined.
        assert!(friedman_key_length(0.05, 1).is_none());
    }

    #[test]
    fn interpret_buckets() {
        assert!(interpret(1.73).contains("monoalphabetic"));
        assert!(interpret(1.3).contains("intermediate"));
        assert!(interpret(1.0).contains("polyalphabetic"));
    }
}
