//! spending-categorizer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Takes a bank / credit-card CSV
//! export, auto-categorizes each row by merchant keywords (user rules first,
//! then a built-in ordered keyword table, then a sign-based fallback), and
//! summarizes spending by category: totals, share of spending, transaction
//! counts and a proportional text bar — plus a categorized CSV for export.
//!
//! Keyword matching: single-word built-in keywords match on TOKEN PREFIX (so
//! `rent` hits RENT/RENTAL but not PARENT/CURRENT; `fee` not COFFEE); built-in
//! keywords containing spaces/punctuation and all user rules substring-match
//! against the lowercased description. The table is ordered specific→generic.

use csv::ReaderBuilder;

/// Hard cap on rows categorized in one call, so a huge paste can't blow up
/// memory. The chat/CLI schema and the page advertise this same bound.
pub const MAX_ROWS: usize = 10_000;

/// Length (in blocks) of the longest summary bar.
const BAR_WIDTH: usize = 20;

/// Category used for money-out rows that match no rule or keyword.
const FALLBACK_EXPENSE: &str = "Other";
/// Category used for money-in rows.
const INCOME: &str = "Income";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    Both,
    Summary,
    Csv,
}

impl Output {
    fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" => Ok(Output::Both),
            "summary" => Ok(Output::Summary),
            "csv" => Ok(Output::Csv),
            other => Err(format!("unknown output '{other}' (use both, summary, or csv)")),
        }
    }
}

/// Collapse a header to lowercase alphanumerics only, so column matching is
/// independent of spacing/punctuation: "Transaction Date" → "transactiondate".
fn collapse(header: &str) -> String {
    header
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Sniff the most likely CSV delimiter from the first non-empty line.
fn sniff_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut best = b',';
    let mut best_count = 0usize;
    for (byte, ch) in [(b',', ','), (b';', ';'), (b'\t', '\t'), (b'|', '|')] {
        let count = line.matches(ch).count();
        if count > best_count {
            best_count = count;
            best = byte;
        }
    }
    best
}

fn delimiter_byte(delimiter: &str, data: &str) -> Result<u8, String> {
    Ok(match delimiter.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => sniff_delimiter(data),
        "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let bytes = other.as_bytes();
            if bytes.len() == 1 {
                bytes[0]
            } else {
                return Err(format!(
                    "delimiter must be auto/comma/semicolon/tab/pipe (or a single char), got '{other}'"
                ));
            }
        }
    })
}

/// Parse CSV text into (headers, rows). Fully blank rows are skipped.
fn parse_csv(data: &str, delimiter: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let delim = delimiter_byte(delimiter, data)?;
    let mut rdr = ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(data.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| format!("could not read the CSV header row: {e}"))?
        .iter()
        .map(|s| s.to_string())
        .collect();
    if headers.is_empty() {
        return Err("the CSV has no header row (the first row must be the column names)".into());
    }
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("could not parse a CSV row: {e}"))?;
        let row: Vec<String> = rec.iter().map(|s| s.to_string()).collect();
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        rows.push(row);
    }
    Ok((headers, rows))
}

/// Find the column index for `wanted`. If `explicit` is given, match it exactly
/// (case-insensitively, punctuation-independent) and error if it's absent. Else
/// return the first header whose collapsed form contains any of `auto_keys`.
fn find_column(
    headers: &[String],
    explicit: &str,
    auto_keys: &[&str],
    wanted: &str,
) -> Result<Option<usize>, String> {
    let collapsed: Vec<String> = headers.iter().map(|h| collapse(h)).collect();
    if !explicit.trim().is_empty() {
        let want = collapse(explicit);
        return collapsed
            .iter()
            .position(|c| *c == want)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "{wanted} column '{}' not found — available columns: {}",
                    explicit.trim(),
                    headers.join(", ")
                )
            });
    }
    for key in auto_keys {
        if let Some(pos) = collapsed.iter().position(|c| c.contains(key)) {
            return Ok(Some(pos));
        }
    }
    Ok(None)
}

/// Parse a money string into a signed f64. Strips currency symbols/spaces,
/// treats `(1,234.56)` and a trailing `DR` as negative (`CR` positive), and
/// handles both US (`1,234.56`) and EU (`1.234,56`) separators.
fn parse_amount(raw: &str) -> Result<Option<f64>, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let mut neg = false;
    let mut body = s.to_string();
    if body.starts_with('(') && body.ends_with(')') {
        neg = true;
        body = body[1..body.len() - 1].to_string();
    }
    let upper = body.to_ascii_uppercase();
    if upper.trim_end().ends_with("DR") {
        neg = true;
        let cut = body.trim_end().len() - 2;
        body = body[..cut].to_string();
    } else if upper.trim_end().ends_with("CR") {
        let cut = body.trim_end().len() - 2;
        body = body[..cut].to_string();
    }
    let mut cleaned: String = body
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-' || *c == '+')
        .collect();
    if cleaned.contains('-') {
        neg = !neg;
    }
    cleaned = cleaned.replace(['-', '+'], "");
    if cleaned.is_empty() {
        return Err(format!("could not read an amount from '{raw}'"));
    }
    let has_dot = cleaned.contains('.');
    let has_comma = cleaned.contains(',');
    let normalized = if has_dot && has_comma {
        if cleaned.rfind('.') > cleaned.rfind(',') {
            cleaned.replace(',', "")
        } else {
            cleaned.replace('.', "").replace(',', ".")
        }
    } else if has_comma {
        let parts: Vec<&str> = cleaned.split(',').collect();
        if parts.len() == 2 && parts[1].len() <= 2 {
            cleaned.replace(',', ".")
        } else {
            cleaned.replace(',', "")
        }
    } else {
        cleaned
    };
    let value: f64 = normalized
        .parse()
        .map_err(|_| format!("could not read an amount from '{raw}'"))?;
    Ok(Some(if neg { -value } else { value }))
}

/// Built-in merchant-keyword table, ordered specific→generic; first match wins.
/// Single-word keywords are TOKEN-PREFIX matched; keywords with a space or
/// punctuation are substring matched (see `keyword_matches`).
const CATEGORY_RULES: &[(&str, &str)] = &[
    // Dining & Takeaway (delivery brands before "uber" → Transport)
    ("uber eats", "Dining & Takeaway"),
    ("ubereats", "Dining & Takeaway"),
    ("eats", "Dining & Takeaway"),
    ("doordash", "Dining & Takeaway"),
    ("grubhub", "Dining & Takeaway"),
    ("deliveroo", "Dining & Takeaway"),
    ("just eat", "Dining & Takeaway"),
    ("starbucks", "Dining & Takeaway"),
    ("mcdonald", "Dining & Takeaway"),
    ("burger", "Dining & Takeaway"),
    ("pizza", "Dining & Takeaway"),
    ("chipotle", "Dining & Takeaway"),
    ("dunkin", "Dining & Takeaway"),
    ("kfc", "Dining & Takeaway"),
    ("taco", "Dining & Takeaway"),
    ("sushi", "Dining & Takeaway"),
    ("coffee", "Dining & Takeaway"),
    ("cafe", "Dining & Takeaway"),
    ("restaurant", "Dining & Takeaway"),
    ("bakery", "Dining & Takeaway"),
    ("bistro", "Dining & Takeaway"),
    ("takeaway", "Dining & Takeaway"),
    ("subway", "Dining & Takeaway"),
    // Groceries
    ("grocer", "Groceries"),
    ("supermarket", "Groceries"),
    ("supermarkt", "Groceries"),
    ("walmart", "Groceries"),
    ("kroger", "Groceries"),
    ("safeway", "Groceries"),
    ("aldi", "Groceries"),
    ("lidl", "Groceries"),
    ("tesco", "Groceries"),
    ("sainsbury", "Groceries"),
    ("asda", "Groceries"),
    ("waitrose", "Groceries"),
    ("trader joe", "Groceries"),
    ("whole foods", "Groceries"),
    ("costco", "Groceries"),
    ("wegmans", "Groceries"),
    ("publix", "Groceries"),
    ("rewe", "Groceries"),
    ("edeka", "Groceries"),
    ("carrefour", "Groceries"),
    // Fuel
    ("gas station", "Fuel"),
    ("petrol", "Fuel"),
    ("fuel", "Fuel"),
    ("shell", "Fuel"),
    ("chevron", "Fuel"),
    ("exxon", "Fuel"),
    ("texaco", "Fuel"),
    ("esso", "Fuel"),
    // Travel (rental-car brands before "rent" → Housing & Rent)
    ("car rental", "Travel"),
    ("rental car", "Travel"),
    ("hertz", "Travel"),
    ("avis", "Travel"),
    ("airline", "Travel"),
    ("airways", "Travel"),
    ("hotel", "Travel"),
    ("airbnb", "Travel"),
    ("booking.com", "Travel"),
    ("expedia", "Travel"),
    ("hostel", "Travel"),
    ("flight", "Travel"),
    ("ryanair", "Travel"),
    ("easyjet", "Travel"),
    ("delta air", "Travel"),
    ("united air", "Travel"),
    ("lufthansa", "Travel"),
    ("marriott", "Travel"),
    ("hilton", "Travel"),
    // Transport
    ("uber", "Transport"),
    ("lyft", "Transport"),
    ("taxi", "Transport"),
    ("transit", "Transport"),
    ("metro", "Transport"),
    ("amtrak", "Transport"),
    ("parking", "Transport"),
    ("toll", "Transport"),
    ("railway", "Transport"),
    // Subscriptions & Streaming (before Shopping so "amazon prime" wins)
    ("netflix", "Subscriptions & Streaming"),
    ("spotify", "Subscriptions & Streaming"),
    ("hulu", "Subscriptions & Streaming"),
    ("disney", "Subscriptions & Streaming"),
    ("hbo", "Subscriptions & Streaming"),
    ("audible", "Subscriptions & Streaming"),
    ("youtube premium", "Subscriptions & Streaming"),
    ("apple.com/bill", "Subscriptions & Streaming"),
    ("itunes", "Subscriptions & Streaming"),
    ("patreon", "Subscriptions & Streaming"),
    ("prime video", "Subscriptions & Streaming"),
    ("amazon prime", "Subscriptions & Streaming"),
    ("subscription", "Subscriptions & Streaming"),
    ("membership", "Subscriptions & Streaming"),
    ("adobe", "Subscriptions & Streaming"),
    ("dropbox", "Subscriptions & Streaming"),
    ("icloud", "Subscriptions & Streaming"),
    ("google one", "Subscriptions & Streaming"),
    ("github", "Subscriptions & Streaming"),
    // Shopping
    ("amazon", "Shopping"),
    ("amzn", "Shopping"),
    ("ebay", "Shopping"),
    ("etsy", "Shopping"),
    ("target", "Shopping"),
    ("best buy", "Shopping"),
    ("ikea", "Shopping"),
    ("zara", "Shopping"),
    ("h&m", "Shopping"),
    ("nike", "Shopping"),
    ("apple store", "Shopping"),
    ("clothing", "Shopping"),
    ("paypal", "Shopping"),
    // Utilities & Phone
    ("electric", "Utilities & Phone"),
    ("energy", "Utilities & Phone"),
    ("water bill", "Utilities & Phone"),
    ("gas bill", "Utilities & Phone"),
    ("utilit", "Utilities & Phone"),
    ("comcast", "Utilities & Phone"),
    ("xfinity", "Utilities & Phone"),
    ("verizon", "Utilities & Phone"),
    ("at&t", "Utilities & Phone"),
    ("t-mobile", "Utilities & Phone"),
    ("vodafone", "Utilities & Phone"),
    ("internet", "Utilities & Phone"),
    ("broadband", "Utilities & Phone"),
    ("phone", "Utilities & Phone"),
    // Housing & Rent
    ("rent", "Housing & Rent"),
    ("mortgage", "Housing & Rent"),
    ("landlord", "Housing & Rent"),
    ("hoa", "Housing & Rent"),
    // Insurance
    ("insurance", "Insurance"),
    ("geico", "Insurance"),
    ("allstate", "Insurance"),
    ("state farm", "Insurance"),
    ("aetna", "Insurance"),
    // Health & Fitness
    ("pharmacy", "Health & Fitness"),
    ("cvs", "Health & Fitness"),
    ("walgreens", "Health & Fitness"),
    ("doctor", "Health & Fitness"),
    ("dental", "Health & Fitness"),
    ("dentist", "Health & Fitness"),
    ("clinic", "Health & Fitness"),
    ("hospital", "Health & Fitness"),
    ("medical", "Health & Fitness"),
    ("gym", "Health & Fitness"),
    ("fitness", "Health & Fitness"),
    // Entertainment
    ("cinema", "Entertainment"),
    ("movie", "Entertainment"),
    ("theater", "Entertainment"),
    ("theatre", "Entertainment"),
    ("steam", "Entertainment"),
    ("playstation", "Entertainment"),
    ("xbox", "Entertainment"),
    ("nintendo", "Entertainment"),
    ("ticketmaster", "Entertainment"),
    ("concert", "Entertainment"),
    // Cash & ATM
    ("atm", "Cash & ATM"),
    ("cash withdrawal", "Cash & ATM"),
    ("cashpoint", "Cash & ATM"),
    // Transfers
    ("wire transfer", "Transfers"),
    ("transfer", "Transfers"),
    ("zelle", "Transfers"),
    ("venmo", "Transfers"),
    ("revolut", "Transfers"),
    ("cash app", "Transfers"),
    // Fees & Interest (generic words last)
    ("overdraft", "Fees & Interest"),
    ("service charge", "Fees & Interest"),
    ("bank charge", "Fees & Interest"),
    ("fee", "Fees & Interest"),
    ("interest", "Fees & Interest"),
];

/// Keywords that mark a money-IN row as income (checked before the merchant
/// table for positive amounts, so "INTEREST PAID" is income, not fees).
const INCOME_KEYWORDS: &[&str] = &[
    "salary",
    "payroll",
    "paycheck",
    "wages",
    "direct deposit",
    "dividend",
    "interest",
    "refund",
    "reimbursement",
    "rebate",
    "cashback",
    "pension",
    "bonus",
];

/// Match one built-in keyword against a description: single-word alphanumeric
/// keywords match if any description token starts with them; anything with a
/// space/punctuation is a plain substring match.
fn keyword_matches(desc_lower: &str, tokens: &[&str], keyword: &str) -> bool {
    if keyword.chars().all(|c| c.is_ascii_alphanumeric()) {
        tokens.iter().any(|t| t.starts_with(keyword))
    } else {
        desc_lower.contains(keyword)
    }
}

/// One user rule: a lowercase substring pattern → category name.
struct UserRule {
    pattern: String,
    category: String,
}

/// Parse the `rules` text: one `keyword = Category` per line (`=>` and `->`
/// also accepted as the separator). Blank lines and `#` comments are ignored.
fn parse_rules(text: &str) -> Result<Vec<UserRule>, String> {
    let mut rules = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let sep = l
            .find("=>")
            .map(|p| (p, 2))
            .or_else(|| l.find("->").map(|p| (p, 2)))
            .or_else(|| l.find('=').map(|p| (p, 1)));
        let (pos, len) = sep.ok_or_else(|| {
            format!(
                "rule on line {} must be 'keyword = Category' — got '{l}'",
                i + 1
            )
        })?;
        let pattern = l[..pos].trim().to_lowercase();
        let category = l[pos + len..].trim().to_string();
        if pattern.is_empty() || category.is_empty() {
            return Err(format!(
                "rule on line {} must have both a keyword and a category",
                i + 1
            ));
        }
        rules.push(UserRule { pattern, category });
    }
    Ok(rules)
}

/// Choose the category for one transaction from its description and sign.
fn categorize(description: &str, signed: f64, rules: &[UserRule]) -> String {
    let desc = description.to_lowercase();
    let tokens: Vec<&str> = desc
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    for r in rules {
        if desc.contains(&r.pattern) {
            return r.category.clone();
        }
    }
    if signed >= 0.0 {
        // Money in: income keywords first, then merchant keywords (a merchant
        // refund lands back in its spending category), else Income.
        for kw in INCOME_KEYWORDS {
            if keyword_matches(&desc, &tokens, kw) {
                return INCOME.to_string();
            }
        }
        for (kw, cat) in CATEGORY_RULES {
            if keyword_matches(&desc, &tokens, kw) {
                return (*cat).to_string();
            }
        }
        INCOME.to_string()
    } else {
        for (kw, cat) in CATEGORY_RULES {
            if keyword_matches(&desc, &tokens, kw) {
                return (*cat).to_string();
            }
        }
        FALLBACK_EXPENSE.to_string()
    }
}

/// Format an amount with its commodity. A symbol currency (`$`, `£`, `€`) is
/// prefixed (`$4.50`); an alphabetic code (`USD`) is suffixed (`4.50 USD`).
fn format_amount(value: f64, currency: &str) -> String {
    let num = format!("{:.2}", value);
    let cur = currency.trim();
    if cur.is_empty() {
        num
    } else if cur.chars().all(|c| c.is_ascii_alphabetic()) {
        format!("{num} {cur}")
    } else {
        format!("{cur}{num}")
    }
}

struct Txn {
    date: String,
    description: String,
    signed: f64,
    category: String,
}

/// Render the spending summary: categories sorted by spend (desc, ties
/// alphabetical), share of total spending, txn counts, a proportional bar,
/// then Total spending / Income / Net cash flow.
fn build_summary(txns: &[Txn], currency: &str) -> String {
    let mut cats: Vec<(String, f64, usize)> = Vec::new();
    let mut income_total = 0.0f64;
    let mut income_count = 0usize;
    for t in txns {
        if t.category == INCOME {
            income_total += t.signed;
            income_count += 1;
        } else {
            let spend = -t.signed;
            match cats.iter_mut().find(|(name, _, _)| *name == t.category) {
                Some((_, total, count)) => {
                    *total += spend;
                    *count += 1;
                }
                None => cats.push((t.category.clone(), spend, 1)),
            }
        }
    }
    cats.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.cmp(&b.0)));
    let total_spend: f64 = cats.iter().map(|(_, v, _)| v).sum();
    let total_count: usize = cats.iter().map(|(_, _, c)| c).sum();
    let max_spend = cats.iter().map(|(_, v, _)| *v).fold(0.0f64, f64::max);
    let net = income_total - total_spend;

    let total_amt = format_amount(total_spend, currency);
    let income_amt = format_amount(income_total, currency);
    let net_amt = if net >= 0.0 {
        format!("+{}", format_amount(net, currency))
    } else {
        format!("-{}", format_amount(-net, currency))
    };

    // Widths in CHARS (not bytes) so multi-byte symbols like € align, matching
    // how format! width padding counts.
    let w = |s: &str| s.chars().count();
    let mut name_w = w("Total spending").max(w("Category"));
    for (name, _, _) in &cats {
        name_w = name_w.max(w(name));
    }
    if income_count > 0 {
        name_w = name_w.max(w("Net cash flow"));
    }
    let mut amt_w = w("Total").max(w(&total_amt));
    for (_, v, _) in &cats {
        amt_w = amt_w.max(w(&format_amount(*v, currency)));
    }
    if income_count > 0 {
        amt_w = amt_w.max(w(&income_amt)).max(w(&net_amt));
    }

    let mut out = String::new();
    out.push_str("Spending by category\n====================\n\n");
    out.push_str(&format!(
        "{:<name_w$}  {:>amt_w$}  {:>6}  {:>4}\n",
        "Category", "Total", "Share", "Txns"
    ));
    let rule_w = name_w + 2 + amt_w + 2 + 6 + 2 + 4;
    out.push_str(&"-".repeat(rule_w));
    out.push('\n');
    for (name, spend, count) in &cats {
        let share = if total_spend > 0.0 {
            format!("{:.1}%", spend / total_spend * 100.0)
        } else {
            String::new()
        };
        let bar_units = if *spend > 0.0 && max_spend > 0.0 {
            ((spend / max_spend * BAR_WIDTH as f64).round() as usize).max(1)
        } else {
            0
        };
        let mut line = format!(
            "{:<name_w$}  {:>amt_w$}  {:>6}  {:>4}",
            name,
            format_amount(*spend, currency),
            share,
            count
        );
        if bar_units > 0 {
            line.push_str("  ");
            line.push_str(&"\u{2588}".repeat(bar_units));
        }
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str(&"-".repeat(rule_w));
    out.push('\n');
    let total_share = if total_spend > 0.0 { "100.0%" } else { "" };
    out.push_str(&format!(
        "{:<name_w$}  {:>amt_w$}  {:>6}  {:>4}\n",
        "Total spending", total_amt, total_share, total_count
    ));
    if income_count > 0 {
        out.push_str(&format!(
            "{:<name_w$}  {:>amt_w$}  {:>6}  {:>4}\n",
            "Income", income_amt, "", income_count
        ));
        out.push_str(&format!(
            "{:<name_w$}  {:>amt_w$}\n",
            "Net cash flow", net_amt
        ));
    }
    out.trim_end().to_string()
}

/// Render the categorized rows as CSV (RFC-4180 quoting via the csv crate).
fn build_csv(txns: &[Txn], has_date: bool) -> Result<String, String> {
    let mut wtr = csv::Writer::from_writer(Vec::new());
    let write_err = |e: csv::Error| format!("could not write the categorized CSV: {e}");
    if has_date {
        wtr.write_record(["Date", "Description", "Amount", "Category"])
            .map_err(write_err)?;
    } else {
        wtr.write_record(["Description", "Amount", "Category"])
            .map_err(write_err)?;
    }
    for t in txns {
        let amount = format!("{:.2}", t.signed);
        if has_date {
            wtr.write_record([&t.date, &t.description, &amount, &t.category])
                .map_err(write_err)?;
        } else {
            wtr.write_record([&t.description, &amount, &t.category])
                .map_err(write_err)?;
        }
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("could not write the categorized CSV: {e}"))?;
    Ok(String::from_utf8_lossy(&bytes).trim_end().to_string())
}

/// Categorize a bank/credit-card CSV export and summarize spending by category.
///
/// Column parameters are header names (blank = auto-detect); `rules` is one
/// `keyword = Category` per line; `output` is both/summary/csv; `currency` a
/// symbol (prefix) or code (suffix); `delimiter` auto/comma/semicolon/tab/pipe;
/// `invert_amount` flips signs for spending-as-positive exports.
#[allow(clippy::too_many_arguments)]
pub fn categorize_spending(
    data: &str,
    description_column: &str,
    amount_column: &str,
    debit_column: &str,
    credit_column: &str,
    date_column: &str,
    rules: &str,
    output: &str,
    currency: &str,
    delimiter: &str,
    invert_amount: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("no input — paste a bank or credit-card CSV export with a header row".into());
    }
    let out_mode = Output::parse(output)?;
    let user_rules = parse_rules(rules)?;

    let (headers, rows) = parse_csv(data, delimiter)?;
    if rows.is_empty() {
        return Err("no data rows found (the CSV has a header row but no transactions)".into());
    }
    if rows.len() > MAX_ROWS {
        return Err(format!(
            "too many rows: {} (max {MAX_ROWS} per run)",
            rows.len()
        ));
    }

    let desc_idx = find_column(
        &headers,
        description_column,
        &[
            "description",
            "payee",
            "narration",
            "details",
            "memo",
            "merchant",
            "name",
            "reference",
            "particulars",
            // Common European export headers.
            "beschreibung",
            "verwendungszweck",
            "omschrijving",
            "libell",
            "concepto",
        ],
        "description",
    )?
    .ok_or_else(|| {
        format!(
            "could not find a description column — name one with description_column. Available: {}",
            headers.join(", ")
        )
    })?;
    let amount_idx = find_column(
        &headers,
        amount_column,
        &["amount", "value", "betrag", "bedrag", "montant", "importe"],
        "amount",
    )?;
    let debit_idx = find_column(
        &headers,
        debit_column,
        &["debit", "withdrawal", "paidout", "outflow", "moneyout"],
        "debit",
    )?;
    let credit_idx = find_column(
        &headers,
        credit_column,
        &["credit", "deposit", "paidin", "inflow", "moneyin"],
        "credit",
    )?;
    if amount_idx.is_none() && debit_idx.is_none() && credit_idx.is_none() {
        return Err(format!(
            "could not find an amount column — name one with amount_column (or debit_column/credit_column). Available: {}",
            headers.join(", ")
        ));
    }
    let date_idx = find_column(
        &headers,
        date_column,
        &["date", "posted", "posting", "datum", "fecha"],
        "date",
    )?;

    let cell = |row: &[String], idx: usize| -> String {
        row.get(idx)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };

    // debit − credit as a signed amount: money out negative, money in positive.
    let from_debit_credit = |row: &[String], rownum: usize| -> Result<Option<f64>, String> {
        let d = match debit_idx {
            Some(di) => parse_amount(&cell(row, di)).map_err(|e| format!("row {rownum}: {e}"))?,
            None => None,
        };
        let c = match credit_idx {
            Some(ci) => parse_amount(&cell(row, ci)).map_err(|e| format!("row {rownum}: {e}"))?,
            None => None,
        };
        Ok(match (d, c) {
            (None, None) => None,
            _ => Some(c.unwrap_or(0.0) - d.unwrap_or(0.0).abs()),
        })
    };

    let mut txns: Vec<Txn> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let rownum = i + 2; // 1-based, counting the header row
        let description = cell(row, desc_idx);
        let mut signed = if let Some(ai) = amount_idx {
            match parse_amount(&cell(row, ai)).map_err(|e| format!("row {rownum}: {e}"))? {
                Some(v) => v,
                None => from_debit_credit(row, rownum)?
                    .ok_or_else(|| format!("row {rownum} has no amount"))?,
            }
        } else {
            from_debit_credit(row, rownum)?.ok_or_else(|| {
                format!("row {rownum} has no amount (both debit and credit are empty)")
            })?
        };
        if invert_amount {
            signed = -signed;
        }
        let category = categorize(&description, signed, &user_rules);
        let date = date_idx.map(|di| cell(row, di)).unwrap_or_default();
        txns.push(Txn {
            date,
            description,
            signed,
            category,
        });
    }

    match out_mode {
        Output::Summary => Ok(build_summary(&txns, currency)),
        Output::Csv => build_csv(&txns, date_idx.is_some()),
        Output::Both => {
            let summary = build_summary(&txns, currency);
            let csv_text = build_csv(&txns, date_idx.is_some())?;
            Ok(format!(
                "{summary}\n\nCategorized transactions\n========================\n{csv_text}"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "Date,Description,Amount\n\
2024-01-05,WALMART SUPERCENTER,-52.30\n\
2024-01-06,STARBUCKS #1234,-4.50\n\
2024-01-07,SHELL GAS STATION,-48.90\n\
2024-01-08,NETFLIX.COM,-15.99\n\
2024-01-09,WALMART SUPERCENTER,-34.80\n\
2024-01-10,ACME PAYROLL,2000.00\n\
2024-01-11,CITY PARKING,-12.00\n";

    #[test]
    fn summary_categorizes_and_totals_exactly() {
        let out =
            categorize_spending(FIXTURE, "", "", "", "", "", "", "summary", "$", "auto", false)
                .unwrap();
        let expected = "\
Spending by category
====================

Category                       Total   Share  Txns
--------------------------------------------------
Groceries                     $87.10   51.7%     2  ████████████████████
Fuel                          $48.90   29.0%     1  ███████████
Subscriptions & Streaming     $15.99    9.5%     1  ████
Transport                     $12.00    7.1%     1  ███
Dining & Takeaway              $4.50    2.7%     1  █
--------------------------------------------------
Total spending               $168.49  100.0%     6
Income                      $2000.00             1
Net cash flow              +$1831.51";
        assert_eq!(out, expected);
    }

    #[test]
    fn csv_output_lists_each_row_with_its_category() {
        let out =
            categorize_spending(FIXTURE, "", "", "", "", "", "", "csv", "$", "auto", false)
                .unwrap();
        let expected = "\
Date,Description,Amount,Category
2024-01-05,WALMART SUPERCENTER,-52.30,Groceries
2024-01-06,STARBUCKS #1234,-4.50,Dining & Takeaway
2024-01-07,SHELL GAS STATION,-48.90,Fuel
2024-01-08,NETFLIX.COM,-15.99,Subscriptions & Streaming
2024-01-09,WALMART SUPERCENTER,-34.80,Groceries
2024-01-10,ACME PAYROLL,2000.00,Income
2024-01-11,CITY PARKING,-12.00,Transport";
        assert_eq!(out, expected);
    }

    #[test]
    fn both_output_is_summary_then_rows() {
        let out =
            categorize_spending(FIXTURE, "", "", "", "", "", "", "both", "$", "auto", false)
                .unwrap();
        assert!(out.starts_with("Spending by category\n"));
        assert!(out.contains(
            "\n\nCategorized transactions\n========================\nDate,Description,Amount,Category\n"
        ));
        assert!(out.ends_with("2024-01-11,CITY PARKING,-12.00,Transport"));
    }

    #[test]
    fn user_rules_override_builtins_and_support_comments() {
        let out = categorize_spending(
            "Date,Description,Amount\n2024-01-06,STARBUCKS #1234,-4.50\n",
            "",
            "",
            "",
            "",
            "",
            "# my rules\nstarbucks = Coffee Habit\n",
            "csv",
            "$",
            "auto",
            false,
        )
        .unwrap();
        assert!(out.contains("STARBUCKS #1234,-4.50,Coffee Habit"), "got: {out}");
    }

    #[test]
    fn debit_credit_columns_and_invert_work() {
        let out = categorize_spending(
            "Date,Details,Debit,Credit\n01/05/2024,TESCO STORES,23.10,\n01/06/2024,ACME SALARY,,1500.00\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "csv",
            "$",
            "auto",
            false,
        )
        .unwrap();
        assert!(out.contains("TESCO STORES,-23.10,Groceries"), "got: {out}");
        assert!(out.contains("ACME SALARY,1500.00,Income"), "got: {out}");
        // Spending-as-positive export: invert flips signs.
        let out = categorize_spending(
            "Date,Description,Amount\n2024-01-05,ALDI,52.30\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "csv",
            "$",
            "auto",
            true,
        )
        .unwrap();
        assert!(out.contains("ALDI,-52.30,Groceries"), "got: {out}");
    }

    #[test]
    fn token_prefix_matching_avoids_false_positives() {
        let csv = "Date,Description,Amount\n\
2024-01-05,PARENT SCHOOL FUND,-10.00\n\
2024-01-06,RENT JANUARY,-900.00\n\
2024-01-07,MONTHLY FEE,-5.00\n\
2024-01-08,PINTEREST ADS,-20.00\n\
2024-01-09,INTEREST PAID,1.23\n\
2024-01-10,UBER EATS ORDER,-18.40\n\
2024-01-11,UBER TRIP,-11.20\n";
        let out = categorize_spending(csv, "", "", "", "", "", "", "csv", "$", "auto", false)
            .unwrap();
        assert!(out.contains("PARENT SCHOOL FUND,-10.00,Other"), "got: {out}");
        assert!(out.contains("RENT JANUARY,-900.00,Housing & Rent"), "got: {out}");
        assert!(out.contains("MONTHLY FEE,-5.00,Fees & Interest"), "got: {out}");
        assert!(out.contains("PINTEREST ADS,-20.00,Other"), "got: {out}");
        assert!(out.contains("INTEREST PAID,1.23,Income"), "got: {out}");
        assert!(out.contains("UBER EATS ORDER,-18.40,Dining & Takeaway"), "got: {out}");
        assert!(out.contains("UBER TRIP,-11.20,Transport"), "got: {out}");
    }

    #[test]
    fn semicolon_delimiter_eu_amounts_and_explicit_columns() {
        let out = categorize_spending(
            "Datum;Beschreibung;Betrag\n15.01.2024;SUPERMARKT KAUFLAND;-42,90\n",
            "beschreibung",
            "betrag",
            "",
            "",
            "datum",
            "",
            "csv",
            "€",
            "semicolon",
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "Date,Description,Amount,Category\n15.01.2024,SUPERMARKT KAUFLAND,-42.90,Groceries"
        );
        // The same German headers must also auto-detect (blank column params).
        let out = categorize_spending(
            "Datum;Beschreibung;Betrag\n15.01.2024;SUPERMARKT KAUFLAND;-42,90\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "csv",
            "€",
            "auto",
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "Date,Description,Amount,Category\n15.01.2024,SUPERMARKT KAUFLAND,-42.90,Groceries"
        );
    }

    #[test]
    fn row_cap_is_enforced_at_the_boundary() {
        let mut at_cap = String::from("Description,Amount\n");
        for i in 0..MAX_ROWS {
            at_cap.push_str(&format!("Merchant {i},-1.00\n"));
        }
        let ok = categorize_spending(&at_cap, "", "", "", "", "", "", "csv", "$", "auto", false);
        assert!(ok.is_ok(), "exactly MAX_ROWS rows must be accepted");
        let mut over = at_cap;
        over.push_str("One More,-1.00\n");
        let err = categorize_spending(&over, "", "", "", "", "", "", "csv", "$", "auto", false)
            .unwrap_err();
        assert!(err.contains("too many rows: 10001"), "got: {err}");
    }

    #[test]
    fn missing_columns_and_bad_inputs_error_clearly() {
        let err = categorize_spending("", "", "", "", "", "", "", "both", "$", "auto", false)
            .unwrap_err();
        assert!(err.contains("no input"), "got: {err}");
        let err = categorize_spending(
            "Foo,Bar\n1,2\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "both",
            "$",
            "auto",
            false,
        )
        .unwrap_err();
        assert!(
            err.contains("could not find a description column"),
            "got: {err}"
        );
        let err = categorize_spending(
            "Description\nStarbucks\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "both",
            "$",
            "auto",
            false,
        )
        .unwrap_err();
        assert!(err.contains("could not find an amount column"), "got: {err}");
        let err = categorize_spending(
            "Description,Amount\nStarbucks,-1\n",
            "",
            "",
            "",
            "",
            "",
            "bad rule line",
            "both",
            "$",
            "auto",
            false,
        )
        .unwrap_err();
        assert!(err.contains("rule on line 1"), "got: {err}");
        let err = categorize_spending(
            "Description,Amount\nStarbucks,-1\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "chart",
            "$",
            "auto",
            false,
        )
        .unwrap_err();
        assert!(err.contains("unknown output 'chart'"), "got: {err}");
        let err = categorize_spending(
            "Description,Amount\nStarbucks,abc\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "both",
            "$",
            "auto",
            false,
        )
        .unwrap_err();
        assert!(
            err.contains("row 2") && err.contains("could not read an amount"),
            "got: {err}"
        );
    }

    #[test]
    fn no_date_column_drops_the_date_from_csv_output() {
        let out = categorize_spending(
            "Description,Amount\nLIDL,-9.99\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "csv",
            "$",
            "auto",
            false,
        )
        .unwrap();
        assert_eq!(out, "Description,Amount,Category\nLIDL,-9.99,Groceries");
    }

    #[test]
    fn currency_code_is_suffixed_and_quoted_descriptions_survive() {
        let out = categorize_spending(
            "Description,Amount\n\"LIDL, BERLIN\",-10.00\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "summary",
            "EUR",
            "auto",
            false,
        )
        .unwrap();
        assert!(out.contains("10.00 EUR"), "got: {out}");
        let out = categorize_spending(
            "Description,Amount\n\"LIDL, BERLIN\",-10.00\n",
            "",
            "",
            "",
            "",
            "",
            "",
            "csv",
            "$",
            "auto",
            false,
        )
        .unwrap();
        assert!(out.contains("\"LIDL, BERLIN\",-10.00,Groceries"), "got: {out}");
    }
}
