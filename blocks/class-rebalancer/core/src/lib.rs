//! gizza-ai/class-rebalancer core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Balances an imbalanced label
//! column of a CSV by random over-sampling the minority class(es) (duplicating
//! rows) and/or under-sampling the majority class(es) (dropping rows) toward a
//! target class ratio.
//!
//! Randomness is a small in-house seeded PRNG (splitmix64), so every surface —
//! chat, CLI, page, tests — is deterministic for a given `seed`; change `seed`
//! for a different draw. No OS RNG / `getrandom` dependency.

use std::collections::{HashMap, HashSet};

/// Hard caps to keep a runaway ratio from exploding memory in the wasm sandbox.
const MAX_INPUT_BYTES: usize = 20_000_000;
const MAX_OUTPUT_ROWS: usize = 5_000_000;

/// splitmix64 — a tiny deterministic PRNG. `next_u64()` yields a u64 stream.
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
    /// for the small ranges here.
    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

/// Fisher–Yates shuffle of `idx` using `rng`, in place.
fn shuffle(idx: &mut [usize], rng: &mut Rng) {
    if idx.len() < 2 {
        return;
    }
    for i in (1..idx.len()).rev() {
        let j = rng.below(i + 1);
        idx.swap(i, j);
    }
}

/// Pick `k` of the members (source row indices) at random (seeded), returned in
/// ascending original order so a kept subset preserves the file's row order.
fn pick_distinct(members: &[usize], k: usize, rng: &mut Rng) -> Vec<usize> {
    let m = members.len();
    let k = k.min(m);
    let mut idx: Vec<usize> = (0..m).collect();
    shuffle(&mut idx, rng);
    idx.truncate(k);
    let mut out: Vec<usize> = idx.iter().map(|&j| members[j]).collect();
    out.sort_unstable();
    out
}

/// Resolve the label-column spec to a 0-based index. A 1-based number or a
/// header name; blank means the LAST column of the reference record.
fn resolve_label_column(
    spec: &str,
    header: Option<&csv::StringRecord>,
    reference: &csv::StringRecord,
) -> Result<usize, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        let n = reference.len();
        if n == 0 {
            return Err("cannot infer a label column from an empty row".into());
        }
        return Ok(n - 1);
    }
    if let Ok(n) = spec.parse::<usize>() {
        if n == 0 {
            return Err("label_column index is 1-based (>= 1)".into());
        }
        return Ok(n - 1);
    }
    match header {
        Some(h) => h
            .iter()
            .position(|c| c == spec)
            .ok_or_else(|| format!("label_column '{spec}' not found in the header")),
        None => Err(format!(
            "label_column '{spec}' is not a number and there is no header to match names"
        )),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Strat {
    Oversample,
    Undersample,
    Combine,
}
fn strat_name(s: Strat) -> &'static str {
    match s {
        Strat::Oversample => "oversample",
        Strat::Undersample => "undersample",
        Strat::Combine => "combine",
    }
}

/// Minimal JSON string escaper (core carries no serde_json dependency).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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
    out
}

/// Balance the DATA rows of a CSV by its label column.
///
/// * `label_column` — a header name (when `has_header`) or a 1-based column
///   number; blank = the last column.
/// * `strategy` — `auto`/`oversample` (duplicate minority rows up), `undersample`
///   (drop majority rows down), or `combine` (both, to a common size).
/// * `target_ratio` — desired minority:majority class ratio after resampling,
///   in (0, 1]. 1.0 = fully balanced.
/// * `seed` — seeds the reproducible PRNG.
/// * `shuffle_out` — shuffle the output rows (seeded); otherwise originals keep
///   file order with duplicates appended.
/// * `output` — `csv` (the rebalanced CSV) or `summary` (a JSON count report).
#[allow(clippy::too_many_arguments)]
pub fn rebalance(
    data: &str,
    label_column: &str,
    strategy: &str,
    target_ratio: f64,
    has_header: bool,
    shuffle_out: bool,
    seed: u64,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the {}-byte limit was exceeded",
            data.len(),
            MAX_INPUT_BYTES
        ));
    }
    let strategy_lc = strategy.trim().to_ascii_lowercase();
    let strat = match strategy_lc.as_str() {
        "" | "auto" | "oversample" | "over" => Strat::Oversample,
        "undersample" | "under" => Strat::Undersample,
        "combine" | "both" => Strat::Combine,
        other => {
            return Err(format!(
                "strategy must be one of auto, oversample, undersample, combine (got '{other}')"
            ))
        }
    };
    if !target_ratio.is_finite() || target_ratio <= 0.0 || target_ratio > 1.0 {
        return Err(format!(
            "target_ratio must be a number greater than 0 and at most 1.0 (got {target_ratio})"
        ));
    }
    let output_lc = output.trim().to_ascii_lowercase();
    let want_summary = match output_lc.as_str() {
        "" | "csv" => false,
        "summary" => true,
        other => return Err(format!("output must be csv or summary (got '{other}')")),
    };

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;
    if records.is_empty() {
        return Err("input has no rows".into());
    }

    let (header, body): (Option<&csv::StringRecord>, &[csv::StringRecord]) = if has_header {
        (records.first(), &records[1..])
    } else {
        (None, &records[..])
    };
    if body.is_empty() {
        return Err("no data rows to rebalance (only a header was found)".into());
    }
    let reference = header.unwrap_or(&body[0]);
    let col = resolve_label_column(label_column, header, reference)?;

    // Group data-row indices by their label value, preserving first-seen order.
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, r) in body.iter().enumerate() {
        let key = r
            .get(col)
            .ok_or_else(|| {
                format!(
                    "data row {} has no column {} (the label column)",
                    i + 1,
                    col + 1
                )
            })?
            .to_string();
        groups
            .entry(key.clone())
            .or_insert_with(|| {
                order.push(key.clone());
                Vec::new()
            })
            .push(i);
    }
    if order.len() < 2 {
        return Err(format!(
            "need at least 2 distinct classes in the label column to rebalance, but found {} ('{}')",
            order.len(),
            order.first().cloned().unwrap_or_default()
        ));
    }

    let counts: Vec<usize> = order.iter().map(|k| groups[k].len()).collect();
    let majority = *counts.iter().max().unwrap();
    let minority = *counts.iter().min().unwrap();

    // Per-strategy target size for a class currently holding `n` rows.
    let target_for = |n: usize| -> usize {
        match strat {
            // Grow small classes up to ratio×majority; never shrink.
            Strat::Oversample => ((target_ratio * majority as f64).round() as usize).max(n),
            // Shrink big classes down to minority/ratio; never grow.
            Strat::Undersample => n.min((minority as f64 / target_ratio).round() as usize),
            // Move every class to the same ratio×majority size (grow AND shrink).
            Strat::Combine => ((target_ratio * majority as f64).round() as usize).max(1),
        }
    };

    let total_after: usize = order.iter().map(|k| target_for(groups[k].len())).sum();
    if total_after > MAX_OUTPUT_ROWS {
        return Err(format!(
            "the requested balance would produce {total_after} rows, over the {MAX_OUTPUT_ROWS}-row limit; lower target_ratio or use undersample"
        ));
    }

    let mut rng = Rng::new(seed);
    let mut keep: HashSet<usize> = HashSet::new(); // source rows emitted once
    let mut extra: Vec<usize> = Vec::new(); // duplicated source rows (oversample)
    let mut after_counts: Vec<usize> = Vec::with_capacity(order.len());
    for key in &order {
        let members = &groups[key];
        let n = members.len();
        let m = target_for(n);
        after_counts.push(m);
        if m == n {
            keep.extend(members.iter().copied());
        } else if m < n {
            for p in pick_distinct(members, m, &mut rng) {
                keep.insert(p);
            }
        } else {
            keep.extend(members.iter().copied());
            for _ in 0..(m - n) {
                extra.push(members[rng.below(n)]);
            }
        }
    }

    if want_summary {
        let total_before: usize = counts.iter().sum();
        let total_after: usize = after_counts.iter().sum();
        let mut s = String::from("{\n  \"classes\": [\n");
        for (i, key) in order.iter().enumerate() {
            s.push_str(&format!(
                "    {{\"label\": {}, \"before\": {}, \"after\": {}}}{}\n",
                json_str(key),
                counts[i],
                after_counts[i],
                if i + 1 < order.len() { "," } else { "" }
            ));
        }
        s.push_str(&format!(
            "  ],\n  \"strategy\": \"{}\",\n  \"target_ratio\": {},\n  \"total_before\": {},\n  \"total_after\": {}\n}}\n",
            strat_name(strat),
            target_ratio,
            total_before,
            total_after
        ));
        return Ok(s);
    }

    // Assemble the output row indices: kept originals in file order, then the
    // duplicated rows; optionally shuffle the whole thing (seeded).
    let mut out_idx: Vec<usize> = Vec::with_capacity(keep.len() + extra.len());
    for (i, _) in body.iter().enumerate() {
        if keep.contains(&i) {
            out_idx.push(i);
        }
    }
    out_idx.extend_from_slice(&extra);
    if shuffle_out {
        shuffle(&mut out_idx, &mut rng);
    }

    let mut wtr = csv::WriterBuilder::new().flexible(true).from_writer(vec![]);
    if let Some(h) = header {
        wtr.write_record(h)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for &i in &out_idx {
        wtr.write_record(&body[i])
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1 spam + 3 ham (label is the last column).
    const DATA: &str = "text,label\nbuy now,spam\nhello,ham\nhi there,ham\nsee you,ham";

    fn class_counts(csv: &str, has_header: bool) -> HashMap<String, usize> {
        let mut m: HashMap<String, usize> = HashMap::new();
        let mut lines = csv.lines();
        if has_header {
            lines.next();
        }
        for l in lines {
            if l.trim().is_empty() {
                continue;
            }
            let label = l.rsplit(',').next().unwrap().to_string();
            *m.entry(label).or_insert(0) += 1;
        }
        m
    }

    #[test]
    fn oversample_single_minority_is_deterministic() {
        // ratio 1.0: spam 1→3 (duplicate the one spam row twice), ham stays 3.
        let out = rebalance(DATA, "label", "auto", 1.0, true, false, 42, "csv").unwrap();
        assert_eq!(
            out,
            "text,label\nbuy now,spam\nhello,ham\nhi there,ham\nsee you,ham\nbuy now,spam\nbuy now,spam\n"
        );
        // Seed-independent because there is only one spam row to duplicate.
        let out99 = rebalance(DATA, "label", "auto", 1.0, true, false, 99, "csv").unwrap();
        assert_eq!(out, out99);
    }

    #[test]
    fn oversample_balances_counts() {
        let out = rebalance(DATA, "label", "oversample", 1.0, true, false, 7, "csv").unwrap();
        let c = class_counts(&out, true);
        assert_eq!(c["spam"], 3);
        assert_eq!(c["ham"], 3);
    }

    #[test]
    fn undersample_shrinks_majority_and_is_reproducible() {
        // ratio 1.0: ham 3→1, spam stays 1.
        let a = rebalance(DATA, "label", "undersample", 1.0, true, false, 5, "csv").unwrap();
        let b = rebalance(DATA, "label", "undersample", 1.0, true, false, 5, "csv").unwrap();
        assert_eq!(a, b, "same seed → same draw");
        let c = class_counts(&a, true);
        assert_eq!(c["spam"], 1);
        assert_eq!(c["ham"], 1);
        // Header + 2 data rows.
        assert_eq!(a.lines().count(), 3);
    }

    #[test]
    fn target_ratio_half_oversample() {
        // majority ham=3, ratio 0.5 → minority target round(0.5*3)=2 (but clamp
        // to at least current); spam 1→2, ham stays 3.
        let out = rebalance(DATA, "label", "oversample", 0.5, true, false, 1, "csv").unwrap();
        let c = class_counts(&out, true);
        assert_eq!(c["spam"], 2);
        assert_eq!(c["ham"], 3);
    }

    #[test]
    fn combine_moves_all_to_common_size() {
        // ratio 1.0 → all classes to majority=3 (spam up, ham unchanged).
        let out = rebalance(DATA, "label", "combine", 1.0, true, false, 3, "csv").unwrap();
        let c = class_counts(&out, true);
        assert_eq!(c["spam"], 3);
        assert_eq!(c["ham"], 3);
        // ratio 0.5 → common size round(0.5*3)=2: spam up to 2, ham down to 2.
        let out2 = rebalance(DATA, "label", "combine", 0.5, true, false, 3, "csv").unwrap();
        let c2 = class_counts(&out2, true);
        assert_eq!(c2["spam"], 2);
        assert_eq!(c2["ham"], 2);
    }

    #[test]
    fn default_label_column_is_last() {
        // No explicit column → last column (label). ratio 1 oversample.
        let out = rebalance(DATA, "", "auto", 1.0, true, false, 42, "csv").unwrap();
        let c = class_counts(&out, true);
        assert_eq!(c["spam"], 3);
        assert_eq!(c["ham"], 3);
    }

    #[test]
    fn one_based_index_and_no_header() {
        let d = "buy now,spam\nhello,ham\nhi there,ham\nsee you,ham";
        let out = rebalance(d, "2", "oversample", 1.0, false, false, 42, "csv").unwrap();
        let c = class_counts(&out, false);
        assert_eq!(c["spam"], 3);
        assert_eq!(c["ham"], 3);
    }

    #[test]
    fn summary_reports_before_after() {
        let out = rebalance(DATA, "label", "auto", 1.0, true, false, 42, "summary").unwrap();
        assert!(out.contains("\"label\": \"spam\", \"before\": 1, \"after\": 3"));
        assert!(out.contains("\"label\": \"ham\", \"before\": 3, \"after\": 3"));
        assert!(out.contains("\"total_before\": 4"));
        assert!(out.contains("\"total_after\": 6"));
        assert!(out.contains("\"strategy\": \"oversample\""));
    }

    #[test]
    fn shuffle_preserves_multiset_of_rows() {
        let plain = rebalance(DATA, "label", "auto", 1.0, true, false, 3, "csv").unwrap();
        let shuf = rebalance(DATA, "label", "auto", 1.0, true, true, 3, "csv").unwrap();
        let mut a: Vec<&str> = plain.lines().skip(1).collect();
        let mut b: Vec<&str> = shuf.lines().skip(1).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "shuffle only reorders, never changes the rows");
    }

    #[test]
    fn errors() {
        assert!(rebalance("   ", "label", "auto", 1.0, true, false, 42, "csv").is_err()); // empty
        assert!(rebalance(DATA, "label", "nope", 1.0, true, false, 42, "csv").is_err()); // bad strategy
        assert!(rebalance(DATA, "label", "auto", 0.0, true, false, 42, "csv").is_err()); // ratio 0
        assert!(rebalance(DATA, "label", "auto", 1.5, true, false, 42, "csv").is_err()); // ratio > 1
        assert!(rebalance(DATA, "missing", "auto", 1.0, true, false, 42, "csv").is_err()); // bad column
        assert!(rebalance(DATA, "label", "auto", 1.0, true, false, 42, "bogus").is_err()); // bad output
                                                                                           // Only one class → nothing to rebalance.
        assert!(rebalance(
            "text,label\na,x\nb,x\nc,x",
            "label",
            "auto",
            1.0,
            true,
            false,
            42,
            "csv"
        )
        .is_err());
    }
}
