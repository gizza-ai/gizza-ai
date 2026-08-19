//! gizza-ai/train-test-split core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Splits a CSV's data rows into
//! train / test (and an optional validation) set.
//!
//! Randomness is a small in-house seeded PRNG (splitmix64), so every surface —
//! chat, CLI, page, tests — is deterministic for a given `seed`; change `seed`
//! for a different split. No OS RNG / `getrandom` dependency.

use std::collections::HashMap;

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
    /// for the row counts involved here.
    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

/// Fisher–Yates shuffle of `idx` using `rng`, in place.
fn shuffle_in_place(idx: &mut [usize], rng: &mut Rng) {
    if idx.len() < 2 {
        return;
    }
    for i in (1..idx.len()).rev() {
        let j = rng.below(i + 1);
        idx.swap(i, j);
    }
}

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single char or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Resolve a column reference — a header name or a 1-based index — to a 0-based
/// column position. `what` names the parameter for the error message.
fn resolve_column(
    what: &str,
    name: &str,
    header: Option<&csv::StringRecord>,
) -> Result<usize, String> {
    let name = name.trim();
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err(format!("{what} index is 1-based (>= 1), got 0"));
        }
        return Ok(n - 1);
    }
    match header {
        Some(h) => h
            .iter()
            .position(|c| c == name)
            .ok_or_else(|| format!("{what} '{name}' not found in the header row")),
        None => Err(format!(
            "{what} '{name}' is not a number and header is off, so there are no column names to \
             match — pass a 1-based column index instead"
        )),
    }
}

/// Turn a size parameter into a row count: below 1 it is a proportion of
/// `total`; 1 or more is an absolute row count. Negative / zero means "none".
fn resolve_size(what: &str, value: f64, total: usize) -> Result<usize, String> {
    if !value.is_finite() {
        return Err(format!("{what} must be a finite number, got '{value}'"));
    }
    if value <= 0.0 {
        return Ok(0);
    }
    if value < 1.0 {
        return Ok(((total as f64) * value).round() as usize);
    }
    Ok(value.round() as usize)
}

/// Which split a row landed in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Split {
    Train,
    Validation,
    Test,
}

/// Largest-remainder allocation of `k` picks across strata of the given sizes:
/// each stratum gets `floor(k * size/total)`, and the leftover goes to the
/// largest fractional remainders (never exceeding a stratum's `cap`).
fn largest_remainder(sizes: &[usize], caps: &[usize], k: usize, total: usize) -> Vec<usize> {
    let mut alloc = vec![0usize; sizes.len()];
    if total == 0 || k == 0 {
        return alloc;
    }
    let mut frac: Vec<(usize, f64)> = Vec::with_capacity(sizes.len());
    let mut assigned = 0usize;
    for (i, &size) in sizes.iter().enumerate() {
        let exact = (k as f64) * (size as f64) / (total as f64);
        let base = (exact.floor() as usize).min(caps[i]);
        alloc[i] = base;
        assigned += base;
        frac.push((i, exact - exact.floor()));
    }
    frac.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut remaining = k.saturating_sub(assigned);
    while remaining > 0 {
        let mut placed = false;
        for &(i, _) in &frac {
            if remaining == 0 {
                break;
            }
            if alloc[i] < caps[i] {
                alloc[i] += 1;
                remaining -= 1;
                placed = true;
            }
        }
        if !placed {
            break;
        }
    }
    alloc
}

/// Group row indices by a key, preserving first-seen key order.
fn group_by(keys: &[String]) -> (Vec<String>, HashMap<String, Vec<usize>>) {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, key) in keys.iter().enumerate() {
        groups
            .entry(key.clone())
            .or_insert_with(|| {
                order.push(key.clone());
                Vec::new()
            })
            .push(i);
    }
    (order, groups)
}

/// Bin numeric strings into `bins` equal-count (quantile) strata by rank, so a
/// continuous target can be stratified like a class label.
fn quantile_bins(values: &[String], bins: usize, column: &str) -> Result<Vec<String>, String> {
    let mut parsed: Vec<(usize, f64)> = Vec::with_capacity(values.len());
    for (i, v) in values.iter().enumerate() {
        let t = v.trim();
        let n = t.parse::<f64>().map_err(|_| {
            format!(
                "stratify_bins needs a numeric stratify column, but row {} of '{column}' is '{t}'",
                i + 1
            )
        })?;
        if !n.is_finite() {
            return Err(format!(
                "stratify_bins needs finite numbers, but row {} of '{column}' is '{t}'",
                i + 1
            ));
        }
        parsed.push((i, n));
    }
    parsed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let total = parsed.len();
    let mut out = vec![String::new(); total];
    for (rank, (i, _)) in parsed.iter().enumerate() {
        let bin = (rank * bins / total.max(1)).min(bins - 1);
        out[*i] = format!("bin{}", bin + 1);
    }
    Ok(out)
}

/// Assign `n_test` / `n_val` of the `total` rows (in the given unit order) to
/// test / validation, everything else to train. `order` is a permutation of the
/// row indices; the first `n_test` become test, the next `n_val` validation.
fn assign_from_order(order: &[usize], n_test: usize, n_val: usize, out: &mut [Split]) {
    for (rank, &i) in order.iter().enumerate() {
        if rank < n_test {
            out[i] = Split::Test;
        } else if rank < n_test + n_val {
            out[i] = Split::Validation;
        }
    }
}

fn write_csv(
    header: Option<&csv::StringRecord>,
    rows: &[&csv::StringRecord],
    delim: u8,
) -> Result<String, String> {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    if let Some(h) = header {
        wtr.write_record(h)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for r in rows {
        wtr.write_record(*r)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8 error: {e}"))
}

fn rows_label(n: usize) -> String {
    if n == 1 {
        "1 row".into()
    } else {
        format!("{n} rows")
    }
}

fn pct(n: usize, total: usize) -> String {
    if total == 0 {
        return "0.0".into();
    }
    format!("{:.1}", (n as f64) * 100.0 / (total as f64))
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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
    out
}

/// Split the DATA rows of a CSV into train / test (and optionally validation).
///
/// * `test_size` / `validation_size` — below 1 = a proportion of the data rows,
///   1 or more = an absolute row count, 0 = none.
/// * `stratify_column` — keep each class's share equal across the splits.
/// * `stratify_bins` — > 0 bins a numeric stratify column into that many
///   equal-count strata first.
/// * `group_column` — keep all rows sharing a value in the SAME split.
/// * `shuffle` — off means a sequential split (train, then validation, then
///   test, in file order); stratification then is not possible.
/// * `output` — `sections` | `train` | `test` | `validation` | `summary` | `json`.
///
/// Rows keep their original file order inside each split.
#[allow(clippy::too_many_arguments)]
pub fn split(
    data: &str,
    test_size: f64,
    validation_size: f64,
    stratify_column: &str,
    stratify_bins: usize,
    group_column: &str,
    shuffle: bool,
    seed: u64,
    has_header: bool,
    delimiter: &str,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste a CSV with at least one data row".into());
    }
    let output = output.trim().to_ascii_lowercase();
    let output = if output.is_empty() {
        "sections".to_string()
    } else {
        output
    };
    if !matches!(
        output.as_str(),
        "sections" | "train" | "test" | "validation" | "summary" | "json"
    ) {
        return Err(format!(
            "output must be one of sections, train, test, validation, summary, json (got \
             '{output}')"
        ));
    }
    let stratify_column = stratify_column.trim();
    let group_column = group_column.trim();
    if !stratify_column.is_empty() && !group_column.is_empty() {
        return Err(
            "stratify_column and group_column cannot be used together — pick class balance \
             (stratify_column) or leak-free groups (group_column)"
                .into(),
        );
    }
    if !stratify_column.is_empty() && !shuffle {
        return Err(
            "stratify_column needs shuffle to be on — a sequential (unshuffled) split cannot \
             balance classes"
                .into(),
        );
    }
    if stratify_bins > 0 && stratify_column.is_empty() {
        return Err("stratify_bins only applies when stratify_column is set".into());
    }

    let delim = delim_byte(delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;

    let (header, body): (Option<&csv::StringRecord>, &[csv::StringRecord]) = if has_header {
        (records.first(), records.get(1..).unwrap_or(&[]))
    } else {
        (None, &records[..])
    };
    let total = body.len();
    if total == 0 {
        return Err(if has_header {
            "the CSV has a header row but no data rows to split".into()
        } else {
            "the CSV has no data rows to split".to_string()
        });
    }

    let n_test = resolve_size("test_size", test_size, total)?;
    let n_val = resolve_size("validation_size", validation_size, total)?;
    if n_test + n_val > total {
        return Err(format!(
            "test ({n_test}) + validation ({n_val}) rows exceed the {total} data rows available"
        ));
    }
    if n_test + n_val == total {
        return Err(format!(
            "the split leaves 0 training rows — test ({n_test}) + validation ({n_val}) take all \
             {total} data rows"
        ));
    }

    let mut rng = Rng::new(seed);
    let mut assign = vec![Split::Train; total];
    // The stratum label of each row, kept for the `summary` balance table (it is
    // the binned label when stratify_bins is on, not the raw cell value).
    let mut strata: Option<Vec<String>> = None;

    if !group_column.is_empty() {
        // Grouped: whole groups move together, greedily filling test then
        // validation, so the sizes land near (not exactly on) the targets.
        let col = resolve_column("group_column", group_column, header)?;
        let keys: Vec<String> = body
            .iter()
            .map(|r| r.get(col).unwrap_or("").to_string())
            .collect();
        let (order, groups) = group_by(&keys);
        let mut gidx: Vec<usize> = (0..order.len()).collect();
        if shuffle {
            shuffle_in_place(&mut gidx, &mut rng);
        }
        let (mut test_rows, mut val_rows) = (0usize, 0usize);
        for &g in &gidx {
            let members = &groups[&order[g]];
            let target = if test_rows < n_test {
                test_rows += members.len();
                Split::Test
            } else if val_rows < n_val {
                val_rows += members.len();
                Split::Validation
            } else {
                Split::Train
            };
            for &i in members {
                assign[i] = target;
            }
        }
    } else if !stratify_column.is_empty() {
        // Stratified: allocate the test/validation quota across strata by
        // largest remainder, then draw within each stratum.
        let col = resolve_column("stratify_column", stratify_column, header)?;
        let raw: Vec<String> = body
            .iter()
            .map(|r| r.get(col).unwrap_or("").to_string())
            .collect();
        let keys = if stratify_bins > 0 {
            quantile_bins(&raw, stratify_bins, stratify_column)?
        } else {
            raw
        };
        let (order, groups) = group_by(&keys);
        let sizes: Vec<usize> = order.iter().map(|k| groups[k].len()).collect();
        let test_alloc = largest_remainder(&sizes, &sizes, n_test, total);
        let caps: Vec<usize> = sizes
            .iter()
            .zip(&test_alloc)
            .map(|(s, t)| s.saturating_sub(*t))
            .collect();
        let val_alloc = largest_remainder(&sizes, &caps, n_val, total);
        for (gi, key) in order.iter().enumerate() {
            let mut members = groups[key].clone();
            shuffle_in_place(&mut members, &mut rng);
            assign_from_order(&members, test_alloc[gi], val_alloc[gi], &mut assign);
        }
        strata = Some(keys);
    } else {
        let mut order: Vec<usize> = (0..total).collect();
        if shuffle {
            shuffle_in_place(&mut order, &mut rng);
            assign_from_order(&order, n_test, n_val, &mut assign);
        } else {
            // Sequential / blocked: train first, then validation, then test —
            // the time-series-safe layout (the test set is the newest rows).
            let n_train = total - n_test - n_val;
            for (rank, &i) in order.iter().enumerate() {
                assign[i] = if rank < n_train {
                    Split::Train
                } else if rank < n_train + n_val {
                    Split::Validation
                } else {
                    Split::Test
                };
            }
        }
    }

    let pick = |want: Split| -> Vec<&csv::StringRecord> {
        body.iter()
            .enumerate()
            .filter(|(i, _)| assign[*i] == want)
            .map(|(_, r)| r)
            .collect()
    };
    let train_rows = pick(Split::Train);
    let val_rows = pick(Split::Validation);
    let test_rows = pick(Split::Test);
    let train_csv = write_csv(header, &train_rows, delim)?;
    let val_csv = write_csv(header, &val_rows, delim)?;
    let test_csv = write_csv(header, &test_rows, delim)?;
    let want_val = n_val > 0 || !val_rows.is_empty();

    match output.as_str() {
        "train" => Ok(train_csv),
        "test" => Ok(test_csv),
        "validation" => {
            if want_val {
                Ok(val_csv)
            } else {
                Err("there is no validation set — set validation_size above 0 first".into())
            }
        }
        "json" => {
            let mut s = String::from("{\n  \"train\": \"");
            s.push_str(&json_escape(&train_csv));
            s.push_str("\",\n  \"test\": \"");
            s.push_str(&json_escape(&test_csv));
            s.push_str("\",\n  \"validation\": \"");
            s.push_str(&json_escape(&val_csv));
            s.push_str("\",\n  \"counts\": { \"train\": ");
            s.push_str(&train_rows.len().to_string());
            s.push_str(", \"validation\": ");
            s.push_str(&val_rows.len().to_string());
            s.push_str(", \"test\": ");
            s.push_str(&test_rows.len().to_string());
            s.push_str(", \"total\": ");
            s.push_str(&total.to_string());
            s.push_str(" }\n}");
            Ok(s)
        }
        "summary" => {
            let mut s = String::from("split,rows,percent\n");
            s.push_str(&format!(
                "train,{},{}\n",
                train_rows.len(),
                pct(train_rows.len(), total)
            ));
            if want_val {
                s.push_str(&format!(
                    "validation,{},{}\n",
                    val_rows.len(),
                    pct(val_rows.len(), total)
                ));
            }
            s.push_str(&format!(
                "test,{},{}\n",
                test_rows.len(),
                pct(test_rows.len(), total)
            ));
            if let Some(keys) = &strata {
                // Per-class balance check: how each stratum is spread over the splits.
                let (order, groups) = group_by(keys);
                s.push_str("\nsplit,class,rows,percent\n");
                for key in &order {
                    let members = &groups[key];
                    let size = members.len();
                    let count = |want: Split| members.iter().filter(|i| assign[**i] == want).count();
                    s.push_str(&format!(
                        "train,{key},{},{}\n",
                        count(Split::Train),
                        pct(count(Split::Train), size)
                    ));
                    if want_val {
                        s.push_str(&format!(
                            "validation,{key},{},{}\n",
                            count(Split::Validation),
                            pct(count(Split::Validation), size)
                        ));
                    }
                    s.push_str(&format!(
                        "test,{key},{},{}\n",
                        count(Split::Test),
                        pct(count(Split::Test), size)
                    ));
                }
            }
            Ok(s)
        }
        _ => {
            // "sections" — every split in one labelled, copy-pasteable block.
            let mut s = String::new();
            s.push_str(&format!("# train ({})\n", rows_label(train_rows.len())));
            s.push_str(&train_csv);
            if want_val {
                s.push_str(&format!("\n# validation ({})\n", rows_label(val_rows.len())));
                s.push_str(&val_csv);
            }
            s.push_str(&format!("\n# test ({})\n", rows_label(test_rows.len())));
            s.push_str(&test_csv);
            Ok(s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = "id,label\n1,a\n2,b\n3,a\n4,b\n5,a\n6,b\n7,a\n8,b\n9,a\n10,b";

    fn rows_of(csv: &str) -> usize {
        csv.lines().count().saturating_sub(1) // minus the header
    }

    #[test]
    fn default_80_20_split_is_reproducible() {
        let a = split(DATA, 0.2, 0.0, "", 0, "", true, 42, true, "comma", "json").unwrap();
        let b = split(DATA, 0.2, 0.0, "", 0, "", true, 42, true, "comma", "json").unwrap();
        assert_eq!(a, b, "same seed → same split");
        assert!(a.contains("\"train\": 8"), "{a}");
        assert!(a.contains("\"test\": 2"), "{a}");
        let c = split(DATA, 0.2, 0.0, "", 0, "", true, 7, true, "comma", "json").unwrap();
        assert_ne!(a, c, "a different seed draws different rows");
    }

    #[test]
    fn sections_output_labels_each_split() {
        let out = split(DATA, 2.0, 1.0, "", 0, "", true, 1, true, "comma", "sections").unwrap();
        assert!(out.starts_with("# train (7 rows)\nid,label\n"), "{out}");
        assert!(out.contains("\n# validation (1 row)\nid,label\n"), "{out}");
        assert!(out.contains("\n# test (2 rows)\nid,label\n"), "{out}");
    }

    #[test]
    fn sequential_split_keeps_file_order() {
        // shuffle off: train = first rows, validation = middle, test = last.
        let train = split(DATA, 0.2, 0.1, "", 0, "", false, 42, true, "comma", "train").unwrap();
        assert_eq!(train, "id,label\n1,a\n2,b\n3,a\n4,b\n5,a\n6,b\n7,a\n");
        let val = split(DATA, 0.2, 0.1, "", 0, "", false, 42, true, "comma", "validation").unwrap();
        assert_eq!(val, "id,label\n8,b\n");
        let test = split(DATA, 0.2, 0.1, "", 0, "", false, 42, true, "comma", "test").unwrap();
        assert_eq!(test, "id,label\n9,a\n10,b\n");
    }

    #[test]
    fn absolute_counts_and_fractions_agree() {
        let by_fraction = split(DATA, 0.3, 0.0, "", 0, "", true, 5, true, "comma", "test").unwrap();
        let by_count = split(DATA, 3.0, 0.0, "", 0, "", true, 5, true, "comma", "test").unwrap();
        assert_eq!(by_fraction, by_count);
        assert_eq!(rows_of(&by_fraction), 3);
    }

    #[test]
    fn stratified_keeps_class_balance() {
        // 5 'a' and 5 'b'; a 40% test set takes 2 of each.
        let test = split(DATA, 0.4, 0.0, "label", 0, "", true, 3, true, "comma", "test").unwrap();
        let a = test.lines().skip(1).filter(|l| l.ends_with(",a")).count();
        let b = test.lines().skip(1).filter(|l| l.ends_with(",b")).count();
        assert_eq!((a, b), (2, 2));
        let train = split(DATA, 0.4, 0.0, "label", 0, "", true, 3, true, "comma", "train").unwrap();
        assert_eq!(rows_of(&train), 6);
    }

    #[test]
    fn stratified_three_way_never_double_assigns() {
        let out = split(DATA, 0.2, 0.2, "label", 0, "", true, 11, true, "comma", "json").unwrap();
        assert!(out.contains("\"train\": 6"), "{out}");
        assert!(out.contains("\"validation\": 2"), "{out}");
        assert!(out.contains("\"test\": 2"), "{out}");
    }

    #[test]
    fn stratify_bins_buckets_a_numeric_column() {
        let d = "id,score\n1,10\n2,20\n3,30\n4,40\n5,50\n6,60\n7,70\n8,80";
        let summary = split(d, 0.25, 0.0, "score", 2, "", true, 4, true, "comma", "summary")
            .unwrap();
        assert!(summary.starts_with("split,rows,percent\ntrain,6,75.0\ntest,2,25.0\n"), "{summary}");
        // One row from each of the two quantile bins (low half / high half).
        let test = split(d, 0.25, 0.0, "score", 2, "", true, 4, true, "comma", "test").unwrap();
        let scores: Vec<i32> = test
            .lines()
            .skip(1)
            .map(|l| l.split(',').nth(1).unwrap().parse().unwrap())
            .collect();
        assert_eq!(scores.len(), 2);
        assert!(scores.iter().any(|s| *s <= 40), "{test}");
        assert!(scores.iter().any(|s| *s > 40), "{test}");
    }

    #[test]
    fn grouped_split_keeps_a_group_together() {
        let d = "patient,visit\np1,1\np1,2\np2,1\np2,2\np3,1\np3,2\np4,1\np4,2";
        let train = split(d, 0.25, 0.0, "", 0, "patient", true, 9, true, "comma", "train").unwrap();
        let test = split(d, 0.25, 0.0, "", 0, "patient", true, 9, true, "comma", "test").unwrap();
        let ids = |csv: &str| -> Vec<String> {
            csv.lines()
                .skip(1)
                .map(|l| l.split(',').next().unwrap().to_string())
                .collect()
        };
        let (tr, te) = (ids(&train), ids(&test));
        assert_eq!(te.len(), 2, "one whole 2-visit patient in test");
        assert!(te[0] == te[1], "both test rows are the same patient");
        assert!(!tr.contains(&te[0]), "no patient appears in both splits");
    }

    #[test]
    fn summary_reports_counts_and_per_class_balance() {
        let out = split(DATA, 0.2, 0.2, "label", 0, "", true, 2, true, "comma", "summary").unwrap();
        assert!(out.contains("train,6,60.0\n"), "{out}");
        assert!(out.contains("validation,2,20.0\n"), "{out}");
        assert!(out.contains("test,2,20.0\n"), "{out}");
        assert!(out.contains("\nsplit,class,rows,percent\n"), "{out}");
        assert!(out.contains("test,a,1,20.0\n"), "{out}");
    }

    #[test]
    fn no_header_uses_column_indexes_and_keeps_every_row() {
        let d = "1,a\n2,b\n3,a\n4,b";
        let out = split(d, 0.5, 0.0, "2", 0, "", true, 6, false, "comma", "json").unwrap();
        assert!(out.contains("\"train\": 2"), "{out}");
        assert!(out.contains("\"test\": 2"), "{out}");
        assert!(out.contains("\"total\": 4"), "{out}");
    }

    #[test]
    fn tab_delimiter_round_trips() {
        let d = "id\tlabel\n1\ta\n2\tb\n3\ta\n4\tb";
        let test = split(d, 0.5, 0.0, "", 0, "", false, 42, true, "tab", "test").unwrap();
        assert_eq!(test, "id\tlabel\n3\ta\n4\tb\n");
    }

    #[test]
    fn every_row_lands_in_exactly_one_split() {
        let out = split(DATA, 0.3, 0.3, "", 0, "", true, 13, true, "comma", "sections").unwrap();
        let mut ids: Vec<&str> = out
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty() && *l != "id,label")
            .map(|l| l.split(',').next().unwrap())
            .collect();
        ids.sort_by_key(|s| s.parse::<usize>().unwrap());
        assert_eq!(ids, ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"]);
    }

    #[test]
    fn errors() {
        // empty input
        assert!(split("   ", 0.2, 0.0, "", 0, "", true, 42, true, "comma", "sections").is_err());
        // header only, no data rows
        assert!(split("id,label", 0.2, 0.0, "", 0, "", true, 42, true, "comma", "sections").is_err());
        // sizes take every row
        let e = split(DATA, 0.5, 0.5, "", 0, "", true, 42, true, "comma", "sections").unwrap_err();
        assert!(e.contains("0 training rows"), "{e}");
        // sizes overflow the row count
        let e = split(DATA, 8.0, 8.0, "", 0, "", true, 42, true, "comma", "sections").unwrap_err();
        assert!(e.contains("exceed the 10 data rows"), "{e}");
        // stratify + no shuffle
        let e = split(DATA, 0.2, 0.0, "label", 0, "", false, 42, true, "comma", "test").unwrap_err();
        assert!(e.contains("shuffle"), "{e}");
        // stratify + group together
        let e =
            split(DATA, 0.2, 0.0, "label", 0, "id", true, 42, true, "comma", "test").unwrap_err();
        assert!(e.contains("cannot be used together"), "{e}");
        // unknown column
        let e = split(DATA, 0.2, 0.0, "nope", 0, "", true, 42, true, "comma", "test").unwrap_err();
        assert!(e.contains("not found in the header row"), "{e}");
        // bins on a non-numeric column
        let e = split(DATA, 0.2, 0.0, "label", 3, "", true, 42, true, "comma", "test").unwrap_err();
        assert!(e.contains("numeric stratify column"), "{e}");
        // bins without a stratify column
        assert!(split(DATA, 0.2, 0.0, "", 3, "", true, 42, true, "comma", "test").is_err());
        // validation output with no validation set
        let e =
            split(DATA, 0.2, 0.0, "", 0, "", true, 42, true, "comma", "validation").unwrap_err();
        assert!(e.contains("validation_size"), "{e}");
        // bad output mode
        assert!(split(DATA, 0.2, 0.0, "", 0, "", true, 42, true, "comma", "nope").is_err());
        // bad delimiter
        assert!(split(DATA, 0.2, 0.0, "", 0, "", true, 42, true, ";;", "test").is_err());
    }
}
