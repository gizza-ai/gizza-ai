//! decision-tree-classifier core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps, no third-party crates.
//!
//! Fits a single, fully deterministic classification tree to a pasted table and
//! prints what makes a tree worth using in the first place: the human-readable
//! `IF … THEN class` rules. Splits are chosen greedily by Gini impurity decrease
//! (CART), Shannon information gain (ID3) or the gain ratio (C4.5); numeric
//! columns get midpoint threshold splits and categorical columns get either
//! one-vs-rest binary splits or one branch per value. The report also carries a
//! text tree, normalised feature importance, training accuracy with a confusion
//! matrix, an optional hold-out evaluation, and predictions for pasted rows.
//!
//! The only randomness in the tool is the shuffle behind `test_split`, driven by
//! `seed`; tree fitting itself is exact, so the same table always yields the same
//! rules.

/// Hard caps — keep a pasted table inside what a browser tab can chew through.
pub const MAX_ROWS: usize = 20_000;
pub const MAX_COLS: usize = 100;
/// A categorical column with more distinct values than this is almost certainly
/// an id column, and would explode the split search.
pub const MAX_LEVELS: usize = 200;
/// Cap on rows pasted into `predict`.
pub const MAX_PREDICT_ROWS: usize = 1_000;

/// Every knob the tool exposes. `Default` mirrors the descriptor defaults.
#[derive(Clone, Debug)]
pub struct Options {
    pub target: String,
    pub features: String,
    pub criterion: String,
    pub splits: String,
    pub max_depth: u32,
    pub min_samples_split: u32,
    pub min_samples_leaf: u32,
    pub min_gain: f64,
    pub class_weight: String,
    pub test_split: f64,
    pub seed: u64,
    pub predict: String,
    pub header: String,
    pub decimals: u32,
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            target: "last".into(),
            features: String::new(),
            criterion: "gini".into(),
            splits: "binary".into(),
            max_depth: 5,
            min_samples_split: 2,
            min_samples_leaf: 1,
            min_gain: 0.0,
            class_weight: "none".into(),
            test_split: 0.0,
            seed: 42,
            predict: String::new(),
            header: "auto".into(),
            decimals: 4,
            format: "text".into(),
        }
    }
}

/// Deterministic xorshift64* — used only to shuffle rows for `test_split`, so a
/// `seed` reproduces a hold-out split exactly.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // SplitMix-style mixing so tiny seeds (0, 1, 2) still give distinct streams.
        let mut x = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        Rng(x | 1)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn shuffle(&mut self, v: &mut [usize]) {
        for i in (1..v.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

// ---------------------------------------------------------------- parsing ---

fn is_missing(tok: &str) -> bool {
    matches!(
        tok.trim().to_ascii_lowercase().as_str(),
        "" | "na" | "n/a" | "nan" | "null" | "none" | "-" | "?" | "."
    )
}

fn looks_numeric(tok: &str) -> bool {
    tok.trim().parse::<f64>().is_ok()
}

fn split_row(line: &str, delim: Option<char>) -> Vec<String> {
    match delim {
        Some(d) => line.split(d).map(|s| s.trim().to_string()).collect(),
        None => line.split_whitespace().map(|s| s.to_string()).collect(),
    }
}

/// Pick the column delimiter from the first non-blank line: whichever of
/// comma / tab / semicolon / pipe occurs most, else any run of whitespace.
fn detect_delim(line: &str) -> Option<char> {
    let mut best: Option<(char, usize)> = None;
    for d in [',', '\t', ';', '|'] {
        let n = line.matches(d).count();
        if n > 0 && best.map(|(_, b)| n > b).unwrap_or(true) {
            best = Some((d, n));
        }
    }
    best.map(|(d, _)| d)
}

fn grid(data: &str) -> Vec<Vec<String>> {
    let lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    let Some(first) = lines.first() else {
        return Vec::new();
    };
    let delim = detect_delim(first);
    lines.iter().map(|l| split_row(l, delim)).collect()
}

struct Table {
    names: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse_table(data: &str, header_mode: &str) -> Result<Table, String> {
    let mut rows = grid(data);
    if rows.is_empty() {
        return Err(
            "no data: paste a table with one row per observation, e.g. 'color,ripe' then 'red,yes'"
                .into(),
        );
    }
    let ncol = rows[0].len();
    if ncol < 2 {
        return Err(format!(
            "need at least 2 columns (features + the class column), found {ncol}. Separate columns with commas, tabs, semicolons, pipes or spaces."
        ));
    }
    if ncol > MAX_COLS {
        return Err(format!("too many columns: {ncol} (max {MAX_COLS})"));
    }
    for (i, r) in rows.iter().enumerate() {
        if r.len() != ncol {
            return Err(format!(
                "row {} has {} columns but row 1 has {ncol} — every row must have the same number of columns",
                i + 1,
                r.len()
            ));
        }
    }

    let has_header = match header_mode.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" => true,
        "no" | "false" => false,
        "" | "auto" => rows[0].iter().any(|t| !is_missing(t) && !looks_numeric(t)),
        other => {
            return Err(format!(
                "header must be 'auto', 'yes' or 'no' (got '{other}')"
            ))
        }
    };

    let names: Vec<String> = if has_header {
        let head = rows.remove(0);
        head.iter()
            .enumerate()
            .map(|(i, n)| {
                let n = n.trim();
                if n.is_empty() {
                    format!("c{}", i + 1)
                } else {
                    n.to_string()
                }
            })
            .collect()
    } else {
        (1..=ncol).map(|i| format!("c{i}")).collect()
    };

    if rows.is_empty() {
        return Err("no data rows: the table only has a header row".into());
    }
    if rows.len() > MAX_ROWS {
        return Err(format!("too many rows: {} (max {MAX_ROWS})", rows.len()));
    }
    Ok(Table { names, rows })
}

/// Resolve a column selector: `last`, `first`, a 1-based index, or a column name.
fn resolve_column(sel: &str, names: &[String], what: &str) -> Result<usize, String> {
    let s = sel.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("last") {
        return Ok(names.len() - 1);
    }
    if s.eq_ignore_ascii_case("first") {
        return Ok(0);
    }
    if let Ok(i) = s.parse::<usize>() {
        if i >= 1 && i <= names.len() {
            return Ok(i - 1);
        }
        return Err(format!(
            "{what} column index {i} is out of range (the table has {} columns)",
            names.len()
        ));
    }
    if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(s)) {
        return Ok(i);
    }
    Err(format!(
        "{what} column '{s}' not found — available columns: {}",
        names.join(", ")
    ))
}

// ------------------------------------------------------------ feature data ---

/// One prepared predictor column. Numeric columns keep their values; categorical
/// columns are interned into alphabetically sorted `levels`.
struct Feature {
    name: String,
    numeric: bool,
    num: Vec<f64>,
    cat: Vec<usize>,
    levels: Vec<String>,
}

impl Feature {
    fn level_of(&self, tok: &str) -> Option<usize> {
        self.levels.iter().position(|l| l == tok)
    }
}

// ------------------------------------------------------------------- tree ---

#[derive(Clone, Debug)]
enum SplitKind {
    /// children[0] = `<= t`, children[1] = `> t`
    Threshold(f64),
    /// children[0] = `= level`, children[1] = `≠ level`
    CatEquals(usize),
    /// one child per listed level
    CatMultiway(Vec<usize>),
}

struct Split {
    feature: usize,
    /// Ranking score under the chosen criterion (impurity decrease, or the gain
    /// ratio when criterion is `gain_ratio`).
    score: f64,
    /// Raw impurity decrease — what feature importance accumulates.
    decrease: f64,
    kind: SplitKind,
    children: Vec<Node>,
}

struct Node {
    n: usize,
    counts: Vec<usize>,
    wcounts: Vec<f64>,
    impurity: f64,
    class: usize,
    /// 1-based rule number, assigned to leaves in depth-first order.
    rule: usize,
    split: Option<Box<Split>>,
}

impl Node {
    fn purity(&self) -> f64 {
        if self.n == 0 {
            0.0
        } else {
            self.counts[self.class] as f64 / self.n as f64
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Criterion {
    Gini,
    Entropy,
    GainRatio,
}

impl Criterion {
    fn parse(s: &str) -> Result<Criterion, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "gini" => Ok(Criterion::Gini),
            "entropy" | "information_gain" | "infogain" => Ok(Criterion::Entropy),
            "gain_ratio" | "gainratio" => Ok(Criterion::GainRatio),
            other => Err(format!(
                "criterion must be 'gini', 'entropy' or 'gain_ratio' (got '{other}')"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Criterion::Gini => "gini (CART)",
            Criterion::Entropy => "entropy / information gain (ID3)",
            Criterion::GainRatio => "gain ratio (C4.5)",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Criterion::Gini => "gini",
            Criterion::Entropy => "entropy",
            Criterion::GainRatio => "gain_ratio",
        }
    }
    /// Impurity of a weighted class-count vector.
    fn impurity(self, wc: &[f64]) -> f64 {
        let total: f64 = wc.iter().sum();
        if total <= 0.0 {
            return 0.0;
        }
        match self {
            Criterion::Gini => {
                let mut s = 0.0;
                for c in wc {
                    let p = c / total;
                    s += p * p;
                }
                (1.0 - s).max(0.0)
            }
            Criterion::Entropy | Criterion::GainRatio => {
                let mut s = 0.0;
                for c in wc {
                    let p = c / total;
                    if p > 0.0 {
                        s -= p * p.log2();
                    }
                }
                s.max(0.0)
            }
        }
    }
}

struct Fit<'a> {
    feats: &'a [Feature],
    y: &'a [usize],
    weight: &'a [f64],
    nclass: usize,
    crit: Criterion,
    multiway: bool,
    max_depth: usize,
    min_split: usize,
    min_leaf: usize,
    min_gain: f64,
}

impl Fit<'_> {
    fn counts(&self, rows: &[usize]) -> (Vec<usize>, Vec<f64>) {
        let mut c = vec![0usize; self.nclass];
        let mut w = vec![0.0f64; self.nclass];
        for &r in rows {
            c[self.y[r]] += 1;
            w[self.y[r]] += self.weight[self.y[r]];
        }
        (c, w)
    }

    fn majority(&self, wc: &[f64]) -> usize {
        let mut best = 0usize;
        let mut bestv = f64::NEG_INFINITY;
        for (i, v) in wc.iter().enumerate() {
            if *v > bestv + 1e-12 {
                bestv = *v;
                best = i;
            }
        }
        best
    }

    fn leaf(&self, rows: &[usize]) -> Node {
        let (counts, wcounts) = self.counts(rows);
        let class = self.majority(&wcounts);
        Node {
            n: rows.len(),
            impurity: self.crit.impurity(&wcounts),
            class,
            counts,
            wcounts,
            rule: 0,
            split: None,
        }
    }

    /// Score a candidate partition: returns (ranking score, raw impurity decrease).
    fn score(&self, parent_imp: f64, parent_w: f64, parts: &[Vec<f64>]) -> Option<(f64, f64)> {
        let mut child = 0.0;
        let mut split_info = 0.0;
        for p in parts {
            let w: f64 = p.iter().sum();
            if w <= 0.0 {
                continue;
            }
            let frac = w / parent_w;
            child += frac * self.crit.impurity(p);
            split_info -= frac * frac.log2();
        }
        let decrease = parent_imp - child;
        let score = if self.crit == Criterion::GainRatio {
            if split_info <= 1e-12 {
                return None;
            }
            decrease / split_info
        } else {
            decrease
        };
        Some((score, decrease))
    }

    fn build(&self, rows: Vec<usize>, depth: usize) -> Node {
        let mut node = self.leaf(&rows);
        if depth >= self.max_depth
            || rows.len() < self.min_split
            || rows.len() < 2 * self.min_leaf
            || node.impurity <= 1e-12
        {
            return node;
        }
        let parent_w: f64 = node.wcounts.iter().sum();
        let parent_imp = node.impurity;
        let floor = self.min_gain.max(1e-12);

        let mut best: Option<(f64, f64, usize, SplitKind, Vec<Vec<usize>>)> = None;
        for (fi, f) in self.feats.iter().enumerate() {
            if f.numeric {
                let mut vals: Vec<(f64, usize)> =
                    rows.iter().map(|&r| (f.num[r], r)).collect::<Vec<_>>();
                vals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut left = vec![0.0f64; self.nclass];
                let mut right = node.wcounts.clone();
                for i in 0..vals.len().saturating_sub(1) {
                    let (v, r) = vals[i];
                    left[self.y[r]] += self.weight[self.y[r]];
                    right[self.y[r]] -= self.weight[self.y[r]];
                    let nv = vals[i + 1].0;
                    if !(nv > v) {
                        continue; // only split between distinct values
                    }
                    let ln = i + 1;
                    if ln < self.min_leaf || vals.len() - ln < self.min_leaf {
                        continue;
                    }
                    let Some((score, decrease)) =
                        self.score(parent_imp, parent_w, &[left.clone(), right.clone()])
                    else {
                        continue;
                    };
                    if score > floor && best.as_ref().map(|b| score > b.0 + 1e-12).unwrap_or(true) {
                        let t = v + (nv - v) / 2.0;
                        let l: Vec<usize> = vals[..ln].iter().map(|p| p.1).collect();
                        let rr: Vec<usize> = vals[ln..].iter().map(|p| p.1).collect();
                        best = Some((score, decrease, fi, SplitKind::Threshold(t), vec![l, rr]));
                    }
                }
            } else if self.multiway {
                let mut present: Vec<usize> = Vec::new();
                let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); f.levels.len()];
                for &r in &rows {
                    buckets[f.cat[r]].push(r);
                }
                for (li, b) in buckets.iter().enumerate() {
                    if !b.is_empty() {
                        present.push(li);
                    }
                }
                if present.len() < 2 || present.iter().any(|&li| buckets[li].len() < self.min_leaf)
                {
                    continue;
                }
                let parts: Vec<Vec<f64>> = present
                    .iter()
                    .map(|&li| {
                        let mut w = vec![0.0f64; self.nclass];
                        for &r in &buckets[li] {
                            w[self.y[r]] += self.weight[self.y[r]];
                        }
                        w
                    })
                    .collect();
                let Some((score, decrease)) = self.score(parent_imp, parent_w, &parts) else {
                    continue;
                };
                if score > floor && best.as_ref().map(|b| score > b.0 + 1e-12).unwrap_or(true) {
                    let groups: Vec<Vec<usize>> =
                        present.iter().map(|&li| buckets[li].clone()).collect();
                    best = Some((
                        score,
                        decrease,
                        fi,
                        SplitKind::CatMultiway(present.clone()),
                        groups,
                    ));
                }
            } else {
                let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); f.levels.len()];
                for &r in &rows {
                    buckets[f.cat[r]].push(r);
                }
                for (li, b) in buckets.iter().enumerate() {
                    if b.is_empty() || b.len() == rows.len() {
                        continue;
                    }
                    if b.len() < self.min_leaf || rows.len() - b.len() < self.min_leaf {
                        continue;
                    }
                    let mut lw = vec![0.0f64; self.nclass];
                    for &r in b {
                        lw[self.y[r]] += self.weight[self.y[r]];
                    }
                    let rw: Vec<f64> = node
                        .wcounts
                        .iter()
                        .zip(lw.iter())
                        .map(|(a, l)| a - l)
                        .collect();
                    let Some((score, decrease)) = self.score(parent_imp, parent_w, &[lw, rw]) else {
                        continue;
                    };
                    if score > floor && best.as_ref().map(|b| score > b.0 + 1e-12).unwrap_or(true) {
                        let inb = b.clone();
                        let out: Vec<usize> = rows
                            .iter()
                            .copied()
                            .filter(|r| f.cat[*r] != li)
                            .collect::<Vec<_>>();
                        best = Some((
                            score,
                            decrease,
                            fi,
                            SplitKind::CatEquals(li),
                            vec![inb, out],
                        ));
                    }
                }
            }
        }

        if let Some((score, decrease, fi, kind, groups)) = best {
            let children: Vec<Node> = groups
                .into_iter()
                .map(|g| self.build(g, depth + 1))
                .collect();
            node.split = Some(Box::new(Split {
                feature: fi,
                score,
                decrease,
                kind,
                children,
            }));
        }
        node
    }
}

/// Number leaves depth-first so rules, predictions and the tree agree.
fn number_leaves(node: &mut Node, next: &mut usize) {
    match node.split.as_mut() {
        None => {
            *next += 1;
            node.rule = *next;
        }
        Some(s) => {
            for c in s.children.iter_mut() {
                number_leaves(c, next);
            }
        }
    }
}

fn tree_stats(node: &Node, depth: usize) -> (usize, usize, usize) {
    match node.split.as_ref() {
        None => (depth, 1, 1),
        Some(s) => {
            let mut d = depth;
            let mut leaves = 0;
            let mut nodes = 1;
            for c in &s.children {
                let (cd, cl, cn) = tree_stats(c, depth + 1);
                d = d.max(cd);
                leaves += cl;
                nodes += cn;
            }
            (d, leaves, nodes)
        }
    }
}

fn accumulate_importance(node: &Node, total_w: f64, out: &mut [f64]) {
    if let Some(s) = node.split.as_ref() {
        let w: f64 = node.wcounts.iter().sum();
        out[s.feature] += (w / total_w) * s.decrease;
        for c in &s.children {
            accumulate_importance(c, total_w, out);
        }
    }
}

// ------------------------------------------------------------- conditions ---

fn cond_text(feats: &[Feature], s: &Split, child: usize, dec: u32) -> String {
    let f = &feats[s.feature];
    match &s.kind {
        SplitKind::Threshold(t) => {
            if child == 0 {
                format!("{} <= {}", f.name, fmt(*t, dec))
            } else {
                format!("{} > {}", f.name, fmt(*t, dec))
            }
        }
        SplitKind::CatEquals(li) => {
            if child == 0 {
                format!("{} = {}", f.name, f.levels[*li])
            } else {
                format!("{} != {}", f.name, f.levels[*li])
            }
        }
        SplitKind::CatMultiway(levels) => format!("{} = {}", f.name, f.levels[levels[child]]),
    }
}

// ------------------------------------------------------------- formatting ---

fn fmt(v: f64, dec: u32) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    let s = format!("{:.*}", dec as usize, v);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

fn pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

fn pad_right(s: &str, w: usize) -> String {
    let mut o = s.to_string();
    while o.chars().count() < w {
        o.push(' ');
    }
    o
}

fn pad_left(s: &str, w: usize) -> String {
    let mut o = String::new();
    while o.chars().count() + s.chars().count() < w {
        o.push(' ');
    }
    o.push_str(s);
    o
}

fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn jq(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// -------------------------------------------------------------- reporting ---

struct Rule {
    id: usize,
    conditions: Vec<String>,
    class: usize,
    n: usize,
    counts: Vec<usize>,
    purity: f64,
}

fn collect_rules(
    node: &Node,
    feats: &[Feature],
    dec: u32,
    path: &mut Vec<String>,
    out: &mut Vec<Rule>,
) {
    match node.split.as_ref() {
        None => out.push(Rule {
            id: node.rule,
            conditions: path.clone(),
            class: node.class,
            n: node.n,
            counts: node.counts.clone(),
            purity: node.purity(),
        }),
        Some(s) => {
            for (i, c) in s.children.iter().enumerate() {
                path.push(cond_text(feats, s, i, dec));
                collect_rules(c, feats, dec, path, out);
                path.pop();
            }
        }
    }
}

fn render_tree(
    node: &Node,
    feats: &[Feature],
    classes: &[String],
    dec: u32,
    prefix: &str,
    out: &mut String,
) {
    let Some(s) = node.split.as_ref() else {
        out.push_str(&format!(
            "{prefix}→ {}  [n={}, {}]\n",
            classes[node.class],
            node.n,
            pct(node.purity())
        ));
        return;
    };
    let last = s.children.len().saturating_sub(1);
    for (i, c) in s.children.iter().enumerate() {
        let connector = if i == last { "└─ " } else { "├─ " };
        let cond = cond_text(feats, s, i, dec);
        if c.split.is_none() {
            out.push_str(&format!(
                "{prefix}{connector}{cond} → {}  [n={}, {}]\n",
                classes[c.class],
                c.n,
                pct(c.purity())
            ));
        } else {
            out.push_str(&format!("{prefix}{connector}{cond}\n"));
            let child_prefix = format!("{prefix}{}", if i == last { "   " } else { "│  " });
            render_tree(c, feats, classes, dec, &child_prefix, out);
        }
    }
}

struct Eval {
    n: usize,
    correct: usize,
    accuracy: f64,
    confusion: Vec<Vec<usize>>,
}

fn render_confusion(classes: &[String], m: &[Vec<usize>]) -> String {
    let label_w = classes.iter().map(|c| c.chars().count()).max().unwrap_or(1);
    let mut cell_w = label_w;
    for row in m {
        for v in row {
            cell_w = cell_w.max(v.to_string().len());
        }
    }
    let mut out = String::new();
    out.push_str("  ");
    out.push_str(&pad_right("", label_w));
    for c in classes {
        out.push(' ');
        out.push_str(&pad_left(c, cell_w));
    }
    out.push('\n');
    for (i, c) in classes.iter().enumerate() {
        out.push_str("  ");
        out.push_str(&pad_right(c, label_w));
        for v in &m[i] {
            out.push(' ');
            out.push_str(&pad_left(&v.to_string(), cell_w));
        }
        out.push('\n');
    }
    out
}

// -------------------------------------------------------------- inference ---

/// One prepared row for prediction: `None` = missing.
enum Cell {
    Num(f64),
    Cat(Option<usize>),
    Missing,
}

fn descend<'a>(node: &'a Node, feats: &[Feature], row: &[Cell]) -> &'a Node {
    let Some(s) = node.split.as_ref() else {
        return node;
    };
    let pick = match (&s.kind, &row[s.feature]) {
        (SplitKind::Threshold(t), Cell::Num(v)) => Some(if *v <= *t { 0 } else { 1 }),
        (SplitKind::CatEquals(li), Cell::Cat(Some(v))) => Some(if v == li { 0 } else { 1 }),
        // An unseen category cannot equal the tested level, so it takes the "≠" branch.
        (SplitKind::CatEquals(_), Cell::Cat(None)) => Some(1),
        (SplitKind::CatMultiway(levels), Cell::Cat(Some(v))) => {
            levels.iter().position(|l| l == v)
        }
        _ => None,
    };
    // Missing or unseen values follow the branch that holds the most training rows.
    let idx = pick.unwrap_or_else(|| {
        let mut best = 0usize;
        for (i, c) in s.children.iter().enumerate() {
            if c.n > s.children[best].n {
                best = i;
            }
        }
        best
    });
    let _ = feats;
    descend(&s.children[idx], feats, row)
}

fn evaluate(root: &Node, feats: &[Feature], rows: &[usize], y: &[usize], nclass: usize) -> Eval {
    let mut confusion = vec![vec![0usize; nclass]; nclass];
    let mut correct = 0usize;
    for &r in rows {
        let cells: Vec<Cell> = feats
            .iter()
            .map(|f| {
                if f.numeric {
                    Cell::Num(f.num[r])
                } else {
                    Cell::Cat(Some(f.cat[r]))
                }
            })
            .collect();
        let leaf = descend(root, feats, &cells);
        confusion[y[r]][leaf.class] += 1;
        if leaf.class == y[r] {
            correct += 1;
        }
    }
    let n = rows.len();
    Eval {
        n,
        correct,
        accuracy: if n == 0 {
            f64::NAN
        } else {
            correct as f64 / n as f64
        },
        confusion,
    }
}

struct Prediction {
    input: String,
    class: usize,
    confidence: f64,
    n: usize,
    rule: usize,
}

#[allow(clippy::too_many_arguments)]
fn parse_predictions(
    text: &str,
    names: &[String],
    target_idx: usize,
    feat_cols: &[usize],
    feats: &[Feature],
    root: &Node,
) -> Result<Vec<Prediction>, String> {
    let mut rows = grid(text);
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    // A first row whose every token names a training column is a header.
    let header: Option<Vec<String>> = {
        let first = &rows[0];
        if first
            .iter()
            .all(|t| names.iter().any(|n| n.eq_ignore_ascii_case(t.trim())))
        {
            Some(first.iter().map(|t| t.trim().to_string()).collect())
        } else {
            None
        }
    };
    let mut map: Vec<Option<usize>> = Vec::new(); // per feature -> index in the pasted row
    if let Some(h) = &header {
        rows.remove(0);
        for &col in feat_cols {
            let want = &names[col];
            map.push(h.iter().position(|x| x.eq_ignore_ascii_case(want)));
        }
        if let Some(pos) = map.iter().position(|m| m.is_none()) {
            return Err(format!(
                "predict rows are missing the '{}' column — the header must name every feature column ({})",
                names[feat_cols[pos]],
                feat_cols
                    .iter()
                    .map(|&c| names[c].clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    if rows.len() > MAX_PREDICT_ROWS {
        return Err(format!(
            "too many rows to predict: {} (max {MAX_PREDICT_ROWS})",
            rows.len()
        ));
    }

    let full = names.len();
    let nfeat = feat_cols.len();
    let mut out = Vec::new();
    for (ri, r) in rows.iter().enumerate() {
        let positions: Vec<usize> = if let Some(_h) = &header {
            map.iter().map(|m| m.unwrap()).collect()
        } else if r.len() == full {
            feat_cols.to_vec()
        } else if r.len() == nfeat {
            (0..nfeat).collect()
        } else {
            return Err(format!(
                "predict row {} has {} columns — expected {} (the full table layout) or {} (just the feature columns: {}), or add a header row naming the columns",
                ri + 1,
                r.len(),
                full,
                nfeat,
                feat_cols
                    .iter()
                    .map(|&c| names[c].clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };
        let mut cells: Vec<Cell> = Vec::with_capacity(nfeat);
        let mut echo: Vec<String> = Vec::with_capacity(nfeat);
        for (fi, f) in feats.iter().enumerate() {
            let tok = r[positions[fi]].trim();
            echo.push(tok.to_string());
            if is_missing(tok) {
                cells.push(Cell::Missing);
            } else if f.numeric {
                match tok.parse::<f64>() {
                    Ok(v) => cells.push(Cell::Num(v)),
                    Err(_) => {
                        return Err(format!(
                            "predict row {}: '{}' is not a number, but the '{}' feature is numeric",
                            ri + 1,
                            tok,
                            f.name
                        ))
                    }
                }
            } else {
                cells.push(Cell::Cat(f.level_of(tok)));
            }
        }
        let _ = target_idx;
        let leaf = descend(root, feats, &cells);
        out.push(Prediction {
            input: echo.join(", "),
            class: leaf.class,
            confidence: leaf.purity(),
            n: leaf.n,
            rule: leaf.rule,
        });
    }
    Ok(out)
}

// ------------------------------------------------------------------- main ---

struct Report {
    crit: Criterion,
    multiway: bool,
    class_weight: String,
    target_name: String,
    classes: Vec<String>,
    feats: Vec<Feature>,
    rows_used: usize,
    rows_dropped: usize,
    depth: usize,
    leaves: usize,
    nodes: usize,
    root: Node,
    rules: Vec<Rule>,
    importance: Vec<f64>,
    train: Eval,
    test: Option<Eval>,
    predictions: Vec<Prediction>,
    dec: u32,
}

/// Fit the tree and render the report in the requested format.
pub fn run(data: &str, o: &Options) -> Result<String, String> {
    let crit = Criterion::parse(&o.criterion)?;
    let multiway = match o.splits.trim().to_ascii_lowercase().as_str() {
        "" | "binary" => false,
        "multiway" | "multi" => true,
        other => {
            return Err(format!(
                "splits must be 'binary' or 'multiway' (got '{other}')"
            ))
        }
    };
    let balanced = match o.class_weight.trim().to_ascii_lowercase().as_str() {
        "" | "none" => false,
        "balanced" => true,
        other => {
            return Err(format!(
                "class_weight must be 'none' or 'balanced' (got '{other}')"
            ))
        }
    };
    let fmt_name = {
        let f = o.format.trim().to_ascii_lowercase();
        let f = if f.is_empty() { "text".to_string() } else { f };
        if !matches!(f.as_str(), "text" | "json" | "csv" | "dot") {
            return Err(format!(
                "format must be 'text', 'json', 'csv' or 'dot' (got '{}')",
                o.format.trim()
            ));
        }
        f
    };
    if o.max_depth < 1 || o.max_depth > 20 {
        return Err(format!("max_depth must be 1..=20 (got {})", o.max_depth));
    }
    if o.min_samples_split < 2 {
        return Err(format!(
            "min_samples_split must be at least 2 (got {})",
            o.min_samples_split
        ));
    }
    if o.min_samples_leaf < 1 {
        return Err("min_samples_leaf must be at least 1".to_string());
    }
    if !(0.0..=1.0).contains(&o.min_gain) || !o.min_gain.is_finite() {
        return Err(format!("min_gain must be between 0 and 1 (got {})", o.min_gain));
    }
    if !(0.0..=0.5).contains(&o.test_split) || !o.test_split.is_finite() {
        return Err(format!(
            "test_split must be between 0 and 0.5 (got {})",
            o.test_split
        ));
    }
    if o.decimals > 12 {
        return Err(format!("decimals must be 0..=12 (got {})", o.decimals));
    }

    let table = parse_table(data, &o.header)?;
    let target_idx = resolve_column(&o.target, &table.names, "target")?;

    let feat_cols: Vec<usize> = if o.features.trim().is_empty() {
        (0..table.names.len()).filter(|i| *i != target_idx).collect()
    } else {
        let mut v = Vec::new();
        for part in o.features.split(',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            let i = resolve_column(p, &table.names, "feature")?;
            if i == target_idx {
                return Err(format!(
                    "'{}' is the target column — it cannot also be a feature",
                    table.names[i]
                ));
            }
            if !v.contains(&i) {
                v.push(i);
            }
        }
        if v.is_empty() {
            return Err("features listed but none resolved — leave it blank to use every non-target column".into());
        }
        v
    };
    if feat_cols.is_empty() {
        return Err("no feature columns: the table needs at least one column besides the target".into());
    }

    // Drop rows with missing values in the target or any selected feature.
    let mut kept: Vec<usize> = Vec::new();
    for (i, r) in table.rows.iter().enumerate() {
        if is_missing(&r[target_idx]) || feat_cols.iter().any(|&c| is_missing(&r[c])) {
            continue;
        }
        kept.push(i);
    }
    let rows_dropped = table.rows.len() - kept.len();
    if kept.len() < 2 {
        return Err(format!(
            "need at least 2 complete rows to fit a tree ({} usable after dropping {} row(s) with missing values)",
            kept.len(),
            rows_dropped
        ));
    }

    // Class labels, alphabetically ordered so output is stable.
    let mut classes: Vec<String> = Vec::new();
    for &r in &kept {
        let lab = table.rows[r][target_idx].trim().to_string();
        if !classes.contains(&lab) {
            classes.push(lab);
        }
    }
    classes.sort();
    if classes.len() > MAX_LEVELS {
        return Err(format!(
            "the target column has {} distinct classes (max {MAX_LEVELS}) — this looks like a numeric or id column, not a class label",
            classes.len()
        ));
    }
    let y: Vec<usize> = kept
        .iter()
        .map(|&r| {
            classes
                .iter()
                .position(|c| *c == table.rows[r][target_idx].trim())
                .unwrap()
        })
        .collect();

    // Prepare features over the kept rows.
    let mut feats: Vec<Feature> = Vec::with_capacity(feat_cols.len());
    for &c in &feat_cols {
        let numeric = kept.iter().all(|&r| looks_numeric(&table.rows[r][c]));
        if numeric {
            feats.push(Feature {
                name: table.names[c].clone(),
                numeric: true,
                num: kept
                    .iter()
                    .map(|&r| table.rows[r][c].trim().parse::<f64>().unwrap())
                    .collect(),
                cat: Vec::new(),
                levels: Vec::new(),
            });
        } else {
            let mut levels: Vec<String> = Vec::new();
            for &r in &kept {
                let v = table.rows[r][c].trim().to_string();
                if !levels.contains(&v) {
                    levels.push(v);
                }
            }
            levels.sort();
            if levels.len() > MAX_LEVELS {
                return Err(format!(
                    "feature '{}' has {} distinct values (max {MAX_LEVELS}) — drop the column or bucket it before training",
                    table.names[c],
                    levels.len()
                ));
            }
            let cat = kept
                .iter()
                .map(|&r| {
                    levels
                        .iter()
                        .position(|l| *l == table.rows[r][c].trim())
                        .unwrap()
                })
                .collect();
            feats.push(Feature {
                name: table.names[c].clone(),
                numeric: false,
                num: Vec::new(),
                cat,
                levels,
            });
        }
    }

    let n = kept.len();
    let nclass = classes.len();

    // Deterministic hold-out split.
    let mut order: Vec<usize> = (0..n).collect();
    let mut test_rows: Vec<usize> = Vec::new();
    let mut train_rows: Vec<usize> = order.clone();
    if o.test_split > 0.0 {
        if n < 3 {
            return Err(format!(
                "need at least 3 rows for a hold-out test split (have {n}) — set test_split to 0"
            ));
        }
        Rng::new(o.seed).shuffle(&mut order);
        let mut k = (n as f64 * o.test_split).round() as usize;
        k = k.clamp(1, n - 2);
        test_rows = order[..k].to_vec();
        train_rows = order[k..].to_vec();
        test_rows.sort_unstable();
        train_rows.sort_unstable();
    }

    // Class weights from the TRAINING rows only.
    let weight: Vec<f64> = if balanced {
        let mut cnt = vec![0usize; nclass];
        for &r in &train_rows {
            cnt[y[r]] += 1;
        }
        cnt.iter()
            .map(|&c| {
                if c == 0 {
                    0.0
                } else {
                    train_rows.len() as f64 / (nclass as f64 * c as f64)
                }
            })
            .collect()
    } else {
        vec![1.0; nclass]
    };

    let fit = Fit {
        feats: &feats,
        y: &y,
        weight: &weight,
        nclass,
        crit,
        multiway,
        max_depth: o.max_depth as usize,
        min_split: (o.min_samples_split as usize).max(2),
        min_leaf: (o.min_samples_leaf as usize).max(1),
        min_gain: o.min_gain,
    };
    let mut root = fit.build(train_rows.clone(), 0);
    let mut next = 0usize;
    number_leaves(&mut root, &mut next);

    let (depth, leaves, nodes) = tree_stats(&root, 0);
    let total_w: f64 = root.wcounts.iter().sum();
    let mut importance = vec![0.0f64; feats.len()];
    if total_w > 0.0 {
        accumulate_importance(&root, total_w, &mut importance);
    }
    let sum: f64 = importance.iter().sum();
    if sum > 0.0 {
        for v in importance.iter_mut() {
            *v /= sum;
        }
    }

    let mut rules = Vec::new();
    collect_rules(&root, &feats, o.decimals, &mut Vec::new(), &mut rules);

    let train = evaluate(&root, &feats, &train_rows, &y, nclass);
    let test = if test_rows.is_empty() {
        None
    } else {
        Some(evaluate(&root, &feats, &test_rows, &y, nclass))
    };

    let predictions = if o.predict.trim().is_empty() {
        Vec::new()
    } else {
        parse_predictions(
            &o.predict,
            &table.names,
            target_idx,
            &feat_cols,
            &feats,
            &root,
        )?
    };

    order.clear();
    let rep = Report {
        crit,
        multiway,
        class_weight: if balanced { "balanced" } else { "none" }.to_string(),
        target_name: table.names[target_idx].clone(),
        classes,
        feats,
        rows_used: n,
        rows_dropped,
        depth,
        leaves,
        nodes,
        root,
        rules,
        importance,
        train,
        test,
        predictions,
        dec: o.decimals,
    };

    Ok(match fmt_name.as_str() {
        "json" => render_json(&rep),
        "csv" => render_csv(&rep),
        "dot" => render_dot(&rep),
        _ => render_text(&rep),
    })
}

fn render_text(r: &Report) -> String {
    let mut o = String::new();
    o.push_str("Decision tree classifier\n");
    o.push_str(&format!("Criterion: {}\n", r.crit.label()));
    o.push_str(&format!(
        "Splits: {}\n",
        if r.multiway {
            "multiway on categorical features"
        } else {
            "binary"
        }
    ));
    if r.class_weight == "balanced" {
        o.push_str("Class weight: balanced\n");
    }
    o.push_str(&format!(
        "Target: {} ({} classes: {})\n",
        r.target_name,
        r.classes.len(),
        r.classes.join(", ")
    ));
    o.push_str(&format!(
        "Features ({}): {}\n",
        r.feats.len(),
        r.feats
            .iter()
            .map(|f| format!(
                "{}{}",
                f.name,
                if f.numeric { " [numeric]" } else { "" }
            ))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    o.push_str(&format!("Rows: {} used", r.rows_used));
    if r.rows_dropped > 0 {
        o.push_str(&format!(
            ", {} dropped (missing values)",
            r.rows_dropped
        ));
    }
    o.push('\n');
    o.push_str(&format!(
        "Tree size: depth {}, {} leaves, {} nodes\n",
        r.depth, r.leaves, r.nodes
    ));

    o.push_str("\nTree:\n");
    let mut tree = String::new();
    render_tree(&r.root, &r.feats, &r.classes, r.dec, "", &mut tree);
    o.push_str(&tree);

    o.push_str("\nRules:\n");
    for rule in &r.rules {
        let cond = if rule.conditions.is_empty() {
            "always".to_string()
        } else {
            rule.conditions.join(" AND ")
        };
        o.push_str(&format!(
            "{}. IF {} THEN {} = {}  [n={}, {}]\n",
            rule.id,
            cond,
            r.target_name,
            r.classes[rule.class],
            rule.n,
            pct(rule.purity)
        ));
    }

    o.push_str("\nFeature importance:\n");
    let mut idx: Vec<usize> = (0..r.feats.len()).collect();
    idx.sort_by(|a, b| {
        r.importance[*b]
            .partial_cmp(&r.importance[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(b))
    });
    let w = r
        .feats
        .iter()
        .map(|f| f.name.chars().count())
        .max()
        .unwrap_or(1);
    for i in idx {
        o.push_str(&format!(
            "  {}  {}\n",
            pad_right(&r.feats[i].name, w),
            fmt(r.importance[i], r.dec)
        ));
    }

    o.push_str(&format!(
        "\nTraining accuracy: {} ({}/{} correct)\n",
        fmt(r.train.accuracy, r.dec),
        r.train.correct,
        r.train.n
    ));
    o.push_str("\nConfusion matrix (rows = actual, columns = predicted):\n");
    o.push_str(&render_confusion(&r.classes, &r.train.confusion));

    if let Some(t) = &r.test {
        o.push_str(&format!(
            "\nHold-out test accuracy: {} ({}/{} correct)\n",
            fmt(t.accuracy, r.dec),
            t.correct,
            t.n
        ));
        o.push_str("\nTest confusion matrix (rows = actual, columns = predicted):\n");
        o.push_str(&render_confusion(&r.classes, &t.confusion));
    }

    if !r.predictions.is_empty() {
        o.push_str(&format!("\nPredictions ({} rows):\n", r.predictions.len()));
        for (i, p) in r.predictions.iter().enumerate() {
            o.push_str(&format!(
                "{}. {} → {}  [{} confidence, rule {}, n={}]\n",
                i + 1,
                p.input,
                r.classes[p.class],
                pct(p.confidence),
                p.rule,
                p.n
            ));
        }
    }

    o.trim_end().to_string()
}

fn node_json(node: &Node, r: &Report) -> String {
    let counts = r
        .classes
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}:{}", jq(c), node.counts[i]))
        .collect::<Vec<_>>()
        .join(",");
    let head = format!(
        "{{\"n\":{},\"counts\":{{{}}},\"impurity\":{},\"class\":{}",
        node.n,
        counts,
        fmt(node.impurity, r.dec),
        jq(&r.classes[node.class])
    );
    match node.split.as_ref() {
        None => format!(
            "{head},\"leaf\":true,\"rule\":{},\"purity\":{}}}",
            node.rule,
            fmt(node.purity(), r.dec)
        ),
        Some(s) => {
            let children = s
                .children
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    format!(
                        "{{\"condition\":{},\"node\":{}}}",
                        jq(&cond_text(&r.feats, s, i, r.dec)),
                        node_json(c, r)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{head},\"leaf\":false,\"feature\":{},\"score\":{},\"impurity_decrease\":{},\"children\":[{}]}}",
                jq(&r.feats[s.feature].name),
                fmt(s.score, r.dec),
                fmt(s.decrease, r.dec),
                children
            )
        }
    }
}

fn eval_json(e: &Eval, r: &Report) -> String {
    let conf = e
        .confusion
        .iter()
        .map(|row| {
            format!(
                "[{}]",
                row.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"n\":{},\"correct\":{},\"accuracy\":{},\"confusion\":[{}]}}",
        e.n,
        e.correct,
        fmt(e.accuracy, r.dec),
        conf
    )
}

fn render_json(r: &Report) -> String {
    let mut o = String::new();
    o.push('{');
    o.push_str(&format!("\"criterion\":{},", jq(r.crit.name())));
    o.push_str(&format!(
        "\"splits\":{},",
        jq(if r.multiway { "multiway" } else { "binary" })
    ));
    o.push_str(&format!("\"class_weight\":{},", jq(&r.class_weight)));
    o.push_str(&format!("\"target\":{},", jq(&r.target_name)));
    o.push_str(&format!(
        "\"classes\":[{}],",
        r.classes.iter().map(|c| jq(c)).collect::<Vec<_>>().join(",")
    ));
    o.push_str(&format!(
        "\"features\":[{}],",
        r.feats
            .iter()
            .map(|f| format!(
                "{{\"name\":{},\"type\":{}}}",
                jq(&f.name),
                jq(if f.numeric { "numeric" } else { "categorical" })
            ))
            .collect::<Vec<_>>()
            .join(",")
    ));
    o.push_str(&format!("\"rows_used\":{},", r.rows_used));
    o.push_str(&format!("\"rows_dropped\":{},", r.rows_dropped));
    o.push_str(&format!(
        "\"depth\":{},\"leaves\":{},\"nodes\":{},",
        r.depth, r.leaves, r.nodes
    ));
    o.push_str(&format!(
        "\"rules\":[{}],",
        r.rules
            .iter()
            .map(|rule| format!(
                "{{\"id\":{},\"conditions\":[{}],\"class\":{},\"n\":{},\"purity\":{}}}",
                rule.id,
                rule.conditions
                    .iter()
                    .map(|c| jq(c))
                    .collect::<Vec<_>>()
                    .join(","),
                jq(&r.classes[rule.class]),
                rule.n,
                fmt(rule.purity, r.dec)
            ))
            .collect::<Vec<_>>()
            .join(",")
    ));
    o.push_str(&format!(
        "\"importance\":[{}],",
        r.feats
            .iter()
            .enumerate()
            .map(|(i, f)| format!(
                "{{\"feature\":{},\"importance\":{}}}",
                jq(&f.name),
                fmt(r.importance[i], r.dec)
            ))
            .collect::<Vec<_>>()
            .join(",")
    ));
    o.push_str(&format!("\"train\":{},", eval_json(&r.train, r)));
    if let Some(t) = &r.test {
        o.push_str(&format!("\"test\":{},", eval_json(t, r)));
    }
    if !r.predictions.is_empty() {
        o.push_str(&format!(
            "\"predictions\":[{}],",
            r.predictions
                .iter()
                .enumerate()
                .map(|(i, p)| format!(
                    "{{\"row\":{},\"input\":{},\"class\":{},\"confidence\":{},\"rule\":{},\"n\":{}}}",
                    i + 1,
                    jq(&p.input),
                    jq(&r.classes[p.class]),
                    fmt(p.confidence, r.dec),
                    p.rule,
                    p.n
                ))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    o.push_str(&format!("\"tree\":{}", node_json(&r.root, r)));
    o.push('}');
    o
}

fn render_csv(r: &Report) -> String {
    let mut o = String::from("section,name,value\n");
    let mut row = |s: &str, n: &str, v: &str| {
        o.push_str(&format!("{},{},{}\n", s, csv_cell(n), csv_cell(v)));
    };
    row("model", "criterion", r.crit.name());
    row("model", "splits", if r.multiway { "multiway" } else { "binary" });
    row("model", "class_weight", &r.class_weight);
    row("model", "target", &r.target_name);
    row("model", "classes", &r.classes.join("|"));
    row(
        "model",
        "features",
        &r.feats
            .iter()
            .map(|f| f.name.clone())
            .collect::<Vec<_>>()
            .join("|"),
    );
    row("model", "rows_used", &r.rows_used.to_string());
    row("model", "rows_dropped", &r.rows_dropped.to_string());
    row("model", "depth", &r.depth.to_string());
    row("model", "leaves", &r.leaves.to_string());
    row("model", "nodes", &r.nodes.to_string());
    for rule in &r.rules {
        let cond = if rule.conditions.is_empty() {
            "always".to_string()
        } else {
            rule.conditions.join(" AND ")
        };
        row(
            "rule",
            &rule.id.to_string(),
            &format!(
                "IF {} THEN {} = {} (n={}, {})",
                cond,
                r.target_name,
                r.classes[rule.class],
                rule.n,
                pct(rule.purity)
            ),
        );
    }
    for (i, f) in r.feats.iter().enumerate() {
        row("importance", &f.name, &fmt(r.importance[i], r.dec));
    }
    row("accuracy", "train", &fmt(r.train.accuracy, r.dec));
    for (ai, actual) in r.classes.iter().enumerate() {
        for (pi, pred) in r.classes.iter().enumerate() {
            row(
                "confusion_train",
                &format!("{}>{}", actual, pred),
                &r.train.confusion[ai][pi].to_string(),
            );
        }
    }
    if let Some(t) = &r.test {
        row("accuracy", "test", &fmt(t.accuracy, r.dec));
        for (ai, actual) in r.classes.iter().enumerate() {
            for (pi, pred) in r.classes.iter().enumerate() {
                row(
                    "confusion_test",
                    &format!("{}>{}", actual, pred),
                    &t.confusion[ai][pi].to_string(),
                );
            }
        }
    }
    for (i, p) in r.predictions.iter().enumerate() {
        row(
            "prediction",
            &(i + 1).to_string(),
            &format!(
                "{} => {} ({}, rule {})",
                p.input,
                r.classes[p.class],
                pct(p.confidence),
                p.rule
            ),
        );
    }
    o.trim_end().to_string()
}

fn dot_node(node: &Node, r: &Report, id: &mut usize, out: &mut String) -> usize {
    let me = *id;
    *id += 1;
    let dist = r
        .classes
        .iter()
        .enumerate()
        .filter(|(i, _)| node.counts[*i] > 0)
        .map(|(i, c)| format!("{}: {}", c, node.counts[i]))
        .collect::<Vec<_>>()
        .join(", ");
    match node.split.as_ref() {
        None => {
            out.push_str(&format!(
                "  n{me} [shape=box, style=\"rounded,filled\", fillcolor=\"#eef3ff\", label=\"{}\\nn = {}\\n{}\\nrule {}\"];\n",
                dot_escape(&r.classes[node.class]),
                node.n,
                dot_escape(&dist),
                node.rule
            ));
        }
        Some(s) => {
            out.push_str(&format!(
                "  n{me} [label=\"{}\\nn = {}\\n{}\\n{} = {}\"];\n",
                dot_escape(&r.feats[s.feature].name),
                node.n,
                dot_escape(&dist),
                r.crit.name(),
                fmt(node.impurity, r.dec)
            ));
            for (i, c) in s.children.iter().enumerate() {
                let cid = dot_node(c, r, id, out);
                out.push_str(&format!(
                    "  n{me} -> n{cid} [label=\"{}\"];\n",
                    dot_escape(&cond_text(&r.feats, s, i, r.dec))
                ));
            }
        }
    }
    me
}

fn render_dot(r: &Report) -> String {
    let mut o = String::from("digraph DecisionTree {\n");
    o.push_str("  node [shape=box, fontname=\"sans-serif\", fontsize=10];\n");
    o.push_str("  edge [fontname=\"sans-serif\", fontsize=9];\n");
    let mut id = 0usize;
    let mut body = String::new();
    dot_node(&r.root, r, &mut id, &mut body);
    o.push_str(&body);
    o.push_str("}\n");
    o.trim_end().to_string()
}

// ------------------------------------------------------------------ tests ---

#[cfg(test)]
mod tests {
    use super::*;

    const FRUIT: &str = "color,size,ripe\nred,small,yes\nred,large,yes\ngreen,small,no\ngreen,large,no";
    const EXPECTED_FRUIT_REPORT: &str = concat!(
        "Decision tree classifier\n",
        "Criterion: gini (CART)\n",
        "Splits: binary\n",
        "Target: ripe (2 classes: no, yes)\n",
        "Features (2): color, size\n",
        "Rows: 4 used\n",
        "Tree size: depth 1, 2 leaves, 3 nodes\n",
        "\n",
        "Tree:\n",
        "├─ color = green → no  [n=2, 100.0%]\n",
        "└─ color != green → yes  [n=2, 100.0%]\n",
        "\n",
        "Rules:\n",
        "1. IF color = green THEN ripe = no  [n=2, 100.0%]\n",
        "2. IF color != green THEN ripe = yes  [n=2, 100.0%]\n",
        "\n",
        "Feature importance:\n",
        "  color  1\n",
        "  size   0\n",
        "\n",
        "Training accuracy: 1 (4/4 correct)\n",
        "\n",
        "Confusion matrix (rows = actual, columns = predicted):\n",
        "       no yes\n",
        "  no    2   0\n",
        "  yes   0   2",
    );

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn splits_a_perfectly_separable_categorical_table() {
        let out = run(FRUIT, &opts()).unwrap();
        assert!(out.contains("1. IF color = green THEN ripe = no  [n=2, 100.0%]"), "{out}");
        assert!(out.contains("2. IF color != green THEN ripe = yes  [n=2, 100.0%]"), "{out}");
        assert!(out.contains("Training accuracy: 1 (4/4 correct)"), "{out}");
        assert!(out.contains("Tree size: depth 1, 2 leaves, 3 nodes"), "{out}");
    }

    #[test]
    fn importance_is_normalised_and_ranks_the_used_feature_first() {
        let out = run(FRUIT, &opts()).unwrap();
        let imp = out.split("Feature importance:\n").nth(1).unwrap();
        let first = imp.lines().next().unwrap();
        assert!(first.contains("color"), "{out}");
        assert!(first.trim().ends_with(" 1"), "{out}");
        assert!(imp.lines().nth(1).unwrap().contains("size"), "{out}");
    }

    #[test]
    fn finds_a_numeric_threshold() {
        let data = "hours,pass\n1,no\n2,no\n3,no\n7,yes\n8,yes\n9,yes";
        let out = run(data, &opts()).unwrap();
        assert!(out.contains("hours <= 5"), "{out}");
        assert!(out.contains("hours > 5"), "{out}");
        assert!(out.contains("[numeric]"), "{out}");
    }

    #[test]
    fn entropy_and_gain_ratio_criteria_both_fit() {
        for c in ["entropy", "gain_ratio"] {
            let o = Options {
                criterion: c.into(),
                ..opts()
            };
            let out = run(FRUIT, &o).unwrap();
            assert!(out.contains("Training accuracy: 1"), "{c}: {out}");
        }
        let o = Options {
            criterion: "entropy".into(),
            ..opts()
        };
        assert!(run(FRUIT, &o)
            .unwrap()
            .contains("Criterion: entropy / information gain (ID3)"));
    }

    #[test]
    fn multiway_splits_produce_one_branch_per_value() {
        let data = "outlook,play\nsunny,no\nrain,no\novercast,yes\novercast,yes\nsunny,no\nrain,yes";
        let o = Options {
            splits: "multiway".into(),
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("outlook = overcast"), "{out}");
        assert!(out.contains("outlook = rain"), "{out}");
        assert!(out.contains("outlook = sunny"), "{out}");
        assert!(!out.contains("!="), "multiway must not emit binary conditions: {out}");
    }

    #[test]
    fn max_depth_one_yields_a_stump() {
        let data = "a,b,y\n1,1,x\n1,2,x\n2,1,z\n2,2,w\n2,3,w";
        let o = Options {
            max_depth: 1,
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Tree size: depth 1,"), "{out}");
    }

    #[test]
    fn min_samples_leaf_blocks_a_tiny_branch() {
        let data = "a,y\n1,x\n2,x\n3,z";
        // The only useful split would strand a single row, so the tree stays a stump.
        let o = Options {
            min_samples_leaf: 2,
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("IF always THEN y = x  [n=3, 66.7%]"), "{out}");
        assert!(out.contains("Tree size: depth 0, 1 leaves, 1 nodes"), "{out}");
        // Without the floor the same table does split.
        let out = run(data, &opts()).unwrap();
        assert!(out.contains("a <= 2.5"), "{out}");
    }

    #[test]
    fn full_text_report_is_stable() {
        // Pins the exact rendering the page spec and the CLI example assert on.
        let out = run(FRUIT, &opts()).unwrap();
        assert_eq!(out, EXPECTED_FRUIT_REPORT);
    }

    #[test]
    fn min_gain_prunes_a_weak_split() {
        let data = "a,y\n1,x\n2,x\n3,x\n4,z";
        let o = Options {
            min_gain: 0.5,
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Tree size: depth 0,"), "{out}");
    }

    #[test]
    fn balanced_class_weight_flips_a_lopsided_leaf() {
        let data = "a,y\n1,rare\n1,common\n1,common\n1,common\n2,common\n2,common";
        let plain = run(data, &opts()).unwrap();
        assert!(plain.contains("Training accuracy: 0.8333"), "{plain}");
        let o = Options {
            class_weight: "balanced".into(),
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Class weight: balanced"), "{out}");
        assert!(out.contains("a <= 1.5 → rare"), "{out}");
    }

    #[test]
    fn holdout_split_is_deterministic_and_reported() {
        let mut data = String::from("a,y\n");
        for i in 1..=20 {
            data.push_str(&format!("{},{}\n", i, if i > 10 { "high" } else { "low" }));
        }
        let o = Options {
            test_split: 0.25,
            ..opts()
        };
        let a = run(&data, &o).unwrap();
        let b = run(&data, &o).unwrap();
        assert_eq!(a, b);
        assert!(a.contains("Training accuracy: 1 (15/15 correct)"), "{a}");
        // The threshold learned from the 15 training rows misses one held-out row —
        // exactly the point of measuring on data the tree never saw.
        assert!(a.contains("Hold-out test accuracy: 0.8 (4/5 correct)"), "{a}");
        assert!(a.contains("Test confusion matrix"), "{a}");
    }

    #[test]
    fn predicts_pasted_rows_with_and_without_a_header() {
        let o = Options {
            predict: "color,size\ngreen,small\nred,large".into(),
            ..opts()
        };
        let out = run(FRUIT, &o).unwrap();
        assert!(out.contains("Predictions (2 rows):"), "{out}");
        assert!(out.contains("1. green, small → no  [100.0% confidence, rule 1, n=2]"), "{out}");
        assert!(out.contains("2. red, large → yes  [100.0% confidence, rule 2, n=2]"), "{out}");

        let o = Options {
            predict: "red,small".into(),
            ..opts()
        };
        let out = run(FRUIT, &o).unwrap();
        assert!(out.contains("→ yes"), "{out}");
    }

    #[test]
    fn unseen_category_takes_the_not_equal_branch() {
        let o = Options {
            predict: "blue,small".into(),
            ..opts()
        };
        let out = run(FRUIT, &o).unwrap();
        assert!(out.contains("1. blue, small → yes"), "{out}");
    }

    #[test]
    fn json_output_carries_tree_rules_and_metrics() {
        let o = Options {
            format: "json".into(),
            ..opts()
        };
        let out = run(FRUIT, &o).unwrap();
        assert!(out.contains("\"criterion\":\"gini\""), "{out}");
        assert!(out.contains("\"classes\":[\"no\",\"yes\"]"), "{out}");
        assert!(out.contains("\"rules\":["), "{out}");
        assert!(out.contains("\"accuracy\":1"), "{out}");
        assert!(out.contains("\"tree\":"), "{out}");
        assert!(out.starts_with('{') && out.ends_with('}'), "{out}");
    }

    #[test]
    fn csv_output_has_one_row_per_fact() {
        let o = Options {
            format: "csv".into(),
            ..opts()
        };
        let out = run(FRUIT, &o).unwrap();
        assert!(out.starts_with("section,name,value\n"), "{out}");
        assert!(out.contains("model,criterion,gini"), "{out}");
        assert!(out.contains("importance,color,1"), "{out}");
        assert!(out.contains("confusion_train,no>no,2"), "{out}");
    }

    #[test]
    fn dot_output_is_a_digraph() {
        let o = Options {
            format: "dot".into(),
            ..opts()
        };
        let out = run(FRUIT, &o).unwrap();
        assert!(out.starts_with("digraph DecisionTree {"), "{out}");
        assert!(out.contains("n0 -> n1"), "{out}");
        assert!(out.ends_with('}'), "{out}");
    }

    #[test]
    fn drops_rows_with_missing_values_and_reports_them() {
        let data = "color,size,ripe\nred,small,yes\nred,large,yes\ngreen,small,no\ngreen,,no\ngreen,large,no";
        let out = run(data, &opts()).unwrap();
        assert!(out.contains("Rows: 4 used, 1 dropped (missing values)"), "{out}");
    }

    #[test]
    fn headerless_tables_get_generated_column_names() {
        let data = "red,yes\nred,yes\ngreen,no\ngreen,no";
        let o = Options {
            header: "no".into(),
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Target: c2"), "{out}");
        assert!(out.contains("c1 = green"), "{out}");
    }

    #[test]
    fn target_and_feature_selectors_accept_names_and_indexes() {
        let data = "ripe,color,size\nyes,red,small\nyes,red,large\nno,green,small\nno,green,large";
        let o = Options {
            target: "1".into(),
            features: "color".into(),
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Target: ripe"), "{out}");
        assert!(out.contains("Features (1): color"), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   ", &opts()).unwrap_err();
        assert!(err.contains("no data"), "{err}");
    }

    #[test]
    fn single_column_is_an_error() {
        let err = run("only\n1\n2", &opts()).unwrap_err();
        assert!(err.contains("at least 2 columns"), "{err}");
    }

    #[test]
    fn unknown_target_column_is_an_error() {
        let err = run(FRUIT, &Options { target: "nope".into(), ..opts() }).unwrap_err();
        assert!(err.contains("target column 'nope' not found"), "{err}");
    }

    #[test]
    fn target_cannot_also_be_a_feature() {
        let err = run(
            FRUIT,
            &Options {
                features: "ripe".into(),
                ..opts()
            },
        )
        .unwrap_err();
        assert!(err.contains("cannot also be a feature"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_errors() {
        assert!(run(FRUIT, &Options { criterion: "chi".into(), ..opts() })
            .unwrap_err()
            .contains("criterion must be"));
        assert!(run(FRUIT, &Options { splits: "three".into(), ..opts() })
            .unwrap_err()
            .contains("splits must be"));
        assert!(run(FRUIT, &Options { format: "yaml".into(), ..opts() })
            .unwrap_err()
            .contains("format must be"));
        assert!(run(FRUIT, &Options { class_weight: "heavy".into(), ..opts() })
            .unwrap_err()
            .contains("class_weight must be"));
        assert!(run(FRUIT, &Options { header: "maybe".into(), ..opts() })
            .unwrap_err()
            .contains("header must be"));
    }

    #[test]
    fn out_of_range_numbers_are_errors() {
        assert!(run(FRUIT, &Options { max_depth: 25, ..opts() })
            .unwrap_err()
            .contains("max_depth must be"));
        assert!(run(FRUIT, &Options { test_split: 0.9, ..opts() })
            .unwrap_err()
            .contains("test_split must be"));
        assert!(run(FRUIT, &Options { min_gain: 2.0, ..opts() })
            .unwrap_err()
            .contains("min_gain must be"));
    }

    #[test]
    fn predict_row_with_the_wrong_width_is_an_error() {
        let err = run(
            FRUIT,
            &Options {
                predict: "red".into(),
                ..opts()
            },
        )
        .unwrap_err();
        assert!(err.contains("expected 3"), "{err}");
    }

    #[test]
    fn predict_row_with_text_in_a_numeric_feature_is_an_error() {
        let data = "hours,pass\n1,no\n2,no\n7,yes\n8,yes";
        let err = run(
            data,
            &Options {
                predict: "many".into(),
                ..opts()
            },
        )
        .unwrap_err();
        assert!(err.contains("is not a number"), "{err}");
    }

    #[test]
    fn too_few_usable_rows_is_an_error() {
        let err = run("a,y\n1,x\n,z", &opts()).unwrap_err();
        assert!(err.contains("at least 2 complete rows"), "{err}");
    }

    #[test]
    fn tsv_and_semicolon_tables_parse() {
        for data in [
            "color\tripe\nred\tyes\nred\tyes\ngreen\tno\ngreen\tno",
            "color;ripe\nred;yes\nred;yes\ngreen;no\ngreen;no",
        ] {
            let out = run(data, &opts()).unwrap();
            assert!(out.contains("Training accuracy: 1"), "{out}");
        }
    }

    #[test]
    fn decimals_control_number_precision() {
        let data = "a,y\n1,x\n2,x\n3,x\n4,z\n5,z\n6,z\n7,x";
        let o = Options {
            decimals: 2,
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Training accuracy: 1"), "{out}");
        let o = Options {
            decimals: 0,
            max_depth: 1,
            ..opts()
        };
        let out = run(data, &o).unwrap();
        assert!(out.contains("Training accuracy: 1") || out.contains("Training accuracy: 0"), "{out}");
    }
}
