//! gizza-ai/reservoir-sampler core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Draws a uniform random sample of fixed size `k` from a line-oriented dataset
//! in ONE pass, using classic reservoir sampling. Two algorithms are exposed:
//!
//! * **Algorithm R** — the textbook version: keep the first `k` records, then
//!   for the i-th record (1-based, i > k) replace a random reservoir slot with
//!   probability k/i. One random draw per record.
//! * **Algorithm L** — the skip-based optimum: draw how many records to jump
//!   over before the next replacement, so the number of random draws is
//!   O(k · log(n/k)) instead of O(n).
//!
//! Both yield a uniform simple random sample WITHOUT replacement, and both read
//! the records as a stream — memory is proportional to the sample size `k`, not
//! to the dataset. Randomness is a small in-house seeded PRNG (splitmix64), so
//! every surface — chat, CLI, page, tests — is deterministic for a given `seed`;
//! change `seed` for a different draw. No OS RNG / `getrandom` dependency.

/// Largest sample size accepted, so a typo can't ask for an absurd reservoir.
pub const MAX_K: u32 = 1_000_000;

/// splitmix64 — a tiny deterministic PRNG.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Avoid a zero state producing a degenerate stream.
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish integer in `0..bound` (bound > 0). Modulo bias is negligible
    /// at these ranges (bound <= MAX_K).
    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    /// Uniform f64 strictly inside (0, 1) — never exactly 0 or 1, so `ln()`
    /// stays finite for Algorithm L.
    fn next_open(&mut self) -> f64 {
        let x = (self.next_u64() >> 11) as f64; // 0 ..= 2^53 - 1
        (x + 0.5) / 9_007_199_254_740_992.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Algo {
    R,
    L,
}

impl Algo {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "l" | "algorithm-l" | "skip" | "optimal" => Ok(Algo::L),
            "r" | "algorithm-r" | "classic" => Ok(Algo::R),
            other => Err(format!(
                "algorithm must be 'l' (skip-based, default) or 'r' (classic), got '{other}'"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Algo::R => "R",
            Algo::L => "L",
        }
    }

    fn value(self) -> &'static str {
        match self {
            Algo::R => "r",
            Algo::L => "l",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    Input,
    Reservoir,
}

impl Order {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "input" | "original" | "source" => Ok(Order::Input),
            "reservoir" | "draw" | "random" => Ok(Order::Reservoir),
            other => Err(format!(
                "order must be 'input' (default) or 'reservoir', got '{other}'"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Lines,
    Numbered,
    Json,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "lines" | "text" => Ok(Format::Lines),
            "numbered" => Ok(Format::Numbered),
            "json" => Ok(Format::Json),
            other => Err(format!(
                "format must be 'lines' (default), 'numbered', or 'json', got '{other}'"
            )),
        }
    }
}

/// One record: its 1-based line number in the original input, and its text.
type Record<'a> = (usize, &'a str);

/// Reservoir-sample `items` down to at most `k`, returning the reservoir plus
/// the number of records scanned. Consumes the iterator exactly once.
fn draw<'a, I>(mut items: I, k: usize, algo: Algo, rng: &mut Rng) -> (Vec<Record<'a>>, usize)
where
    I: Iterator<Item = Record<'a>>,
{
    let mut reservoir: Vec<Record<'a>> = Vec::with_capacity(k.min(4096));
    let mut scanned = 0usize;

    // Phase 1 (shared): the first k records fill the reservoir outright.
    for item in items.by_ref() {
        scanned += 1;
        reservoir.push(item);
        if reservoir.len() == k {
            break;
        }
    }
    if reservoir.len() < k {
        // The whole dataset is smaller than the requested sample.
        return (reservoir, scanned);
    }

    match algo {
        Algo::R => {
            for item in items {
                scanned += 1;
                // Record #scanned survives with probability k/scanned.
                let j = rng.below(scanned);
                if j < k {
                    reservoir[j] = item;
                }
            }
        }
        Algo::L => {
            let mut w = (rng.next_open().ln() / k as f64).exp();
            loop {
                // How many records to jump over before the next replacement.
                let raw = rng.next_open().ln() / (1.0 - w).ln();
                let gap = if raw.is_finite() && raw >= 0.0 && raw < usize::MAX as f64 {
                    raw.floor() as usize
                } else {
                    // Degenerate jump: drain the rest so `scanned` stays exact.
                    usize::MAX
                };

                let mut skipped = 0usize;
                let mut chosen = None;
                for item in items.by_ref() {
                    scanned += 1;
                    if skipped == gap {
                        chosen = Some(item);
                        break;
                    }
                    skipped += 1;
                }
                match chosen {
                    Some(item) => {
                        let j = rng.below(k);
                        reservoir[j] = item;
                        w *= (rng.next_open().ln() / k as f64).exp();
                    }
                    None => break, // stream exhausted
                }
            }
        }
    }

    (reservoir, scanned)
}

/// Append `s` to `out` as a JSON string literal.
fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Draw a uniform random sample of `k` lines from `data` in one pass.
///
/// * `k` — sample size; `0` means the default of 10. Must be <= [`MAX_K`].
/// * `algorithm` — `"l"` (skip-based, default) or `"r"` (classic).
/// * `seed` — PRNG seed; same seed + same input => same sample.
/// * `skip_empty` — drop blank / whitespace-only lines before sampling.
/// * `header` — treat the first line as a header: never sampled, and echoed at
///   the top of `lines`/`numbered` output.
/// * `order` — `"input"` (original order, default) or `"reservoir"` (draw order).
/// * `format` — `"lines"`, `"numbered"` (line number + TAB + text), or `"json"`.
/// * `stats` — append (or, for JSON, wrap in) the one-pass statistics.
#[allow(clippy::too_many_arguments)]
pub fn sample(
    data: &str,
    k: u32,
    algorithm: &str,
    seed: u64,
    skip_empty: bool,
    header: bool,
    order: &str,
    format: &str,
    stats: bool,
) -> Result<String, String> {
    let algo = Algo::parse(algorithm)?;
    let order = Order::parse(order)?;
    let format = Format::parse(format)?;
    let k = if k == 0 { 10 } else { k };
    if k > MAX_K {
        return Err(format!(
            "k must be between 1 and {MAX_K} records, got {k}"
        ));
    }

    // Drop exactly one trailing newline so "a\nb\n" is 2 records, not 3.
    let mut body = data;
    if let Some(b) = body.strip_suffix('\n') {
        body = b.strip_suffix('\r').unwrap_or(b);
    }

    let mut all = body
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .enumerate()
        .map(|(i, l)| (i + 1, l));

    // An empty input splits into a single empty record — that is zero records.
    let empty_input = body.is_empty();

    let header_line: Option<Record<'_>> = if header && !empty_input {
        all.next()
    } else {
        None
    };
    if header && header_line.is_none() {
        return Err("header is on but the input has no lines to use as a header".into());
    }

    let mut rng = Rng::new(seed);
    let (mut reservoir, scanned) = if empty_input {
        (Vec::new(), 0)
    } else {
        let filtered = all.filter(|(_, l)| !skip_empty || !l.trim().is_empty());
        draw(filtered, k as usize, algo, &mut rng)
    };

    if reservoir.is_empty() {
        return Err(format!(
            "no records to sample: the input has no{} lines{}",
            if skip_empty { " non-blank" } else { "" },
            if header { " after the header row" } else { "" }
        ));
    }

    if order == Order::Input {
        reservoir.sort_by_key(|(i, _)| *i);
    }

    let taken = reservoir.len();
    let probability = taken as f64 / scanned as f64;

    let mut out = String::new();
    match format {
        Format::Lines | Format::Numbered => {
            if let Some((n, text)) = header_line {
                if format == Format::Numbered {
                    out.push_str(&format!("{n}\t"));
                }
                out.push_str(text);
                out.push('\n');
            }
            for (idx, (n, text)) in reservoir.iter().enumerate() {
                if idx > 0 {
                    out.push('\n');
                }
                if format == Format::Numbered {
                    out.push_str(&format!("{n}\t"));
                }
                out.push_str(text);
            }
            if stats {
                out.push_str(&format!(
                    "\n\n# sampled {taken} of {scanned} records | p = {probability:.4} | algorithm {} | seed {seed}",
                    algo.label()
                ));
            }
        }
        Format::Json => {
            let mut array = String::from("[");
            for (idx, (n, text)) in reservoir.iter().enumerate() {
                if idx > 0 {
                    array.push(',');
                }
                array.push_str(&format!("{{\"line\":{n},\"text\":"));
                push_json_string(&mut array, text);
                array.push('}');
            }
            array.push(']');
            if stats {
                out.push_str(&format!(
                    "{{\"scanned\":{scanned},\"sampled\":{taken},\"probability\":{probability:.4},\"algorithm\":\"{}\",\"seed\":{seed},\"sample\":{array}}}",
                    algo.value()
                ));
            } else {
                out.push_str(&array);
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEN: &str = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";

    fn sampled_lines(out: &str) -> Vec<&str> {
        out.lines().collect()
    }

    #[test]
    fn draws_the_requested_number_of_records() {
        let out = sample(TEN, 3, "l", 42, true, false, "input", "lines", false).unwrap();
        assert_eq!(sampled_lines(&out).len(), 3);
    }

    #[test]
    fn is_deterministic_for_a_seed_and_changes_with_it() {
        let a = sample(TEN, 3, "l", 42, true, false, "input", "lines", false).unwrap();
        let b = sample(TEN, 3, "l", 42, true, false, "input", "lines", false).unwrap();
        assert_eq!(a, b, "same seed must reproduce the same sample");
        let mut differs = false;
        for seed in 0..40u64 {
            if sample(TEN, 3, "l", seed, true, false, "input", "lines", false).unwrap() != a {
                differs = true;
                break;
            }
        }
        assert!(differs, "some other seed must yield a different sample");
    }

    #[test]
    fn keeps_input_order_by_default_and_draw_order_on_request() {
        let ordered = sample(TEN, 4, "r", 7, true, false, "input", "lines", false).unwrap();
        let picked = sampled_lines(&ordered);
        let mut sorted = picked.clone();
        sorted.sort_unstable();
        assert_eq!(picked, sorted, "input order == alphabetical for this fixture");

        // The reservoir order is a permutation of the same draw.
        let raw = sample(TEN, 4, "r", 7, true, false, "reservoir", "lines", false).unwrap();
        let mut a = sampled_lines(&raw);
        a.sort_unstable();
        assert_eq!(a, sorted);
    }

    #[test]
    fn both_algorithms_sample_without_replacement() {
        for algo in ["l", "r"] {
            for seed in 0..25u64 {
                let out = sample(TEN, 4, algo, seed, true, false, "input", "lines", false).unwrap();
                let mut picked = sampled_lines(&out);
                let count = picked.len();
                picked.sort_unstable();
                picked.dedup();
                assert_eq!(picked.len(), count, "algorithm {algo} repeated a record");
                assert_eq!(count, 4);
            }
        }
    }

    #[test]
    fn algorithm_l_is_uniform_over_many_seeds() {
        // Each of the 10 records should show up in a 1-of-10 sample roughly
        // 10% of the time; with 500 draws a broken skip would be obvious.
        let mut hits = [0usize; 10];
        for seed in 0..500u64 {
            let out = sample(TEN, 1, "l", seed, true, false, "input", "lines", false).unwrap();
            let idx = (out.as_bytes()[0] - b'a') as usize;
            hits[idx] += 1;
        }
        for (i, h) in hits.iter().enumerate() {
            assert!(
                (10..=100).contains(h),
                "record {i} drawn {h}/500 times — not plausibly uniform"
            );
        }
    }

    #[test]
    fn returns_everything_when_the_dataset_is_smaller_than_k() {
        let out = sample("x\ny", 10, "l", 1, true, false, "input", "lines", false).unwrap();
        assert_eq!(out, "x\ny");
    }

    #[test]
    fn skips_blank_lines_unless_told_not_to() {
        let data = "a\n\n   \nb";
        let out = sample(data, 10, "l", 1, true, false, "input", "lines", false).unwrap();
        assert_eq!(out, "a\nb");
        let kept = sample(data, 10, "l", 1, false, false, "input", "lines", false).unwrap();
        assert_eq!(kept, "a\n\n   \nb");
    }

    #[test]
    fn header_is_kept_out_of_the_draw_and_echoed() {
        let data = "name\nalice\nbob\ncarol";
        let out = sample(data, 2, "l", 5, true, true, "input", "lines", false).unwrap();
        let lines = sampled_lines(&out);
        assert_eq!(lines[0], "name");
        assert_eq!(lines.len(), 3);
        assert!(!lines[1..].contains(&"name"));
    }

    #[test]
    fn numbered_format_uses_original_line_numbers() {
        let out = sample(TEN, 10, "l", 1, true, false, "input", "numbered", false).unwrap();
        assert_eq!(out.lines().next().unwrap(), "1\ta");
        assert_eq!(out.lines().last().unwrap(), "10\tj");
    }

    #[test]
    fn json_format_escapes_and_carries_line_numbers() {
        let out = sample("he\"llo\tx", 1, "l", 1, true, false, "input", "json", false).unwrap();
        assert_eq!(out, r#"[{"line":1,"text":"he\"llo\tx"}]"#);
    }

    #[test]
    fn json_with_stats_wraps_the_sample_in_an_object() {
        let out = sample(TEN, 2, "l", 3, true, false, "input", "json", true).unwrap();
        assert!(out.starts_with(r#"{"scanned":10,"sampled":2,"probability":0.2000,"algorithm":"l","seed":3,"sample":[{"line":"#));
        assert!(out.ends_with("]}"));
    }

    #[test]
    fn stats_line_reports_the_one_pass_summary() {
        let out = sample(TEN, 2, "r", 3, true, false, "input", "lines", true).unwrap();
        assert!(
            out.ends_with("\n\n# sampled 2 of 10 records | p = 0.2000 | algorithm R | seed 3"),
            "got: {out}"
        );
    }

    #[test]
    fn crlf_input_is_normalised() {
        let out = sample("a\r\nb\r\n", 2, "l", 1, true, false, "input", "lines", false).unwrap();
        assert_eq!(out, "a\nb");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = sample("", 3, "l", 1, true, false, "input", "lines", false).unwrap_err();
        assert!(err.contains("no records to sample"), "got: {err}");
    }

    #[test]
    fn blank_only_input_is_an_error() {
        let err = sample("\n\n  \n", 3, "l", 1, true, false, "input", "lines", false).unwrap_err();
        assert!(err.contains("no records to sample"), "got: {err}");
    }

    #[test]
    fn unknown_algorithm_order_and_format_are_rejected() {
        let e = sample(TEN, 2, "z", 1, true, false, "input", "lines", false).unwrap_err();
        assert!(e.contains("algorithm must be"), "got: {e}");
        let e = sample(TEN, 2, "l", 1, true, false, "sideways", "lines", false).unwrap_err();
        assert!(e.contains("order must be"), "got: {e}");
        let e = sample(TEN, 2, "l", 1, true, false, "input", "yaml", false).unwrap_err();
        assert!(e.contains("format must be"), "got: {e}");
    }

    #[test]
    fn oversized_k_is_rejected() {
        let e = sample(TEN, MAX_K + 1, "l", 1, true, false, "input", "lines", false).unwrap_err();
        assert!(e.contains("k must be between"), "got: {e}");
    }

    #[test]
    fn k_zero_falls_back_to_the_default_of_ten() {
        let out = sample(TEN, 0, "l", 1, true, false, "input", "lines", false).unwrap();
        assert_eq!(out.lines().count(), 10);
    }

    #[test]
    fn scanned_count_ignores_skipped_blanks_and_the_header() {
        let data = "hdr\na\n\nb\nc\n";
        let out = sample(data, 1, "l", 2, true, true, "input", "lines", true).unwrap();
        assert!(out.contains("# sampled 1 of 3 records"), "got: {out}");
    }
}
