//! combinations-permutations core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Counts nCr / nPr (with or without repetition, plus circular permutations) using exact
//! arbitrary-precision arithmetic, and optionally enumerates the actual selections of a pool.

// ---------------------------------------------------------------------------
// Minimal unsigned bignum (base 1e9 limbs, little-endian).
//
// Counts here routinely exceed u128: C(1000, 500) has 299 digits. Only the four
// operations the combinatorics needs are implemented — multiply by a small
// scalar, exact-divide by a small scalar, compare, and print.
// ---------------------------------------------------------------------------

const BASE: u64 = 1_000_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Big {
    /// Least-significant limb first; empty means zero (no trailing zero limbs).
    limbs: Vec<u32>,
}

impl Big {
    pub fn zero() -> Self {
        Big { limbs: Vec::new() }
    }

    pub fn from_u64(mut v: u64) -> Self {
        let mut limbs = Vec::new();
        while v > 0 {
            limbs.push((v % BASE) as u32);
            v /= BASE;
        }
        Big { limbs }
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// Multiply in place by a small scalar. `m` stays far below the u64 headroom
    /// (every caller passes a value bounded by MAX_N + MAX_R).
    fn mul_small(&mut self, m: u64) {
        if m == 0 || self.is_zero() {
            self.limbs.clear();
            return;
        }
        let mut carry: u64 = 0;
        for limb in self.limbs.iter_mut() {
            let cur = (*limb as u64) * m + carry;
            *limb = (cur % BASE) as u32;
            carry = cur / BASE;
        }
        while carry > 0 {
            self.limbs.push((carry % BASE) as u32);
            carry /= BASE;
        }
    }

    /// Divide in place by a small scalar. Every call site divides a value that is
    /// mathematically an exact multiple of `d`, so any remainder would be a bug.
    fn div_small(&mut self, d: u64) {
        debug_assert!(d > 0);
        let mut rem: u64 = 0;
        for limb in self.limbs.iter_mut().rev() {
            let cur = rem * BASE + (*limb as u64);
            *limb = (cur / d) as u32;
            rem = cur % d;
        }
        debug_assert_eq!(rem, 0, "combinatorial division is always exact");
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    /// `Some(v)` when the value fits in a u64, else `None`.
    pub fn to_u64(&self) -> Option<u64> {
        let mut out: u64 = 0;
        for &limb in self.limbs.iter().rev() {
            out = out.checked_mul(BASE)?.checked_add(limb as u64)?;
        }
        Some(out)
    }

    /// Approximate value, `inf` once the magnitude leaves the f64 range.
    fn to_f64(&self) -> f64 {
        let mut out = 0.0f64;
        for &limb in self.limbs.iter().rev() {
            out = out * BASE as f64 + limb as f64;
        }
        out
    }
}

impl std::fmt::Display for Big {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.limbs.is_empty() {
            return write!(f, "0");
        }
        let mut it = self.limbs.iter().rev();
        write!(f, "{}", it.next().unwrap())?;
        for limb in it {
            write!(f, "{limb:09}")?;
        }
        Ok(())
    }
}

/// `2598960` -> `2,598,960`.
fn group_digits(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(b as char);
    }
    out
}

fn factorial(k: u64) -> Big {
    let mut out = Big::from_u64(1);
    for i in 2..=k {
        out.mul_small(i);
    }
    out
}

/// n! / (r! * (n - r)!), built multiplicatively so nothing ever overflows a step.
fn n_choose_r(n: u64, r: u64) -> Big {
    if r > n {
        return Big::zero();
    }
    let r = r.min(n - r);
    let mut out = Big::from_u64(1);
    for i in 0..r {
        out.mul_small(n - i);
        out.div_small(i + 1);
    }
    out
}

/// n! / (n - r)!
fn n_perm_r(n: u64, r: u64) -> Big {
    if r > n {
        return Big::zero();
    }
    let mut out = Big::from_u64(1);
    for i in 0..r {
        out.mul_small(n - i);
    }
    out
}

fn pow(n: u64, r: u64) -> Big {
    let mut out = Big::from_u64(1);
    for _ in 0..r {
        out.mul_small(n);
    }
    out
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Largest `n` / `r` accepted. Counting past this is still instant, but the cap
/// keeps a pathological request (a million-digit power) from stalling a tab.
pub const MAX_N: u64 = 1000;
pub const MAX_R: u64 = 1000;
/// Hard ceiling on `max_results`, whatever the caller asks for.
pub const MAX_RESULTS_CAP: u64 = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Combinations,
    Permutations,
    CircularPermutations,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Summary,
    Count,
    Lines,
    Csv,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemSeparator {
    Auto,
    Comma,
    Newline,
    Semicolon,
    Pipe,
    Tab,
    Space,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinSeparator {
    Comma,
    Space,
    None,
    Dash,
    Underscore,
    Pipe,
    Slash,
    Plus,
    Dot,
    Custom,
}

pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim() {
        "" | "combinations" => Ok(Mode::Combinations),
        "permutations" => Ok(Mode::Permutations),
        "circular_permutations" => Ok(Mode::CircularPermutations),
        other => Err(format!(
            "unknown mode '{other}' — expected combinations, permutations or circular_permutations"
        )),
    }
}

pub fn parse_output_format(s: &str) -> Result<OutputFormat, String> {
    match s.trim() {
        "" | "summary" => Ok(OutputFormat::Summary),
        "count" => Ok(OutputFormat::Count),
        "lines" => Ok(OutputFormat::Lines),
        "csv" => Ok(OutputFormat::Csv),
        "json" => Ok(OutputFormat::Json),
        other => Err(format!(
            "unknown output_format '{other}' — expected summary, count, lines, csv or json"
        )),
    }
}

pub fn parse_item_separator(s: &str) -> Result<ItemSeparator, String> {
    match s.trim() {
        "" | "auto" => Ok(ItemSeparator::Auto),
        "comma" => Ok(ItemSeparator::Comma),
        "newline" => Ok(ItemSeparator::Newline),
        "semicolon" => Ok(ItemSeparator::Semicolon),
        "pipe" => Ok(ItemSeparator::Pipe),
        "tab" => Ok(ItemSeparator::Tab),
        "space" => Ok(ItemSeparator::Space),
        other => Err(format!(
            "unknown item_separator '{other}' — expected auto, comma, newline, semicolon, pipe, tab or space"
        )),
    }
}

pub fn parse_join_separator(s: &str) -> Result<JoinSeparator, String> {
    match s.trim() {
        "" | "comma" => Ok(JoinSeparator::Comma),
        "space" => Ok(JoinSeparator::Space),
        "none" => Ok(JoinSeparator::None),
        "dash" => Ok(JoinSeparator::Dash),
        "underscore" => Ok(JoinSeparator::Underscore),
        "pipe" => Ok(JoinSeparator::Pipe),
        "slash" => Ok(JoinSeparator::Slash),
        "plus" => Ok(JoinSeparator::Plus),
        "dot" => Ok(JoinSeparator::Dot),
        "custom" => Ok(JoinSeparator::Custom),
        other => Err(format!(
            "unknown join_separator '{other}' — expected comma, space, none, dash, underscore, pipe, slash, plus, dot or custom"
        )),
    }
}

fn join_string(join: JoinSeparator, custom: &str) -> String {
    match join {
        JoinSeparator::Comma => ", ".into(),
        JoinSeparator::Space => " ".into(),
        JoinSeparator::None => String::new(),
        JoinSeparator::Dash => "-".into(),
        JoinSeparator::Underscore => "_".into(),
        JoinSeparator::Pipe => "|".into(),
        JoinSeparator::Slash => "/".into(),
        JoinSeparator::Plus => "+".into(),
        JoinSeparator::Dot => ".".into(),
        JoinSeparator::Custom => custom.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Item parsing
// ---------------------------------------------------------------------------

fn split_items(raw: &str, sep: ItemSeparator) -> Vec<String> {
    let chars: &[char] = match sep {
        ItemSeparator::Comma => &[','],
        ItemSeparator::Newline => &['\n', '\r'],
        ItemSeparator::Semicolon => &[';'],
        ItemSeparator::Pipe => &['|'],
        // A pasted spreadsheet column arrives tab- AND newline-separated.
        ItemSeparator::Tab => &['\t', '\n', '\r'],
        ItemSeparator::Space => &[' ', '\t', '\n', '\r'],
        ItemSeparator::Auto => {
            for probe in [
                ItemSeparator::Tab,
                ItemSeparator::Newline,
                ItemSeparator::Comma,
                ItemSeparator::Semicolon,
                ItemSeparator::Pipe,
            ] {
                let hit = match probe {
                    ItemSeparator::Tab => raw.contains('\t'),
                    ItemSeparator::Newline => raw.contains('\n'),
                    ItemSeparator::Comma => raw.contains(','),
                    ItemSeparator::Semicolon => raw.contains(';'),
                    _ => raw.contains('|'),
                };
                if hit {
                    return split_items(raw, probe);
                }
            }
            // A single space-separated line is the last resort, so "a b c" works
            // but "New York" alone stays one item.
            &[' ']
        }
    };
    raw.split(|c| chars.contains(&c))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn dedupe_items(items: Vec<String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    seen
}

// ---------------------------------------------------------------------------
// Counting
// ---------------------------------------------------------------------------

/// Exact number of selections for the given shape. `Err` for shapes the tool
/// deliberately does not model.
pub fn count(n: u64, r: u64, mode: Mode, repetition: bool) -> Result<Big, String> {
    match (mode, repetition) {
        (Mode::Combinations, false) => Ok(n_choose_r(n, r)),
        (Mode::Combinations, true) => {
            if r == 0 {
                return Ok(Big::from_u64(1));
            }
            Ok(n_choose_r(n + r - 1, r))
        }
        (Mode::Permutations, false) => Ok(n_perm_r(n, r)),
        (Mode::Permutations, true) => Ok(pow(n, r)),
        (Mode::CircularPermutations, true) => Err(
            "circular_permutations does not support repetition — counting circular arrangements \
             with repeats is necklace counting, which this tool does not compute. Turn repetition \
             off, or use mode=permutations."
                .into(),
        ),
        (Mode::CircularPermutations, false) => {
            if r == 0 {
                return Err(
                    "circular_permutations needs r of at least 1 — there is no circle of 0 items."
                        .into(),
                );
            }
            let chosen = n_choose_r(n, r);
            if chosen.is_zero() {
                return Ok(chosen);
            }
            // C(n, r) distinct seatings, each arranged in (r - 1)! rotation classes.
            Ok(mul_big(&chosen, &factorial(r - 1)))
        }
    }
}

/// Schoolbook multiply. Only used once (circular permutations), where both sides
/// are already-computed counts.
fn mul_big(a: &Big, b: &Big) -> Big {
    if a.is_zero() || b.is_zero() {
        return Big::zero();
    }
    let mut acc = vec![0u64; a.limbs.len() + b.limbs.len()];
    for (i, &x) in a.limbs.iter().enumerate() {
        let mut carry: u64 = 0;
        for (j, &y) in b.limbs.iter().enumerate() {
            let cur = acc[i + j] + (x as u64) * (y as u64) + carry;
            acc[i + j] = cur % BASE;
            carry = cur / BASE;
        }
        let mut k = i + b.limbs.len();
        while carry > 0 {
            let cur = acc[k] + carry;
            acc[k] = cur % BASE;
            carry = cur / BASE;
            k += 1;
        }
    }
    let mut limbs: Vec<u32> = acc.into_iter().map(|v| v as u32).collect();
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
    Big { limbs }
}

// ---------------------------------------------------------------------------
// Enumeration
// ---------------------------------------------------------------------------

fn each_selection<F: FnMut(&[usize])>(n: usize, r: usize, mode: Mode, repetition: bool, f: &mut F) {
    match (mode, repetition) {
        (Mode::Combinations, false) => each_combination_no_repetition(n, r, f),
        (Mode::Combinations, true) => {
            let mut idx = vec![0usize; r];
            loop {
                f(&idx);
                let mut i = r;
                loop {
                    if i == 0 {
                        return;
                    }
                    i -= 1;
                    if idx[i] != n - 1 {
                        break;
                    }
                }
                let v = idx[i] + 1;
                for j in i..r {
                    idx[j] = v;
                }
            }
        }
        (Mode::Permutations, true) => {
            let mut idx = vec![0usize; r];
            loop {
                f(&idx);
                let mut i = r;
                loop {
                    if i == 0 {
                        return;
                    }
                    i -= 1;
                    if idx[i] != n - 1 {
                        break;
                    }
                    idx[i] = 0;
                }
                idx[i] += 1;
            }
        }
        (Mode::Permutations, false) => {
            let mut used = vec![false; n];
            let mut cur: Vec<usize> = Vec::with_capacity(r);
            perm_rec(n, r, &mut used, &mut cur, f);
        }
        (Mode::CircularPermutations, _) => {
            // One representative per rotation class: keep the lowest-indexed
            // chosen item fixed at the front and permute the rest.
            each_combination_no_repetition(n, r, &mut |combo: &[usize]| {
                let rest: Vec<usize> = combo[1..].to_vec();
                let mut used = vec![false; rest.len()];
                let mut cur: Vec<usize> = Vec::with_capacity(rest.len());
                circle_rec(combo[0], &rest, &mut used, &mut cur, f);
            });
        }
    }
}

fn each_combination_no_repetition<F: FnMut(&[usize])>(n: usize, r: usize, f: &mut F) {
    let mut idx: Vec<usize> = (0..r).collect();
    loop {
        f(&idx);
        let mut i = r;
        loop {
            if i == 0 {
                return;
            }
            i -= 1;
            if idx[i] != i + n - r {
                break;
            }
        }
        idx[i] += 1;
        for j in i + 1..r {
            idx[j] = idx[j - 1] + 1;
        }
    }
}

fn perm_rec<F: FnMut(&[usize])>(
    n: usize,
    r: usize,
    used: &mut Vec<bool>,
    cur: &mut Vec<usize>,
    f: &mut F,
) {
    if cur.len() == r {
        f(cur);
        return;
    }
    for i in 0..n {
        if used[i] {
            continue;
        }
        used[i] = true;
        cur.push(i);
        perm_rec(n, r, used, cur, f);
        cur.pop();
        used[i] = false;
    }
}

fn circle_rec<F: FnMut(&[usize])>(
    head: usize,
    rest: &[usize],
    used: &mut Vec<bool>,
    cur: &mut Vec<usize>,
    f: &mut F,
) {
    if cur.len() == rest.len() {
        let mut row = Vec::with_capacity(cur.len() + 1);
        row.push(head);
        row.extend_from_slice(cur);
        f(&row);
        return;
    }
    for i in 0..rest.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        cur.push(rest[i]);
        circle_rec(head, rest, used, cur, f);
        cur.pop();
        used[i] = false;
    }
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn json_string(s: &str) -> String {
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

fn notation(n: u64, r: u64, mode: Mode, repetition: bool) -> String {
    match (mode, repetition) {
        (Mode::Combinations, false) => format!("C({n}, {r})"),
        (Mode::Combinations, true) => format!("C({n}, {r}) with repetition"),
        (Mode::Permutations, false) => format!("P({n}, {r})"),
        (Mode::Permutations, true) => format!("P({n}, {r}) with repetition"),
        (Mode::CircularPermutations, _) => format!("Circular P({n}, {r})"),
    }
}

fn formula(n: u64, r: u64, mode: Mode, repetition: bool) -> String {
    match (mode, repetition) {
        (Mode::Combinations, false) => format!(
            "C(n, r) = n! / (r! * (n - r)!) = {n}! / ({r}! * {}!)",
            n.saturating_sub(r)
        ),
        (Mode::Combinations, true) => {
            let top = n + r.saturating_sub(1);
            format!(
                "C(n + r - 1, r) = (n + r - 1)! / (r! * (n - 1)!) = {top}! / ({r}! * {}!)",
                n.saturating_sub(1)
            )
        }
        (Mode::Permutations, false) => {
            format!("P(n, r) = n! / (n - r)! = {n}! / {}!", n.saturating_sub(r))
        }
        (Mode::Permutations, true) => format!("n^r = {n}^{r}"),
        (Mode::CircularPermutations, _) => format!(
            "C(n, r) * (r - 1)! = C({n}, {r}) * {}!",
            r.saturating_sub(1)
        ),
    }
}

fn odds_line(total: &Big) -> String {
    if total.is_zero() {
        return "n/a (0 results)".into();
    }
    let digits = total.to_string();
    let f = total.to_f64();
    if f.is_finite() && f > 0.0 {
        let pct = 100.0 / f;
        let shown = if pct >= 0.0001 {
            format!("{pct:.6}%")
        } else {
            format!("{pct:.3e}%")
        };
        format!("1 in {} ({shown})", group_digits(&digits))
    } else {
        format!("1 in {}", group_digits(&digits))
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Count and/or enumerate. `items` wins over `n_param` when non-empty.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    items: &str,
    n_param: u64,
    r: u64,
    mode: Mode,
    repetition: bool,
    item_sep: ItemSeparator,
    dedupe: bool,
    out_format: OutputFormat,
    join: JoinSeparator,
    custom_join: &str,
    max_results: u64,
) -> Result<String, String> {
    if r > MAX_R {
        return Err(format!("r is {r}, above the maximum of {MAX_R}"));
    }

    let pool: Vec<String> = if items.trim().is_empty() {
        Vec::new()
    } else {
        let parsed = split_items(items, item_sep);
        let parsed = if dedupe { dedupe_items(parsed) } else { parsed };
        if parsed.is_empty() {
            return Err(
                "the item list is empty after trimming — expected something like 'a, b, c, d'"
                    .into(),
            );
        }
        parsed
    };

    let n = if pool.is_empty() {
        n_param
    } else {
        pool.len() as u64
    };
    if n == 0 {
        return Err(
            "expected either items (a pool such as 'a, b, c, d') or n of at least 1 (how many \
             items there are to choose from), but both were empty"
                .into(),
        );
    }
    if n > MAX_N {
        return Err(format!("n is {n}, above the maximum of {MAX_N}"));
    }
    if !pool.is_empty() && pool.len() as u64 > MAX_N {
        return Err(format!(
            "the pool has {} items, above the maximum of {MAX_N}",
            pool.len()
        ));
    }

    let total = count(n, r, mode, repetition)?;

    match out_format {
        OutputFormat::Count => Ok(total.to_string()),
        OutputFormat::Summary => {
            let mut out = format!(
                "{} = {}\n\n",
                notation(n, r, mode, repetition),
                group_digits(&total.to_string())
            );
            out.push_str(&format!(
                "Order matters: {}\n",
                match mode {
                    Mode::Combinations => "no",
                    _ => "yes",
                }
            ));
            out.push_str(&format!(
                "Repetition: {}\n",
                if repetition { "allowed" } else { "not allowed" }
            ));
            out.push_str(&format!("Formula: {}\n", formula(n, r, mode, repetition)));
            out.push_str(&format!(
                "Odds of one specific result: {}",
                odds_line(&total)
            ));
            if total.is_zero() {
                out.push_str(&format!(
                    "\nNote: r ({r}) is larger than n ({n}) and repetition is off, so no selection is possible."
                ));
            }
            if !pool.is_empty() {
                let shown: Vec<&str> = pool.iter().take(20).map(|s| s.as_str()).collect();
                let mut line = format!("\nPool ({} items): {}", pool.len(), shown.join(", "));
                if pool.len() > shown.len() {
                    line.push_str(&format!(", ... (+{} more)", pool.len() - shown.len()));
                }
                out.push_str(&line);
            }
            Ok(out)
        }
        OutputFormat::Lines | OutputFormat::Csv | OutputFormat::Json => {
            if r == 0 {
                return Err(
                    "r must be at least 1 to enumerate — use output_format=summary or count for \
                     the r = 0 case (there is exactly one empty selection)."
                        .into(),
                );
            }
            if total.is_zero() {
                return Err(format!(
                    "nothing to enumerate: r ({r}) is larger than n ({n}) and repetition is off. \
                     Lower r, raise n, or turn repetition on."
                ));
            }
            let cap = max_results.clamp(1, MAX_RESULTS_CAP);
            match total.to_u64() {
                Some(v) if v <= cap => {}
                _ => {
                    return Err(format!(
                        "{} = {} results, above max_results ({cap}). Raise max_results (hard cap \
                         {MAX_RESULTS_CAP}), narrow the input, or use output_format=count.",
                        notation(n, r, mode, repetition),
                        total
                    ))
                }
            }

            // With no pasted pool, enumerate over 1 .. n, which is what the
            // lottery/dice framing expects.
            let labels: Vec<String> = if pool.is_empty() {
                (1..=n).map(|i| i.to_string()).collect()
            } else {
                pool.clone()
            };

            let sep = join_string(join, custom_join);
            let mut out = String::new();
            let mut first = true;
            if out_format == OutputFormat::Json {
                out.push('[');
            }
            each_selection(
                labels.len(),
                r as usize,
                mode,
                repetition,
                &mut |sel: &[usize]| {
                    if !first {
                        out.push_str(if out_format == OutputFormat::Json {
                            ","
                        } else {
                            "\n"
                        });
                    }
                    first = false;
                    match out_format {
                        OutputFormat::Json => {
                            out.push('[');
                            for (i, &idx) in sel.iter().enumerate() {
                                if i > 0 {
                                    out.push(',');
                                }
                                out.push_str(&json_string(&labels[idx]));
                            }
                            out.push(']');
                        }
                        OutputFormat::Csv => {
                            let row: Vec<String> =
                                sel.iter().map(|&i| csv_field(&labels[i])).collect();
                            out.push_str(&row.join(","));
                        }
                        _ => {
                            let row: Vec<&str> = sel.iter().map(|&i| labels[i].as_str()).collect();
                            out.push_str(&row.join(&sep));
                        }
                    }
                },
            );
            if out_format == OutputFormat::Json {
                out.push(']');
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(n: u64, r: u64) -> String {
        count(n, r, Mode::Combinations, false).unwrap().to_string()
    }

    #[test]
    fn counts_match_textbook_values() {
        assert_eq!(c(10, 3), "120");
        assert_eq!(c(52, 5), "2598960");
        assert_eq!(c(49, 6), "13983816");
        assert_eq!(c(5, 0), "1");
        assert_eq!(c(0, 0), "1");
        assert_eq!(c(3, 5), "0");
        assert_eq!(
            count(10, 3, Mode::Permutations, false).unwrap().to_string(),
            "720"
        );
        assert_eq!(
            count(10, 3, Mode::Permutations, true).unwrap().to_string(),
            "1000"
        );
        // Combinations with repetition: C(n + r - 1, r) = C(12, 3).
        assert_eq!(
            count(10, 3, Mode::Combinations, true).unwrap().to_string(),
            "220"
        );
        // Circular: C(6, 6) * 5! = 120.
        assert_eq!(
            count(6, 6, Mode::CircularPermutations, false)
                .unwrap()
                .to_string(),
            "120"
        );
        assert_eq!(
            count(5, 3, Mode::CircularPermutations, false)
                .unwrap()
                .to_string(),
            "20"
        );
    }

    #[test]
    fn big_counts_are_exact_beyond_u128() {
        // 100! / (50!)^2 — 30 digits, past u64 and checked against the known value.
        assert_eq!(c(100, 50), "100891344545564193334812497256");
        // C(1000, 500) has 300 digits; assert the shape plus both ends.
        let huge = c(1000, 500);
        assert_eq!(huge.len(), 300);
        assert!(huge.starts_with("27028824094543656951"));
        assert!(huge.ends_with("9821216320"));
        // 200! via P(200, 200) — 375 digits, which no f64/u128 path could hold.
        assert_eq!(
            count(200, 200, Mode::Permutations, false)
                .unwrap()
                .to_string()
                .len(),
            375
        );
    }

    #[test]
    fn summary_is_the_calculator_view() {
        let out = compute(
            "",
            52,
            5,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Summary,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(
            out,
            "C(52, 5) = 2,598,960\n\n\
             Order matters: no\n\
             Repetition: not allowed\n\
             Formula: C(n, r) = n! / (r! * (n - r)!) = 52! / (5! * 47!)\n\
             Odds of one specific result: 1 in 2,598,960 (3.848e-5%)"
        );
    }

    #[test]
    fn enumerates_combinations_of_a_pool() {
        let out = compute(
            "a, b, c, d",
            0,
            2,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Lines,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "a, b\na, c\na, d\nb, c\nb, d\nc, d");
    }

    #[test]
    fn enumerates_permutations_and_repetition_variants() {
        let perms = compute(
            "a, b, c",
            0,
            2,
            Mode::Permutations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Lines,
            JoinSeparator::None,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(perms, "ab\nac\nba\nbc\nca\ncb");

        let with_rep = compute(
            "a, b",
            0,
            2,
            Mode::Permutations,
            true,
            ItemSeparator::Auto,
            false,
            OutputFormat::Lines,
            JoinSeparator::None,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(with_rep, "aa\nab\nba\nbb");

        let combo_rep = compute(
            "a, b",
            0,
            2,
            Mode::Combinations,
            true,
            ItemSeparator::Auto,
            false,
            OutputFormat::Lines,
            JoinSeparator::None,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(combo_rep, "aa\nab\nbb");
    }

    #[test]
    fn circular_enumeration_drops_rotations() {
        // 4 people at a round table: 3! = 6 distinct seatings, all starting at `a`.
        let out = compute(
            "a, b, c, d",
            0,
            4,
            Mode::CircularPermutations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Lines,
            JoinSeparator::Dash,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "a-b-c-d\na-b-d-c\na-c-b-d\na-c-d-b\na-d-b-c\na-d-c-b");
        assert_eq!(out.lines().count(), 6);
    }

    #[test]
    fn empty_pool_enumerates_one_to_n() {
        let out = compute(
            "",
            4,
            3,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Json,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(
            out,
            r#"[["1","2","3"],["1","2","4"],["1","3","4"],["2","3","4"]]"#
        );
    }

    #[test]
    fn csv_quotes_only_what_needs_it() {
        let out = compute(
            "New York|Paris, France|Rome",
            0,
            2,
            Mode::Combinations,
            false,
            ItemSeparator::Pipe,
            false,
            OutputFormat::Csv,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(
            out,
            "New York,\"Paris, France\"\nNew York,Rome\n\"Paris, France\",Rome"
        );
    }

    #[test]
    fn dedupe_collapses_the_pool() {
        let out = compute(
            "a, b, a, b",
            0,
            2,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            true,
            OutputFormat::Lines,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(out, "a, b");
    }

    #[test]
    fn custom_join_and_count_format() {
        let joined = compute(
            "1 2 3",
            0,
            2,
            Mode::Combinations,
            false,
            ItemSeparator::Space,
            false,
            OutputFormat::Lines,
            JoinSeparator::Custom,
            " :: ",
            10_000,
        )
        .unwrap();
        assert_eq!(joined, "1 :: 2\n1 :: 3\n2 :: 3");

        let n_only = compute(
            "",
            49,
            6,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Count,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap();
        assert_eq!(n_only, "13983816");
    }

    #[test]
    fn cap_is_reported_with_the_exact_count() {
        let err = compute(
            "",
            49,
            6,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Lines,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap_err();
        assert!(err.contains("13983816"), "{err}");
        assert!(err.contains("max_results (10000)"), "{err}");
    }

    #[test]
    fn rejects_bad_inputs() {
        assert!(parse_mode("triangles").is_err());
        assert!(parse_output_format("yaml").is_err());
        assert!(parse_item_separator("caret").is_err());
        assert!(parse_join_separator("tilde").is_err());
        // No pool and no n.
        let err = compute(
            "",
            0,
            2,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Summary,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap_err();
        assert!(err.contains("expected either items"), "{err}");
        // Circular + repetition is explicitly out of model.
        let err = count(5, 3, Mode::CircularPermutations, true).unwrap_err();
        assert!(err.contains("necklace counting"), "{err}");
        // r > n without repetition has nothing to list.
        let err = compute(
            "a, b",
            0,
            3,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Lines,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap_err();
        assert!(err.contains("nothing to enumerate"), "{err}");
        // n above the documented ceiling.
        let err = compute(
            "",
            5000,
            2,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Summary,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap_err();
        assert!(err.contains("above the maximum"), "{err}");
    }

    #[test]
    fn zero_count_summary_explains_itself() {
        let out = compute(
            "",
            3,
            5,
            Mode::Combinations,
            false,
            ItemSeparator::Auto,
            false,
            OutputFormat::Summary,
            JoinSeparator::Comma,
            "",
            10_000,
        )
        .unwrap();
        assert!(out.starts_with("C(3, 5) = 0\n"), "{out}");
        assert!(
            out.contains("Odds of one specific result: n/a (0 results)"),
            "{out}"
        );
        assert!(out.contains("no selection is possible"), "{out}");
    }
}
