//! percentile-rank-calculator core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Given a reference dataset and one or more target values, report where each target
//! falls in the distribution: its percentile rank (four standard tie-handling methods),
//! how many dataset values sit below/equal/above it, its quartile, and its z-score.

/// Maximum dataset size accepted in one run.
pub const MAX_DATA_POINTS: usize = 10_000;
/// Maximum number of target values ranked in one run.
pub const MAX_VALUES: usize = 100;
/// Maximum rounding precision for reported statistics.
pub const MAX_DECIMALS: u32 = 6;

/// Tie-handling convention used to turn counts into a percentile rank.
/// The four methods match SciPy's `percentileofscore(kind=…)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// `count(<= x) / n` — the formula used by most online percentile-rank calculators.
    Weak,
    /// `count(< x) / n`.
    Strict,
    /// Midpoint of strict and weak — splits ties evenly.
    Mean,
    /// Average ranking of tied values (SciPy `rank`).
    Rank,
}

impl Method {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "weak" => Ok(Method::Weak),
            "strict" => Ok(Method::Strict),
            "mean" => Ok(Method::Mean),
            "rank" => Ok(Method::Rank),
            other => Err(format!(
                "Unknown method '{other}'. Use weak, strict, mean, or rank."
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Method::Weak => "weak",
            Method::Strict => "strict",
            Method::Mean => "mean",
            Method::Rank => "rank",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Method::Weak => "share of values less than or equal to the target",
            Method::Strict => "share of values strictly less than the target",
            Method::Mean => "midpoint of the strict and weak results, so ties split evenly",
            Method::Rank => "average ranking of tied values, as in SciPy percentileofscore",
        }
    }
}

/// Report options.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub method: Method,
    pub decimals: u32,
    pub include_stats: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            method: Method::Weak,
            decimals: 2,
            include_stats: true,
        }
    }
}

/// Validate a caller-supplied rounding precision (surfaces send it as a signed int/string).
pub fn decimals_from(v: i64) -> Result<u32, String> {
    if !(0..=MAX_DECIMALS as i64).contains(&v) {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS}; got {v}."
        ));
    }
    Ok(v as u32)
}

/// One target value's position inside the reference dataset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ranked {
    pub value: f64,
    pub percentile: f64,
    pub below: usize,
    pub equal: usize,
    pub above: usize,
}

/// Split a free-form list of numbers on commas, semicolons, and any whitespace.
/// `label` names the field in error messages.
pub fn parse_numbers(input: &str, label: &str) -> Result<Vec<f64>, String> {
    let mut out = Vec::new();
    for token in input
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|t| !t.is_empty())
    {
        let v: f64 = token.parse().map_err(|_| {
            format!("'{token}' in the {label} is not a number. Separate numbers with commas, spaces, semicolons, or newlines.")
        })?;
        if !v.is_finite() {
            return Err(format!(
                "'{token}' in the {label} is not a finite number. NaN and infinity are not accepted."
            ));
        }
        out.push(v);
    }
    Ok(out)
}

/// Count of dataset values strictly below / equal to / above `x`, on a sorted slice.
pub fn counts(sorted: &[f64], x: f64) -> (usize, usize, usize) {
    let left = sorted.partition_point(|&v| v < x);
    let right = sorted.partition_point(|&v| v <= x);
    (left, right - left, sorted.len() - right)
}

/// Percentile rank of `x` within the sorted dataset, 0..=100.
pub fn percentile_rank(sorted: &[f64], x: f64, method: Method) -> f64 {
    let n = sorted.len() as f64;
    if n == 0.0 {
        return f64::NAN;
    }
    let (below, equal, _) = counts(sorted, x);
    let left = below as f64;
    let right = (below + equal) as f64;
    match method {
        Method::Weak => right * 100.0 / n,
        Method::Strict => left * 100.0 / n,
        Method::Mean => (left + right) * 50.0 / n,
        Method::Rank => (left + right + if equal > 0 { 1.0 } else { 0.0 }) * 50.0 / n,
    }
}

/// Rank every target value against the sorted dataset.
pub fn rank_all(sorted: &[f64], targets: &[f64], method: Method) -> Vec<Ranked> {
    targets
        .iter()
        .map(|&x| {
            let (below, equal, above) = counts(sorted, x);
            Ranked {
                value: x,
                percentile: percentile_rank(sorted, x, method),
                below,
                equal,
                above,
            }
        })
        .collect()
}

/// Linear-interpolation percentile (numpy default / Excel PERCENTILE.INC), `p` in 0.0..=1.0.
pub fn percentile_value(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    if n == 1 {
        return sorted[0];
    }
    let rank = p * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] + (rank - lo as f64) * (sorted[hi] - sorted[lo])
    }
}

/// Quartile the percentile rank falls into.
pub fn quartile_label(p: f64) -> &'static str {
    if p < 25.0 {
        "Q1"
    } else if p < 50.0 {
        "Q2"
    } else if p < 75.0 {
        "Q3"
    } else {
        "Q4"
    }
}

fn fmt_num(v: f64, decimals: u32) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    let s = format!("{:.*}", decimals as usize, v);
    let t = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if t == "-0" {
        "0".to_string()
    } else {
        t
    }
}

/// Echo a raw input value without the report's rounding.
fn fmt_value(v: f64) -> String {
    fmt_num(v, 10)
}

/// Build the full human-readable percentile-rank report.
pub fn report(data: &str, values: &str, opts: &Options) -> Result<String, String> {
    if opts.decimals > MAX_DECIMALS {
        return Err(format!(
            "decimals must be between 0 and {MAX_DECIMALS}; got {}.",
            opts.decimals
        ));
    }

    let nums = parse_numbers(data, "dataset")?;
    if nums.is_empty() {
        return Err("The dataset is empty. Enter at least one number, separated by commas, spaces, semicolons, or newlines.".to_string());
    }
    if nums.len() > MAX_DATA_POINTS {
        return Err(format!(
            "The dataset has {} numbers; the limit is {MAX_DATA_POINTS} per run.",
            nums.len()
        ));
    }

    let targets = parse_numbers(values, "values to rank")?;
    if targets.is_empty() {
        return Err(
            "No value to rank. Enter one or more numbers to locate inside the dataset.".to_string(),
        );
    }
    if targets.len() > MAX_VALUES {
        return Err(format!(
            "You asked to rank {} values; the limit is {MAX_VALUES} per run.",
            targets.len()
        ));
    }

    let mut sorted = nums.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("values are finite"));
    let n = sorted.len();
    let nf = n as f64;
    let sum: f64 = sorted.iter().sum();
    let mean = sum / nf;
    let sd = if n > 1 {
        (sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (nf - 1.0)).sqrt()
    } else {
        f64::NAN
    };

    let d = opts.decimals;
    let mut out = String::new();
    out.push_str(&format!(
        "Percentile rank — {} method ({}), N = {}\n\n",
        opts.method.label(),
        opts.method.blurb(),
        n
    ));

    for r in rank_all(&sorted, &targets, opts.method) {
        let z = if sd.is_finite() && sd > 0.0 {
            fmt_num((r.value - mean) / sd, d)
        } else {
            "n/a".to_string()
        };
        out.push_str(&format!(
            "{} → {}  (below: {}, equal: {}, above: {}, quartile: {}, z: {})\n",
            fmt_value(r.value),
            fmt_num(r.percentile, d),
            r.below,
            r.equal,
            r.above,
            quartile_label(r.percentile),
            z
        ));
    }

    if opts.include_stats {
        let q1 = percentile_value(&sorted, 0.25);
        let median = percentile_value(&sorted, 0.5);
        let q3 = percentile_value(&sorted, 0.75);
        out.push_str(&format!(
            "\nDataset summary\n  n = {}   min = {}   max = {}   range = {}\n  mean = {}   median = {}   sd (sample) = {}\n  Q1 = {}   Q3 = {}   IQR = {}\n",
            n,
            fmt_num(sorted[0], d),
            fmt_num(sorted[n - 1], d),
            fmt_num(sorted[n - 1] - sorted[0], d),
            fmt_num(mean, d),
            fmt_num(median, d),
            fmt_num(sd, d),
            fmt_num(q1, d),
            fmt_num(q3, d),
            fmt_num(q3 - q1, d),
        ));
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted_of(v: &[f64]) -> Vec<f64> {
        let mut s = v.to_vec();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        s
    }

    #[test]
    fn weak_method_matches_the_common_online_formula() {
        // 11 of 17 values are <= 25 → 64.705…%
        let s = sorted_of(&[
            6.0, 12.0, 13.0, 17.0, 17.0, 18.0, 20.0, 23.0, 24.0, 24.0, 25.0, 26.0, 27.0, 27.0,
            30.0, 32.0, 33.0,
        ]);
        let p = percentile_rank(&s, 25.0, Method::Weak);
        assert!((p - 1100.0 / 17.0).abs() < 1e-9, "got {p}");
    }

    #[test]
    fn all_four_methods_split_ties_as_documented() {
        let s = sorted_of(&[1.0, 2.0, 2.0, 2.0, 5.0]);
        assert_eq!(percentile_rank(&s, 2.0, Method::Strict), 20.0);
        assert_eq!(percentile_rank(&s, 2.0, Method::Weak), 80.0);
        assert_eq!(percentile_rank(&s, 2.0, Method::Mean), 50.0);
        assert_eq!(percentile_rank(&s, 2.0, Method::Rank), 60.0);
        // With no tie, mean and rank agree.
        assert_eq!(percentile_rank(&s, 3.0, Method::Mean), 80.0);
        assert_eq!(percentile_rank(&s, 3.0, Method::Rank), 80.0);
    }

    #[test]
    fn values_outside_the_dataset_clamp_to_the_ends() {
        let s = sorted_of(&[10.0, 20.0, 30.0]);
        assert_eq!(percentile_rank(&s, 0.0, Method::Weak), 0.0);
        assert_eq!(percentile_rank(&s, 99.0, Method::Weak), 100.0);
        assert_eq!(counts(&s, 0.0), (0, 0, 3));
        assert_eq!(counts(&s, 99.0), (3, 0, 0));
    }

    #[test]
    fn parses_mixed_separators_and_negatives() {
        let v = parse_numbers("1, 2;3\n-4\t5.5", "dataset").unwrap();
        assert_eq!(v, vec![1.0, 2.0, 3.0, -4.0, 5.5]);
    }

    #[test]
    fn report_happy_path_is_exact() {
        let got = report(
            "6, 12, 13, 17, 17, 18, 20, 23, 24, 24, 25, 26, 27, 27, 30, 32, 33",
            "25",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(
            got,
            "Percentile rank — weak method (share of values less than or equal to the target), N = 17\n\
             \n\
             25 → 64.71  (below: 10, equal: 1, above: 6, quartile: Q3, z: 0.41)\n\
             \n\
             Dataset summary\n  \
             n = 17   min = 6   max = 33   range = 27\n  \
             mean = 22   median = 24   sd (sample) = 7.4\n  \
             Q1 = 17   Q3 = 27   IQR = 10"
        );
    }

    #[test]
    fn report_ranks_several_values_and_can_hide_stats() {
        let got = report(
            "10 20 30 40",
            "15, 40",
            &Options {
                method: Method::Mean,
                decimals: 1,
                include_stats: false,
            },
        )
        .unwrap();
        assert_eq!(
            got,
            "Percentile rank — mean method (midpoint of the strict and weak results, so ties split evenly), N = 4\n\
             \n\
             15 → 25  (below: 1, equal: 0, above: 3, quartile: Q2, z: -0.8)\n\
             40 → 87.5  (below: 3, equal: 1, above: 0, quartile: Q4, z: 1.2)"
        );
    }

    #[test]
    fn single_value_dataset_has_no_z_score() {
        let got = report("42", "42", &Options::default()).unwrap();
        assert!(got.contains("z: n/a"), "got {got}");
        assert!(got.contains("sd (sample) = n/a"), "got {got}");
    }

    #[test]
    fn empty_dataset_is_an_error() {
        let err = report("   ", "5", &Options::default()).unwrap_err();
        assert!(err.contains("dataset is empty"), "got {err}");
    }

    #[test]
    fn non_numeric_token_is_an_error_naming_the_token() {
        let err = report("1, 2, abc", "2", &Options::default()).unwrap_err();
        assert!(
            err.contains("'abc'") && err.contains("dataset"),
            "got {err}"
        );
    }

    #[test]
    fn missing_target_value_is_an_error() {
        let err = report("1, 2, 3", "", &Options::default()).unwrap_err();
        assert!(err.contains("No value to rank"), "got {err}");
    }

    #[test]
    fn dataset_cap_is_enforced_at_the_boundary() {
        let ok: String = (0..MAX_DATA_POINTS)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert!(report(&ok, "5", &Options::default()).is_ok());
        let too_many = format!("{ok},1");
        let err = report(&too_many, "5", &Options::default()).unwrap_err();
        assert!(err.contains("10001") && err.contains("limit"), "got {err}");
    }

    #[test]
    fn too_many_targets_is_an_error() {
        let many: String = (0..=MAX_VALUES)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let err = report("1,2,3", &many, &Options::default()).unwrap_err();
        assert!(err.contains("limit is 100"), "got {err}");
    }

    #[test]
    fn unknown_method_is_an_error() {
        let err = Method::parse("median").unwrap_err();
        assert!(err.contains("Unknown method"), "got {err}");
    }

    #[test]
    fn decimals_above_the_cap_are_rejected() {
        let err = report(
            "1,2,3",
            "2",
            &Options {
                decimals: 7,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("decimals must be"), "got {err}");
    }
}
