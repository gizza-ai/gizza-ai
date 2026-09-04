//! financial-ratio-analyzer core — turn pasted income-statement and
//! balance-sheet figures into the standard liquidity, leverage, margin, return,
//! efficiency and market ratios. Pure compute, shared by the chat skill block,
//! the CLI and the web page. No wafer/wasm-bindgen deps.
//!
//! Conventions:
//! * Input is a block of `label: value` lines. The label is matched against an
//!   alias table (`net sales` == `revenue`), the value tolerates currency
//!   symbols, thousands separators, accounting parentheses for negatives and
//!   `k`/`m`/`bn` scale suffixes.
//! * Subtotals that were not pasted are DERIVED from their parts where the
//!   accounting identity allows it, and every derivation is reported.
//! * Percentages are stored as percentage points (12.5 means 12.5%); `times`
//!   ratios are stored as plain multiples; `days` as days.
//! * A ratio whose inputs are missing is `None` with a note naming exactly what
//!   is missing — it is never silently reported as zero.
//!
//! Educational arithmetic only — not financial, investment, tax or accounting
//! advice.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Largest accepted number of non-blank input lines per statement. A filed
/// balance sheet plus income statement is well under 100 lines; 400 leaves room
/// for a whole annual report section pasted verbatim.
pub const MAX_LINES: usize = 400;

// ---------------------------------------------------------------------------
// Recognized line items
// ---------------------------------------------------------------------------

pub const CASH: &str = "cash";
pub const MARKETABLE_SECURITIES: &str = "marketable_securities";
pub const ACCOUNTS_RECEIVABLE: &str = "accounts_receivable";
pub const INVENTORY: &str = "inventory";
pub const PREPAID_EXPENSES: &str = "prepaid_expenses";
pub const OTHER_CURRENT_ASSETS: &str = "other_current_assets";
pub const CURRENT_ASSETS: &str = "current_assets";
pub const FIXED_ASSETS: &str = "fixed_assets";
pub const TOTAL_ASSETS: &str = "total_assets";
pub const ACCOUNTS_PAYABLE: &str = "accounts_payable";
pub const SHORT_TERM_DEBT: &str = "short_term_debt";
pub const OTHER_CURRENT_LIABILITIES: &str = "other_current_liabilities";
pub const CURRENT_LIABILITIES: &str = "current_liabilities";
pub const LONG_TERM_DEBT: &str = "long_term_debt";
pub const OTHER_LONG_TERM_LIABILITIES: &str = "other_long_term_liabilities";
pub const TOTAL_LIABILITIES: &str = "total_liabilities";
pub const RETAINED_EARNINGS: &str = "retained_earnings";
pub const TOTAL_EQUITY: &str = "total_equity";
pub const REVENUE: &str = "revenue";
pub const COGS: &str = "cogs";
pub const GROSS_PROFIT: &str = "gross_profit";
pub const OPERATING_EXPENSES: &str = "operating_expenses";
pub const OPERATING_INCOME: &str = "operating_income";
pub const DEPRECIATION_AMORTIZATION: &str = "depreciation_amortization";
pub const EBITDA: &str = "ebitda";
pub const INTEREST_EXPENSE: &str = "interest_expense";
pub const PRETAX_INCOME: &str = "pretax_income";
pub const TAXES: &str = "taxes";
pub const NET_INCOME: &str = "net_income";
pub const SHARES_OUTSTANDING: &str = "shares_outstanding";
pub const SHARE_PRICE: &str = "share_price";

/// Canonical key -> accepted labels. The canonical key itself is always
/// accepted (matching is done on the normalized form, so `Total Assets`,
/// `total-assets` and `total_assets` are the same label).
const ALIASES: &[(&str, &[&str])] = &[
    (
        CASH,
        &[
            "cash",
            "cash and equivalents",
            "cash and cash equivalents",
            "cash equivalents",
            "cash on hand",
        ],
    ),
    (
        MARKETABLE_SECURITIES,
        &[
            "marketable securities",
            "short term investments",
            "securities",
            "temporary investments",
        ],
    ),
    (
        ACCOUNTS_RECEIVABLE,
        &[
            "accounts receivable",
            "receivables",
            "trade receivables",
            "net receivables",
            "debtors",
            "ar",
        ],
    ),
    (INVENTORY, &["inventory", "inventories", "stock on hand"]),
    (
        PREPAID_EXPENSES,
        &["prepaid expenses", "prepaids", "prepaid"],
    ),
    (OTHER_CURRENT_ASSETS, &["other current assets"]),
    (CURRENT_ASSETS, &["current assets", "total current assets"]),
    (
        FIXED_ASSETS,
        &[
            "fixed assets",
            "net fixed assets",
            "non current assets",
            "total non current assets",
            "property plant and equipment",
            "ppe",
            "net ppe",
        ],
    ),
    (TOTAL_ASSETS, &["total assets", "assets"]),
    (
        ACCOUNTS_PAYABLE,
        &[
            "accounts payable",
            "payables",
            "trade payables",
            "creditors",
            "ap",
        ],
    ),
    (
        SHORT_TERM_DEBT,
        &[
            "short term debt",
            "notes payable",
            "current portion of long term debt",
            "cpltd",
            "current debt",
            "short term borrowings",
        ],
    ),
    (
        OTHER_CURRENT_LIABILITIES,
        &[
            "other current liabilities",
            "accrued liabilities",
            "accrued expenses",
        ],
    ),
    (
        CURRENT_LIABILITIES,
        &["current liabilities", "total current liabilities"],
    ),
    (
        LONG_TERM_DEBT,
        &[
            "long term debt",
            "long term liabilities",
            "long term borrowings",
            "total long term liabilities",
            "non current liabilities",
            "ltd",
        ],
    ),
    (
        OTHER_LONG_TERM_LIABILITIES,
        &[
            "other long term liabilities",
            "other non current liabilities",
        ],
    ),
    (
        TOTAL_LIABILITIES,
        &["total liabilities", "liabilities", "total debt"],
    ),
    (
        RETAINED_EARNINGS,
        &[
            "retained earnings",
            "accumulated earnings",
            "accumulated profit",
        ],
    ),
    (
        TOTAL_EQUITY,
        &[
            "total equity",
            "equity",
            "shareholders equity",
            "shareholder equity",
            "stockholders equity",
            "owners equity",
            "net worth",
            "book value",
        ],
    ),
    (
        REVENUE,
        &[
            "revenue",
            "revenues",
            "total revenue",
            "net revenue",
            "net sales",
            "sales",
            "total sales",
            "turnover",
        ],
    ),
    (
        COGS,
        &[
            "cogs",
            "cost of goods sold",
            "cost of sales",
            "cost of revenue",
        ],
    ),
    (GROSS_PROFIT, &["gross profit", "gross income"]),
    (
        OPERATING_EXPENSES,
        &[
            "operating expenses",
            "total operating expenses",
            "opex",
            "sg a",
            "sga",
            "selling general and administrative",
        ],
    ),
    (
        OPERATING_INCOME,
        &[
            "operating income",
            "operating profit",
            "operating earnings",
            "ebit",
            "earnings before interest and taxes",
        ],
    ),
    (
        DEPRECIATION_AMORTIZATION,
        &[
            "depreciation amortization",
            "depreciation and amortization",
            "depreciation",
            "amortization",
            "d a",
        ],
    ),
    (
        EBITDA,
        &[
            "ebitda",
            "earnings before interest taxes depreciation and amortization",
        ],
    ),
    (
        INTEREST_EXPENSE,
        &[
            "interest expense",
            "interest expenses",
            "interest",
            "finance costs",
        ],
    ),
    (
        PRETAX_INCOME,
        &[
            "pretax income",
            "pre tax income",
            "income before taxes",
            "earnings before taxes",
            "profit before tax",
            "ebt",
        ],
    ),
    (
        TAXES,
        &[
            "taxes",
            "tax",
            "income tax",
            "income tax expense",
            "tax expense",
            "provision for income taxes",
        ],
    ),
    (
        NET_INCOME,
        &[
            "net income",
            "net profit",
            "net earnings",
            "net result",
            "profit after tax",
            "bottom line",
        ],
    ),
    (
        SHARES_OUTSTANDING,
        &[
            "shares outstanding",
            "shares",
            "share count",
            "diluted shares",
            "common shares outstanding",
        ],
    ),
    (
        SHARE_PRICE,
        &[
            "share price",
            "price per share",
            "stock price",
            "market price",
        ],
    ),
];

// ---------------------------------------------------------------------------
// Output model
// ---------------------------------------------------------------------------

/// Ratio family. `Group::Market` rows only appear when share data is pasted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    Liquidity,
    Leverage,
    Margins,
    Returns,
    Efficiency,
    Market,
}

impl Group {
    fn key(self) -> &'static str {
        match self {
            Group::Liquidity => "liquidity",
            Group::Leverage => "leverage",
            Group::Margins => "margins",
            Group::Returns => "returns",
            Group::Efficiency => "efficiency",
            Group::Market => "market",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Group::Liquidity => "Liquidity",
            Group::Leverage => "Leverage and solvency",
            Group::Margins => "Margins",
            Group::Returns => "Returns",
            Group::Efficiency => "Efficiency",
            Group::Market => "Market",
        }
    }
}

const GROUP_ORDER: &[Group] = &[
    Group::Liquidity,
    Group::Leverage,
    Group::Margins,
    Group::Returns,
    Group::Efficiency,
    Group::Market,
];

/// How a ratio's value should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    /// A plain multiple, printed with an `x` suffix.
    Times,
    /// Percentage points (12.5 == 12.5%).
    Percent,
    /// A number of days.
    Days,
    /// A currency amount, printed with the currency symbol.
    Money,
    /// A dimensionless score (Altman Z).
    Score,
}

/// One computed ratio.
#[derive(Debug, Clone, Serialize)]
pub struct Ratio {
    /// Stable machine key, e.g. `current_ratio`.
    pub key: &'static str,
    /// Human label used in the summary and table output.
    pub label: &'static str,
    /// Ratio family.
    pub group: Group,
    /// How to render `value`.
    pub unit: Unit,
    /// The formula actually applied.
    pub formula: &'static str,
    /// The computed value, or `None` when inputs are missing.
    pub value: Option<f64>,
    /// Same ratio for the prior period, when `prior_figures` was supplied.
    pub prior: Option<f64>,
    /// `value - prior`.
    pub change: Option<f64>,
    /// Why the value is `None`, or an extra remark (e.g. the Altman variant).
    pub note: Option<String>,
    /// Low end of the rule-of-thumb healthy range, when one is defined.
    pub benchmark_low: Option<f64>,
    /// High end of the rule-of-thumb healthy range, when one is defined.
    pub benchmark_high: Option<f64>,
    /// `ok`, `low` or `high` against the benchmark range.
    pub status: Option<&'static str>,
}

/// Every figure the parser accepted for one statement.
#[derive(Debug, Clone, Serialize)]
pub struct Figures {
    /// Line items keyed by canonical name.
    pub items: BTreeMap<String, f64>,
    /// Canonical names that were computed rather than pasted.
    pub derived: Vec<String>,
}

/// ROE split into its three DuPont drivers.
#[derive(Debug, Clone, Serialize)]
pub struct DuPont {
    /// Net income / revenue, in percentage points.
    pub net_margin_pct: f64,
    /// Revenue / assets.
    pub asset_turnover: f64,
    /// Assets / equity.
    pub equity_multiplier: f64,
    /// The product, in percentage points — equals ROE.
    pub roe_pct: f64,
}

/// The complete analysis.
#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    /// Current-period figures after derivation.
    pub figures: Figures,
    /// Prior-period figures, when supplied.
    pub prior_figures: Option<Figures>,
    /// `average` or `ending` balance-sheet basis actually used.
    pub basis: String,
    /// Days used for the day-count ratios (DSO, DIO, DPO, CCC).
    pub days_in_period: f64,
    /// Ratio family filter that was applied (`all` or one group).
    pub groups: String,
    /// The ratios, in report order.
    pub ratios: Vec<Ratio>,
    /// DuPont decomposition of ROE, when computable.
    pub dupont: Option<DuPont>,
    /// Share of benchmarked ratios inside their healthy range, 0-100.
    pub health_score: Option<f64>,
    /// Benchmarked ratios that are inside their range.
    pub benchmarks_in_range: usize,
    /// Benchmarked ratios that had a value at all.
    pub benchmarks_checked: usize,
    /// Non-fatal remarks: ignored lines, unknown labels, identity mismatches.
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Value parsing
// ---------------------------------------------------------------------------

const CURRENCY_CHARS: &[char] = &[
    '$', '\u{20ac}', '\u{a3}', '\u{a5}', '\u{20b9}', '\u{20bd}', '\u{a2}',
];

/// Parse one pasted amount: `1,250,000`, `$1.2m`, `(4,500)`, `-3 000`, `2bn`.
fn parse_amount(raw: &str) -> Option<f64> {
    let mut s = raw.trim().to_string();
    if s.is_empty() || s.contains('%') {
        return None;
    }
    let mut negative = false;
    if s.starts_with('(') && s.ends_with(')') {
        negative = true;
        s = s[1..s.len() - 1].trim().to_string();
    }
    s = s
        .chars()
        .filter(|c| !CURRENCY_CHARS.contains(c) && *c != ',' && *c != '_' && *c != '\'')
        .collect();
    if let Some(rest) = s.strip_prefix('-') {
        negative = !negative;
        s = rest.trim().to_string();
    } else if let Some(rest) = s.strip_prefix('+') {
        s = rest.trim().to_string();
    }
    let lower = s.to_ascii_lowercase();
    let (digits, scale) = if let Some(d) = lower.strip_suffix("bn") {
        (d, 1e9)
    } else if let Some(d) = lower.strip_suffix("mm") {
        (d, 1e6)
    } else if let Some(d) = lower.strip_suffix('k') {
        (d, 1e3)
    } else if let Some(d) = lower.strip_suffix('m') {
        (d, 1e6)
    } else if let Some(d) = lower.strip_suffix('b') {
        (d, 1e9)
    } else {
        (lower.as_str(), 1.0)
    };
    let digits = digits.trim();
    if digits.is_empty() || !digits.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    // Reject exponent notation when a scale suffix was consumed ("1e3k").
    let v: f64 = digits.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    Some(if negative { -v * scale } else { v * scale })
}

/// Normalize a label for alias matching: lowercase, non-alphanumerics become
/// single spaces (`SG&A` -> `sg a`, `Total_Assets` -> `total assets`).
fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_space = true;
        }
    }
    out
}

fn canonical_key(label: &str) -> Option<&'static str> {
    let n = norm(label);
    if n.is_empty() {
        return None;
    }
    for (key, aliases) in ALIASES {
        if norm(key) == n {
            return Some(key);
        }
        for alias in *aliases {
            if *alias == n {
                return Some(key);
            }
        }
    }
    None
}

/// Split one line into `(label, amount)` by taking the LONGEST trailing run
/// that parses as an amount. This handles `Total assets: 1,250,000`,
/// `Revenue = 4.2m`, `Inventory<TAB>120000` and `Net loss (4,500)` with one
/// rule, without mistaking a thousands separator for a field separator.
fn split_line(line: &str) -> Option<(&str, f64)> {
    let trimmed = line.trim_end();
    for (i, ch) in trimmed.char_indices() {
        if !(ch.is_ascii_digit()
            || ch == '('
            || ch == '-'
            || ch == '+'
            || ch == '.'
            || CURRENCY_CHARS.contains(&ch))
        {
            continue;
        }
        if i > 0 {
            let prev = trimmed[..i].chars().next_back().unwrap();
            if prev.is_ascii_alphanumeric() {
                continue;
            }
        }
        let label = trimmed[..i]
            .trim()
            .trim_end_matches([':', '=', ',', ';', '\t', '-', '.']);
        if label.trim().is_empty() || !label.chars().any(|c| c.is_alphabetic()) {
            continue;
        }
        if let Some(v) = parse_amount(&trimmed[i..]) {
            return Some((label.trim(), v));
        }
    }
    None
}

struct Parsed {
    figures: Figures,
    warnings: Vec<String>,
}

fn parse_statement(text: &str, which: &str) -> Result<Parsed, String> {
    let lines: Vec<&str> = text
        .split(['\n', '\r', ';'])
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "{which} has {} lines; the maximum is {MAX_LINES}",
            lines.len()
        ));
    }
    let mut items: BTreeMap<String, f64> = BTreeMap::new();
    let mut unknown: Vec<String> = Vec::new();
    let mut ignored: Vec<String> = Vec::new();
    let mut duplicated: Vec<String> = Vec::new();
    for line in &lines {
        match split_line(line) {
            Some((label, value)) => match canonical_key(label) {
                Some(key) => {
                    if items.insert(key.to_string(), value).is_some() {
                        duplicated.push(key.to_string());
                    }
                }
                None => unknown.push(label.to_string()),
            },
            None => ignored.push((*line).to_string()),
        }
    }
    if items.is_empty() {
        return Err(format!(
            "no recognized line items in {which}. Use `label: value` lines such as `Revenue: 500000`, `Net income: 40000`, `Total assets: 300000`, `Current liabilities: 80000`"
        ));
    }
    let mut warnings = Vec::new();
    if !duplicated.is_empty() {
        warnings.push(format!(
            "{which}: repeated line item{} kept the last value: {}",
            plural(duplicated.len()),
            duplicated.join(", ")
        ));
    }
    if !unknown.is_empty() {
        warnings.push(format!(
            "{which}: {} unrecognized label{} ignored: {}",
            unknown.len(),
            plural(unknown.len()),
            quoted_list(&unknown)
        ));
    }
    if !ignored.is_empty() {
        warnings.push(format!(
            "{which}: {} line{} had no readable amount and {} skipped: {}",
            ignored.len(),
            plural(ignored.len()),
            if ignored.len() == 1 { "was" } else { "were" },
            quoted_list(&ignored)
        ));
    }
    let derived = derive(&mut items);
    Ok(Parsed {
        figures: Figures { items, derived },
        warnings,
    })
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn quoted_list(v: &[String]) -> String {
    let shown: Vec<String> = v.iter().take(5).map(|s| format!("\"{s}\"")).collect();
    let mut out = shown.join(", ");
    if v.len() > 5 {
        let _ = write!(out, " and {} more", v.len() - 5);
    }
    out
}

/// Fill in subtotals the accounting identities allow. Returns the canonical
/// names that were computed rather than pasted, in report order.
fn derive(items: &mut BTreeMap<String, f64>) -> Vec<String> {
    let mut derived: Vec<String> = Vec::new();
    let get = |m: &BTreeMap<String, f64>, k: &str| m.get(k).copied();
    for _ in 0..4 {
        let set = |m: &mut BTreeMap<String, f64>, derived: &mut Vec<String>, k: &str, v: f64| {
            if !m.contains_key(k) && v.is_finite() {
                m.insert(k.to_string(), v);
                derived.push(k.to_string());
            }
        };
        // Balance sheet
        if !items.contains_key(CURRENT_ASSETS) {
            let parts = [
                CASH,
                MARKETABLE_SECURITIES,
                ACCOUNTS_RECEIVABLE,
                INVENTORY,
                PREPAID_EXPENSES,
                OTHER_CURRENT_ASSETS,
            ];
            if parts.iter().any(|k| items.contains_key(*k)) {
                let sum: f64 = parts.iter().filter_map(|k| get(items, k)).sum();
                set(items, &mut derived, CURRENT_ASSETS, sum);
            }
        }
        if !items.contains_key(CURRENT_LIABILITIES) {
            let parts = [ACCOUNTS_PAYABLE, SHORT_TERM_DEBT, OTHER_CURRENT_LIABILITIES];
            if parts.iter().any(|k| items.contains_key(*k)) {
                let sum: f64 = parts.iter().filter_map(|k| get(items, k)).sum();
                set(items, &mut derived, CURRENT_LIABILITIES, sum);
            }
        }
        if let (Some(ca), Some(fa)) = (get(items, CURRENT_ASSETS), get(items, FIXED_ASSETS)) {
            set(items, &mut derived, TOTAL_ASSETS, ca + fa);
        }
        if let (Some(ta), Some(ca)) = (get(items, TOTAL_ASSETS), get(items, CURRENT_ASSETS)) {
            set(items, &mut derived, FIXED_ASSETS, ta - ca);
        }
        if !items.contains_key(TOTAL_LIABILITIES) {
            if let Some(cl) = get(items, CURRENT_LIABILITIES) {
                let ltd = get(items, LONG_TERM_DEBT).unwrap_or(0.0);
                let other = get(items, OTHER_LONG_TERM_LIABILITIES).unwrap_or(0.0);
                set(items, &mut derived, TOTAL_LIABILITIES, cl + ltd + other);
            } else if let (Some(ta), Some(te)) =
                (get(items, TOTAL_ASSETS), get(items, TOTAL_EQUITY))
            {
                set(items, &mut derived, TOTAL_LIABILITIES, ta - te);
            }
        }
        if let (Some(ta), Some(tl)) = (get(items, TOTAL_ASSETS), get(items, TOTAL_LIABILITIES)) {
            set(items, &mut derived, TOTAL_EQUITY, ta - tl);
        }
        if let (Some(tl), Some(te)) = (get(items, TOTAL_LIABILITIES), get(items, TOTAL_EQUITY)) {
            set(items, &mut derived, TOTAL_ASSETS, tl + te);
        }
        // Income statement
        if let (Some(rev), Some(cogs)) = (get(items, REVENUE), get(items, COGS)) {
            set(items, &mut derived, GROSS_PROFIT, rev - cogs);
        }
        if let (Some(rev), Some(gp)) = (get(items, REVENUE), get(items, GROSS_PROFIT)) {
            set(items, &mut derived, COGS, rev - gp);
        }
        if !items.contains_key(OPERATING_INCOME) {
            if let (Some(gp), Some(opex)) =
                (get(items, GROSS_PROFIT), get(items, OPERATING_EXPENSES))
            {
                set(items, &mut derived, OPERATING_INCOME, gp - opex);
            } else if let (Some(e), Some(da)) =
                (get(items, EBITDA), get(items, DEPRECIATION_AMORTIZATION))
            {
                set(items, &mut derived, OPERATING_INCOME, e - da);
            } else if let (Some(pbt), Some(int)) =
                (get(items, PRETAX_INCOME), get(items, INTEREST_EXPENSE))
            {
                set(items, &mut derived, OPERATING_INCOME, pbt + int);
            }
        }
        if let (Some(oi), Some(da)) = (
            get(items, OPERATING_INCOME),
            get(items, DEPRECIATION_AMORTIZATION),
        ) {
            set(items, &mut derived, EBITDA, oi + da);
        }
        if !items.contains_key(PRETAX_INCOME) {
            if let (Some(ni), Some(tax)) = (get(items, NET_INCOME), get(items, TAXES)) {
                set(items, &mut derived, PRETAX_INCOME, ni + tax);
            } else if let Some(oi) = get(items, OPERATING_INCOME) {
                let int = get(items, INTEREST_EXPENSE).unwrap_or(0.0);
                if items.contains_key(INTEREST_EXPENSE) {
                    set(items, &mut derived, PRETAX_INCOME, oi - int);
                }
            }
        }
        if let (Some(pbt), Some(tax)) = (get(items, PRETAX_INCOME), get(items, TAXES)) {
            set(items, &mut derived, NET_INCOME, pbt - tax);
        }
        if let (Some(pbt), Some(ni)) = (get(items, PRETAX_INCOME), get(items, NET_INCOME)) {
            set(items, &mut derived, TAXES, pbt - ni);
        }
    }
    derived
}

// ---------------------------------------------------------------------------
// Ratio computation
// ---------------------------------------------------------------------------

type Sheet = BTreeMap<String, f64>;

fn need(s: &Sheet, keys: &[&str]) -> Result<Vec<f64>, String> {
    let missing: Vec<&str> = keys
        .iter()
        .copied()
        .filter(|k| !s.contains_key(*k))
        .collect();
    if missing.is_empty() {
        Ok(keys.iter().map(|k| s[*k]).collect())
    } else {
        Err(format!("needs {}", missing.join(" + ")))
    }
}

fn div(num: f64, den: f64, den_label: &str) -> Result<f64, String> {
    if den == 0.0 {
        Err(format!("{den_label} is zero"))
    } else {
        Ok(num / den)
    }
}

fn opt(s: &Sheet, k: &str) -> f64 {
    s.get(k).copied().unwrap_or(0.0)
}

struct Builder {
    out: Vec<Ratio>,
}

impl Builder {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        key: &'static str,
        label: &'static str,
        group: Group,
        unit: Unit,
        formula: &'static str,
        value: Result<f64, String>,
        bench: (Option<f64>, Option<f64>),
    ) {
        let (value, note) = match value {
            Ok(v) if v.is_finite() => (Some(v), None),
            Ok(_) => (None, Some("result is not a finite number".to_string())),
            Err(e) => (None, Some(e)),
        };
        let status = value.and_then(|v| status_for(v, bench));
        self.out.push(Ratio {
            key,
            label,
            group,
            unit,
            formula,
            value,
            prior: None,
            change: None,
            note,
            benchmark_low: bench.0,
            benchmark_high: bench.1,
            status,
        });
    }
}

fn status_for(v: f64, bench: (Option<f64>, Option<f64>)) -> Option<&'static str> {
    match bench {
        (None, None) => None,
        (low, high) => {
            if low.is_some_and(|l| v < l) {
                Some("low")
            } else if high.is_some_and(|h| v > h) {
                Some("high")
            } else {
                Some("ok")
            }
        }
    }
}

const NO_BENCH: (Option<f64>, Option<f64>) = (None, None);

/// Compute every ratio. `end` holds ending balances and all flow figures;
/// `avg` holds the balance-sheet figures to use as denominators (identical to
/// `end` unless a prior period was supplied on an `average` basis).
fn compute(end: &Sheet, avg: &Sheet, days: f64) -> Vec<Ratio> {
    let mut b = Builder { out: Vec::new() };

    // ---- Liquidity ------------------------------------------------------
    b.add(
        "current_ratio",
        "Current ratio",
        Group::Liquidity,
        Unit::Times,
        "current_assets / current_liabilities",
        need(end, &[CURRENT_ASSETS, CURRENT_LIABILITIES])
            .and_then(|v| div(v[0], v[1], CURRENT_LIABILITIES)),
        (Some(1.5), Some(3.0)),
    );
    let quick_assets = if end.contains_key(CASH)
        || end.contains_key(MARKETABLE_SECURITIES)
        || end.contains_key(ACCOUNTS_RECEIVABLE)
    {
        Ok(opt(end, CASH) + opt(end, MARKETABLE_SECURITIES) + opt(end, ACCOUNTS_RECEIVABLE))
    } else {
        need(end, &[CURRENT_ASSETS, INVENTORY])
            .map(|v| v[0] - v[1] - opt(end, PREPAID_EXPENSES))
            .map_err(|_| {
                "needs cash + marketable_securities + accounts_receivable, or current_assets + inventory"
                    .to_string()
            })
    };
    b.add(
        "quick_ratio",
        "Quick ratio (acid test)",
        Group::Liquidity,
        Unit::Times,
        "(cash + marketable_securities + accounts_receivable) / current_liabilities",
        quick_assets.and_then(|qa| {
            need(end, &[CURRENT_LIABILITIES]).and_then(|v| div(qa, v[0], CURRENT_LIABILITIES))
        }),
        (Some(1.0), None),
    );
    let cash_assets = if end.contains_key(CASH) || end.contains_key(MARKETABLE_SECURITIES) {
        Ok(opt(end, CASH) + opt(end, MARKETABLE_SECURITIES))
    } else {
        Err("needs cash + marketable_securities".to_string())
    };
    b.add(
        "cash_ratio",
        "Cash ratio",
        Group::Liquidity,
        Unit::Times,
        "(cash + marketable_securities) / current_liabilities",
        cash_assets.and_then(|ca| {
            need(end, &[CURRENT_LIABILITIES]).and_then(|v| div(ca, v[0], CURRENT_LIABILITIES))
        }),
        (Some(0.2), None),
    );
    b.add(
        "working_capital",
        "Net working capital",
        Group::Liquidity,
        Unit::Money,
        "current_assets - current_liabilities",
        need(end, &[CURRENT_ASSETS, CURRENT_LIABILITIES]).map(|v| v[0] - v[1]),
        (Some(0.0), None),
    );
    b.add(
        "working_capital_to_revenue",
        "Working capital / revenue",
        Group::Liquidity,
        Unit::Percent,
        "(current_assets - current_liabilities) / revenue",
        need(end, &[CURRENT_ASSETS, CURRENT_LIABILITIES, REVENUE])
            .and_then(|v| div((v[0] - v[1]) * 100.0, v[2], REVENUE)),
        NO_BENCH,
    );

    // ---- Leverage and solvency ------------------------------------------
    b.add(
        "debt_to_equity",
        "Debt to equity",
        Group::Leverage,
        Unit::Times,
        "total_liabilities / total_equity",
        need(end, &[TOTAL_LIABILITIES, TOTAL_EQUITY]).and_then(|v| div(v[0], v[1], TOTAL_EQUITY)),
        (None, Some(2.0)),
    );
    b.add(
        "debt_ratio",
        "Debt ratio",
        Group::Leverage,
        Unit::Times,
        "total_liabilities / total_assets",
        need(end, &[TOTAL_LIABILITIES, TOTAL_ASSETS]).and_then(|v| div(v[0], v[1], TOTAL_ASSETS)),
        (None, Some(0.6)),
    );
    b.add(
        "equity_ratio",
        "Equity ratio",
        Group::Leverage,
        Unit::Times,
        "total_equity / total_assets",
        need(end, &[TOTAL_EQUITY, TOTAL_ASSETS]).and_then(|v| div(v[0], v[1], TOTAL_ASSETS)),
        (Some(0.4), None),
    );
    b.add(
        "equity_multiplier",
        "Equity multiplier",
        Group::Leverage,
        Unit::Times,
        "total_assets / total_equity",
        need(end, &[TOTAL_ASSETS, TOTAL_EQUITY]).and_then(|v| div(v[0], v[1], TOTAL_EQUITY)),
        NO_BENCH,
    );
    b.add(
        "long_term_debt_to_equity",
        "Long-term debt to equity",
        Group::Leverage,
        Unit::Times,
        "long_term_debt / total_equity",
        need(end, &[LONG_TERM_DEBT, TOTAL_EQUITY]).and_then(|v| div(v[0], v[1], TOTAL_EQUITY)),
        NO_BENCH,
    );
    let interest_bearing = if end.contains_key(SHORT_TERM_DEBT) || end.contains_key(LONG_TERM_DEBT)
    {
        Ok(opt(end, SHORT_TERM_DEBT) + opt(end, LONG_TERM_DEBT))
    } else {
        Err("needs short_term_debt + long_term_debt".to_string())
    };
    b.add(
        "net_debt",
        "Net debt",
        Group::Leverage,
        Unit::Money,
        "short_term_debt + long_term_debt - cash - marketable_securities",
        interest_bearing
            .clone()
            .map(|d| d - opt(end, CASH) - opt(end, MARKETABLE_SECURITIES)),
        NO_BENCH,
    );
    b.add(
        "net_debt_to_ebitda",
        "Net debt / EBITDA",
        Group::Leverage,
        Unit::Times,
        "(short_term_debt + long_term_debt - cash - marketable_securities) / ebitda",
        interest_bearing.and_then(|d| {
            let net = d - opt(end, CASH) - opt(end, MARKETABLE_SECURITIES);
            need(end, &[EBITDA]).and_then(|v| div(net, v[0], EBITDA))
        }),
        (None, Some(3.0)),
    );
    b.add(
        "interest_coverage",
        "Interest coverage (EBIT)",
        Group::Leverage,
        Unit::Times,
        "operating_income / interest_expense",
        need(end, &[OPERATING_INCOME, INTEREST_EXPENSE])
            .and_then(|v| div(v[0], v[1], INTEREST_EXPENSE)),
        (Some(3.0), None),
    );
    b.add(
        "ebitda_interest_coverage",
        "Interest coverage (EBITDA)",
        Group::Leverage,
        Unit::Times,
        "ebitda / interest_expense",
        need(end, &[EBITDA, INTEREST_EXPENSE]).and_then(|v| div(v[0], v[1], INTEREST_EXPENSE)),
        NO_BENCH,
    );
    let (z_value, z_note, z_low) = altman_z(end);
    b.out.push(Ratio {
        key: "altman_z_score",
        label: "Altman Z-Score",
        group: Group::Leverage,
        unit: Unit::Score,
        formula: "weighted working capital, retained earnings, EBIT, equity/liabilities and sales over assets",
        value: z_value,
        prior: None,
        change: None,
        note: Some(z_note),
        benchmark_low: z_low,
        benchmark_high: None,
        status: z_value.and_then(|v| status_for(v, (z_low, None))),
    });

    // ---- Margins ---------------------------------------------------------
    b.add(
        "gross_margin",
        "Gross margin",
        Group::Margins,
        Unit::Percent,
        "gross_profit / revenue",
        need(end, &[GROSS_PROFIT, REVENUE]).and_then(|v| div(v[0] * 100.0, v[1], REVENUE)),
        NO_BENCH,
    );
    b.add(
        "operating_margin",
        "Operating margin",
        Group::Margins,
        Unit::Percent,
        "operating_income / revenue",
        need(end, &[OPERATING_INCOME, REVENUE]).and_then(|v| div(v[0] * 100.0, v[1], REVENUE)),
        (Some(5.0), None),
    );
    b.add(
        "ebitda_margin",
        "EBITDA margin",
        Group::Margins,
        Unit::Percent,
        "ebitda / revenue",
        need(end, &[EBITDA, REVENUE]).and_then(|v| div(v[0] * 100.0, v[1], REVENUE)),
        NO_BENCH,
    );
    b.add(
        "pretax_margin",
        "Pretax margin",
        Group::Margins,
        Unit::Percent,
        "pretax_income / revenue",
        need(end, &[PRETAX_INCOME, REVENUE]).and_then(|v| div(v[0] * 100.0, v[1], REVENUE)),
        NO_BENCH,
    );
    b.add(
        "net_margin",
        "Net profit margin",
        Group::Margins,
        Unit::Percent,
        "net_income / revenue",
        need(end, &[NET_INCOME, REVENUE]).and_then(|v| div(v[0] * 100.0, v[1], REVENUE)),
        (Some(5.0), None),
    );

    // ---- Returns ---------------------------------------------------------
    b.add(
        "return_on_assets",
        "Return on assets (ROA)",
        Group::Returns,
        Unit::Percent,
        "net_income / total_assets",
        need(end, &[NET_INCOME]).and_then(|ni| {
            need(avg, &[TOTAL_ASSETS]).and_then(|ta| div(ni[0] * 100.0, ta[0], TOTAL_ASSETS))
        }),
        (Some(5.0), None),
    );
    b.add(
        "return_on_equity",
        "Return on equity (ROE)",
        Group::Returns,
        Unit::Percent,
        "net_income / total_equity",
        need(end, &[NET_INCOME]).and_then(|ni| {
            need(avg, &[TOTAL_EQUITY]).and_then(|te| div(ni[0] * 100.0, te[0], TOTAL_EQUITY))
        }),
        (Some(10.0), None),
    );
    b.add(
        "return_on_capital_employed",
        "Return on capital employed",
        Group::Returns,
        Unit::Percent,
        "operating_income / (total_assets - current_liabilities)",
        need(end, &[OPERATING_INCOME]).and_then(|oi| {
            need(avg, &[TOTAL_ASSETS, CURRENT_LIABILITIES])
                .and_then(|v| div(oi[0] * 100.0, v[0] - v[1], "capital employed"))
        }),
        NO_BENCH,
    );
    b.add(
        "return_on_invested_capital",
        "Return on invested capital",
        Group::Returns,
        Unit::Percent,
        "operating_income * (1 - effective tax rate) / (total_liabilities interest-bearing + total_equity)",
        roic(end, avg),
        NO_BENCH,
    );

    // ---- Efficiency ------------------------------------------------------
    b.add(
        "asset_turnover",
        "Asset turnover",
        Group::Efficiency,
        Unit::Times,
        "revenue / total_assets",
        need(end, &[REVENUE])
            .and_then(|r| need(avg, &[TOTAL_ASSETS]).and_then(|v| div(r[0], v[0], TOTAL_ASSETS))),
        NO_BENCH,
    );
    b.add(
        "fixed_asset_turnover",
        "Fixed-asset turnover",
        Group::Efficiency,
        Unit::Times,
        "revenue / fixed_assets",
        need(end, &[REVENUE])
            .and_then(|r| need(avg, &[FIXED_ASSETS]).and_then(|v| div(r[0], v[0], FIXED_ASSETS))),
        NO_BENCH,
    );
    let inv_turnover = need(end, &[COGS])
        .and_then(|c| need(avg, &[INVENTORY]).and_then(|v| div(c[0], v[0], INVENTORY)));
    b.add(
        "inventory_turnover",
        "Inventory turnover",
        Group::Efficiency,
        Unit::Times,
        "cogs / inventory",
        inv_turnover.clone(),
        NO_BENCH,
    );
    b.add(
        "days_inventory_outstanding",
        "Days inventory outstanding",
        Group::Efficiency,
        Unit::Days,
        "days_in_period / inventory_turnover",
        inv_turnover
            .clone()
            .and_then(|t| div(days, t, "inventory turnover")),
        NO_BENCH,
    );
    let ar_turnover = need(end, &[REVENUE]).and_then(|r| {
        need(avg, &[ACCOUNTS_RECEIVABLE]).and_then(|v| div(r[0], v[0], ACCOUNTS_RECEIVABLE))
    });
    b.add(
        "receivables_turnover",
        "Receivables turnover",
        Group::Efficiency,
        Unit::Times,
        "revenue / accounts_receivable",
        ar_turnover.clone(),
        NO_BENCH,
    );
    b.add(
        "days_sales_outstanding",
        "Days sales outstanding",
        Group::Efficiency,
        Unit::Days,
        "days_in_period / receivables_turnover",
        ar_turnover
            .clone()
            .and_then(|t| div(days, t, "receivables turnover")),
        NO_BENCH,
    );
    let ap_turnover = need(end, &[COGS]).and_then(|c| {
        need(avg, &[ACCOUNTS_PAYABLE]).and_then(|v| div(c[0], v[0], ACCOUNTS_PAYABLE))
    });
    b.add(
        "payables_turnover",
        "Payables turnover",
        Group::Efficiency,
        Unit::Times,
        "cogs / accounts_payable",
        ap_turnover.clone(),
        NO_BENCH,
    );
    b.add(
        "days_payables_outstanding",
        "Days payables outstanding",
        Group::Efficiency,
        Unit::Days,
        "days_in_period / payables_turnover",
        ap_turnover
            .clone()
            .and_then(|t| div(days, t, "payables turnover")),
        NO_BENCH,
    );
    let ccc = (|| -> Result<f64, String> {
        let dio = div(days, inv_turnover.clone()?, "inventory turnover")?;
        let dso = div(days, ar_turnover.clone()?, "receivables turnover")?;
        let dpo = div(days, ap_turnover.clone()?, "payables turnover")?;
        Ok(dio + dso - dpo)
    })();
    b.add(
        "cash_conversion_cycle",
        "Cash conversion cycle",
        Group::Efficiency,
        Unit::Days,
        "days_inventory_outstanding + days_sales_outstanding - days_payables_outstanding",
        ccc,
        NO_BENCH,
    );
    b.add(
        "working_capital_turnover",
        "Working-capital turnover",
        Group::Efficiency,
        Unit::Times,
        "revenue / (current_assets - current_liabilities)",
        need(end, &[REVENUE]).and_then(|r| {
            need(avg, &[CURRENT_ASSETS, CURRENT_LIABILITIES])
                .and_then(|v| div(r[0], v[0] - v[1], "working capital"))
        }),
        NO_BENCH,
    );

    // ---- Market ----------------------------------------------------------
    let eps = need(end, &[NET_INCOME, SHARES_OUTSTANDING])
        .and_then(|v| div(v[0], v[1], SHARES_OUTSTANDING));
    b.add(
        "earnings_per_share",
        "Earnings per share",
        Group::Market,
        Unit::Money,
        "net_income / shares_outstanding",
        eps.clone(),
        NO_BENCH,
    );
    let bvps = need(end, &[TOTAL_EQUITY, SHARES_OUTSTANDING])
        .and_then(|v| div(v[0], v[1], SHARES_OUTSTANDING));
    b.add(
        "book_value_per_share",
        "Book value per share",
        Group::Market,
        Unit::Money,
        "total_equity / shares_outstanding",
        bvps.clone(),
        NO_BENCH,
    );
    b.add(
        "market_capitalization",
        "Market capitalization",
        Group::Market,
        Unit::Money,
        "shares_outstanding * share_price",
        need(end, &[SHARES_OUTSTANDING, SHARE_PRICE]).map(|v| v[0] * v[1]),
        NO_BENCH,
    );
    b.add(
        "price_to_earnings",
        "Price / earnings",
        Group::Market,
        Unit::Times,
        "share_price / earnings_per_share",
        eps.clone().and_then(|e| {
            need(end, &[SHARE_PRICE]).and_then(|p| div(p[0], e, "earnings per share"))
        }),
        NO_BENCH,
    );
    b.add(
        "price_to_book",
        "Price / book",
        Group::Market,
        Unit::Times,
        "share_price / book_value_per_share",
        bvps.and_then(|bv| {
            need(end, &[SHARE_PRICE]).and_then(|p| div(p[0], bv, "book value per share"))
        }),
        NO_BENCH,
    );
    b.add(
        "earnings_yield",
        "Earnings yield",
        Group::Market,
        Unit::Percent,
        "earnings_per_share / share_price",
        eps.and_then(|e| need(end, &[SHARE_PRICE]).and_then(|p| div(e * 100.0, p[0], SHARE_PRICE))),
        NO_BENCH,
    );

    b.out
}

/// Return on invested capital, using the effective tax rate implied by the
/// pasted tax and pretax figures. Invested capital is interest-bearing debt
/// plus equity when the debt split is known, otherwise total capital employed.
fn roic(end: &Sheet, avg: &Sheet) -> Result<f64, String> {
    let oi = need(end, &[OPERATING_INCOME])?[0];
    let pbt = need(end, &[PRETAX_INCOME])
        .map_err(|_| "needs pretax_income + taxes to derive the effective tax rate".to_string())?
        [0];
    let tax = need(end, &[TAXES])
        .map_err(|_| "needs pretax_income + taxes to derive the effective tax rate".to_string())?
        [0];
    let rate = if pbt == 0.0 {
        0.0
    } else {
        (tax / pbt).clamp(0.0, 1.0)
    };
    let equity = need(avg, &[TOTAL_EQUITY])?[0];
    let debt = if avg.contains_key(SHORT_TERM_DEBT) || avg.contains_key(LONG_TERM_DEBT) {
        opt(avg, SHORT_TERM_DEBT) + opt(avg, LONG_TERM_DEBT)
    } else {
        need(avg, &[TOTAL_LIABILITIES])?[0]
    };
    div(oi * (1.0 - rate) * 100.0, debt + equity, "invested capital")
}

/// Altman Z-Score. The public-company Z is used when a market capitalization
/// can be formed from `shares_outstanding * share_price`; otherwise the
/// private-company Z' variant with book equity and its own weights is used.
fn altman_z(end: &Sheet) -> (Option<f64>, String, Option<f64>) {
    let required = [
        CURRENT_ASSETS,
        CURRENT_LIABILITIES,
        RETAINED_EARNINGS,
        OPERATING_INCOME,
        TOTAL_ASSETS,
        TOTAL_LIABILITIES,
        REVENUE,
        TOTAL_EQUITY,
    ];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|k| !end.contains_key(*k))
        .collect();
    if !missing.is_empty() {
        return (None, format!("needs {}", missing.join(" + ")), None);
    }
    let ta = end[TOTAL_ASSETS];
    let tl = end[TOTAL_LIABILITIES];
    if ta == 0.0 || tl == 0.0 {
        return (
            None,
            "total_assets and total_liabilities must be non-zero".to_string(),
            None,
        );
    }
    let wc = end[CURRENT_ASSETS] - end[CURRENT_LIABILITIES];
    let re = end[RETAINED_EARNINGS];
    let ebit = end[OPERATING_INCOME];
    let sales = end[REVENUE];
    let market_cap = match (end.get(SHARES_OUTSTANDING), end.get(SHARE_PRICE)) {
        (Some(s), Some(p)) => Some(s * p),
        _ => None,
    };
    let (z, variant, distress, safe) = match market_cap {
        Some(mve) => (
            1.2 * wc / ta + 1.4 * re / ta + 3.3 * ebit / ta + 0.6 * mve / tl + 1.0 * sales / ta,
            "public-company Z (market value of equity)",
            1.81,
            2.99,
        ),
        None => (
            0.717 * wc / ta
                + 0.847 * re / ta
                + 3.107 * ebit / ta
                + 0.420 * end[TOTAL_EQUITY] / tl
                + 0.998 * sales / ta,
            "private-company Z' (book value of equity)",
            1.23,
            2.90,
        ),
    };
    let zone = if z < distress {
        "distress zone"
    } else if z < safe {
        "grey zone"
    } else {
        "safe zone"
    };
    (Some(z), format!("{variant}; {zone}"), Some(safe))
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn fmt_plain(v: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, v);
    let negative = s.starts_with('-');
    let body = s.trim_start_matches('-');
    let (int_part, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let mut grouped = String::new();
    let bytes = int_part.as_bytes();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*c as char);
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&grouped);
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

fn fmt_money(v: f64, currency: &str, decimals: usize) -> String {
    let body = fmt_plain(v.abs(), decimals);
    if v < 0.0 {
        format!("-{currency}{body}")
    } else {
        format!("{currency}{body}")
    }
}

fn fmt_value(v: f64, unit: Unit, currency: &str, decimals: usize) -> String {
    match unit {
        Unit::Times => format!("{}x", fmt_plain(v, decimals)),
        Unit::Percent => format!("{}%", fmt_plain(v, decimals)),
        Unit::Days => format!("{} d", fmt_plain(v, decimals)),
        Unit::Money => fmt_money(v, currency, decimals),
        Unit::Score => fmt_plain(v, decimals),
    }
}

fn fmt_change(v: f64, unit: Unit, currency: &str, decimals: usize) -> String {
    let body = match unit {
        Unit::Money => fmt_money(v.abs(), currency, decimals),
        _ => fmt_value(v.abs(), unit, currency, decimals),
    };
    let sign = if v > 0.0 {
        "+"
    } else if v < 0.0 {
        "-"
    } else {
        " "
    };
    format!("{sign}{body}")
}

fn fmt_bench(r: &Ratio, currency: &str, decimals: usize) -> String {
    let low = r
        .benchmark_low
        .map(|v| fmt_value(v, r.unit, currency, decimals));
    let high = r
        .benchmark_high
        .map(|v| fmt_value(v, r.unit, currency, decimals));
    match (low, high) {
        (Some(l), Some(h)) => format!("target {l}-{h}"),
        (Some(l), None) => format!("target >= {l}"),
        (None, Some(h)) => format!("target <= {h}"),
        (None, None) => String::new(),
    }
}

fn pad_right(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        format!("{s} ")
    } else {
        format!("{s}{}", " ".repeat(width - len))
    }
}

fn pad_left(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        format!(" {s}")
    } else {
        format!("{}{s}", " ".repeat(width - len))
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

const GROUP_CHOICES: &[&str] = &[
    "all",
    "liquidity",
    "leverage",
    "margins",
    "returns",
    "efficiency",
    "market",
];
const BASIS_CHOICES: &[&str] = &["average", "ending"];
const OUTPUT_CHOICES: &[&str] = &["summary", "table", "csv", "json"];

fn check_choice(name: &str, value: &str, choices: &[&str]) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    if choices.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!(
            "{name} must be one of {}, got `{}`",
            choices.join(", "),
            value.trim()
        ))
    }
}

/// Run the analysis and render it in the requested `output` shape.
#[allow(clippy::too_many_arguments)]
pub fn run(
    figures: &str,
    prior_figures: &str,
    groups: &str,
    basis: &str,
    days_in_period: i64,
    benchmarks: bool,
    decimals: i64,
    currency: &str,
    output: &str,
) -> Result<String, String> {
    if figures.trim().is_empty() {
        return Err("figures is required: paste `label: value` lines such as `Revenue: 500000`, `Net income: 40000`, `Total assets: 300000`, `Current liabilities: 80000`".to_string());
    }
    let groups = check_choice("groups", groups, GROUP_CHOICES)?;
    let basis_req = check_choice("basis", basis, BASIS_CHOICES)?;
    let output = check_choice("output", output, OUTPUT_CHOICES)?;
    if !(1..=366).contains(&days_in_period) {
        return Err(format!(
            "days_in_period must be between 1 and 366, got {days_in_period}"
        ));
    }
    if !(0..=6).contains(&decimals) {
        return Err(format!("decimals must be between 0 and 6, got {decimals}"));
    }
    let decimals = decimals as usize;
    let days = days_in_period as f64;

    let current = parse_statement(figures, "figures")?;
    let prior = if prior_figures.trim().is_empty() {
        None
    } else {
        Some(parse_statement(prior_figures, "prior_figures")?)
    };

    let mut warnings = current.warnings.clone();
    if let Some(p) = &prior {
        warnings.extend(p.warnings.clone());
    }
    warnings.extend(identity_warnings(
        &current.figures.items,
        "figures",
        currency,
        decimals,
    ));
    if let Some(p) = &prior {
        warnings.extend(identity_warnings(
            &p.figures.items,
            "prior_figures",
            currency,
            decimals,
        ));
    }
    if current
        .figures
        .items
        .get(TOTAL_EQUITY)
        .is_some_and(|v| *v <= 0.0)
    {
        warnings.push(
            "total_equity is zero or negative, so equity-based ratios (ROE, debt to equity) are not meaningful".to_string(),
        );
    }

    // Balance-sheet denominators: averaged with the prior period when asked for
    // and available, otherwise the ending balances.
    let effective_basis = if prior.is_some() && basis_req == "average" {
        "average"
    } else {
        "ending"
    };
    let mut avg = current.figures.items.clone();
    if effective_basis == "average" {
        let p = &prior.as_ref().unwrap().figures.items;
        for (k, v) in avg.iter_mut() {
            if let Some(pv) = p.get(k) {
                *v = (*v + pv) / 2.0;
            }
        }
    }

    let mut ratios = compute(&current.figures.items, &avg, days);
    if let Some(p) = &prior {
        let prior_ratios = compute(&p.figures.items, &p.figures.items, days);
        for (r, pr) in ratios.iter_mut().zip(prior_ratios.iter()) {
            r.prior = pr.value;
            r.change = match (r.value, pr.value) {
                (Some(a), Some(b)) => Some(a - b),
                _ => None,
            };
        }
    }
    if groups != "all" {
        ratios.retain(|r| r.group.key() == groups);
    } else {
        // Market ratios only make sense with share data; drop the whole family
        // when none was pasted rather than printing six n/a rows.
        let has_shares = current.figures.items.contains_key(SHARES_OUTSTANDING)
            || current.figures.items.contains_key(SHARE_PRICE);
        if !has_shares {
            ratios.retain(|r| r.group != Group::Market);
        }
    }
    if ratios.is_empty() {
        return Err(format!(
            "no ratios to report for groups={groups}; try groups=all"
        ));
    }

    let checked = ratios
        .iter()
        .filter(|r| r.value.is_some() && r.status.is_some())
        .count();
    let in_range = ratios.iter().filter(|r| r.status == Some("ok")).count();
    let health = if checked == 0 {
        None
    } else {
        Some(100.0 * in_range as f64 / checked as f64)
    };

    let dupont = dupont(&current.figures.items, &avg);

    let analysis = Analysis {
        figures: current.figures,
        prior_figures: prior.map(|p| p.figures),
        basis: effective_basis.to_string(),
        days_in_period: days,
        groups: groups.clone(),
        ratios,
        dupont,
        health_score: health,
        benchmarks_in_range: in_range,
        benchmarks_checked: checked,
        warnings,
    };

    match output.as_str() {
        "json" => serde_json::to_string_pretty(&analysis)
            .map_err(|e| format!("could not serialize the analysis: {e}")),
        "csv" => Ok(render_csv(&analysis)),
        "table" => Ok(render_sections(&analysis, currency, decimals, benchmarks)),
        _ => Ok(render_summary(&analysis, currency, decimals, benchmarks)),
    }
}

fn identity_warnings(items: &Sheet, which: &str, currency: &str, decimals: usize) -> Vec<String> {
    let mut out = Vec::new();
    if let (Some(ta), Some(tl), Some(te)) = (
        items.get(TOTAL_ASSETS),
        items.get(TOTAL_LIABILITIES),
        items.get(TOTAL_EQUITY),
    ) {
        let gap = ta - (tl + te);
        if gap.abs() > 0.005 * ta.abs().max(1.0) {
            out.push(format!(
                "{which}: assets {} do not equal liabilities {} plus equity {} (off by {})",
                fmt_money(*ta, currency, decimals),
                fmt_money(*tl, currency, decimals),
                fmt_money(*te, currency, decimals),
                fmt_money(gap, currency, decimals)
            ));
        }
    }
    out
}

fn dupont(end: &Sheet, avg: &Sheet) -> Option<DuPont> {
    let ni = *end.get(NET_INCOME)?;
    let rev = *end.get(REVENUE)?;
    let ta = *avg.get(TOTAL_ASSETS)?;
    let te = *avg.get(TOTAL_EQUITY)?;
    if rev == 0.0 || ta == 0.0 || te == 0.0 {
        return None;
    }
    let net_margin_pct = ni / rev * 100.0;
    let asset_turnover = rev / ta;
    let equity_multiplier = ta / te;
    Some(DuPont {
        net_margin_pct,
        asset_turnover,
        equity_multiplier,
        roe_pct: net_margin_pct * asset_turnover * equity_multiplier,
    })
}

const LABEL_W: usize = 30;
const VALUE_W: usize = 14;

fn render_row(
    r: &Ratio,
    currency: &str,
    decimals: usize,
    benchmarks: bool,
    has_prior: bool,
) -> String {
    let mut line = String::new();
    line.push_str("  ");
    line.push_str(&pad_right(r.label, LABEL_W));
    let value = match r.value {
        Some(v) => fmt_value(v, r.unit, currency, decimals),
        None => "n/a".to_string(),
    };
    line.push_str(&pad_left(&value, VALUE_W));
    if has_prior {
        let prior = match r.prior {
            Some(v) => fmt_value(v, r.unit, currency, decimals),
            None => "n/a".to_string(),
        };
        let change = match r.change {
            Some(v) => fmt_change(v, r.unit, currency, decimals),
            None => "n/a".to_string(),
        };
        line.push_str(&pad_left(&prior, VALUE_W));
        line.push_str(&pad_left(&change, VALUE_W));
    }
    let trailing = match (&r.value, &r.note) {
        (None, Some(note)) => note.clone(),
        (Some(_), _) => {
            let mut parts: Vec<String> = Vec::new();
            if benchmarks {
                if let Some(status) = r.status {
                    parts.push(format!("{status:<4}"));
                    let b = fmt_bench(r, currency, decimals);
                    if !b.is_empty() {
                        parts.push(b);
                    }
                }
            }
            if r.key == "altman_z_score" {
                if let Some(note) = &r.note {
                    parts.push(note.clone());
                }
            }
            parts.join(" ")
        }
        _ => String::new(),
    };
    if !trailing.is_empty() {
        line.push_str("   ");
        line.push_str(trailing.trim_end());
    }
    line.trim_end().to_string()
}

fn render_sections(a: &Analysis, currency: &str, decimals: usize, benchmarks: bool) -> String {
    let has_prior = a.prior_figures.is_some();
    let mut out = String::new();
    let mut first = true;
    for group in GROUP_ORDER {
        let rows: Vec<&Ratio> = a.ratios.iter().filter(|r| r.group == *group).collect();
        if rows.is_empty() {
            continue;
        }
        if !first {
            out.push('\n');
        }
        first = false;
        let _ = writeln!(out, "{}", group.title());
        if has_prior {
            let _ = writeln!(
                out,
                "  {}{}{}{}",
                pad_right("", LABEL_W),
                pad_left("current", VALUE_W),
                pad_left("prior", VALUE_W),
                pad_left("change", VALUE_W)
            );
        }
        for r in rows {
            let _ = writeln!(
                out,
                "{}",
                render_row(r, currency, decimals, benchmarks, has_prior)
            );
        }
    }
    out.trim_end().to_string()
}

fn render_summary(a: &Analysis, currency: &str, decimals: usize, benchmarks: bool) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "Financial ratio analysis: {} ratio{}",
        a.ratios.len(),
        plural(a.ratios.len())
    );
    let _ = writeln!(
        out,
        "Basis: {} balance-sheet figures | {}-day period | group: {}",
        a.basis,
        fmt_plain(a.days_in_period, 0),
        a.groups
    );
    if benchmarks {
        match a.health_score {
            Some(score) => {
                let _ = writeln!(
                    out,
                    "Health score: {} / 100 ({} of {} benchmarked ratios in range)",
                    fmt_plain(score, 0),
                    a.benchmarks_in_range,
                    a.benchmarks_checked
                );
            }
            None => {
                let _ = writeln!(
                    out,
                    "Health score: n/a (no benchmarked ratio could be computed)"
                );
            }
        }
    }
    out.push('\n');
    out.push_str(&render_sections(a, currency, decimals, benchmarks));
    out.push('\n');

    if let Some(d) = &a.dupont {
        let _ = write!(
            out,
            "\nDuPont\n  ROE = net margin {} x asset turnover {} x equity multiplier {} = {}\n",
            fmt_value(d.net_margin_pct, Unit::Percent, currency, decimals),
            fmt_value(d.asset_turnover, Unit::Times, currency, decimals),
            fmt_value(d.equity_multiplier, Unit::Times, currency, decimals),
            fmt_value(d.roe_pct, Unit::Percent, currency, decimals)
        );
    }

    let _ = write!(
        out,
        "\nFigures\n  Read {} line item{} from figures",
        a.figures.items.len(),
        plural(a.figures.items.len())
    );
    if let Some(p) = &a.prior_figures {
        let _ = write!(out, " and {} from prior_figures", p.items.len());
    }
    out.push('\n');
    if !a.figures.derived.is_empty() {
        let _ = writeln!(out, "  Derived: {}", a.figures.derived.join(", "));
    }

    if !a.warnings.is_empty() {
        out.push_str("\nWarnings\n");
        for w in &a.warnings {
            let _ = writeln!(out, "  - {w}");
        }
    }
    let _ = write!(
        out,
        "\nEducational arithmetic only, not financial, investment, tax or accounting advice."
    );
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(a: &Analysis) -> String {
    let mut out = String::from(
        "group,key,label,value,unit,prior,change,benchmark_low,benchmark_high,status,formula,note\n",
    );
    let num = |v: Option<f64>| v.map(|x| format!("{x}")).unwrap_or_default();
    for r in &a.ratios {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            r.group.key(),
            r.key,
            csv_escape(r.label),
            num(r.value),
            match r.unit {
                Unit::Times => "times",
                Unit::Percent => "percent",
                Unit::Days => "days",
                Unit::Money => "money",
                Unit::Score => "score",
            },
            num(r.prior),
            num(r.change),
            num(r.benchmark_low),
            num(r.benchmark_high),
            r.status.unwrap_or_default(),
            csv_escape(r.formula),
            csv_escape(r.note.as_deref().unwrap_or_default())
        );
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Revenue: 1,200,000
COGS: 720,000
Operating expenses: 300,000
Depreciation and amortization: 40,000
Interest expense: 20,000
Taxes: 40,000
Net income: 120,000
Cash: 90,000
Accounts receivable: 150,000
Inventory: 180,000
Total current assets: 420,000
Fixed assets: 580,000
Accounts payable: 110,000
Short term debt: 60,000
Total current liabilities: 170,000
Long term debt: 330,000
Retained earnings: 200,000
Total equity: 500,000";

    fn summary(figs: &str) -> String {
        run(figs, "", "all", "average", 365, true, 2, "$", "summary").unwrap()
    }

    #[test]
    fn computes_core_ratios_from_a_pasted_statement() {
        let out = summary(SAMPLE);
        // 420,000 / 170,000
        assert!(
            out.contains("Current ratio                          2.47x"),
            "{out}"
        );
        // (90,000 + 150,000) / 170,000
        assert!(
            out.contains("Quick ratio (acid test)                1.41x"),
            "{out}"
        );
        // 500,000 / 500,000 total liabilities
        assert!(
            out.contains("Debt to equity                         1.00x"),
            "{out}"
        );
        // gross profit 480,000 / revenue
        assert!(
            out.contains("Gross margin                          40.00%"),
            "{out}"
        );
        // net income 120,000 / revenue
        assert!(
            out.contains("Net profit margin                     10.00%"),
            "{out}"
        );
        // 120,000 / 1,000,000 total assets
        assert!(
            out.contains("Return on assets (ROA)                12.00%"),
            "{out}"
        );
        // 120,000 / 500,000 equity
        assert!(
            out.contains("Return on equity (ROE)                24.00%"),
            "{out}"
        );
        assert!(out.contains("Derived:"), "{out}");
        assert!(
            out.contains("not financial, investment, tax or accounting advice"),
            "{out}"
        );
    }

    #[test]
    fn derives_missing_subtotals_and_reports_them() {
        let out = summary(SAMPLE);
        // total_assets = 420,000 + 580,000; gross_profit = revenue - cogs;
        // operating_income = gross_profit - opex; total_liabilities from parts.
        assert!(out.contains("total_assets"), "{out}");
        assert!(out.contains("gross_profit"), "{out}");
        assert!(out.contains("operating_income"), "{out}");
    }

    #[test]
    fn parses_messy_amounts_and_aliases() {
        let figs = "Net sales = $1.2m\nCost of sales\t(720,000)\nNet profit: 120k\nAssets: 1,000,000\nLiabilities: 500,000";
        let out = summary(figs);
        assert!(out.contains("Gross margin"), "{out}");
        // 1,200,000 revenue with a negative-parenthesized COGS of -720,000
        // yields a gross profit above revenue, which is still reported.
        assert!(
            out.contains("Net profit margin                     10.00%"),
            "{out}"
        );
        assert!(
            out.contains("Return on assets (ROA)                12.00%"),
            "{out}"
        );
    }

    #[test]
    fn prior_period_adds_a_change_column_and_averages_balances() {
        let prior =
            "Revenue: 1,000,000\nNet income: 80,000\nTotal assets: 800,000\nTotal equity: 400,000";
        let out = run(
            SAMPLE, prior, "returns", "average", 365, true, 2, "$", "summary",
        )
        .unwrap();
        assert!(out.contains("current"), "{out}");
        assert!(out.contains("prior"), "{out}");
        assert!(out.contains("change"), "{out}");
        // avg assets = (1,000,000 + 800,000) / 2 = 900,000 -> 13.33%
        assert!(
            out.contains("Return on assets (ROA)                13.33%"),
            "{out}"
        );
    }

    #[test]
    fn health_score_counts_benchmarked_ratios() {
        let out = summary(SAMPLE);
        assert!(out.contains("Health score:"), "{out}");
        assert!(out.contains("benchmarked ratios in range"), "{out}");
    }

    #[test]
    fn missing_inputs_report_what_is_needed_instead_of_zero() {
        let out = summary("Revenue: 1000\nNet income: 100");
        assert!(out.contains("Current ratio"), "{out}");
        assert!(out.contains("n/a"), "{out}");
        assert!(out.contains("needs current_assets"), "{out}");
    }

    #[test]
    fn csv_and_json_shapes() {
        let csv = run(SAMPLE, "", "liquidity", "ending", 365, true, 2, "$", "csv").unwrap();
        assert!(csv.starts_with("group,key,label,value,unit,"), "{csv}");
        assert!(
            csv.contains("liquidity,current_ratio,Current ratio,"),
            "{csv}"
        );
        let json = run(SAMPLE, "", "all", "ending", 365, true, 2, "$", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["basis"], "ending");
        assert_eq!(v["ratios"][0]["key"], "current_ratio");
        assert!(v["dupont"]["roe_pct"].as_f64().unwrap() > 23.9);
    }

    #[test]
    fn altman_z_uses_the_private_variant_without_share_data() {
        let json = run(SAMPLE, "", "leverage", "ending", 365, true, 2, "$", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let z = v["ratios"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["key"] == "altman_z_score")
            .unwrap()
            .clone();
        assert!(z["note"].as_str().unwrap().contains("private-company"));
        assert!(z["value"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn market_ratios_appear_only_with_share_data() {
        let plain = summary(SAMPLE);
        assert!(!plain.contains("Earnings per share"), "{plain}");
        let with_shares = summary(&format!(
            "{SAMPLE}\nShares outstanding: 100,000\nShare price: 24"
        ));
        assert!(with_shares.contains("Earnings per share"), "{with_shares}");
        // EPS 120,000 / 100,000 = 1.20; P/E = 24 / 1.20 = 20
        assert!(
            with_shares.contains("Price / earnings                      20.00x"),
            "{with_shares}"
        );
    }

    #[test]
    fn warns_when_the_balance_sheet_does_not_balance() {
        let out = summary("Revenue: 1000\nNet income: 100\nTotal assets: 1000\nTotal liabilities: 400\nTotal equity: 500");
        assert!(out.contains("do not equal liabilities"), "{out}");
    }

    #[test]
    fn reports_unrecognized_and_unreadable_lines() {
        let out = summary("Balance sheet\nGoodwill: 5000\nRevenue: 1000\nNet income: 100");
        assert!(out.contains("unrecognized label"), "{out}");
        assert!(out.contains("Goodwill"), "{out}");
        assert!(out.contains("no readable amount"), "{out}");
        assert!(out.contains("Balance sheet"), "{out}");
    }

    #[test]
    fn rejects_empty_figures() {
        let err = run("   ", "", "all", "average", 365, true, 2, "$", "summary").unwrap_err();
        assert!(err.contains("figures is required"), "{err}");
    }

    #[test]
    fn rejects_a_statement_with_no_recognized_items() {
        let err = run(
            "Widgets sold: 42\nOffices: 3",
            "",
            "all",
            "average",
            365,
            true,
            2,
            "$",
            "summary",
        )
        .unwrap_err();
        assert!(err.contains("no recognized line items"), "{err}");
    }

    #[test]
    fn rejects_bad_choices_and_out_of_range_numbers() {
        let err = run(
            SAMPLE, "", "profit", "average", 365, true, 2, "$", "summary",
        )
        .unwrap_err();
        assert!(err.contains("groups must be one of"), "{err}");
        let err = run(SAMPLE, "", "all", "mean", 365, true, 2, "$", "summary").unwrap_err();
        assert!(err.contains("basis must be one of"), "{err}");
        let err = run(SAMPLE, "", "all", "average", 365, true, 2, "$", "chart").unwrap_err();
        assert!(err.contains("output must be one of"), "{err}");
        let err = run(SAMPLE, "", "all", "average", 400, true, 2, "$", "summary").unwrap_err();
        assert!(
            err.contains("days_in_period must be between 1 and 366"),
            "{err}"
        );
        let err = run(SAMPLE, "", "all", "average", 365, true, 9, "$", "summary").unwrap_err();
        assert!(err.contains("decimals must be between 0 and 6"), "{err}");
    }

    #[test]
    fn rejects_too_many_lines() {
        let figs = format!("Revenue: 1000\n{}", "Note line 1\n".repeat(MAX_LINES));
        let err = run(&figs, "", "all", "average", 365, true, 2, "$", "summary").unwrap_err();
        assert!(
            err.contains(&format!("the maximum is {MAX_LINES}")),
            "{err}"
        );
    }

    #[test]
    fn line_cap_boundary_is_accepted() {
        let mut figs = String::from("Revenue: 1000\nNet income: 100\n");
        for _ in 0..(MAX_LINES - 2) {
            figs.push_str("Note line\n");
        }
        assert!(run(&figs, "", "all", "average", 365, true, 2, "$", "summary").is_ok());
    }

    #[test]
    fn benchmarks_off_hides_targets_and_score() {
        let out = run(
            SAMPLE,
            "",
            "liquidity",
            "ending",
            365,
            false,
            2,
            "$",
            "summary",
        )
        .unwrap();
        assert!(!out.contains("Health score"), "{out}");
        assert!(!out.contains("target"), "{out}");
    }

    #[test]
    fn days_in_period_drives_the_day_count_ratios() {
        let a = run(
            SAMPLE,
            "",
            "efficiency",
            "ending",
            365,
            true,
            2,
            "$",
            "summary",
        )
        .unwrap();
        let b = run(
            SAMPLE,
            "",
            "efficiency",
            "ending",
            360,
            true,
            2,
            "$",
            "summary",
        )
        .unwrap();
        assert!(a.contains("Days sales outstanding"), "{a}");
        assert_ne!(a, b);
    }

    #[test]
    fn amount_parser_handles_the_documented_forms() {
        assert_eq!(parse_amount("1,250,000"), Some(1_250_000.0));
        assert_eq!(parse_amount("$1.2m"), Some(1_200_000.0));
        assert_eq!(parse_amount("(4,500)"), Some(-4_500.0));
        assert_eq!(parse_amount("-3000"), Some(-3000.0));
        assert_eq!(parse_amount("340k"), Some(340_000.0));
        assert_eq!(parse_amount("2bn"), Some(2e9));
        assert_eq!(parse_amount("40%"), None);
        assert_eq!(parse_amount("n/a"), None);
    }

    #[test]
    fn line_splitter_finds_the_longest_numeric_suffix() {
        assert_eq!(
            split_line("Total assets: 1,250,000"),
            Some(("Total assets", 1_250_000.0))
        );
        assert_eq!(split_line("Revenue,500000"), Some(("Revenue", 500_000.0)));
        assert_eq!(
            split_line("FY2024 net income  90,000"),
            Some(("FY2024 net income", 90_000.0))
        );
        assert_eq!(split_line("Balance sheet"), None);
    }
}
