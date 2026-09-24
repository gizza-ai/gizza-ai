//! caesar-cipher-breaker core — pure compute, shared by the chat skill block and
//! the web page.
//!
//! Cracks a Caesar (shift) cipher without the key: every one of the 26 possible
//! decryptions is scored against the letter frequencies of a chosen language,
//! and the best-scoring shift is reported with the recovered plaintext.
//!
//! Two scores are computed per shift, both from the same letter counts:
//!
//! * **log-likelihood** — the probability of the decrypted letters under an
//!   independent single-letter model of the language. Turned into a posterior
//!   over the 26 shifts with a softmax (uniform prior), which is the
//!   `confidence` percentage. This is the ranking metric.
//! * **chi-squared** — the classic goodness-of-fit statistic against the same
//!   expected frequencies, reported as a familiar secondary diagnostic (lower
//!   is a better fit).
//!
//! Fully deterministic: no clock, no randomness, no I/O.

/// Hard cap on the input. `output = "all"` prints 26 decryptions, so the cap
/// also bounds the worst-case output size.
pub const MAX_TEXT_CHARS: usize = 20_000;

/// Below this many scored letters the frequency match is noisy, and the output
/// says so instead of pretending otherwise.
pub const MIN_RELIABLE_LETTERS: usize = 20;

/// Width of the per-shift text preview in the ranked table.
const PREVIEW_CHARS: usize = 72;

/// Confidence is a model posterior, not proof — never print a bare 100%.
const MAX_CONFIDENCE_PCT: f64 = 99.9;

/// Smallest expected share (percent) any letter is given, so a letter that is
/// absent from a published table can never make a decryption impossible.
const FREQ_FLOOR_PCT: f64 = 0.01;

/// Accepted `output` values, in descriptor order.
pub const OUTPUTS: [&str; 4] = ["best", "ranked", "all", "report"];

/// Accepted `language` values, in descriptor order.
pub const LANGUAGES: [&str; 6] = [
    "english",
    "french",
    "german",
    "spanish",
    "italian",
    "portuguese",
];

// ---------------------------------------------------------------------------
// Published single-letter frequency tables (percent, a..z)
//
// Accented letters are folded into their base letter by these tables, so the
// rows need not sum to exactly 100 — `profile()` normalises them.
// ---------------------------------------------------------------------------

const ENGLISH_PCT: [f64; 26] = [
    8.167, 1.492, 2.782, 4.253, 12.702, 2.228, 2.015, 6.094, 6.966, 0.153, 0.772, 4.025, 2.406,
    6.749, 7.507, 1.929, 0.095, 5.987, 6.327, 9.056, 2.758, 0.978, 2.360, 0.150, 1.974, 0.074,
];

const FRENCH_PCT: [f64; 26] = [
    7.636, 0.901, 3.260, 3.669, 14.715, 1.066, 0.866, 0.737, 7.529, 0.613, 0.074, 5.456, 2.968,
    7.095, 5.796, 2.521, 1.362, 6.693, 7.948, 7.244, 6.311, 1.838, 0.049, 0.427, 0.128, 0.326,
];

const GERMAN_PCT: [f64; 26] = [
    6.516, 1.886, 2.732, 5.076, 16.396, 1.656, 3.009, 4.577, 6.550, 0.268, 1.417, 3.437, 2.534,
    9.776, 2.594, 0.670, 0.018, 7.003, 7.270, 6.154, 4.166, 0.846, 1.921, 0.034, 0.039, 1.134,
];

const SPANISH_PCT: [f64; 26] = [
    11.525, 2.215, 4.019, 5.010, 12.181, 0.692, 1.768, 0.703, 6.247, 0.493, 0.011, 4.967, 3.157,
    6.712, 8.683, 2.510, 0.877, 6.871, 7.977, 4.632, 2.927, 1.138, 0.017, 0.215, 1.008, 0.467,
];

const ITALIAN_PCT: [f64; 26] = [
    11.745, 0.927, 4.501, 3.736, 11.792, 1.153, 1.644, 0.636, 10.143, 0.011, 0.009, 6.510, 2.512,
    6.883, 9.832, 3.056, 0.505, 6.367, 4.981, 5.623, 3.011, 2.097, 0.033, 0.003, 0.020, 1.181,
];

const PORTUGUESE_PCT: [f64; 26] = [
    14.634, 1.043, 3.882, 4.992, 12.570, 1.023, 1.303, 1.281, 6.186, 0.397, 0.015, 2.779, 4.738,
    5.046, 10.733, 2.523, 1.204, 6.530, 7.814, 4.336, 4.632, 1.665, 0.037, 0.253, 0.006, 0.470,
];

/// Expected letter probabilities for a language, normalised to sum to 1 with a
/// small floor on every letter.
fn profile(language: &str) -> Result<[f64; 26], String> {
    let pct: &[f64; 26] = match language {
        "english" => &ENGLISH_PCT,
        "french" => &FRENCH_PCT,
        "german" => &GERMAN_PCT,
        "spanish" => &SPANISH_PCT,
        "italian" => &ITALIAN_PCT,
        "portuguese" => &PORTUGUESE_PCT,
        other => {
            return Err(format!(
                "language must be one of {} (got '{other}')",
                LANGUAGES.join(", ")
            ))
        }
    };
    let mut p = [0.0f64; 26];
    let mut total = 0.0;
    for i in 0..26 {
        p[i] = pct[i].max(FREQ_FLOOR_PCT);
        total += p[i];
    }
    for v in p.iter_mut() {
        *v /= total;
    }
    Ok(p)
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// One candidate decryption: `shift` letters back from the ciphertext.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    /// Shift applied to recover the plaintext (0..=25).
    pub shift: u32,
    /// Chi-squared goodness of fit against the language profile (lower fits better).
    pub chi2: f64,
    /// Posterior probability of this shift, over all 26 (0.0..=1.0).
    pub confidence: f64,
}

/// A-Z counts of `text`, case-folded. Non-ASCII letters are not counted.
fn letter_counts(text: &str) -> [usize; 26] {
    let mut counts = [0usize; 26];
    for ch in text.chars() {
        if ch.is_ascii_alphabetic() {
            counts[(ch.to_ascii_lowercase() as u8 - b'a') as usize] += 1;
        }
    }
    counts
}

/// Score all 26 shifts, best first (ties broken by the smaller shift).
fn rank_shifts(counts: &[usize; 26], n: usize, p: &[f64; 26]) -> Vec<Candidate> {
    let total = n as f64;
    let mut loglik = [0.0f64; 26];
    let mut chi2 = [0.0f64; 26];
    for shift in 0..26usize {
        let mut ll = 0.0;
        let mut x2 = 0.0;
        for plain in 0..26usize {
            // Cipher letter `plain + shift` decrypts to `plain`.
            let observed = counts[(plain + shift) % 26] as f64;
            let expected = total * p[plain];
            ll += observed * p[plain].ln();
            x2 += (observed - expected) * (observed - expected) / expected;
        }
        loglik[shift] = ll;
        chi2[shift] = x2;
    }

    // Softmax over the log-likelihoods = posterior with a uniform prior.
    let best_ll = loglik.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let weights: Vec<f64> = loglik.iter().map(|ll| (ll - best_ll).exp()).collect();
    let weight_sum: f64 = weights.iter().sum();

    let mut out: Vec<Candidate> = (0..26)
        .map(|shift| Candidate {
            shift: shift as u32,
            chi2: chi2[shift],
            confidence: weights[shift] / weight_sum,
        })
        .collect();
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.shift.cmp(&b.shift))
    });
    out
}

/// Index of coincidence of the letter counts (0.0 when there are <2 letters).
/// English prose sits near 0.067; a flat 0.038 hints at a polyalphabetic cipher.
pub fn index_of_coincidence(counts: &[usize; 26], n: usize) -> f64 {
    if n < 2 {
        return 0.0;
    }
    let pairs: f64 = counts.iter().map(|&c| (c as f64) * (c as f64 - 1.0)).sum();
    pairs / (n as f64 * (n as f64 - 1.0))
}

/// Shift `text` back by `shift` letters, preserving case, spacing and
/// punctuation. With `shift_digits`, digits are rotated back by `shift % 10`
/// too (the "alphabet with digits" Caesar variant).
pub fn decrypt(text: &str, shift: u32, shift_digits: bool) -> String {
    let back = (26 - (shift % 26)) as u8;
    let digit_back = (10 - (shift % 10)) as u8;
    text.chars()
        .map(|ch| {
            if ch.is_ascii_lowercase() {
                (b'a' + (ch as u8 - b'a' + back) % 26) as char
            } else if ch.is_ascii_uppercase() {
                (b'A' + (ch as u8 - b'A' + back) % 26) as char
            } else if shift_digits && ch.is_ascii_digit() {
                (b'0' + (ch as u8 - b'0' + digit_back) % 10) as char
            } else {
                ch
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn confidence_pct(confidence: f64) -> f64 {
    (confidence * 100.0).min(MAX_CONFIDENCE_PCT)
}

/// Collapse every run of whitespace so one candidate occupies one table row.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn preview(s: &str) -> String {
    let flat = one_line(s);
    if flat.chars().count() <= PREVIEW_CHARS {
        flat
    } else {
        let head: String = flat.chars().take(PREVIEW_CHARS).collect();
        format!("{head}...")
    }
}

fn headline(best: &Candidate, letters: usize, language: &str) -> String {
    format!(
        "Best shift: {} (confidence {:.1}%, {} letters scored against {} frequencies)",
        best.shift,
        confidence_pct(best.confidence),
        letters,
        language
    )
}

fn short_text_note(letters: usize) -> Option<String> {
    if letters >= MIN_RELIABLE_LETTERS {
        return None;
    }
    Some(format!(
        "Note: only {letters} letters were scored. Frequency analysis needs about \
         {MIN_RELIABLE_LETTERS} letters to be reliable, so compare the ranked candidates \
         before trusting this shift."
    ))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Crack `text` and render the requested `output` view.
///
/// * `output` — one of [`OUTPUTS`].
/// * `language` — one of [`LANGUAGES`], the expected letter frequencies.
/// * `top` — how many candidates the `ranked` view lists (1..=26).
/// * `shift_digits` — also rotate digits 0-9 back by `shift % 10`.
pub fn crack(
    text: &str,
    output: &str,
    language: &str,
    top: u32,
    shift_digits: bool,
) -> Result<String, String> {
    let chars = text.chars().count();
    if chars > MAX_TEXT_CHARS {
        return Err(format!(
            "text is {chars} characters; the limit is {MAX_TEXT_CHARS} — crack a shorter \
             excerpt (a few sentences is plenty to find the shift)"
        ));
    }
    if !OUTPUTS.contains(&output) {
        return Err(format!(
            "output must be one of {} (got '{output}')",
            OUTPUTS.join(", ")
        ));
    }
    let p = profile(language)?;
    if !(1..=26).contains(&top) {
        return Err(format!("top must be between 1 and 26 (got {top})"));
    }

    let counts = letter_counts(text);
    let letters: usize = counts.iter().sum();
    if letters == 0 {
        return Err(format!(
            "no A-Z letters to analyse (found 0 letters in {chars} characters) — a Caesar \
             cipher only shifts letters, so there is nothing to score"
        ));
    }

    let ranked = rank_shifts(&counts, letters, &p);
    let best = ranked[0];
    let plaintext = decrypt(text, best.shift, shift_digits);

    let mut head = vec![headline(&best, letters, language)];
    if output == "report" {
        let ic = index_of_coincidence(&counts, letters);
        head.push(format!(
            "Index of coincidence: {ic:.4} (single-alphabet prose runs near 0.0667; a flat \
             0.0385 suggests a polyalphabetic cipher such as Vigenere, not a Caesar shift)"
        ));
    }
    if let Some(note) = short_text_note(letters) {
        head.push(note);
    }

    let mut out = head.join("\n");
    out.push_str("\n\n");
    out.push_str(&plaintext);

    match output {
        "best" => {}
        "ranked" => {
            let n = (top as usize).min(26);
            out.push_str(&format!("\n\nTop {n} of 26 shifts by score:\n"));
            for (i, c) in ranked.iter().take(n).enumerate() {
                out.push_str(&format!(
                    "{:>3}. shift {:>2}  chi2 {:>9.1}  conf {:>5.1}%  {}\n",
                    i + 1,
                    c.shift,
                    c.chi2,
                    confidence_pct(c.confidence),
                    preview(&decrypt(text, c.shift, shift_digits))
                ));
            }
            out.pop();
        }
        "all" => {
            out.push_str("\n\nAll 26 decryptions (shift 0 is the text unchanged):\n");
            let by_shift = {
                let mut v = ranked.clone();
                v.sort_by_key(|c| c.shift);
                v
            };
            for c in &by_shift {
                out.push_str(&format!(
                    "shift {:>2}  conf {:>5.1}%  {}\n",
                    c.shift,
                    confidence_pct(c.confidence),
                    one_line(&decrypt(text, c.shift, shift_digits))
                ));
            }
            out.pop();
        }
        "report" => {
            out.push_str(&format!(
                "\n\nCipher letter frequencies (count, share, expected in {language}, \
                 plaintext letter at shift {}):\n",
                best.shift
            ));
            let plain_of = |cipher: usize| -> char {
                (b'a' + ((cipher + 26 - best.shift as usize) % 26) as u8) as char
            };
            for i in 0..26usize {
                out.push_str(&format!(
                    "  {}  {:>5}  {:>5.1}%  {:>5.1}%  -> {}\n",
                    (b'A' + i as u8) as char,
                    counts[i],
                    100.0 * counts[i] as f64 / letters as f64,
                    100.0 * p[i],
                    plain_of(i)
                ));
            }
            out.push_str("\nAll 26 shifts by score:\n");
            for (i, c) in ranked.iter().enumerate() {
                out.push_str(&format!(
                    "{:>3}. shift {:>2}  chi2 {:>9.1}  conf {:>5.1}%\n",
                    i + 1,
                    c.shift,
                    c.chi2,
                    confidence_pct(c.confidence)
                ));
            }
            out.pop();
        }
        _ => unreachable!("output validated above"),
    }

    Ok(out)
}

/// Default-options convenience wrapper: English frequency scoring, best shift
/// only, five candidates internally, and letters only (digits preserved).
pub fn run(input: &str) -> Result<String, String> {
    crack(input, "best", "english", 5, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "The quick brown fox jumps over the lazy dog." at shift 3.
    const PANGRAM_C3: &str = "Wkh txlfn eurzq ira mxpsv ryhu wkh odcb grj.";
    const PANGRAM: &str = "The quick brown fox jumps over the lazy dog.";

    /// Test-side encipher, so fixtures are never hand-shifted wrongly.
    fn encrypt(plain: &str, shift: u32, shift_digits: bool) -> String {
        let fwd = (shift % 26) as u8;
        let dfwd = (shift % 10) as u8;
        plain
            .chars()
            .map(|ch| {
                if ch.is_ascii_lowercase() {
                    (b'a' + (ch as u8 - b'a' + fwd) % 26) as char
                } else if ch.is_ascii_uppercase() {
                    (b'A' + (ch as u8 - b'A' + fwd) % 26) as char
                } else if shift_digits && ch.is_ascii_digit() {
                    (b'0' + (ch as u8 - b'0' + dfwd) % 10) as char
                } else {
                    ch
                }
            })
            .collect()
    }

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let plain = "Meet at 12 sharp, gate 9!";
        for shift in 0..26 {
            assert_eq!(decrypt(&encrypt(plain, shift, true), shift, true), plain);
            assert_eq!(decrypt(&encrypt(plain, shift, false), shift, false), plain);
        }
    }

    #[test]
    fn the_pangram_fixture_is_a_shift_of_three() {
        assert_eq!(encrypt(PANGRAM, 3, false), PANGRAM_C3);
    }

    #[test]
    fn best_finds_the_shift_and_recovers_the_plaintext() {
        let out = crack(PANGRAM_C3, "best", "english", 5, false).unwrap();
        let mut lines = out.lines();
        assert_eq!(
            lines.next().unwrap(),
            "Best shift: 3 (confidence 98.6%, 35 letters scored against english frequencies)"
        );
        assert_eq!(lines.next().unwrap(), "");
        assert_eq!(lines.next().unwrap(), PANGRAM);
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn run_uses_default_options() {
        let out = run(PANGRAM_C3).unwrap();
        assert!(out.starts_with("Best shift: 3 "), "{out}");
        assert!(out.ends_with(PANGRAM), "{out}");
    }

    #[test]
    fn rot13_round_trips() {
        let cipher = decrypt(PANGRAM, 13, false); // ROT13 is its own inverse
        let out = crack(&cipher, "best", "english", 5, false).unwrap();
        assert!(out.starts_with("Best shift: 13 "), "{out}");
        assert!(out.ends_with(PANGRAM), "{out}");
    }

    #[test]
    fn case_punctuation_and_digits_survive_by_default() {
        let plain = "Attack at 07:00, Bravo-2 — bring the ladder and both ropes!";
        let out = crack(&encrypt(plain, 5, false), "best", "english", 5, false).unwrap();
        assert!(out.starts_with("Best shift: 5 "), "{out}");
        assert!(out.ends_with(plain), "{out}");
    }

    #[test]
    fn shift_digits_rotates_digits_too() {
        let plain = "Meet at 12 sharp by gate 9, bring the blue folder and both keys";
        let cipher = encrypt(plain, 3, true);
        let out = crack(&cipher, "best", "english", 5, true).unwrap();
        assert!(out.ends_with(plain), "{out}");
        // Without the flag the digits are left as they came in.
        let out = crack(&cipher, "best", "english", 5, false).unwrap();
        assert!(
            out.ends_with("Meet at 45 sharp by gate 2, bring the blue folder and both keys"),
            "{out}"
        );
    }

    #[test]
    fn ranked_lists_the_requested_number_of_candidates() {
        let out = crack(PANGRAM_C3, "ranked", "english", 3, false).unwrap();
        assert!(out.contains("Top 3 of 26 shifts by score:"), "{out}");
        let rows: Vec<&str> = out.lines().filter(|l| l.contains("shift ")).collect();
        assert_eq!(rows.len(), 3, "{out}");
        assert!(rows[0].contains("shift  3"), "{}", rows[0]);
        assert!(rows[0].contains("conf  98.6%"), "{}", rows[0]);
    }

    #[test]
    fn ranked_top_is_capped_at_26() {
        let out = crack(PANGRAM_C3, "ranked", "english", 26, false).unwrap();
        assert!(out.contains("Top 26 of 26 shifts by score:"), "{out}");
        assert_eq!(
            out.lines().filter(|l| l.contains("chi2")).count(),
            26,
            "{out}"
        );
    }

    #[test]
    fn all_lists_every_shift_in_order() {
        let out = crack(PANGRAM_C3, "all", "english", 5, false).unwrap();
        let rows: Vec<&str> = out.lines().filter(|l| l.starts_with("shift ")).collect();
        assert_eq!(rows.len(), 26);
        assert!(
            rows[0].ends_with(one_line(PANGRAM_C3).as_str()),
            "{}",
            rows[0]
        );
        assert!(rows[3].ends_with(PANGRAM), "{}", rows[3]);
    }

    #[test]
    fn report_includes_ic_frequencies_and_every_shift() {
        let out = crack(PANGRAM_C3, "report", "english", 5, false).unwrap();
        assert!(out.contains("Index of coincidence: 0."), "{out}");
        assert!(out.contains("Cipher letter frequencies"), "{out}");
        assert!(out.contains("-> t"), "{out}"); // cipher W maps to plain t
        assert_eq!(
            out.lines().filter(|l| l.contains("chi2")).count(),
            26,
            "{out}"
        );
    }

    #[test]
    fn other_languages_use_their_own_profile() {
        let plain = "bonjour le monde comme il fait beau ce matin a paris";
        let cipher = encrypt(plain, 7, false);
        let out = crack(&cipher, "best", "french", 5, false).unwrap();
        assert!(out.starts_with("Best shift: 7 "), "{out}");
        assert!(out.ends_with(plain), "{out}");
    }

    #[test]
    fn short_input_warns_instead_of_pretending() {
        let out = crack("Khoor", "best", "english", 5, false).unwrap();
        assert!(out.contains("Note: only 5 letters were scored."), "{out}");
    }

    #[test]
    fn text_without_letters_is_an_error() {
        let err = crack("12:34 — 56!", "best", "english", 5, false).unwrap_err();
        assert!(
            err.starts_with("no A-Z letters to analyse (found 0 letters in"),
            "{err}"
        );
    }

    #[test]
    fn unknown_output_and_language_are_errors() {
        let err = crack("Khoor zruog", "guess", "english", 5, false).unwrap_err();
        assert_eq!(
            err,
            "output must be one of best, ranked, all, report (got 'guess')"
        );
        let err = crack("Khoor zruog", "best", "klingon", 5, false).unwrap_err();
        assert_eq!(
            err,
            "language must be one of english, french, german, spanish, italian, portuguese \
             (got 'klingon')"
        );
    }

    #[test]
    fn top_out_of_range_is_an_error() {
        let err = crack("Khoor zruog", "ranked", "english", 0, false).unwrap_err();
        assert_eq!(err, "top must be between 1 and 26 (got 0)");
        let err = crack("Khoor zruog", "ranked", "english", 27, false).unwrap_err();
        assert_eq!(err, "top must be between 1 and 26 (got 27)");
    }

    #[test]
    fn oversized_text_is_an_error() {
        let big = "a".repeat(MAX_TEXT_CHARS + 1);
        let err = crack(&big, "best", "english", 5, false).unwrap_err();
        assert!(
            err.starts_with("text is 20001 characters; the limit is 20000"),
            "{err}"
        );
    }

    #[test]
    fn exactly_at_the_cap_is_accepted() {
        let big = "a".repeat(MAX_TEXT_CHARS);
        assert!(crack(&big, "best", "english", 5, false).is_ok());
    }

    #[test]
    fn confidence_never_claims_certainty() {
        let long = PANGRAM_C3.repeat(20);
        let out = crack(&long, "best", "english", 5, false).unwrap();
        assert!(out.contains("confidence 99.9%"), "{out}");
    }
}
