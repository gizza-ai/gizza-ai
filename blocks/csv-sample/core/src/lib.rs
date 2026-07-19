//! gizza-ai/csv-sample core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Takes a random, systematic,
//! top-N, bottom-N, or stratified sample of the DATA rows of a CSV.
//!
//! Randomness is a small in-house seeded PRNG (splitmix64), so every surface —
//! chat, CLI, page, tests — is deterministic for a given `seed`; change `seed`
//! for a different draw. No OS RNG / `getrandom` dependency.

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

fn resolve_column(name: &str, header: Option<&csv::StringRecord>) -> Result<usize, String> {
    let name = name.trim();
    if let Ok(n) = name.parse::<usize>() {
        if n == 0 {
            return Err("stratify_column index is 1-based (>= 1)".into());
        }
        return Ok(n - 1);
    }
    match header {
        Some(h) => h
            .iter()
            .position(|c| c == name)
            .ok_or_else(|| format!("stratify_column '{name}' not found in the header")),
        None => Err(format!(
            "stratify_column '{name}' is not a number and there is no header to match names"
        )),
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

/// Pick `k` of the `m` positions `0..m` at random (seeded), returned in ascending
/// original order so the sample keeps the file's row order.
fn pick_random(m: usize, k: usize, rng: &mut Rng) -> Vec<usize> {
    let k = k.min(m);
    let mut idx: Vec<usize> = (0..m).collect();
    shuffle(&mut idx, rng);
    idx.truncate(k);
    idx.sort_unstable();
    idx
}

/// How many rows to sample: `percent` (0..=100) wins when > 0, else `n`. Result
/// is clamped to `total`.
fn target_count(total: usize, n: usize, percent: f64) -> usize {
    if percent > 0.0 {
        let p = percent.min(100.0);
        ((total as f64) * p / 100.0).round() as usize
    } else {
        n
    }
    .min(total)
}

/// Systematic sample: after a seeded random start in `0..step`, take every
/// `step`-th row, where `step = round(total / k)`.
fn pick_systematic(total: usize, k: usize, rng: &mut Rng) -> Vec<usize> {
    let k = k.min(total);
    if k == 0 {
        return vec![];
    }
    if k == total {
        return (0..total).collect();
    }
    let step = ((total as f64) / (k as f64)).round().max(1.0) as usize;
    let start = rng.below(step);
    let mut out = Vec::with_capacity(k);
    let mut i = start;
    while i < total && out.len() < k {
        out.push(i);
        i += step;
    }
    // If rounding left us short, backfill from the front with unused indices.
    if out.len() < k {
        let mut used: Vec<bool> = vec![false; total];
        for &p in &out {
            used[p] = true;
        }
        for (p, u) in used.iter().enumerate() {
            if out.len() >= k {
                break;
            }
            if !*u {
                out.push(p);
            }
        }
        out.sort_unstable();
    }
    out
}

/// Proportional stratified sample over the value of `col`. Each stratum gets
/// `floor(k * size/total)` rows; the leftover is handed out by largest
/// fractional remainder (capped at each stratum's size) so the total is exactly
/// `k` when `k <= total`. Rows within a stratum are chosen with the seeded PRNG.
/// The final selection keeps original row order.
fn pick_stratified(rows: &[csv::StringRecord], col: usize, k: usize, rng: &mut Rng) -> Vec<usize> {
    let total = rows.len();
    let k = k.min(total);
    if k == 0 {
        return vec![];
    }
    // Group row indices by the stratify-column value, preserving first-seen order.
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        let key = r.get(col).unwrap_or("").to_string();
        groups
            .entry(key.clone())
            .or_insert_with(|| {
                order.push(key.clone());
                Vec::new()
            })
            .push(i);
    }
    // Largest-remainder allocation per stratum: (key, base_alloc, fractional_part).
    let mut alloc: Vec<(String, usize, f64)> = Vec::with_capacity(order.len());
    let mut assigned = 0usize;
    for key in &order {
        let size = groups[key].len();
        let exact = (k as f64) * (size as f64) / (total as f64);
        let base = (exact.floor() as usize).min(size);
        assigned += base;
        alloc.push((key.clone(), base, exact - exact.floor()));
    }
    // Distribute the remainder by descending fractional part, respecting caps.
    let mut remaining = k.saturating_sub(assigned);
    if remaining > 0 {
        let mut ord: Vec<usize> = (0..alloc.len()).collect();
        ord.sort_by(|&a, &b| {
            alloc[b]
                .2
                .partial_cmp(&alloc[a].2)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        loop {
            let mut placed_any = false;
            for &gi in &ord {
                if remaining == 0 {
                    break;
                }
                let size = groups[&alloc[gi].0].len();
                if alloc[gi].1 < size {
                    alloc[gi].1 += 1;
                    remaining -= 1;
                    placed_any = true;
                }
            }
            if remaining == 0 || !placed_any {
                break;
            }
        }
    }
    // Sample within each stratum and collect.
    let mut chosen: Vec<usize> = Vec::with_capacity(k);
    for (key, take, _) in &alloc {
        let members = &groups[key];
        let picks = pick_random(members.len(), *take, rng);
        for p in picks {
            chosen.push(members[p]);
        }
    }
    chosen.sort_unstable();
    chosen
}

/// Take a sample of the CSV's data rows. `method` is one of
/// `random | systematic | top | bottom | stratified`. The header row (when
/// `has_header`) is always emitted first. Returns the sampled CSV.
#[allow(clippy::too_many_arguments)]
pub fn sample(
    data: &str,
    method: &str,
    n: usize,
    percent: f64,
    stratify_column: &str,
    seed: u64,
    has_header: bool,
    delimiter: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let method = method.trim().to_ascii_lowercase();
    if !matches!(
        method.as_str(),
        "random" | "systematic" | "top" | "bottom" | "stratified"
    ) {
        return Err(format!(
            "method must be one of random, systematic, top, bottom, stratified (got '{method}')"
        ));
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
    if records.is_empty() {
        return Ok(String::new());
    }

    let (header, body): (Option<&csv::StringRecord>, &[csv::StringRecord]) = if has_header {
        (records.first(), &records[1..])
    } else {
        (None, &records[..])
    };
    let total = body.len();
    let k = target_count(total, n, percent);

    let mut rng = Rng::new(seed);
    let picks: Vec<usize> = match method.as_str() {
        "top" => (0..k.min(total)).collect(),
        "bottom" => (total.saturating_sub(k)..total).collect(),
        "random" => pick_random(total, k, &mut rng),
        "systematic" => pick_systematic(total, k, &mut rng),
        "stratified" => {
            if stratify_column.trim().is_empty() {
                return Err("stratified sampling needs a stratify_column".into());
            }
            let col = resolve_column(stratify_column, header)?;
            pick_stratified(body, col, k, &mut rng)
        }
        _ => unreachable!(),
    };

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);
    if let Some(h) = header {
        wtr.write_record(h)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for &i in &picks {
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

    const DATA: &str = "name,group\nA,x\nB,y\nC,x\nD,y\nE,x\nF,y";

    #[test]
    fn top_n_keeps_header_and_first_rows() {
        let out = sample(DATA, "top", 2, 0.0, "", 42, true, "comma").unwrap();
        assert_eq!(out, "name,group\nA,x\nB,y\n");
    }

    #[test]
    fn bottom_n_takes_last_rows() {
        let out = sample(DATA, "bottom", 2, 0.0, "", 42, true, "comma").unwrap();
        assert_eq!(out, "name,group\nE,x\nF,y\n");
    }

    #[test]
    fn random_is_deterministic_and_keeps_order() {
        let a = sample(DATA, "random", 3, 0.0, "", 7, true, "comma").unwrap();
        let b = sample(DATA, "random", 3, 0.0, "", 7, true, "comma").unwrap();
        assert_eq!(a, b, "same seed → same sample");
        // Header + exactly 3 data rows, in original file order.
        let lines: Vec<&str> = a.lines().collect();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "name,group");
        let idx: Vec<usize> = lines[1..]
            .iter()
            .map(|l| "ABCDEF".find(l.chars().next().unwrap()).unwrap())
            .collect();
        let mut sorted = idx.clone();
        sorted.sort_unstable();
        assert_eq!(idx, sorted, "sample rows keep original order");
        // A different seed can change the draw.
        let c = sample(DATA, "random", 3, 0.0, "", 99, true, "comma").unwrap();
        assert_ne!(a, c);
    }

    #[test]
    fn percent_overrides_n() {
        // 6 data rows, 50% → 3 rows.
        let out = sample(DATA, "top", 1, 50.0, "", 42, true, "comma").unwrap();
        assert_eq!(out.lines().count(), 4); // header + 3
    }

    #[test]
    fn stratified_is_proportional() {
        // 3 x-rows, 3 y-rows; want 4 → 2 from each stratum.
        let out = sample(DATA, "stratified", 4, 0.0, "group", 1, true, "comma").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 5); // header + 4
        let x = lines[1..].iter().filter(|l| l.ends_with(",x")).count();
        let y = lines[1..].iter().filter(|l| l.ends_with(",y")).count();
        assert_eq!(x, 2);
        assert_eq!(y, 2);
    }

    #[test]
    fn systematic_spacing() {
        // 6 rows, want 3 → step 2, seeded start; exactly 3 rows in order.
        let out = sample(DATA, "systematic", 3, 0.0, "", 5, true, "comma").unwrap();
        assert_eq!(out.lines().count(), 4);
    }

    #[test]
    fn n_larger_than_rows_returns_all() {
        let out = sample(DATA, "random", 100, 0.0, "", 42, true, "comma").unwrap();
        assert_eq!(out.lines().count(), 7); // header + all 6
    }

    #[test]
    fn no_header_uses_index_column() {
        let d = "A,x\nB,y\nC,x\nD,y";
        let out = sample(d, "stratified", 2, 0.0, "2", 3, false, "comma").unwrap();
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn errors() {
        assert!(sample("  ", "random", 2, 0.0, "", 42, true, "comma").is_err()); // empty
        assert!(sample(DATA, "nope", 2, 0.0, "", 42, true, "comma").is_err()); // bad method
        assert!(sample(DATA, "stratified", 2, 0.0, "", 42, true, "comma").is_err()); // no column
        assert!(sample(DATA, "stratified", 2, 0.0, "missing", 42, true, "comma").is_err()); // bad column
    }
}
